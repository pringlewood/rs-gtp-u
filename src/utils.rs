//! GTP-U Utility Functions
//!
//! This module provides utility functions and data structures for GTP-U protocol
//! implementation, including:
//! - TEID (Tunnel Endpoint Identifier) generation
//! - Anti-replay protection using sliding window
//! - IPv4 checksum calculation
//! - Rate limiting using token bucket algorithm
//! - Circular buffer for zero-copy packet handling
//!
//! # Examples
//!
//! ```rust
//! use rs_gtp_u::utils::TeidGenerator;
//!
//! let mut gen = TeidGenerator::new();
//! let teid = gen.generate(); // Generate a random TEID
//! assert_ne!(teid, 0); // TEID is never zero
//! ```

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ============================================================================
// TEID Generator
// ============================================================================

/// Cryptographic TEID Generator
///
/// Generates TEIDs (Tunnel Endpoint Identifiers) using a cryptographically
/// secure random number generator. TEIDs are guaranteed to be non-zero,
/// as TEID 0 is reserved for signaling messages.
///
/// # Thread Safety
///
/// This type is thread-safe and can be shared across threads using `Arc`.
///
/// # Examples
///
/// ```rust
/// use rs_gtp_u::utils::TeidGenerator;
///
/// let mut gen = TeidGenerator::new();
/// let teid1 = gen.generate();
/// let teid2 = gen.generate();
///
/// assert_ne!(teid1, 0);
/// assert_ne!(teid2, 0);
/// ```
#[derive(Debug)]
pub struct TeidGenerator {
    inner: Arc<Mutex<TeidGeneratorInner>>,
}

#[derive(Debug)]
struct TeidGeneratorInner {
    rng: rand::rngs::StdRng,
}

impl TeidGenerator {
    /// Create a new TEID generator
    pub fn new() -> Self {
        use rand::SeedableRng;
        Self {
            inner: Arc::new(Mutex::new(TeidGeneratorInner {
                rng: rand::rngs::StdRng::from_entropy(),
            })),
        }
    }

    /// Generate a single TEID
    ///
    /// Returns a non-zero u32 value suitable for use as a TEID.
    pub fn generate(&self) -> u32 {
        use rand::Rng;

        let mut inner = self.inner.lock().unwrap();
        let mut teid: u32 = 0;

        // Ensure TEID is never zero (reserved value)
        while teid == 0 {
            teid = inner.rng.gen();
        }

        teid
    }

    /// Generate a range of unique TEIDs
    ///
    /// Returns a vector of `count` unique non-zero TEIDs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rs_gtp_u::utils::TeidGenerator;
    ///
    /// let mut gen = TeidGenerator::new();
    /// let teids = gen.generate_range(10);
    ///
    /// assert_eq!(teids.len(), 10);
    /// // All TEIDs are unique
    /// ```
    pub fn generate_range(&self, count: usize) -> Vec<u32> {
        let mut teids = Vec::with_capacity(count);
        let mut seen = HashSet::new();

        while teids.len() < count {
            let teid = self.generate();
            if seen.insert(teid) {
                teids.push(teid);
            }
        }

        teids
    }
}

impl Default for TeidGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for TeidGenerator {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

// ============================================================================
// Anti-Replay Window
// ============================================================================

/// Anti-replay protection using sliding window
///
/// Implements a sliding window algorithm for detecting duplicate and
/// out-of-order packets based on sequence numbers. This is critical for
/// security in GTP-U tunnels.
///
/// # Algorithm
///
/// The window tracks the highest received sequence number and a bitmap
/// of recently seen sequence numbers. Packets are classified as:
/// - **Accepted**: New packet or old packet within window not seen before
/// - **Rejected**: Duplicate packet or packet too old (outside window)
///
/// # Examples
///
/// ```rust
/// use rs_gtp_u::utils::AntiReplayWindow;
///
/// let mut window = AntiReplayWindow::new(64);
///
/// // Accept first packet
/// assert!(window.check(1));
///
/// // Reject duplicate
/// assert!(!window.check(1));
///
/// // Accept newer packet
/// assert!(window.check(2));
/// ```
#[derive(Debug)]
pub struct AntiReplayWindow {
    window_size: u16,
    highest_seq: Arc<Mutex<u16>>,
    bitmap: Arc<Mutex<Vec<u64>>>,
}

impl AntiReplayWindow {
    /// Create a new anti-replay window
    ///
    /// # Arguments
    ///
    /// * `window_size` - Size of the sliding window (typically 64 or 128)
    pub fn new(window_size: u16) -> Self {
        let bitmap_size = ((window_size + 63) / 64) as usize;
        Self {
            window_size,
            highest_seq: Arc::new(Mutex::new(0)),
            bitmap: Arc::new(Mutex::new(vec![0u64; bitmap_size])),
        }
    }

    /// Check if a sequence number should be accepted
    ///
    /// Returns `true` if the packet should be accepted, `false` if it should
    /// be rejected (duplicate or too old).
    pub fn check(&self, sequence: u16) -> bool {
        let mut highest_seq = self.highest_seq.lock().unwrap();
        let mut bitmap = self.bitmap.lock().unwrap();

        let diff = sequence.wrapping_sub(*highest_seq);

        if diff == 0 {
            // Exact duplicate
            false
        } else if diff < 32768 {
            // Newer packet (forward direction)
            if diff >= self.window_size {
                // Outside window, clear and accept
                bitmap.fill(0);
                *highest_seq = sequence;
                Self::set_bit(&mut bitmap, 0);
                true
            } else {
                // Within window, shift and check
                Self::shift_window(&mut bitmap, diff);
                *highest_seq = sequence;
                if Self::get_bit(&bitmap, 0) {
                    false // Duplicate
                } else {
                    Self::set_bit(&mut bitmap, 0);
                    true
                }
            }
        } else {
            // Older packet (backward direction)
            let age = 0u16.wrapping_sub(diff);
            // Reject if too old or duplicate
            if age >= self.window_size || Self::get_bit(&bitmap, age) {
                false
            } else {
                Self::set_bit(&mut bitmap, age);
                true
            }
        }
    }

    /// Set a bit in the bitmap
    fn set_bit(bitmap: &mut [u64], pos: u16) {
        let idx = (pos / 64) as usize;
        let bit = (pos % 64) as u32;
        if idx < bitmap.len() {
            bitmap[idx] |= 1u64 << bit;
        }
    }

    /// Get a bit from the bitmap
    fn get_bit(bitmap: &[u64], pos: u16) -> bool {
        let idx = (pos / 64) as usize;
        let bit = (pos % 64) as u32;
        if idx < bitmap.len() {
            (bitmap[idx] & (1u64 << bit)) != 0
        } else {
            false
        }
    }

    /// Shift the window forward
    fn shift_window(bitmap: &mut [u64], shift: u16) {
        if shift >= (bitmap.len() * 64) as u16 {
            bitmap.fill(0);
            return;
        }

        let word_shift = (shift / 64) as usize;
        let bit_shift = (shift % 64) as u32;

        // Shift by whole words
        if word_shift > 0 {
            for i in (word_shift..bitmap.len()).rev() {
                bitmap[i] = bitmap[i - word_shift];
            }
            for i in 0..word_shift.min(bitmap.len()) {
                bitmap[i] = 0;
            }
        }

        // Shift remaining bits
        if bit_shift > 0 && bit_shift < 64 {
            for i in (0..bitmap.len()).rev() {
                bitmap[i] >>= bit_shift;
                if i > 0 {
                    let comp_shift = 64 - bit_shift;
                    bitmap[i] |= bitmap[i - 1] << comp_shift;
                }
            }
        }
    }

    /// Get the highest sequence number seen
    pub fn highest_sequence(&self) -> u16 {
        *self.highest_seq.lock().unwrap()
    }
}

impl Clone for AntiReplayWindow {
    fn clone(&self) -> Self {
        Self {
            window_size: self.window_size,
            highest_seq: Arc::clone(&self.highest_seq),
            bitmap: Arc::clone(&self.bitmap),
        }
    }
}

// ============================================================================
// IPv4 Checksum
// ============================================================================

/// Calculate IPv4 checksum
///
/// Implements the standard IPv4 header checksum algorithm as specified
/// in RFC 791.
///
/// # Examples
///
/// ```rust
/// use rs_gtp_u::utils::calculate_ipv4_checksum;
///
/// let data = [0x45, 0x00, 0x00, 0x3c]; // IPv4 header fragment
/// let checksum = calculate_ipv4_checksum(&data);
/// ```
pub fn calculate_ipv4_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;

    // Sum 16-bit words
    while i + 1 < data.len() {
        sum += ((data[i] as u32) << 8) | (data[i + 1] as u32);
        i += 2;
    }

    // Add remaining byte if odd length
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }

    // Fold 32-bit sum to 16 bits
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    // One's complement
    !sum as u16
}

// ============================================================================
// Rate Limiter
// ============================================================================

/// Rate limiter using token bucket algorithm
///
/// Implements token bucket rate limiting with automatic token refill.
/// This is useful for controlling packet transmission rates in GTP-U tunnels.
///
/// # Algorithm
///
/// - Tokens are added to the bucket at a fixed `refill_rate` per second
/// - Bucket has a maximum `capacity`
/// - Operations consume tokens; if insufficient tokens available, operation fails
///
/// # Examples
///
/// ```rust
/// use rs_gtp_u::utils::RateLimiter;
///
/// let mut limiter = RateLimiter::new(100, 10); // 100 capacity, 10 tokens/sec
///
/// // Try to consume 50 tokens
/// assert!(limiter.try_consume(50));
///
/// // Check available tokens
/// let available = limiter.get_available_tokens();
/// ```
#[derive(Debug)]
pub struct RateLimiter {
    capacity: u64,
    tokens: Arc<Mutex<f64>>,
    refill_rate: u64,
    last_refill: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of tokens in the bucket
    /// * `refill_rate` - Number of tokens added per second
    pub fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            capacity,
            tokens: Arc::new(Mutex::new(capacity as f64)),
            refill_rate,
            last_refill: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Try to consume tokens
    ///
    /// Returns `true` if tokens were consumed, `false` if insufficient tokens available.
    pub fn try_consume(&self, tokens: u64) -> bool {
        self.refill();

        let mut current_tokens = self.tokens.lock().unwrap();
        if *current_tokens >= tokens as f64 {
            *current_tokens -= tokens as f64;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&self) {
        let mut last_refill = self.last_refill.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill);
        let elapsed_sec = elapsed.as_secs_f64();

        let tokens_to_add = (self.refill_rate as f64) * elapsed_sec;

        if tokens_to_add > 0.0 {
            let mut current_tokens = self.tokens.lock().unwrap();
            *current_tokens = (*current_tokens + tokens_to_add).min(self.capacity as f64);
            *last_refill = now;
        }
    }

    /// Get the number of available tokens
    pub fn get_available_tokens(&self) -> u64 {
        self.refill();
        let tokens = self.tokens.lock().unwrap();
        *tokens as u64
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            capacity: self.capacity,
            tokens: Arc::clone(&self.tokens),
            refill_rate: self.refill_rate,
            last_refill: Arc::clone(&self.last_refill),
        }
    }
}

// ============================================================================
// Circular Buffer
// ============================================================================

/// Circular buffer for zero-copy packet handling
///
/// A generic thread-safe circular buffer (ring buffer) implementation.
/// Useful for buffering packets in GTP-U processing pipelines.
///
/// # Examples
///
/// ```rust
/// use rs_gtp_u::utils::CircularBuffer;
///
/// let mut buffer = CircularBuffer::<u32>::new(4);
///
/// buffer.push(1).unwrap();
/// buffer.push(2).unwrap();
///
/// assert_eq!(buffer.pop(), Some(1));
/// assert_eq!(buffer.pop(), Some(2));
/// ```
#[derive(Debug)]
pub struct CircularBuffer<T> {
    buffer: Arc<Mutex<CircularBufferInner<T>>>,
}

#[derive(Debug)]
struct CircularBufferInner<T> {
    data: Vec<Option<T>>,
    read_pos: usize,
    write_pos: usize,
    count: usize,
}

/// Error type for circular buffer operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircularBufferError {
    /// Buffer is full
    BufferFull,
}

impl std::fmt::Display for CircularBufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircularBufferError::BufferFull => write!(f, "Buffer is full"),
        }
    }
}

impl std::error::Error for CircularBufferError {}

impl<T> CircularBuffer<T> {
    /// Create a new circular buffer with the given capacity
    pub fn new(capacity: usize) -> Self {
        let mut data = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            data.push(None);
        }

        Self {
            buffer: Arc::new(Mutex::new(CircularBufferInner {
                data,
                read_pos: 0,
                write_pos: 0,
                count: 0,
            })),
        }
    }

    /// Push an item into the buffer
    ///
    /// Returns `Err(CircularBufferError::BufferFull)` if the buffer is full.
    pub fn push(&self, item: T) -> Result<(), CircularBufferError> {
        let mut inner = self.buffer.lock().unwrap();

        if inner.count >= inner.data.len() {
            return Err(CircularBufferError::BufferFull);
        }

        let write_pos = inner.write_pos;
        inner.data[write_pos] = Some(item);
        inner.write_pos = (write_pos + 1) % inner.data.len();
        inner.count += 1;

        Ok(())
    }

    /// Pop an item from the buffer
    ///
    /// Returns `None` if the buffer is empty.
    pub fn pop(&self) -> Option<T> {
        let mut inner = self.buffer.lock().unwrap();

        if inner.count == 0 {
            return None;
        }

        let read_pos = inner.read_pos;
        let item = inner.data[read_pos].take();
        inner.read_pos = (read_pos + 1) % inner.data.len();
        inner.count -= 1;

        item
    }

    /// Get the number of items in the buffer
    pub fn len(&self) -> usize {
        let inner = self.buffer.lock().unwrap();
        inner.count
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the capacity of the buffer
    pub fn capacity(&self) -> usize {
        let inner = self.buffer.lock().unwrap();
        inner.data.len()
    }

    /// Check if the buffer is full
    pub fn is_full(&self) -> bool {
        let inner = self.buffer.lock().unwrap();
        inner.count >= inner.data.len()
    }
}

impl<T> Clone for CircularBuffer<T> {
    fn clone(&self) -> Self {
        Self {
            buffer: Arc::clone(&self.buffer),
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
    fn test_teid_generation() {
        let gen = TeidGenerator::new();

        let teid1 = gen.generate();
        let teid2 = gen.generate();

        assert_ne!(teid1, 0);
        assert_ne!(teid2, 0);
        // TEIDs should generally be different (not guaranteed but highly likely)
    }

    #[test]
    fn test_teid_range_generation() {
        let gen = TeidGenerator::new();

        let teids = gen.generate_range(10);

        assert_eq!(teids.len(), 10);

        // Verify all TEIDs are non-zero and unique
        let mut seen = HashSet::new();
        for teid in teids {
            assert_ne!(teid, 0);
            assert!(seen.insert(teid), "TEID should be unique");
        }
    }

    #[test]
    fn test_anti_replay_window() {
        let window = AntiReplayWindow::new(64);

        // Accept first packet
        assert!(window.check(1));

        // Reject duplicate
        assert!(!window.check(1));

        // Accept newer packet
        assert!(window.check(2));
        assert!(window.check(3));

        // Accept old packet within window (first time)
        assert!(window.check(2));

        // Reject duplicate old packet (second time)
        assert!(!window.check(2));
    }

    #[test]
    fn test_anti_replay_window_old_packets() {
        let window = AntiReplayWindow::new(64);

        // Set highest sequence
        assert!(window.check(100));

        // Accept old packet within window
        assert!(window.check(50));

        // Reject duplicate
        assert!(!window.check(50));

        // Reject packet too old (outside window)
        assert!(!window.check(35));
    }

    #[test]
    fn test_ipv4_checksum() {
        // Test with a known IPv4 header
        let header = [
            0x45, 0x00, 0x00, 0x3c, 0x1c, 0x46, 0x40, 0x00, 0x40, 0x06, 0x00, 0x00, 0xac, 0x10,
            0x0a, 0x63, 0xac, 0x10, 0x0a, 0x0c,
        ];

        let checksum = calculate_ipv4_checksum(&header);
        // The checksum should be a valid 16-bit value
        assert!(checksum != 0);
    }

    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new(100, 10);

        // Should be able to consume initial tokens
        assert!(limiter.try_consume(50));
        assert!(limiter.try_consume(50));

        // Should fail when out of tokens
        assert!(!limiter.try_consume(1));
    }

    #[test]
    fn test_rate_limiter_refill() {
        let limiter = RateLimiter::new(100, 1000); // High refill rate

        // Consume all tokens
        assert!(limiter.try_consume(100));
        assert!(!limiter.try_consume(1));

        // Wait for refill
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Should have tokens now
        assert!(limiter.try_consume(50));
    }

    #[test]
    fn test_circular_buffer() {
        let buffer = CircularBuffer::<u32>::new(4);

        assert!(buffer.is_empty());

        buffer.push(1).unwrap();
        buffer.push(2).unwrap();
        buffer.push(3).unwrap();

        assert_eq!(buffer.len(), 3);

        assert_eq!(buffer.pop(), Some(1));
        assert_eq!(buffer.pop(), Some(2));

        buffer.push(4).unwrap();
        buffer.push(5).unwrap();

        assert_eq!(buffer.pop(), Some(3));
        assert_eq!(buffer.pop(), Some(4));
    }

    #[test]
    fn test_circular_buffer_full() {
        let buffer = CircularBuffer::<u32>::new(2);

        buffer.push(1).unwrap();
        buffer.push(2).unwrap();

        // Buffer is full
        assert!(buffer.is_full());
        assert!(buffer.push(3).is_err());
    }

    #[test]
    fn test_circular_buffer_empty() {
        let buffer = CircularBuffer::<u32>::new(4);

        assert_eq!(buffer.pop(), None);

        buffer.push(1).unwrap();
        assert_eq!(buffer.pop(), Some(1));
        assert_eq!(buffer.pop(), None);
    }
}
