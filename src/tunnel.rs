//! GTP-U Tunnel Management with State Machine
//!
//! This module provides tunnel lifecycle management following 3GPP TS 29.281.
//! It implements thread-safe tunnel state machines, statistics tracking, and
//! multi-tunnel management with TEID allocation.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │     TunnelManager                   │
//! │  - HashMap<TEID, Tunnel>            │
//! │  - Atomic TEID allocation           │
//! │  - Cleanup idle tunnels             │
//! ├─────────────────────────────────────┤
//! │     Tunnel                          │
//! │  - State machine                    │
//! │  - Thread-safe statistics           │
//! │  - QoS flow management              │
//! │  - Anti-replay protection           │
//! └─────────────────────────────────────┘
//! ```
//!
//! # Thread Safety
//!
//! All types in this module are thread-safe:
//! - `TunnelStats` uses `Mutex<StatsInner>` for statistics
//! - `Tunnel` uses `Mutex` for state transitions
//! - `TunnelManager` uses `Mutex` for tunnel map access
//! - TEID allocation uses `AtomicU32`
//!
//! # Examples
//!
//! ```rust
//! use rs_gtp_u::tunnel::{TunnelManager, TunnelConfig};
//! use std::net::SocketAddr;
//!
//! let mut manager = TunnelManager::new();
//!
//! let config = TunnelConfig {
//!     local_teid: manager.allocate_teid(),
//!     remote_teid: 0x5678,
//!     local_address: "192.168.1.1:2152".parse().unwrap(),
//!     remote_address: "192.168.1.2:2152".parse().unwrap(),
//!     idle_timeout: std::time::Duration::from_secs(300),
//! };
//!
//! let teid = manager.create_tunnel(config).unwrap();
//! assert_eq!(manager.get_tunnel_count(), 1);
//! ```

use crate::utils::AntiReplayWindow;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ============================================================================
// Type Aliases
// ============================================================================

/// Tunnel Endpoint Identifier (TEID) as per 3GPP TS 29.281
pub type Teid = u32;

/// QoS Flow Identifier (QFI) - 6-bit value (0-63)
///
/// This will be moved to qos.rs in Week 7, but we define it here for now
/// to support QoS flow management in tunnels.
pub type Qfi = u8;

// ============================================================================
// Tunnel State Machine
// ============================================================================

/// Tunnel state as per 3GPP lifecycle
///
/// The tunnel state machine enforces valid state transitions to maintain
/// protocol correctness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelState {
    /// Not yet established
    Inactive,
    /// Being established
    Establishing,
    /// Fully operational
    Active,
    /// Being modified (QoS update, handover, etc.)
    Modifying,
    /// Being released
    Releasing,
    /// Released/deleted
    Released,
}

impl TunnelState {
    /// Check if a transition from the current state to the target state is valid
    ///
    /// # State Transitions
    ///
    /// ```text
    /// Inactive → Establishing
    /// Establishing → Active | Releasing
    /// Active → Modifying | Releasing
    /// Modifying → Active | Releasing
    /// Releasing → Released
    /// Released → (none - terminal state)
    /// ```
    pub fn can_transition_to(&self, target: TunnelState) -> bool {
        use TunnelState::*;
        match self {
            Inactive => target == Establishing,
            Establishing => target == Active || target == Releasing,
            Active => target == Modifying || target == Releasing,
            Modifying => target == Active || target == Releasing,
            Releasing => target == Released,
            Released => false,
        }
    }
}

// ============================================================================
// Tunnel Statistics
// ============================================================================

/// Inner statistics data (protected by Mutex)
#[derive(Debug, Clone)]
struct StatsInner {
    packets_tx: u64,
    packets_rx: u64,
    bytes_tx: u64,
    bytes_rx: u64,
    errors: u64,
    created_at: Instant,
    last_activity: Instant,
}

/// Thread-safe tunnel statistics
///
/// All statistics updates are protected by a mutex to ensure consistency
/// when accessed from multiple threads.
#[derive(Debug, Clone)]
pub struct TunnelStats {
    inner: Arc<Mutex<StatsInner>>,
}

impl TunnelStats {
    /// Create new statistics with current timestamp
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            inner: Arc::new(Mutex::new(StatsInner {
                packets_tx: 0,
                packets_rx: 0,
                bytes_tx: 0,
                bytes_rx: 0,
                errors: 0,
                created_at: now,
                last_activity: now,
            })),
        }
    }

    /// Record a transmitted packet
    pub fn record_tx(&self, bytes: usize) {
        let mut stats = self.inner.lock().unwrap();
        stats.packets_tx += 1;
        stats.bytes_tx += bytes as u64;
        stats.last_activity = Instant::now();
    }

    /// Record a received packet
    pub fn record_rx(&self, bytes: usize) {
        let mut stats = self.inner.lock().unwrap();
        stats.packets_rx += 1;
        stats.bytes_rx += bytes as u64;
        stats.last_activity = Instant::now();
    }

    /// Record an error
    pub fn record_error(&self) {
        let mut stats = self.inner.lock().unwrap();
        stats.errors += 1;
    }

    /// Get tunnel lifetime duration
    pub fn get_lifetime(&self) -> Duration {
        let stats = self.inner.lock().unwrap();
        stats.created_at.elapsed()
    }

    /// Get idle time since last activity
    pub fn get_idle_time(&self) -> Duration {
        let stats = self.inner.lock().unwrap();
        stats.last_activity.elapsed()
    }

    /// Get transmit packet count
    pub fn packets_tx(&self) -> u64 {
        self.inner.lock().unwrap().packets_tx
    }

    /// Get receive packet count
    pub fn packets_rx(&self) -> u64 {
        self.inner.lock().unwrap().packets_rx
    }

    /// Get transmit byte count
    pub fn bytes_tx(&self) -> u64 {
        self.inner.lock().unwrap().bytes_tx
    }

    /// Get receive byte count
    pub fn bytes_rx(&self) -> u64 {
        self.inner.lock().unwrap().bytes_rx
    }

    /// Get error count
    pub fn errors(&self) -> u64 {
        self.inner.lock().unwrap().errors
    }
}

impl Default for TunnelStats {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tunnel Configuration
// ============================================================================

/// Tunnel configuration parameters
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelConfig {
    /// Local TEID for this tunnel
    pub local_teid: Teid,
    /// Remote TEID for this tunnel
    pub remote_teid: Teid,
    /// Local GTP-U endpoint address
    pub local_address: SocketAddr,
    /// Remote GTP-U endpoint address
    pub remote_address: SocketAddr,
    /// Idle timeout duration (default: 5 minutes)
    pub idle_timeout: Duration,
}

impl TunnelConfig {
    /// Validate tunnel configuration
    ///
    /// # Errors
    ///
    /// Returns `TunnelError::InvalidLocalTeid` if local TEID is 0
    pub fn validate(&self) -> Result<(), TunnelError> {
        if self.local_teid == 0 {
            return Err(TunnelError::InvalidLocalTeid);
        }
        // Note: remote_teid can be 0 for certain signaling messages
        Ok(())
    }
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self {
            local_teid: 0,
            remote_teid: 0,
            local_address: "0.0.0.0:2152".parse().unwrap(),
            remote_address: "0.0.0.0:2152".parse().unwrap(),
            idle_timeout: Duration::from_secs(300), // 5 minutes
        }
    }
}

// ============================================================================
// Tunnel Errors
// ============================================================================

/// Tunnel-related errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelError {
    /// Invalid local TEID (cannot be 0)
    InvalidLocalTeid,
    /// Invalid state transition
    InvalidStateTransition {
        /// Current state
        from: TunnelState,
        /// Target state
        to: TunnelState,
    },
}

impl std::fmt::Display for TunnelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TunnelError::InvalidLocalTeid => write!(f, "Invalid local TEID (cannot be 0)"),
            TunnelError::InvalidStateTransition { from, to } => {
                write!(f, "Invalid state transition from {:?} to {:?}", from, to)
            }
        }
    }
}

impl std::error::Error for TunnelError {}

// ============================================================================
// GTP-U Tunnel
// ============================================================================

/// GTP-U Tunnel with state machine and statistics
///
/// A tunnel represents a GTP-U connection between two endpoints, identified
/// by local and remote TEIDs. It maintains state, statistics, QoS flows,
/// and optional anti-replay protection.
#[derive(Debug, Clone)]
pub struct Tunnel {
    /// Tunnel configuration
    pub config: TunnelConfig,
    /// Current state (protected by mutex)
    state: Arc<Mutex<TunnelState>>,
    /// Statistics
    pub stats: TunnelStats,
    /// QoS Flow Identifiers (QFIs) associated with this tunnel
    qos_flows: Arc<Mutex<Vec<Qfi>>>,
    /// Last sequence number (for anti-replay protection)
    last_sequence: Arc<Mutex<Option<u16>>>,
    /// Anti-replay window for sequence number validation
    replay_window: AntiReplayWindow,
}

impl Tunnel {
    /// Create a new tunnel with the given configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid
    pub fn new(config: TunnelConfig) -> Result<Self, TunnelError> {
        config.validate()?;

        Ok(Self {
            config,
            state: Arc::new(Mutex::new(TunnelState::Inactive)),
            stats: TunnelStats::new(),
            qos_flows: Arc::new(Mutex::new(Vec::new())),
            last_sequence: Arc::new(Mutex::new(None)),
            replay_window: AntiReplayWindow::new(128), // 128-packet window
        })
    }

    /// Transition to a new state
    ///
    /// # Errors
    ///
    /// Returns an error if the state transition is not valid
    pub fn transition_to(&self, new_state: TunnelState) -> Result<(), TunnelError> {
        let mut state = self.state.lock().unwrap();
        if !state.can_transition_to(new_state) {
            return Err(TunnelError::InvalidStateTransition {
                from: *state,
                to: new_state,
            });
        }
        *state = new_state;
        Ok(())
    }

    /// Activate the tunnel (inactive → establishing → active)
    ///
    /// # Errors
    ///
    /// Returns an error if the state transitions are not valid
    pub fn activate(&self) -> Result<(), TunnelError> {
        self.transition_to(TunnelState::Establishing)?;
        self.transition_to(TunnelState::Active)?;
        Ok(())
    }

    /// Release the tunnel (current → releasing → released)
    ///
    /// # Errors
    ///
    /// Returns an error if the state transitions are not valid
    pub fn release(&self) -> Result<(), TunnelError> {
        self.transition_to(TunnelState::Releasing)?;
        self.transition_to(TunnelState::Released)?;
        Ok(())
    }

    /// Add a QoS flow to the tunnel
    ///
    /// If the QFI already exists, this method does nothing.
    pub fn add_qos_flow(&self, qfi: Qfi) {
        let mut flows = self.qos_flows.lock().unwrap();
        if !flows.contains(&qfi) {
            flows.push(qfi);
        }
    }

    /// Remove a QoS flow from the tunnel
    pub fn remove_qos_flow(&self, qfi: Qfi) {
        let mut flows = self.qos_flows.lock().unwrap();
        flows.retain(|&q| q != qfi);
    }

    /// Get all QoS flows
    pub fn get_qos_flows(&self) -> Vec<Qfi> {
        self.qos_flows.lock().unwrap().clone()
    }

    /// Check if the tunnel is idle (no activity within idle timeout)
    pub fn is_idle(&self) -> bool {
        self.stats.get_idle_time() > self.config.idle_timeout
    }

    /// Check if the tunnel is in active state
    pub fn is_active(&self) -> bool {
        *self.state.lock().unwrap() == TunnelState::Active
    }

    /// Get current state
    pub fn get_state(&self) -> TunnelState {
        *self.state.lock().unwrap()
    }

    /// Check sequence number for anti-replay protection
    ///
    /// Returns `true` if the sequence number is valid (not a duplicate or very old),
    /// `false` otherwise.
    ///
    /// # Anti-Replay Algorithm
    ///
    /// Uses a sliding window anti-replay mechanism that:
    /// - Accepts packets with sequence numbers greater than the last seen
    /// - Rejects duplicate packets (same sequence number)
    /// - Accepts out-of-order packets within the window
    /// - Rejects packets outside the window (too old)
    pub fn check_sequence(&self, sequence: u16) -> bool {
        // Use anti-replay window for robust sequence checking
        let result = self.replay_window.check(sequence);

        // Also update last_sequence for backward compatibility
        if result {
            *self.last_sequence.lock().unwrap() = Some(sequence);
        }

        result
    }
}

// ============================================================================
// Tunnel Manager
// ============================================================================

/// Thread-safe tunnel manager
///
/// Manages multiple GTP-U tunnels with automatic TEID allocation,
/// lookup by TEID, and cleanup of idle tunnels.
#[derive(Debug)]
pub struct TunnelManager {
    tunnels: Arc<Mutex<HashMap<Teid, Tunnel>>>,
    teid_counter: AtomicU32,
}

impl TunnelManager {
    /// Create a new tunnel manager
    pub fn new() -> Self {
        Self {
            tunnels: Arc::new(Mutex::new(HashMap::new())),
            teid_counter: AtomicU32::new(0x1000), // Start from 0x1000
        }
    }

    /// Allocate a new TEID
    ///
    /// Thread-safe TEID allocation using atomic increment.
    pub fn allocate_teid(&self) -> Teid {
        self.teid_counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Create a new tunnel and add it to the manager
    ///
    /// The tunnel is automatically activated upon creation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration is invalid
    /// - The state transition fails during activation
    pub fn create_tunnel(&mut self, config: TunnelConfig) -> Result<Teid, TunnelError> {
        let tunnel = Tunnel::new(config.clone())?;
        tunnel.activate()?;

        let mut tunnels = self.tunnels.lock().unwrap();
        tunnels.insert(config.local_teid, tunnel);

        Ok(config.local_teid)
    }

    /// Get a tunnel by TEID
    ///
    /// Returns `None` if the tunnel does not exist.
    pub fn get_tunnel(&self, teid: Teid) -> Option<Tunnel> {
        let tunnels = self.tunnels.lock().unwrap();
        tunnels.get(&teid).cloned()
    }

    /// Remove a tunnel by TEID
    ///
    /// The tunnel is released before removal.
    ///
    /// # Errors
    ///
    /// Returns an error if the state transition fails during release
    pub fn remove_tunnel(&mut self, teid: Teid) -> Result<(), TunnelError> {
        let mut tunnels = self.tunnels.lock().unwrap();
        if let Some(tunnel) = tunnels.get(&teid) {
            tunnel.release()?;
            tunnels.remove(&teid);
        }
        Ok(())
    }

    /// Get total tunnel count
    pub fn get_tunnel_count(&self) -> usize {
        self.tunnels.lock().unwrap().len()
    }

    /// Get active tunnel count
    pub fn get_active_tunnels(&self) -> usize {
        let tunnels = self.tunnels.lock().unwrap();
        tunnels.values().filter(|t| t.is_active()).count()
    }

    /// Clean up idle tunnels
    ///
    /// Removes all tunnels that have been idle longer than their configured timeout.
    ///
    /// Returns the number of tunnels removed.
    pub fn cleanup_idle_tunnels(&mut self) -> usize {
        let mut tunnels = self.tunnels.lock().unwrap();
        let idle_teids: Vec<Teid> = tunnels
            .iter()
            .filter(|(_, tunnel)| tunnel.is_idle())
            .map(|(teid, _)| *teid)
            .collect();

        let count = idle_teids.len();
        for teid in idle_teids {
            if let Some(tunnel) = tunnels.get(&teid) {
                let _ = tunnel.release(); // Ignore errors during cleanup
                tunnels.remove(&teid);
            }
        }

        count
    }

    /// Update statistics for a tunnel
    ///
    /// Records either a transmitted or received packet for the specified tunnel.
    pub fn update_stats(&self, teid: Teid, bytes: usize, is_tx: bool) {
        if let Some(tunnel) = self.get_tunnel(teid) {
            if is_tx {
                tunnel.stats.record_tx(bytes);
            } else {
                tunnel.stats.record_rx(bytes);
            }
        }
    }
}

impl Default for TunnelManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_state_transitions() {
        use TunnelState::*;

        // Valid transitions
        assert!(Inactive.can_transition_to(Establishing));
        assert!(Establishing.can_transition_to(Active));
        assert!(Establishing.can_transition_to(Releasing));
        assert!(Active.can_transition_to(Modifying));
        assert!(Active.can_transition_to(Releasing));
        assert!(Modifying.can_transition_to(Active));
        assert!(Modifying.can_transition_to(Releasing));
        assert!(Releasing.can_transition_to(Released));

        // Invalid transitions
        assert!(!Inactive.can_transition_to(Active));
        assert!(!Released.can_transition_to(Inactive));
        assert!(!Active.can_transition_to(Establishing));
    }

    #[test]
    fn test_tunnel_lifecycle() {
        let config = TunnelConfig {
            local_teid: 0x1234,
            remote_teid: 0x5678,
            local_address: "192.168.1.1:2152".parse().unwrap(),
            remote_address: "192.168.1.2:2152".parse().unwrap(),
            idle_timeout: Duration::from_secs(300),
        };

        let tunnel = Tunnel::new(config).unwrap();
        assert_eq!(tunnel.get_state(), TunnelState::Inactive);

        tunnel.activate().unwrap();
        assert_eq!(tunnel.get_state(), TunnelState::Active);
        assert!(tunnel.is_active());

        tunnel.release().unwrap();
        assert_eq!(tunnel.get_state(), TunnelState::Released);
        assert!(!tunnel.is_active());
    }

    #[test]
    fn test_tunnel_manager_lifecycle() {
        let mut manager = TunnelManager::new();

        let config = TunnelConfig {
            local_teid: manager.allocate_teid(),
            remote_teid: 0x5678,
            local_address: "192.168.1.1:2152".parse().unwrap(),
            remote_address: "192.168.1.2:2152".parse().unwrap(),
            idle_timeout: Duration::from_secs(300),
        };

        let teid = manager.create_tunnel(config).unwrap();
        assert_eq!(manager.get_tunnel_count(), 1);
        assert_eq!(manager.get_active_tunnels(), 1);

        let tunnel = manager.get_tunnel(teid).unwrap();
        assert!(tunnel.is_active());

        manager.remove_tunnel(teid).unwrap();
        assert_eq!(manager.get_tunnel_count(), 0);
    }

    #[test]
    fn test_tunnel_qos_flow_management() {
        let config = TunnelConfig {
            local_teid: 0x1234,
            remote_teid: 0x5678,
            local_address: "192.168.1.1:2152".parse().unwrap(),
            remote_address: "192.168.1.2:2152".parse().unwrap(),
            idle_timeout: Duration::from_secs(300),
        };

        let tunnel = Tunnel::new(config).unwrap();

        tunnel.add_qos_flow(9);
        tunnel.add_qos_flow(5);
        assert_eq!(tunnel.get_qos_flows().len(), 2);

        // Adding duplicate does nothing
        tunnel.add_qos_flow(9);
        assert_eq!(tunnel.get_qos_flows().len(), 2);

        tunnel.remove_qos_flow(9);
        assert_eq!(tunnel.get_qos_flows().len(), 1);
    }

    #[test]
    fn test_tunnel_statistics() {
        let config = TunnelConfig {
            local_teid: 0x1234,
            remote_teid: 0x5678,
            local_address: "192.168.1.1:2152".parse().unwrap(),
            remote_address: "192.168.1.2:2152".parse().unwrap(),
            idle_timeout: Duration::from_secs(300),
        };

        let tunnel = Tunnel::new(config).unwrap();

        tunnel.stats.record_tx(100);
        tunnel.stats.record_tx(200);
        tunnel.stats.record_rx(150);

        assert_eq!(tunnel.stats.packets_tx(), 2);
        assert_eq!(tunnel.stats.packets_rx(), 1);
        assert_eq!(tunnel.stats.bytes_tx(), 300);
        assert_eq!(tunnel.stats.bytes_rx(), 150);
    }

    #[test]
    fn test_tunnel_sequence_check() {
        let config = TunnelConfig {
            local_teid: 0x1234,
            remote_teid: 0x5678,
            local_address: "192.168.1.1:2152".parse().unwrap(),
            remote_address: "192.168.1.2:2152".parse().unwrap(),
            idle_timeout: Duration::from_secs(300),
        };

        let tunnel = Tunnel::new(config).unwrap();

        // First packet always valid
        assert!(tunnel.check_sequence(100));

        // Newer packet valid
        assert!(tunnel.check_sequence(101));

        // Duplicate packet rejected
        assert!(!tunnel.check_sequence(101));

        // Old packet within window accepted (first time)
        assert!(tunnel.check_sequence(100));

        // Duplicate old packet rejected (second time)
        assert!(!tunnel.check_sequence(100));
    }

    #[test]
    fn test_invalid_config() {
        let config = TunnelConfig {
            local_teid: 0, // Invalid!
            remote_teid: 0x5678,
            local_address: "192.168.1.1:2152".parse().unwrap(),
            remote_address: "192.168.1.2:2152".parse().unwrap(),
            idle_timeout: Duration::from_secs(300),
        };

        let result = Tunnel::new(config);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TunnelError::InvalidLocalTeid);
    }

    #[test]
    fn test_invalid_state_transition() {
        let config = TunnelConfig {
            local_teid: 0x1234,
            remote_teid: 0x5678,
            local_address: "192.168.1.1:2152".parse().unwrap(),
            remote_address: "192.168.1.2:2152".parse().unwrap(),
            idle_timeout: Duration::from_secs(300),
        };

        let tunnel = Tunnel::new(config).unwrap();

        // Try to go directly from Inactive to Active (should fail)
        let result = tunnel.transition_to(TunnelState::Active);
        assert!(result.is_err());
    }

    #[test]
    fn test_teid_allocation() {
        let manager = TunnelManager::new();

        let teid1 = manager.allocate_teid();
        let teid2 = manager.allocate_teid();
        let teid3 = manager.allocate_teid();

        // TEIDs should be sequential
        assert_eq!(teid2, teid1 + 1);
        assert_eq!(teid3, teid2 + 1);
    }

    #[test]
    fn test_manager_update_stats() {
        let mut manager = TunnelManager::new();

        let config = TunnelConfig {
            local_teid: manager.allocate_teid(),
            remote_teid: 0x5678,
            local_address: "192.168.1.1:2152".parse().unwrap(),
            remote_address: "192.168.1.2:2152".parse().unwrap(),
            idle_timeout: Duration::from_secs(300),
        };

        let teid = manager.create_tunnel(config).unwrap();

        manager.update_stats(teid, 100, true);
        manager.update_stats(teid, 200, false);

        let tunnel = manager.get_tunnel(teid).unwrap();
        assert_eq!(tunnel.stats.packets_tx(), 1);
        assert_eq!(tunnel.stats.packets_rx(), 1);
        assert_eq!(tunnel.stats.bytes_tx(), 100);
        assert_eq!(tunnel.stats.bytes_rx(), 200);
    }
}
