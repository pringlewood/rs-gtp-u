//! GTP-U Message Demo
//!
//! This example demonstrates complete GTP-U message creation, encoding, and decoding
//! with extension headers and QFI extraction for 5G QoS.

use rs_gtp_u::{ExtensionHeader, GtpuMessage, PduSessionContainer};
use std::io::Cursor;

fn print_hex_dump(label: &str, bytes: &[u8]) {
    println!("{}", label);
    print!("  Hex: ");
    for (i, byte) in bytes.iter().enumerate() {
        print!("{:02X} ", byte);
        if (i + 1) % 16 == 0 {
            println!();
            print!("       ");
        }
    }
    println!("\n");
}

fn main() {
    println!("=== GTP-U Message Demo ===\n");

    // Example 1: Simple G-PDU (most common message type)
    println!("Example 1: Basic G-PDU Message");
    println!("─────────────────────────────────────────");

    let payload = b"Hello, GTP-U World!".to_vec();
    let mut gpdu = GtpuMessage::create_gpdu(0x12345678, payload.clone());

    println!("G-PDU Message:");
    println!("  TEID: 0x{:08X}", gpdu.header.teid);
    println!("  Payload: {:?}", String::from_utf8_lossy(&gpdu.payload));
    println!("  Payload Length: {} bytes", gpdu.payload.len());
    println!();

    // Encode
    let mut buffer = Vec::new();
    gpdu.encode(&mut buffer).unwrap();
    print_hex_dump("Encoded G-PDU:", &buffer);

    // Decode and verify
    let mut cursor = Cursor::new(&buffer);
    let decoded = GtpuMessage::decode(&mut cursor).unwrap();
    println!(
        "✓ Decode successful: TEID = 0x{:08X}, Payload = {:?}\n",
        decoded.header.teid,
        String::from_utf8_lossy(&decoded.payload)
    );

    // Example 2: G-PDU with 5G QoS (PDU Session Container)
    println!("Example 2: G-PDU with 5G QoS Support (QFI Extraction)");
    println!("─────────────────────────────────────────");

    let qos_payload = b"5G QoS-aware packet".to_vec();
    let mut qos_gpdu = GtpuMessage::create_gpdu(0xABCDEF01, qos_payload.clone());

    // Add PDU Session Container for 5G QoS
    let pdu_container = ExtensionHeader::PduSessionContainer(PduSessionContainer::new(
        1,    // Uplink PDU
        9,    // QFI = 9 (QoS Flow Identifier)
        0,    // PPI = 0
        true, // RQI = true (Reflective QoS Indicator)
    ));
    qos_gpdu.add_extension_header(pdu_container);

    println!("G-PDU with PDU Session Container:");
    println!("  TEID: 0x{:08X}", qos_gpdu.header.teid);
    println!(
        "  Payload: {:?}",
        String::from_utf8_lossy(&qos_gpdu.payload)
    );
    println!("  Extension Headers: {}", qos_gpdu.extension_headers.len());
    println!();

    // Extract QFI (critical for 5G QoS routing)
    if let Some(qfi) = qos_gpdu.get_qfi() {
        println!("  ✓ QFI Extracted: {} (QoS Flow Identifier)", qfi);
    }

    // Get full PDU Session Container details
    if let Some(psc) = qos_gpdu.get_pdu_session_container() {
        println!("  PDU Session Container:");
        println!(
            "    - PDU Type: {} ({})",
            psc.pdu_type,
            if psc.pdu_type == 0 {
                "Downlink"
            } else {
                "Uplink"
            }
        );
        println!("    - QFI: {} (QoS Flow ID)", psc.qfi);
        println!("    - PPI: {} (Paging Policy)", psc.ppi);
        println!("    - RQI: {} (Reflective QoS)", psc.rqi);
    }
    println!();

    // Encode
    let mut qos_buffer = Vec::new();
    qos_gpdu.encode(&mut qos_buffer).unwrap();
    print_hex_dump("Encoded G-PDU with Extension Header:", &qos_buffer);

    // Decode and verify QFI extraction
    let mut qos_cursor = Cursor::new(&qos_buffer);
    let decoded_qos = GtpuMessage::decode(&mut qos_cursor).unwrap();
    println!("✓ Decode successful:");
    println!("  TEID: 0x{:08X}", decoded_qos.header.teid);
    println!("  QFI: {:?}", decoded_qos.get_qfi());
    println!(
        "  Payload: {:?}\n",
        String::from_utf8_lossy(&decoded_qos.payload)
    );

    // Example 3: Echo Request (Path Management)
    println!("Example 3: Echo Request (Path Management)");
    println!("─────────────────────────────────────────");

    let mut echo_req = GtpuMessage::create_echo_request(12345);

    println!("Echo Request:");
    println!("  Message Type: {:?}", echo_req.header.message_type);
    println!(
        "  TEID: 0x{:08X} (Always 0 for signaling)",
        echo_req.header.teid
    );
    println!("  Sequence Number: {:?}", echo_req.header.sequence_number);
    println!();

    // Encode
    let mut echo_buffer = Vec::new();
    echo_req.encode(&mut echo_buffer).unwrap();
    print_hex_dump("Encoded Echo Request:", &echo_buffer);

    // Decode and verify
    let mut echo_cursor = Cursor::new(&echo_buffer);
    let decoded_echo = GtpuMessage::decode(&mut echo_cursor).unwrap();
    println!("✓ Decode successful:");
    println!("  Message Type: {:?}", decoded_echo.header.message_type);
    println!("  Sequence: {:?}\n", decoded_echo.header.sequence_number);

    // Example 4: End Marker (Handover Indication)
    println!("Example 4: End Marker (Handover)");
    println!("─────────────────────────────────────────");

    let end_marker = GtpuMessage::create_end_marker(0x99887766, 999);

    println!("End Marker:");
    println!("  Message Type: {:?}", end_marker.header.message_type);
    println!("  TEID: 0x{:08X}", end_marker.header.teid);
    println!("  Sequence Number: {:?}", end_marker.header.sequence_number);
    println!();

    // Example 5: Multiple QFI Values (5G QoS Flows)
    println!("Example 5: Different QoS Flows (Multiple QFI Values)");
    println!("─────────────────────────────────────────");

    let qos_flows = [
        (1, "GBR voice"),
        (5, "IMS signaling"),
        (9, "Default bearer"),
        (32, "Video streaming"),
        (63, "Maximum QFI value"),
    ];

    for (qfi, description) in &qos_flows {
        let mut msg = GtpuMessage::create_gpdu_from_slice(0x11111111, b"test");

        let pdu_container =
            ExtensionHeader::PduSessionContainer(PduSessionContainer::new(1, *qfi, 0, false));
        msg.add_extension_header(pdu_container);

        // Encode and decode
        let mut buf = Vec::new();
        msg.encode(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = GtpuMessage::decode(&mut cursor).unwrap();

        println!("  QFI {}: {} ✓", decoded.get_qfi().unwrap(), description);
    }
    println!();

    // Example 6: Message Length Calculation
    println!("Example 6: Message Length Calculation");
    println!("─────────────────────────────────────────");

    let mut msg = GtpuMessage::create_gpdu_from_slice(0x12345678, b"Small payload");
    println!("Initial length: {} bytes", msg.header.length);

    // Add payload
    msg.set_payload_from_slice(b"Larger payload with more data");
    println!("After larger payload: {} bytes", msg.header.length);

    // Add extension header
    let pdu_container =
        ExtensionHeader::PduSessionContainer(PduSessionContainer::new(1, 9, 0, false));
    msg.add_extension_header(pdu_container);
    println!("After extension header: {} bytes", msg.header.length);
    println!(
        "  (includes 4 bytes optional header + 4 bytes extension + {} bytes payload)",
        msg.payload.len()
    );
    println!();

    // Example 7: Extension Header Type Checking
    println!("Example 7: Extension Header Type Checking");
    println!("─────────────────────────────────────────");

    let mut msg_with_ext = GtpuMessage::create_gpdu_from_slice(0x12345678, b"test");
    let pdu_container =
        ExtensionHeader::PduSessionContainer(PduSessionContainer::new(1, 9, 0, false));
    msg_with_ext.add_extension_header(pdu_container);

    println!("Message extension headers:");
    println!(
        "  Has PDU Session Container: {}",
        msg_with_ext.has_extension_header_type(rs_gtp_u::ExtensionHeaderType::PduSessionContainer)
    );
    println!(
        "  Has PDCP PDU Number: {}",
        msg_with_ext.has_extension_header_type(rs_gtp_u::ExtensionHeaderType::PdcpPduNumber)
    );
    println!(
        "  Has UDP Port: {}",
        msg_with_ext.has_extension_header_type(rs_gtp_u::ExtensionHeaderType::UdpPort)
    );
    println!();

    // Example 8: Round-trip Test Suite
    println!("Example 8: Round-Trip Validation");
    println!("─────────────────────────────────────────");

    let test_cases = vec![
        (
            "Simple G-PDU",
            GtpuMessage::create_gpdu_from_slice(0x12345678, b"Simple"),
        ),
        ("Echo Request", GtpuMessage::create_echo_request(123)),
        (
            "End Marker",
            GtpuMessage::create_end_marker(0x99887766, 999),
        ),
    ];

    for (name, mut original) in test_cases {
        let mut buffer = Vec::new();
        original.encode(&mut buffer).unwrap();

        let mut cursor = Cursor::new(&buffer);
        let decoded = GtpuMessage::decode(&mut cursor).unwrap();

        let matches = original.header.message_type == decoded.header.message_type
            && original.header.teid == decoded.header.teid
            && original.payload == decoded.payload;

        println!("  {} {}", if matches { "✓" } else { "✗" }, name);
    }

    println!("\n=== All Examples Complete ===");
}
