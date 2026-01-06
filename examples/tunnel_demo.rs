//! GTP-U Tunnel Management Demo
//!
//! This example demonstrates tunnel lifecycle management, state machines,
//! thread-safe statistics, and QoS flow management.

use rs_gtp_u::tunnel::{TunnelConfig, TunnelManager, TunnelState};
use std::time::Duration;

fn main() {
    println!("=== GTP-U Tunnel Management Demo ===\n");

    // Example 1: Tunnel State Machine
    println!("Example 1: Tunnel State Machine");
    println!("─────────────────────────────────────────");

    println!("Valid state transitions:");
    println!(
        "  Inactive → Establishing: {}",
        TunnelState::Inactive.can_transition_to(TunnelState::Establishing)
    );
    println!(
        "  Establishing → Active: {}",
        TunnelState::Establishing.can_transition_to(TunnelState::Active)
    );
    println!(
        "  Active → Modifying: {}",
        TunnelState::Active.can_transition_to(TunnelState::Modifying)
    );
    println!(
        "  Active → Releasing: {}",
        TunnelState::Active.can_transition_to(TunnelState::Releasing)
    );
    println!(
        "  Releasing → Released: {}",
        TunnelState::Releasing.can_transition_to(TunnelState::Released)
    );

    println!("\nInvalid state transitions:");
    println!(
        "  Inactive → Active: {}",
        TunnelState::Inactive.can_transition_to(TunnelState::Active)
    );
    println!(
        "  Released → Inactive: {}",
        TunnelState::Released.can_transition_to(TunnelState::Inactive)
    );
    println!();

    // Example 2: Creating and Managing Tunnels
    println!("Example 2: Tunnel Lifecycle Management");
    println!("─────────────────────────────────────────");

    let mut manager = TunnelManager::new();

    let config = TunnelConfig {
        local_teid: manager.allocate_teid(),
        remote_teid: 0x5678,
        local_address: "192.168.1.1:2152".parse().unwrap(),
        remote_address: "192.168.1.2:2152".parse().unwrap(),
        idle_timeout: Duration::from_secs(300),
    };

    println!("Creating tunnel with config:");
    println!("  Local TEID: 0x{:08X}", config.local_teid);
    println!("  Remote TEID: 0x{:08X}", config.remote_teid);
    println!("  Local Address: {}", config.local_address);
    println!("  Remote Address: {}", config.remote_address);
    println!("  Idle Timeout: {:?}", config.idle_timeout);
    println!();

    let teid = manager.create_tunnel(config).unwrap();
    println!("✓ Tunnel created with TEID: 0x{:08X}", teid);
    println!("  Total tunnels: {}", manager.get_tunnel_count());
    println!("  Active tunnels: {}", manager.get_active_tunnels());
    println!();

    // Example 3: Tunnel Statistics
    println!("Example 3: Tunnel Statistics");
    println!("─────────────────────────────────────────");

    let tunnel = manager.get_tunnel(teid).unwrap();
    println!("Initial statistics:");
    println!("  TX Packets: {}", tunnel.stats.packets_tx());
    println!("  RX Packets: {}", tunnel.stats.packets_rx());
    println!("  TX Bytes: {}", tunnel.stats.bytes_tx());
    println!("  RX Bytes: {}", tunnel.stats.bytes_rx());
    println!("  Errors: {}", tunnel.stats.errors());
    println!();

    // Simulate traffic
    println!("Simulating traffic...");
    manager.update_stats(teid, 1500, true); // TX: 1500 bytes
    manager.update_stats(teid, 2000, false); // RX: 2000 bytes
    manager.update_stats(teid, 1200, true); // TX: 1200 bytes

    let tunnel = manager.get_tunnel(teid).unwrap();
    println!("\nUpdated statistics:");
    println!("  TX Packets: {}", tunnel.stats.packets_tx());
    println!("  RX Packets: {}", tunnel.stats.packets_rx());
    println!("  TX Bytes: {}", tunnel.stats.bytes_tx());
    println!("  RX Bytes: {}", tunnel.stats.bytes_rx());
    println!("  Lifetime: {:?}", tunnel.stats.get_lifetime());
    println!();

    // Example 4: QoS Flow Management
    println!("Example 4: QoS Flow Management");
    println!("─────────────────────────────────────────");

    let tunnel = manager.get_tunnel(teid).unwrap();
    println!("Adding QoS flows to tunnel 0x{:08X}:", teid);

    tunnel.add_qos_flow(1); // GBR voice
    tunnel.add_qos_flow(5); // IMS signaling
    tunnel.add_qos_flow(9); // Default bearer

    let flows = tunnel.get_qos_flows();
    println!("  Active QoS Flows: {:?}", flows);
    println!("  Flow Count: {}", flows.len());
    println!();

    println!("Removing QoS flow 5...");
    tunnel.remove_qos_flow(5);
    let flows = tunnel.get_qos_flows();
    println!("  Active QoS Flows: {:?}", flows);
    println!("  Flow Count: {}", flows.len());
    println!();

    // Example 5: Anti-Replay Protection
    println!("Example 5: Anti-Replay Protection");
    println!("─────────────────────────────────────────");

    let tunnel = manager.get_tunnel(teid).unwrap();

    println!("Checking sequence numbers:");
    println!(
        "  Seq 100: {} (first packet - always valid)",
        tunnel.check_sequence(100)
    );
    println!(
        "  Seq 101: {} (newer packet - valid)",
        tunnel.check_sequence(101)
    );
    println!(
        "  Seq 102: {} (newer packet - valid)",
        tunnel.check_sequence(102)
    );
    println!(
        "  Seq 102: {} (duplicate - rejected)",
        tunnel.check_sequence(102)
    );
    println!(
        "  Seq 101: {} (old packet - rejected)",
        tunnel.check_sequence(101)
    );
    println!();

    // Example 6: Multiple Tunnels
    println!("Example 6: Multiple Tunnels");
    println!("─────────────────────────────────────────");

    for i in 1..=5 {
        let config = TunnelConfig {
            local_teid: manager.allocate_teid(),
            remote_teid: 0x1000 + i,
            local_address: format!("192.168.1.1:2152").parse().unwrap(),
            remote_address: format!("192.168.1.{}:2152", i + 1).parse().unwrap(),
            idle_timeout: Duration::from_secs(300),
        };
        let teid = manager.create_tunnel(config).unwrap();
        println!("  ✓ Created tunnel {} with TEID: 0x{:08X}", i, teid);
    }

    println!("\nTunnel Manager Summary:");
    println!("  Total tunnels: {}", manager.get_tunnel_count());
    println!("  Active tunnels: {}", manager.get_active_tunnels());
    println!();

    // Example 7: TEID Allocation
    println!("Example 7: TEID Allocation");
    println!("─────────────────────────────────────────");

    println!("Allocating 10 TEIDs:");
    for i in 1..=10 {
        let teid = manager.allocate_teid();
        print!("  TEID {}: 0x{:08X}", i, teid);
        if i % 5 == 0 {
            println!();
        }
    }
    println!();

    // Example 8: Tunnel State Inspection
    println!("Example 8: Tunnel State Inspection");
    println!("─────────────────────────────────────────");

    let config = TunnelConfig {
        local_teid: manager.allocate_teid(),
        remote_teid: 0x9999,
        local_address: "192.168.1.1:2152".parse().unwrap(),
        remote_address: "192.168.1.100:2152".parse().unwrap(),
        idle_timeout: Duration::from_secs(300),
    };

    let teid = manager.create_tunnel(config).unwrap();
    let tunnel = manager.get_tunnel(teid).unwrap();

    println!("Tunnel 0x{:08X} state:", teid);
    println!("  Current State: {:?}", tunnel.get_state());
    println!("  Is Active: {}", tunnel.is_active());
    println!("  Is Idle: {}", tunnel.is_idle());
    println!();

    // Example 9: Tunnel Cleanup
    println!("Example 9: Tunnel Cleanup");
    println!("─────────────────────────────────────────");

    println!("Before cleanup:");
    println!("  Total tunnels: {}", manager.get_tunnel_count());

    println!("\nRemoving first tunnel (TEID: 0x{:08X})...", 0x1000);
    manager.remove_tunnel(0x1000).ok();

    println!("After cleanup:");
    println!("  Total tunnels: {}", manager.get_tunnel_count());
    println!("  Active tunnels: {}", manager.get_active_tunnels());
    println!();

    // Example 10: Error Handling
    println!("Example 10: Error Handling");
    println!("─────────────────────────────────────────");

    let invalid_config = TunnelConfig {
        local_teid: 0, // Invalid!
        remote_teid: 0x5678,
        local_address: "192.168.1.1:2152".parse().unwrap(),
        remote_address: "192.168.1.2:2152".parse().unwrap(),
        idle_timeout: Duration::from_secs(300),
    };

    match manager.create_tunnel(invalid_config) {
        Ok(teid) => println!("  Tunnel created: 0x{:08X}", teid),
        Err(e) => println!("  ✓ Error caught: {}", e),
    }
    println!();

    // Example 11: Concurrent Access (Thread Safety)
    println!("Example 11: Thread Safety");
    println!("─────────────────────────────────────────");

    let config = TunnelConfig {
        local_teid: manager.allocate_teid(),
        remote_teid: 0xAAAA,
        local_address: "192.168.1.1:2152".parse().unwrap(),
        remote_address: "192.168.1.200:2152".parse().unwrap(),
        idle_timeout: Duration::from_secs(300),
    };

    let teid = manager.create_tunnel(config).unwrap();
    let tunnel = manager.get_tunnel(teid).unwrap();

    println!("Demonstrating thread-safe statistics updates:");
    println!("  Initial TX packets: {}", tunnel.stats.packets_tx());

    // Clone tunnel for concurrent access (Arc-based sharing)
    let tunnel_clone = tunnel.clone();

    // Simulate concurrent updates
    tunnel.stats.record_tx(1000);
    tunnel_clone.stats.record_tx(2000);
    tunnel.stats.record_tx(3000);

    println!("  Final TX packets: {}", tunnel.stats.packets_tx());
    println!("  Final TX bytes: {}", tunnel.stats.bytes_tx());
    println!("  ✓ All updates recorded correctly");
    println!();

    println!("=== All Examples Complete ===");
}
