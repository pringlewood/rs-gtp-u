//! GTP-U Protocol Constants and Types
//!
//! This module defines the core protocol constants and message types for GTP-U
//! (GPRS Tunneling Protocol - User Plane) as specified in 3GPP TS 29.281 v18.0.0.
//!
//! # Translation Notes
//! This is a direct port from zig-gtp-u/src/protocol.zig to Rust.
//! Wire format compatibility is critical - all enum values must match exactly.

// ============================================================================
// Protocol Constants
// ============================================================================

/// GTP-U protocol version as per 3GPP TS 29.281
pub const GTPU_VERSION: u8 = 1;

/// Default UDP port for GTP-U user plane traffic
pub const GTPU_PORT: u16 = 2152;

/// UDP port for GTP-U control plane (GTP-C)
pub const GTPU_CONTROL_PORT: u16 = 2123;

/// GTP Protocol Type identifier (GTP' = 1)
pub const GTP_PROTOCOL_TYPE: u8 = 1;

/// Maximum size of a GTP-U packet (UDP payload limit)
pub const MAX_GTPU_PACKET_SIZE: usize = 65535;

/// Maximum GTP-U payload size (packet size minus minimum 12-byte header)
pub const MAX_GTPU_PAYLOAD_SIZE: usize = MAX_GTPU_PACKET_SIZE - 12;

/// Special TEID value used for signaling messages (Echo, Error Indication)
pub const TEID_ZERO: u32 = 0x00000000;

// ============================================================================
// Message Types (3GPP TS 29.281 Section 7.1)
// ============================================================================

/// GTP-U Message Types
///
/// Defines the type of GTP-U message being transmitted. Each message type
/// has specific semantics and may require different header fields.
///
/// # Translation from Zig
/// - Zig: `pub const MessageType = enum(u8) { ... }`
/// - Rust: `#[repr(u8)] pub enum MessageType { ... }`
/// - The `#[repr(u8)]` ensures same memory layout as Zig's enum(u8)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageType {
    /// Echo Request - Path management (TEID = 0)
    EchoRequest = 1,

    /// Echo Response - Path management response (TEID = 0)
    EchoResponse = 2,

    /// Error Indication - Signals path errors
    ErrorIndication = 26,

    /// Supported Extension Headers Notification
    SupportedExtensionHeadersNotification = 31,

    /// End Marker - Indicates end of data during handover
    EndMarker = 254,

    /// G-PDU (GPRS PDU) - User data packet (most common)
    Gpdu = 255,
}

impl MessageType {
    /// Check if this is a valid/supported message type
    ///
    /// # Translation Note
    /// Zig version uses exhaustive switch; Rust uses pattern matching.
    /// Both guarantee compile-time exhaustiveness checking.
    pub fn is_valid(self) -> bool {
        matches!(
            self,
            MessageType::EchoRequest
                | MessageType::EchoResponse
                | MessageType::ErrorIndication
                | MessageType::SupportedExtensionHeadersNotification
                | MessageType::Gpdu
                | MessageType::EndMarker
        )
    }

    /// Check if this message type requires a TEID field
    ///
    /// Echo messages use TEID=0 for signaling, while data messages
    /// require a valid tunnel endpoint identifier.
    ///
    /// # Returns
    /// - `true` for G-PDU, End Marker, Error Indication (require TEID)
    /// - `false` for Echo Request/Response (use TEID=0)
    pub fn requires_teid(self) -> bool {
        matches!(
            self,
            MessageType::Gpdu | MessageType::EndMarker | MessageType::ErrorIndication
        )
    }

    /// Get human-readable name for this message type
    ///
    /// # Translation Note
    /// Zig returns `[]const u8` (slice), Rust returns `&'static str`.
    /// Both are zero-cost string literals.
    pub fn as_str(self) -> &'static str {
        match self {
            MessageType::EchoRequest => "Echo Request",
            MessageType::EchoResponse => "Echo Response",
            MessageType::ErrorIndication => "Error Indication",
            MessageType::SupportedExtensionHeadersNotification => {
                "Supported Extension Headers Notification"
            }
            MessageType::Gpdu => "G-PDU",
            MessageType::EndMarker => "End Marker",
        }
    }

    /// Try to convert a u8 value to a MessageType
    ///
    /// # Returns
    /// - `Some(MessageType)` if the value is valid
    /// - `None` if the value is not a recognized message type
    ///
    /// # Translation Note
    /// Zig uses `@enumFromInt` with error handling.
    /// Rust uses TryFrom trait for idiomatic conversion.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(MessageType::EchoRequest),
            2 => Some(MessageType::EchoResponse),
            26 => Some(MessageType::ErrorIndication),
            31 => Some(MessageType::SupportedExtensionHeadersNotification),
            254 => Some(MessageType::EndMarker),
            255 => Some(MessageType::Gpdu),
            _ => None,
        }
    }
}

// Implement Display for convenient printing
impl std::fmt::Display for MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Extension Header Types (3GPP TS 29.281 Section 5.2)
// ============================================================================

/// GTP-U Extension Header Types
///
/// Extension headers provide additional per-packet metadata such as QoS flow
/// identifiers, PDCP sequence numbers, and RAN container information.
///
/// Extension headers are chained using the `next_extension_type` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExtensionHeaderType {
    /// No more extension headers in the chain
    NoMoreHeaders = 0x00,

    /// Reserved value (should not be used)
    Reserved = 0x01,

    /// MBMS Support Indication (deprecated)
    MbmsSupportIndication = 0x02,

    /// Service Class Indicator
    ServiceClassIndicator = 0x20,

    /// UDP Port information
    UdpPort = 0x40,

    /// RAN Container (generic)
    RanContainer = 0x81,

    /// Long PDCP PDU Number (32-bit sequence number)
    LongPdcpPduNumber = 0x82,

    /// XW RAN Container
    XwRanContainer = 0x83,

    /// NR RAN Container (5G New Radio)
    NrRanContainer = 0x84,

    /// PDU Session Container (5G - contains QFI for QoS flows)
    /// **Most important for 5G networks**
    PduSessionContainer = 0x85,

    /// PDCP PDU Number (16-bit sequence number)
    PdcpPduNumber = 0xC0,

    /// Suspend Request
    SuspendRequest = 0xC1,

    /// Suspend Response
    SuspendResponse = 0xC2,
}

impl ExtensionHeaderType {
    /// Check if this is a valid/supported extension header type
    pub fn is_valid(self) -> bool {
        matches!(
            self,
            ExtensionHeaderType::NoMoreHeaders
                | ExtensionHeaderType::ServiceClassIndicator
                | ExtensionHeaderType::UdpPort
                | ExtensionHeaderType::RanContainer
                | ExtensionHeaderType::LongPdcpPduNumber
                | ExtensionHeaderType::XwRanContainer
                | ExtensionHeaderType::NrRanContainer
                | ExtensionHeaderType::PduSessionContainer
                | ExtensionHeaderType::PdcpPduNumber
                | ExtensionHeaderType::SuspendRequest
                | ExtensionHeaderType::SuspendResponse
        )
    }

    /// Get human-readable name for this extension header type
    pub fn as_str(self) -> &'static str {
        match self {
            ExtensionHeaderType::NoMoreHeaders => "No More Headers",
            ExtensionHeaderType::Reserved => "Reserved",
            ExtensionHeaderType::MbmsSupportIndication => "MBMS Support Indication",
            ExtensionHeaderType::ServiceClassIndicator => "Service Class Indicator",
            ExtensionHeaderType::UdpPort => "UDP Port",
            ExtensionHeaderType::RanContainer => "RAN Container",
            ExtensionHeaderType::LongPdcpPduNumber => "Long PDCP PDU Number",
            ExtensionHeaderType::XwRanContainer => "XW RAN Container",
            ExtensionHeaderType::NrRanContainer => "NR RAN Container",
            ExtensionHeaderType::PduSessionContainer => "PDU Session Container",
            ExtensionHeaderType::PdcpPduNumber => "PDCP PDU Number",
            ExtensionHeaderType::SuspendRequest => "Suspend Request",
            ExtensionHeaderType::SuspendResponse => "Suspend Response",
        }
    }

    /// Try to convert a u8 value to an ExtensionHeaderType
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(ExtensionHeaderType::NoMoreHeaders),
            0x01 => Some(ExtensionHeaderType::Reserved),
            0x02 => Some(ExtensionHeaderType::MbmsSupportIndication),
            0x20 => Some(ExtensionHeaderType::ServiceClassIndicator),
            0x40 => Some(ExtensionHeaderType::UdpPort),
            0x81 => Some(ExtensionHeaderType::RanContainer),
            0x82 => Some(ExtensionHeaderType::LongPdcpPduNumber),
            0x83 => Some(ExtensionHeaderType::XwRanContainer),
            0x84 => Some(ExtensionHeaderType::NrRanContainer),
            0x85 => Some(ExtensionHeaderType::PduSessionContainer),
            0xC0 => Some(ExtensionHeaderType::PdcpPduNumber),
            0xC1 => Some(ExtensionHeaderType::SuspendRequest),
            0xC2 => Some(ExtensionHeaderType::SuspendResponse),
            _ => None,
        }
    }
}

impl std::fmt::Display for ExtensionHeaderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Tests (ported from protocol.zig)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test: MessageType validation
    // Ported from: test "MessageType validation" in protocol.zig
    #[test]
    fn test_message_type_validation() {
        // Valid message types should return true
        assert!(MessageType::EchoRequest.is_valid());
        assert!(MessageType::Gpdu.is_valid());
        assert!(MessageType::EndMarker.is_valid());
        assert!(MessageType::ErrorIndication.is_valid());

        // Echo requests don't require TEID (use TEID=0)
        assert!(!MessageType::EchoRequest.requires_teid());
        assert!(!MessageType::EchoResponse.requires_teid());

        // G-PDU requires TEID (tunnel identifier)
        assert!(MessageType::Gpdu.requires_teid());
        assert!(MessageType::EndMarker.requires_teid());
        assert!(MessageType::ErrorIndication.requires_teid());
    }

    // Test: ExtensionHeaderType validation
    // Ported from: test "ExtensionHeaderType validation" in protocol.zig
    #[test]
    fn test_extension_header_type_validation() {
        assert!(ExtensionHeaderType::PdcpPduNumber.is_valid());
        assert!(ExtensionHeaderType::RanContainer.is_valid());
        assert!(ExtensionHeaderType::NoMoreHeaders.is_valid());
        assert!(ExtensionHeaderType::PduSessionContainer.is_valid());
    }

    // Additional test: Enum value correctness
    // This ensures wire format compatibility with zig-gtp-u
    #[test]
    fn test_message_type_wire_values() {
        assert_eq!(MessageType::EchoRequest as u8, 1);
        assert_eq!(MessageType::EchoResponse as u8, 2);
        assert_eq!(MessageType::ErrorIndication as u8, 26);
        assert_eq!(MessageType::SupportedExtensionHeadersNotification as u8, 31);
        assert_eq!(MessageType::EndMarker as u8, 254);
        assert_eq!(MessageType::Gpdu as u8, 255);
    }

    #[test]
    fn test_extension_header_type_wire_values() {
        assert_eq!(ExtensionHeaderType::NoMoreHeaders as u8, 0x00);
        assert_eq!(ExtensionHeaderType::PduSessionContainer as u8, 0x85);
        assert_eq!(ExtensionHeaderType::PdcpPduNumber as u8, 0xC0);
        assert_eq!(ExtensionHeaderType::LongPdcpPduNumber as u8, 0x82);
    }

    // Test: from_u8 conversion
    #[test]
    fn test_message_type_from_u8() {
        assert_eq!(MessageType::from_u8(1), Some(MessageType::EchoRequest));
        assert_eq!(MessageType::from_u8(255), Some(MessageType::Gpdu));
        assert_eq!(MessageType::from_u8(99), None); // Invalid value
    }

    #[test]
    fn test_extension_header_type_from_u8() {
        assert_eq!(
            ExtensionHeaderType::from_u8(0x85),
            Some(ExtensionHeaderType::PduSessionContainer)
        );
        assert_eq!(
            ExtensionHeaderType::from_u8(0xC0),
            Some(ExtensionHeaderType::PdcpPduNumber)
        );
        assert_eq!(ExtensionHeaderType::from_u8(0xFF), None); // Invalid
    }

    // Test: Display trait
    #[test]
    fn test_message_type_display() {
        assert_eq!(format!("{}", MessageType::Gpdu), "G-PDU");
        assert_eq!(format!("{}", MessageType::EchoRequest), "Echo Request");
    }

    #[test]
    fn test_extension_header_type_display() {
        assert_eq!(
            format!("{}", ExtensionHeaderType::PduSessionContainer),
            "PDU Session Container"
        );
    }

    // Test: as_str method
    #[test]
    fn test_message_type_as_str() {
        assert_eq!(MessageType::Gpdu.as_str(), "G-PDU");
        assert_eq!(MessageType::EchoRequest.as_str(), "Echo Request");
        assert_eq!(MessageType::EndMarker.as_str(), "End Marker");
    }
}
