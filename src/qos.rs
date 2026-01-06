//! QoS Flow Support for 5G
//!
//! This module implements Quality of Service (QoS) flow management as per
//! 3GPP TS 23.501 for 5G System Architecture.
//!
//! # QoS Concepts
//!
//! - **5QI (5G QoS Identifier)**: Standardized QoS profile (e.g., voice, video, data)
//! - **QFI (QoS Flow Identifier)**: Unique identifier for QoS flows (0-63)
//! - **GBR (Guaranteed Bit Rate)**: QoS flows with guaranteed bandwidth
//! - **Non-GBR**: QoS flows without bandwidth guarantees
//! - **ARP (Allocation and Retention Priority)**: Priority for resource allocation
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────┐
//! │       QosFlowManager                   │
//! │  - HashMap<QFI, QosFlow>               │
//! │  - Thread-safe operations              │
//! ├────────────────────────────────────────┤
//! │       QosFlow                          │
//! │  - QosFlowParams (5QI, ARP, rates)     │
//! │  - QosFlowStats (packets, delays)      │
//! │  - Active/inactive state               │
//! └────────────────────────────────────────┘
//! ```

use crate::tunnel::Qfi;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ============================================================================
// 5G QoS Characteristics (5QI)
// ============================================================================

/// 5G QoS Identifier (5QI) values as per 3GPP TS 23.501 Table 5.7.4-1
///
/// These standardized values define QoS characteristics including:
/// - Priority level
/// - Packet delay budget
/// - Packet error rate
/// - Default maximum data burst volume
/// - Default averaging window
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum QosCharacteristics {
    // ========================================================================
    // GBR (Guaranteed Bit Rate) Resource Types (1-9)
    // ========================================================================
    /// Conversational Voice (5QI 1)
    /// - Priority: 20
    /// - Packet Delay Budget: 100ms
    /// - Packet Error Rate: 10^-2
    ConversationalVoice = 1,

    /// Conversational Video (Live streaming) (5QI 2)
    /// - Priority: 40
    /// - Packet Delay Budget: 150ms
    /// - Packet Error Rate: 10^-3
    ConversationalVideo = 2,

    /// Real-time Gaming (5QI 3)
    /// - Priority: 30
    /// - Packet Delay Budget: 50ms
    /// - Packet Error Rate: 10^-3
    RealTimeGaming = 3,

    /// Non-conversational Video (Buffered streaming) (5QI 4)
    /// - Priority: 50
    /// - Packet Delay Budget: 300ms
    /// - Packet Error Rate: 10^-6
    NonConversationalVideo = 4,

    /// IMS Signaling (5QI 5)
    /// - Priority: 10
    /// - Packet Delay Budget: 100ms
    /// - Packet Error Rate: 10^-6
    ImsSignaling = 5,

    /// Video (TCP-based) (Buffered streaming) (5QI 6)
    /// - Priority: 60
    /// - Packet Delay Budget: 300ms
    /// - Packet Error Rate: 10^-6
    VideoBufferedStreaming = 6,

    /// Voice, Video (Live streaming), Interactive Gaming (5QI 7)
    /// - Priority: 70
    /// - Packet Delay Budget: 100ms
    /// - Packet Error Rate: 10^-3
    VoicePremium = 7,

    /// Video (TCP-based), e.g., www, e-mail, chat, ftp, p2p (5QI 8)
    /// - Priority: 80
    /// - Packet Delay Budget: 300ms
    /// - Packet Error Rate: 10^-6
    VideoInteractiveGaming = 8,

    /// Video (TCP-based) (Buffered streaming) (5QI 9)
    /// - Priority: 90
    /// - Packet Delay Budget: 300ms (may be relaxed for buffered streaming)
    /// - Packet Error Rate: 10^-6
    VideoNonConversational = 9,

    // ========================================================================
    // Non-GBR Resource Types
    // ========================================================================
    /// Default bearer (5QI 10, reused value)
    /// - Priority: Configurable (default: 127 lowest)
    DefaultBearer = 10,

    /// IMS Video (5QI 65)
    ImsVideo = 65,

    /// TCP-based (e.g., www, e-mail, chat, ftp, p2p file sharing) (5QI 66)
    TcpBased = 66,

    /// Live video streaming (5QI 67)
    LiveVideoStreaming = 67,

    /// Premium messaging (e.g., chat) (5QI 69)
    PremiumMessaging = 69,

    /// Mission Critical user plane Push To Talk voice (5QI 70)
    MissionCriticalPushToTalk = 70,

    /// Mission Critical video user plane (5QI 71)
    MissionCriticalVideo = 71,

    /// Mission Critical Data (5QI 72)
    MissionCriticalData = 72,

    /// V2X messages (5QI 73)
    V2xMessages = 73,

    /// Low Latency eMBB applications (5QI 74)
    LowLatencyEmbb = 74,

    /// Ultra-low Latency eMBB applications (5QI 75)
    UltraLowLatency = 75,

    /// Electricity distribution (5QI 76)
    ElectricityDistribution = 76,
}

impl QosCharacteristics {
    /// Check if this QoS characteristic is a GBR (Guaranteed Bit Rate) type
    ///
    /// GBR flows (5QI 1-9) require guaranteed bandwidth allocation.
    pub fn is_gbr(&self) -> bool {
        (*self as u8) >= 1 && (*self as u8) <= 9
    }

    /// Check if this QoS characteristic is delay-reliant
    ///
    /// Delay-reliant flows have strict packet delay budget requirements.
    pub fn is_delay_reliant(&self) -> bool {
        matches!(
            self,
            QosCharacteristics::ConversationalVoice
                | QosCharacteristics::ConversationalVideo
                | QosCharacteristics::RealTimeGaming
                | QosCharacteristics::ImsSignaling
                | QosCharacteristics::MissionCriticalPushToTalk
                | QosCharacteristics::MissionCriticalVideo
                | QosCharacteristics::V2xMessages
                | QosCharacteristics::UltraLowLatency
        )
    }

    /// Get the 5QI value as u8
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Create from u8 value
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(QosCharacteristics::ConversationalVoice),
            2 => Some(QosCharacteristics::ConversationalVideo),
            3 => Some(QosCharacteristics::RealTimeGaming),
            4 => Some(QosCharacteristics::NonConversationalVideo),
            5 => Some(QosCharacteristics::ImsSignaling),
            6 => Some(QosCharacteristics::VideoBufferedStreaming),
            7 => Some(QosCharacteristics::VoicePremium),
            8 => Some(QosCharacteristics::VideoInteractiveGaming),
            9 => Some(QosCharacteristics::VideoNonConversational),
            10 => Some(QosCharacteristics::DefaultBearer),
            65 => Some(QosCharacteristics::ImsVideo),
            66 => Some(QosCharacteristics::TcpBased),
            67 => Some(QosCharacteristics::LiveVideoStreaming),
            69 => Some(QosCharacteristics::PremiumMessaging),
            70 => Some(QosCharacteristics::MissionCriticalPushToTalk),
            71 => Some(QosCharacteristics::MissionCriticalVideo),
            72 => Some(QosCharacteristics::MissionCriticalData),
            73 => Some(QosCharacteristics::V2xMessages),
            74 => Some(QosCharacteristics::LowLatencyEmbb),
            75 => Some(QosCharacteristics::UltraLowLatency),
            76 => Some(QosCharacteristics::ElectricityDistribution),
            _ => None,
        }
    }
}

// ============================================================================
// Allocation and Retention Priority (ARP)
// ============================================================================

/// Allocation and Retention Priority (ARP)
///
/// Controls resource allocation and preemption behavior as per 3GPP TS 23.501.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocationRetentionPriority {
    /// Priority level: 1 (highest) to 15 (lowest)
    pub priority_level: u8,
    /// Can this bearer preempt other bearers?
    pub preemption_capability: bool,
    /// Can this bearer be preempted by others?
    pub preemption_vulnerability: bool,
}

impl AllocationRetentionPriority {
    /// Create a new ARP with the given priority level
    ///
    /// Default: no preemption capability, vulnerable to preemption
    pub fn new(priority: u8) -> Self {
        Self {
            priority_level: priority,
            preemption_capability: false,
            preemption_vulnerability: true,
        }
    }

    /// Create a new ARP with full control
    pub fn new_with_preemption(
        priority: u8,
        can_preempt: bool,
        vulnerable: bool,
    ) -> Self {
        Self {
            priority_level: priority,
            preemption_capability: can_preempt,
            preemption_vulnerability: vulnerable,
        }
    }
}

// ============================================================================
// QoS Flow Parameters
// ============================================================================

/// QoS Flow Parameters
///
/// Complete set of parameters defining a QoS flow's characteristics.
#[derive(Debug, Clone, PartialEq)]
pub struct QosFlowParams {
    /// QoS Flow Identifier (0-63)
    pub qfi: Qfi,
    /// 5G QoS Identifier (5QI)
    pub fiveqi: QosCharacteristics,
    /// Allocation and Retention Priority
    pub arp: AllocationRetentionPriority,

    // GBR parameters (required for GBR flows, optional for Non-GBR)
    /// Guaranteed Flow Bit Rate - Uplink (bits per second)
    pub gfbr_uplink: Option<u64>,
    /// Guaranteed Flow Bit Rate - Downlink (bits per second)
    pub gfbr_downlink: Option<u64>,
    /// Maximum Flow Bit Rate - Uplink (bits per second)
    pub mfbr_uplink: Option<u64>,
    /// Maximum Flow Bit Rate - Downlink (bits per second)
    pub mfbr_downlink: Option<u64>,

    // Additional QoS parameters
    /// Packet Delay Budget (milliseconds)
    pub packet_delay_budget: u16,
    /// Packet Error Rate (10^-x, e.g., 3 means 10^-3)
    pub packet_error_rate: u8,
    /// Averaging Window (milliseconds)
    pub averaging_window: u16,
}

impl QosFlowParams {
    /// Create a new QoS flow with default parameters
    pub fn new(qfi: Qfi, fiveqi: QosCharacteristics, arp: AllocationRetentionPriority) -> Self {
        Self {
            qfi,
            fiveqi,
            arp,
            gfbr_uplink: None,
            gfbr_downlink: None,
            mfbr_uplink: None,
            mfbr_downlink: None,
            packet_delay_budget: 100,
            packet_error_rate: 3,
            averaging_window: 2000,
        }
    }

    /// Check if this is a GBR flow
    pub fn is_gbr(&self) -> bool {
        self.fiveqi.is_gbr()
    }

    /// Validate the QoS flow parameters
    pub fn validate(&self) -> Result<(), QosError> {
        // Validate QFI range
        if self.qfi > 63 {
            return Err(QosError::InvalidQfi);
        }

        // Validate ARP priority level
        if self.arp.priority_level > 15 || self.arp.priority_level < 1 {
            return Err(QosError::InvalidArpPriority);
        }

        // GBR flows must have bit rate parameters
        if self.is_gbr() {
            if self.gfbr_uplink.is_none() || self.gfbr_downlink.is_none() {
                return Err(QosError::MissingGbrParameters);
            }
        }

        Ok(())
    }
}

// ============================================================================
// QoS Errors
// ============================================================================

/// QoS-related errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QosError {
    /// Invalid QFI (must be 0-63)
    InvalidQfi,
    /// Invalid ARP priority (must be 1-15)
    InvalidArpPriority,
    /// Missing GBR parameters for GBR flow
    MissingGbrParameters,
}

impl std::fmt::Display for QosError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QosError::InvalidQfi => write!(f, "Invalid QFI (must be 0-63)"),
            QosError::InvalidArpPriority => write!(f, "Invalid ARP priority (must be 1-15)"),
            QosError::MissingGbrParameters => {
                write!(f, "Missing GBR parameters (GFBR uplink/downlink required)")
            }
        }
    }
}

impl std::error::Error for QosError {}

// ============================================================================
// QoS Flow Statistics
// ============================================================================

/// Inner statistics data (protected by Mutex)
#[derive(Debug, Clone)]
struct StatsInner {
    packets_transmitted: u64,
    packets_received: u64,
    bytes_transmitted: u64,
    bytes_received: u64,
    packets_dropped: u64,
    /// Reserved for future use - packets that exceeded delay budget
    #[allow(dead_code)]
    packets_delayed: u64,

    // Timing statistics
    min_delay_us: u64,
    max_delay_us: u64,
    avg_delay_us: u64,
    delay_samples: u64,
}

/// Thread-safe QoS flow statistics
#[derive(Debug, Clone)]
pub struct QosFlowStats {
    inner: Arc<Mutex<StatsInner>>,
}

impl QosFlowStats {
    /// Create new statistics
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(StatsInner {
                packets_transmitted: 0,
                packets_received: 0,
                bytes_transmitted: 0,
                bytes_received: 0,
                packets_dropped: 0,
                packets_delayed: 0,
                min_delay_us: u64::MAX,
                max_delay_us: 0,
                avg_delay_us: 0,
                delay_samples: 0,
            })),
        }
    }

    /// Record a packet
    pub fn record_packet(&self, bytes: usize, delay_us: u64, is_tx: bool) {
        let mut stats = self.inner.lock().unwrap();

        if is_tx {
            stats.packets_transmitted += 1;
            stats.bytes_transmitted += bytes as u64;
        } else {
            stats.packets_received += 1;
            stats.bytes_received += bytes as u64;
        }

        // Update delay statistics
        stats.delay_samples += 1;
        if delay_us < stats.min_delay_us {
            stats.min_delay_us = delay_us;
        }
        if delay_us > stats.max_delay_us {
            stats.max_delay_us = delay_us;
        }

        // Running average calculation
        let old_avg = stats.avg_delay_us;
        let n = stats.delay_samples;
        stats.avg_delay_us = (old_avg * (n - 1) + delay_us) / n;
    }

    /// Record a dropped packet
    pub fn record_drop(&self) {
        let mut stats = self.inner.lock().unwrap();
        stats.packets_dropped += 1;
    }

    /// Calculate packet loss rate
    pub fn packet_loss_rate(&self) -> f64 {
        let stats = self.inner.lock().unwrap();
        let total = stats.packets_transmitted + stats.packets_dropped;
        if total == 0 {
            return 0.0;
        }
        stats.packets_dropped as f64 / total as f64
    }

    /// Calculate throughput in bits per second
    pub fn throughput_bps(&self, elapsed_seconds: f64) -> f64 {
        let stats = self.inner.lock().unwrap();
        if elapsed_seconds <= 0.0 {
            return 0.0;
        }
        let total_bytes = stats.bytes_transmitted + stats.bytes_received;
        (total_bytes as f64) * 8.0 / elapsed_seconds
    }

    /// Get average delay in microseconds
    pub fn avg_delay_us(&self) -> u64 {
        self.inner.lock().unwrap().avg_delay_us
    }

    /// Get minimum delay in microseconds
    pub fn min_delay_us(&self) -> u64 {
        self.inner.lock().unwrap().min_delay_us
    }

    /// Get maximum delay in microseconds
    pub fn max_delay_us(&self) -> u64 {
        self.inner.lock().unwrap().max_delay_us
    }

    /// Get packet counts
    pub fn get_packet_counts(&self) -> (u64, u64, u64) {
        let stats = self.inner.lock().unwrap();
        (stats.packets_transmitted, stats.packets_received, stats.packets_dropped)
    }
}

impl Default for QosFlowStats {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// QoS Flow
// ============================================================================

/// QoS Flow
///
/// Represents a single QoS flow with parameters, statistics, and state.
#[derive(Debug, Clone)]
pub struct QosFlow {
    /// Flow parameters
    pub params: QosFlowParams,
    /// Flow statistics
    pub stats: QosFlowStats,
    /// Active state
    active: Arc<Mutex<bool>>,
}

impl QosFlow {
    /// Create a new QoS flow
    ///
    /// # Errors
    ///
    /// Returns an error if the parameters are invalid.
    pub fn new(params: QosFlowParams) -> Result<Self, QosError> {
        params.validate()?;

        Ok(Self {
            params,
            stats: QosFlowStats::new(),
            active: Arc::new(Mutex::new(true)),
        })
    }

    /// Check if the flow can accept a packet
    pub fn can_accept_packet(&self, _packet_size: usize) -> bool {
        *self.active.lock().unwrap()
    }

    /// Check if a packet should be dropped based on QoS policy
    pub fn should_drop(&self) -> bool {
        if !*self.active.lock().unwrap() {
            return true;
        }

        // Check if we're exceeding packet error rate
        let plr = self.stats.packet_loss_rate();
        let threshold = 10f64.powf(-(self.params.packet_error_rate as f64));

        // Allow 50% margin above threshold
        plr > threshold * 1.5
    }

    /// Check if the flow exceeds its delay budget
    pub fn exceeds_delay_budget(&self) -> bool {
        let avg_delay_ms = self.stats.avg_delay_us() / 1000;
        avg_delay_ms > self.params.packet_delay_budget as u64
    }

    /// Check if the flow is active
    pub fn is_active(&self) -> bool {
        *self.active.lock().unwrap()
    }

    /// Set the flow active state
    pub fn set_active(&self, active: bool) {
        *self.active.lock().unwrap() = active;
    }
}

// ============================================================================
// QoS Flow Manager
// ============================================================================

/// QoS Flow Manager
///
/// Thread-safe management of multiple QoS flows.
#[derive(Debug)]
pub struct QosFlowManager {
    flows: Arc<Mutex<HashMap<Qfi, QosFlow>>>,
}

impl QosFlowManager {
    /// Create a new QoS flow manager
    pub fn new() -> Self {
        Self {
            flows: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new QoS flow
    ///
    /// # Errors
    ///
    /// Returns an error if the flow parameters are invalid.
    pub fn create_flow(&self, params: QosFlowParams) -> Result<(), QosError> {
        let flow = QosFlow::new(params.clone())?;
        let mut flows = self.flows.lock().unwrap();
        flows.insert(params.qfi, flow);
        Ok(())
    }

    /// Remove a QoS flow
    pub fn remove_flow(&self, qfi: Qfi) {
        let mut flows = self.flows.lock().unwrap();
        flows.remove(&qfi);
    }

    /// Get a QoS flow
    pub fn get_flow(&self, qfi: Qfi) -> Option<QosFlow> {
        let flows = self.flows.lock().unwrap();
        flows.get(&qfi).cloned()
    }

    /// Classify a packet to determine its QFI
    ///
    /// In a real implementation, this would perform Deep Packet Inspection (DPI)
    /// or use flow rules to classify traffic.
    pub fn classify_packet(&self, _packet_data: &[u8]) -> Qfi {
        // Default QFI for unclassified traffic
        9 // Default bearer
    }

    /// Get the number of active flows
    pub fn get_active_flows(&self) -> usize {
        let flows = self.flows.lock().unwrap();
        flows.values().filter(|f| f.is_active()).count()
    }

    /// Get all flow QFIs
    pub fn get_flow_qfis(&self) -> Vec<Qfi> {
        let flows = self.flows.lock().unwrap();
        flows.keys().copied().collect()
    }
}

impl Default for QosFlowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for QosFlowManager {
    fn clone(&self) -> Self {
        Self {
            flows: Arc::clone(&self.flows),
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
    fn test_qos_characteristics() {
        assert!(QosCharacteristics::ConversationalVoice.is_gbr());
        assert!(!QosCharacteristics::TcpBased.is_gbr());
        assert!(QosCharacteristics::ConversationalVoice.is_delay_reliant());
        assert!(!QosCharacteristics::TcpBased.is_delay_reliant());
    }

    #[test]
    fn test_qos_characteristics_from_u8() {
        assert_eq!(
            QosCharacteristics::from_u8(1),
            Some(QosCharacteristics::ConversationalVoice)
        );
        assert_eq!(
            QosCharacteristics::from_u8(9),
            Some(QosCharacteristics::VideoNonConversational)
        );
        assert_eq!(
            QosCharacteristics::from_u8(10),
            Some(QosCharacteristics::DefaultBearer)
        );
        assert_eq!(QosCharacteristics::from_u8(255), None);
    }

    #[test]
    fn test_qos_flow_validation_non_gbr() {
        let flow = QosFlowParams::new(
            9,
            QosCharacteristics::DefaultBearer,
            AllocationRetentionPriority::new(5),
        );
        assert!(flow.validate().is_ok());
    }

    #[test]
    fn test_qos_flow_validation_gbr_missing_params() {
        let flow = QosFlowParams::new(
            1,
            QosCharacteristics::ConversationalVoice,
            AllocationRetentionPriority::new(5),
        );
        // Missing gfbr_uplink and gfbr_downlink
        assert_eq!(flow.validate().unwrap_err(), QosError::MissingGbrParameters);
    }

    #[test]
    fn test_qos_flow_validation_gbr_with_params() {
        let mut flow = QosFlowParams::new(
            1,
            QosCharacteristics::ConversationalVoice,
            AllocationRetentionPriority::new(5),
        );
        flow.gfbr_uplink = Some(64000);
        flow.gfbr_downlink = Some(64000);
        assert!(flow.validate().is_ok());
    }

    #[test]
    fn test_qos_flow_manager() {
        let manager = QosFlowManager::new();

        let params = QosFlowParams::new(
            9,
            QosCharacteristics::DefaultBearer,
            AllocationRetentionPriority::new(5),
        );

        manager.create_flow(params).unwrap();
        assert!(manager.get_flow(9).is_some());
        assert_eq!(manager.get_active_flows(), 1);

        manager.remove_flow(9);
        assert!(manager.get_flow(9).is_none());
        assert_eq!(manager.get_active_flows(), 0);
    }

    #[test]
    fn test_qos_flow_stats() {
        let stats = QosFlowStats::new();

        stats.record_packet(1000, 50, true);
        stats.record_packet(2000, 100, false);
        stats.record_packet(1500, 75, true);

        let (tx, rx, dropped) = stats.get_packet_counts();
        assert_eq!(tx, 2);
        assert_eq!(rx, 1);
        assert_eq!(dropped, 0);

        assert_eq!(stats.min_delay_us(), 50);
        assert_eq!(stats.max_delay_us(), 100);
        assert_eq!(stats.avg_delay_us(), 75);
    }

    #[test]
    fn test_qos_flow_packet_loss_rate() {
        let stats = QosFlowStats::new();

        stats.record_packet(1000, 50, true);
        stats.record_packet(1000, 50, true);
        stats.record_drop();

        let plr = stats.packet_loss_rate();
        assert!((plr - 0.333).abs() < 0.01); // ~33.3%
    }

    #[test]
    fn test_allocation_retention_priority() {
        let arp = AllocationRetentionPriority::new(5);
        assert_eq!(arp.priority_level, 5);
        assert!(!arp.preemption_capability);
        assert!(arp.preemption_vulnerability);

        let arp2 = AllocationRetentionPriority::new_with_preemption(3, true, false);
        assert_eq!(arp2.priority_level, 3);
        assert!(arp2.preemption_capability);
        assert!(!arp2.preemption_vulnerability);
    }

    #[test]
    fn test_invalid_qfi() {
        let flow = QosFlowParams::new(
            64, // Invalid QFI (must be 0-63)
            QosCharacteristics::DefaultBearer,
            AllocationRetentionPriority::new(5),
        );
        assert_eq!(flow.validate().unwrap_err(), QosError::InvalidQfi);
    }

    #[test]
    fn test_invalid_arp_priority() {
        let mut flow = QosFlowParams::new(
            9,
            QosCharacteristics::DefaultBearer,
            AllocationRetentionPriority::new(5),
        );
        flow.arp.priority_level = 16; // Invalid (must be 1-15)
        assert_eq!(flow.validate().unwrap_err(), QosError::InvalidArpPriority);
    }

    #[test]
    fn test_qos_flow_active_state() {
        let params = QosFlowParams::new(
            9,
            QosCharacteristics::DefaultBearer,
            AllocationRetentionPriority::new(5),
        );
        let flow = QosFlow::new(params).unwrap();

        assert!(flow.is_active());
        flow.set_active(false);
        assert!(!flow.is_active());
        assert!(!flow.can_accept_packet(1000));
    }
}
