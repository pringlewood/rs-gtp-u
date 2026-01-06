//! GTP-U Utilities Demo
//!
//! This example demonstrates the utility functions and data structures:
//! - TEID (Tunnel Endpoint Identifier) generation
//! - Anti-replay window for sequence number validation
//! - IPv4 checksum calculation
//! - Rate limiting using token bucket algorithm
//! - Circular buffer for packet handling

use rs_gtp_u::utils::{
    calculate_ipv4_checksum, AntiReplayWindow, CircularBuffer, RateLimiter, TeidGenerator,
};
use std::time::Duration;

fn main() {
    println!("=== GTP-U Utilities Demo ===\n");

    // Example 1: TEID Generation
    println!("Example 1: TEID Generation");
    println!("─────────────────────────────────────────");

    let gen = TeidGenerator::new();

    println!("Generating single TEIDs:");
    for i in 1..=5 {
        let teid = gen.generate();
        println!("  TEID {}: 0x{:08X} ({})", i, teid, teid);
    }
    println!();

    // Example 2: TEID Range Generation
    println!("Example 2: TEID Range Generation");
    println!("─────────────────────────────────────────");

    let teids = gen.generate_range(10);
    println!("Generated 10 unique TEIDs:");
    for (i, teid) in teids.iter().enumerate() {
        println!("  TEID {}: 0x{:08X}", i + 1, teid);
    }
    println!();

    // Example 3: Anti-Replay Window Basics
    println!("Example 3: Anti-Replay Window - Basic Usage");
    println!("─────────────────────────────────────────");

    let window = AntiReplayWindow::new(64);

    println!("Window size: 64 packets");
    println!("\nSequence number validation:");

    // Accept first packet
    let seq1 = window.check(100);
    println!("  Sequence 100 (first): {} ✓", if seq1 { "Accept" } else { "Reject" });

    // Reject duplicate
    let seq1_dup = window.check(100);
    println!(
        "  Sequence 100 (duplicate): {} ✗",
        if seq1_dup { "Accept" } else { "Reject" }
    );

    // Accept newer packet
    let seq2 = window.check(101);
    println!("  Sequence 101 (newer): {} ✓", if seq2 { "Accept" } else { "Reject" });

    let seq3 = window.check(102);
    println!("  Sequence 102 (newer): {} ✓", if seq3 { "Accept" } else { "Reject" });

    println!("\nHighest sequence seen: {}", window.highest_sequence());
    println!();

    // Example 4: Anti-Replay Window - Out-of-Order Packets
    println!("Example 4: Anti-Replay Window - Out-of-Order Packets");
    println!("─────────────────────────────────────────");

    let window2 = AntiReplayWindow::new(128);

    println!("Simulating out-of-order packet arrival:\n");

    // Packets arrive: 100, 102, 101 (out of order)
    let sequences = [100, 102, 101, 103, 99, 104];
    for seq in sequences {
        let result = window2.check(seq);
        println!(
            "  Sequence {}: {}",
            seq,
            if result {
                "✓ Accepted"
            } else {
                "✗ Rejected (duplicate or too old)"
            }
        );
    }

    println!("\nHighest sequence: {}", window2.highest_sequence());
    println!();

    // Example 5: IPv4 Checksum Calculation
    println!("Example 5: IPv4 Checksum Calculation");
    println!("─────────────────────────────────────────");

    // Sample IPv4 header (20 bytes, checksum field zeroed)
    let ipv4_header = [
        0x45, 0x00, // Version, IHL, DSCP, ECN
        0x00, 0x3c, // Total length (60 bytes)
        0x1c, 0x46, // Identification
        0x40, 0x00, // Flags, Fragment offset
        0x40, 0x06, // TTL (64), Protocol (TCP)
        0x00, 0x00, // Checksum (to be calculated)
        0xac, 0x10, 0x0a, 0x63, // Source IP: 172.16.10.99
        0xac, 0x10, 0x0a, 0x0c, // Dest IP: 172.16.10.12
    ];

    let checksum = calculate_ipv4_checksum(&ipv4_header);
    println!("IPv4 Header:");
    println!("  Source IP: 172.16.10.99");
    println!("  Dest IP: 172.16.10.12");
    println!("  Calculated checksum: 0x{:04X}", checksum);
    println!();

    // Example 6: Rate Limiter - Token Bucket
    println!("Example 6: Rate Limiter - Token Bucket");
    println!("─────────────────────────────────────────");

    let limiter = RateLimiter::new(100, 10); // 100 capacity, 10 tokens/sec

    println!("Rate limiter configuration:");
    println!("  Capacity: 100 tokens");
    println!("  Refill rate: 10 tokens/second");
    println!("\nToken consumption:");

    // Try to consume tokens
    let operations = [
        ("Operation 1", 30),
        ("Operation 2", 40),
        ("Operation 3", 30),
        ("Operation 4", 20), // Should fail (insufficient tokens)
    ];

    for (name, tokens) in operations {
        let result = limiter.try_consume(tokens);
        let available = limiter.get_available_tokens();
        println!(
            "  {} ({} tokens): {} (Available: {})",
            name,
            tokens,
            if result {
                "✓ Consumed"
            } else {
                "✗ Rejected"
            },
            available
        );
    }

    // Wait for refill
    println!("\nWaiting 2 seconds for token refill...");
    std::thread::sleep(Duration::from_secs(2));

    let available_after = limiter.get_available_tokens();
    println!("Available tokens after refill: {}", available_after);
    println!();

    // Example 7: Circular Buffer
    println!("Example 7: Circular Buffer");
    println!("─────────────────────────────────────────");

    let buffer = CircularBuffer::<u32>::new(4);

    println!("Buffer capacity: {}", buffer.capacity());
    println!("Initial state: empty = {}\n", buffer.is_empty());

    println!("Pushing items:");
    for i in 1..=3 {
        buffer.push(i * 10).unwrap();
        println!("  Pushed {} (length: {})", i * 10, buffer.len());
    }

    println!("\nPopping items:");
    while let Some(item) = buffer.pop() {
        println!("  Popped {} (remaining: {})", item, buffer.len());
    }

    println!("\nBuffer is empty: {}", buffer.is_empty());
    println!();

    // Example 8: Circular Buffer - Wraparound
    println!("Example 8: Circular Buffer - Wraparound");
    println!("─────────────────────────────────────────");

    let buffer2 = CircularBuffer::<String>::new(4);

    println!("Demonstrating circular wraparound:\n");

    // Fill buffer
    println!("Filling buffer:");
    for i in 1..=4 {
        buffer2.push(format!("Packet-{}", i)).unwrap();
        println!("  Pushed Packet-{}", i);
    }

    println!("\nBuffer is full: {}", buffer2.is_full());

    // Try to push when full
    let result = buffer2.push("Packet-5".to_string());
    println!(
        "Trying to push when full: {}",
        if result.is_err() {
            "✗ Failed (buffer full)"
        } else {
            "✓ Success"
        }
    );

    // Pop two items
    println!("\nPopping 2 items:");
    buffer2.pop();
    buffer2.pop();

    // Push two more (wraparound)
    println!("Pushing 2 more items (wraparound):");
    buffer2.push("Packet-5".to_string()).unwrap();
    buffer2.push("Packet-6".to_string()).unwrap();

    println!("\nFinal buffer contents:");
    while let Some(packet) = buffer2.pop() {
        println!("  {}", packet);
    }
    println!();

    // Example 9: Practical Use Case - Tunnel Management
    println!("Example 9: Practical Use Case - Tunnel Management");
    println!("─────────────────────────────────────────");

    println!("Simulating GTP-U tunnel creation:\n");

    // Generate TEIDs for 5 tunnels
    let tunnel_teids = gen.generate_range(5);

    for (i, teid) in tunnel_teids.iter().enumerate() {
        println!("Tunnel {}:", i + 1);
        println!("  Local TEID: 0x{:08X}", teid);
        println!("  Anti-replay window: 128 packets");
        println!("  Rate limit: 1000 packets/sec");
        println!();
    }

    // Example 10: Performance Characteristics
    println!("Example 10: Performance Characteristics");
    println!("─────────────────────────────────────────");

    println!("Generating 1000 TEIDs...");
    let start = std::time::Instant::now();
    let _many_teids = gen.generate_range(1000);
    let elapsed = start.elapsed();
    println!("  Time: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    println!(
        "  Rate: {:.0} TEIDs/sec",
        1000.0 / elapsed.as_secs_f64()
    );

    println!("\nAnti-replay window checks (10000 sequences)...");
    let window3 = AntiReplayWindow::new(128);
    let start = std::time::Instant::now();
    for i in 0..10000 {
        window3.check(i);
    }
    let elapsed = start.elapsed();
    println!("  Time: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    println!(
        "  Rate: {:.0} checks/sec",
        10000.0 / elapsed.as_secs_f64()
    );

    println!("\nCircular buffer operations (10000 push/pop)...");
    let buffer3 = CircularBuffer::<u32>::new(1000);
    let start = std::time::Instant::now();
    for i in 0..5000 {
        buffer3.push(i).ok();
        buffer3.pop();
    }
    let elapsed = start.elapsed();
    println!("  Time: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    println!(
        "  Rate: {:.0} ops/sec",
        10000.0 / elapsed.as_secs_f64()
    );
    println!();

    // Summary
    println!("=== Summary ===");
    println!("Utilities provide:");
    println!("  ✓ Cryptographic TEID generation");
    println!("  ✓ Anti-replay protection with sliding window");
    println!("  ✓ IPv4 checksum calculation");
    println!("  ✓ Token bucket rate limiting");
    println!("  ✓ Circular buffer for packet queuing");
    println!("\n=== All Examples Complete ===");
}
