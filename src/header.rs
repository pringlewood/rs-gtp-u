//! GTP-U Header Implementation
//!
//! This module implements the GTP-U header structure and encoding/decoding
//! as specified in 3GPP TS 29.281 Section 5.
//!
//! # Wire Format
//!
//! ```text
//! Mandatory Header (8 bytes):
//! ┌────────────┬────────────┬─────────────┬─────────────┐
//! │ Flags (1B) │ Type (1B)  │ Length (2B) │ TEID (4B)   │
//! └────────────┴────────────┴─────────────┴─────────────┘
//!
//! Optional Header (4 bytes, if flags.s OR flags.e OR flags.pn):
//! ┌─────────────┬──────────┬──────────────┐
//! │ Seq# (2B)   │ N-PDU (1B│ Next Ext (1B)│
//! └─────────────┴──────────┴──────────────┘
//!
//! Flags byte (bit layout):
//! ┌───┬───┬───┬───┬───┬───┬───┬───┐
//! │Ver│Ver│Ver│PT │Spr│E  │S  │PN │
//! └───┴───┴───┴───┴───┴───┴───┴───┘
//!  7   6   5   4   3   2   1   0
//! ```
//!
//! # Translation from Zig
//!
//! This is a direct port from zig-gtp-u/src/header.zig with the following mappings:
//! - Zig `packed struct(u8)` → Rust manual bit manipulation
//! - Zig `anytype` reader/writer → Rust trait bounds
//! - Zig `?T` → Rust `Option<T>`

use crate::protocol::{ExtensionHeaderType, MessageType, GTPU_VERSION};
use std::io::{self, Read, Write};

// ============================================================================
// Header Flags
// ============================================================================

/// GTP-U Header Flags
///
/// These flags indicate which optional fields are present and control
/// protocol behavior.
///
/// # Bit Layout (LSB to MSB)
/// ```text
/// Bit 0: PN  - N-PDU Number present
/// Bit 1: S   - Sequence Number present
/// Bit 2: E   - Extension Header present
/// Bit 3: Spare (must be 0)
/// Bit 4: PT  - Protocol Type (always 1 for GTP)
/// Bit 5-7: Version (always 1 for GTP-U)
/// ```
///
/// # Translation Note
/// Zig uses `packed struct(u8)` with bit-level fields. Rust doesn't have
/// direct equivalent, so we use manual bit manipulation with clear methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderFlags {
    raw: u8,
}

impl HeaderFlags {
    /// Create flags with default values (version=1, pt=1, others=0)
    pub fn new() -> Self {
        Self {
            raw: (GTPU_VERSION << 5) | (1 << 4), // Version=1, PT=1
        }
    }

    /// Create flags from raw byte (for decoding)
    pub fn from_raw(byte: u8) -> Self {
        Self { raw: byte }
    }

    /// Get raw byte value
    pub fn as_raw(self) -> u8 {
        self.raw
    }

    // Getters

    /// Get PN (N-PDU Number) flag
    pub fn pn(self) -> bool {
        (self.raw & 0b0000_0001) != 0
    }

    /// Get S (Sequence Number) flag
    pub fn s(self) -> bool {
        (self.raw & 0b0000_0010) != 0
    }

    /// Get E (Extension Header) flag
    pub fn e(self) -> bool {
        (self.raw & 0b0000_0100) != 0
    }

    /// Get PT (Protocol Type) field (always 1 for GTP)
    pub fn pt(self) -> u8 {
        (self.raw & 0b0001_0000) >> 4
    }

    /// Get GTP-U version (should always be 1)
    pub fn version(self) -> u8 {
        (self.raw & 0b1110_0000) >> 5
    }

    // Setters

    /// Set PN (N-PDU Number) flag
    pub fn set_pn(&mut self, value: bool) {
        if value {
            self.raw |= 0b0000_0001;
        } else {
            self.raw &= !0b0000_0001;
        }
    }

    /// Set S (Sequence Number) flag
    pub fn set_s(&mut self, value: bool) {
        if value {
            self.raw |= 0b0000_0010;
        } else {
            self.raw &= !0b0000_0010;
        }
    }

    /// Set E (Extension Header) flag
    pub fn set_e(&mut self, value: bool) {
        if value {
            self.raw |= 0b0000_0100;
        } else {
            self.raw &= !0b0000_0100;
        }
    }

    /// Check if any optional fields are present
    ///
    /// Ported from: `pub fn hasOptionalFields(self: HeaderFlags) bool`
    pub fn has_optional_fields(self) -> bool {
        self.pn() || self.s() || self.e()
    }
}

impl Default for HeaderFlags {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// GTP-U Header
// ============================================================================

/// GTP-U Header Structure
///
/// Contains all fields of a GTP-U header including optional fields.
/// The optional fields are only encoded/decoded if the corresponding
/// flags are set.
///
/// # Examples
///
/// ```
/// use rs_gtp_u::header::GtpuHeader;
/// use rs_gtp_u::protocol::MessageType;
///
/// // Create a basic G-PDU header
/// let header = GtpuHeader::new(MessageType::Gpdu, 0x12345678);
/// assert_eq!(header.teid, 0x12345678);
/// assert_eq!(header.size(), 8); // Mandatory only
///
/// // With sequence number
/// let mut header_with_seq = GtpuHeader::new(MessageType::Gpdu, 0x12345678);
/// header_with_seq.sequence_number = Some(42);
/// header_with_seq.flags.set_s(true);
/// assert_eq!(header_with_seq.size(), 12); // Mandatory + optional
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GtpuHeader {
    /// Header flags indicating version, protocol type, and optional fields
    pub flags: HeaderFlags,

    /// Message type (G-PDU, Echo Request, etc.)
    pub message_type: MessageType,

    /// Length of payload in bytes (NOT including mandatory header)
    pub length: u16,

    /// Tunnel Endpoint Identifier
    pub teid: u32,

    // Optional fields (present based on flags)
    /// Sequence number (present if flags.s == true)
    pub sequence_number: Option<u16>,

    /// N-PDU number (present if flags.pn == true)
    pub n_pdu_number: Option<u8>,

    /// Next extension header type (present if flags.e == true)
    pub next_extension_type: Option<ExtensionHeaderType>,
}

impl GtpuHeader {
    /// Size of mandatory header fields
    pub const MANDATORY_SIZE: usize = 8;

    /// Size of optional header fields (when present)
    pub const OPTIONAL_SIZE: usize = 4;

    /// Create a new GTP-U header with default values
    ///
    /// Ported from: `pub fn init(message_type, teid) GtpuHeader`
    ///
    /// # Examples
    ///
    /// ```
    /// # use rs_gtp_u::header::GtpuHeader;
    /// # use rs_gtp_u::protocol::MessageType;
    /// let header = GtpuHeader::new(MessageType::Gpdu, 0x12345678);
    /// assert_eq!(header.teid, 0x12345678);
    /// assert_eq!(header.length, 0);
    /// ```
    pub fn new(message_type: MessageType, teid: u32) -> Self {
        Self {
            flags: HeaderFlags::new(),
            message_type,
            length: 0,
            teid,
            sequence_number: None,
            n_pdu_number: None,
            next_extension_type: None,
        }
    }

    /// Calculate total size of header in bytes
    ///
    /// Ported from: `pub fn size(self: GtpuHeader) usize`
    ///
    /// Returns 8 bytes for mandatory header, plus 4 bytes if any
    /// optional fields are present.
    pub fn size(&self) -> usize {
        let mut result = Self::MANDATORY_SIZE;
        if self.flags.has_optional_fields() {
            result += Self::OPTIONAL_SIZE;
        }
        result
    }

    /// Encode header to bytes
    ///
    /// Ported from: `pub fn encode(self: GtpuHeader, writer: anytype) !void`
    ///
    /// Writes the header in network byte order (big-endian) as specified
    /// in 3GPP TS 29.281 Section 5.
    ///
    /// # Errors
    ///
    /// Returns `io::Error` if writing fails.
    ///
    /// # Translation Note
    /// - Zig `writer: anytype` → Rust `W: Write` trait bound
    /// - Zig `.big` endianness → Rust `to_be_bytes()`
    /// - Zig `@bitCast` → Rust `as_raw()`
    pub fn encode<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        // Mandatory part (8 bytes)
        writer.write_all(&[self.flags.as_raw()])?;
        writer.write_all(&[self.message_type as u8])?;
        writer.write_all(&self.length.to_be_bytes())?;
        writer.write_all(&self.teid.to_be_bytes())?;

        // Optional part (4 bytes) - only if flags indicate
        if self.flags.has_optional_fields() {
            writer.write_all(&self.sequence_number.unwrap_or(0).to_be_bytes())?;
            writer.write_all(&[self.n_pdu_number.unwrap_or(0)])?;

            let next_ext = if let Some(ext) = self.next_extension_type {
                ext as u8
            } else {
                0
            };
            writer.write_all(&[next_ext])?;
        }

        Ok(())
    }

    /// Decode header from bytes
    ///
    /// Ported from: `pub fn decode(reader: anytype) !GtpuHeader`
    ///
    /// Reads the header from network byte order (big-endian) and validates
    /// the version field.
    ///
    /// # Errors
    ///
    /// Returns:
    /// - `io::Error` if reading fails or buffer is too short
    /// - `io::ErrorKind::InvalidData` if version is invalid
    ///
    /// # Translation Note
    /// - Zig error union `!T` → Rust `Result<T, io::Error>`
    /// - Zig `@enumFromInt` → Rust `MessageType::from_u8()` with validation
    pub fn decode<R: Read>(reader: &mut R) -> io::Result<Self> {
        // Read mandatory part (8 bytes)
        let mut buf = [0u8; 1];

        // Flags byte
        reader.read_exact(&mut buf)?;
        let flags = HeaderFlags::from_raw(buf[0]);

        // Validate version
        if flags.version() != GTPU_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid GTP-U version: expected {}, got {}",
                    GTPU_VERSION,
                    flags.version()
                ),
            ));
        }

        // Message type
        reader.read_exact(&mut buf)?;
        let message_type = MessageType::from_u8(buf[0]).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid message type: 0x{:02X}", buf[0]),
            )
        })?;

        // Length (2 bytes, big-endian)
        let mut buf2 = [0u8; 2];
        reader.read_exact(&mut buf2)?;
        let length = u16::from_be_bytes(buf2);

        // TEID (4 bytes, big-endian)
        let mut buf4 = [0u8; 4];
        reader.read_exact(&mut buf4)?;
        let teid = u32::from_be_bytes(buf4);

        let mut header = Self {
            flags,
            message_type,
            length,
            teid,
            sequence_number: None,
            n_pdu_number: None,
            next_extension_type: None,
        };

        // Read optional part if present
        if flags.has_optional_fields() {
            // Sequence number (2 bytes)
            reader.read_exact(&mut buf2)?;
            header.sequence_number = Some(u16::from_be_bytes(buf2));

            // N-PDU number (1 byte)
            reader.read_exact(&mut buf)?;
            header.n_pdu_number = Some(buf[0]);

            // Next extension type (1 byte)
            reader.read_exact(&mut buf)?;
            header.next_extension_type = if buf[0] != 0 {
                ExtensionHeaderType::from_u8(buf[0])
            } else {
                None
            };
        }

        Ok(header)
    }
}

// Implement Display for nice formatting (port of Zig's format function)
impl std::fmt::Display for GtpuHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "GTP-U Header:")?;
        writeln!(f, "  Version: {}", self.flags.version())?;
        writeln!(
            f,
            "  Message Type: {} (0x{:02X})",
            self.message_type, self.message_type as u8
        )?;
        writeln!(f, "  Length: {}", self.length)?;
        writeln!(f, "  TEID: 0x{:08X}", self.teid)?;

        if let Some(seq) = self.sequence_number {
            writeln!(f, "  Sequence: {}", seq)?;
        }
        if let Some(npdu) = self.n_pdu_number {
            writeln!(f, "  N-PDU: {}", npdu)?;
        }
        if let Some(ext) = self.next_extension_type {
            writeln!(f, "  Next Extension: {}", ext)?;
        }

        Ok(())
    }
}

// ============================================================================
// Tests (ported from header.zig)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // Test: Basic encode/decode
    // Ported from: test "GtpuHeader basic encode/decode"
    #[test]
    fn test_gtpu_header_basic_encode_decode() {
        let mut header = GtpuHeader::new(MessageType::Gpdu, 0x12345678);
        header.length = 100;

        // Encode
        let mut buffer = Vec::new();
        header.encode(&mut buffer).unwrap();

        // Should be 8 bytes (mandatory only)
        assert_eq!(buffer.len(), GtpuHeader::MANDATORY_SIZE);

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decoded = GtpuHeader::decode(&mut cursor).unwrap();

        // Verify
        assert_eq!(header.flags.version(), decoded.flags.version());
        assert_eq!(header.message_type, decoded.message_type);
        assert_eq!(header.length, decoded.length);
        assert_eq!(header.teid, decoded.teid);
        assert_eq!(decoded.sequence_number, None);
    }

    // Test: With optional fields
    // Ported from: test "GtpuHeader with optional fields"
    #[test]
    fn test_gtpu_header_with_optional_fields() {
        let mut header = GtpuHeader::new(MessageType::Gpdu, 0x12345678);
        header.length = 100;
        header.flags.set_s(true);
        header.flags.set_e(true);
        header.sequence_number = Some(42);
        header.next_extension_type = Some(ExtensionHeaderType::PdcpPduNumber);

        // Encode
        let mut buffer = Vec::new();
        header.encode(&mut buffer).unwrap();

        // Verify size
        assert_eq!(
            buffer.len(),
            GtpuHeader::MANDATORY_SIZE + GtpuHeader::OPTIONAL_SIZE
        );

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decoded = GtpuHeader::decode(&mut cursor).unwrap();

        // Verify optional fields
        assert_eq!(header.sequence_number, decoded.sequence_number);
        assert_eq!(header.next_extension_type, decoded.next_extension_type);
        assert_eq!(decoded.sequence_number, Some(42));
    }

    // Additional test: Wire format values
    #[test]
    fn test_header_wire_format() {
        let header = GtpuHeader::new(MessageType::Gpdu, 0x12345678);

        let mut buffer = Vec::new();
        header.encode(&mut buffer).unwrap();

        // Check exact bytes
        assert_eq!(buffer[0], 0x30); // Version=1, PT=1, no optional fields
        assert_eq!(buffer[1], 0xFF); // G-PDU = 255
        assert_eq!(buffer[2], 0x00); // Length MSB
        assert_eq!(buffer[3], 0x00); // Length LSB
        assert_eq!(buffer[4], 0x12); // TEID byte 0
        assert_eq!(buffer[5], 0x34); // TEID byte 1
        assert_eq!(buffer[6], 0x56); // TEID byte 2
        assert_eq!(buffer[7], 0x78); // TEID byte 3
    }

    // Test: Flags manipulation
    #[test]
    fn test_header_flags() {
        let mut flags = HeaderFlags::new();

        // Default state
        assert_eq!(flags.version(), 1);
        assert_eq!(flags.pt(), 1);
        assert!(!flags.s());
        assert!(!flags.e());
        assert!(!flags.pn());
        assert!(!flags.has_optional_fields());

        // Set flags
        flags.set_s(true);
        assert!(flags.s());
        assert!(flags.has_optional_fields());

        flags.set_e(true);
        assert!(flags.e());

        flags.set_pn(true);
        assert!(flags.pn());

        // Clear flag
        flags.set_s(false);
        assert!(!flags.s());
        assert!(flags.has_optional_fields()); // Still has e and pn
    }

    // Test: Size calculation
    #[test]
    fn test_header_size() {
        let mut header = GtpuHeader::new(MessageType::Gpdu, 0x12345678);
        assert_eq!(header.size(), 8);

        header.flags.set_s(true);
        header.sequence_number = Some(42);
        assert_eq!(header.size(), 12);
    }

    // Test: Invalid version
    #[test]
    fn test_invalid_version() {
        let buffer = vec![0x10, 0xFF, 0x00, 0x00, 0x12, 0x34, 0x56, 0x78];
        // First byte: version=0 (invalid), PT=1

        let mut cursor = Cursor::new(&buffer);
        let result = GtpuHeader::decode(&mut cursor);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    // Test: Display formatting
    #[test]
    fn test_header_display() {
        let header = GtpuHeader::new(MessageType::EchoRequest, 0x0);
        let formatted = format!("{}", header);

        assert!(formatted.contains("GTP-U Header"));
        assert!(formatted.contains("Version: 1"));
        assert!(formatted.contains("Echo Request"));
        assert!(formatted.contains("TEID: 0x00000000"));
    }
}
