//! Memory Pool Demo
//!
//! This example demonstrates memory pooling and zero-copy packet processing:
//! - Generic memory pools for fixed-size allocations
//! - Packet buffer pools with reference counting
//! - Batch processing for improved performance
//! - Message batching with QFI/TEID grouping
//! - Statistics tracking

use rs_gtp_u::pool::{BatchProcessor, BatchStats, MessageBatch, PacketBuffer, PacketBufferPool};

fn main() {
    println!("=== Memory Pool Demo ===\n");

    // Example 1: Packet Buffer Pool
    println!("Example 1: Packet Buffer Pool");
    println!("─────────────────────────────────────────");

    let pool = PacketBufferPool::new(10, 2048);

    println!("Pool Configuration:");
    println!("  Capacity: {} buffers", pool.capacity());
    println!("  Buffer size: {} bytes", pool.buffer_size());
    println!("  Available: {}", pool.available());
    println!();

    // Example 2: Acquire and Release
    println!("Example 2: Buffer Acquire/Release");
    println!("─────────────────────────────────────────");

    println!("Acquiring buffers:");
    let buf1 = pool.acquire().unwrap();
    println!("  ✓ Buffer 1 acquired (available: {})", pool.available());

    let buf2 = pool.acquire().unwrap();
    println!("  ✓ Buffer 2 acquired (available: {})", pool.available());

    // Use the buffers
    buf1.set_length(100);
    println!("\nUsing buffers:");
    println!(
        "  Buffer 1: length={}, capacity={}",
        buf1.length(),
        buf1.capacity()
    );

    buf2.set_length(200);
    println!(
        "  Buffer 2: length={}, capacity={}",
        buf2.length(),
        buf2.capacity()
    );

    // Release buffers
    println!("\nReleasing buffers:");
    drop(buf1);
    println!("  ✓ Buffer 1 released (available: {})", pool.available());

    drop(buf2);
    println!("  ✓ Buffer 2 released (available: {})", pool.available());
    println!();

    // Example 3: Reference Counting
    println!("Example 3: Reference Counting");
    println!("─────────────────────────────────────────");

    let buf = pool.acquire().unwrap();
    println!("Buffer acquired:");
    println!("  Reference count: {}", buf.ref_count());
    println!("  Pool available: {}", pool.available());

    // Add reference
    buf.add_ref();
    println!("\nAfter add_ref():");
    println!("  Reference count: {}", buf.ref_count());

    // First drop decrements ref count
    drop(buf);
    println!("\nAfter first drop:");
    println!(
        "  Pool available: {} (buffer not returned yet)",
        pool.available()
    );

    println!("\n(Buffer will be returned when all references are dropped)");
    println!();

    // Example 4: Zero-Copy Usage
    println!("Example 4: Zero-Copy Buffer Usage");
    println!("─────────────────────────────────────────");

    let mut buf = pool.acquire().unwrap();
    println!("Writing data to buffer...");

    // Simulate writing GTP-U header
    {
        let data = buf.get_data_mut();
        data[0] = 0x30; // GTP-U version 1, no extension
        data[1] = 0xFF; // Message type: G-PDU
        data[2] = 0x00; // Length high byte
        data[3] = 0x10; // Length low byte (16 bytes)
    }
    buf.set_length(4);

    println!("  ✓ Wrote {} bytes", buf.length());
    println!("  Data: {:02X?}", &buf.get_data()[..4]);
    println!();

    drop(buf);

    // Example 5: Batch Processing
    println!("Example 5: Batch Processing");
    println!("─────────────────────────────────────────");

    let processor = BatchProcessor::new(5);
    println!("Batch processor created (max size: 5)");

    println!("\nAdding packets to batch:");
    processor.add(1);
    println!("  Added packet index 1 (size: {})", processor.size());

    processor.add(2);
    println!("  Added packet index 2 (size: {})", processor.size());

    processor.add(3);
    println!("  Added packet index 3 (size: {})", processor.size());

    println!("\nProcessing batch:");
    let mut count = 0;
    processor.process(|index| {
        count += 1;
        println!("  Processing packet index {}", index);
    });

    println!("\nProcessed {} packets", count);
    println!("Batch size after processing: {}", processor.size());
    println!();

    // Example 6: Message Batch
    println!("Example 6: Message Batching");
    println!("─────────────────────────────────────────");

    let mut batch = MessageBatch::new();

    println!("Adding messages to batch:");
    batch.add_encoded(&[0x30, 0xFF, 0x00, 0x10], 0x12345678, Some(9));
    println!("  ✓ Message 1: TEID=0x12345678, QFI=9");

    batch.add_encoded(&[0x30, 0xFF, 0x00, 0x08], 0x87654321, Some(5));
    println!("  ✓ Message 2: TEID=0x87654321, QFI=5");

    batch.add_encoded(&[0x30, 0xFF, 0x00, 0x04], 0x12345678, Some(9));
    println!("  ✓ Message 3: TEID=0x12345678, QFI=9");

    batch.add_encoded(&[0x30, 0xFF, 0x00, 0x02], 0xAAAAAAAA, None);
    println!("  ✓ Message 4: TEID=0xAAAAAAAA, QFI=None");

    println!("\nBatch statistics:");
    println!("  Message count: {}", batch.count());
    println!("  Total size: {} bytes", batch.total_size());
    println!();

    // Example 7: Group by QFI
    println!("Example 7: Grouping Messages by QFI");
    println!("─────────────────────────────────────────");

    let qfi_groups = batch.group_by_qfi();
    println!("Grouped {} messages by QFI:\n", batch.count());

    for (qfi, messages) in &qfi_groups {
        println!("  QFI {}: {} messages", qfi, messages.len());
        for (i, msg) in messages.iter().enumerate() {
            println!(
                "    Message {}: TEID=0x{:08X}, {} bytes",
                i + 1,
                msg.teid,
                msg.data.len()
            );
        }
    }

    println!("\n(Messages without QFI are not grouped)");
    println!();

    // Example 8: Group by TEID
    println!("Example 8: Grouping Messages by TEID");
    println!("─────────────────────────────────────────");

    let teid_groups = batch.group_by_teid();
    println!("Grouped {} messages by TEID:\n", batch.count());

    for (teid, messages) in &teid_groups {
        println!("  TEID 0x{:08X}: {} messages", teid, messages.len());
        for (i, msg) in messages.iter().enumerate() {
            let qfi_str = match msg.qfi {
                Some(q) => format!("QFI={}", q),
                None => "QFI=None".to_string(),
            };
            println!(
                "    Message {}: {}, {} bytes",
                i + 1,
                qfi_str,
                msg.data.len()
            );
        }
    }
    println!();

    // Example 9: Batch Statistics
    println!("Example 9: Batch Statistics");
    println!("─────────────────────────────────────────");

    let mut stats = BatchStats::new();

    println!("Recording batches...");
    stats.record_batch(&batch);

    // Create another batch
    let mut batch2 = MessageBatch::new();
    batch2.add_encoded(&[0x30, 0xFF, 0x00, 0x20], 0x11111111, Some(1));
    batch2.add_encoded(&[0x30, 0xFF, 0x00, 0x20], 0x22222222, Some(9));
    stats.record_batch(&batch2);

    println!("\nStatistics:");
    println!("  Batches processed: {}", stats.batches_processed);
    println!("  Messages processed: {}", stats.messages_processed);
    println!("  Bytes processed: {}", stats.bytes_processed);
    println!(
        "  Avg messages/batch: {:.2}",
        stats.avg_messages_per_batch()
    );
    println!("  Avg bytes/message: {:.2}", stats.avg_bytes_per_message());

    println!("\nQFI Distribution:");
    for (qfi, count) in stats.qfi_distribution.iter().enumerate() {
        if *count > 0 {
            println!("  QFI {}: {} messages", qfi, count);
        }
    }
    println!();

    // Example 10: Performance Test
    println!("Example 10: Performance Characteristics");
    println!("─────────────────────────────────────────");

    let test_pool = PacketBufferPool::new(1000, 2048);

    println!("Buffer acquire/release (10000 iterations)...");
    let start = std::time::Instant::now();
    for _ in 0..10000 {
        let buf = test_pool.acquire().unwrap();
        drop(buf);
    }
    let elapsed = start.elapsed();
    println!("  Time: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    println!("  Rate: {:.0} ops/sec", 10000.0 / elapsed.as_secs_f64());

    println!("\nMessage batching (10000 messages)...");
    let mut perf_batch = MessageBatch::new();
    let start = std::time::Instant::now();
    for i in 0..10000 {
        perf_batch.add_encoded(&[0x30, 0xFF, 0x00, 0x10], i as u32, Some((i % 10) as u8));
    }
    let elapsed = start.elapsed();
    println!("  Time: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    println!(
        "  Rate: {:.0} messages/sec",
        10000.0 / elapsed.as_secs_f64()
    );

    println!("\nGrouping by QFI (10000 messages)...");
    let start = std::time::Instant::now();
    let _groups = perf_batch.group_by_qfi();
    let elapsed = start.elapsed();
    println!("  Time: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    println!(
        "  Rate: {:.0} messages/sec",
        10000.0 / elapsed.as_secs_f64()
    );
    println!();

    // Example 11: Memory Usage
    println!("Example 11: Memory Usage");
    println!("─────────────────────────────────────────");

    println!(
        "PacketBuffer size: {} bytes",
        std::mem::size_of::<PacketBuffer>()
    );
    println!(
        "MessageBatch overhead: {} bytes",
        std::mem::size_of::<MessageBatch>()
    );

    let pool_usage = test_pool.capacity() * test_pool.buffer_size();
    println!("\nPool memory usage:");
    println!(
        "  Buffers: {} × {} bytes",
        test_pool.capacity(),
        test_pool.buffer_size()
    );
    println!(
        "  Total: {} KB ({} MB)",
        pool_usage / 1024,
        pool_usage / (1024 * 1024)
    );
    println!();

    // Summary
    println!("=== Summary ===");
    println!("Memory pooling provides:");
    println!("  ✓ Zero-copy packet processing");
    println!("  ✓ Reference counted buffer management");
    println!("  ✓ Batch processing for performance");
    println!("  ✓ Message grouping by QFI/TEID");
    println!("  ✓ Thread-safe operations");
    println!("  ✓ Automatic memory reuse");
    println!("\n=== All Examples Complete ===");
}
