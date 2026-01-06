# rs-gtp-u

High-performance GTP-U (GPRS Tunneling Protocol - User Plane) implementation for 5G networks in Rust.

## Overview

**rs-gtp-u** is a direct port of [zig-gtp-u](../zig-gtp-u) from Zig to Rust, maintaining wire format compatibility and performance characteristics while leveraging Rust's safety guarantees.

This library implements 3GPP TS 29.281 v18.0.0 (GTP-U Protocol) for building User Plane Functions (UPF) in 5G core networks.

## Translation Status

### ✅ Phase 1: Foundation (Week 1) - **COMPLETE**

- [x] **protocol.rs** - Protocol constants and message types
  - All constants translated (GTPU_VERSION, ports, etc.)
  - MessageType enum with all variants
  - ExtensionHeaderType enum with all variants
  - Helper methods (is_valid, requires_teid, as_str, from_u8)
  - **12 tests passing** (100% coverage of protocol.zig tests)

### ✅ Phase 2: Header (Week 2) - **COMPLETE**

- [x] **header.rs** - GTP-U header encode/decode
  - HeaderFlags with bit manipulation (replaces Zig packed struct)
  - GtpuHeader with mandatory (8B) and optional (4B) fields
  - encode() and decode() methods with full validation
  - **19 tests passing** (7 new header tests + 12 from protocol)
  - Wire format validated byte-by-byte
  - Working example: `examples/header_demo.rs`

### ⏳ Phase 3: Extension & Message (Week 2-3) - **IN PROGRESS**

- [ ] extension.rs - Extension header chain (PDU Session Container, etc.)
- [ ] message.rs - GtpuMessage abstraction

### ⏳ Phase 3: Management Layer (Week 3) - **TODO**

- [ ] tunnel.rs - Tunnel state machine
- [ ] session.rs - PDU session management
- [ ] path.rs - Echo/RTT monitoring
- [ ] qos.rs - QoS flow support
- [ ] pool.rs - Memory pooling
- [ ] utils.rs - TEID generation, anti-replay

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
rs-gtp-u = { path = "../rs-gtp-u" }
```

Basic usage:

```rust
use rs_gtp_u::{MessageType, GTPU_PORT};

fn main() {
    // Check message type properties
    let gpdu = MessageType::Gpdu;
    assert!(gpdu.requires_teid());
    assert_eq!(gpdu as u8, 255);

    println!("Message: {}", gpdu);  // "G-PDU"

    // Default port
    println!("GTP-U port: {}", GTPU_PORT);  // 2152
}
```

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test module
cargo test protocol::tests

# Check test coverage
cargo test --lib
```

**Current test results:**
```
running 12 tests
test protocol::tests::test_extension_header_type_from_u8 ... ok
test protocol::tests::test_extension_header_type_display ... ok
test protocol::tests::test_extension_header_type_validation ... ok
test protocol::tests::test_extension_header_type_wire_values ... ok
test protocol::tests::test_message_type_display ... ok
test protocol::tests::test_message_type_as_str ... ok
test protocol::tests::test_message_type_from_u8 ... ok
test protocol::tests::test_message_type_validation ... ok
test protocol::tests::test_message_type_wire_values ... ok
test tests::test_basic_usage ... ok
test tests::test_re_exports ... ok
test tests::test_version_info ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured
```

## Wire Format Validation

All enum values are validated against zig-gtp-u to ensure byte-level compatibility:

| Enum | Zig Value | Rust Value | Status |
|------|-----------|------------|--------|
| `MessageType::EchoRequest` | 1 | 1 | ✅ |
| `MessageType::EchoResponse` | 2 | 2 | ✅ |
| `MessageType::ErrorIndication` | 26 | 26 | ✅ |
| `MessageType::EndMarker` | 254 | 254 | ✅ |
| `MessageType::Gpdu` | 255 | 255 | ✅ |
| `ExtensionHeaderType::PduSessionContainer` | 0x85 | 0x85 | ✅ |
| `ExtensionHeaderType::PdcpPduNumber` | 0xC0 | 0xC0 | ✅ |

## Translation Philosophy

### Zig → Rust Patterns

| Zig Pattern | Rust Equivalent | Example |
|-------------|-----------------|---------|
| `pub const CONSTANT = value;` | `pub const CONSTANT: Type = value;` | `GTPU_VERSION: u8 = 1` |
| `enum(u8) { ... }` | `#[repr(u8)] enum { ... }` | `MessageType` |
| `pub fn isValid(self: T) bool` | `pub fn is_valid(self) -> bool` | Snake case naming |
| `[]const u8` (string) | `&'static str` | Zero-cost string literals |
| `@enumFromInt(value)` | `from_u8(value) -> Option<T>` | Safe conversion |
| `test "name"` | `#[test] fn test_name()` | Unit tests |

### Key Differences

1. **Error Handling**
   - Zig: `!Type` (error union)
   - Rust: `Result<T, E>` (will be used in header.rs)

2. **Memory Management**
   - Zig: Explicit allocator parameter
   - Rust: Ownership/borrowing (no GC, no manual alloc)

3. **Enums**
   - Zig: Can have arbitrary values with `_` catchall
   - Rust: All variants must be defined, use `from_u8()` for parsing

4. **Naming Conventions**
   - Zig: `camelCase` for functions
   - Rust: `snake_case` for functions, `PascalCase` for types

## Documentation

Generate API docs:

```bash
cargo doc --open --no-deps
```

## Next Steps

See [TRANSLATION_GUIDE.md](./TRANSLATION_GUIDE.md) for the week-by-week plan.

**Week 2 Focus**: Translate `header.zig` and `extension.zig`
- Wire format encoding/decoding
- Extension header chain handling
- PDU Session Container (critical for QoS)

## Contributing

This is a learning project translating zig-gtp-u to Rust. Contributions should:
- Maintain wire format compatibility with zig-gtp-u
- Follow Rust idioms and best practices
- Include tests for all functionality
- Document translation decisions

## License

Same as zig-gtp-u: MIT License

## References

- **Source**: [zig-gtp-u](../zig-gtp-u)
- **3GPP TS 29.281**: GTP-U Protocol
- **3GPP TS 23.501**: 5G System Architecture
- **Translation Plan**: [upf-project-plan/08-revised-plan-with-gtpu.md](../upf-project-plan/08-revised-plan-with-gtpu.md)
