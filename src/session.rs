//! GTP-U Session Management
//!
//! This module manages PDU (Protocol Data Unit) sessions and their associated tunnels
//! following 3GPP TS 23.501 for 5G System Architecture.
//!
//! # PDU Sessions
//!
//! A PDU session represents a logical connection between the UE (User Equipment)
//! and a Data Network (DN), identified by a DNN (Data Network Name). Each session
//! has:
//! - Uplink tunnel (gNB → UPF)
//! - Downlink tunnel (UPF → gNB)
//! - QoS flows for traffic differentiation
//! - Session type (IPv4, IPv6, IPv4v6, Ethernet, Unstructured)
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────┐
//! │       SessionManager                   │
//! │  - HashMap<PduSessionId, PduSession>   │
//! │  - TunnelManager reference             │
//! │  - Create/remove sessions              │
//! ├────────────────────────────────────────┤
//! │       PduSession                       │
//! │  - Uplink/downlink tunnels (TEIDs)     │
//! │  - QoS flow management                 │
//! │  - State machine                       │
//! │  - DNN (Data Network Name)             │
//! └────────────────────────────────────────┘
//! ```
//!
//! # Examples
//!
//! ```rust
//! use rs_gtp_u::session::{SessionManager, PduSessionType};
//! use rs_gtp_u::tunnel::TunnelManager;
//!
//! let mut tunnel_mgr = TunnelManager::new();
//! let mut session_mgr = SessionManager::new(&mut tunnel_mgr);
//!
//! let local_addr = "192.168.1.1:2152".parse().unwrap();
//! let remote_addr = "192.168.1.2:2152".parse().unwrap();
//!
//! session_mgr.create_session(
//!     1,
//!     PduSessionType::Ipv4,
//!     "internet",
//!     local_addr,
//!     remote_addr,
//! ).unwrap();
//! ```

use crate::qos::{QosFlowManager, QosFlowParams};
use crate::tunnel::{Qfi, Teid, TunnelConfig, TunnelManager};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ============================================================================
// Type Aliases
// ============================================================================

/// PDU Session ID (0-255)
pub type PduSessionId = u8;

// ============================================================================
// Re-exports for Convenience
// ============================================================================

// Re-export QoS types for easy access from session module
pub use crate::qos::{AllocationRetentionPriority, QosCharacteristics, QosError};

// ============================================================================
// PDU Session Type
// ============================================================================

/// PDU Session Type as per 3GPP TS 23.501
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PduSessionType {
    /// IPv4 session
    Ipv4,
    /// IPv6 session
    Ipv6,
    /// Dual-stack IPv4v6 session
    Ipv4v6,
    /// Ethernet session
    Ethernet,
    /// Unstructured session
    Unstructured,
}

impl std::fmt::Display for PduSessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PduSessionType::Ipv4 => write!(f, "IPv4"),
            PduSessionType::Ipv6 => write!(f, "IPv6"),
            PduSessionType::Ipv4v6 => write!(f, "IPv4v6"),
            PduSessionType::Ethernet => write!(f, "Ethernet"),
            PduSessionType::Unstructured => write!(f, "Unstructured"),
        }
    }
}

// ============================================================================
// Session State
// ============================================================================

/// PDU Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Not yet established
    Inactive,
    /// Being established
    Establishing,
    /// Fully operational
    Active,
    /// Being modified (QoS update, etc.)
    Modifying,
    /// Being released
    Releasing,
    /// Released/deleted
    Released,
}

// ============================================================================
// Session Errors
// ============================================================================

/// Session-related errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// Invalid state transition
    InvalidStateTransition,
    /// Tunnels not configured (missing uplink or downlink)
    TunnelsNotConfigured,
    /// Session already exists with this ID
    SessionAlreadyExists,
    /// Tunnel creation failed
    TunnelCreationFailed(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::InvalidStateTransition => write!(f, "Invalid state transition"),
            SessionError::TunnelsNotConfigured => {
                write!(f, "Tunnels not configured (missing uplink or downlink)")
            }
            SessionError::SessionAlreadyExists => write!(f, "Session already exists"),
            SessionError::TunnelCreationFailed(msg) => {
                write!(f, "Tunnel creation failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for SessionError {}

// ============================================================================
// PDU Session
// ============================================================================

/// PDU Session
///
/// Represents a logical connection between UE and Data Network, with
/// associated uplink/downlink tunnels and QoS flows.
#[derive(Debug, Clone)]
pub struct PduSession {
    /// Session ID (0-255)
    pub id: PduSessionId,
    /// Session type (IPv4, IPv6, etc.)
    pub session_type: PduSessionType,
    /// Current state
    state: Arc<Mutex<SessionState>>,
    /// Uplink tunnel TEID (gNB → UPF)
    uplink_tunnel: Arc<Mutex<Option<Teid>>>,
    /// Downlink tunnel TEID (UPF → gNB)
    downlink_tunnel: Arc<Mutex<Option<Teid>>>,
    /// QoS flow manager
    qos_manager: QosFlowManager,
    /// UE IP address (assigned by UPF)
    ue_ip: Arc<Mutex<Option<IpAddr>>>,
    /// Data Network Name (DNN) - equivalent to APN in 4G
    pub dnn: String,
    /// Creation timestamp
    created_at: Instant,
    /// Last modification timestamp
    modified_at: Arc<Mutex<Instant>>,
}

impl PduSession {
    /// Create a new PDU session
    pub fn new(id: PduSessionId, session_type: PduSessionType, dnn: &str) -> Self {
        let now = Instant::now();
        Self {
            id,
            session_type,
            state: Arc::new(Mutex::new(SessionState::Inactive)),
            uplink_tunnel: Arc::new(Mutex::new(None)),
            downlink_tunnel: Arc::new(Mutex::new(None)),
            qos_manager: QosFlowManager::new(),
            ue_ip: Arc::new(Mutex::new(None)),
            dnn: dnn.to_string(),
            created_at: now,
            modified_at: Arc::new(Mutex::new(now)),
        }
    }

    /// Set uplink tunnel TEID
    pub fn set_uplink_tunnel(&self, teid: Teid) {
        *self.uplink_tunnel.lock().unwrap() = Some(teid);
        *self.modified_at.lock().unwrap() = Instant::now();
    }

    /// Set downlink tunnel TEID
    pub fn set_downlink_tunnel(&self, teid: Teid) {
        *self.downlink_tunnel.lock().unwrap() = Some(teid);
        *self.modified_at.lock().unwrap() = Instant::now();
    }

    /// Get uplink tunnel TEID
    pub fn get_uplink_tunnel(&self) -> Option<Teid> {
        *self.uplink_tunnel.lock().unwrap()
    }

    /// Get downlink tunnel TEID
    pub fn get_downlink_tunnel(&self) -> Option<Teid> {
        *self.downlink_tunnel.lock().unwrap()
    }

    /// Set UE IP address
    pub fn set_ue_ip(&self, ip: IpAddr) {
        *self.ue_ip.lock().unwrap() = Some(ip);
        *self.modified_at.lock().unwrap() = Instant::now();
    }

    /// Get UE IP address
    pub fn get_ue_ip(&self) -> Option<IpAddr> {
        *self.ue_ip.lock().unwrap()
    }

    /// Add a QoS flow to the session
    ///
    /// # Errors
    ///
    /// Returns an error if the QoS flow parameters are invalid.
    pub fn add_qos_flow(&self, params: QosFlowParams) -> Result<(), crate::qos::QosError> {
        self.qos_manager.create_flow(params)?;
        *self.modified_at.lock().unwrap() = Instant::now();
        Ok(())
    }

    /// Remove a QoS flow from the session
    pub fn remove_qos_flow(&self, qfi: Qfi) {
        self.qos_manager.remove_flow(qfi);
        *self.modified_at.lock().unwrap() = Instant::now();
    }

    /// Get number of active QoS flows
    pub fn get_active_qos_flows(&self) -> usize {
        self.qos_manager.get_active_flows()
    }

    /// Activate the session
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - State is not inactive or establishing
    /// - Uplink or downlink tunnel is not configured
    pub fn activate(&self) -> Result<(), SessionError> {
        let mut state = self.state.lock().unwrap();
        if *state != SessionState::Inactive && *state != SessionState::Establishing {
            return Err(SessionError::InvalidStateTransition);
        }

        if self.uplink_tunnel.lock().unwrap().is_none()
            || self.downlink_tunnel.lock().unwrap().is_none()
        {
            return Err(SessionError::TunnelsNotConfigured);
        }

        *state = SessionState::Active;
        *self.modified_at.lock().unwrap() = Instant::now();
        Ok(())
    }

    /// Release the session
    pub fn release(&self) {
        *self.state.lock().unwrap() = SessionState::Releasing;
        *self.modified_at.lock().unwrap() = Instant::now();
    }

    /// Check if session is active
    pub fn is_active(&self) -> bool {
        *self.state.lock().unwrap() == SessionState::Active
    }

    /// Get current state
    pub fn get_state(&self) -> SessionState {
        *self.state.lock().unwrap()
    }

    /// Get session lifetime
    pub fn get_lifetime(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }
}

// ============================================================================
// Session Manager
// ============================================================================

/// Session Manager
///
/// Manages multiple PDU sessions with automatic tunnel creation and cleanup.
#[derive(Debug)]
pub struct SessionManager<'a> {
    sessions: Arc<Mutex<HashMap<PduSessionId, PduSession>>>,
    tunnel_manager: &'a mut TunnelManager,
}

impl<'a> SessionManager<'a> {
    /// Create a new session manager
    pub fn new(tunnel_manager: &'a mut TunnelManager) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            tunnel_manager,
        }
    }

    /// Create a new PDU session with uplink and downlink tunnels
    ///
    /// # Arguments
    ///
    /// * `id` - PDU session ID (0-255)
    /// * `session_type` - Session type (IPv4, IPv6, etc.)
    /// * `dnn` - Data Network Name
    /// * `local_addr` - Local GTP-U endpoint address
    /// * `remote_addr` - Remote GTP-U endpoint address
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Session already exists with this ID
    /// - Tunnel creation fails
    pub fn create_session(
        &mut self,
        id: PduSessionId,
        session_type: PduSessionType,
        dnn: &str,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
    ) -> Result<(), SessionError> {
        let mut sessions = self.sessions.lock().unwrap();

        // Check if session already exists
        if sessions.contains_key(&id) {
            return Err(SessionError::SessionAlreadyExists);
        }

        // Create session
        let session = PduSession::new(id, session_type, dnn);

        // Create uplink tunnel (gNB -> UPF)
        let ul_config = TunnelConfig {
            local_teid: self.tunnel_manager.allocate_teid(),
            remote_teid: 0, // Will be set later via signaling
            local_address: local_addr,
            remote_address: remote_addr,
            idle_timeout: std::time::Duration::from_secs(300),
        };

        let ul_teid = self
            .tunnel_manager
            .create_tunnel(ul_config)
            .map_err(|e| SessionError::TunnelCreationFailed(e.to_string()))?;

        // Create downlink tunnel (UPF -> gNB)
        let dl_config = TunnelConfig {
            local_teid: self.tunnel_manager.allocate_teid(),
            remote_teid: 0, // Will be set later via signaling
            local_address: local_addr,
            remote_address: remote_addr,
            idle_timeout: std::time::Duration::from_secs(300),
        };

        let dl_teid = self
            .tunnel_manager
            .create_tunnel(dl_config)
            .map_err(|e| SessionError::TunnelCreationFailed(e.to_string()))?;

        session.set_uplink_tunnel(ul_teid);
        session.set_downlink_tunnel(dl_teid);

        sessions.insert(id, session);
        Ok(())
    }

    /// Get a session by ID
    pub fn get_session(&self, id: PduSessionId) -> Option<PduSession> {
        let sessions = self.sessions.lock().unwrap();
        sessions.get(&id).cloned()
    }

    /// Remove a session and its associated tunnels
    pub fn remove_session(&mut self, id: PduSessionId) -> Result<(), SessionError> {
        let mut sessions = self.sessions.lock().unwrap();

        if let Some(session) = sessions.get(&id) {
            // Release tunnels
            if let Some(ul_teid) = session.get_uplink_tunnel() {
                self.tunnel_manager
                    .remove_tunnel(ul_teid)
                    .map_err(|e| SessionError::TunnelCreationFailed(e.to_string()))?;
            }
            if let Some(dl_teid) = session.get_downlink_tunnel() {
                self.tunnel_manager
                    .remove_tunnel(dl_teid)
                    .map_err(|e| SessionError::TunnelCreationFailed(e.to_string()))?;
            }

            sessions.remove(&id);
        }

        Ok(())
    }

    /// Get total session count
    pub fn get_session_count(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }

    /// Get active session count
    pub fn get_active_sessions(&self) -> usize {
        let sessions = self.sessions.lock().unwrap();
        sessions.values().filter(|s| s.is_active()).count()
    }

    /// Find session by tunnel TEID
    ///
    /// Returns the session that owns the specified tunnel (uplink or downlink).
    pub fn find_session_by_teid(&self, teid: Teid) -> Option<PduSession> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .values()
            .find(|s| {
                s.get_uplink_tunnel() == Some(teid) || s.get_downlink_tunnel() == Some(teid)
            })
            .cloned()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdu_session_lifecycle() {
        let session = PduSession::new(1, PduSessionType::Ipv4, "internet");

        assert_eq!(session.id, 1);
        assert_eq!(session.session_type, PduSessionType::Ipv4);
        assert_eq!(session.get_state(), SessionState::Inactive);
        assert_eq!(session.dnn, "internet");
    }

    #[test]
    fn test_session_manager_lifecycle() {
        let mut tunnel_mgr = TunnelManager::new();
        let mut session_mgr = SessionManager::new(&mut tunnel_mgr);

        let local_addr = "192.168.1.1:2152".parse().unwrap();
        let remote_addr = "192.168.1.2:2152".parse().unwrap();

        session_mgr
            .create_session(1, PduSessionType::Ipv4, "internet", local_addr, remote_addr)
            .unwrap();

        assert_eq!(session_mgr.get_session_count(), 1);

        let session = session_mgr.get_session(1).unwrap();
        assert!(session.get_uplink_tunnel().is_some());
        assert!(session.get_downlink_tunnel().is_some());

        session_mgr.remove_session(1).unwrap();
        assert_eq!(session_mgr.get_session_count(), 0);
    }

    #[test]
    fn test_session_with_qos_flows() {
        let session = PduSession::new(1, PduSessionType::Ipv4, "internet");

        let qos_params = QosFlowParams::new(
            9,
            QosCharacteristics::DefaultBearer,
            AllocationRetentionPriority::new(5),
        );

        session.add_qos_flow(qos_params).unwrap();
        assert_eq!(session.get_active_qos_flows(), 1);

        session.remove_qos_flow(9);
        assert_eq!(session.get_active_qos_flows(), 0);
    }

    #[test]
    fn test_ipv6_pdu_session() {
        let session = PduSession::new(2, PduSessionType::Ipv6, "internet");

        assert_eq!(session.id, 2);
        assert_eq!(session.session_type, PduSessionType::Ipv6);
        assert_eq!(session.get_state(), SessionState::Inactive);
        assert_eq!(session.dnn, "internet");
    }

    #[test]
    fn test_dual_stack_ipv4v6_pdu_session() {
        let session = PduSession::new(3, PduSessionType::Ipv4v6, "internet");

        assert_eq!(session.id, 3);
        assert_eq!(session.session_type, PduSessionType::Ipv4v6);
        assert_eq!(session.get_state(), SessionState::Inactive);
    }

    #[test]
    fn test_session_manager_with_ipv6() {
        let mut tunnel_mgr = TunnelManager::new();
        let mut session_mgr = SessionManager::new(&mut tunnel_mgr);

        let local_addr = "[2001:db8::1]:2152".parse().unwrap();
        let remote_addr = "[2001:db8::2]:2152".parse().unwrap();

        session_mgr
            .create_session(1, PduSessionType::Ipv6, "internet", local_addr, remote_addr)
            .unwrap();

        assert_eq!(session_mgr.get_session_count(), 1);

        let session = session_mgr.get_session(1).unwrap();
        assert_eq!(session.session_type, PduSessionType::Ipv6);
        assert!(session.get_uplink_tunnel().is_some());
        assert!(session.get_downlink_tunnel().is_some());

        session_mgr.remove_session(1).unwrap();
        assert_eq!(session_mgr.get_session_count(), 0);
    }

    #[test]
    fn test_session_activation() {
        let session = PduSession::new(1, PduSessionType::Ipv4, "internet");

        session.set_uplink_tunnel(0x1000);
        session.set_downlink_tunnel(0x2000);

        assert!(!session.is_active());
        session.activate().unwrap();
        assert!(session.is_active());
    }

    #[test]
    fn test_session_activation_without_tunnels() {
        let session = PduSession::new(1, PduSessionType::Ipv4, "internet");

        // Try to activate without setting tunnels
        let result = session.activate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SessionError::TunnelsNotConfigured);
    }

    #[test]
    fn test_find_session_by_teid() {
        let mut tunnel_mgr = TunnelManager::new();
        let mut session_mgr = SessionManager::new(&mut tunnel_mgr);

        let local_addr = "192.168.1.1:2152".parse().unwrap();
        let remote_addr = "192.168.1.2:2152".parse().unwrap();

        session_mgr
            .create_session(1, PduSessionType::Ipv4, "internet", local_addr, remote_addr)
            .unwrap();

        let session = session_mgr.get_session(1).unwrap();
        let ul_teid = session.get_uplink_tunnel().unwrap();

        let found = session_mgr.find_session_by_teid(ul_teid).unwrap();
        assert_eq!(found.id, 1);
    }

    #[test]
    fn test_ue_ip_assignment() {
        let session = PduSession::new(1, PduSessionType::Ipv4, "internet");

        assert!(session.get_ue_ip().is_none());

        let ue_ip = "10.0.0.1".parse().unwrap();
        session.set_ue_ip(ue_ip);

        assert_eq!(session.get_ue_ip(), Some(ue_ip));
    }

    #[test]
    fn test_session_already_exists() {
        let mut tunnel_mgr = TunnelManager::new();
        let mut session_mgr = SessionManager::new(&mut tunnel_mgr);

        let local_addr = "192.168.1.1:2152".parse().unwrap();
        let remote_addr = "192.168.1.2:2152".parse().unwrap();

        session_mgr
            .create_session(1, PduSessionType::Ipv4, "internet", local_addr, remote_addr)
            .unwrap();

        // Try to create again with same ID
        let result = session_mgr.create_session(
            1,
            PduSessionType::Ipv4,
            "internet",
            local_addr,
            remote_addr,
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SessionError::SessionAlreadyExists);
    }
}
