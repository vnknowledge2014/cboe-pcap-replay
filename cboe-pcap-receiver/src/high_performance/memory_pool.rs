use std::sync::atomic::{AtomicUsize, Ordering};
use crossbeam_queue::SegQueue;
use smallvec::SmallVec;

/// High-performance memory pool for zero-allocation packet processing
pub struct PacketMemoryPool {
    // Pre-allocated packet buffers
    small_buffers: SegQueue<Vec<u8>>,    // <= 1KB
    medium_buffers: SegQueue<Vec<u8>>,   // 1KB - 8KB
    large_buffers: SegQueue<Vec<u8>>,    // > 8KB
    
    // Pool statistics
    small_allocated: AtomicUsize,
    medium_allocated: AtomicUsize,
    large_allocated: AtomicUsize,
    small_in_use: AtomicUsize,
    medium_in_use: AtomicUsize,
    large_in_use: AtomicUsize,
}

impl PacketMemoryPool {
    pub fn new(small_count: usize, medium_count: usize, large_count: usize) -> Self {
        let pool = Self {
            small_buffers: SegQueue::new(),
            medium_buffers: SegQueue::new(),
            large_buffers: SegQueue::new(),
            small_allocated: AtomicUsize::new(0),
            medium_allocated: AtomicUsize::new(0),
            large_allocated: AtomicUsize::new(0),
            small_in_use: AtomicUsize::new(0),
            medium_in_use: AtomicUsize::new(0),
            large_in_use: AtomicUsize::new(0),
        };

        // Pre-allocate buffers
        for _ in 0..small_count {
            pool.small_buffers.push(vec![0u8; 1024]);
            pool.small_allocated.fetch_add(1, Ordering::Relaxed);
        }

        for _ in 0..medium_count {
            pool.medium_buffers.push(vec![0u8; 8192]);
            pool.medium_allocated.fetch_add(1, Ordering::Relaxed);
        }

        for _ in 0..large_count {
            pool.large_buffers.push(vec![0u8; 65536]);
            pool.large_allocated.fetch_add(1, Ordering::Relaxed);
        }

        pool
    }

    /// Get a buffer suitable for the given size
    pub fn acquire_buffer(&self, size: usize) -> PooledBuffer {
        if size <= 1024 {
            if let Some(mut buffer) = self.small_buffers.pop() {
                buffer.clear();
                buffer.reserve(size);
                self.small_in_use.fetch_add(1, Ordering::Relaxed);
                return PooledBuffer::new(buffer, BufferType::Small, self);
            }
        } else if size <= 8192 {
            if let Some(mut buffer) = self.medium_buffers.pop() {
                buffer.clear();
                buffer.reserve(size);
                self.medium_in_use.fetch_add(1, Ordering::Relaxed);
                return PooledBuffer::new(buffer, BufferType::Medium, self);
            }
        } else {
            if let Some(mut buffer) = self.large_buffers.pop() {
                buffer.clear();
                buffer.reserve(size);
                self.large_in_use.fetch_add(1, Ordering::Relaxed);
                return PooledBuffer::new(buffer, BufferType::Large, self);
            }
        }

        // Fallback to heap allocation if pool is exhausted
        PooledBuffer::new(Vec::with_capacity(size), BufferType::Heap, self)
    }

    /// Return a buffer to the appropriate pool
    fn return_buffer(&self, mut buffer: Vec<u8>, buffer_type: BufferType) {
        match buffer_type {
            BufferType::Small => {
                if buffer.capacity() >= 1024 {
                    buffer.clear();
                    self.small_buffers.push(buffer);
                    self.small_in_use.fetch_sub(1, Ordering::Relaxed);
                }
            }
            BufferType::Medium => {
                if buffer.capacity() >= 8192 {
                    buffer.clear();
                    self.medium_buffers.push(buffer);
                    self.medium_in_use.fetch_sub(1, Ordering::Relaxed);
                }
            }
            BufferType::Large => {
                if buffer.capacity() >= 65536 {
                    buffer.clear();
                    self.large_buffers.push(buffer);
                    self.large_in_use.fetch_sub(1, Ordering::Relaxed);
                }
            }
            BufferType::Heap => {
                // Heap allocated buffers are dropped
            }
        }
    }

    /// Get pool statistics
    pub fn get_stats(&self) -> PoolStats {
        PoolStats {
            small_allocated: self.small_allocated.load(Ordering::Relaxed),
            medium_allocated: self.medium_allocated.load(Ordering::Relaxed),
            large_allocated: self.large_allocated.load(Ordering::Relaxed),
            small_in_use: self.small_in_use.load(Ordering::Relaxed),
            medium_in_use: self.medium_in_use.load(Ordering::Relaxed),
            large_in_use: self.large_in_use.load(Ordering::Relaxed),
            small_available: self.small_buffers.len(),
            medium_available: self.medium_buffers.len(),
            large_available: self.large_buffers.len(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum BufferType {
    Small,
    Medium,
    Large,
    Heap,
}

/// RAII wrapper for pooled buffers
pub struct PooledBuffer<'a> {
    buffer: Option<Vec<u8>>,
    buffer_type: BufferType,
    pool: &'a PacketMemoryPool,
}

impl<'a> PooledBuffer<'a> {
    fn new(buffer: Vec<u8>, buffer_type: BufferType, pool: &'a PacketMemoryPool) -> Self {
        Self {
            buffer: Some(buffer),
            buffer_type,
            pool,
        }
    }

    /// Get mutable reference to the buffer
    pub fn buffer_mut(&mut self) -> &mut Vec<u8> {
        self.buffer.as_mut().unwrap()
    }

    /// Get immutable reference to the buffer
    pub fn buffer(&self) -> &Vec<u8> {
        self.buffer.as_ref().unwrap()
    }

    /// Take ownership of the buffer (prevents return to pool)
    pub fn into_vec(mut self) -> Vec<u8> {
        self.buffer.take().unwrap()
    }
}

impl<'a> Drop for PooledBuffer<'a> {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            self.pool.return_buffer(buffer, self.buffer_type);
        }
    }
}

impl<'a> std::ops::Deref for PooledBuffer<'a> {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        self.buffer()
    }
}

impl<'a> std::ops::DerefMut for PooledBuffer<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer_mut()
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub small_allocated: usize,    // Total small buffers allocated
    pub medium_allocated: usize,   // Total medium buffers allocated
    pub large_allocated: usize,    // Total large buffers allocated
    pub small_in_use: usize,       // Small buffers currently in use
    pub medium_in_use: usize,      // Medium buffers currently in use
    pub large_in_use: usize,       // Large buffers currently in use
    pub small_available: usize,    // Small buffers available in pool
    pub medium_available: usize,   // Medium buffers available in pool
    pub large_available: usize,    // Large buffers available in pool
}

impl PoolStats {
    pub fn efficiency(&self) -> f64 {
        let total_allocated = self.small_allocated + self.medium_allocated + self.large_allocated;
        let total_in_use = self.small_in_use + self.medium_in_use + self.large_in_use;
        
        if total_allocated == 0 {
            0.0
        } else {
            (total_in_use as f64 / total_allocated as f64) * 100.0
        }
    }

    pub fn total_memory_kb(&self) -> usize {
        (self.small_allocated * 1) +      // 1KB each
        (self.medium_allocated * 8) +     // 8KB each
        (self.large_allocated * 64)       // 64KB each
    }
}

/// Batch buffer manager for processing multiple packets efficiently
pub struct BatchBufferManager {
    pool: PacketMemoryPool,
    batch_buffers: SegQueue<SmallVec<[Vec<u8>; 32]>>,
}

impl BatchBufferManager {
    pub fn new() -> Self {
        Self {
            pool: PacketMemoryPool::new(1000, 500, 100), // Default pool sizes
            batch_buffers: SegQueue::new(),
        }
    }

    /// Acquire buffers for batch processing
    pub fn acquire_batch_buffers(&self, count: usize, avg_size: usize) -> SmallVec<[PooledBuffer; 32]> {
        let mut buffers = SmallVec::new();
        
        for _ in 0..count.min(32) {
            buffers.push(self.pool.acquire_buffer(avg_size));
        }
        
        buffers
    }

    /// Get pool statistics
    pub fn get_stats(&self) -> PoolStats {
        self.pool.get_stats()
    }
}

// Global memory pool instance
lazy_static::lazy_static! {
    pub static ref GLOBAL_PACKET_POOL: PacketMemoryPool = PacketMemoryPool::new(10000, 5000, 1000);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pool_basic() {
        let pool = PacketMemoryPool::new(10, 5, 2);
        
        // Test small buffer acquisition
        let mut buffer = pool.acquire_buffer(512);
        buffer.extend_from_slice(&[1, 2, 3, 4]);
        assert_eq!(buffer.len(), 4);
        
        let stats_before = pool.get_stats();
        assert_eq!(stats_before.small_in_use, 1);
        
        // Drop buffer to return it to pool
        drop(buffer);
        
        let stats_after = pool.get_stats();
        assert_eq!(stats_after.small_in_use, 0);
        assert_eq!(stats_after.small_available, 10);
    }

    #[test]
    fn test_buffer_size_selection() {
        let pool = PacketMemoryPool::new(5, 5, 5);
        
        // Test different size allocations
        let _small = pool.acquire_buffer(500);     // Should get small buffer
        let _medium = pool.acquire_buffer(4000);   // Should get medium buffer
        let _large = pool.acquire_buffer(20000);   // Should get large buffer
        
        let stats = pool.get_stats();
        assert_eq!(stats.small_in_use, 1);
        assert_eq!(stats.medium_in_use, 1);
        assert_eq!(stats.large_in_use, 1);
    }

    #[test]
    fn test_pool_efficiency() {
        let pool = PacketMemoryPool::new(100, 50, 25);
        let stats = pool.get_stats();
        
        assert_eq!(stats.efficiency(), 0.0); // No buffers in use
        assert!(stats.total_memory_kb() > 0);
    }
}