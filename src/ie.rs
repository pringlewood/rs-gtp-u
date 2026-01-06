//! GTP-U Information Elements
//!
//! 3GPP TS 29.281 Section 8
//!
//! Information Elements (IEs) are used in signaling messages like Echo Response,
//! Error Indication, and Supported Extension Headers Notification.
//!
//! # Translation Note
//!
//! Translated from `zig-gtp-u/src/ie.zig`
//!
//! Key differences:
//! - Zig's tagged union → Rust enum with associated data
//! - Byte slices managed with Vec<u8> (owned) instead of allocator-based slices
//! - Drop trait instead of explicit deinit()

use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// IE Type identifiers
///
/// Identifies the type of Information Element.
///
/// # Translation Note
///
/// Zig version uses non-exhaustive enum with `_` catchall.
/// Rust version uses explicit from_u8() for unknown types.
///
/// Source: `zig-gtp-u/src/ie.zig` lines 7-15
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IEType {
    /// Recovery IE (restart counter)
    Recovery = 14,
    /// Tunnel Endpoint Identifier
    TunnelEndpointIdentifier = 16,
    /// GTP User Plane PDCP PDU Number
    GtpUserPlanePdcpPduNumber = 18,
    /// GSN Address (IP address)
    GsnAddress = 133,
    /// Extension Header Type List
    ExtensionHeaderTypeList = 141,
    /// Private Extension
    PrivateExtension = 255,
}

impl IEType {
    /// Converts a u8 to an IEType
    ///
    /// Returns None for unknown IE types.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            14 => Some(Self::Recovery),
            16 => Some(Self::TunnelEndpointIdentifier),
            18 => Some(Self::GtpUserPlanePdcpPduNumber),
            133 => Some(Self::GsnAddress),
            141 => Some(Self::ExtensionHeaderTypeList),
            255 => Some(Self::PrivateExtension),
            _ => None,
        }
    }
}

/// Cause values for Error Indication and other messages
///
/// Indicates the reason for an error or the result of an operation.
///
/// # Translation Note
///
/// Source: `zig-gtp-u/src/ie.zig` lines 18-31
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Cause {
    /// Request accepted
    RequestAccepted = 128,
    /// New PDN prefix
    NewPdnPrefix = 129,
    /// New APN restriction
    NewApnRestriction = 130,

    // Error causes
    /// Context not found
    ContextNotFound = 192,
    /// Invalid length
    InvalidLength = 193,
    /// Service not supported
    ServiceNotSupported = 194,
    /// Mandatory IE missing
    MandatoryIeMissing = 195,
    /// System failure
    SystemFailure = 204,
}

impl Cause {
    /// Converts a u8 to a Cause
    ///
    /// Returns None for unknown cause values.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            128 => Some(Self::RequestAccepted),
            129 => Some(Self::NewPdnPrefix),
            130 => Some(Self::NewApnRestriction),
            192 => Some(Self::ContextNotFound),
            193 => Some(Self::InvalidLength),
            194 => Some(Self::ServiceNotSupported),
            195 => Some(Self::MandatoryIeMissing),
            204 => Some(Self::SystemFailure),
            _ => None,
        }
    }
}

/// QoS Profile structure (simplified)
///
/// Defines quality of service characteristics for a bearer.
///
/// # Translation Note
///
/// Source: `zig-gtp-u/src/ie.zig` lines 34-41
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QosProfile {
    /// QoS Class Identifier (5QI in 5G)
    pub qci: u8,
    /// Allocation and Retention Priority
    pub arp: u8,
    /// Guaranteed Bit Rate Uplink (bps)
    pub gbr_uplink: u64,
    /// Guaranteed Bit Rate Downlink (bps)
    pub gbr_downlink: u64,
    /// Maximum Bit Rate Uplink (bps)
    pub mbr_uplink: u64,
    /// Maximum Bit Rate Downlink (bps)
    pub mbr_downlink: u64,
}

/// User Location Information (simplified for 5G)
///
/// Identifies the user's location in the network.
///
/// # Translation Note
///
/// Source: `zig-gtp-u/src/ie.zig` lines 44-49
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserLocationInfo {
    /// Mobile Country Code
    pub mcc: u16,
    /// Mobile Network Code
    pub mnc: u16,
    /// Tracking Area Code
    pub tac: u32,
    /// Cell Identity
    pub cell_id: u64,
}

/// Private Extension structure
///
/// Allows vendor-specific extensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateExtension {
    /// Enterprise ID
    pub enterprise_id: u16,
    /// Extension value
    pub value: Vec<u8>,
}

/// Information Element
///
/// Tagged enum containing all supported IE types.
///
/// # Wire Format
///
/// ```text
/// ┌──────┬────────┬─────────┐
/// │ Type │ Length │  Value  │
/// │ (1B) │  (2B)  │(variable)│
/// └──────┴────────┴─────────┘
/// ```
///
/// # Memory Management
///
/// IEs that contain Vec<u8> (gsn_address, extension_header_type_list, private_extension)
/// are automatically freed when the IE is dropped.
///
/// # Translation Note
///
/// Zig version uses tagged union with manual deinit().
/// Rust version uses enum with Drop trait for automatic cleanup.
///
/// Source: `zig-gtp-u/src/ie.zig` lines 52-196
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InformationElement {
    /// Recovery IE - restart counter
    Recovery(u8),

    /// Tunnel Endpoint Identifier
    TunnelEndpointIdentifier(u32),

    /// GTP User Plane PDCP PDU Number
    GtpUserPlanePdcpPduNumber(u16),

    /// GSN Address (IP address as bytes)
    GsnAddress(Vec<u8>),

    /// Extension Header Type List
    ExtensionHeaderTypeList(Vec<u8>),

    /// Private Extension
    PrivateExtension(PrivateExtension),
}

impl InformationElement {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Creates a Recovery IE
    ///
    /// # Arguments
    ///
    /// * `restart_counter` - Restart counter value
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn initRecovery(restart_counter: u8)`
    pub fn recovery(restart_counter: u8) -> Self {
        Self::Recovery(restart_counter)
    }

    /// Creates a Tunnel Endpoint Identifier IE
    ///
    /// # Arguments
    ///
    /// * `teid` - TEID value
    pub fn teid(teid: u32) -> Self {
        Self::TunnelEndpointIdentifier(teid)
    }

    /// Creates a GSN Address IE from an IP address
    ///
    /// # Arguments
    ///
    /// * `address` - IP address (v4 or v6)
    ///
    /// # Translation Note
    ///
    /// Zig version uses std.net.Address and allocator.
    /// Rust version uses IpAddr and Vec<u8> for ownership.
    pub fn gsn_address(address: IpAddr) -> Self {
        let bytes = match address {
            IpAddr::V4(addr) => addr.octets().to_vec(),
            IpAddr::V6(addr) => addr.octets().to_vec(),
        };
        Self::GsnAddress(bytes)
    }

    /// Creates an Extension Header Type List IE
    ///
    /// # Arguments
    ///
    /// * `types` - List of extension header type values
    pub fn extension_header_type_list(types: Vec<u8>) -> Self {
        Self::ExtensionHeaderTypeList(types)
    }

    /// Creates a Private Extension IE
    ///
    /// # Arguments
    ///
    /// * `enterprise_id` - Enterprise identifier
    /// * `value` - Extension value
    pub fn private_extension(enterprise_id: u16, value: Vec<u8>) -> Self {
        Self::PrivateExtension(PrivateExtension {
            enterprise_id,
            value,
        })
    }

    // ========================================================================
    // Size Calculation
    // ========================================================================

    /// Returns the total size of the IE in wire format
    ///
    /// Format: Type (1 byte) + Length (2 bytes) + Value
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn size(self: InformationElement) usize`
    pub fn size(&self) -> usize {
        let value_size = match self {
            Self::Recovery(_) => 1,
            Self::TunnelEndpointIdentifier(_) => 4,
            Self::GtpUserPlanePdcpPduNumber(_) => 2,
            Self::GsnAddress(addr) => addr.len(),
            Self::ExtensionHeaderTypeList(list) => list.len(),
            Self::PrivateExtension(ext) => 2 + ext.value.len(), // enterprise_id + value
        };

        1 + 2 + value_size // type + length + value
    }

    // ========================================================================
    // Encoding
    // ========================================================================

    /// Encodes the IE to wire format
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn encode(self: InformationElement, writer: anytype) !void`
    pub fn encode<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        // Write IE type
        let ie_type = match self {
            Self::Recovery(_) => IEType::Recovery,
            Self::TunnelEndpointIdentifier(_) => IEType::TunnelEndpointIdentifier,
            Self::GtpUserPlanePdcpPduNumber(_) => IEType::GtpUserPlanePdcpPduNumber,
            Self::GsnAddress(_) => IEType::GsnAddress,
            Self::ExtensionHeaderTypeList(_) => IEType::ExtensionHeaderTypeList,
            Self::PrivateExtension(_) => IEType::PrivateExtension,
        };
        writer.write_all(&[ie_type as u8])?;

        // Calculate and write length (excludes type and length fields)
        let value_size = self.size() - 3;
        writer.write_all(&(value_size as u16).to_be_bytes())?;

        // Write value
        match self {
            Self::Recovery(val) => writer.write_all(&[*val])?,
            Self::TunnelEndpointIdentifier(val) => writer.write_all(&val.to_be_bytes())?,
            Self::GtpUserPlanePdcpPduNumber(val) => writer.write_all(&val.to_be_bytes())?,
            Self::GsnAddress(addr) => writer.write_all(addr)?,
            Self::ExtensionHeaderTypeList(list) => writer.write_all(list)?,
            Self::PrivateExtension(ext) => {
                writer.write_all(&ext.enterprise_id.to_be_bytes())?;
                writer.write_all(&ext.value)?;
            }
        }

        Ok(())
    }

    // ========================================================================
    // Decoding
    // ========================================================================

    /// Decodes an IE from wire format
    ///
    /// # Translation Note
    ///
    /// Zig version: `pub fn decode(allocator: std.mem.Allocator, reader: anytype) !InformationElement`
    /// Rust version: No allocator needed, Vec handles allocation
    pub fn decode<R: Read>(reader: &mut R) -> io::Result<Self> {
        // Read IE type
        let mut type_byte = [0u8; 1];
        reader.read_exact(&mut type_byte)?;

        let ie_type = IEType::from_u8(type_byte[0]).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown IE type: {}", type_byte[0]),
            )
        })?;

        // Read length
        let mut length_bytes = [0u8; 2];
        reader.read_exact(&mut length_bytes)?;
        let length = u16::from_be_bytes(length_bytes) as usize;

        // Decode based on type
        match ie_type {
            IEType::Recovery => {
                let mut val = [0u8; 1];
                reader.read_exact(&mut val)?;
                Ok(Self::Recovery(val[0]))
            }
            IEType::TunnelEndpointIdentifier => {
                let mut bytes = [0u8; 4];
                reader.read_exact(&mut bytes)?;
                Ok(Self::TunnelEndpointIdentifier(u32::from_be_bytes(bytes)))
            }
            IEType::GtpUserPlanePdcpPduNumber => {
                let mut bytes = [0u8; 2];
                reader.read_exact(&mut bytes)?;
                Ok(Self::GtpUserPlanePdcpPduNumber(u16::from_be_bytes(bytes)))
            }
            IEType::GsnAddress => {
                let mut addr = vec![0u8; length];
                reader.read_exact(&mut addr)?;
                Ok(Self::GsnAddress(addr))
            }
            IEType::ExtensionHeaderTypeList => {
                let mut list = vec![0u8; length];
                reader.read_exact(&mut list)?;
                Ok(Self::ExtensionHeaderTypeList(list))
            }
            IEType::PrivateExtension => {
                let mut enterprise_bytes = [0u8; 2];
                reader.read_exact(&mut enterprise_bytes)?;
                let enterprise_id = u16::from_be_bytes(enterprise_bytes);

                let value_len = length - 2;
                let mut value = vec![0u8; value_len];
                reader.read_exact(&mut value)?;

                Ok(Self::PrivateExtension(PrivateExtension {
                    enterprise_id,
                    value,
                }))
            }
        }
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Converts GSN Address IE to an IpAddr
    ///
    /// Returns None if not a GSN Address IE or if the address format is invalid.
    pub fn as_ip_addr(&self) -> Option<IpAddr> {
        if let Self::GsnAddress(bytes) = self {
            match bytes.len() {
                4 => {
                    let octets: [u8; 4] = bytes[..].try_into().ok()?;
                    Some(IpAddr::V4(Ipv4Addr::from(octets)))
                }
                16 => {
                    let octets: [u8; 16] = bytes[..].try_into().ok()?;
                    Some(IpAddr::V6(Ipv6Addr::from(octets)))
                }
                _ => None,
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_ie_recovery_encode_decode() {
        let ie = InformationElement::recovery(42);

        // Encode
        let mut buffer = Vec::new();
        ie.encode(&mut buffer).unwrap();

        // Check size
        assert_eq!(buffer.len(), ie.size());

        // Decode
        let mut cursor = Cursor::new(&buffer);
        let decoded = InformationElement::decode(&mut cursor).unwrap();

        assert_eq!(ie, decoded);
        if let InformationElement::Recovery(val) = decoded {
            assert_eq!(val, 42);
        } else {
            panic!("Expected Recovery IE");
        }
    }

    #[test]
    fn test_ie_teid_encode_decode() {
        let ie = InformationElement::teid(0x12345678);

        let mut buffer = Vec::new();
        ie.encode(&mut buffer).unwrap();

        let mut cursor = Cursor::new(&buffer);
        let decoded = InformationElement::decode(&mut cursor).unwrap();

        assert_eq!(ie, decoded);
    }

    #[test]
    fn test_ie_gsn_address_ipv4() {
        let addr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let ie = InformationElement::gsn_address(addr);

        let mut buffer = Vec::new();
        ie.encode(&mut buffer).unwrap();

        let mut cursor = Cursor::new(&buffer);
        let decoded = InformationElement::decode(&mut cursor).unwrap();

        assert_eq!(ie, decoded);

        // Test conversion back to IpAddr
        let decoded_addr = decoded.as_ip_addr().unwrap();
        assert_eq!(decoded_addr, addr);
    }

    #[test]
    fn test_ie_gsn_address_ipv6() {
        let addr = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        let ie = InformationElement::gsn_address(addr);

        let mut buffer = Vec::new();
        ie.encode(&mut buffer).unwrap();

        let mut cursor = Cursor::new(&buffer);
        let decoded = InformationElement::decode(&mut cursor).unwrap();

        assert_eq!(ie, decoded);

        let decoded_addr = decoded.as_ip_addr().unwrap();
        assert_eq!(decoded_addr, addr);
    }

    #[test]
    fn test_ie_extension_header_type_list() {
        let types = vec![0x85, 0xC0, 0x00]; // PDU Session Container, PDCP PDU Number, No More
        let ie = InformationElement::extension_header_type_list(types.clone());

        let mut buffer = Vec::new();
        ie.encode(&mut buffer).unwrap();

        let mut cursor = Cursor::new(&buffer);
        let decoded = InformationElement::decode(&mut cursor).unwrap();

        assert_eq!(ie, decoded);
    }

    #[test]
    fn test_ie_private_extension() {
        let ie = InformationElement::private_extension(1234, vec![0xAB, 0xCD, 0xEF]);

        let mut buffer = Vec::new();
        ie.encode(&mut buffer).unwrap();

        let mut cursor = Cursor::new(&buffer);
        let decoded = InformationElement::decode(&mut cursor).unwrap();

        assert_eq!(ie, decoded);

        if let InformationElement::PrivateExtension(ext) = decoded {
            assert_eq!(ext.enterprise_id, 1234);
            assert_eq!(ext.value, vec![0xAB, 0xCD, 0xEF]);
        } else {
            panic!("Expected PrivateExtension IE");
        }
    }

    #[test]
    fn test_ie_size_calculation() {
        let ie_recovery = InformationElement::recovery(42);
        assert_eq!(ie_recovery.size(), 1 + 2 + 1); // type + length + value

        let ie_teid = InformationElement::teid(0x12345678);
        assert_eq!(ie_teid.size(), 1 + 2 + 4); // type + length + value

        let ie_addr = InformationElement::gsn_address(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)));
        assert_eq!(ie_addr.size(), 1 + 2 + 4); // type + length + 4 bytes IPv4
    }

    #[test]
    fn test_ietype_from_u8() {
        assert_eq!(IEType::from_u8(14), Some(IEType::Recovery));
        assert_eq!(IEType::from_u8(16), Some(IEType::TunnelEndpointIdentifier));
        assert_eq!(IEType::from_u8(255), Some(IEType::PrivateExtension));
        assert_eq!(IEType::from_u8(99), None); // Unknown type
    }

    #[test]
    fn test_cause_from_u8() {
        assert_eq!(Cause::from_u8(128), Some(Cause::RequestAccepted));
        assert_eq!(Cause::from_u8(192), Some(Cause::ContextNotFound));
        assert_eq!(Cause::from_u8(204), Some(Cause::SystemFailure));
        assert_eq!(Cause::from_u8(99), None); // Unknown cause
    }
}
