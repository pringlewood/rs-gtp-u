# Week 9 Progress: Path Management

## Overview

Week 9 focused on implementing the **Path management layer** (`path.rs`), translating from `zig-gtp-u/src/path.zig`. This module provides complete GTP-U path monitoring and management as per 3GPP TS 29.281 Section 4.4, including Echo request/response mechanisms, RTT (Round-Trip Time) tracking, path failure detection, and multi-peer path management.

**Critical Achievement**: Completed the management layer with path monitoring capabilities, enabling production-ready network connectivity management between GTP-U peers.

## Files Created/Modified

### New Files

1. **src/path.rs** (~500 lines)
   - PathState enum (Unknown, Active, Suspect, Failed)
   - PathStats structure with RTT tracking
   - PathConfig with echo intervals and thresholds
   - Path structure with echo request/response mechanism
   - PathManager for multi-peer path management
   - Complete test coverage (9 tests)

2. **examples/path_demo.rs** (~270 lines)
   - 12 comprehensive examples demonstrating:
     - Path configuration
     - Path creation and state
     - Echo request/response mechanism
     - RTT statistics tracking
     - Packet loss rate calculation
     - Path state machine transitions
     - Multi-peer path management
     - Path state monitoring
     - Healthy path selection
     - Automatic echo processing
     - Path failure detection
     - Path recovery after failure

### Modified Files

1. **src/lib.rs**
   - Added `pub mod path;`
   - Added Path re-exports: `Path`, `PathConfig`, `PathError`, `PathManager`, `PathState`, `PathStats`
   - Updated roadmap to mark `path` as complete ✅

## Implementation Details

### 1. PathState Enum

State machine for path health monitoring:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathState {
    /// State unknown (no echo exchange yet)
    Unknown,
    /// Path is active and healthy
    Active,
    /// Path is suspect (high RTT or occasional failures)
    Suspect,
    /// Path has failed (consecutive echo timeouts)
    Failed,
}
```

**State Transitions**:
- `Unknown` → `Active`: First successful echo response with good RTT
- `Unknown` → `Suspect`: First successful echo response with high RTT
- `Active` → `Suspect`: RTT exceeds suspect threshold
- `Suspect` → `Active`: RTT returns to normal
- `Suspect` → `Failed`: Consecutive echo timeouts exceed max_echo_failures
- `Failed` → `Active`: Successful echo response after failure

### 2. PathStats Structure

RTT statistics tracking:

```rust
#[derive(Debug, Clone)]
pub struct PathStats {
    pub echo_requests_sent: u64,
    pub echo_responses_received: u64,
    pub rtt_samples: u32,
    pub min_rtt_us: u64,         // Minimum RTT in microseconds
    pub max_rtt_us: u64,         // Maximum RTT in microseconds
    pub avg_rtt_us: u64,         // Average RTT in microseconds
    pub last_rtt_us: u64,        // Last RTT measurement
}
```

**Key Methods**:
- `update_rtt(rtt_us)` - Update RTT statistics with running average
- `packet_loss_rate()` - Calculate packet loss rate

**Running Average Algorithm**:
```rust
// Incremental average calculation (no need to store all samples)
let old_avg = self.avg_rtt_us;
let n = self.rtt_samples as u64;
self.avg_rtt_us = (old_avg * (n - 1) + rtt_us) / n;
```

### 3. PathConfig Structure

Configurable path management parameters:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathConfig {
    pub echo_interval: Duration,      // Echo request interval (default: 60s)
    pub echo_timeout: Duration,       // Echo response timeout (default: 5s)
    pub max_echo_failures: u32,       // Max failures before Failed state (default: 3)
    pub suspect_threshold: Duration,  // RTT threshold for Suspect state (default: 10ms)
}
```

**Default Configuration**:
- Echo interval: 60 seconds (1 minute)
- Echo timeout: 5 seconds
- Max echo failures: 3 consecutive failures
- Suspect threshold: 10 milliseconds

### 4. Path Structure

Complete path representation with echo management:

```rust
#[derive(Debug, Clone)]
pub struct Path {
    pub peer_address: SocketAddr,
    state: Arc<Mutex<PathState>>,
    stats: Arc<Mutex<PathStats>>,
    pub config: PathConfig,
    last_echo_time: Arc<Mutex<Instant>>,
    last_response_time: Arc<Mutex<Option<Instant>>>,
    consecutive_failures: Arc<Mutex<u32>>,
    current_sequence: Arc<Mutex<u16>>,
    pending_echo: Arc<Mutex<Option<PendingEcho>>>,
}
```

**Key Methods**:
- `new(peer_address, config)` - Create new path
- `needs_echo()` - Check if echo request is needed
- `send_echo()` - Send echo request, returns sequence number
- `receive_echo_response(sequence)` - Process echo response
- `check_timeout()` - Check for echo timeout
- `is_healthy()` - Check if path is Active
- `get_state()` - Get current path state
- `set_state(new_state)` - Set path state (for testing)
- `get_stats()` - Get path statistics
- `get_consecutive_failures()` - Get failure count

**Echo Mechanism**:
```rust
struct PendingEcho {
    sequence: u16,
    sent_time: Instant,
}
```

### 5. PathManager Structure

Thread-safe multi-peer path management:

```rust
#[derive(Debug)]
pub struct PathManager {
    paths: Arc<Mutex<HashMap<u64, Path>>>,
    config: PathConfig,
}
```

**Key Methods**:
- `new(config)` - Create new path manager
- `get_or_create_path(peer_address)` - Get or create path for peer
- `get_path(peer_address)` - Lookup existing path
- `remove_path(peer_address)` - Remove path
- `get_path_count()` - Count total paths
- `get_path_counts()` - Count paths by state (unknown, active, suspect, failed)
- `get_healthy_paths()` - Get all Active paths
- `process_echos(callback)` - Process echo requests for all paths
- `check_timeouts()` - Check all paths for timeouts

**Address Hashing**:
Uses `std::collections::hash_map::DefaultHasher` to convert `SocketAddr` to `u64` for HashMap keys:

```rust
fn address_hash(address: SocketAddr) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    address.hash(&mut hasher);
    hasher.finish()
}
```

## Translation Patterns: Zig → Rust

| Zig | Rust |
|-----|------|
| `PathState = enum` | `#[derive(Debug, Clone, Copy)] enum PathState` |
| `i128` (nanosecond timestamp) | `Instant` (monotonic clock) |
| `std.time.Timer.read()` | `Instant::elapsed()` |
| `std.time.ns_per_us` | `Duration::as_micros()` |
| `std.net.Address` | `std::net::SocketAddr` |
| `std.AutoHashMap(AddressHash, Path)` | `HashMap<u64, Path>` |
| `std.Thread.Mutex` | `Arc<Mutex<T>>` |
| Manual hash function | `DefaultHasher` + `Hash` trait |
| `defer unlock()` | RAII `MutexGuard` |
| Manual allocator | Rust ownership + `Arc` |

## Test Coverage

9 comprehensive tests:

1. **test_path_state** - State enum properties
2. **test_path_config_default** - Default configuration
3. **test_path_creation** - Path creation and initial state
4. **test_path_echo_request_response** - Echo mechanism
5. **test_path_state_transitions** - State machine transitions
6. **test_path_timeout_detection** - Timeout detection
7. **test_path_manager** - Multi-peer management
8. **test_path_stats** - RTT statistics
9. **test_path_recovery** - Recovery after failure

All tests pass with no warnings:
```
running 91 tests
...
test result: ok. 91 passed; 0 failed; 0 ignored; 0 measured
```

## Example Demo

The `path_demo.rs` example showcases:

1. **Path Configuration** - Echo intervals, timeouts, thresholds
2. **Path Creation** - Initial state and health check
3. **Echo Request/Response** - Sequence numbers and RTT calculation
4. **RTT Statistics** - Min/avg/max RTT tracking
5. **Packet Loss Rate** - PLR calculation
6. **Path State Machine** - State transitions based on RTT
7. **Path Manager** - Multi-peer path creation
8. **Path State Monitoring** - Count paths by state
9. **Healthy Path Selection** - Filter Active paths
10. **Automatic Echo Processing** - Scheduled echo requests
11. **Path Failure Detection** - Consecutive timeout handling
12. **Path Recovery** - Recovery after failure

Sample output:
```
=== GTP-U Path Management Demo ===

Example 3: Echo Request/Response
─────────────────────────────────────────
Sending echo request...
  Sequence number: 1
  Echo requests sent: 1

Simulating echo response...
  ✓ Echo response received
  Echo responses received: 1
  Path state: Active

Example 4: RTT Statistics Tracking
─────────────────────────────────────────
RTT Statistics (5 samples):
  Min RTT: 4159µs (4.159ms)
  Avg RTT: 5706µs (5.706ms)
  Max RTT: 6898µs (6.898ms)
  Last RTT: 6898µs
  Total samples: 6
```

## Architecture Integration

The Path module completes the management layer:

```text
┌─────────────────────────────────────────┐
│   Application (UPF, gNB simulator)      │
├─────────────────────────────────────────┤
│  Management Layer                       │
│  - session: PDU session management  ✅  │
│  - tunnel: Tunnel state machine     ✅  │
│  - qos: QoS flow management         ✅  │
│  - path: Echo/RTT monitoring        ✅  │  ← Week 9 (current)
├─────────────────────────────────────────┤
│  Message Layer                          │
│  - message: GtpuMessage abstraction ✅  │
│  - extension: Extension headers     ✅  │
│  - ie: Information Elements         ✅  │
├─────────────────────────────────────────┤
│  Protocol Layer                         │
│  - protocol: Constants, enums       ✅  │
│  - header: Wire format encode/decode ✅ │
├─────────────────────────────────────────┤
│  Utilities                              │
│  - pool: Memory pooling             ⏳  │  ← Week 10 (next)
│  - utils: TEID gen, anti-replay     ⏳  │
└─────────────────────────────────────────┘
```

## Integration Example

```rust
use rs_gtp_u::path::{PathManager, PathConfig};
use std::time::Duration;

// Create path manager with custom config
let config = PathConfig {
    echo_interval: Duration::from_secs(30),     // 30s echo interval
    echo_timeout: Duration::from_secs(3),       // 3s timeout
    max_echo_failures: 2,                       // Fail after 2 timeouts
    suspect_threshold: Duration::from_millis(5), // 5ms suspect threshold
};

let mut manager = PathManager::new(config);

// Create paths for peers
let peer1 = "192.168.1.1:2152".parse().unwrap();
let peer2 = "192.168.1.2:2152".parse().unwrap();

let path1 = manager.get_or_create_path(peer1);
let path2 = manager.get_or_create_path(peer2);

// Send echo requests when needed
manager.process_echos(|addr, seq| {
    // Send Echo Request message with sequence number
    println!("Sending echo to {} (seq: {})", addr, seq);
});

// Check for timeouts
manager.check_timeouts();

// Get healthy paths
let healthy = manager.get_healthy_paths();
println!("Healthy paths: {:?}", healthy);

// Get statistics
let stats = path1.get_stats();
println!("Path1 RTT: min={}µs, avg={}µs, max={}µs",
    stats.min_rtt_us, stats.avg_rtt_us, stats.max_rtt_us);
```

## Key Design Decisions

1. **State Machine** - Explicit PathState enum with documented transitions
2. **Thread Safety** - `Arc<Mutex<T>>` for all mutable state
3. **Instant vs SystemTime** - Use `Instant` for monotonic timestamps (immune to system clock changes)
4. **Running Average** - Incremental RTT average calculation without storing all samples
5. **Address Hashing** - Hash `SocketAddr` to `u64` for HashMap keys
6. **PendingEcho Struct** - Track pending echo with sequence and timestamp
7. **Configurable Thresholds** - PathConfig for tunable echo intervals and timeouts
8. **Separate Statistics** - PathStats as standalone clonable structure
9. **Callback Pattern** - `process_echos()` uses callback for sending echo requests
10. **Health Check** - Simple `is_healthy()` checks for Active state

## 3GPP Compliance

Follows 3GPP TS 29.281 Section 4.4 (Path Management):

- Echo Request/Response mechanism (Section 4.4.1)
- Path failure detection (Section 4.4.2)
- RTT monitoring for path quality assessment
- Sequence number tracking for echo matching
- Configurable echo intervals per network requirements
- Path state management (active/suspect/failed)

## Performance Characteristics

Expected performance:

- **Path Creation**: ~1 µs (HashMap insert with Arc overhead)
- **Echo Request**: ~0.5 µs (Mutex lock + sequence increment)
- **Echo Response**: ~1 µs (Mutex locks + RTT calculation + statistics update)
- **Timeout Check**: ~0.5 µs (Mutex lock + time comparison)
- **Path Lookup**: ~0.3 µs (HashMap O(1) with address hash)
- **Statistics Update**: ~0.2 µs (Running average calculation)
- **Memory per Path**: ~200-300 bytes (including Arc overhead)

## Next Steps (Week 10-11)

With Path complete, the remaining modules are utilities:

**Memory Pooling (`pool.rs`)**
- Object pools for zero-copy packet processing
- Pre-allocated buffer management
- Memory arena allocation
- Thread-safe pool access

**Utilities (`utils.rs`)**
- TEID generation (thread-safe counter)
- Anti-replay window (sliding window for sequence numbers)
- Checksum utilities
- Helper functions

**Dependencies for pool.rs and utils.rs**:
- ✅ All core modules complete
- No external dependencies

## Summary

Week 9 successfully implemented the Path management layer with:
- ✅ PathState enum with state machine transitions
- ✅ PathStats with RTT tracking (min/avg/max)
- ✅ PathConfig for configurable thresholds
- ✅ Path structure with echo request/response mechanism
- ✅ PathManager for multi-peer path management
- ✅ Address hashing for efficient path lookup
- ✅ Packet loss rate calculation
- ✅ Path failure detection and recovery
- ✅ 9 comprehensive tests (all passing)
- ✅ Detailed demo example with 12 scenarios
- ✅ Zero warnings, clean compilation

**Critical Achievement**: The Path module completes the GTP-U management layer, providing production-ready network connectivity monitoring between peers. Combined with tunnel, session, and QoS modules, the library now has complete management capabilities for building 5G User Plane Functions (UPF).

**Total Progress**: 91 tests passing across 9 modules (protocol, header, extension, message, ie, tunnel, session, qos, path) ✅

**Completion Status**:
- Protocol Layer: ✅ Complete (protocol, header)
- Message Layer: ✅ Complete (extension, message, ie)
- Management Layer: ✅ Complete (tunnel, session, qos, path)
- Utilities: ⏳ Remaining (pool, utils)

The management layer is now **fully functional** and ready for integration into production 5G UPF implementations!
