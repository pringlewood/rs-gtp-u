//! QoS Flow Management Demo
//!
//! This example demonstrates 5G QoS flow management including:
//! - 5QI (5G QoS Identifier) characteristics
//! - GBR (Guaranteed Bit Rate) and Non-GBR flows
//! - Allocation and Retention Priority (ARP)
//! - QoS flow statistics and monitoring
//! - Flow validation and error handling

use rs_gtp_u::qos::{
    AllocationRetentionPriority, QosCharacteristics, QosFlow, QosFlowManager, QosFlowParams,
};

fn main() {
    println!("=== 5G QoS Flow Management Demo ===\n");

    // Example 1: QoS Characteristics (5QI Values)
    println!("Example 1: 5QI Characteristics");
    println!("─────────────────────────────────────────");

    let characteristics = [
        (
            QosCharacteristics::ConversationalVoice,
            "Conversational Voice",
        ),
        (
            QosCharacteristics::ConversationalVideo,
            "Conversational Video",
        ),
        (QosCharacteristics::RealTimeGaming, "Real-time Gaming"),
        (QosCharacteristics::DefaultBearer, "Default Bearer"),
        (QosCharacteristics::TcpBased, "TCP-based"),
        (
            QosCharacteristics::LiveVideoStreaming,
            "Live Video Streaming",
        ),
    ];

    println!("GBR (Guaranteed Bit Rate) Flows:");
    for (qos, name) in &characteristics {
        if qos.is_gbr() {
            println!("  ✓ {}: 5QI {}", name, qos.as_u8());
        }
    }

    println!("\nNon-GBR Flows:");
    for (qos, name) in &characteristics {
        if !qos.is_gbr() {
            println!("  ✓ {}: 5QI {}", name, qos.as_u8());
        }
    }

    println!("\nDelay-Reliant Flows:");
    for (qos, name) in &characteristics {
        if qos.is_delay_reliant() {
            println!("  ⏱ {}: 5QI {}", name, qos.as_u8());
        }
    }
    println!();

    // Example 2: Creating a Non-GBR QoS Flow
    println!("Example 2: Non-GBR QoS Flow (Default Bearer)");
    println!("─────────────────────────────────────────");

    let non_gbr_params = QosFlowParams::new(
        9, // QFI
        QosCharacteristics::DefaultBearer,
        AllocationRetentionPriority::new(15), // Lowest priority
    );

    println!("QoS Flow Parameters:");
    println!("  QFI: {}", non_gbr_params.qfi);
    println!(
        "  5QI: {} ({})",
        non_gbr_params.fiveqi.as_u8(),
        "Default Bearer"
    );
    println!("  ARP Priority: {}", non_gbr_params.arp.priority_level);
    println!(
        "  Packet Delay Budget: {}ms",
        non_gbr_params.packet_delay_budget
    );
    println!(
        "  Packet Error Rate: 10^-{}",
        non_gbr_params.packet_error_rate
    );
    println!("  Is GBR: {}", non_gbr_params.is_gbr());

    match non_gbr_params.validate() {
        Ok(_) => println!("  ✓ Parameters valid"),
        Err(e) => println!("  ✗ Validation error: {}", e),
    }
    println!();

    // Example 3: Creating a GBR QoS Flow (Voice)
    println!("Example 3: GBR QoS Flow (Conversational Voice)");
    println!("─────────────────────────────────────────");

    let mut gbr_params = QosFlowParams::new(
        1, // QFI
        QosCharacteristics::ConversationalVoice,
        AllocationRetentionPriority::new(2), // High priority
    );

    // GBR flows require bit rate parameters
    gbr_params.gfbr_uplink = Some(64000); // 64 kbps guaranteed uplink
    gbr_params.gfbr_downlink = Some(64000); // 64 kbps guaranteed downlink
    gbr_params.mfbr_uplink = Some(128000); // 128 kbps maximum uplink
    gbr_params.mfbr_downlink = Some(128000); // 128 kbps maximum downlink
    gbr_params.packet_delay_budget = 100; // 100ms delay budget
    gbr_params.packet_error_rate = 2; // 10^-2 error rate

    println!("QoS Flow Parameters:");
    println!("  QFI: {}", gbr_params.qfi);
    println!(
        "  5QI: {} ({})",
        gbr_params.fiveqi.as_u8(),
        "Conversational Voice"
    );
    println!("  ARP Priority: {}", gbr_params.arp.priority_level);
    println!("  Is GBR: {}", gbr_params.is_gbr());
    println!("  GFBR Uplink: {} bps", gbr_params.gfbr_uplink.unwrap());
    println!("  GFBR Downlink: {} bps", gbr_params.gfbr_downlink.unwrap());
    println!("  MFBR Uplink: {} bps", gbr_params.mfbr_uplink.unwrap());
    println!("  MFBR Downlink: {} bps", gbr_params.mfbr_downlink.unwrap());
    println!(
        "  Packet Delay Budget: {}ms",
        gbr_params.packet_delay_budget
    );
    println!("  Packet Error Rate: 10^-{}", gbr_params.packet_error_rate);

    match gbr_params.validate() {
        Ok(_) => println!("  ✓ Parameters valid"),
        Err(e) => println!("  ✗ Validation error: {}", e),
    }
    println!();

    // Example 4: Allocation and Retention Priority (ARP)
    println!("Example 4: Allocation and Retention Priority (ARP)");
    println!("─────────────────────────────────────────");

    let arp_examples = [
        (1, "Emergency services", true, false),
        (5, "Voice call", false, true),
        (10, "Video streaming", false, true),
        (15, "Best effort data", false, true),
    ];

    for (priority, service, can_preempt, vulnerable) in &arp_examples {
        let arp =
            AllocationRetentionPriority::new_with_preemption(*priority, *can_preempt, *vulnerable);
        println!("  {} (Priority {}):", service, priority);
        println!("    Can preempt: {}", arp.preemption_capability);
        println!("    Vulnerable: {}", arp.preemption_vulnerability);
    }
    println!();

    // Example 5: QoS Flow Manager
    println!("Example 5: QoS Flow Manager");
    println!("─────────────────────────────────────────");

    let manager = QosFlowManager::new();

    // Add multiple flows
    println!("Creating multiple QoS flows...");

    let flows_to_create = vec![
        (9, QosCharacteristics::DefaultBearer, "Default bearer"),
        (5, QosCharacteristics::ImsSignaling, "IMS signaling"),
        (1, QosCharacteristics::ConversationalVoice, "Voice"),
    ];

    for (qfi, fiveqi, description) in &flows_to_create {
        let mut params = QosFlowParams::new(*qfi, *fiveqi, AllocationRetentionPriority::new(5));

        // Add GBR parameters if needed
        if fiveqi.is_gbr() {
            params.gfbr_uplink = Some(64000);
            params.gfbr_downlink = Some(64000);
        }

        match manager.create_flow(params) {
            Ok(_) => println!("  ✓ Created QFI {}: {}", qfi, description),
            Err(e) => println!("  ✗ Failed to create QFI {}: {}", qfi, e),
        }
    }

    println!("\nActive flows: {}", manager.get_active_flows());
    println!();

    // Example 6: QoS Flow Statistics
    println!("Example 6: QoS Flow Statistics");
    println!("─────────────────────────────────────────");

    let params = QosFlowParams::new(
        9,
        QosCharacteristics::DefaultBearer,
        AllocationRetentionPriority::new(5),
    );
    let flow = QosFlow::new(params).unwrap();

    println!("Simulating packet transmission...");
    flow.stats.record_packet(1000, 50, true); // TX: 1000 bytes, 50µs delay
    flow.stats.record_packet(2000, 75, false); // RX: 2000 bytes, 75µs delay
    flow.stats.record_packet(1500, 100, true); // TX: 1500 bytes, 100µs delay
    flow.stats.record_packet(1800, 60, false); // RX: 1800 bytes, 60µs delay
    flow.stats.record_drop(); // Dropped packet

    let (tx, rx, dropped) = flow.stats.get_packet_counts();
    println!("\nStatistics:");
    println!("  TX packets: {}", tx);
    println!("  RX packets: {}", rx);
    println!("  Dropped packets: {}", dropped);
    println!(
        "  Packet loss rate: {:.2}%",
        flow.stats.packet_loss_rate() * 100.0
    );
    println!("  Min delay: {}µs", flow.stats.min_delay_us());
    println!("  Avg delay: {}µs", flow.stats.avg_delay_us());
    println!("  Max delay: {}µs", flow.stats.max_delay_us());
    println!(
        "  Throughput: {:.2} Mbps",
        flow.stats.throughput_bps(1.0) / 1_000_000.0
    );
    println!();

    // Example 7: QoS Policy Enforcement
    println!("Example 7: QoS Policy Enforcement");
    println!("─────────────────────────────────────────");

    let params = QosFlowParams::new(
        9,
        QosCharacteristics::DefaultBearer,
        AllocationRetentionPriority::new(5),
    );
    let flow = QosFlow::new(params).unwrap();

    println!("Flow active: {}", flow.is_active());
    println!("Can accept packet: {}", flow.can_accept_packet(1000));
    println!("Should drop: {}", flow.should_drop());

    flow.set_active(false);
    println!("\nAfter deactivating flow:");
    println!("Flow active: {}", flow.is_active());
    println!("Can accept packet: {}", flow.can_accept_packet(1000));
    println!("Should drop: {}", flow.should_drop());
    println!();

    // Example 8: Error Handling
    println!("Example 8: QoS Flow Validation Errors");
    println!("─────────────────────────────────────────");

    // Invalid QFI (>63)
    println!("Testing invalid QFI (64)...");
    let invalid_qfi = QosFlowParams::new(
        64, // Invalid!
        QosCharacteristics::DefaultBearer,
        AllocationRetentionPriority::new(5),
    );
    match invalid_qfi.validate() {
        Ok(_) => println!("  ✓ Valid"),
        Err(e) => println!("  ✗ Error: {}", e),
    }

    // Missing GBR parameters for GBR flow
    println!("\nTesting GBR flow without bit rate parameters...");
    let missing_gbr = QosFlowParams::new(
        1,
        QosCharacteristics::ConversationalVoice, // GBR flow
        AllocationRetentionPriority::new(5),
        // Missing gfbr_uplink and gfbr_downlink
    );
    match missing_gbr.validate() {
        Ok(_) => println!("  ✓ Valid"),
        Err(e) => println!("  ✗ Error: {}", e),
    }

    // Invalid ARP priority
    println!("\nTesting invalid ARP priority (16)...");
    let mut invalid_arp = QosFlowParams::new(
        9,
        QosCharacteristics::DefaultBearer,
        AllocationRetentionPriority::new(5),
    );
    invalid_arp.arp.priority_level = 16; // Invalid!
    match invalid_arp.validate() {
        Ok(_) => println!("  ✓ Valid"),
        Err(e) => println!("  ✗ Error: {}", e),
    }
    println!();

    // Example 9: QoS Flow Comparison
    println!("Example 9: Comparing Different QoS Flows");
    println!("─────────────────────────────────────────");

    let comparison_flows = vec![
        (
            QosCharacteristics::ConversationalVoice,
            "Voice",
            100,
            2,
            Some(64000),
        ),
        (
            QosCharacteristics::LiveVideoStreaming,
            "Video",
            300,
            3,
            None,
        ),
        (QosCharacteristics::TcpBased, "Web/Email", 300, 6, None),
        (
            QosCharacteristics::RealTimeGaming,
            "Gaming",
            50,
            3,
            Some(500000),
        ),
    ];

    for (qos, name, delay, error, bitrate) in &comparison_flows {
        println!("{}:", name);
        println!("  5QI: {}", qos.as_u8());
        println!("  GBR: {}", if qos.is_gbr() { "Yes" } else { "No" });
        println!(
            "  Delay-reliant: {}",
            if qos.is_delay_reliant() { "Yes" } else { "No" }
        );
        println!("  Delay budget: {}ms", delay);
        println!("  Error rate: 10^-{}", error);
        if let Some(rate) = bitrate {
            println!("  Required bitrate: {} kbps", rate / 1000);
        }
        println!();
    }

    // Example 10: Complete Flow Lifecycle
    println!("Example 10: Complete QoS Flow Lifecycle");
    println!("─────────────────────────────────────────");

    let manager = QosFlowManager::new();

    println!("1. Creating flow...");
    let params = QosFlowParams::new(
        9,
        QosCharacteristics::DefaultBearer,
        AllocationRetentionPriority::new(5),
    );
    manager.create_flow(params).unwrap();
    println!("   ✓ Flow created (QFI 9)");

    println!("\n2. Getting flow...");
    if let Some(flow) = manager.get_flow(9) {
        println!("   ✓ Flow found");
        println!("   Active: {}", flow.is_active());
        println!("   5QI: {}", flow.params.fiveqi.as_u8());
    }

    println!("\n3. Recording traffic...");
    if let Some(flow) = manager.get_flow(9) {
        for i in 1..=5 {
            flow.stats.record_packet(1000, 50 + i * 10, true);
        }
        let (tx, _, _) = flow.stats.get_packet_counts();
        println!("   ✓ Recorded {} packets", tx);
    }

    println!("\n4. Removing flow...");
    manager.remove_flow(9);
    println!("   ✓ Flow removed");
    println!("   Active flows: {}", manager.get_active_flows());

    println!("\n=== All Examples Complete ===");
}
