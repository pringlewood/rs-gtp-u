//! GTP-U Path Management Demo
//!
//! This example demonstrates path management and monitoring including:
//! - Path state machine (unknown → active/suspect → failed)
//! - Echo request/response mechanism
//! - RTT (Round-Trip Time) tracking
//! - Path failure detection
//! - Multi-peer path management

use rs_gtp_u::path::{Path, PathConfig, PathManager, PathState};
use std::time::Duration;

fn main() {
    println!("=== GTP-U Path Management Demo ===\n");

    // Example 1: Path Configuration
    println!("Example 1: Path Configuration");
    println!("─────────────────────────────────────────");

    let config = PathConfig {
        echo_interval: Duration::from_secs(60),
        echo_timeout: Duration::from_secs(5),
        max_echo_failures: 3,
        suspect_threshold: Duration::from_millis(10),
    };

    println!("Path Configuration:");
    println!("  Echo Interval: {:?}", config.echo_interval);
    println!("  Echo Timeout: {:?}", config.echo_timeout);
    println!("  Max Echo Failures: {}", config.max_echo_failures);
    println!("  Suspect Threshold: {:?}", config.suspect_threshold);
    println!();

    // Example 2: Creating a Path
    println!("Example 2: Path Creation and State");
    println!("─────────────────────────────────────────");

    let peer_addr = "192.168.1.1:2152".parse().unwrap();
    let path = Path::new(peer_addr, config.clone());

    println!("Path created for peer: {}", path.peer_address);
    println!("  Initial state: {:?}", path.get_state());
    println!("  Is healthy: {}", path.is_healthy());
    println!();

    // Example 3: Echo Request/Response Mechanism
    println!("Example 3: Echo Request/Response");
    println!("─────────────────────────────────────────");

    println!("Sending echo request...");
    let sequence = path.send_echo();
    println!("  Sequence number: {}", sequence);

    let stats = path.get_stats();
    println!("  Echo requests sent: {}", stats.echo_requests_sent);
    println!();

    // Simulate echo response
    println!("Simulating echo response...");
    std::thread::sleep(Duration::from_micros(5000)); // 5ms RTT
    path.receive_echo_response(sequence).unwrap();

    let stats = path.get_stats();
    println!("  ✓ Echo response received");
    println!(
        "  Echo responses received: {}",
        stats.echo_responses_received
    );
    println!("  Path state: {:?}", path.get_state());
    println!();

    // Example 4: RTT Statistics
    println!("Example 4: RTT Statistics Tracking");
    println!("─────────────────────────────────────────");

    // Simulate multiple echo exchanges
    for i in 1..=5 {
        let seq = path.send_echo();
        std::thread::sleep(Duration::from_micros(3000 + i * 500));
        path.receive_echo_response(seq).ok();
    }

    let stats = path.get_stats();
    println!("RTT Statistics (5 samples):");
    println!(
        "  Min RTT: {}µs ({:.3}ms)",
        stats.min_rtt_us,
        stats.min_rtt_us as f64 / 1000.0
    );
    println!(
        "  Avg RTT: {}µs ({:.3}ms)",
        stats.avg_rtt_us,
        stats.avg_rtt_us as f64 / 1000.0
    );
    println!(
        "  Max RTT: {}µs ({:.3}ms)",
        stats.max_rtt_us,
        stats.max_rtt_us as f64 / 1000.0
    );
    println!("  Last RTT: {}µs", stats.last_rtt_us);
    println!("  Total samples: {}", stats.rtt_samples);
    println!();

    // Example 5: Packet Loss Rate
    println!("Example 5: Packet Loss Rate");
    println!("─────────────────────────────────────────");

    let stats = path.get_stats();
    let plr = stats.packet_loss_rate();
    println!("Packet Loss Statistics:");
    println!("  Requests sent: {}", stats.echo_requests_sent);
    println!("  Responses received: {}", stats.echo_responses_received);
    println!("  Packet loss rate: {:.2}%", plr * 100.0);
    println!();

    // Example 6: Path State Machine
    println!("Example 6: Path State Machine");
    println!("─────────────────────────────────────────");

    let config_fast = PathConfig {
        suspect_threshold: Duration::from_micros(2000), // 2ms
        ..Default::default()
    };
    let path = Path::new("192.168.1.2:2152".parse().unwrap(), config_fast);

    println!("Initial state: {:?}", path.get_state());

    // Good RTT → Active
    let seq = path.send_echo();
    std::thread::sleep(Duration::from_micros(1000)); // 1ms
    path.receive_echo_response(seq).unwrap();
    println!("After good RTT (1ms): {:?}", path.get_state());

    // High RTT → Suspect
    let seq = path.send_echo();
    std::thread::sleep(Duration::from_micros(5000)); // 5ms
    path.receive_echo_response(seq).unwrap();
    println!("After high RTT (5ms): {:?}", path.get_state());
    println!();

    // Example 7: Path Manager
    println!("Example 7: Path Manager - Multi-Peer Management");
    println!("─────────────────────────────────────────");

    let mut manager = PathManager::new(PathConfig::default());

    let peers = [
        "192.168.1.1:2152",
        "192.168.1.2:2152",
        "192.168.1.3:2152",
        "192.168.1.4:2152",
    ];

    println!("Creating paths for {} peers...", peers.len());
    for peer_str in &peers {
        let peer_addr = peer_str.parse().unwrap();
        manager.get_or_create_path(peer_addr);
        println!("  ✓ Path created for {}", peer_str);
    }

    println!("\nTotal paths: {}", manager.get_path_count());
    println!();

    // Example 8: Path State Monitoring
    println!("Example 8: Path State Monitoring");
    println!("─────────────────────────────────────────");

    // Set different states for demonstration
    let path1 = manager
        .get_path("192.168.1.1:2152".parse().unwrap())
        .unwrap();
    path1.set_state(PathState::Active);

    let path2 = manager
        .get_path("192.168.1.2:2152".parse().unwrap())
        .unwrap();
    path2.set_state(PathState::Suspect);

    let path3 = manager
        .get_path("192.168.1.3:2152".parse().unwrap())
        .unwrap();
    path3.set_state(PathState::Failed);

    let (unknown, active, suspect, failed) = manager.get_path_counts();
    println!("Path State Summary:");
    println!("  Unknown: {}", unknown);
    println!("  Active: {} ✓", active);
    println!("  Suspect: {} ⚠", suspect);
    println!("  Failed: {} ✗", failed);
    println!();

    // Example 9: Healthy Path Selection
    println!("Example 9: Healthy Path Selection");
    println!("─────────────────────────────────────────");

    let healthy_paths = manager.get_healthy_paths();
    println!("Healthy paths (Active state):");
    for addr in &healthy_paths {
        println!("  ✓ {}", addr);
    }
    println!("Total healthy: {}", healthy_paths.len());
    println!();

    // Example 10: Echo Processing
    println!("Example 10: Automatic Echo Processing");
    println!("─────────────────────────────────────────");

    let config_short = PathConfig {
        echo_interval: Duration::from_millis(100),
        ..Default::default()
    };
    let mut manager = PathManager::new(config_short);

    let peer = "192.168.1.100:2152".parse().unwrap();
    manager.get_or_create_path(peer);

    println!("Waiting for echo interval...");
    std::thread::sleep(Duration::from_millis(150));

    println!("Processing echos...");
    let mut echo_count = 0;
    manager.process_echos(|addr, seq| {
        echo_count += 1;
        println!("  → Echo request to {} (seq: {})", addr, seq);
    });

    println!("Echos sent: {}", echo_count);
    println!();

    // Example 11: Path Failure Detection
    println!("Example 11: Path Failure Detection");
    println!("─────────────────────────────────────────");

    let config_quick_fail = PathConfig {
        echo_timeout: Duration::from_millis(100),
        max_echo_failures: 2,
        ..Default::default()
    };
    let path = Path::new("192.168.1.200:2152".parse().unwrap(), config_quick_fail);

    println!("Simulating echo timeouts...");
    println!("  Initial state: {:?}", path.get_state());
    println!(
        "  Consecutive failures: {}",
        path.get_consecutive_failures()
    );

    // First timeout
    path.send_echo();
    std::thread::sleep(Duration::from_millis(150));
    path.check_timeout();
    println!("\nAfter 1st timeout:");
    println!("  State: {:?}", path.get_state());
    println!(
        "  Consecutive failures: {}",
        path.get_consecutive_failures()
    );

    // Second timeout
    path.send_echo();
    std::thread::sleep(Duration::from_millis(150));
    path.check_timeout();
    println!("\nAfter 2nd timeout:");
    println!("  State: {:?}", path.get_state());
    println!(
        "  Consecutive failures: {}",
        path.get_consecutive_failures()
    );
    println!("  Is healthy: {}", path.is_healthy());
    println!();

    // Example 12: Path Recovery
    println!("Example 12: Path Recovery After Failure");
    println!("─────────────────────────────────────────");

    println!("Path before recovery:");
    println!("  State: {:?}", path.get_state());
    println!(
        "  Consecutive failures: {}",
        path.get_consecutive_failures()
    );

    println!("\nReceiving successful echo response...");
    let seq = path.send_echo();
    std::thread::sleep(Duration::from_micros(5000));
    path.receive_echo_response(seq).unwrap();

    println!("Path after recovery:");
    println!("  State: {:?}", path.get_state());
    println!(
        "  Consecutive failures: {}",
        path.get_consecutive_failures()
    );
    println!("  Is healthy: {}", path.is_healthy());
    println!();

    // Summary
    println!("=== Summary ===");
    println!("Path management provides:");
    println!("  ✓ Automatic echo request/response");
    println!("  ✓ RTT tracking (min/avg/max)");
    println!("  ✓ Path failure detection");
    println!("  ✓ State machine (unknown → active/suspect → failed)");
    println!("  ✓ Multi-peer management");
    println!("  ✓ Healthy path selection");
    println!("\n=== All Examples Complete ===");
}
