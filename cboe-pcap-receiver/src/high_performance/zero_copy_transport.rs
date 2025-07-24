use std::sync::Arc;
use std::net::{IpAddr, SocketAddr};
use socket2::{Socket, Domain, Type, Protocol};
use memmap2::MmapOptions;
use std::fs::File;
use anyhow::Result;
use crate::packet::PacketData;

/// Zero-copy transport layer for maximum performance
pub struct ZeroCopyTransport {
    socket: Socket,
    target_addr: SocketAddr,
    send_buffer: Arc<[u8]>,
    use_mmap: bool,
}

impl ZeroCopyTransport {
    /// Create new zero-copy transport
    pub fn new(target_ip: IpAddr, target_port: u16, buffer_size: usize) -> Result<Self> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        
        // Configure socket for zero-copy operations
        Self::configure_zero_copy_socket(&socket, buffer_size)?;
        
        let target_addr = SocketAddr::new(target_ip, target_port);
        let send_buffer = vec![0u8; buffer_size].into();

        Ok(Self {
            socket,
            target_addr,
            send_buffer,
            use_mmap: false,
        })
    }

    /// Create zero-copy transport with memory-mapped buffer
    pub fn with_mmap(
        target_ip: IpAddr, 
        target_port: u16, 
        mmap_file: File,
        mmap_size: usize
    ) -> Result<Self> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        Self::configure_zero_copy_socket(&socket, mmap_size)?;
        
        // Memory map the file for zero-copy operations
        let mmap = unsafe {
            MmapOptions::new()
                .len(mmap_size)
                .map(&mmap_file)?
        };
        
        let target_addr = SocketAddr::new(target_ip, target_port);
        let send_buffer: Arc<[u8]> = mmap.as_ref().into();

        Ok(Self {
            socket,
            target_addr,
            send_buffer,
            use_mmap: true,
        })
    }

    /// Configure socket for zero-copy operations
    fn configure_zero_copy_socket(socket: &Socket, buffer_size: usize) -> Result<()> {
        // Set large send buffer
        socket.set_send_buffer_size(buffer_size)?;
        
        // Enable reuse for high-performance
        socket.set_reuse_address(true)?;
        socket.set_reuse_port(true)?;
        
        // Disable Nagle's algorithm for low latency
        socket.set_nodelay(true)?;
        
        // Platform-specific optimizations
        #[cfg(target_os = "linux")]
        {
            // Enable zero-copy sending on Linux
            use std::os::unix::io::AsRawFd;
            let fd = socket.as_raw_fd();
            
            // Try to set SO_ZEROCOPY
            let zerocopy: libc::c_int = 1;
            unsafe {
                libc::setsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_ZEROCOPY,
                    &zerocopy as *const _ as *const libc::c_void,
                    std::mem::size_of_val(&zerocopy) as libc::socklen_t,
                );
            }
        }
        
        Ok(())
    }

    /// Zero-copy send using direct buffer access
    pub fn send_zero_copy(&self, data: &[u8]) -> Result<usize> {
        // For true zero-copy, we would use sendfile() or similar
        // For now, use optimized socket send
        match self.socket.send_to(data, &self.target_addr.into()) {
            Ok(bytes_sent) => Ok(bytes_sent),
            Err(e) => Err(anyhow::anyhow!("Zero-copy send failed: {}", e)),
        }
    }

    /// Batch zero-copy send for multiple packets
    pub fn send_batch_zero_copy(&self, packets: &[PacketData]) -> Result<usize> {
        let mut total_sent = 0;
        
        // Use vectored I/O for batch sending when available
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            use libc::{iovec, msghdr, sendmsg};
            
            // Prepare iovecs for vectored I/O
            let mut iovecs: Vec<iovec> = Vec::with_capacity(packets.len().min(64));
            for packet in packets.iter().take(64) {
                iovecs.push(iovec {
                    iov_base: packet.payload.as_ptr() as *mut libc::c_void,
                    iov_len: packet.payload.len(),
                });
            }
            
            if !iovecs.is_empty() {
                let msg = msghdr {
                    msg_name: &self.target_addr as *const _ as *mut libc::c_void,
                    msg_namelen: std::mem::size_of::<SocketAddr>() as u32,
                    msg_iov: iovecs.as_mut_ptr(),
                    msg_iovlen: iovecs.len(),  // Sử dụng kiểu mặc định của trường này
                    msg_control: std::ptr::null_mut(),
                    msg_controllen: 0,
                    msg_flags: 0,
                };
                
                unsafe {
                    let result = sendmsg(self.socket.as_raw_fd(), &msg, 0);
                    if result >= 0 {
                        total_sent = iovecs.len();
                    }
                }
            }
        }
        
        // Fallback for non-Unix or if vectored I/O failed
        #[cfg(not(unix))]
        {
            for packet in packets {
                if self.send_zero_copy(&packet.payload).is_ok() {
                    total_sent += 1;
                }
            }
        }
        
        Ok(total_sent)
    }

    /// Memory-mapped send for large data transfers
    pub fn send_mmap_region(&self, offset: usize, length: usize) -> Result<usize> {
        if !self.use_mmap {
            return Err(anyhow::anyhow!("Memory mapping not enabled"));
        }
        
        if offset + length > self.send_buffer.len() {
            return Err(anyhow::anyhow!("Offset and length exceed buffer size"));
        }
        
        let data = &self.send_buffer[offset..offset + length];
        self.send_zero_copy(data)
    }

    /// Get transport statistics
    pub fn get_stats(&self) -> ZeroCopyStats {
        ZeroCopyStats {
            buffer_size: self.send_buffer.len(),
            use_mmap: self.use_mmap,
            target_addr: self.target_addr,
            zerocopy_enabled: self.is_zerocopy_enabled(),
        }
    }

    /// Check if zero-copy is actually enabled
    fn is_zerocopy_enabled(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::io::AsRawFd;
            let fd = self.socket.as_raw_fd();
            let mut zerocopy: libc::c_int = 0;
            let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
            
            unsafe {
                let result = libc::getsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_ZEROCOPY,
                    &mut zerocopy as *mut _ as *mut libc::c_void,
                    &mut len,
                );
                result == 0 && zerocopy != 0
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        false
    }
}

#[derive(Debug, Clone)]
pub struct ZeroCopyStats {
    pub buffer_size: usize,
    pub use_mmap: bool,
    pub target_addr: SocketAddr,
    pub zerocopy_enabled: bool,
}

/// Memory-mapped PCAP reader for zero-copy packet access
pub struct MmapPcapReader {
    mmap: memmap2::Mmap,
    current_offset: usize,
    file_size: usize,
}

impl MmapPcapReader {
    /// Open PCAP file with memory mapping
    pub fn open(file_path: &str) -> Result<Self> {
        let file = File::open(file_path)?;
        let file_size = file.metadata()?.len() as usize;
        
        let mmap = unsafe {
            MmapOptions::new().map(&file)?
        };

        Ok(Self {
            mmap,
            current_offset: 24, // Skip PCAP global header
            file_size,
        })
    }

    /// Read next packet without copying data
    pub fn next_packet_zero_copy(&mut self) -> Option<&[u8]> {
        if self.current_offset + 16 >= self.file_size {
            return None;
        }

        // Read packet header (16 bytes)
        let header = &self.mmap[self.current_offset..self.current_offset + 16];
        
        // Extract packet length (capture length at offset 8)
        let cap_len = u32::from_le_bytes([
            header[8], header[9], header[10], header[11]
        ]) as usize;

        self.current_offset += 16;
        
        if self.current_offset + cap_len > self.file_size {
            return None;
        }

        // Return direct reference to mapped memory (zero-copy!)
        let packet_data = &self.mmap[self.current_offset..self.current_offset + cap_len];
        self.current_offset += cap_len;

        Some(packet_data)
    }

    /// Get mapping statistics
    pub fn get_stats(&self) -> MmapStats {
        MmapStats {
            file_size: self.file_size,
            current_offset: self.current_offset,
            bytes_remaining: self.file_size.saturating_sub(self.current_offset),
            progress_percent: (self.current_offset as f64 / self.file_size as f64) * 100.0,
        }
    }

    /// Reset to beginning of file
    pub fn reset(&mut self) {
        self.current_offset = 24; // Skip global header again
    }
}

#[derive(Debug, Clone)]
pub struct MmapStats {
    pub file_size: usize,
    pub current_offset: usize,
    pub bytes_remaining: usize,
    pub progress_percent: f64,
}

/// High-performance packet buffer pool with zero-copy semantics
pub struct ZeroCopyBufferPool {
    buffers: crossbeam_queue::SegQueue<Box<[u8]>>,
    buffer_size: usize,
    total_allocated: std::sync::atomic::AtomicUsize,
}

impl ZeroCopyBufferPool {
    pub fn new(buffer_size: usize, initial_count: usize) -> Self {
        let pool = Self {
            buffers: crossbeam_queue::SegQueue::new(),
            buffer_size,
            total_allocated: std::sync::atomic::AtomicUsize::new(0),
        };

        // Pre-allocate buffers
        for _ in 0..initial_count {
            let buffer = vec![0u8; buffer_size].into_boxed_slice();
            pool.buffers.push(buffer);
            pool.total_allocated.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        pool
    }

    /// Acquire a buffer (zero-copy when possible)
    pub fn acquire(&self) -> Box<[u8]> {
        if let Some(buffer) = self.buffers.pop() {
            buffer
        } else {
            // Allocate new buffer if pool is empty
            self.total_allocated.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            vec![0u8; self.buffer_size].into_boxed_slice()
        }
    }

    /// Return buffer to pool
    pub fn release(&self, buffer: Box<[u8]>) {
        if buffer.len() == self.buffer_size {
            self.buffers.push(buffer);
        }
        // Drop buffers that don't match our size
    }

    /// Get pool statistics
    pub fn get_stats(&self) -> ZeroCopyPoolStats {
        ZeroCopyPoolStats {
            buffer_size: self.buffer_size,
            available_buffers: self.buffers.len(),
            total_allocated: self.total_allocated.load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZeroCopyPoolStats {
    pub buffer_size: usize,
    pub available_buffers: usize,
    pub total_allocated: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test] 
    fn test_zero_copy_transport() {
        let transport = ZeroCopyTransport::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            12345,
            1024 * 1024
        ).unwrap();
        
        let stats = transport.get_stats();
        assert_eq!(stats.buffer_size, 1024 * 1024);
        assert_eq!(stats.target_addr.port(), 12345);
    }

    #[test]
    fn test_buffer_pool() {
        let pool = ZeroCopyBufferPool::new(1024, 10);
        let stats = pool.get_stats();
        
        assert_eq!(stats.buffer_size, 1024);
        assert_eq!(stats.available_buffers, 10);
        
        let buffer = pool.acquire();
        assert_eq!(buffer.len(), 1024);
        
        pool.release(buffer);
        assert_eq!(pool.get_stats().available_buffers, 10);
    }
}