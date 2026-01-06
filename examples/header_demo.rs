//! GTP-U Header Encoding/Decoding Demo
//!
//! This example demonstrates the complete header encode/decode cycle,
//! including optional fields and wire format inspection.

use rs_gtp_u::{GtpuHeader, MessageType};
use std::io::Cursor;

fn print_hex_dump(label: &str, bytes: &[u8]) {
    println!("{}", label);
    print!("  Hex: ");
    for byte in bytes {
        print!("{:02X} ", byte);
    }
    println!();

    print!("  Dec: ");
    for byte in bytes {
        print!("{:3} ", byte);
    }
    println!("\n");
}

fn main() {
    println!("=== GTP-U Header Encoding/Decoding Demo ===\n");

    // Example 1: Basic G-PDU header (mandatory only)
    println!("Example 1: Basic G-PDU Header (8 bytes)");
    println!("─────────────────────────────────────────");

    let header1 = GtpuHeader::new(MessageType::Gpdu, 0x12345678);
    println!("{}", header1);

    let mut buffer1 = Vec::new();
    header1.encode(&mut buffer1).unwrap();

    print_hex_dump("Encoded bytes:", &buffer1);

    // Decode and verify
    let mut cursor1 = Cursor::new(&buffer1);
    let decoded1 = GtpuHeader::decode(&mut cursor1).unwrap();
    println!("✓ Decode successful: TEID = 0x{:08X}\n", decoded1.teid);

    // Example 2: Echo Request (uses TEID=0)
    println!("Example 2: Echo Request (8 bytes)");
    println!("─────────────────────────────────────────");

    let header2 = GtpuHeader::new(MessageType::EchoRequest, 0x00000000);
    println!("{}", header2);

    let mut buffer2 = Vec::new();
    header2.encode(&mut buffer2).unwrap();

    print_hex_dump("Encoded bytes:", &buffer2);

    let mut cursor2 = Cursor::new(&buffer2);
    let decoded2 = GtpuHeader::decode(&mut cursor2).unwrap();
    println!(
        "✓ Decode successful: Message Type = {}\n",
        decoded2.message_type
    );

    // Example 3: G-PDU with sequence number (12 bytes)
    println!("Example 3: G-PDU with Sequence Number (12 bytes)");
    println!("─────────────────────────────────────────");

    let mut header3 = GtpuHeader::new(MessageType::Gpdu, 0xABCDEF01);
    header3.length = 1024;
    header3.flags.set_s(true); // Enable sequence number
    header3.sequence_number = Some(42);

    println!("{}", header3);

    let mut buffer3 = Vec::new();
    header3.encode(&mut buffer3).unwrap();

    print_hex_dump("Encoded bytes:", &buffer3);

    let mut cursor3 = Cursor::new(&buffer3);
    let decoded3 = GtpuHeader::decode(&mut cursor3).unwrap();
    println!(
        "✓ Decode successful: Sequence = {:?}\n",
        decoded3.sequence_number
    );

    // Example 4: G-PDU with extension header pointer (12 bytes)
    println!("Example 4: G-PDU with Extension Header (12 bytes)");
    println!("─────────────────────────────────────────");

    let mut header4 = GtpuHeader::new(MessageType::Gpdu, 0x99887766);
    header4.length = 500;
    header4.flags.set_e(true); // Enable extension header
    header4.next_extension_type = Some(rs_gtp_u::ExtensionHeaderType::PduSessionContainer);

    println!("{}", header4);

    let mut buffer4 = Vec::new();
    header4.encode(&mut buffer4).unwrap();

    print_hex_dump("Encoded bytes:", &buffer4);

    let mut cursor4 = Cursor::new(&buffer4);
    let decoded4 = GtpuHeader::decode(&mut cursor4).unwrap();
    println!(
        "✓ Decode successful: Next Ext = {:?}\n",
        decoded4.next_extension_type
    );

    // Example 5: Complete header with all optional fields
    println!("Example 5: Complete Header with All Optional Fields (12 bytes)");
    println!("─────────────────────────────────────────");

    let mut header5 = GtpuHeader::new(MessageType::Gpdu, 0xFFFFFFFF);
    header5.length = 2048;
    header5.flags.set_s(true);
    header5.flags.set_e(true);
    header5.flags.set_pn(true);
    header5.sequence_number = Some(1234);
    header5.n_pdu_number = Some(5);
    header5.next_extension_type = Some(rs_gtp_u::ExtensionHeaderType::PdcpPduNumber);

    println!("{}", header5);

    let mut buffer5 = Vec::new();
    header5.encode(&mut buffer5).unwrap();

    print_hex_dump("Encoded bytes:", &buffer5);

    let mut cursor5 = Cursor::new(&buffer5);
    let decoded5 = GtpuHeader::decode(&mut cursor5).unwrap();
    println!("✓ Decode successful:");
    println!("  Sequence: {:?}", decoded5.sequence_number);
    println!("  N-PDU: {:?}", decoded5.n_pdu_number);
    println!("  Next Ext: {:?}\n", decoded5.next_extension_type);

    // Example 6: Round-trip test
    println!("Example 6: Round-Trip Verification");
    println!("─────────────────────────────────────────");

    for test_case in &[
        ("Basic G-PDU", header1),
        ("Echo Request", header2),
        ("With Sequence", header3.clone()),
        ("With Extension", header4.clone()),
        ("Complete", header5.clone()),
    ] {
        let mut buf = Vec::new();
        test_case.1.encode(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = GtpuHeader::decode(&mut cursor).unwrap();

        let matches = test_case.1 == decoded;
        println!("  {} {}", if matches { "✓" } else { "✗" }, test_case.0);
    }

    println!("\n=== All Examples Complete ===");
}
