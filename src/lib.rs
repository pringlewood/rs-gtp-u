//! rs-gtp-u: High-Performance GTP-U Protocol Implementation for 5G Networks
//!
//! This library provides a complete implementation of the GTP-U (GPRS Tunneling Protocol - User Plane)
//! as specified in 3GPP TS 29.281 v18.0.0. It is designed for building User Plane Functions (UPF)
//! in 5G core networks.
//!
//! # Translation from zig-gtp-u
//!
//! This library is a direct port of the zig-gtp-u implementation, maintaining:
//! - Wire format compatibility (critical for interoperability)
//! - API design patterns (adapted to Rust idioms)
//! - Performance characteristics (zero-copy where possible)
//! - Test coverage (all tests ported)
//!
//! # Architecture
//!
//! The library is organized in layers, mirroring the original Zig implementation:
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │   Application (UPF, gNB simulator)      │
//! ├─────────────────────────────────────────┤
//! │  Management Layer                       │
//! │  - session: PDU session management      │
//! │  - tunnel: Tunnel state machine         │
//! │  - path: Echo/RTT monitoring            │
//! │  - qos: QoS flow management             │
//! ├─────────────────────────────────────────┤
//! │  Message Layer                          │
//! │  - message: GtpuMessage abstraction     │
//! │  - extension: Extension headers         │
//! │  - ie: Information Elements             │
//! ├─────────────────────────────────────────┤
//! │  Protocol Layer                         │
//! │  - protocol: Constants, enums           │
//! │  - header: Wire format encode/decode    │
//! ├─────────────────────────────────────────┤
//! │  Utilities                              │
//! │  - pool: Memory pooling                 │
//! │  - utils: TEID gen, anti-replay         │
//! └─────────────────────────────────────────┘
//! ```
//!
//! # Quick Start
//!
//! ```rust
//! use rs_gtp_u::protocol::{MessageType, GTPU_PORT};
//!
//! // Check message type properties
//! assert!(MessageType::Gpdu.requires_teid());
//! assert!(!MessageType::EchoRequest.requires_teid());
//!
//! // Display message type
//! println!("Message: {}", MessageType::Gpdu);  // "G-PDU"
//! ```
//!
//! # Feature Modules (Roadmap)
//!
//! - ✅ `protocol` - Protocol constants and message types
//! - ✅ `header` - GTP-U header encoding/decoding
//! - ✅ `extension` - Extension header support (PDU Session Container, etc.)
//! - ✅ `message` - Complete message abstraction
//! - ✅ `ie` - Information Elements for signaling messages
//! - ✅ `tunnel` - Tunnel management with state machine
//! - ✅ `session` - PDU session management
//! - ✅ `qos` - QoS flow support (5QI, GBR/Non-GBR)
//! - ✅ `path` - Path management with Echo/RTT monitoring
//! - ✅ `utils` - TEID generation, anti-replay window, rate limiting
//! - ✅ `pool` - Memory pooling for zero-copy operations
//!
//! # 3GPP Compliance
//!
//! This implementation strictly follows:
//! - **3GPP TS 29.281 v18.0.0**: GTP-U Protocol
//! - **3GPP TS 23.501 v18.0.0**: 5G System Architecture
//!
//! All message types, extension headers, and wire formats are compliant with these specifications.

#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(unsafe_op_in_unsafe_fn)]

// ============================================================================
// Module Declarations
// ============================================================================

pub mod protocol;
pub mod header;
pub mod extension;
pub mod message;
pub mod ie;
pub mod tunnel;
pub mod session;
pub mod qos;
pub mod path;
pub mod utils;
pub mod pool;

// ============================================================================
// Re-exports for Convenience
// ============================================================================

// Re-export commonly used types from protocol module
pub use protocol::{
    ExtensionHeaderType, MessageType, GTPU_CONTROL_PORT, GTPU_PORT, GTPU_VERSION,
    GTP_PROTOCOL_TYPE, MAX_GTPU_PACKET_SIZE, MAX_GTPU_PAYLOAD_SIZE, TEID_ZERO,
};

// Re-export commonly used types from header module
pub use header::{GtpuHeader, HeaderFlags};

// Re-export commonly used types from extension module
pub use extension::{
    ExtensionHeader, LongPdcpPduNumber, PdcpPduNumber, PduSessionContainer,
    ServiceClassIndicator, UdpPortExtension,
};

// Re-export commonly used types from message module
pub use message::GtpuMessage;

// Re-export commonly used types from ie module
pub use ie::{Cause, IEType, InformationElement, PrivateExtension, QosProfile, UserLocationInfo};

// Re-export commonly used types from tunnel module
pub use tunnel::{Qfi, Teid, Tunnel, TunnelConfig, TunnelError, TunnelManager, TunnelState, TunnelStats};

// Re-export commonly used types from session module
pub use session::{PduSession, PduSessionId, PduSessionType, SessionError, SessionManager, SessionState};

// Re-export commonly used types from qos module
pub use qos::{
    AllocationRetentionPriority, QosCharacteristics, QosError, QosFlow, QosFlowManager,
    QosFlowParams, QosFlowStats,
};

// Re-export commonly used types from path module
pub use path::{Path, PathConfig, PathError, PathManager, PathState, PathStats};

// Re-export commonly used types from utils module
pub use utils::{
    calculate_ipv4_checksum, AntiReplayWindow, CircularBuffer, CircularBufferError, RateLimiter,
    TeidGenerator,
};

// Re-export commonly used types from pool module
pub use pool::{
    BatchProcessor, BatchStats, EncodedMessage, MemoryPool, MessageBatch, PacketBuffer,
    PacketBufferPool, PooledPacketBuffer,
};

// ============================================================================
// Version Information
// ============================================================================

/// Library version following semantic versioning
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Major version number
pub const VERSION_MAJOR: &str = env!("CARGO_PKG_VERSION_MAJOR");

/// Minor version number
pub const VERSION_MINOR: &str = env!("CARGO_PKG_VERSION_MINOR");

/// Patch version number
pub const VERSION_PATCH: &str = env!("CARGO_PKG_VERSION_PATCH");

// ============================================================================
// Library-wide Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info() {
        // Ensure version info is accessible
        assert!(!VERSION.is_empty());
        println!("rs-gtp-u version: {}", VERSION);
    }

    #[test]
    fn test_re_exports() {
        // Test that re-exported constants are accessible
        assert_eq!(GTPU_VERSION, 1);
        assert_eq!(GTPU_PORT, 2152);

        // Test that re-exported types are accessible
        let msg_type = MessageType::Gpdu;
        assert!(msg_type.is_valid());
    }

    #[test]
    fn test_basic_usage() {
        // Demonstrate basic library usage
        let echo = MessageType::EchoRequest;
        assert!(!echo.requires_teid());
        assert_eq!(echo.as_str(), "Echo Request");

        let gpdu = MessageType::Gpdu;
        assert!(gpdu.requires_teid());
        assert_eq!(gpdu as u8, 255);
    }
}
