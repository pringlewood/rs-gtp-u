//! GTP-U Path Management with RTT Monitoring
//!
//! This module implements path management as per 3GPP TS 29.281 Section 4.4,
//! providing Echo request/response mechanisms, RTT monitoring, and path
//! failure detection.
//!
//! # Path Management
//!
//! Paths represent network connectivity between GTP-U peers. The module:
//! - Monitors path health via Echo requests/responses
//! - Tracks Round-Trip Time (RTT) statistics
//! - Detects path failures through timeouts
//! - Manages path state (unknown → active/suspect → failed)
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────┐
//! │       PathManager                      │
//! │  - HashMap<AddressHash, Path>          │
//! │  - Automatic echo scheduling           │
//! │  - Timeout detection                   │
//! ├────────────────────────────────────────┤
//! │       Path                             │
//! │  - State machine                       │
//! │  - RTT statistics                      │
//! │  - Echo sequence tracking              │
//! │  - Failure counting                    │
//! └────────────────────────────────────────┘
//! ```
//!
//! # Examples
//!
//! ```rust
//! use rs_gtp_u::path::{PathManager, PathConfig};
//! use std::time::Duration;
//!
//! let config = PathConfig {
//!     echo_interval: Duration::from_secs(60),
//!     echo_timeout: Duration::from_secs(5),
//!     max_echo_failures: 3,
//!     suspect_threshold: Duration::from_millis(10),
//! };
//!
//! let mut manager = PathManager::new(config);
//!
//! let peer_addr = "192.168.1.1:2152".parse().unwrap();
//! let path = manager.get_or_create_path(peer_addr);
//! ```

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ============================================================================
// Path State
// ============================================================================

/// Path state as per 3GPP TS 29.281
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathState {
    /// State unknown (no echo exchange yet)
    Unknown,
    /// Path is active and healthy
    Active,
    /// Path is suspect (high RTT or occasional failures)
    Suspect,
    /// Path has failed (consecutive echo timeouts)
    Failed,
}

// ============================================================================
// Path Statistics
// ============================================================================

/// Path statistics for monitoring
#[derive(Debug, Clone)]
pub struct PathStats {
    /// Echo requests sent
    pub echo_requests_sent: u64,
    /// Echo responses received
    pub echo_responses_received: u64,
    /// Number of RTT samples
    pub rtt_samples: u32,
    /// Minimum RTT in microseconds
    pub min_rtt_us: u64,
    /// Maximum RTT in microseconds
    pub max_rtt_us: u64,
    /// Average RTT in microseconds
    pub avg_rtt_us: u64,
    /// Last RTT measurement in microseconds
    pub last_rtt_us: u64,
}

impl PathStats {
    /// Create new statistics
    pub fn new() -> Self {
        Self {
            echo_requests_sent: 0,
            echo_responses_received: 0,
            rtt_samples: 0,
            min_rtt_us: u64::MAX,
            max_rtt_us: 0,
            avg_rtt_us: 0,
            last_rtt_us: 0,
        }
    }

    /// Update RTT statistics with a new measurement
    pub fn update_rtt(&mut self, rtt_us: u64) {
        self.rtt_samples += 1;
        self.last_rtt_us = rtt_us;

        if rtt_us < self.min_rtt_us {
            self.min_rtt_us = rtt_us;
        }
        if rtt_us > self.max_rtt_us {
            self.max_rtt_us = rtt_us;
        }

        // Update running average
        let old_avg = self.avg_rtt_us;
        let n = self.rtt_samples as u64;
        self.avg_rtt_us = (old_avg * (n - 1) + rtt_us) / n;
    }

    /// Calculate packet loss rate
    pub fn packet_loss_rate(&self) -> f64 {
        if self.echo_requests_sent == 0 {
            return 0.0;
        }
        let lost = self.echo_requests_sent - self.echo_responses_received;
        lost as f64 / self.echo_requests_sent as f64
    }
}

impl Default for PathStats {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Path Configuration
// ============================================================================

/// Path configuration parameters
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathConfig {
    /// Echo request interval
    pub echo_interval: Duration,
    /// Echo response timeout
    pub echo_timeout: Duration,
    /// Maximum consecutive failures before marking path as failed
    pub max_echo_failures: u32,
    /// RTT threshold for suspect state
    pub suspect_threshold: Duration,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            echo_interval: Duration::from_secs(60),      // 1 minute
            echo_timeout: Duration::from_secs(5),        // 5 seconds
            max_echo_failures: 3,                        // 3 failures
            suspect_threshold: Duration::from_millis(10), // 10ms
        }
    }
}

// ============================================================================
// Path Errors
// ============================================================================

/// Path-related errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// Sequence number mismatch in echo response
    SequenceMismatch,
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::SequenceMismatch => write!(f, "Echo response sequence mismatch"),
        }
    }
}

impl std::error::Error for PathError {}

// ============================================================================
// Pending Echo
// ============================================================================

/// Pending echo request awaiting response
#[derive(Debug, Clone)]
struct PendingEcho {
    sequence: u16,
    sent_time: Instant,
}

// ============================================================================
// Path
// ============================================================================

/// GTP-U Path with RTT monitoring and failure detection
#[derive(Debug, Clone)]
pub struct Path {
    /// Peer address
    pub peer_address: SocketAddr,
    /// Current path state
    state: Arc<Mutex<PathState>>,
    /// Path statistics
    stats: Arc<Mutex<PathStats>>,
    /// Configuration
    pub config: PathConfig,
    /// Last echo request time
    last_echo_time: Arc<Mutex<Instant>>,
    /// Last echo response time
    last_response_time: Arc<Mutex<Option<Instant>>>,
    /// Consecutive echo failures
    consecutive_failures: Arc<Mutex<u32>>,
    /// Current sequence number
    current_sequence: Arc<Mutex<u16>>,
    /// Pending echo request
    pending_echo: Arc<Mutex<Option<PendingEcho>>>,
}

impl Path {
    /// Create a new path
    pub fn new(peer_address: SocketAddr, config: PathConfig) -> Self {
        let now = Instant::now();
        Self {
            peer_address,
            state: Arc::new(Mutex::new(PathState::Unknown)),
            stats: Arc::new(Mutex::new(PathStats::new())),
            config,
            last_echo_time: Arc::new(Mutex::new(now)),
            last_response_time: Arc::new(Mutex::new(None)),
            consecutive_failures: Arc::new(Mutex::new(0)),
            current_sequence: Arc::new(Mutex::new(0)),
            pending_echo: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if path needs an echo request
    pub fn needs_echo(&self) -> bool {
        let pending = self.pending_echo.lock().unwrap();
        if pending.is_some() {
            return false; // Don't send if we have a pending echo
        }
        drop(pending);

        let last_echo = self.last_echo_time.lock().unwrap();
        last_echo.elapsed() >= self.config.echo_interval
    }

    /// Send an echo request
    ///
    /// Returns the sequence number to use in the Echo Request message.
    pub fn send_echo(&self) -> u16 {
        let mut current_seq = self.current_sequence.lock().unwrap();
        *current_seq = current_seq.wrapping_add(1);
        let sequence = *current_seq;

        let now = Instant::now();
        *self.pending_echo.lock().unwrap() = Some(PendingEcho {
            sequence,
            sent_time: now,
        });
        *self.last_echo_time.lock().unwrap() = now;

        let mut stats = self.stats.lock().unwrap();
        stats.echo_requests_sent += 1;

        sequence
    }

    /// Receive an echo response
    ///
    /// # Errors
    ///
    /// Returns `PathError::SequenceMismatch` if the sequence number doesn't match
    /// the pending echo request.
    pub fn receive_echo_response(&self, sequence: u16) -> Result<(), PathError> {
        let mut pending = self.pending_echo.lock().unwrap();

        if let Some(pending_echo) = pending.as_ref() {
            if pending_echo.sequence != sequence {
                return Err(PathError::SequenceMismatch);
            }

            // Calculate RTT
            let rtt = pending_echo.sent_time.elapsed();
            let rtt_us = rtt.as_micros() as u64;

            let mut stats = self.stats.lock().unwrap();
            stats.update_rtt(rtt_us);
            stats.echo_responses_received += 1;
            drop(stats);

            *self.last_response_time.lock().unwrap() = Some(Instant::now());
            *self.consecutive_failures.lock().unwrap() = 0;
            *pending = None;

            // Update path state based on RTT
            let mut state = self.state.lock().unwrap();
            if rtt > self.config.suspect_threshold {
                *state = PathState::Suspect;
            } else {
                *state = PathState::Active;
            }
        }

        Ok(())
    }

    /// Check for echo timeout
    pub fn check_timeout(&self) {
        let mut pending = self.pending_echo.lock().unwrap();

        if let Some(pending_echo) = pending.as_ref() {
            if pending_echo.sent_time.elapsed() > self.config.echo_timeout {
                // Echo timeout
                let mut failures = self.consecutive_failures.lock().unwrap();
                *failures += 1;
                *pending = None;

                let mut state = self.state.lock().unwrap();
                if *failures >= self.config.max_echo_failures {
                    *state = PathState::Failed;
                } else {
                    *state = PathState::Suspect;
                }
            }
        }
    }

    /// Check if path is healthy
    pub fn is_healthy(&self) -> bool {
        *self.state.lock().unwrap() == PathState::Active
    }

    /// Get current path state
    pub fn get_state(&self) -> PathState {
        *self.state.lock().unwrap()
    }

    /// Set path state (for testing/demonstration purposes)
    ///
    /// In production, state transitions should occur automatically through
    /// echo response handling and timeout detection.
    pub fn set_state(&self, new_state: PathState) {
        *self.state.lock().unwrap() = new_state;
    }

    /// Get path statistics
    pub fn get_stats(&self) -> PathStats {
        self.stats.lock().unwrap().clone()
    }

    /// Get consecutive failure count
    pub fn get_consecutive_failures(&self) -> u32 {
        *self.consecutive_failures.lock().unwrap()
    }
}

// ============================================================================
// Path Manager
// ============================================================================

/// Path Manager for multiple peers
///
/// Thread-safe management of GTP-U paths with automatic echo scheduling
/// and timeout detection.
#[derive(Debug)]
pub struct PathManager {
    paths: Arc<Mutex<HashMap<u64, Path>>>,
    config: PathConfig,
}

impl PathManager {
    /// Create a new path manager
    pub fn new(config: PathConfig) -> Self {
        Self {
            paths: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    /// Hash a socket address for use as a map key
    fn address_hash(address: SocketAddr) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        address.hash(&mut hasher);
        hasher.finish()
    }

    /// Get or create a path for a peer address
    pub fn get_or_create_path(&mut self, peer_address: SocketAddr) -> Path {
        let mut paths = self.paths.lock().unwrap();
        let hash = Self::address_hash(peer_address);

        paths
            .entry(hash)
            .or_insert_with(|| Path::new(peer_address, self.config.clone()))
            .clone()
    }

    /// Get a path for a peer address
    pub fn get_path(&self, peer_address: SocketAddr) -> Option<Path> {
        let paths = self.paths.lock().unwrap();
        let hash = Self::address_hash(peer_address);
        paths.get(&hash).cloned()
    }

    /// Remove a path
    pub fn remove_path(&mut self, peer_address: SocketAddr) {
        let mut paths = self.paths.lock().unwrap();
        let hash = Self::address_hash(peer_address);
        paths.remove(&hash);
    }

    /// Process echo requests and timeouts
    ///
    /// This should be called periodically. It:
    /// 1. Checks for echo timeouts
    /// 2. Sends new echo requests for paths that need them
    ///
    /// The callback is invoked for each echo request that needs to be sent,
    /// with the peer address and sequence number.
    pub fn process_echos<F>(&self, mut send_callback: F)
    where
        F: FnMut(SocketAddr, u16),
    {
        let paths = self.paths.lock().unwrap();

        for path in paths.values() {
            // Check for timeouts
            path.check_timeout();

            // Send echo if needed
            if path.needs_echo() {
                let sequence = path.send_echo();
                send_callback(path.peer_address, sequence);
            }
        }
    }

    /// Get all healthy path addresses
    pub fn get_healthy_paths(&self) -> Vec<SocketAddr> {
        let paths = self.paths.lock().unwrap();
        paths
            .values()
            .filter(|p| p.is_healthy())
            .map(|p| p.peer_address)
            .collect()
    }

    /// Get count of paths by state
    pub fn get_path_counts(&self) -> (usize, usize, usize, usize) {
        let paths = self.paths.lock().unwrap();
        let mut unknown = 0;
        let mut active = 0;
        let mut suspect = 0;
        let mut failed = 0;

        for path in paths.values() {
            match path.get_state() {
                PathState::Unknown => unknown += 1,
                PathState::Active => active += 1,
                PathState::Suspect => suspect += 1,
                PathState::Failed => failed += 1,
            }
        }

        (unknown, active, suspect, failed)
    }

    /// Get total path count
    pub fn get_path_count(&self) -> usize {
        self.paths.lock().unwrap().len()
    }
}

impl Default for PathManager {
    fn default() -> Self {
        Self::new(PathConfig::default())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_rtt_statistics() {
        let config = PathConfig::default();
        let path = Path::new("192.168.1.1:2152".parse().unwrap(), config);

        let mut stats = path.stats.lock().unwrap();
        stats.update_rtt(1000); // 1ms
        stats.update_rtt(1500); // 1.5ms
        stats.update_rtt(2000); // 2ms

        assert_eq!(stats.rtt_samples, 3);
        assert_eq!(stats.min_rtt_us, 1000);
        assert_eq!(stats.max_rtt_us, 2000);
        assert_eq!(stats.avg_rtt_us, 1500);
    }

    #[test]
    fn test_path_manager_lifecycle() {
        let config = PathConfig::default();
        let mut manager = PathManager::new(config);

        let addr1 = "192.168.1.1:2152".parse().unwrap();
        let addr2 = "192.168.1.2:2152".parse().unwrap();

        // Create paths
        manager.get_or_create_path(addr1);
        manager.get_or_create_path(addr2);

        // Verify paths exist
        assert!(manager.get_path(addr1).is_some());
        assert!(manager.get_path(addr2).is_some());
        assert_eq!(manager.get_path_count(), 2);

        // Remove path
        manager.remove_path(addr1);
        assert!(manager.get_path(addr1).is_none());
        assert!(manager.get_path(addr2).is_some());
        assert_eq!(manager.get_path_count(), 1);
    }

    #[test]
    fn test_path_echo_sequence() {
        let config = PathConfig::default();
        let path = Path::new("192.168.1.1:2152".parse().unwrap(), config);

        let seq1 = path.send_echo();
        let seq2 = path.send_echo();

        assert_eq!(seq2, seq1.wrapping_add(1));
    }

    #[test]
    fn test_path_state_transitions() {
        let config = PathConfig {
            suspect_threshold: Duration::from_millis(10),
            ..Default::default()
        };
        let path = Path::new("192.168.1.1:2152".parse().unwrap(), config);

        assert_eq!(path.get_state(), PathState::Unknown);

        // Send echo and receive response with good RTT
        let seq = path.send_echo();
        std::thread::sleep(Duration::from_micros(5000)); // 5ms
        path.receive_echo_response(seq).unwrap();
        assert_eq!(path.get_state(), PathState::Active);
    }

    #[test]
    fn test_path_echo_response_sequence_mismatch() {
        let config = PathConfig::default();
        let path = Path::new("192.168.1.1:2152".parse().unwrap(), config);

        let seq = path.send_echo();
        let result = path.receive_echo_response(seq + 1);

        assert_eq!(result.unwrap_err(), PathError::SequenceMismatch);
    }

    #[test]
    fn test_path_stats_packet_loss_rate() {
        let config = PathConfig::default();
        let path = Path::new("192.168.1.1:2152".parse().unwrap(), config);

        let mut stats = path.stats.lock().unwrap();
        stats.echo_requests_sent = 10;
        stats.echo_responses_received = 7;

        let plr = stats.packet_loss_rate();
        assert!((plr - 0.3).abs() < 0.01); // 30% loss
    }

    #[test]
    fn test_path_needs_echo() {
        let config = PathConfig {
            echo_interval: Duration::from_millis(100),
            ..Default::default()
        };
        let path = Path::new("192.168.1.1:2152".parse().unwrap(), config);

        // Initially should need echo (after interval passes)
        std::thread::sleep(Duration::from_millis(150));
        assert!(path.needs_echo());

        // After sending, should not need another immediately
        path.send_echo();
        assert!(!path.needs_echo());
    }

    #[test]
    fn test_path_manager_get_healthy_paths() {
        let config = PathConfig::default();
        let mut manager = PathManager::new(config);

        let addr1 = "192.168.1.1:2152".parse().unwrap();
        let addr2 = "192.168.1.2:2152".parse().unwrap();

        let path1 = manager.get_or_create_path(addr1);
        let path2 = manager.get_or_create_path(addr2);

        // Mark one as active
        *path1.state.lock().unwrap() = PathState::Active;
        *path2.state.lock().unwrap() = PathState::Failed;

        let healthy = manager.get_healthy_paths();
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0], addr1);
    }

    #[test]
    fn test_path_manager_get_path_counts() {
        let config = PathConfig::default();
        let mut manager = PathManager::new(config);

        let addr1 = "192.168.1.1:2152".parse().unwrap();
        let addr2 = "192.168.1.2:2152".parse().unwrap();
        let addr3 = "192.168.1.3:2152".parse().unwrap();

        let path1 = manager.get_or_create_path(addr1);
        let path2 = manager.get_or_create_path(addr2);
        let path3 = manager.get_or_create_path(addr3);

        *path1.state.lock().unwrap() = PathState::Active;
        *path2.state.lock().unwrap() = PathState::Suspect;
        *path3.state.lock().unwrap() = PathState::Failed;

        let (unknown, active, suspect, failed) = manager.get_path_counts();
        assert_eq!(unknown, 0);
        assert_eq!(active, 1);
        assert_eq!(suspect, 1);
        assert_eq!(failed, 1);
    }
}
