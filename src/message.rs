//! GTP-U Message Implementation
//!
//! 3GPP TS 29.281 Section 5
//!
//! This module provides the complete GTP-U message abstraction, combining headers,
//! extension headers, and payload into a cohesive message structure.
//!
//! # Translation Note
//!
//! Translated from `zig-gtp-u/src/message.zig`
//!
//! Key differences from Zig version:
//! - Rust ownership system eliminates need for `owns_payload` flag
//! - `Vec<T>` instead of `ArrayList(T)`
//! - `Drop` trait instead of explicit `deinit()`
//! - No allocator parameter (Rust manages memory automatically)
use crate::extension::{ExtensionHeader, PduSessionContainer};
use crate::header::GtpuHeader;
use crate::ie::InformationElement;
use crate::protocol::{ExtensionHeaderType, MessageType};
use std::io::{self, Read, Write};
use std::net::IpAddr;

/// GTP-U Message Structure
///
/// Represents a complete GTP-U message including:
/// - Mandatory header (8 or 12 bytes)
/// - Optional extension headers (for 5G QoS, PDCP, etc.)
/// - Payload (user data for G-PDU)
///
/// # Memory Management
///
/// Rust's ownership system automatically manages memory:
/// - Extension headers are owned by the Vec
/// - Payload is owned by the Vec<u8>
/// - Drop trait automatically cleans up when message goes out of scope
///
/// # Translation Note
///
/// Zig version uses explicit allocator and owns_payload flag.
/// Rust version uses ownership for cleaner memory management.
///
/// Source: `zig-gtp-u/src/message.zig` lines 13-260
#[derive(Debug, Clone)]
pub struct GtpuMessage {
    /// GTP-U header (mandatory and optional fields)
    pub header: GtpuHeader,

    /// Extension headers (PDU Session Container, PDCP, etc.)
    pub extension_headers: Vec<ExtensionHeader>,

    /// Information Elements (for signaling messages like Echo Response, Error Indication)
    pub information_elements: Vec<InformationElement>,

    /// Message payload (for G-PDU: user data; for others: may be empty)
    pub payload: Vec<u8>,
}

impl GtpuMessage {
    /// Creates a new GTP-U message
    ///
    /// # Arguments
    ///
    /// * `message_type` - Type of message (G-PDU, Echo Request, etc.)
    /// * `teid` - Tunnel Endpoint Identifier (0 for signaling messages)
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn init(allocator: std.mem.Allocator, message_type, teid)`
    /// Rust version: No allocator needed
    pub fn new(message_type: MessageType, teid: u32) -> Self {
        Self {
            header: GtpuHeader::new(message_type, teid),
            extension_headers: Vec::new(),
            information_elements: Vec::new(),
            payload: Vec::new(),
        }
    }

    /// Sets the message payload
    ///
    /// Replaces any existing payload and updates the header length field.
    ///
    /// # Translation Note
    ///
    /// Zig version takes a slice reference: `payload: []const u8`
    /// Rust version takes ownership or accepts anything that converts to Vec<u8>
    pub fn set_payload(&mut self, payload: Vec<u8>) {
        self.payload = payload;
        self.update_length();
    }

    /// Sets the message payload from a slice (copies the data)
    ///
    /// Convenience method for setting payload from borrowed data.
    pub fn set_payload_from_slice(&mut self, payload: &[u8]) {
        self.payload = payload.to_vec();
        self.update_length();
    }

    /// Adds an extension header to the message
    ///
    /// Extension headers are chained in order of addition.
    /// The first extension header added will have its type set in the GTP-U header.
    ///
    /// # Translation Note
    ///
    /// Zig version:
    /// ```zig
    /// pub fn addExtensionHeader(self: *GtpuMessage, ext: extension.ExtensionHeader) !void
    /// ```
    ///
    /// Rust version doesn't need error handling for Vec::push (can't fail in normal conditions)
    pub fn add_extension_header(&mut self, ext: ExtensionHeader) {
        // If this is the first extension header, set the next_extension_type in the header
        if self.extension_headers.is_empty() {
            // Extract the type from the extension header
            let ext_type = match &ext {
                ExtensionHeader::NoMoreHeaders => ExtensionHeaderType::NoMoreHeaders,
                ExtensionHeader::PduSessionContainer(_) => ExtensionHeaderType::PduSessionContainer,
                ExtensionHeader::PdcpPduNumber(_) => ExtensionHeaderType::PdcpPduNumber,
                ExtensionHeader::LongPdcpPduNumber(_) => ExtensionHeaderType::LongPdcpPduNumber,
                ExtensionHeader::ServiceClassIndicator(_) => ExtensionHeaderType::ServiceClassIndicator,
                ExtensionHeader::UdpPort(_) => ExtensionHeaderType::UdpPort,
            };

            self.header.next_extension_type = Some(ext_type);
            self.header.flags.set_s(true); // Extension headers require optional fields
        }

        self.extension_headers.push(ext);
        self.header.flags.set_e(true);
        self.update_length();
    }

    /// Adds an information element to the message
    ///
    /// Information elements are used in signaling messages like Echo Response,
    /// Error Indication, and Supported Extension Headers Notification.
    ///
    /// # Arguments
    ///
    /// * `ie` - Information element to add
    pub fn add_information_element(&mut self, ie: InformationElement) {
        self.information_elements.push(ie);
        self.update_length();
    }

    /// Updates the header length field based on current message content
    ///
    /// Calculates total length including:
    /// - Optional header fields (4 bytes if present)
    /// - Extension headers (variable length, 4-byte aligned)
    /// - Payload
    ///
    /// # Translation Note
    ///
    /// Zig version: `fn updateLength(self: *GtpuMessage) void`
    fn update_length(&mut self) {
        let mut length: usize = 0;

        // Optional header fields
        if self.header.flags.has_optional_fields() {
            length += 4;
        }

        // Extension headers
        for ext in &self.extension_headers {
            length += ext.size();
        }

        // Information elements
        for ie in &self.information_elements {
            length += ie.size();
        }

        // Payload
        length += self.payload.len();

        self.header.length = length as u16;
    }

    /// Encodes the message to wire format
    ///
    /// # Wire Format
    ///
    /// ```text
    /// ┌─────────────────┐
    /// │ GTP-U Header    │ 8 or 12 bytes
    /// ├─────────────────┤
    /// │ Extension       │ Variable (4-byte aligned)
    /// │ Headers         │
    /// ├─────────────────┤
    /// │ Payload         │ Variable
    /// └─────────────────┘
    /// ```
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn encode(self: *GtpuMessage, writer: anytype) !void`
    pub fn encode<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        // Update length before encoding
        self.update_length();

        // Encode header
        self.header.encode(writer)?;

        // Encode extension headers
        for (i, ext) in self.extension_headers.iter().enumerate() {
            // Determine next extension type
            let next_ext_type = if i + 1 < self.extension_headers.len() {
                // Get type of next extension header
                match &self.extension_headers[i + 1] {
                    ExtensionHeader::NoMoreHeaders => Some(ExtensionHeaderType::NoMoreHeaders),
                    ExtensionHeader::PduSessionContainer(_) => Some(ExtensionHeaderType::PduSessionContainer),
                    ExtensionHeader::PdcpPduNumber(_) => Some(ExtensionHeaderType::PdcpPduNumber),
                    ExtensionHeader::LongPdcpPduNumber(_) => Some(ExtensionHeaderType::LongPdcpPduNumber),
                    ExtensionHeader::ServiceClassIndicator(_) => Some(ExtensionHeaderType::ServiceClassIndicator),
                    ExtensionHeader::UdpPort(_) => Some(ExtensionHeaderType::UdpPort),
                }
            } else {
                None // Last extension header
            };

            ext.encode(writer, next_ext_type)?;
        }

        // Encode information elements
        for ie in &self.information_elements {
            ie.encode(writer)?;
        }

        // Encode payload
        writer.write_all(&self.payload)?;

        Ok(())
    }

    /// Decodes a message from wire format
    ///
    /// # Arguments
    ///
    /// * `reader` - Input reader
    ///
    /// # Returns
    ///
    /// Decoded GtpuMessage
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Header decoding fails
    /// - Invalid length field
    /// - Extension header decoding fails
    /// - Payload read fails
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn decode(allocator: std.mem.Allocator, reader: anytype) !GtpuMessage`
    pub fn decode<R: Read>(reader: &mut R) -> io::Result<Self> {
        // Decode header
        let hdr = GtpuHeader::decode(reader)?;

        let mut msg = GtpuMessage::new(hdr.message_type, hdr.teid);
        msg.header = hdr;

        // Calculate remaining length after header
        // The length field includes optional fields (4 bytes if present), which have already been consumed
        let mut remaining_length = msg.header.length as usize;
        if msg.header.flags.has_optional_fields() {
            if remaining_length >= 4 {
                remaining_length -= 4;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid length: too small for optional fields",
                ));
            }
        }

        // Decode extension headers if present
        if msg.header.flags.e() {
            let mut next_type = msg.header.next_extension_type.unwrap_or(ExtensionHeaderType::NoMoreHeaders);

            while next_type != ExtensionHeaderType::NoMoreHeaders {
                let decode_result = ExtensionHeader::decode(reader, next_type)?;
                let ext_size = decode_result.header.size();

                if ext_size > remaining_length {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Extension header size exceeds remaining length",
                    ));
                }

                remaining_length -= ext_size;
                msg.extension_headers.push(decode_result.header);
                next_type = decode_result.next_type;
            }
        }

        // For G-PDU, the rest is payload
        if msg.header.message_type == MessageType::Gpdu {
            let mut payload_buf = vec![0u8; remaining_length];
            reader.read_exact(&mut payload_buf)?;
            msg.payload = payload_buf;
        } else {
            // For other message types (Echo Response, Error Indication), decode IEs
            while remaining_length > 0 {
                let ie = InformationElement::decode(reader)?;
                let ie_size = ie.size();

                if ie_size > remaining_length {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "IE size exceeds remaining length",
                    ));
                }

                remaining_length -= ie_size;
                msg.information_elements.push(ie);
            }
        }

        Ok(msg)
    }

    // ========================================================================
    // Message Builders (Convenience Constructors)
    // ========================================================================

    /// Creates an Echo Request message
    ///
    /// Echo requests use TEID 0 and require a sequence number.
    /// Used for path management and keep-alive.
    ///
    /// # Arguments
    ///
    /// * `sequence` - Sequence number for matching response
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn createEchoRequest(allocator: std.mem.Allocator, sequence: u16) !GtpuMessage`
    pub fn create_echo_request(sequence: u16) -> Self {
        let mut msg = GtpuMessage::new(MessageType::EchoRequest, 0);
        msg.header.flags.set_s(true);
        msg.header.sequence_number = Some(sequence);
        msg
    }

    /// Creates a G-PDU (General Packet Data Unit) message
    ///
    /// G-PDU is the most common message type, carrying user plane data.
    ///
    /// # Arguments
    ///
    /// * `teid` - Tunnel Endpoint Identifier
    /// * `payload` - User data to encapsulate
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn createGpdu(allocator: std.mem.Allocator, teid: u32, payload: []const u8)`
    pub fn create_gpdu(teid: u32, payload: Vec<u8>) -> Self {
        let mut msg = GtpuMessage::new(MessageType::Gpdu, teid);
        msg.set_payload(payload);
        msg
    }

    /// Creates a G-PDU message from a payload slice (copies the data)
    ///
    /// Convenience method for creating G-PDU from borrowed data.
    pub fn create_gpdu_from_slice(teid: u32, payload: &[u8]) -> Self {
        let mut msg = GtpuMessage::new(MessageType::Gpdu, teid);
        msg.set_payload_from_slice(payload);
        msg
    }

    /// Creates an End Marker message
    ///
    /// End Markers indicate the end of a data stream during handover.
    ///
    /// # Arguments
    ///
    /// * `teid` - Tunnel Endpoint Identifier
    /// * `sequence` - Sequence number
    pub fn create_end_marker(teid: u32, sequence: u16) -> Self {
        let mut msg = GtpuMessage::new(MessageType::EndMarker, teid);
        msg.header.flags.set_s(true);
        msg.header.sequence_number = Some(sequence);
        msg
    }

    /// Creates an Echo Response message
    ///
    /// Echo responses match echo requests by sequence number and include a Recovery IE.
    ///
    /// # Arguments
    ///
    /// * `sequence` - Sequence number from matching Echo Request
    /// * `restart_counter` - Restart counter value for Recovery IE
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn createEchoResponse(allocator: std.mem.Allocator, sequence: u16) !GtpuMessage`
    pub fn create_echo_response(sequence: u16, restart_counter: u8) -> Self {
        let mut msg = GtpuMessage::new(MessageType::EchoResponse, 0);
        msg.header.flags.set_s(true);
        msg.header.sequence_number = Some(sequence);

        // Add Recovery IE
        let recovery = InformationElement::recovery(restart_counter);
        msg.add_information_element(recovery);

        msg
    }

    /// Creates an Error Indication message
    ///
    /// Error Indication messages report errors to a peer.
    ///
    /// # Arguments
    ///
    /// * `teid` - Tunnel Endpoint Identifier
    /// * `peer_address` - IP address of the peer that sent the erroneous packet
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn createErrorIndication(allocator, teid, peer_address) !GtpuMessage`
    pub fn create_error_indication(teid: u32, peer_address: IpAddr) -> Self {
        let mut msg = GtpuMessage::new(MessageType::ErrorIndication, teid);

        // Add Peer Address IE (GSN Address)
        let peer_addr_ie = InformationElement::gsn_address(peer_address);
        msg.add_information_element(peer_addr_ie);

        msg
    }

    // ========================================================================
    // Helper Methods (5G QoS Support)
    // ========================================================================

    /// Extracts QFI (QoS Flow Identifier) from PDU Session Container extension header
    ///
    /// Returns None if no PDU Session Container is present in the message.
    /// This is essential for 5G QoS enforcement and flow-based differentiation.
    ///
    /// # Returns
    ///
    /// - `Some(qfi)` - QFI value (0-63) if PDU Session Container exists
    /// - `None` - No PDU Session Container in message
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn getQFI(self: *const GtpuMessage) ?u6`
    /// Rust version: Returns u8 instead of u6 (Rust doesn't have u6)
    pub fn get_qfi(&self) -> Option<u8> {
        for ext in &self.extension_headers {
            if let ExtensionHeader::PduSessionContainer(psc) = ext {
                return Some(psc.qfi);
            }
        }
        None
    }

    /// Extracts full PDU Session Container information from the message
    ///
    /// Returns None if no PDU Session Container is present.
    /// Provides access to QFI, PDU type (UL/DL), PPI, and RQI fields.
    ///
    /// # Returns
    ///
    /// - `Some(&PduSessionContainer)` - Reference to PDU Session Container if exists
    /// - `None` - No PDU Session Container in message
    ///
    /// # Translation Note
    ///
    /// Zig version returns a copy: `?extension.PduSessionContainer`
    /// Rust version returns a reference to avoid copying
    pub fn get_pdu_session_container(&self) -> Option<&PduSessionContainer> {
        for ext in &self.extension_headers {
            if let ExtensionHeader::PduSessionContainer(psc) = ext {
                return Some(psc);
            }
        }
        None
    }

    /// Checks if message contains an extension header of a specific type
    ///
    /// # Arguments
    ///
    /// * `ext_type` - Extension header type to check for
    ///
    /// # Returns
    ///
    /// `true` if the message contains at least one extension header of the specified type
    pub fn has_extension_header_type(&self, ext_type: ExtensionHeaderType) -> bool {
        for ext in &self.extension_headers {
            let current_type = match ext {
                ExtensionHeader::NoMoreHeaders => ExtensionHeaderType::NoMoreHeaders,
                ExtensionHeader::PduSessionContainer(_) => ExtensionHeaderType::PduSessionContainer,
                ExtensionHeader::PdcpPduNumber(_) => ExtensionHeaderType::PdcpPduNumber,
                ExtensionHeader::LongPdcpPduNumber(_) => ExtensionHeaderType::LongPdcpPduNumber,
                ExtensionHeader::ServiceClassIndicator(_) => ExtensionHeaderType::ServiceClassIndicator,
                ExtensionHeader::UdpPort(_) => ExtensionHeaderType::UdpPort,
            };

            if current_type == ext_type {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::PduSessionContainer;
    use std::io::Cursor;

    #[test]
    fn test_message_new() {
        let msg = GtpuMessage::new(MessageType::Gpdu, 0x12345678);

        assert_eq!(msg.header.message_type, MessageType::Gpdu);
        assert_eq!(msg.header.teid, 0x12345678);
        assert!(msg.extension_headers.is_empty());
        assert!(msg.payload.is_empty());
    }

    #[test]
    fn test_message_echo_request() {
        let msg = GtpuMessage::create_echo_request(123);

        assert_eq!(msg.header.message_type, MessageType::EchoRequest);
        assert_eq!(msg.header.teid, 0);
        assert_eq!(msg.header.sequence_number, Some(123));
        assert!(msg.header.flags.s());
    }

    #[test]
    fn test_message_gpdu_encode_decode() {
        let payload = b"Hello, GTP-U!".to_vec();
        let mut msg = GtpuMessage::create_gpdu(0x12345678, payload.clone());

        // Encode
        let mut buffer = Vec::new();
        msg.encode(&mut buffer).unwrap();

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decoded = GtpuMessage::decode(&mut cursor).unwrap();

        assert_eq!(decoded.header.message_type, MessageType::Gpdu);
        assert_eq!(decoded.header.teid, 0x12345678);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn test_message_qfi_extraction() {
        let payload = b"test payload".to_vec();
        let mut msg = GtpuMessage::create_gpdu(0x12345678, payload);

        // Add PDU Session Container with QFI = 9
        let pdu_container = ExtensionHeader::PduSessionContainer(PduSessionContainer::new(
            1,     // Uplink
            9,     // QFI
            0,     // PPI
            false, // RQI
        ));
        msg.add_extension_header(pdu_container);

        // Test QFI extraction
        let qfi = msg.get_qfi();
        assert_eq!(qfi, Some(9));

        // Test PDU Session Container extraction
        let psc = msg.get_pdu_session_container();
        assert!(psc.is_some());
        assert_eq!(psc.unwrap().pdu_type, 1);
        assert_eq!(psc.unwrap().qfi, 9);

        // Test extension header type check
        assert!(msg.has_extension_header_type(ExtensionHeaderType::PduSessionContainer));
        assert!(!msg.has_extension_header_type(ExtensionHeaderType::PdcpPduNumber));
    }

    #[test]
    fn test_message_qfi_extraction_returns_none_when_no_pdu_container() {
        let payload = b"test payload".to_vec();
        let msg = GtpuMessage::create_gpdu(0x12345678, payload);

        // No extension headers added
        assert_eq!(msg.get_qfi(), None);
        assert!(msg.get_pdu_session_container().is_none());
        assert!(!msg.has_extension_header_type(ExtensionHeaderType::PduSessionContainer));
    }

    #[test]
    fn test_message_with_extension_header_encode_decode() {
        let payload = b"test data".to_vec();
        let mut msg = GtpuMessage::create_gpdu(0xABCDEF01, payload.clone());

        // Add PDU Session Container
        let pdu_container = ExtensionHeader::PduSessionContainer(PduSessionContainer::new(
            1,    // Uplink
            9,    // QFI
            0,    // PPI
            true, // RQI
        ));
        msg.add_extension_header(pdu_container);

        // Encode
        let mut buffer = Vec::new();
        msg.encode(&mut buffer).unwrap();

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decoded = GtpuMessage::decode(&mut cursor).unwrap();

        assert_eq!(decoded.header.message_type, MessageType::Gpdu);
        assert_eq!(decoded.header.teid, 0xABCDEF01);
        assert_eq!(decoded.payload, payload);
        assert_eq!(decoded.extension_headers.len(), 1);

        // Verify QFI extraction works after decode
        assert_eq!(decoded.get_qfi(), Some(9));
    }

    #[test]
    fn test_message_update_length() {
        let mut msg = GtpuMessage::new(MessageType::Gpdu, 0x12345678);

        // Initially empty
        assert_eq!(msg.header.length, 0);

        // Add payload
        msg.set_payload_from_slice(b"Hello");
        assert_eq!(msg.header.length, 5);

        // Add extension header
        let pdu_container = ExtensionHeader::PduSessionContainer(PduSessionContainer::new(1, 9, 0, false));
        msg.add_extension_header(pdu_container);

        // Length should include optional header (4 bytes) + extension (4 bytes) + payload (5 bytes) = 13
        assert_eq!(msg.header.length, 13);
    }

    #[test]
    fn test_echo_request_encode_decode() {
        let mut request = GtpuMessage::create_echo_request(123);

        // Encode
        let mut buffer = Vec::new();
        request.encode(&mut buffer).unwrap();

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decoded = GtpuMessage::decode(&mut cursor).unwrap();

        assert_eq!(decoded.header.message_type, MessageType::EchoRequest);
        assert_eq!(decoded.header.sequence_number, Some(123));
    }

    #[test]
    fn test_end_marker_creation() {
        let msg = GtpuMessage::create_end_marker(0x11223344, 999);

        assert_eq!(msg.header.message_type, MessageType::EndMarker);
        assert_eq!(msg.header.teid, 0x11223344);
        assert_eq!(msg.header.sequence_number, Some(999));
        assert!(msg.header.flags.s());
    }

    #[test]
    fn test_echo_response_encode_decode() {
        let mut response = GtpuMessage::create_echo_response(123, 42);

        assert_eq!(response.header.message_type, MessageType::EchoResponse);
        assert_eq!(response.header.teid, 0);
        assert_eq!(response.header.sequence_number, Some(123));
        assert_eq!(response.information_elements.len(), 1);

        // Encode
        let mut buffer = Vec::new();
        response.encode(&mut buffer).unwrap();

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decoded = GtpuMessage::decode(&mut cursor).unwrap();

        assert_eq!(decoded.header.message_type, MessageType::EchoResponse);
        assert_eq!(decoded.header.sequence_number, Some(123));
        assert_eq!(decoded.information_elements.len(), 1);

        // Verify Recovery IE
        if let InformationElement::Recovery(restart_counter) = &decoded.information_elements[0] {
            assert_eq!(*restart_counter, 42);
        } else {
            panic!("Expected Recovery IE");
        }
    }

    #[test]
    fn test_error_indication() {
        use std::net::{IpAddr, Ipv4Addr};

        let peer_addr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let mut msg = GtpuMessage::create_error_indication(0x12345678, peer_addr);

        assert_eq!(msg.header.message_type, MessageType::ErrorIndication);
        assert_eq!(msg.header.teid, 0x12345678);
        assert_eq!(msg.information_elements.len(), 1);

        // Encode
        let mut buffer = Vec::new();
        msg.encode(&mut buffer).unwrap();

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decoded = GtpuMessage::decode(&mut cursor).unwrap();

        assert_eq!(decoded.header.message_type, MessageType::ErrorIndication);
        assert_eq!(decoded.information_elements.len(), 1);

        // Verify GSN Address IE
        let decoded_addr = decoded.information_elements[0].as_ip_addr().unwrap();
        assert_eq!(decoded_addr, peer_addr);
    }
}
