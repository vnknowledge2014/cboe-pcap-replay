use crate::packet::PacketData;
use crate::high_performance::{SequencedRingBuffer, PacketMemoryPool, SimdPacketProcessor};
use std::sync::Arc;
use smallvec::SmallVec;
use anyhow::Result;
use socket2::Socket;
use std::net::SocketAddr;

/// High-performance batch processor for packet sending
pub struct BatchPacketProcessor {
    ring_buffer: Arc<SequencedRingBuffer>,
    memory_pool: Arc<PacketMemoryPool>,
    batch_size: usize,
    use_simd: bool,
}

impl BatchPacketProcessor {
    pub fn new(
        ring_buffer: Arc<SequencedRingBuffer>, 
        memory_pool: Arc<PacketMemoryPool>,
        batch_size: usize
    ) -> Self {
        Self {
            ring_buffer,
            memory_pool,
            batch_size,
            use_simd: SimdPacketProcessor::get_simd_info().has_avx2 || 
                     SimdPacketProcessor::get_simd_info().has_neon,
        }
    }

    /// Process packets in high-performance batches
    pub fn process_batch<F>(&self, mut sender: F) -> Result<usize>
    where
        F: FnMut(&[PacketData]) -> Result<()>,
    {
        let slots = self.ring_buffer.consume_available();
        if slots.is_empty() {
            return Ok(0);
        }

        // Convert slots to PacketData in batches
        let mut batch = SmallVec::<[PacketData; 32]>::new();
        let mut processed = 0;

        for chunk in slots.chunks(self.batch_size.min(32)) {
            batch.clear();
            
            // Use SIMD-optimized conversion when available
            if self.use_simd && chunk.len() >= 4 {
                self.simd_convert_batch(chunk, &mut batch)?;
            } else {
                for slot in chunk {
                    batch.push(slot.to_packet_data());
                }
            }

            sender(&batch)?;
            processed += batch.len();
        }

        Ok(processed)
    }

    /// SIMD-optimized batch conversion
    fn simd_convert_batch(
        &self, 
        slots: &[&crate::high_performance::MessageSlot], 
        batch: &mut SmallVec<[PacketData; 32]>
    ) -> Result<()> {
        // Pre-allocate buffers from pool
        let _buffers = SmallVec::<[Vec<u8>; 32]>::new();
        
        for slot in slots {
            let mut buffer = self.memory_pool.acquire_buffer(slot.data.len());
            
            // Copy data to buffer (temporarily disable SIMD due to type issues)
            buffer.extend_from_slice(&slot.data);
            
            batch.push(PacketData {
                dest_port: slot.port,
                timestamp: slot.timestamp,
                payload: buffer.into_vec(),
            });
        }

        Ok(())
    }

    /// Process packets by port with sequence ordering
    pub fn process_by_port<F>(&self, port: u16, mut sender: F) -> Result<usize>
    where
        F: FnMut(&[PacketData]) -> Result<()>,
    {
        let slots = self.ring_buffer.consume_ordered_by_port(port);
        if slots.is_empty() {
            return Ok(0);
        }

        // Convert to PacketData maintaining sequence order
        let mut packets = SmallVec::<[PacketData; 16]>::new();
        for slot in &slots {
            packets.push(slot.to_packet_data());
        }

        sender(&packets)?;
        Ok(packets.len())
    }

    /// Get processor statistics
    pub fn get_stats(&self) -> BatchProcessorStats {
        let buffer_stats = self.ring_buffer.get_stats();
        let pool_stats = self.memory_pool.get_stats();
        
        BatchProcessorStats {
            ring_buffer_utilization: buffer_stats.buffer_utilization,
            available_slots: buffer_stats.available_slots,
            memory_pool_efficiency: pool_stats.efficiency(),
            total_memory_kb: pool_stats.total_memory_kb(),
            simd_enabled: self.use_simd,
            batch_size: self.batch_size,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatchProcessorStats {
    pub ring_buffer_utilization: f64,
    pub available_slots: u64,
    pub memory_pool_efficiency: f64,
    pub total_memory_kb: usize,
    pub simd_enabled: bool,
    pub batch_size: usize,
}

/// High-performance UDP sender with batching support
pub struct BatchUdpSender {
    sockets: std::collections::HashMap<u16, Socket>,
    target_ip: std::net::IpAddr,
    batch_size: usize,
}

impl BatchUdpSender {
    pub fn new(target_ip: std::net::IpAddr, ports: &[u16], batch_size: usize) -> Result<Self> {
        let mut sockets = std::collections::HashMap::new();
        
        for &port in ports {
            let socket = Socket::new(
                socket2::Domain::IPV4, 
                socket2::Type::DGRAM, 
                Some(socket2::Protocol::UDP)
            )?;
            
            // Optimize socket for high-performance sending
            socket.set_send_buffer_size(1024 * 1024)?; // 1MB send buffer
            // Note: set_nodelay is only for TCP sockets, not UDP
            
            sockets.insert(port, socket);
        }

        Ok(Self {
            sockets,
            target_ip,
            batch_size,
        })
    }

    /// Send packets in batches for optimal performance
    pub fn send_batch(&self, packets: &[PacketData]) -> Result<usize> {
        let mut sent = 0;
        
        // Group packets by port for efficient sending
        let mut port_groups: std::collections::HashMap<u16, Vec<&PacketData>> = 
            std::collections::HashMap::new();
        
        for packet in packets {
            port_groups.entry(packet.dest_port)
                .or_insert_with(Vec::new)
                .push(packet);
        }

        // Send each port's packets in batch
        for (port, port_packets) in port_groups {
            if let Some(socket) = self.sockets.get(&port) {
                sent += self.send_port_batch(socket, port, &port_packets)?;
            }
        }

        Ok(sent)
    }

    /// Send a batch of packets for a specific port
    fn send_port_batch(&self, socket: &Socket, port: u16, packets: &[&PacketData]) -> Result<usize> {
        let dest_addr = SocketAddr::new(self.target_ip, port);
        let mut sent = 0;

        // Send in chunks to avoid overwhelming the network stack
        for chunk in packets.chunks(self.batch_size.min(64)) {
            for packet in chunk {
                match socket.send_to(&packet.payload, &dest_addr.into()) {
                    Ok(_) => sent += 1,
                    Err(e) => {
                        // Log error but continue with other packets
                        tracing::warn!("Failed to send packet to {}:{}: {}", self.target_ip, port, e);
                    }
                }
            }
        }

        Ok(sent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::high_performance::SequencedRingBuffer;

    #[test]
    fn test_batch_processor() {
        let ring_buffer = Arc::new(SequencedRingBuffer::new());
        let memory_pool = Arc::new(PacketMemoryPool::new(100, 50, 25));
        let processor = BatchPacketProcessor::new(ring_buffer, memory_pool, 32);
        
        let stats = processor.get_stats();
        assert_eq!(stats.batch_size, 32);
    }
}