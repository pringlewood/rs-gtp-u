//! GTP-U Extension Headers
//!
//! 3GPP TS 29.281 Section 5.2
//!
//! Extension headers provide additional information for GTP-U packets, particularly
//! critical for 5G QoS support through PDU Session Container (QFI extraction).
//!
//! # Translation Note
//!
//! Translated from `zig-gtp-u/src/extension.zig`
//!
//! Key Zig → Rust patterns:
//! - Zig's struct bit packing → Manual bit manipulation
//! - Zig's tagged union → Rust enum with data
//! - `!void` error unions → `Result<(), io::Error>`
//! - `anytype` → Generic trait bounds `<W: Write>`, `<R: Read>`

use crate::protocol::ExtensionHeaderType;
use std::io::{self, Read, Write};

/// PDU Session Container for 5G QoS
///
/// Contains QoS Flow Identifier (QFI) critical for 5G packet routing and QoS handling.
///
/// # Wire Format (2 bytes)
///
/// ```text
/// Byte 1:
/// ┌────────┬────────┐
/// │PDU Type│  QFI   │
/// │ (4b)   │(hi 4b) │
/// └────────┴────────┘
///
/// Byte 2:
/// ┌────────┬────────┬────────┬────────┐
/// │QFI(lo) │  PPI   │  RQI   │ Spare  │
/// │ (2b)   │ (3b)   │ (1b)   │ (2b)   │
/// └────────┴────────┴────────┴────────┘
/// ```
///
/// # Translation Note
///
/// Zig uses bit-packed struct fields (u4, u6, u3, bool), Rust uses manual bit manipulation.
/// This provides the same wire format with explicit control over bit layout.
///
/// Source: `zig-gtp-u/src/extension.zig` lines 10-36
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PduSessionContainer {
    /// PDU Type: 0=Downlink, 1=Uplink
    pub pdu_type: u8, // 4 bits (0-15)

    /// QoS Flow Identifier (0-63)
    pub qfi: u8, // 6 bits

    /// Paging Policy Indicator (0-7)
    pub ppi: u8, // 3 bits

    /// Reflective QoS Indicator
    pub rqi: bool,
}

impl PduSessionContainer {
    /// Creates a new PDU Session Container
    ///
    /// # Panics
    ///
    /// Panics if `pdu_type > 15`, `qfi > 63`, or `ppi > 7`.
    pub fn new(pdu_type: u8, qfi: u8, ppi: u8, rqi: bool) -> Self {
        assert!(pdu_type <= 15, "pdu_type must be 4 bits (0-15)");
        assert!(qfi <= 63, "qfi must be 6 bits (0-63)");
        assert!(ppi <= 7, "ppi must be 3 bits (0-7)");

        Self {
            pdu_type,
            qfi,
            ppi,
            rqi,
        }
    }

    /// Encodes the PDU Session Container to wire format (2 bytes)
    ///
    /// # Translation Note
    ///
    /// Zig version uses bit packing:
    /// ```zig
    /// const byte1 = (@as(u8, self.pdu_type) << 4) | (@as(u8, self.qfi >> 2));
    /// const byte2 = (@as(u8, @as(u2, @truncate(self.qfi))) << 6) | ...
    /// ```
    ///
    /// Rust version uses explicit bit manipulation for clarity.
    pub fn encode<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        // Byte 1: PDU Type (upper 4 bits) | QFI high 4 bits (lower 4 bits)
        let byte1 = (self.pdu_type << 4) | (self.qfi >> 2);

        // Byte 2: QFI low 2 bits (upper 2 bits) | PPI (3 bits) | RQI (1 bit) | Spare (2 bits)
        let qfi_low = (self.qfi & 0x03) << 6; // Extract lower 2 bits, shift to position
        let ppi_bits = (self.ppi & 0x07) << 3; // 3 bits, shift to position
        let rqi_bit = if self.rqi { 0x04 } else { 0x00 }; // Bit 2
        let byte2 = qfi_low | ppi_bits | rqi_bit; // Spare bits are 0

        writer.write_all(&[byte1, byte2])
    }

    /// Decodes a PDU Session Container from wire format (2 bytes)
    ///
    /// # Translation Note
    ///
    /// Zig version:
    /// ```zig
    /// .qfi = (@as(u6, @truncate(byte1)) << 2) | @as(u6, @truncate(byte2 >> 6))
    /// ```
    ///
    /// Rust equivalent with explicit masking.
    pub fn decode<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 2];
        reader.read_exact(&mut bytes)?;

        let byte1 = bytes[0];
        let byte2 = bytes[1];

        // Extract PDU Type from upper 4 bits of byte1
        let pdu_type = byte1 >> 4;

        // Extract QFI: upper 4 bits from byte1, lower 2 bits from byte2
        let qfi_high = (byte1 & 0x0F) << 2; // Lower 4 bits of byte1
        let qfi_low = byte2 >> 6; // Upper 2 bits of byte2
        let qfi = qfi_high | qfi_low;

        // Extract PPI: bits 5-3 of byte2
        let ppi = (byte2 >> 3) & 0x07;

        // Extract RQI: bit 2 of byte2
        let rqi = (byte2 & 0x04) != 0;

        Ok(Self {
            pdu_type,
            qfi,
            ppi,
            rqi,
        })
    }
}

/// PDCP PDU Number (16-bit)
///
/// Used for PDCP sequence number in 4G/5G radio protocols.
///
/// # Translation Note
///
/// Simple struct with big-endian u16 encoding.
/// Source: `zig-gtp-u/src/extension.zig` lines 39-52
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdcpPduNumber {
    /// PDCP sequence number (0-65535)
    pub pdu_number: u16,
}

impl PdcpPduNumber {
    /// Creates a new PDCP PDU Number
    pub fn new(pdu_number: u16) -> Self {
        Self { pdu_number }
    }

    /// Encodes to wire format (2 bytes, big-endian)
    pub fn encode<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.pdu_number.to_be_bytes())
    }

    /// Decodes from wire format (2 bytes, big-endian)
    pub fn decode<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 2];
        reader.read_exact(&mut bytes)?;
        Ok(Self {
            pdu_number: u16::from_be_bytes(bytes),
        })
    }
}

/// Long PDCP PDU Number (24-bit)
///
/// Used for PDCP sequence number in Unacknowledged Mode (UM) Data Radio Bearers.
///
/// # Translation Note
///
/// Rust doesn't have u24, so we use u32 with validation.
/// Wire format: 3 bytes big-endian.
/// Source: `zig-gtp-u/src/extension.zig` lines 54-73
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LongPdcpPduNumber {
    /// PDCP sequence number (0-16777215, 24-bit max)
    pub pdu_number: u32,
}

impl LongPdcpPduNumber {
    /// Maximum value for 24-bit number
    pub const MAX_VALUE: u32 = 0x00FF_FFFF;

    /// Creates a new Long PDCP PDU Number
    ///
    /// # Panics
    ///
    /// Panics if `pdu_number > 0x00FFFFFF` (24-bit max).
    pub fn new(pdu_number: u32) -> Self {
        assert!(
            pdu_number <= Self::MAX_VALUE,
            "pdu_number must be 24 bits (0-16777215)"
        );
        Self { pdu_number }
    }

    /// Encodes to wire format (3 bytes, big-endian)
    ///
    /// # Translation Note
    ///
    /// Zig version writes byte-by-byte with truncation:
    /// ```zig
    /// try writer.writeByte(@truncate(self.pdu_number >> 16));
    /// try writer.writeByte(@truncate(self.pdu_number >> 8));
    /// try writer.writeByte(@truncate(self.pdu_number));
    /// ```
    pub fn encode<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let bytes = [
            ((self.pdu_number >> 16) & 0xFF) as u8,
            ((self.pdu_number >> 8) & 0xFF) as u8,
            (self.pdu_number & 0xFF) as u8,
        ];
        writer.write_all(&bytes)
    }

    /// Decodes from wire format (3 bytes, big-endian)
    ///
    /// # Translation Note
    ///
    /// Zig version:
    /// ```zig
    /// .pdu_number = (@as(u24, b1) << 16) | (@as(u24, b2) << 8) | b3
    /// ```
    pub fn decode<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 3];
        reader.read_exact(&mut bytes)?;

        let pdu_number = ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32);

        Ok(Self { pdu_number })
    }
}

/// Service Class Indicator
///
/// Indicates service class for QoS handling.
///
/// # Translation Note
///
/// Simple single-byte value.
/// Source: `zig-gtp-u/src/extension.zig` lines 75-88
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceClassIndicator {
    /// Service class value
    pub service_class: u8,
}

impl ServiceClassIndicator {
    /// Creates a new Service Class Indicator
    pub fn new(service_class: u8) -> Self {
        Self { service_class }
    }

    /// Encodes to wire format (1 byte)
    pub fn encode<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&[self.service_class])
    }

    /// Decodes from wire format (1 byte)
    pub fn decode<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte)?;
        Ok(Self {
            service_class: byte[0],
        })
    }
}

/// UDP Port Extension
///
/// Contains UDP port number.
///
/// # Translation Note
///
/// Simple u16 big-endian encoding.
/// Source: `zig-gtp-u/src/extension.zig` lines 90-103
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdpPortExtension {
    /// UDP port number
    pub port: u16,
}

impl UdpPortExtension {
    /// Creates a new UDP Port Extension
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Encodes to wire format (2 bytes, big-endian)
    pub fn encode<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.port.to_be_bytes())
    }

    /// Decodes from wire format (2 bytes, big-endian)
    pub fn decode<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 2];
        reader.read_exact(&mut bytes)?;
        Ok(Self {
            port: u16::from_be_bytes(bytes),
        })
    }
}

/// Extension Header types
///
/// Tagged enum containing all extension header variants.
///
/// # Translation Note
///
/// Zig uses tagged union:
/// ```zig
/// pub const ExtensionHeader = union(ExtensionHeaderType) {
///     pdu_session_container: PduSessionContainer,
///     ...
/// };
/// ```
///
/// Rust uses enum with data fields.
/// Source: `zig-gtp-u/src/extension.zig` lines 135-284
#[derive(Debug, Clone, PartialEq)]
pub enum ExtensionHeader {
    /// No more extension headers
    NoMoreHeaders,

    /// PDU Session Container (5G QoS - contains QFI)
    PduSessionContainer(PduSessionContainer),

    /// PDCP PDU Number (16-bit)
    PdcpPduNumber(PdcpPduNumber),

    /// Long PDCP PDU Number (24-bit)
    LongPdcpPduNumber(LongPdcpPduNumber),

    /// Service Class Indicator
    ServiceClassIndicator(ServiceClassIndicator),

    /// UDP Port
    UdpPort(UdpPortExtension),
    // Note: RanContainer variants skipped for now (require heap allocation)
}

/// Result type for decoding extension headers
#[derive(Debug, Clone, PartialEq)]
pub struct DecodeResult {
    /// Decoded extension header
    pub header: ExtensionHeader,
    /// Next extension header type in chain
    pub next_type: ExtensionHeaderType,
}

impl ExtensionHeader {
    /// Returns the content size in bytes (not including length and next_type fields)
    ///
    /// # Translation Note
    ///
    /// Zig version calculates content size for 4-byte alignment.
    /// Source: `zig-gtp-u/src/extension.zig` lines 159-173
    pub fn content_size(&self) -> usize {
        match self {
            Self::PduSessionContainer(_) => 2,
            Self::PdcpPduNumber(_) => 2,
            Self::LongPdcpPduNumber(_) => 3,
            Self::ServiceClassIndicator(_) => 1,
            Self::UdpPort(_) => 2,
            Self::NoMoreHeaders => 0,
        }
    }

    /// Returns total size of extension header in wire format (always multiple of 4)
    ///
    /// Extension header format: length (1 byte) + content + padding + next_type (1 byte)
    /// Total size = length_value * 4
    ///
    /// # Translation Note
    ///
    /// Zig calculation:
    /// ```zig
    /// const units = (content_size + 2 + 3) / 4; // +2 for length and next_type, +3 for ceiling
    /// return units * 4;
    /// ```
    pub fn size(&self) -> usize {
        let content_size = self.content_size();
        // Need to fit: length(1) + content + next_type(1) in units*4 bytes
        // units = ceiling((content_size + 2) / 4)
        let units = (content_size + 2 + 3) / 4; // +3 for ceiling division
        units * 4
    }

    /// Encodes extension header to wire format with 4-byte alignment
    ///
    /// # Arguments
    ///
    /// * `writer` - Output writer
    /// * `next_ext_type` - Next extension header type (or None for last header)
    ///
    /// # Translation Note
    ///
    /// Zig version uses same algorithm: write length, content, padding, next_type.
    /// Source: `zig-gtp-u/src/extension.zig` lines 182-230
    pub fn encode<W: Write>(
        &self,
        writer: &mut W,
        next_ext_type: Option<ExtensionHeaderType>,
    ) -> io::Result<()> {
        let content_size = self.content_size();

        // Calculate length in 4-byte units
        let units = ((content_size + 2 + 3) / 4) as u8;
        writer.write_all(&[units])?;

        // Write content
        match self {
            Self::PduSessionContainer(psc) => psc.encode(writer)?,
            Self::PdcpPduNumber(ppn) => ppn.encode(writer)?,
            Self::LongPdcpPduNumber(lppn) => lppn.encode(writer)?,
            Self::ServiceClassIndicator(sci) => sci.encode(writer)?,
            Self::UdpPort(up) => up.encode(writer)?,
            Self::NoMoreHeaders => {} // No content
        }

        // Write padding to align to 4-byte boundary
        let total_size = units as usize * 4;
        let already_written = 1 + content_size; // length + content
        let remaining = total_size - already_written; // includes next_type + padding
        let padding_size = remaining - 1; // -1 for next_type

        for _ in 0..padding_size {
            writer.write_all(&[0])?;
        }

        // Write next extension type
        let next = next_ext_type.unwrap_or(ExtensionHeaderType::NoMoreHeaders);
        writer.write_all(&[next as u8])
    }

    /// Decodes extension header from wire format
    ///
    /// # Arguments
    ///
    /// * `reader` - Input reader
    /// * `ext_type` - Expected extension header type
    ///
    /// # Returns
    ///
    /// DecodeResult containing the decoded header and next type in chain.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Unsupported extension header type
    /// - Read fails
    /// - Invalid data
    ///
    /// # Translation Note
    ///
    /// Zig version uses same algorithm, reading length then content.
    /// Source: `zig-gtp-u/src/extension.zig` lines 243-284
    pub fn decode<R: Read>(
        reader: &mut R,
        ext_type: ExtensionHeaderType,
    ) -> io::Result<DecodeResult> {
        // Read length in 4-byte units
        let mut length_byte = [0u8; 1];
        reader.read_exact(&mut length_byte)?;

        // Decode based on type
        let header = match ext_type {
            ExtensionHeaderType::PduSessionContainer => {
                ExtensionHeader::PduSessionContainer(PduSessionContainer::decode(reader)?)
            }
            ExtensionHeaderType::PdcpPduNumber => {
                ExtensionHeader::PdcpPduNumber(PdcpPduNumber::decode(reader)?)
            }
            ExtensionHeaderType::LongPdcpPduNumber => {
                ExtensionHeader::LongPdcpPduNumber(LongPdcpPduNumber::decode(reader)?)
            }
            ExtensionHeaderType::ServiceClassIndicator => {
                ExtensionHeader::ServiceClassIndicator(ServiceClassIndicator::decode(reader)?)
            }
            ExtensionHeaderType::UdpPort => {
                ExtensionHeader::UdpPort(UdpPortExtension::decode(reader)?)
            }
            ExtensionHeaderType::NoMoreHeaders => ExtensionHeader::NoMoreHeaders,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Unsupported extension header type: {:?}", ext_type),
                ));
            }
        };

        // Validate the on-wire length byte against the expected size for this type
        let expected_units = (header.size() / 4) as u8;
        if length_byte[0] != expected_units {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Extension header {:?} length {} (expected {})",
                    ext_type, length_byte[0], expected_units
                ),
            ));
        }

        // Consume padding between content and the trailing next_type byte
        let padding_size = header.size() - 1 - header.content_size() - 1; // - length - next_type
        let mut padding = [0u8; 4];
        reader.read_exact(&mut padding[..padding_size])?;

        // Read next extension type (last byte of the 4-byte aligned block)
        let mut next_type_byte = [0u8; 1];
        reader.read_exact(&mut next_type_byte)?;

        let next_type = ExtensionHeaderType::from_u8(next_type_byte[0]).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Unknown next extension header type: 0x{:02X}",
                    next_type_byte[0]
                ),
            )
        })?;

        Ok(DecodeResult { header, next_type })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_pdu_session_container_encode_decode() {
        let psc = PduSessionContainer::new(
            1,    // Uplink
            9,    // QFI 9
            0,    // PPI 0
            true, // RQI true
        );

        // Encode
        let mut buffer = Vec::new();
        psc.encode(&mut buffer).unwrap();

        assert_eq!(
            buffer.len(),
            2,
            "PDU Session Container should encode to 2 bytes"
        );

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decoded = PduSessionContainer::decode(&mut cursor).unwrap();

        assert_eq!(psc.pdu_type, decoded.pdu_type);
        assert_eq!(psc.qfi, decoded.qfi);
        assert_eq!(psc.ppi, decoded.ppi);
        assert_eq!(psc.rqi, decoded.rqi);
    }

    #[test]
    fn test_pdu_session_container_bit_packing() {
        // Test QFI=9 (binary: 001001)
        // Should split as: high 4 bits = 0010, low 2 bits = 01
        let psc = PduSessionContainer::new(1, 9, 0, true);

        let mut buffer = Vec::new();
        psc.encode(&mut buffer).unwrap();

        // Byte 1: pdu_type(1) << 4 | qfi_high(0010) = 0001_0010 = 0x12
        assert_eq!(buffer[0], 0x12);

        // Byte 2: qfi_low(01) << 6 | ppi(000) << 3 | rqi(1) << 2 = 0100_0100 = 0x44
        assert_eq!(buffer[1], 0x44);
    }

    #[test]
    fn test_pdcp_pdu_number_encode_decode() {
        let ppn = PdcpPduNumber::new(12345);

        // Encode
        let mut buffer = Vec::new();
        ppn.encode(&mut buffer).unwrap();

        assert_eq!(buffer.len(), 2);

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decoded = PdcpPduNumber::decode(&mut cursor).unwrap();

        assert_eq!(ppn.pdu_number, decoded.pdu_number);
    }

    #[test]
    fn test_long_pdcp_pdu_number_encode_decode() {
        let lppn = LongPdcpPduNumber::new(0x123456); // 24-bit value

        // Encode
        let mut buffer = Vec::new();
        lppn.encode(&mut buffer).unwrap();

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer[0], 0x12);
        assert_eq!(buffer[1], 0x34);
        assert_eq!(buffer[2], 0x56);

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decoded = LongPdcpPduNumber::decode(&mut cursor).unwrap();

        assert_eq!(lppn.pdu_number, decoded.pdu_number);
    }

    #[test]
    #[should_panic(expected = "must be 24 bits")]
    fn test_long_pdcp_pdu_number_overflow() {
        LongPdcpPduNumber::new(0x01000000); // Too large for 24 bits
    }

    #[test]
    fn test_service_class_indicator() {
        let sci = ServiceClassIndicator::new(42);

        let mut buffer = Vec::new();
        sci.encode(&mut buffer).unwrap();

        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer[0], 42);

        let mut cursor = Cursor::new(&buffer);
        let decoded = ServiceClassIndicator::decode(&mut cursor).unwrap();

        assert_eq!(sci.service_class, decoded.service_class);
    }

    #[test]
    fn test_udp_port_extension() {
        let up = UdpPortExtension::new(8080);

        let mut buffer = Vec::new();
        up.encode(&mut buffer).unwrap();

        assert_eq!(buffer.len(), 2);

        let mut cursor = Cursor::new(&buffer);
        let decoded = UdpPortExtension::decode(&mut cursor).unwrap();

        assert_eq!(up.port, decoded.port);
    }

    #[test]
    fn test_extension_header_size() {
        let psc_ext = ExtensionHeader::PduSessionContainer(PduSessionContainer::new(1, 9, 0, true));

        // Content size = 2 bytes
        // Total needed = length(1) + content(2) + next_type(1) = 4 bytes
        // Units = (2 + 2 + 3) / 4 = 1
        // Total size = 1 * 4 = 4 bytes
        assert_eq!(psc_ext.content_size(), 2);
        assert_eq!(psc_ext.size(), 4);
    }

    #[test]
    fn test_extension_header_full_encode_decode() {
        let ppn = PdcpPduNumber::new(999);
        let ext = ExtensionHeader::PdcpPduNumber(ppn);

        // Encode with no next extension
        let mut buffer = Vec::new();
        ext.encode(&mut buffer, None).unwrap();

        // Should be 4 bytes: length(1) + content(2) + next_type(1) = 4
        assert_eq!(buffer.len(), 4);
        assert_eq!(buffer[0], 1); // Length = 1 unit
        assert_eq!(buffer[3], 0x00); // Next type = NoMoreHeaders

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decode_result =
            ExtensionHeader::decode(&mut cursor, ExtensionHeaderType::PdcpPduNumber).unwrap();

        assert_eq!(decode_result.next_type, ExtensionHeaderType::NoMoreHeaders);

        if let ExtensionHeader::PdcpPduNumber(decoded_ppn) = decode_result.header {
            assert_eq!(ppn.pdu_number, decoded_ppn.pdu_number);
        } else {
            panic!("Expected PdcpPduNumber variant");
        }
    }

    #[test]
    fn test_extension_header_with_padding() {
        // Service class indicator has 1 byte content
        // Total needed: length(1) + content(1) + next_type(1) = 3 bytes
        // Units = (1 + 2 + 3) / 4 = 1
        // Total size = 1 * 4 = 4 bytes
        // Padding = 4 - 1 - 1 - 1 = 1 byte

        let sci = ServiceClassIndicator::new(5);
        let ext = ExtensionHeader::ServiceClassIndicator(sci);

        let mut buffer = Vec::new();
        ext.encode(&mut buffer, Some(ExtensionHeaderType::PduSessionContainer))
            .unwrap();

        assert_eq!(buffer.len(), 4);
        assert_eq!(buffer[0], 1); // Length = 1 unit
        assert_eq!(buffer[1], 5); // Content
        assert_eq!(buffer[2], 0); // Padding
        assert_eq!(buffer[3], 0x85); // Next type = PDU Session Container

        // Decode must consume the padding byte and land on next_type
        let mut cursor = Cursor::new(&buffer);
        let result =
            ExtensionHeader::decode(&mut cursor, ExtensionHeaderType::ServiceClassIndicator)
                .unwrap();
        assert_eq!(result.header, ext);
        assert_eq!(result.next_type, ExtensionHeaderType::PduSessionContainer);
    }

    #[test]
    fn test_extension_header_bad_length_byte_is_error() {
        // PDCP PDU Number block claims 2 units but the type is fixed at 1 unit
        let buffer = [2u8, 0x03, 0xE7, 0x00];
        let mut cursor = Cursor::new(&buffer[..]);
        let err = ExtensionHeader::decode(&mut cursor, ExtensionHeaderType::PdcpPduNumber)
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_extension_header_unknown_next_type_is_error() {
        // Valid PDCP PDU Number block, but next_type 0xFF is not a known extension type
        let buffer = [1u8, 0x03, 0xE7, 0xFF];
        let mut cursor = Cursor::new(&buffer[..]);
        let err = ExtensionHeader::decode(&mut cursor, ExtensionHeaderType::PdcpPduNumber)
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
