use anyhow::Result;
use clap::Parser;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn, debug};
use std::ffi::CString;
use pnet::packet::ethernet::{EthernetPacket, EtherTypes};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::udp::UdpPacket;
use pnet::packet::Packet;

#[derive(Parser, Debug)]
#[command(name = "cboe-pcap-receiver")]
#[command(about = "BPF-based UDP receiver to verify CBOE packet reception with sequence checking")]
struct Args {
    /// Ports to listen on (comma-separated)
    #[arg(short, long)]
    ports: String,

    /// Interface to listen on
    #[arg(short = 'i', long, default_value = "lo0")]
    interface: String,

    /// Time to run in seconds (0 = infinite)
    #[arg(short, long, default_value = "0")]
    time: u64,

    /// Report interval in seconds
    #[arg(short = 't', long, default_value = "1")]
    interval: u64,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone)]
struct PortStats {
    port: u16,
    packets_received: u64,
    bytes_received: u64,
    last_report_time: Instant,
    last_report_packets: u64,
    last_report_bytes: u64,
    max_gap_ms: u64,
    min_sequence: Option<u32>,
    max_sequence: Option<u32>,
    last_packet_time: Option<Instant>,
    // Store all detected sequence numbers 
    sequence_numbers: HashMap<u32, bool>,
    last_sequence: Option<u32>,
    missing_sequences: u64,
    out_of_order_sequences: u64,
    duplicate_sequences: u64,
}

impl PortStats {
    fn new(port: u16) -> Self {
        Self {
            port,
            packets_received: 0,
            bytes_received: 0,
            last_report_time: Instant::now(),
            last_report_packets: 0,
            last_report_bytes: 0,
            max_gap_ms: 0,
            min_sequence: None,
            max_sequence: None,
            last_packet_time: None,
            sequence_numbers: HashMap::new(),
            last_sequence: None,
            missing_sequences: 0,
            out_of_order_sequences: 0,
            duplicate_sequences: 0,
        }
    }

    fn record_packet(&mut self, size: usize, data: &[u8]) {
        self.packets_received += 1;
        self.bytes_received += size as u64;

        let now = Instant::now();

        // Track packet timing
        if let Some(last_time) = self.last_packet_time {
            let gap = now.duration_since(last_time).as_millis() as u64;
            if gap > self.max_gap_ms {
                self.max_gap_ms = gap;
            }
        }
        self.last_packet_time = Some(now);

        // Check for sequence number (first 4 bytes are sequence)
        if data.len() >= 4 {
            let seq = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            
            // Update min/max sequence seen
            if let Some(min_seq) = self.min_sequence {
                self.min_sequence = Some(min_seq.min(seq));
            } else {
                self.min_sequence = Some(seq);
            }
            
            if let Some(max_seq) = self.max_sequence {
                self.max_sequence = Some(max_seq.max(seq));
            } else {
                self.max_sequence = Some(seq);
            }

            // Check for duplicates
            if self.sequence_numbers.contains_key(&seq) {
                self.duplicate_sequences += 1;
                debug!("Port {}: Duplicate sequence {} detected", self.port, seq);
            }
            
            // Record the sequence
            self.sequence_numbers.insert(seq, true);

            // Check for missing or out-of-order sequences
            if let Some(last_seq) = self.last_sequence {
                if seq < last_seq {
                    // Out of order sequence
                    self.out_of_order_sequences += 1;
                    debug!("Port {}: Out-of-order sequence {} received after {}", 
                          self.port, seq, last_seq);
                } else if seq > last_seq + 1 {
                    // Missing sequences detected
                    let missing = seq - last_seq - 1;
                    let missing_u64: u64 = u64::from(missing);
                    self.missing_sequences += missing_u64;
                    debug!("Port {}: Missing {} sequence(s) between {} and {}", 
                          self.port, missing, last_seq, seq);
                }
            }
            self.last_sequence = Some(seq);
        }
    }

    fn get_report(&mut self) -> String {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_report_time).as_secs_f64();
        
        let packet_rate = (self.packets_received - self.last_report_packets) as f64 / elapsed;
        let bytes_rate = (self.bytes_received - self.last_report_bytes) as f64 / elapsed;
        
        // Calculate sequence statistics
        let total_expected = if let (Some(min), Some(max)) = (self.min_sequence, self.max_sequence) {
            if max >= min {
                (max - min + 1) as u64
            } else {
                0
            }
        } else {
            0
        };
        
        let total_received = self.sequence_numbers.len() as u64;
        let completeness = if total_expected > 0 {
            (total_received as f64 / total_expected as f64) * 100.0
        } else {
            0.0
        };
        
        let report = format!(
            "Port {:5}: {:8} packets ({:6.0} pps), {:10} bytes ({:6.2} MB/s), max gap: {} ms\n  \
             Sequence range: {:?}-{:?}, completeness: {:.2}%, missing: {}, out-of-order: {}, duplicates: {}",
            self.port,
            self.packets_received,
            packet_rate,
            self.bytes_received,
            bytes_rate / 1_048_576.0,
            self.max_gap_ms,
            self.min_sequence.unwrap_or(0),
            self.max_sequence.unwrap_or(0),
            completeness,
            self.missing_sequences,
            self.out_of_order_sequences,
            self.duplicate_sequences
        );
        
        self.last_report_time = now;
        self.last_report_packets = self.packets_received;
        self.last_report_bytes = self.bytes_received;
        
        report
    }
}

// BPF constants
const BIOCSETIF: libc::c_ulong = 0x8020426c;
const BIOCGBLEN: libc::c_ulong = 0x40044266;
const BIOCSETBLEN: libc::c_ulong = 0xc0044266;
const BIOCIMMEDIATE: libc::c_ulong = 0x80044270;
const BIOCSDLT: libc::c_ulong = 0x80044278;
const BIOCPROMISC: libc::c_ulong = 0x20004269;
const BIOCSETF: libc::c_ulong = 0x80084267;

// BPF instruction
#[repr(C)]
struct BpfInsn {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

// BPF program
#[repr(C)]
struct BpfProgram {
    bf_len: u32,
    bf_insns: *const BpfInsn,
}

// BPF header
#[repr(C)]
struct BpfHdr {
    bh_tstamp: libc::timeval,
    bh_caplen: u32,
    bh_datalen: u32,
    bh_hdrlen: u16,
}

// BPF device wrapper
struct BpfDevice {
    fd: i32,
    buffer: Vec<u8>,
    buffer_len: usize,
}

impl BpfDevice {
    fn open(interface: &str, buffer_size: usize, ports: &[u16]) -> Result<Self> {
        // Find available BPF device
        let mut fd = -1;
        for i in 0..99 {
            let dev_name = format!("/dev/bpf{}", i);
            fd = unsafe { libc::open(CString::new(dev_name.clone())?.as_ptr(), libc::O_RDWR) };
            if fd >= 0 {
                info!("Opened BPF device: {}", dev_name);
                break;
            }
        }
        
        if fd < 0 {
            return Err(anyhow::anyhow!("Failed to open BPF device"));
        }
        
        // Set buffer length
        let mut buf_len: u32 = buffer_size as u32;
        if unsafe { libc::ioctl(fd, BIOCSETBLEN, &mut buf_len) } < 0 {
            unsafe { libc::close(fd) };
            return Err(anyhow::anyhow!("Failed to set BPF buffer length"));
        }
        
        // Get actual buffer length
        let mut actual_len: u32 = 0;
        if unsafe { libc::ioctl(fd, BIOCGBLEN, &mut actual_len) } < 0 {
            unsafe { libc::close(fd) };
            return Err(anyhow::anyhow!("Failed to get BPF buffer length"));
        }
        
        info!("BPF buffer size: {} bytes", actual_len);
        
        // Bind to interface
        let ifreq = unsafe {
            let mut req: libc::ifreq = std::mem::zeroed();
            let if_name = CString::new(interface)?;
            let if_name_bytes = if_name.as_bytes_with_nul();
            
            if if_name_bytes.len() > req.ifr_name.len() {
                return Err(anyhow::anyhow!("Interface name too long"));
            }
            
            std::ptr::copy_nonoverlapping(
                if_name_bytes.as_ptr(),
                req.ifr_name.as_mut_ptr() as *mut u8,
                if_name_bytes.len()
            );
            
            req
        };
        
        if unsafe { libc::ioctl(fd, BIOCSETIF, &ifreq) } < 0 {
            unsafe { libc::close(fd) };
            return Err(anyhow::anyhow!("Failed to bind BPF to interface: {}", interface));
        }
        
        // Enable immediate mode (don't wait for buffer to fill)
        let immediate: u32 = 1;
        if unsafe { libc::ioctl(fd, BIOCIMMEDIATE, &immediate) } < 0 {
            unsafe { libc::close(fd) };
            return Err(anyhow::anyhow!("Failed to set immediate mode"));
        }
        
        // Create BPF filter program to capture only UDP packets for specific ports
        let filter = build_bpf_filter(ports)?;
        
        // Apply the filter
        if unsafe { libc::ioctl(fd, BIOCSETF, &filter) } < 0 {
            unsafe { libc::close(fd) };
            return Err(anyhow::anyhow!("Failed to set BPF filter"));
        }
        
        info!("BPF filter set to capture UDP packets for ports: {:?}", ports);
        
        Ok(Self {
            fd,
            buffer: vec![0u8; actual_len as usize],
            buffer_len: actual_len as usize,
        })
    }
    
    fn read_packet(&mut self) -> Result<Option<(Vec<u8>, u16)>> {
        // Read from BPF device
        let n = unsafe {
            libc::read(self.fd, self.buffer.as_mut_ptr() as *mut libc::c_void, self.buffer_len)
        };
        
        if n < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::WouldBlock {
                return Ok(None);
            }
            return Err(anyhow::anyhow!("Failed to read from BPF: {}", err));
        }
        
        if n == 0 {
            return Ok(None);
        }
        
        // Process the packet
        let mut offset = 0;
        while offset < n as usize {
            let hdr = unsafe {
                &*(self.buffer.as_ptr().add(offset) as *const BpfHdr)
            };
            
            let packet_start = offset + hdr.bh_hdrlen as usize;
            let packet_data = &self.buffer[packet_start..packet_start + hdr.bh_caplen as usize];
            
            // Parse Ethernet header
            if let Some(eth) = EthernetPacket::new(packet_data) {
                if eth.get_ethertype() == EtherTypes::Ipv4 {
                    // Parse IPv4 header
                    if let Some(ipv4) = Ipv4Packet::new(eth.payload()) {
                        if ipv4.get_next_level_protocol() == IpNextHeaderProtocols::Udp {
                            // Parse UDP header
                            if let Some(udp) = UdpPacket::new(ipv4.payload()) {
                                let dest_port = udp.get_destination();
                                let payload = udp.payload().to_vec();
                                
                                return Ok(Some((payload, dest_port)));
                            }
                        }
                    }
                }
            }
            
            // Move to next packet in buffer
            offset += BPF_WORDALIGN(hdr.bh_hdrlen as usize + hdr.bh_caplen as usize);
        }
        
        Ok(None)
    }
}

// Word-align for BPF
const fn BPF_WORDALIGN(x: usize) -> usize {
    (x + 3) & !3
}

impl Drop for BpfDevice {
    fn drop(&mut self) {
        if self.fd >= 0 {
            unsafe { libc::close(self.fd) };
        }
    }
}

// Build BPF filter program to capture UDP packets for specific ports
fn build_bpf_filter(ports: &[u16]) -> Result<BpfProgram> {
    // BPF instructions for: "udp and (dst port PORT1 or dst port PORT2 ...)"
    // This is a simplified version - a complete implementation would involve
    // translating BPF filter expressions to actual instructions
    
    // For educational purposes, we're creating a simple UDP filter with port checks
    // In a production environment, you'd use libpcap's pcap_compile to create this
    
    // Basic UDP filter instructions (matches UDP traffic)
    // - Check if it's an IP packet (12 bytes in, ethertype = 0x0800)
    // - Check if protocol is UDP (23 bytes in, protocol = 17)
    // - Check destination port against each port in our list
    
    let mut instructions: Vec<BpfInsn> = Vec::new();
    
    // Check for IP (Ethertype = 0x0800)
    instructions.push(BpfInsn {
        code: 0x28, // BPF_LD|BPF_H|BPF_ABS
        jt: 0,
        jf: 0,
        k: 12, // Ethertype offset
    });
    instructions.push(BpfInsn {
        code: 0x15, // BPF_JEQ|BPF_K
        jt: 0,
        jf: (ports.len() * 2 + 3) as u8, // Skip to the end if not IP
        k: 0x0800, // Ethertype for IPv4
    });
    
    // Check for UDP (Protocol = 17)
    instructions.push(BpfInsn {
        code: 0x30, // BPF_LD|BPF_B|BPF_ABS
        jt: 0,
        jf: 0,
        k: 23, // Protocol offset in IPv4
    });
    instructions.push(BpfInsn {
        code: 0x15, // BPF_JEQ|BPF_K
        jt: 0,
        jf: (ports.len() * 2 + 1) as u8, // Skip to the end if not UDP
        k: 17, // Protocol number for UDP
    });
    
    // Load UDP destination port (offset depends on IPv4 header length)
    instructions.push(BpfInsn {
        code: 0x30, // BPF_LD|BPF_B|BPF_ABS
        jt: 0,
        jf: 0,
        k: 14, // IPv4 header offset to get IHL
    });
    instructions.push(BpfInsn {
        code: 0x54, // BPF_ALU|BPF_AND|BPF_K
        jt: 0,
        jf: 0,
        k: 0x0f, // Mask for IHL
    });
    instructions.push(BpfInsn {
        code: 0x04, // BPF_ALU|BPF_MUL|BPF_K
        jt: 0,
        jf: 0,
        k: 4, // IHL is in 32-bit words
    });
    instructions.push(BpfInsn {
        code: 0x07, // BPF_ALU|BPF_ADD|BPF_K
        jt: 0,
        jf: 0,
        k: 14, // Add Ethernet header length
    });
    instructions.push(BpfInsn {
        code: 0x02, // BPF_ST
        jt: 0,
        jf: 0,
        k: 0, // Store to scratch memory
    });
    
    // Check destination port
    instructions.push(BpfInsn {
        code: 0x60, // BPF_LD|BPF_MEM
        jt: 0,
        jf: 0,
        k: 0, // Load from scratch memory
    });
    instructions.push(BpfInsn {
        code: 0x07, // BPF_ALU|BPF_ADD|BPF_K
        jt: 0,
        jf: 0,
        k: 2, // Add offset to destination port field
    });
    instructions.push(BpfInsn {
        code: 0xb1, // BPF_LDX|BPF_MSH|BPF_B
        jt: 0,
        jf: 0,
        k: 14, // Load IP header length
    });
    instructions.push(BpfInsn {
        code: 0x48, // BPF_LD|BPF_W|BPF_IMM
        jt: 0,
        jf: 0,
        k: 0, // Reset accumulator
    });
    
    // For each port, add a check
    for port in ports {
        instructions.push(BpfInsn {
            code: 0x40, // BPF_LD|BPF_H|BPF_IND
            jt: 0,
            jf: 0,
            k: 0, // Load UDP destination port (indexed)
        });
        instructions.push(BpfInsn {
            code: 0x15, // BPF_JEQ|BPF_K
            jt: 3, // Jump to "return true" if matches
            jf: 0, // Continue to next check
            k: *port as u32, // Port to match
        });
    }
    
    // Default return false (if no port matches)
    instructions.push(BpfInsn {
        code: 0x06, // BPF_RET|BPF_K
        jt: 0,
        jf: 0,
        k: 0, // Return 0 (false)
    });
    
    // Return true (with full packet length)
    instructions.push(BpfInsn {
        code: 0x06, // BPF_RET|BPF_K
        jt: 0,
        jf: 0,
        k: 0xffff, // Return entire packet
    });
    
    // Create BPF program
    let program = BpfProgram {
        bf_len: instructions.len() as u32,
        bf_insns: Box::leak(instructions.into_boxed_slice()).as_ptr(),
    };
    
    Ok(program)
}

async fn setup_bpf_receiver(
    interface: &str,
    ports: &[u16],
    stats_tx: mpsc::Sender<(u16, PortStats)>,
) -> Result<()> {
    // Check if running as root (BPF typically requires root privileges)
    #[cfg(unix)]
    {
        // Check if running as root using libc function
        let euid = unsafe { libc::geteuid() };
        if euid != 0 {
            warn!("Not running as root (EUID: {}). BPF access typically requires root privileges.", euid);
            warn!("Try running with sudo if you encounter permission errors.");
        }
    }
    
    // Open BPF device and set up filters
    let mut bpf = BpfDevice::open(interface, 1024 * 1024, ports)?; // 1MB buffer
    
    info!("BPF receiver started on interface {} for ports: {:?}", interface, ports);
    
    // Create stats objects for each port
    let mut port_stats: HashMap<u16, PortStats> = HashMap::new();
    for &port in ports {
        port_stats.insert(port, PortStats::new(port));
    }
    
    // Main receive loop
    loop {
        match bpf.read_packet() {
            Ok(Some((payload, dest_port))) => {
                // Get or create stats for this port
                let stats = port_stats.entry(dest_port).or_insert_with(|| PortStats::new(dest_port));
                
                // Record the packet
                stats.record_packet(payload.len(), &payload);
                
                // Send updated stats
                if let Err(e) = stats_tx.send((dest_port, stats.clone())).await {
                    error!("Failed to send stats: {}", e);
                }
            }
            Ok(None) => {
                // No packet available, wait a bit
                tokio::time::sleep(Duration::from_micros(100)).await;
            }
            Err(e) => {
                error!("Error reading packet: {}", e);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
}

async fn stats_reporter(
    mut stats_rx: mpsc::Receiver<(u16, PortStats)>,
    interval: u64,
) {
    let mut port_stats: HashMap<u16, PortStats> = HashMap::new();
    let mut last_report = Instant::now();
    
    while let Some((port, stats)) = stats_rx.recv().await {
        port_stats.insert(port, stats);
        
        let now = Instant::now();
        if now.duration_since(last_report).as_secs() >= interval {
            // Report all port statistics
            info!("=== Packet Reception Report ===");
            
            let mut total_packets = 0;
            let mut total_bytes = 0;
            let mut total_missing = 0;
            let mut total_out_of_order = 0;
            let mut total_duplicates = 0;
            
            // Get reports from all ports
            let mut reports = Vec::new();
            for (_, stats) in port_stats.iter_mut() {
                reports.push(stats.get_report());
                total_packets += stats.packets_received;
                total_bytes += stats.bytes_received;
                total_missing += stats.missing_sequences;
                total_out_of_order += stats.out_of_order_sequences;
                total_duplicates += stats.duplicate_sequences;
            }
            
            // Sort reports by port
            reports.sort();
            for report in reports {
                info!("{}", report);
            }
            
            // Show summary
            info!(
                "TOTAL   : {:8} packets, {:10} bytes, missing: {}, out-of-order: {}, duplicates: {}",
                total_packets,
                total_bytes,
                total_missing,
                total_out_of_order,
                total_duplicates
            );
            info!("==============================");
            
            last_report = now;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse arguments
    let args = Args::parse();
    
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(if args.verbose { 
            tracing::Level::DEBUG 
        } else { 
            tracing::Level::INFO 
        })
        .init();
    
    info!("Starting CBOE Packet Receiver with BPF capture");
    
    // Parse ports
    let ports: Vec<u16> = args.ports
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    
    if ports.is_empty() {
        error!("No valid ports specified");
        return Ok(());
    }
    
    info!("Capturing UDP traffic on interface {} for {} ports: {:?}", args.interface, ports.len(), ports);
    
    // Create stats channel
    let (stats_tx, stats_rx) = mpsc::channel(1000);
    
    // Start stats reporter
    let reporter_handle = tokio::spawn(stats_reporter(stats_rx, args.interval));
    
    // Start BPF receiver
    let receiver_handle = tokio::spawn(async move {
        if let Err(e) = setup_bpf_receiver(&args.interface, &ports, stats_tx).await {
            error!("BPF receiver error: {}", e);
        }
    });
    
    // Setup timeout if specified
    if args.time > 0 {
        tokio::time::sleep(Duration::from_secs(args.time)).await;
        info!("Time limit reached, shutting down");
    } else {
        // Wait for Ctrl+C
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        
        // Use a separate variable for the handler
        let shutdown_sender = Arc::new(Mutex::new(Some(shutdown_tx)));
        let shutdown_sender_clone = shutdown_sender.clone();
        
        ctrlc::set_handler(move || {
            if let Some(tx) = shutdown_sender_clone.lock().unwrap().take() {
                let _ = tx.send(());
            }
        })?;
        
        shutdown_rx.await?;
        info!("Received Ctrl+C, shutting down");
    }
    
    // Clean shutdown
    receiver_handle.abort();
    reporter_handle.abort();
    
    info!("CBOE Packet Receiver completed");
    Ok(())
}