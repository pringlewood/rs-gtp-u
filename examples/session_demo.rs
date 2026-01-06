//! GTP-U Session Management Demo
//!
//! This example demonstrates PDU session lifecycle management, including:
//! - Session creation with uplink/downlink tunnels
//! - IPv4, IPv6, and dual-stack sessions
//! - QoS flow management
//! - Session activation and state management
//! - Finding sessions by TEID

use rs_gtp_u::qos::{AllocationRetentionPriority, QosCharacteristics, QosFlowParams};
use rs_gtp_u::session::{PduSessionType, SessionManager};
use rs_gtp_u::tunnel::TunnelManager;

fn main() {
    println!("=== GTP-U Session Management Demo ===\n");

    // Example 1: Basic PDU Session Creation (IPv4)
    println!("Example 1: Basic IPv4 PDU Session");
    println!("─────────────────────────────────────────");

    let mut tunnel_mgr = TunnelManager::new();
    let mut session_mgr = SessionManager::new(&mut tunnel_mgr);

    let local_addr = "192.168.1.1:2152".parse().unwrap();
    let remote_addr = "192.168.1.2:2152".parse().unwrap();

    println!("Creating IPv4 PDU session...");
    println!("  Session ID: 1");
    println!("  Session Type: {}", PduSessionType::Ipv4);
    println!("  DNN: internet");
    println!("  Local Address: {}", local_addr);
    println!("  Remote Address: {}", remote_addr);

    session_mgr
        .create_session(1, PduSessionType::Ipv4, "internet", local_addr, remote_addr)
        .unwrap();

    println!("\n✓ Session created successfully");
    println!("  Total sessions: {}", session_mgr.get_session_count());

    let session = session_mgr.get_session(1).unwrap();
    println!("  Uplink TEID: 0x{:08X}", session.get_uplink_tunnel().unwrap());
    println!(
        "  Downlink TEID: 0x{:08X}",
        session.get_downlink_tunnel().unwrap()
    );
    println!();

    // Example 2: IPv6 PDU Session
    println!("Example 2: IPv6 PDU Session");
    println!("─────────────────────────────────────────");

    let local_ipv6 = "[2001:db8::1]:2152".parse().unwrap();
    let remote_ipv6 = "[2001:db8::2]:2152".parse().unwrap();

    println!("Creating IPv6 PDU session...");
    println!("  Session ID: 2");
    println!("  Session Type: {}", PduSessionType::Ipv6);
    println!("  DNN: internet");

    session_mgr
        .create_session(
            2,
            PduSessionType::Ipv6,
            "internet",
            local_ipv6,
            remote_ipv6,
        )
        .unwrap();

    println!("\n✓ IPv6 session created successfully");
    let session = session_mgr.get_session(2).unwrap();
    println!("  Session Type: {}", session.session_type);
    println!("  Uplink TEID: 0x{:08X}", session.get_uplink_tunnel().unwrap());
    println!(
        "  Downlink TEID: 0x{:08X}",
        session.get_downlink_tunnel().unwrap()
    );
    println!();

    // Example 3: Dual-Stack IPv4v6 Session
    println!("Example 3: Dual-Stack IPv4v6 Session");
    println!("─────────────────────────────────────────");

    println!("Creating dual-stack PDU session...");
    println!("  Session ID: 3");
    println!("  Session Type: {}", PduSessionType::Ipv4v6);
    println!("  DNN: internet");

    session_mgr
        .create_session(3, PduSessionType::Ipv4v6, "internet", local_addr, remote_addr)
        .unwrap();

    println!("\n✓ Dual-stack session created successfully");
    let session = session_mgr.get_session(3).unwrap();
    println!("  Session Type: {}", session.session_type);
    println!();

    // Example 4: Session with QoS Flows
    println!("Example 4: Session with QoS Flows");
    println!("─────────────────────────────────────────");

    let session = session_mgr.get_session(1).unwrap();

    println!("Adding QoS flows to session 1...");

    // Add multiple QoS flows
    let qos_flows = [
        (1, "GBR voice"),
        (5, "IMS signaling"),
        (9, "Default bearer"),
        (32, "Video streaming"),
    ];

    for (qfi, description) in &qos_flows {
        let params = QosFlowParams::new(
            *qfi,
            QosCharacteristics::DefaultBearer,
            AllocationRetentionPriority::new(5),
        );
        session.add_qos_flow(params).unwrap();
        println!("  ✓ Added QFI {}: {}", qfi, description);
    }

    println!("\nActive QoS flows: {}", session.get_active_qos_flows());

    // Remove a flow
    println!("\nRemoving QFI 5 (IMS signaling)...");
    session.remove_qos_flow(5);
    println!("Active QoS flows: {}", session.get_active_qos_flows());
    println!();

    // Example 5: Session Activation
    println!("Example 5: Session Activation");
    println!("─────────────────────────────────────────");

    let session = session_mgr.get_session(1).unwrap();

    println!("Session state before activation: {:?}", session.get_state());
    println!("Is active: {}", session.is_active());

    println!("\nActivating session...");
    session.activate().unwrap();

    println!("Session state after activation: {:?}", session.get_state());
    println!("Is active: {}", session.is_active());
    println!();

    // Example 6: UE IP Assignment
    println!("Example 6: UE IP Assignment");
    println!("─────────────────────────────────────────");

    let session = session_mgr.get_session(1).unwrap();

    println!("UE IP before assignment: {:?}", session.get_ue_ip());

    let ue_ip = "10.45.0.1".parse().unwrap();
    session.set_ue_ip(ue_ip);

    println!("UE IP after assignment: {:?}", session.get_ue_ip());
    println!();

    // Example 7: Find Session by TEID
    println!("Example 7: Find Session by TEID");
    println!("─────────────────────────────────────────");

    let session = session_mgr.get_session(1).unwrap();
    let ul_teid = session.get_uplink_tunnel().unwrap();
    let dl_teid = session.get_downlink_tunnel().unwrap();

    println!("Looking up sessions by TEID...");
    println!("  Uplink TEID: 0x{:08X}", ul_teid);

    let found_session = session_mgr.find_session_by_teid(ul_teid).unwrap();
    println!("  ✓ Found session ID: {}", found_session.id);
    println!("    DNN: {}", found_session.dnn);

    println!("\n  Downlink TEID: 0x{:08X}", dl_teid);
    let found_session = session_mgr.find_session_by_teid(dl_teid).unwrap();
    println!("  ✓ Found session ID: {}", found_session.id);
    println!();

    // Example 8: Multiple Sessions
    println!("Example 8: Multiple Sessions");
    println!("─────────────────────────────────────────");

    println!("Creating multiple sessions for different DNNs...");

    let dnn_configs = [
        (10, "internet"),
        (11, "ims"),
        (12, "enterprise"),
        (13, "iot"),
    ];

    for (id, dnn) in &dnn_configs {
        session_mgr
            .create_session(*id, PduSessionType::Ipv4, dnn, local_addr, remote_addr)
            .unwrap();
        let session = session_mgr.get_session(*id).unwrap();
        session.activate().ok();
        println!("  ✓ Session {} ({})", id, dnn);
    }

    println!("\nSession Manager Summary:");
    println!("  Total sessions: {}", session_mgr.get_session_count());
    println!("  Active sessions: {}", session_mgr.get_active_sessions());
    println!();

    // Example 9: Session Lifecycle
    println!("Example 9: Session Lifecycle");
    println!("─────────────────────────────────────────");

    let session = session_mgr.get_session(10).unwrap();

    println!("Session 10 lifecycle:");
    println!("  State: {:?}", session.get_state());
    println!("  Lifetime: {:?}", session.get_lifetime());

    println!("\nReleasing session...");
    session.release();
    println!("  State: {:?}", session.get_state());
    println!();

    // Example 10: Session Removal
    println!("Example 10: Session Removal");
    println!("─────────────────────────────────────────");

    println!("Sessions before removal: {}", session_mgr.get_session_count());

    println!("\nRemoving session 10...");
    session_mgr.remove_session(10).unwrap();

    println!("Sessions after removal: {}", session_mgr.get_session_count());
    println!();

    // Example 11: Error Handling
    println!("Example 11: Error Handling");
    println!("─────────────────────────────────────────");

    println!("Attempting to create duplicate session (ID: 1)...");
    match session_mgr.create_session(1, PduSessionType::Ipv4, "test", local_addr, remote_addr) {
        Ok(_) => println!("  Session created"),
        Err(e) => println!("  ✓ Error caught: {}", e),
    }

    println!("\nAttempting to activate session without tunnels...");
    let new_session = rs_gtp_u::session::PduSession::new(99, PduSessionType::Ipv4, "test");
    match new_session.activate() {
        Ok(_) => println!("  Session activated"),
        Err(e) => println!("  ✓ Error caught: {}", e),
    }
    println!();

    // Example 12: Session Types Comparison
    println!("Example 12: Session Types Comparison");
    println!("─────────────────────────────────────────");

    let session_types = [
        PduSessionType::Ipv4,
        PduSessionType::Ipv6,
        PduSessionType::Ipv4v6,
        PduSessionType::Ethernet,
        PduSessionType::Unstructured,
    ];

    println!("Available PDU Session Types:");
    for session_type in &session_types {
        println!("  - {}", session_type);
    }
    println!();

    // Example 13: Session Information Display
    println!("Example 13: Session Information Display");
    println!("─────────────────────────────────────────");

    let session = session_mgr.get_session(1).unwrap();

    println!("Session 1 Details:");
    println!("  ID: {}", session.id);
    println!("  Type: {}", session.session_type);
    println!("  DNN: {}", session.dnn);
    println!("  State: {:?}", session.get_state());
    println!("  Active: {}", session.is_active());
    println!(
        "  Uplink TEID: 0x{:08X}",
        session.get_uplink_tunnel().unwrap()
    );
    println!(
        "  Downlink TEID: 0x{:08X}",
        session.get_downlink_tunnel().unwrap()
    );
    if let Some(ue_ip) = session.get_ue_ip() {
        println!("  UE IP: {}", ue_ip);
    }
    println!("  QoS Flows: {}", session.get_active_qos_flows());
    println!("  Lifetime: {:?}", session.get_lifetime());
    println!();

    // Final Summary
    println!("=== Final Summary ===");
    println!("Total sessions in manager: {}", session_mgr.get_session_count());
    println!(
        "Active sessions in manager: {}",
        session_mgr.get_active_sessions()
    );

    // List all sessions
    println!("\nAll Sessions:");
    for id in [1, 2, 3, 11, 12, 13] {
        if let Some(session) = session_mgr.get_session(id) {
            println!(
                "  Session {}: {} ({}) - {:?}",
                session.id,
                session.dnn,
                session.session_type,
                session.get_state()
            );
        }
    }

    println!("\n=== All Examples Complete ===");
}
