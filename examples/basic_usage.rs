//! Basic usage example for rs-gtp-u
//!
//! This example demonstrates the fundamental protocol types and their usage.

use rs_gtp_u::{ExtensionHeaderType, MessageType, GTPU_PORT, GTPU_VERSION};

fn main() {
    println!("=== rs-gtp-u Basic Usage Example ===\n");

    // Protocol constants
    println!("Protocol Information:");
    println!("  GTP-U Version: {}", GTPU_VERSION);
    println!("  Default Port: {}", GTPU_PORT);
    println!();

    // Message types
    println!("Message Types:");
    let message_types = [
        MessageType::EchoRequest,
        MessageType::EchoResponse,
        MessageType::ErrorIndication,
        MessageType::EndMarker,
        MessageType::Gpdu,
    ];

    for msg_type in &message_types {
        println!("  {:3} - {} - TEID Required: {}",
            *msg_type as u8,
            msg_type,
            msg_type.requires_teid()
        );
    }
    println!();

    // Extension headers
    println!("Important Extension Headers:");
    let ext_headers = [
        ExtensionHeaderType::PduSessionContainer,
        ExtensionHeaderType::PdcpPduNumber,
        ExtensionHeaderType::LongPdcpPduNumber,
        ExtensionHeaderType::NrRanContainer,
    ];

    for ext in &ext_headers {
        println!("  0x{:02X} - {}", *ext as u8, ext);
    }
    println!();

    // Demonstrate validation
    println!("Message Type Validation:");
    let gpdu = MessageType::Gpdu;
    println!("  G-PDU is valid: {}", gpdu.is_valid());
    println!("  G-PDU requires TEID: {}", gpdu.requires_teid());
    println!("  G-PDU wire value: {}", gpdu as u8);
    println!();

    // Demonstrate parsing
    println!("Parsing from Wire Format:");
    let wire_value: u8 = 255;
    match MessageType::from_u8(wire_value) {
        Some(msg_type) => {
            println!("  0x{:02X} parsed as: {}", wire_value, msg_type);
        }
        None => {
            println!("  0x{:02X} is not a valid message type", wire_value);
        }
    }

    let invalid_value: u8 = 99;
    match MessageType::from_u8(invalid_value) {
        Some(msg_type) => {
            println!("  0x{:02X} parsed as: {}", invalid_value, msg_type);
        }
        None => {
            println!("  0x{:02X} is not a valid message type", invalid_value);
        }
    }
    println!();

    // Compare Echo vs G-PDU
    println!("Echo Request vs G-PDU Comparison:");
    let echo = MessageType::EchoRequest;
    let gpdu = MessageType::Gpdu;

    println!("  Echo Request:");
    println!("    - Wire value: {}", echo as u8);
    println!("    - Requires TEID: {} (uses TEID=0 for signaling)", echo.requires_teid());
    println!("    - Use case: Path management");

    println!("  G-PDU:");
    println!("    - Wire value: {}", gpdu as u8);
    println!("    - Requires TEID: {} (identifies tunnel)", gpdu.requires_teid());
    println!("    - Use case: User data tunneling");
    println!();

    println!("=== Example Complete ===");
}
