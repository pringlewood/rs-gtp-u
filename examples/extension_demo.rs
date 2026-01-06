//! GTP-U Extension Headers Demo
//!
//! This example demonstrates the use of extension headers, particularly
//! the PDU Session Container for 5G QoS (QFI extraction).

use rs_gtp_u::{
    ExtensionHeader, ExtensionHeaderType, LongPdcpPduNumber, PdcpPduNumber, PduSessionContainer,
    ServiceClassIndicator, UdpPortExtension,
};
use std::io::Cursor;

fn print_hex_dump(label: &str, bytes: &[u8]) {
    println!("{}", label);
    print!("  Hex: ");
    for byte in bytes {
        print!("{:02X} ", byte);
    }
    println!("\n");
}

fn main() {
    println!("=== GTP-U Extension Headers Demo ===\n");

    // Example 1: PDU Session Container (Critical for 5G QoS)
    println!("Example 1: PDU Session Container (5G QoS)");
    println!("─────────────────────────────────────────");

    let pdu_container = PduSessionContainer::new(
        1,    // Uplink PDU
        9,    // QFI = 9 (QoS Flow Identifier)
        0,    // PPI = 0
        true, // RQI = true (Reflective QoS Indicator)
    );

    println!("PDU Session Container:");
    println!(
        "  PDU Type: {} ({})",
        pdu_container.pdu_type,
        if pdu_container.pdu_type == 0 {
            "Downlink"
        } else {
            "Uplink"
        }
    );
    println!("  QFI: {} (QoS Flow Identifier)", pdu_container.qfi);
    println!("  PPI: {} (Paging Policy Indicator)", pdu_container.ppi);
    println!("  RQI: {} (Reflective QoS)", pdu_container.rqi);
    println!();

    // Encode as standalone
    let mut buffer = Vec::new();
    pdu_container.encode(&mut buffer).unwrap();
    print_hex_dump("Raw PDU Container bytes (2 bytes):", &buffer);

    // Decode and verify
    let mut cursor = Cursor::new(&buffer);
    let decoded = PduSessionContainer::decode(&mut cursor).unwrap();
    println!("✓ Decode successful: QFI = {}\n", decoded.qfi);

    // Example 2: Full Extension Header with 4-byte alignment
    println!("Example 2: Extension Header Format (4-byte aligned)");
    println!("─────────────────────────────────────────");

    let ext_header = ExtensionHeader::PduSessionContainer(pdu_container);

    let mut ext_buffer = Vec::new();
    ext_header.encode(&mut ext_buffer, None).unwrap();

    print_hex_dump("Full Extension Header (4 bytes):", &ext_buffer);

    println!("Wire format breakdown:");
    println!("  Byte 0: Length = {} (in 4-byte units)", ext_buffer[0]);
    println!("  Byte 1-2: PDU Container content");
    println!(
        "  Byte 3: Next extension type = 0x{:02X} ({})",
        ext_buffer[3],
        if ext_buffer[3] == 0 {
            "No more headers"
        } else {
            "Has next"
        }
    );
    println!();

    // Decode extension header
    let mut cursor = Cursor::new(&ext_buffer);
    let decode_result =
        ExtensionHeader::decode(&mut cursor, ExtensionHeaderType::PduSessionContainer).unwrap();

    if let ExtensionHeader::PduSessionContainer(decoded_psc) = decode_result.header {
        println!("✓ Extension decode successful: QFI = {}\n", decoded_psc.qfi);
    }

    // Example 3: PDCP PDU Number
    println!("Example 3: PDCP PDU Number (16-bit)");
    println!("─────────────────────────────────────────");

    let pdcp = PdcpPduNumber::new(12345);
    println!("PDCP PDU Number: {}", pdcp.pdu_number);

    let mut pdcp_buf = Vec::new();
    pdcp.encode(&mut pdcp_buf).unwrap();
    print_hex_dump("PDCP bytes (2 bytes, big-endian):", &pdcp_buf);

    let mut cursor = Cursor::new(&pdcp_buf);
    let decoded_pdcp = PdcpPduNumber::decode(&mut cursor).unwrap();
    println!("✓ Decode successful: {}\n", decoded_pdcp.pdu_number);

    // Example 4: Long PDCP PDU Number (24-bit)
    println!("Example 4: Long PDCP PDU Number (24-bit)");
    println!("─────────────────────────────────────────");

    let long_pdcp = LongPdcpPduNumber::new(0xABCDEF);
    println!("Long PDCP PDU Number: 0x{:06X}", long_pdcp.pdu_number);

    let mut long_pdcp_buf = Vec::new();
    long_pdcp.encode(&mut long_pdcp_buf).unwrap();
    print_hex_dump("Long PDCP bytes (3 bytes, big-endian):", &long_pdcp_buf);

    let mut cursor = Cursor::new(&long_pdcp_buf);
    let decoded_long = LongPdcpPduNumber::decode(&mut cursor).unwrap();
    println!("✓ Decode successful: 0x{:06X}\n", decoded_long.pdu_number);

    // Example 5: Service Class Indicator
    println!("Example 5: Service Class Indicator");
    println!("─────────────────────────────────────────");

    let sci = ServiceClassIndicator::new(42);
    println!("Service Class: {}", sci.service_class);

    let mut sci_buf = Vec::new();
    sci.encode(&mut sci_buf).unwrap();
    print_hex_dump("Service Class bytes (1 byte):", &sci_buf);

    let mut cursor = Cursor::new(&sci_buf);
    let decoded_sci = ServiceClassIndicator::decode(&mut cursor).unwrap();
    println!("✓ Decode successful: {}\n", decoded_sci.service_class);

    // Example 6: UDP Port Extension
    println!("Example 6: UDP Port Extension");
    println!("─────────────────────────────────────────");

    let udp_port = UdpPortExtension::new(8080);
    println!("UDP Port: {}", udp_port.port);

    let mut udp_buf = Vec::new();
    udp_port.encode(&mut udp_buf).unwrap();
    print_hex_dump("UDP Port bytes (2 bytes, big-endian):", &udp_buf);

    let mut cursor = Cursor::new(&udp_buf);
    let decoded_port = UdpPortExtension::decode(&mut cursor).unwrap();
    println!("✓ Decode successful: {}\n", decoded_port.port);

    // Example 7: Extension Header Chain (simulated)
    println!("Example 7: Extension Header Chaining");
    println!("─────────────────────────────────────────");
    println!("Extension headers can be chained in GTP-U packets:");
    println!("  Header 1: PDU Session Container (type 0x85)");
    println!("  Header 2: PDCP PDU Number (type 0xC0)");
    println!("  Header 3: No More Headers (type 0x00)");
    println!();

    // Encode first extension (PDU Session Container) pointing to next
    let ext1 = ExtensionHeader::PduSessionContainer(pdu_container);
    let mut chain_buf = Vec::new();
    ext1.encode(&mut chain_buf, Some(ExtensionHeaderType::PdcpPduNumber))
        .unwrap();

    println!("First extension header:");
    print_hex_dump("  ", &chain_buf);
    println!(
        "  Last byte (0x{:02X}) points to PDCP PDU Number",
        chain_buf.last().unwrap()
    );
    println!();

    // Encode second extension (PDCP) as last in chain
    let ext2 = ExtensionHeader::PdcpPduNumber(pdcp);
    let mut second_buf = Vec::new();
    ext2.encode(&mut second_buf, None).unwrap();

    println!("Second extension header:");
    print_hex_dump("  ", &second_buf);
    println!(
        "  Last byte (0x{:02X}) = No More Headers",
        second_buf.last().unwrap()
    );
    println!();

    // Example 8: QFI Extraction Demo (Critical for 5G)
    println!("Example 8: QFI Extraction for 5G QoS");
    println!("─────────────────────────────────────────");
    println!("The QFI (QoS Flow Identifier) is critical for 5G packet routing:");
    println!();

    for (qfi, description) in &[
        (1, "GBR voice"),
        (5, "IMS signaling"),
        (9, "Default bearer"),
        (63, "Maximum QFI value"),
    ] {
        let psc = PduSessionContainer::new(1, *qfi, 0, false);
        let mut buf = Vec::new();
        psc.encode(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = PduSessionContainer::decode(&mut cursor).unwrap();

        println!("  QFI {}: {} ✓", decoded.qfi, description);
    }

    println!("\n=== All Examples Complete ===");
}
