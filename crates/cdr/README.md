# `zerodds-cdr`

XCDR1/XCDR2 encoder/decoder, endianness, alignment.
Part of [**ZeroDDS**](../../README.md). Safety class **SAFE** —
`forbid(unsafe_code)`, no_std + alloc.

---

## Quick Start

```rust
use zerodds_cdr::{BufferWriter, BufferReader, Endianness, CdrEncode, CdrDecode};

// Encoder
let mut w = BufferWriter::new(Endianness::Little);
42u32.encode(&mut w)?;
"hello".to_string().encode(&mut w)?;
let bytes: Vec<u8> = w.into_bytes();

// Decoder
let mut r = BufferReader::new(&bytes, Endianness::Little);
let n: u32 = u32::decode(&mut r)?;
let s: String = String::decode(&mut r)?;
assert_eq!((n, &*s), (42, "hello"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Structs with Extensibility

```rust
use zerodds_cdr::struct_enc::{encode_appendable, decode_appendable, encode_mutable_member,
                          read_all_mutable_members, encode_final};
use zerodds_cdr::{BufferWriter, BufferReader, Endianness, CdrEncode, CdrDecode};

let mut w = BufferWriter::new(Endianness::Little);

// @final: tight-packed, no header
encode_final(&mut w, |w| { 1u32.encode(w)?; 2u8.encode(w) })?;

// @appendable: 4-byte DHEADER, forward-compatible
encode_appendable(&mut w, |w| { 100u32.encode(w)?; 200u8.encode(w) })?;

// @mutable: one EMHEADER with member ID per member
encode_mutable_member(&mut w, /*member_id=*/ 1, /*must_understand=*/ false,
    |w| 999u32.encode(w))?;
# Ok::<(), zerodds_cdr::EncodeError>(())
```

---

## Architecture

```text
┌────────────┐  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐
│ T: CdrEnc. │─▶│ BufferWriter│─▶│ Vec<u8>      │─▶│ Wire (UDP/    │
│ (typed val)│  │ (alignment, │  │ (bytes)      │  │  Shared-Mem)  │
│            │  │  endianness)│  │              │  │               │
└────────────┘  └─────────────┘  └──────────────┘  └───────────────┘
                                                            │
                                                            ▼
┌────────────┐  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐
│ T: CdrDec. │◀─│ BufferReader│◀─│ &[u8]        │◀─│ Wire input    │
│ (typed val)│  │             │  │              │  │               │
└────────────┘  └─────────────┘  └──────────────┘  └───────────────┘
```

### Modules

| Module | Purpose | Status |
|---|---|---|
| `error` | `EncodeError`, `DecodeError` | stable |
| `endianness` | `Endianness::{Big, Little}` + conversion helpers | stable |
| `buffer` | `BufferWriter` (alloc) + `BufferReader` with alignment tracking | stable |
| `encode` | `CdrEncode`/`CdrDecode` traits + primitive impls | stable |
| `composite` (alloc) | `String`/`Vec<T>`/`[T;N]`/`Option<T>` | stable |
| `struct_enc` (alloc) | `@final`/`@appendable`/`@mutable` helpers | stable |

---

## Wire-Format Conformance

OMG XTypes 1.3 §7.4 (CDR Encoding Rules):

- **Primitives** (§7.4.1): alignment relative to stream start, BE/LE per
  stream encapsulation
- **String** (§7.4.4): `uint32` length incl. null terminator + UTF-8 + `\0`
- **Sequence** (§7.4.4.2): `uint32` element count + elements
- **Array** (§7.4.4.3): N elements without length prefix
- **Optional** (§7.4.5.1.4): `uint8` present flag + value
- **`@final` struct** (§7.4.5.1.1): tight-packed
- **`@appendable` struct** (§7.4.3.4.5): DHEADER (uint32 = body length)
- **`@mutable` struct** (§7.4.3.4.2): EMHEADER (m-bit + LC + 28-bit ID)
  + NEXTINT

## Coverage

- **XCDR1 (CDR_BE / CDR_LE)** and **XCDR2** — both
  encapsulation schemes complete. XCDR1 for legacy vendor compat
  (RTI Connext, Cyclone DDS default for small types).
- **EMHEADER length codes**: LC0..3 + LC4 + LC5..7 — all 8 codes
  produce byte-identical output to the OMG XTypes 1.3 spec.
- **Type-Object encoding** (XTypes §7.3) — separate crate
  `zerodds-types` (TypeIdentifier, TypeObject, TypeMap, KeyHash).
- **Code-gen**: `zerodds-idlc` emits a CDR encoder + decoder per IDL
  type; manual calls to the helper functions remain possible.

## Tests

```bash
cargo test -p zerodds-cdr                        # 84 lib + 7 integration
cargo test -p zerodds-cdr --no-default-features --features alloc  # no_std + alloc
```

---

## License

See the workspace `Cargo.toml`.
