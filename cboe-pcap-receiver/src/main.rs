use anyhow::Result;
use clap::Parser;
use std::collections::HashMap;
use std::net::{IpAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;
use tracing::{error, info, warn, debug};

mod packet;
mod high_performance;
mod high_performance_receiver;

use high_performance_receiver::{FluxOptimizedReceiver, ReceiverStatistics};

#[derive(Parser, Debug)]
#[command(name = "cboe-pcap-receiver")]
#[command(about = "UDP receiver to monitor CBOE packet reception with sequence checking")]
struct Args {
    /// Ports to listen on (comma-separated)
    #[arg(short, long)]
    ports: String,

    /// Interface IP to bind to
    #[arg(short = 'i', long, default_value = "127.0.0.1")]
    interface: String,

    /// Time to run in seconds (0 = infinite)
    #[arg(short, long, default_value = "0")]
    time: u64,

    /// Report interval in seconds
    #[arg(short = 'r', long, default_value = "5")]
    interval: u64,

    /// Verbose logging (default: false)
    #[arg(short, long, default_value = "false")]
    verbose: bool,

    /// Use flux-optimized high-performance receiver (default: false)
    #[arg(long, default_value = "false")]
    flux_optimized: bool,
}

#[derive(Debug, Clone)]
struct PortStats {
    port: u16,
    packets_received: u64,
    bytes_received: u64,
    last_report_time: Instant,
    last_report_packets: u64,
    last_report_bytes: u64,
    _max_gap_ms: u64,
    sequences_seen: HashMap<u8, SequenceTracker>,
    first_packet_time: Option<Instant>,
    last_packet_time: Option<Instant>,
}

#[derive(Debug, Clone)]
struct SequenceTracker {
    unit: u8,
    last_sequence: Option<u32>,
    expected_next: Option<u32>,
    gaps: u64,
    duplicates: u64,
    out_of_order: u64,
    total_messages: u64,
    min_sequence: Option<u32>,
    max_sequence: Option<u32>,
}

impl SequenceTracker {
    fn new(unit: u8) -> Self {
        Self {
            unit,
            last_sequence: None,
            expected_next: None,
            gaps: 0,
            duplicates: 0,
            out_of_order: 0,
            total_messages: 0,
            min_sequence: None,
            max_sequence: None,
        }
    }

    fn process_sequence(&mut self, sequence: u32) {
        self.total_messages += 1;

        // Track min/max
        if self.min_sequence.is_none() || sequence < self.min_sequence.unwrap() {
            self.min_sequence = Some(sequence);
        }
        if self.max_sequence.is_none() || sequence > self.max_sequence.unwrap() {
            self.max_sequence = Some(sequence);
        }

        if let Some(last) = self.last_sequence {
            if sequence == last {
                self.duplicates += 1;
                debug!("Unit {} duplicate sequence: {}", self.unit, sequence);
            } else if sequence < last {
                self.out_of_order += 1;
                debug!("Unit {} out of order: {} (last: {})", self.unit, sequence, last);
            } else if sequence != last + 1 {
                let gap = sequence - last - 1;
                self.gaps += gap as u64;
                debug!("Unit {} sequence gap: {} messages between {} and {}", 
                       self.unit, gap, last, sequence);
            }
        }

        self.last_sequence = Some(sequence);
        self.expected_next = Some(sequence + 1);
    }

    fn get_stats(&self) -> (u64, u64, u64, u64, Option<u32>, Option<u32>) {
        (self.total_messages, self.gaps, self.duplicates, self.out_of_order, 
         self.min_sequence, self.max_sequence)
    }
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
            _max_gap_ms: 0,
            sequences_seen: HashMap::new(),
            first_packet_time: None,
            last_packet_time: None,
        }
    }

    fn update(&mut self, bytes: usize, sequence_unit: Option<u8>, sequence_number: Option<u32>) {
        let now = Instant::now();
        
        if self.first_packet_time.is_none() {
            self.first_packet_time = Some(now);
        }
        
        if let Some(last_time) = self.last_packet_time {
            let gap_ms = now.duration_since(last_time).as_millis() as u64;
            if gap_ms > self._max_gap_ms {
                self._max_gap_ms = gap_ms;
            }
        }
        
        self.last_packet_time = Some(now);
        self.packets_received += 1;
        self.bytes_received += bytes as u64;

        // Process sequence information if available
        if let (Some(unit), Some(sequence)) = (sequence_unit, sequence_number) {
            let tracker = self.sequences_seen.entry(unit).or_insert_with(|| SequenceTracker::new(unit));
            tracker.process_sequence(sequence);
        }
    }

    fn get_rate(&self) -> (f64, f64) {
        let duration = self.last_report_time.elapsed().as_secs_f64();
        if duration == 0.0 {
            return (0.0, 0.0);
        }

        let packet_delta = self.packets_received - self.last_report_packets;
        let byte_delta = self.bytes_received - self.last_report_bytes;
        
        (packet_delta as f64 / duration, byte_delta as f64 / duration)
    }

    fn reset_rate_counters(&mut self) {
        self.last_report_time = Instant::now();
        self.last_report_packets = self.packets_received;
        self.last_report_bytes = self.bytes_received;
    }
}

// Parse CBOE PITCH Sequenced Unit Header
fn parse_sequenced_unit_header(data: &[u8]) -> Option<(u16, u8, u8, u32)> {
    if data.len() < 8 {
        return None;
    }

    let length = u16::from_le_bytes([data[0], data[1]]);
    let count = data[2];
    let unit = data[3];
    let sequence = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    Some((length, count, unit, sequence))
}

// Parse CBOE PITCH message type from payload
fn parse_message_type(data: &[u8]) -> Option<u8> {
    if data.len() < 10 { // 8 bytes header + 2 bytes message minimum
        return None;
    }

    // Skip sequenced unit header (8 bytes) and message length (1 byte)
    Some(data[9]) // Message type is at offset 9
}

fn setup_udp_listener(port: u16, bind_ip: &str) -> Result<UdpSocket> {
    let addr = format!("{}:{}", bind_ip, port);
    let socket = UdpSocket::bind(&addr)?;
    
    // Set socket options for better performance
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;
    
    // Try to increase receive buffer size using socket2
    use std::os::unix::io::AsRawFd;
    unsafe {
        let fd = socket.as_raw_fd();
        let buf_size = 1024 * 1024i32;
        if libc::setsockopt(
            fd, 
            libc::SOL_SOCKET, 
            libc::SO_RCVBUF, 
            &buf_size as *const _ as *const libc::c_void, 
            std::mem::size_of::<i32>() as libc::socklen_t
        ) < 0 {
            warn!("Failed to set large receive buffer for port {}", port);
        }
    }

    info!("Listening on UDP {}:{}", bind_ip, port);
    Ok(socket)
}

fn listen_on_port(port: u16, bind_ip: String, stats: Arc<Mutex<HashMap<u16, PortStats>>>) -> Result<()> {
    let socket = setup_udp_listener(port, &bind_ip)?;
    let mut buffer = vec![0u8; 65536]; // Max UDP packet size

    loop {
        match socket.recv_from(&mut buffer) {
            Ok((size, src)) => {
                debug!("Received {} bytes from {} on port {}", size, src, port);
                
                // Parse CBOE PITCH header if present
                let (unit, sequence) = if let Some((_, _, unit, sequence)) = parse_sequenced_unit_header(&buffer[..size]) {
                    (Some(unit), Some(sequence))
                } else {
                    (None, None)
                };

                // Log message type for verbose mode
                if let Some(msg_type) = parse_message_type(&buffer[..size]) {
                    debug!("Port {} received message type 0x{:02X} (unit: {:?}, seq: {:?})", 
                           port, msg_type, unit, sequence);
                }

                // Update statistics
                if let Ok(mut stats_map) = stats.lock() {
                    let port_stats = stats_map.entry(port).or_insert_with(|| PortStats::new(port));
                    port_stats.update(size, unit, sequence);
                }
            }
            Err(e) => {
                match e.kind() {
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
                        // Normal timeout, continue listening
                        continue;
                    }
                    _ => {
                        error!("Error receiving on port {}: {}", port, e);
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        }
    }
}

fn print_statistics(stats: &Arc<Mutex<HashMap<u16, PortStats>>>) {
    if let Ok(mut stats_map) = stats.lock() {
        println!("\n=== CBOE Packet Reception Statistics ===");
        println!("{:<8} {:<12} {:<12} {:<15} {:<15} {:<12} {:<8}", 
                "Port", "Packets", "Bytes", "Pkt/sec", "MB/sec", "Max Gap(ms)", "Units");
        println!("{}", "─".repeat(90));

        let mut total_packets = 0u64;
        let mut total_bytes = 0u64;

        for (port, port_stats) in stats_map.iter_mut() {
            let (pkt_rate, byte_rate) = port_stats.get_rate();
            let mb_rate = byte_rate / (1024.0 * 1024.0);
            
            println!("{:<8} {:<12} {:<12} {:<15.1} {:<15.3} {:<12} {:<8}",
                    port,
                    port_stats.packets_received,
                    port_stats.bytes_received,
                    pkt_rate,
                    mb_rate,
                    port_stats._max_gap_ms,
                    port_stats.sequences_seen.len());

            total_packets += port_stats.packets_received;
            total_bytes += port_stats.bytes_received;
            
            // Print sequence analysis for each unit on this port
            for (unit, tracker) in &port_stats.sequences_seen {
                let (total, gaps, dups, ooo, min_seq, max_seq) = tracker.get_stats();
                println!("  └─ Unit {}: {} msgs, {} gaps, {} dups, {} OOO, seq range: {:?} - {:?}",
                        unit, total, gaps, dups, ooo, min_seq, max_seq);
            }

            port_stats.reset_rate_counters();
        }

        println!("{}", "─".repeat(90));
        println!("Total: {} packets, {} bytes ({:.1} MB)", 
                total_packets, total_bytes, total_bytes as f64 / (1024.0 * 1024.0));
        
        if !stats_map.is_empty() {
            let avg_rate = stats_map.values()
                .map(|s| s.get_rate().0)
                .sum::<f64>() / stats_map.len() as f64;
            println!("Average rate: {:.1} packets/sec across {} ports", avg_rate, stats_map.len());
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .init();

    let ports: Vec<u16> = args.ports
        .split(',')
        .map(|s| s.trim().parse())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("Invalid port number: {}", e))?;

    if ports.is_empty() {
        return Err(anyhow::anyhow!("At least one port must be specified"));
    }

    let bind_ip: IpAddr = args.interface.parse()
        .map_err(|e| anyhow::anyhow!("Invalid IP address: {}", e))?;

    info!("Starting CBOE Packet Receiver ({})", 
          if args.flux_optimized { "Flux-Optimized" } else { "Standard" });
    info!("Listening on interface {} for {} ports: {:?}", args.interface, ports.len(), ports);

    if args.flux_optimized {
        run_flux_optimized_receiver(bind_ip, ports, &args).await
    } else {
        run_standard_receiver(bind_ip, ports, &args).await
    }
}

async fn run_flux_optimized_receiver(bind_ip: IpAddr, ports: Vec<u16>, args: &Args) -> Result<()> {
    info!("Initializing flux-optimized high-performance receiver");
    
    // Create flux-optimized receiver
    let receiver = Arc::new(FluxOptimizedReceiver::new(bind_ip, ports)?);
    
    // Setup graceful shutdown
    let receiver_for_shutdown = receiver.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("Received Ctrl+C, shutting down flux-optimized receiver");
        receiver_for_shutdown.stop();
    });
    
    // Start receiver workers
    let worker_handles = receiver.start()?;
    
    // Statistics reporting loop
    let start_time = Instant::now();
    let run_duration = if args.time > 0 { 
        Some(Duration::from_secs(args.time)) 
    } else { 
        None 
    };
    
    let mut last_stats = receiver.get_stats();
    let mut last_time = Instant::now();
    
    loop {
        tokio::time::sleep(Duration::from_secs(args.interval)).await;
        
        // Check if we should stop
        if let Some(duration) = run_duration {
            if start_time.elapsed() >= duration {
                info!("Run duration ({} seconds) completed", args.time);
                break;
            }
        }
        
        // Print flux-optimized statistics
        let current_stats = receiver.get_stats();
        print_flux_statistics(&current_stats, &last_stats, last_time.elapsed().as_secs_f64());
        
        last_stats = current_stats;
        last_time = Instant::now();
    }
    
    // Stop receiver
    receiver.stop();
    
    // Wait for worker threads
    for (i, handle) in worker_handles.into_iter().enumerate() {
        if let Err(e) = handle.join() {
            warn!("Error joining worker thread {}: {:?}", i, e);
        }
    }
    
    // Final statistics
    let final_stats = receiver.get_stats();
    print_final_flux_statistics(&final_stats, start_time.elapsed().as_secs_f64());
    
    info!("Flux-optimized CBOE Packet Receiver completed");
    Ok(())
}

async fn run_standard_receiver(_bind_ip: IpAddr, ports: Vec<u16>, args: &Args) -> Result<()> {
    info!("Using standard receiver implementation");
    
    // Setup statistics tracking
    let stats = Arc::new(Mutex::new(HashMap::new()));
    
    // Initialize stats for each port
    {
        let mut stats_map = stats.lock().unwrap();
        for &port in &ports {
            stats_map.insert(port, PortStats::new(port));
        }
    }

    // Spawn listener threads for each port
    let mut handles = vec![];
    for port in ports {
        let stats_clone = Arc::clone(&stats);
        let bind_ip = args.interface.clone();
        
        let handle = thread::spawn(move || {
            if let Err(e) = listen_on_port(port, bind_ip, stats_clone) {
                error!("Listener for port {} failed: {}", port, e);
            }
        });
        handles.push(handle);
    }

    // Setup Ctrl+C handler
    let stats_for_shutdown = Arc::clone(&stats);
    ctrlc::set_handler(move || {
        info!("Received Ctrl+C, shutting down");
        print_statistics(&stats_for_shutdown);
        std::process::exit(0);
    })?;

    // Statistics reporting loop
    let start_time = Instant::now();
    let run_duration = if args.time > 0 { 
        Some(Duration::from_secs(args.time)) 
    } else { 
        None 
    };

    loop {
        thread::sleep(Duration::from_secs(args.interval));
        
        // Check if we should stop
        if let Some(duration) = run_duration {
            if start_time.elapsed() >= duration {
                info!("Run duration ({} seconds) completed", args.time);
                break;
            }
        }

        print_statistics(&stats);
    }

    // Final statistics
    info!("CBOE Packet Receiver completed");
    print_statistics(&stats);

    Ok(())
}

/// Print flux-optimized statistics
fn print_flux_statistics(current: &ReceiverStatistics, last: &ReceiverStatistics, elapsed: f64) {
    println!("\n=== FLUX-OPTIMIZED CBOE Receiver Statistics ===");
    
    // Calculate rates
    let packet_rate = ((current.total_received - last.total_received) as f64 / elapsed).round() as u64;
    let byte_rate = ((current.total_bytes - last.total_bytes) as f64 / elapsed).round() as u64;
    let mb_rate = byte_rate as f64 / (1024.0 * 1024.0);
    
    println!("Overall Performance:");
    println!("  Total Received: {} packets ({} bytes)", current.total_received, current.total_bytes);
    println!("  Rate: {} pps, {:.2} MB/s", packet_rate, mb_rate);
    println!("  Processed: {} packets", current.total_processed);
    println!("  Errors: {}", current.errors);
    
    println!("\nSequence Analysis:");
    println!("  Gaps: {}, Duplicates: {}, Out-of-Order: {}", 
             current.sequence_gaps, current.duplicates, current.out_of_order);
    
    println!("\nFlux Optimizations:");
    println!("  Ring Buffer Utilization: {:.1}%", current.ring_buffer_utilization);
    println!("  Memory Pool Efficiency: {:.1}%", current.memory_pool_efficiency);
    println!("  Batch Size: {}, SIMD: {}", current.batch_size, current.simd_enabled);
    
    println!("\nPer-Port Statistics (with Performance Metrics):");
    println!("{:<8} {:<12} {:<12} {:<8} {:<8} {:<8} {:<12} {:<12}", 
             "Port", "Packets", "Bytes", "Gaps", "Dups", "OOO", "Avg PPS", "Peak PPS");
    println!("{}", "─".repeat(90));
    
    for (port, port_stats) in &current.port_statistics {
        println!("{:<8} {:<12} {:<12} {:<8} {:<8} {:<8} {:<12.0} {:<12.0}",
                port,
                port_stats.packets_received,
                port_stats.bytes_received,
                port_stats.gaps_detected,
                port_stats.duplicates,
                port_stats.out_of_order,
                port_stats.avg_throughput_pps,
                port_stats.peak_throughput_pps);
        
        // Show performance percentiles
        if port_stats.packets_received > 100 {
            println!("    ├─ Latency (μs): p50={} p55={} p95={} p99={} max={}",
                    port_stats.latency_p50_us,
                    port_stats.latency_p55_us,
                    port_stats.latency_p95_us,
                    port_stats.latency_p99_us,
                    port_stats.latency_max_us);
            println!("    ├─ Processing (ns): p50={} p55={} p95={} p99={} max={}",
                    port_stats.processing_p50_ns,
                    port_stats.processing_p55_ns,
                    port_stats.processing_p95_ns,
                    port_stats.processing_p99_ns,
                    port_stats.processing_max_ns);
        }
        
        // Show unit sequences for detailed analysis
        if !port_stats.unit_sequences.is_empty() {
            for (unit, seq) in &port_stats.unit_sequences {
                println!("    └─ Unit {}: sequence {}", unit, seq);
            }
        }
    }
}

/// Print final flux-optimized statistics
fn print_final_flux_statistics(stats: &ReceiverStatistics, total_time: f64) {
    println!("\n=== FINAL FLUX-OPTIMIZED STATISTICS ===");
    println!("Total Runtime: {:.2} seconds", total_time);
    println!("Total Packets: {}", stats.total_received);
    println!("Total Bytes: {} ({:.2} MB)", stats.total_bytes, stats.total_bytes as f64 / (1024.0 * 1024.0));
    println!("Average Rate: {:.0} pps", stats.total_received as f64 / total_time);
    println!("Sequence Issues: {} gaps, {} duplicates, {} out-of-order", 
             stats.sequence_gaps, stats.duplicates, stats.out_of_order);
    println!("Final Ring Buffer Utilization: {:.1}%", stats.ring_buffer_utilization);
    println!("Final Memory Pool Efficiency: {:.1}%", stats.memory_pool_efficiency);
}