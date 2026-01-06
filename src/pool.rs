//! Memory Pool for High-Performance Packet Processing
//!
//! This module provides memory pooling and zero-copy buffer management for
//! high-throughput GTP-U packet processing. It includes:
//! - Generic memory pools for fixed-size allocations
//! - Packet buffer pools with reference counting
//! - Batch processing for improved performance
//! - Message batching with QFI/TEID grouping
//!
//! # Zero-Copy Design
//!
//! The pool uses reference counting to enable zero-copy operations:
//! - Buffers can be shared across threads safely
//! - Automatic return to pool when reference count reaches zero
//! - Buffer slicing without copying data
//!
//! # Examples
//!
//! ```rust
//! use rs_gtp_u::pool::PacketBufferPool;
//!
//! let mut pool = PacketBufferPool::new(10, 2048);
//!
//! // Acquire a buffer from the pool
//! if let Some(mut buffer) = pool.acquire() {
//!     // Use the buffer
//!     buffer.set_length(100);
//!
//!     // Automatically returned to pool when dropped
//! }
//! ```

use crate::tunnel::Qfi;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// ============================================================================
// Memory Pool (Generic)
// ============================================================================

/// Generic memory pool for fixed-size allocations
///
/// Provides efficient allocation and deallocation of objects of type T
/// using a free list. Thread-safe for concurrent access.
///
/// # Type Parameters
///
/// * `T` - The type of objects to pool
#[derive(Debug)]
pub struct MemoryPool<T> {
    pool: Vec<T>,
    free_list: Arc<Mutex<Vec<usize>>>, // Indices of free items
    capacity: usize,
}

impl<T> MemoryPool<T>
where
    T: Default + Clone,
{
    /// Create a new memory pool with the given capacity
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of items in the pool
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rs_gtp_u::pool::MemoryPool;
    ///
    /// let pool: MemoryPool<Vec<u8>> = MemoryPool::new(10);
    /// ```
    pub fn new(capacity: usize) -> Self {
        let mut pool = Vec::with_capacity(capacity);
        let mut free_list = Vec::with_capacity(capacity);

        // Initialize pool with default values
        for i in 0..capacity {
            pool.push(T::default());
            free_list.push(i);
        }

        Self {
            pool,
            free_list: Arc::new(Mutex::new(free_list)),
            capacity,
        }
    }

    /// Acquire an item from the pool
    ///
    /// Returns `None` if the pool is empty.
    pub fn acquire(&self) -> Option<usize> {
        let mut free_list = self.free_list.lock().unwrap();
        free_list.pop()
    }

    /// Release an item back to the pool
    ///
    /// # Arguments
    ///
    /// * `index` - Index of the item to release
    pub fn release(&self, index: usize) {
        if index < self.capacity {
            let mut free_list = self.free_list.lock().unwrap();
            free_list.push(index);
        }
    }

    /// Get a reference to an item in the pool
    ///
    /// # Arguments
    ///
    /// * `index` - Index of the item
    ///
    /// # Safety
    ///
    /// The index must be valid (< capacity).
    pub fn get(&self, index: usize) -> Option<&T> {
        self.pool.get(index)
    }

    /// Get a mutable reference to an item in the pool
    ///
    /// # Arguments
    ///
    /// * `index` - Index of the item
    ///
    /// # Safety
    ///
    /// The index must be valid (< capacity).
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.pool.get_mut(index)
    }

    /// Get the number of available items
    pub fn available(&self) -> usize {
        let free_list = self.free_list.lock().unwrap();
        free_list.len()
    }

    /// Get the total capacity of the pool
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the utilization percentage
    pub fn utilization_percent(&self) -> f64 {
        let available = self.available();
        let used = self.capacity - available;
        (used as f64 / self.capacity as f64) * 100.0
    }
}

// ============================================================================
// Packet Buffer
// ============================================================================

/// Packet buffer for zero-copy operations with reference counting
///
/// A buffer that tracks its reference count atomically, enabling safe
/// sharing across threads. When the reference count reaches zero, the
/// buffer can be returned to the pool.
#[derive(Debug)]
pub struct PacketBuffer {
    data: Vec<u8>,
    length: AtomicUsize,
    capacity: usize,
    refs: AtomicU32,
}

impl PacketBuffer {
    /// Create a new packet buffer with the given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity],
            length: AtomicUsize::new(0),
            capacity,
            refs: AtomicU32::new(1),
        }
    }

    /// Get the data slice (up to current length)
    pub fn get_data(&self) -> &[u8] {
        let len = self.length.load(Ordering::Acquire);
        &self.data[..len.min(self.capacity)]
    }

    /// Get a mutable data slice (up to capacity)
    pub fn get_data_mut(&mut self) -> &mut [u8] {
        &mut self.data[..self.capacity]
    }

    /// Set the length of valid data in the buffer
    pub fn set_length(&self, length: usize) {
        self.length
            .store(length.min(self.capacity), Ordering::Release);
    }

    /// Get the current length
    pub fn length(&self) -> usize {
        self.length.load(Ordering::Acquire)
    }

    /// Get the capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Add a reference
    pub fn add_ref(&self) {
        self.refs.fetch_add(1, Ordering::Relaxed);
    }

    /// Release a reference, returns the new reference count
    pub fn release(&self) -> u32 {
        self.refs.fetch_sub(1, Ordering::Release).saturating_sub(1)
    }

    /// Get the current reference count
    pub fn ref_count(&self) -> u32 {
        self.refs.load(Ordering::Acquire)
    }

    /// Reset the buffer for reuse
    fn reset(&self) {
        self.length.store(0, Ordering::Release);
        self.refs.store(1, Ordering::Release);
    }
}

impl Default for PacketBuffer {
    fn default() -> Self {
        Self::new(2048) // Default 2KB buffers
    }
}

impl Clone for PacketBuffer {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            length: AtomicUsize::new(self.length.load(Ordering::Acquire)),
            capacity: self.capacity,
            refs: AtomicU32::new(1),
        }
    }
}

// ============================================================================
// Packet Buffer Pool
// ============================================================================

/// Pool of packet buffers with automatic memory management
///
/// Manages a fixed pool of packet buffers, providing acquire/release
/// semantics with reference counting for safe zero-copy operations.
#[derive(Debug)]
pub struct PacketBufferPool {
    pool: Arc<Mutex<MemoryPool<PacketBuffer>>>,
    buffer_size: usize,
}

impl PacketBufferPool {
    /// Create a new packet buffer pool
    ///
    /// # Arguments
    ///
    /// * `num_buffers` - Number of buffers in the pool
    /// * `buffer_size` - Size of each buffer in bytes
    pub fn new(num_buffers: usize, buffer_size: usize) -> Self {
        let mut pool = MemoryPool::new(num_buffers);

        // Initialize each buffer with the specified size
        for i in 0..num_buffers {
            if let Some(buf) = pool.get_mut(i) {
                *buf = PacketBuffer::new(buffer_size);
            }
        }

        Self {
            pool: Arc::new(Mutex::new(pool)),
            buffer_size,
        }
    }

    /// Acquire a buffer from the pool
    ///
    /// Returns `None` if no buffers are available.
    pub fn acquire(&self) -> Option<PooledPacketBuffer> {
        let pool = self.pool.lock().unwrap();
        if let Some(index) = pool.acquire() {
            if let Some(buffer) = pool.get(index) {
                buffer.reset();
                return Some(PooledPacketBuffer {
                    buffer: buffer as *const PacketBuffer as *mut PacketBuffer,
                    index,
                    pool: Arc::clone(&self.pool),
                });
            }
        }
        None
    }

    /// Get the number of available buffers
    pub fn available(&self) -> usize {
        let pool = self.pool.lock().unwrap();
        pool.available()
    }

    /// Get the total capacity
    pub fn capacity(&self) -> usize {
        let pool = self.pool.lock().unwrap();
        pool.capacity()
    }

    /// Get the buffer size
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

/// A packet buffer that is owned by a pool
///
/// When dropped, automatically returns the buffer to the pool if the
/// reference count reaches zero.
pub struct PooledPacketBuffer {
    buffer: *mut PacketBuffer,
    index: usize,
    pool: Arc<Mutex<MemoryPool<PacketBuffer>>>,
}

impl std::fmt::Debug for PooledPacketBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledPacketBuffer")
            .field("index", &self.index)
            .field("buffer", &self.buffer())
            .finish()
    }
}

impl PooledPacketBuffer {
    /// Get a reference to the buffer
    pub fn buffer(&self) -> &PacketBuffer {
        unsafe { &*self.buffer }
    }

    /// Get a mutable reference to the buffer
    pub fn buffer_mut(&mut self) -> &mut PacketBuffer {
        unsafe { &mut *self.buffer }
    }

    /// Add a reference to prevent early return to pool
    pub fn add_ref(&self) {
        self.buffer().add_ref();
    }
}

impl Drop for PooledPacketBuffer {
    fn drop(&mut self) {
        let refs = self.buffer().release();
        if refs == 0 {
            let pool = self.pool.lock().unwrap();
            pool.release(self.index);
        }
    }
}

impl std::ops::Deref for PooledPacketBuffer {
    type Target = PacketBuffer;

    fn deref(&self) -> &Self::Target {
        self.buffer()
    }
}

impl std::ops::DerefMut for PooledPacketBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer_mut()
    }
}

// ============================================================================
// Batch Processor
// ============================================================================

/// Batch processor for packet batching
///
/// Collects packets into batches for more efficient processing.
#[derive(Debug)]
pub struct BatchProcessor {
    batch: Arc<Mutex<Vec<Option<usize>>>>, // Indices into pool
    max_batch_size: usize,
}

impl BatchProcessor {
    /// Create a new batch processor
    ///
    /// # Arguments
    ///
    /// * `max_batch_size` - Maximum number of packets in a batch
    pub fn new(max_batch_size: usize) -> Self {
        Self {
            batch: Arc::new(Mutex::new(vec![None; max_batch_size])),
            max_batch_size,
        }
    }

    /// Add a packet to the batch
    ///
    /// Returns `false` if the batch is full.
    pub fn add(&self, index: usize) -> bool {
        let mut batch = self.batch.lock().unwrap();
        for slot in batch.iter_mut() {
            if slot.is_none() {
                *slot = Some(index);
                return true;
            }
        }
        false
    }

    /// Process all packets in the batch with a callback
    ///
    /// # Arguments
    ///
    /// * `callback` - Function to call for each packet index
    pub fn process<F>(&self, mut callback: F)
    where
        F: FnMut(usize),
    {
        let mut batch = self.batch.lock().unwrap();
        for slot in batch.iter_mut() {
            if let Some(index) = slot.take() {
                callback(index);
            }
        }
    }

    /// Clear the batch
    pub fn clear(&self) {
        let mut batch = self.batch.lock().unwrap();
        batch.fill(None);
    }

    /// Get the current batch size
    pub fn size(&self) -> usize {
        let batch = self.batch.lock().unwrap();
        batch.iter().filter(|s| s.is_some()).count()
    }

    /// Check if the batch is full
    pub fn is_full(&self) -> bool {
        self.size() >= self.max_batch_size
    }
}

// ============================================================================
// Message Batch
// ============================================================================

/// Encoded message with metadata
#[derive(Debug, Clone)]
pub struct EncodedMessage {
    /// Encoded message data
    pub data: Vec<u8>,
    /// TEID (Tunnel Endpoint Identifier)
    pub teid: u32,
    /// QFI (QoS Flow Identifier), if present
    pub qfi: Option<Qfi>,
}

/// Batch of encoded messages for high-throughput scenarios
///
/// Supports grouping messages by QFI or TEID for optimized processing.
#[derive(Debug)]
pub struct MessageBatch {
    messages: Vec<EncodedMessage>,
    total_bytes: usize,
}

impl MessageBatch {
    /// Create a new message batch
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            total_bytes: 0,
        }
    }

    /// Add a pre-encoded message to the batch
    ///
    /// # Arguments
    ///
    /// * `data` - Encoded message data
    /// * `teid` - Tunnel Endpoint Identifier
    /// * `qfi` - QoS Flow Identifier (optional)
    pub fn add_encoded(&mut self, data: &[u8], teid: u32, qfi: Option<Qfi>) {
        self.total_bytes += data.len();
        self.messages.push(EncodedMessage {
            data: data.to_vec(),
            teid,
            qfi,
        });
    }

    /// Get the number of messages in the batch
    pub fn count(&self) -> usize {
        self.messages.len()
    }

    /// Get the total size of all messages
    pub fn total_size(&self) -> usize {
        self.total_bytes
    }

    /// Clear the batch
    pub fn clear(&mut self) {
        self.messages.clear();
        self.total_bytes = 0;
    }

    /// Get messages grouped by QFI for QoS-based processing
    ///
    /// Returns a HashMap where keys are QFIs and values are vectors of messages.
    /// Messages without a QFI are not included.
    pub fn group_by_qfi(&self) -> HashMap<Qfi, Vec<&EncodedMessage>> {
        let mut groups: HashMap<Qfi, Vec<&EncodedMessage>> = HashMap::new();

        for msg in &self.messages {
            if let Some(qfi) = msg.qfi {
                groups.entry(qfi).or_insert_with(Vec::new).push(msg);
            }
        }

        groups
    }

    /// Get messages grouped by TEID for tunnel-based processing
    ///
    /// Returns a HashMap where keys are TEIDs and values are vectors of messages.
    pub fn group_by_teid(&self) -> HashMap<u32, Vec<&EncodedMessage>> {
        let mut groups: HashMap<u32, Vec<&EncodedMessage>> = HashMap::new();

        for msg in &self.messages {
            groups.entry(msg.teid).or_insert_with(Vec::new).push(msg);
        }

        groups
    }

    /// Get all messages
    pub fn messages(&self) -> &[EncodedMessage] {
        &self.messages
    }
}

impl Default for MessageBatch {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Batch Statistics
// ============================================================================

/// Statistics for batch processing
#[derive(Debug)]
pub struct BatchStats {
    /// Number of batches processed
    pub batches_processed: u64,
    /// Number of messages processed
    pub messages_processed: u64,
    /// Total bytes processed
    pub bytes_processed: u64,
    /// Distribution of messages by QFI
    pub qfi_distribution: [u64; 64],
}

impl Default for BatchStats {
    fn default() -> Self {
        Self {
            batches_processed: 0,
            messages_processed: 0,
            bytes_processed: 0,
            qfi_distribution: [0; 64],
        }
    }
}

impl BatchStats {
    /// Create new batch statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a batch
    pub fn record_batch(&mut self, batch: &MessageBatch) {
        self.batches_processed += 1;
        self.messages_processed += batch.count() as u64;
        self.bytes_processed += batch.total_size() as u64;

        for msg in &batch.messages {
            if let Some(qfi) = msg.qfi {
                if (qfi as usize) < 64 {
                    self.qfi_distribution[qfi as usize] += 1;
                }
            }
        }
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        self.batches_processed = 0;
        self.messages_processed = 0;
        self.bytes_processed = 0;
        self.qfi_distribution = [0; 64];
    }

    /// Get average messages per batch
    pub fn avg_messages_per_batch(&self) -> f64 {
        if self.batches_processed == 0 {
            0.0
        } else {
            self.messages_processed as f64 / self.batches_processed as f64
        }
    }

    /// Get average bytes per message
    pub fn avg_bytes_per_message(&self) -> f64 {
        if self.messages_processed == 0 {
            0.0
        } else {
            self.bytes_processed as f64 / self.messages_processed as f64
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pool_basic() {
        let pool: MemoryPool<Vec<u8>> = MemoryPool::new(10);

        assert_eq!(pool.capacity(), 10);
        assert_eq!(pool.available(), 10);

        let idx1 = pool.acquire();
        assert!(idx1.is_some());
        assert_eq!(pool.available(), 9);

        let idx2 = pool.acquire();
        assert!(idx2.is_some());
        assert_eq!(pool.available(), 8);

        pool.release(idx1.unwrap());
        assert_eq!(pool.available(), 9);

        pool.release(idx2.unwrap());
        assert_eq!(pool.available(), 10);
    }

    #[test]
    fn test_packet_buffer() {
        let buffer = PacketBuffer::new(1024);

        assert_eq!(buffer.capacity(), 1024);
        assert_eq!(buffer.length(), 0);
        assert_eq!(buffer.ref_count(), 1);

        buffer.set_length(100);
        assert_eq!(buffer.length(), 100);

        buffer.add_ref();
        assert_eq!(buffer.ref_count(), 2);

        let refs = buffer.release();
        assert_eq!(refs, 1);
    }

    #[test]
    fn test_packet_buffer_pool() {
        let pool = PacketBufferPool::new(5, 1024);

        assert_eq!(pool.capacity(), 5);
        assert_eq!(pool.available(), 5);

        let buf1 = pool.acquire();
        assert!(buf1.is_some());
        assert_eq!(pool.available(), 4);

        let buf1 = buf1.unwrap();
        assert_eq!(buf1.capacity(), 1024);

        buf1.set_length(100);
        assert_eq!(buf1.length(), 100);

        drop(buf1);
        assert_eq!(pool.available(), 5);
    }

    #[test]
    fn test_packet_buffer_ref_counting() {
        let pool = PacketBufferPool::new(1, 1024);

        let buf = pool.acquire().unwrap();
        assert_eq!(buf.ref_count(), 1);
        assert_eq!(pool.available(), 0);

        buf.add_ref();
        assert_eq!(buf.ref_count(), 2);

        drop(buf);
        // Buffer still has 1 reference, not returned to pool
        assert_eq!(pool.available(), 0);
    }

    #[test]
    fn test_batch_processor() {
        let processor = BatchProcessor::new(10);

        assert!(processor.add(0));
        assert!(processor.add(1));
        assert_eq!(processor.size(), 2);

        let mut processed = Vec::new();
        processor.process(|idx| {
            processed.push(idx);
        });

        assert_eq!(processed, vec![0, 1]);
        assert_eq!(processor.size(), 0);
    }

    #[test]
    fn test_message_batch() {
        let mut batch = MessageBatch::new();

        batch.add_encoded(&[0x30, 0xFF, 0x00, 0x10], 0x12345678, Some(9));
        batch.add_encoded(&[0x30, 0xFF, 0x00, 0x08], 0x87654321, Some(5));
        batch.add_encoded(&[0x30, 0xFF, 0x00, 0x04], 0x12345678, Some(9));

        assert_eq!(batch.count(), 3);
        assert_eq!(batch.total_size(), 12);

        batch.clear();
        assert_eq!(batch.count(), 0);
        assert_eq!(batch.total_size(), 0);
    }

    #[test]
    fn test_message_batch_group_by_qfi() {
        let mut batch = MessageBatch::new();

        batch.add_encoded(&[0x30, 0xFF], 0x11111111, Some(9));
        batch.add_encoded(&[0x30, 0xFE], 0x22222222, Some(5));
        batch.add_encoded(&[0x30, 0xFD], 0x33333333, Some(9));
        batch.add_encoded(&[0x30, 0xFC], 0x44444444, None);

        let groups = batch.group_by_qfi();

        assert_eq!(groups.len(), 2);
        assert_eq!(groups.get(&9).unwrap().len(), 2);
        assert_eq!(groups.get(&5).unwrap().len(), 1);
    }

    #[test]
    fn test_message_batch_group_by_teid() {
        let mut batch = MessageBatch::new();

        batch.add_encoded(&[0x30, 0xFF], 0xAAAAAAAA, Some(9));
        batch.add_encoded(&[0x30, 0xFE], 0xBBBBBBBB, Some(5));
        batch.add_encoded(&[0x30, 0xFD], 0xAAAAAAAA, Some(9));

        let groups = batch.group_by_teid();

        assert_eq!(groups.len(), 2);
        assert_eq!(groups.get(&0xAAAAAAAA).unwrap().len(), 2);
        assert_eq!(groups.get(&0xBBBBBBBB).unwrap().len(), 1);
    }

    #[test]
    fn test_batch_stats() {
        let mut stats = BatchStats::new();
        let mut batch = MessageBatch::new();

        batch.add_encoded(&[0x30, 0xFF, 0x00, 0x10], 0x12345678, Some(9));
        batch.add_encoded(&[0x30, 0xFF, 0x00, 0x08], 0x87654321, Some(5));

        stats.record_batch(&batch);

        assert_eq!(stats.batches_processed, 1);
        assert_eq!(stats.messages_processed, 2);
        assert_eq!(stats.bytes_processed, 8);
        assert_eq!(stats.qfi_distribution[9], 1);
        assert_eq!(stats.qfi_distribution[5], 1);

        stats.reset();
        assert_eq!(stats.batches_processed, 0);
    }
}
