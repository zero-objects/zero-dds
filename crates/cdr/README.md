# `zerodds-cdr`

XCDR1/XCDR2 Encoder/Decoder, Endianness, Alignment.
Teil von [**ZeroDDS**](../../README.md). Safety-Klasse **SAFE** —
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

## Strukturen mit Extensibility

```rust
use zerodds_cdr::struct_enc::{encode_appendable, decode_appendable, encode_mutable_member,
                          read_all_mutable_members, encode_final};
use zerodds_cdr::{BufferWriter, BufferReader, Endianness, CdrEncode, CdrDecode};

let mut w = BufferWriter::new(Endianness::Little);

// @final: tight-packed, kein Header
encode_final(&mut w, |w| { 1u32.encode(w)?; 2u8.encode(w) })?;

// @appendable: 4-byte DHEADER, forward-kompatibel
encode_appendable(&mut w, |w| { 100u32.encode(w)?; 200u8.encode(w) })?;

// @mutable: pro Member ein EMHEADER mit Member-ID
encode_mutable_member(&mut w, /*member_id=*/ 1, /*must_understand=*/ false,
    |w| 999u32.encode(w))?;
# Ok::<(), zerodds_cdr::EncodeError>(())
```

---

## Architektur

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

### Module

| Modul | Zweck | Status |
|---|---|---|
| `error` | `EncodeError`, `DecodeError` | stabil |
| `endianness` | `Endianness::{Big, Little}` + Konvertierungs-Helpers | stabil |
| `buffer` | `BufferWriter` (alloc) + `BufferReader` mit Alignment-Tracking | stabil |
| `encode` | `CdrEncode`/`CdrDecode` Traits + Primitive-Impls | stabil |
| `composite` (alloc) | `String`/`Vec<T>`/`[T;N]`/`Option<T>` | stabil |
| `struct_enc` (alloc) | `@final`/`@appendable`/`@mutable` Helpers | stabil |

---

## Wire-Format-Konformitaet

OMG XTypes 1.3 §7.4 (CDR Encoding Rules):

- **Primitives** (§7.4.1): alignment relativ zu Stream-Anfang, BE/LE pro
  Stream-Encapsulation
- **String** (§7.4.4): `uint32`-Laenge inkl. Null-Terminator + UTF-8 + `\0`
- **Sequence** (§7.4.4.2): `uint32`-Element-Anzahl + Elemente
- **Array** (§7.4.4.3): N Elemente ohne Length-Prefix
- **Optional** (§7.4.5.1.4): `uint8`-Present-Flag + Wert
- **`@final`-Struct** (§7.4.5.1.1): tight-packed
- **`@appendable`-Struct** (§7.4.3.4.5): DHEADER (uint32 = body length)
- **`@mutable`-Struct` (§7.4.3.4.2): EMHEADER (m-bit + LC + 28-bit ID)
  + NEXTINT

## Coverage

- **XCDR1 (CDR_BE / CDR_LE)** und **XCDR2** — beide
  Encapsulation-Schemes vollstaendig. XCDR1 fuer Legacy-Vendor-Compat
  (RTI Connext, Cyclone DDS Default fuer kleine Typen).
- **EMHEADER-Length-Codes**: LC0..3 + LC4 + LC5..7 — alle 8 Codes
  produzieren bytetidentisch zur OMG XTypes 1.3-Spec.
- **Type-Object-Encoding** (XTypes §7.3) — eigenes Crate
  `zerodds-types` (TypeIdentifier, TypeObject, TypeMap, KeyHash).
- **Code-Gen**: `zerodds-idlc` emittiert pro IDL-Type CDR-Encoder +
  Decoder; manuelle Aufrufe der Helper-Funktionen sind weiter
  moeglich.

## Tests

```bash
cargo test -p zerodds-cdr                        # 84 lib + 7 integration
cargo test -p zerodds-cdr --no-default-features --features alloc  # no_std + alloc
```

---

## Lizenz

Siehe Workspace-`Cargo.toml`.
