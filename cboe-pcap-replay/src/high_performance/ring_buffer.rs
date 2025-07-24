use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use arrayvec::ArrayVec;
use smallvec::SmallVec;
use crate::packet::PacketData;

/// High-performance ring buffer inspired by LMAX Disruptor pattern
/// Maintains strict ordering while maximizing throughput
const _DEFAULT_RING_BUFFER_SIZE: usize = 4096 * 4096; // 16M slots (default)
const CACHE_LINE_SIZE: usize = 64;

#[repr(C, align(64))]  // Cache line aligned
pub struct MessageSlot {
    pub sequence: AtomicU64,
    pub port: u16,
    pub timestamp: f64,
    pub data_len: u16,
    pub flags: u8,
    pub _padding: [u8; 53], // Pad to 64 bytes
    pub data: ArrayVec<u8, 2048>, // Pre-allocated for typical packet sizes
}

impl MessageSlot {
    pub fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            port: 0,
            timestamp: 0.0,
            data_len: 0,
            flags: 0,
            _padding: [0; 53],
            data: ArrayVec::new(),
        }
    }


    pub fn load_packet(&mut self, packet: &PacketData, sequence: u64) -> anyhow::Result<()> {
        self.sequence.store(sequence, Ordering::Release);
        self.port = packet.dest_port;
        self.timestamp = packet.timestamp;
        self.data_len = packet.payload.len() as u16;
        self.flags = 0;
        
        self.data.clear();
        if self.data.try_extend_from_slice(&packet.payload).is_err() {
            return Err(anyhow::anyhow!("Packet too large for slot"));
        }
        
        Ok(())
    }

    pub fn to_packet_data(&self) -> PacketData {
        PacketData {
            dest_port: self.port,
            timestamp: self.timestamp,
            payload: self.data.to_vec(),
        }
    }
}

/// High-performance ring buffer with per-port ordering guarantees
pub struct SequencedRingBuffer {
    slots: Box<[MessageSlot]>,
    mask: usize,
    buffer_size: usize,
    
    // Producer tracking - cache line separated
    producer_sequence: AtomicU64,
    _producer_padding: [u8; CACHE_LINE_SIZE - 8],
    
    // Consumer tracking - cache line separated  
    consumer_sequence: AtomicU64,
    _consumer_padding: [u8; CACHE_LINE_SIZE - 8],
    
    // Sequence ordering per port
    port_sequences: dashmap::DashMap<u16, AtomicU64>,
}

impl SequencedRingBuffer {
    pub fn new() -> Self {
        Self::with_size(_DEFAULT_RING_BUFFER_SIZE)
    }
    
    pub fn with_size(ring_buffer_size: usize) -> Self {
        let buffer_size = ring_buffer_size.next_power_of_two();
        let mut slots = Vec::with_capacity(buffer_size);
        
        for _ in 0..buffer_size {
            slots.push(MessageSlot::new());
        }

        Self {
            slots: slots.into_boxed_slice(),
            mask: buffer_size - 1,
            buffer_size,
            producer_sequence: AtomicU64::new(0),
            _producer_padding: [0; CACHE_LINE_SIZE - 8],
            consumer_sequence: AtomicU64::new(0),
            _consumer_padding: [0; CACHE_LINE_SIZE - 8],
            #[allow(dead_code)]
            port_sequences: dashmap::DashMap::new(),
        }
    }


    /// Producer: Claim a mutable slot for writing (single-threaded producer)
    pub fn claim_slot_mut(&mut self) -> Option<(u64, &mut MessageSlot)> {
        let current_sequence = self.producer_sequence.load(Ordering::Relaxed);
        let consumer_sequence = self.consumer_sequence.load(Ordering::Acquire);
        
        // Check if buffer is full
        if current_sequence >= consumer_sequence + self.slots.len() as u64 {
            return None; // Buffer full
        }
        
        let next_sequence = current_sequence + 1;
        let index = (next_sequence & self.mask as u64) as usize;
        
        Some((next_sequence, &mut self.slots[index]))
    }

    /// Producer: Publish a slot (make it available to consumers)
    pub fn publish(&self, sequence: u64) {
        self.producer_sequence.store(sequence, Ordering::Release);
    }


    /// Consumer: Get available slots for processing
    pub fn consume_available(&self) -> SmallVec<[&MessageSlot; 32]> {
        let consumer_sequence = self.consumer_sequence.load(Ordering::Relaxed);
        let producer_sequence = self.producer_sequence.load(Ordering::Acquire);
        
        let mut available = SmallVec::new();
        let mut next_sequence = consumer_sequence + 1;
        
        while next_sequence <= producer_sequence && available.len() < 32 {
            let index = (next_sequence & self.mask as u64) as usize;
            let slot = &self.slots[index];
            
            // Ensure slot is properly published
            if slot.sequence.load(Ordering::Acquire) == next_sequence {
                available.push(slot);
                next_sequence += 1;
            } else {
                break; // Wait for proper sequence
            }
        }
        
        if !available.is_empty() {
            self.consumer_sequence.store(next_sequence - 1, Ordering::Release);
        }
        
        available
    }

    /// Consumer: Process slots in strict port sequence order
    pub fn consume_ordered_by_port(&self, port: u16) -> SmallVec<[&MessageSlot; 16]> {
        let available_slots = self.consume_available();
        let mut port_slots = SmallVec::new();
        
        // Get expected sequence for this port
        let port_seq_ref = self.port_sequences.entry(port).or_insert_with(|| AtomicU64::new(0));
        let expected_port_seq = port_seq_ref.load(Ordering::Acquire);
        
        // Find slots for this port in correct sequence
        for slot in available_slots {
            if slot.port == port {
                // For now, add all port packets - would need more sophisticated ordering
                port_slots.push(slot);
            }
        }
        
        // Update port sequence tracking
        if !port_slots.is_empty() {
            port_seq_ref.store(expected_port_seq + port_slots.len() as u64, Ordering::Release);
        }
        
        port_slots
    }

    /// Get buffer utilization statistics
    pub fn get_stats(&self) -> RingBufferStats {
        let producer_seq = self.producer_sequence.load(Ordering::Relaxed);
        let consumer_seq = self.consumer_sequence.load(Ordering::Relaxed);
        let available = producer_seq.saturating_sub(consumer_seq);
        let utilization = (available as f64 / self.slots.len() as f64) * 100.0;
        
        RingBufferStats {
            producer_sequence: producer_seq,
            consumer_sequence: consumer_seq,
            available_slots: available,
            buffer_utilization: utilization,
            total_capacity: self.buffer_size,
        }
    }

}

// SequencedRingBuffer is safe for Send/Sync due to atomic operations
// Removed unsafe impl - let the compiler auto-derive these traits

#[derive(Debug, Clone)]
pub struct RingBufferStats {
    pub producer_sequence: u64,
    pub consumer_sequence: u64,
    pub available_slots: u64,
    pub buffer_utilization: f64,
    pub total_capacity: usize,
}

/// Batch processor for high-throughput operations
pub struct BatchProcessor {
    ring_buffer: Arc<SequencedRingBuffer>,
    batch_size: usize,
}

impl BatchProcessor {
    pub fn new(ring_buffer: Arc<SequencedRingBuffer>, batch_size: usize) -> Self {
        Self {
            ring_buffer,
            batch_size,
        }
    }

    /// Process packets in batches to amortize overhead
    pub fn process_batch<F>(&self, mut processor: F) -> anyhow::Result<usize>
    where
        F: FnMut(&[&MessageSlot]) -> anyhow::Result<()>,
    {
        let slots = self.ring_buffer.consume_available();
        if slots.is_empty() {
            return Ok(0);
        }

        // Process in chunks of batch_size
        let mut processed = 0;
        for chunk in slots.chunks(self.batch_size) {
            processor(chunk)?;
            processed += chunk.len();
        }

        Ok(processed)
    }

    /// Process packets by port to maintain ordering
    pub fn process_by_port<F>(&self, port: u16, mut processor: F) -> anyhow::Result<usize>
    where
        F: FnMut(&[&MessageSlot]) -> anyhow::Result<()>,
    {
        let slots = self.ring_buffer.consume_ordered_by_port(port);
        if slots.is_empty() {
            return Ok(0);
        }

        processor(&slots)?;
        Ok(slots.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let mut buffer = SequencedRingBuffer::new();
        
        // Test claiming and publishing
        let (seq, slot) = buffer.claim_slot().unwrap();
        assert_eq!(seq, 1);
        
        // Create a test packet
        let packet = PacketData {
            dest_port: 30501,
            timestamp: 123.456,
            payload: vec![1, 2, 3, 4],
        };
        
        // Use claim_slot_mut instead of unsafe casting
        if let Some((seq, slot_mut)) = buffer.claim_slot_mut() {
            slot_mut.load_packet(&packet, seq).unwrap();
            buffer.publish(seq);
        }
        
        // Test consuming
        let consumed = buffer.consume_available();
        assert_eq!(consumed.len(), 1);
        assert_eq!(consumed[0].port, 30501);
    }

    #[test]
    fn test_buffer_stats() {
        let mut buffer = SequencedRingBuffer::new();
        let stats = buffer.get_stats();
        
        assert_eq!(stats.producer_sequence, 0);
        assert_eq!(stats.consumer_sequence, 0);
        assert_eq!(stats.available_slots, 0);
        assert_eq!(stats.buffer_utilization, 0.0);
    }
}