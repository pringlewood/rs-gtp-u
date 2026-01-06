# Zig to Rust Translation Guide

This document describes the systematic process of translating zig-gtp-u to rs-gtp-u.

## Translation Methodology

### Step-by-Step Process

For each Zig source file:

1. **Read & Understand** - Study the Zig code completely
2. **Map Patterns** - Identify Zig patterns and their Rust equivalents
3. **Translate** - Convert to idiomatic Rust
4. **Port Tests** - Translate all test blocks
5. **Validate** - Ensure wire format compatibility
6. **Document** - Add translation notes

## Pattern Translation Reference

### Constants

**Zig:**
```zig
pub const GTPU_VERSION = 1;
pub const GTPU_PORT = 2152;
```

**Rust:**
```rust
pub const GTPU_VERSION: u8 = 1;
pub const GTPU_PORT: u16 = 2152;
```

**Notes:**
- Rust requires explicit type annotations for clarity
- Both are compile-time constants (zero runtime cost)

### Enums

**Zig:**
```zig
pub const MessageType = enum(u8) {
    echo_request = 1,
    echo_response = 2,
    g_pdu = 255,

    _,  // Allow unknown values

    pub fn isValid(self: MessageType) bool {
        return switch (self) {
            .echo_request, .echo_response, .g_pdu => true,
            else => false,
        };
    }
};
```

**Rust:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageType {
    EchoRequest = 1,
    EchoResponse = 2,
    Gpdu = 255,
}

impl MessageType {
    pub fn is_valid(self) -> bool {
        matches!(
            self,
            MessageType::EchoRequest
                | MessageType::EchoResponse
                | MessageType::Gpdu
        )
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(MessageType::EchoRequest),
            2 => Some(MessageType::EchoResponse),
            255 => Some(MessageType::Gpdu),
            _ => None,
        }
    }
}
```

**Notes:**
- `#[repr(u8)]` ensures same memory layout as Zig's `enum(u8)`
- Rust enums are closed (no `_` catchall)
- Add `from_u8()` for safe parsing of wire format
- `#[derive(...)]` adds common traits automatically
- Use `PascalCase` for variants (Rust convention)

### String Literals

**Zig:**
```zig
pub fn toString(self: MessageType) []const u8 {
    return switch (self) {
        .echo_request => "Echo Request",
        .g_pdu => "G-PDU",
        else => "Unknown",
    };
}
```

**Rust:**
```rust
pub fn as_str(self) -> &'static str {
    match self {
        MessageType::EchoRequest => "Echo Request",
        MessageType::Gpdu => "G-PDU",
    }
}
```

**Notes:**
- `[]const u8` → `&'static str`
- Both are zero-cost (point to static data)
- No "Unknown" case needed (exhaustive enum)
- Use `snake_case` for method names

### Tests

**Zig:**
```zig
test "MessageType validation" {
    try std.testing.expect(MessageType.echo_request.isValid());
    try std.testing.expect(MessageType.g_pdu.isValid());
}
```

**Rust:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_validation() {
        assert!(MessageType::EchoRequest.is_valid());
        assert!(MessageType::Gpdu.is_valid());
    }
}
```

**Notes:**
- Zig: Top-level `test "name"` blocks
- Rust: `#[cfg(test)] mod tests` with `#[test]` functions
- `try expect()` → `assert!()`
- Import parent scope with `use super::*`

## Module Translation Order

Based on dependency analysis:

```
Week 1: ✅ COMPLETE
├── protocol.rs (no dependencies)

Week 2: 🔄 IN PROGRESS
├── header.rs (depends: protocol)
├── extension.rs (depends: protocol)
└── message.rs (depends: header, extension)

Week 3:
├── tunnel.rs (depends: message, utils)
├── session.rs (depends: tunnel)
├── path.rs (depends: message)
├── qos.rs (depends: session)
└── pool.rs (standalone)
```

## Week 1 Retrospective: protocol.rs

### What Went Well

✅ **Wire Format Validation**
- All enum values match exactly (1:1 compatibility)
- Tests verify this automatically

✅ **Idiomatic Rust**
- Used `#[derive]` traits for common functionality
- Implemented `Display` trait for nice printing
- `from_u8()` provides safe parsing

✅ **Test Coverage**
- All Zig tests ported (12 tests)
- Added extra tests for wire format validation
- 100% pass rate on first try

### Lessons Learned

📝 **Enum Translation**
- Zig's `_` catchall requires explicit `from_u8()` in Rust
- Rust's exhaustive matching is safer
- Use `matches!()` macro for cleaner validation

📝 **Naming Conventions**
- Zig: `isValid()`, `toString()`
- Rust: `is_valid()`, `as_str()`
- Be consistent with Rust conventions

📝 **Documentation**
- Use `///` doc comments for public items
- Include "Translation Note" sections
- Reference 3GPP spec sections

## Week 2 Preview: header.rs

### Translation Challenges

**Zig Header Structure:**
```zig
pub const GtpuHeader = struct {
    version: u3,
    pt_flag: bool,
    e_flag: bool,
    s_flag: bool,
    pn_flag: bool,
    message_type: MessageType,
    length: u16,
    teid: u32,
    sequence_number: ?u16,  // Optional
    n_pdu_number: ?u8,      // Optional
    next_extension_type: ?ExtensionHeaderType,

    pub fn encode(self: *const GtpuHeader, writer: anytype) !void {
        // Bit manipulation for flags
        var flags_byte: u8 = @as(u8, self.version) << 5;
        flags_byte |= if (self.pt_flag) 0x10 else 0;
        // ... etc

        try writer.writeByte(flags_byte);
        try writer.writeByte(@enumToInt(self.message_type));
        try writer.writeInt(u16, self.length, .big);
        try writer.writeInt(u32, self.teid, .big);
    }
};
```

**Rust Approach:**

1. **Flags as bitflags:**
```rust
use bitflags::bitflags;

bitflags! {
    pub struct HeaderFlags: u8 {
        const PN = 0b0000_0001;  // N-PDU Number present
        const S  = 0b0000_0010;  // Sequence Number present
        const E  = 0b0000_0100;  // Extension Header present
        const PT = 0b0001_0000;  // Protocol Type (always set)
    }
}
```

2. **Header structure:**
```rust
#[derive(Debug, Clone)]
pub struct GtpuHeader {
    pub version: u8,
    pub flags: HeaderFlags,
    pub message_type: MessageType,
    pub length: u16,
    pub teid: u32,
    pub sequence_number: Option<u16>,
    pub n_pdu_number: Option<u8>,
    pub next_extension_type: Option<ExtensionHeaderType>,
}
```

3. **Encoding:**
```rust
impl GtpuHeader {
    pub fn encode(&self, buf: &mut Vec<u8>) -> Result<usize, Error> {
        let mut flags_byte = (self.version << 5) | 0x10;  // PT always set

        if self.sequence_number.is_some() {
            flags_byte |= HeaderFlags::S.bits();
        }

        buf.push(flags_byte);
        buf.push(self.message_type as u8);
        buf.extend_from_slice(&self.length.to_be_bytes());
        buf.extend_from_slice(&self.teid.to_be_bytes());

        // Optional fields...

        Ok(buf.len())
    }
}
```

### Key Translation Points

1. **Optional Fields**
   - Zig: `?u16` (nullable)
   - Rust: `Option<u16>`
   - Both have same semantics

2. **Bit Manipulation**
   - Zig: `@as(u8, value) << 5`
   - Rust: `(value as u8) << 5`
   - Prefer explicit casts in Rust

3. **Endianness**
   - Zig: `.big` parameter to `writeInt()`
   - Rust: `to_be_bytes()` method
   - Both ensure network byte order

4. **Error Handling**
   - Zig: `!void` (error union)
   - Rust: `Result<(), Error>`
   - Will define custom `Error` type

## Validation Strategy

### Cross-Language Testing

Create test data in Zig, validate in Rust:

**Zig test generator:**
```zig
test "generate test vectors" {
    const header = GtpuHeader{
        .version = 1,
        .flags = .{ .s = true, .e = true },
        .message_type = .g_pdu,
        .length = 100,
        .teid = 0x12345678,
        .sequence_number = 42,
    };

    var buf: [256]u8 = undefined;
    var fbs = std.io.fixedBufferStream(&buf);
    try header.encode(fbs.writer());

    // Print hex for Rust test
    std.debug.print("Test vector: ", .{});
    for (fbs.getWritten()) |byte| {
        std.debug.print("0x{x:0>2}, ", .{byte});
    }
}
```

**Rust test validator:**
```rust
#[test]
fn test_header_zig_compatibility() {
    // Hex from Zig test generator
    let zig_bytes = [
        0x32, 0xFF, 0x00, 0x64,  // flags, type, length
        0x12, 0x34, 0x56, 0x78,  // TEID
        0x00, 0x2A,              // sequence
    ];

    let header = GtpuHeader::decode(&zig_bytes).unwrap();
    assert_eq!(header.teid, 0x12345678);
    assert_eq!(header.sequence_number, Some(42));

    // Re-encode and compare
    let mut rust_bytes = Vec::new();
    header.encode(&mut rust_bytes).unwrap();

    assert_eq!(rust_bytes.as_slice(), &zig_bytes);
}
```

## Tools & Scripts

### compare_encodings.sh

```bash
#!/bin/bash
# Compare Zig and Rust encodings

echo "=== Zig Encoding ==="
cd ../zig-gtp-u
zig run examples/encode_header.zig | xxd

echo ""
echo "=== Rust Encoding ==="
cd ../rs-gtp-u
cargo run --example encode_header | xxd

echo ""
echo "=== Diff ==="
diff <(cd ../zig-gtp-u && zig run examples/encode_header.zig | xxd) \
     <(cargo run --example encode_header | xxd)
```

## Checklist for Each Module

Before marking a module complete:

- [ ] All Zig functions/methods translated
- [ ] All Zig tests ported to Rust
- [ ] Wire format tests pass
- [ ] Cross-language validation passes
- [ ] Documentation complete (with translation notes)
- [ ] Examples compile and run
- [ ] `cargo clippy` has no warnings
- [ ] `cargo fmt` applied

## Resources

- **Zig Language Reference**: https://ziglang.org/documentation/master/
- **Rust Book**: https://doc.rust-lang.org/book/
- **3GPP TS 29.281**: GTP-U specification
- **zig-gtp-u Source**: ../zig-gtp-u/src/

## Questions & Decisions Log

### Q: Should we use `unsafe` for performance?

**A:** No, unless absolutely necessary and well-justified.
- Zig's manual memory management → Rust's ownership
- Zero-copy patterns possible without `unsafe`
- Use `unsafe` only for FFI or proven bottlenecks

### Q: How to handle Zig's `anytype` parameters?

**A:** Use Rust generics with trait bounds.
- Zig: `fn foo(writer: anytype) !void`
- Rust: `fn foo<W: Write>(writer: &mut W) -> Result<()>`

### Q: What about Zig's comptime?

**A:** Use Rust's const generics or macros.
- Most comptime can be done at build time
- Procedural macros for complex code generation
- Const generics for type-level parameters

## Next Steps

1. **Week 2**: Translate header.rs and extension.rs
2. Create example programs (encode_header, decode_message)
3. Set up benchmark comparisons with zig-gtp-u
4. Document performance characteristics

---

**Status**: Week 1 Complete ✅ | Week 2 Starting 🔄
