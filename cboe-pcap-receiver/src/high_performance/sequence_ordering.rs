use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;
use parking_lot::RwLock;
use anyhow::Result;

/// Sequence ordering manager to ensure packet ordering per port
/// Critical for maintaining CBOE PITCH message sequence integrity
#[derive(Debug)]
pub struct SequenceOrderingManager {
    // Per-port sequence tracking
    port_sequences: RwLock<HashMap<u16, AtomicU64>>,
    // Global sequence for cross-port ordering
    global_sequence: AtomicU64,
    // Reorder window size
    reorder_window: usize,
}

impl SequenceOrderingManager {
    pub fn new(reorder_window: usize) -> Self {
        Self {
            port_sequences: RwLock::new(HashMap::new()),
            global_sequence: AtomicU64::new(0),
            reorder_window,
        }
    }

    /// Get next sequence number for a port (producer side)
    pub fn next_sequence_for_port(&self, port: u16) -> u64 {
        let sequences = self.port_sequences.read();
        if let Some(seq) = sequences.get(&port) {
            seq.fetch_add(1, Ordering::SeqCst)
        } else {
            drop(sequences);
            let mut sequences = self.port_sequences.write();
            let seq = sequences.entry(port).or_insert_with(|| AtomicU64::new(0));
            seq.fetch_add(1, Ordering::SeqCst)
        }
    }

    /// Get next global sequence
    pub fn next_global_sequence(&self) -> u64 {
        self.global_sequence.fetch_add(1, Ordering::SeqCst)
    }

    /// Validate sequence ordering (consumer side)
    pub fn validate_sequence(&self, port: u16, sequence: u64) -> Result<bool> {
        let sequences = self.port_sequences.read();
        if let Some(expected_seq) = sequences.get(&port) {
            let expected = expected_seq.load(Ordering::Acquire);
            // Allow for reorder window
            Ok(sequence >= expected.saturating_sub(self.reorder_window as u64) 
               && sequence <= expected + self.reorder_window as u64)
        } else {
            // First packet for this port
            Ok(true)
        }
    }

    /// Update expected sequence for a port
    pub fn update_expected_sequence(&self, port: u16, sequence: u64) {
        let sequences = self.port_sequences.read();
        if let Some(seq) = sequences.get(&port) {
            seq.store(sequence + 1, Ordering::Release);
        } else {
            drop(sequences);
            let mut sequences = self.port_sequences.write();
            sequences.insert(port, AtomicU64::new(sequence + 1));
        }
    }

    /// Get statistics for monitoring
    pub fn get_stats(&self) -> HashMap<u16, u64> {
        self.port_sequences
            .read()
            .iter()
            .map(|(&port, seq)| (port, seq.load(Ordering::Relaxed)))
            .collect()
    }
}

/// Per-port sequence tracker for fine-grained ordering
#[derive(Debug)]
pub struct PortSequenceTracker {
    port: u16,
    next_expected: AtomicU64,
    last_processed: AtomicU64,
    gaps_detected: AtomicU64,
    out_of_order: AtomicU64,
}

impl PortSequenceTracker {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            next_expected: AtomicU64::new(1),
            last_processed: AtomicU64::new(0),
            gaps_detected: AtomicU64::new(0),
            out_of_order: AtomicU64::new(0),
        }
    }

    /// Process a sequence number and return ordering status
    pub fn process_sequence(&self, sequence: u64) -> SequenceStatus {
        let expected = self.next_expected.load(Ordering::Acquire);
        let last = self.last_processed.load(Ordering::Acquire);

        match sequence.cmp(&expected) {
            std::cmp::Ordering::Equal => {
                // Perfect sequence
                self.next_expected.store(sequence + 1, Ordering::Release);
                self.last_processed.store(sequence, Ordering::Release);
                SequenceStatus::InOrder
            }
            std::cmp::Ordering::Greater => {
                // Gap detected
                let gap_size = sequence - expected;
                self.gaps_detected.fetch_add(gap_size, Ordering::Relaxed);
                self.next_expected.store(sequence + 1, Ordering::Release);
                self.last_processed.store(sequence, Ordering::Release);
                SequenceStatus::Gap(gap_size)
            }
            std::cmp::Ordering::Less => {
                if sequence > last {
                    // Late but acceptable
                    self.last_processed.store(sequence, Ordering::Release);
                    SequenceStatus::Late
                } else {
                    // Out of order or duplicate
                    self.out_of_order.fetch_add(1, Ordering::Relaxed);
                    SequenceStatus::OutOfOrder
                }
            }
        }
    }

    pub fn get_stats(&self) -> (u64, u64, u64, u64) {
        (
            self.next_expected.load(Ordering::Relaxed),
            self.last_processed.load(Ordering::Relaxed),
            self.gaps_detected.load(Ordering::Relaxed),
            self.out_of_order.load(Ordering::Relaxed),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SequenceStatus {
    InOrder,
    Gap(u64),
    Late,
    OutOfOrder,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_ordering() {
        let manager = SequenceOrderingManager::new(10);
        
        // Test basic sequence generation
        assert_eq!(manager.next_sequence_for_port(30501), 0);
        assert_eq!(manager.next_sequence_for_port(30501), 1);
        assert_eq!(manager.next_sequence_for_port(30502), 0);
        
        // Test sequence validation
        assert!(manager.validate_sequence(30501, 2).unwrap());
        manager.update_expected_sequence(30501, 2);
    }

    #[test]
    fn test_port_sequence_tracker() {
        let tracker = PortSequenceTracker::new(30501);
        
        assert_eq!(tracker.process_sequence(1), SequenceStatus::InOrder);
        assert_eq!(tracker.process_sequence(2), SequenceStatus::InOrder);
        assert_eq!(tracker.process_sequence(5), SequenceStatus::Gap(3));
        assert_eq!(tracker.process_sequence(3), SequenceStatus::Late);
        assert_eq!(tracker.process_sequence(2), SequenceStatus::OutOfOrder);
    }
}