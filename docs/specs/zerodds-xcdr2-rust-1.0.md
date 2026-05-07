# `zerodds-xcdr2-rust` v1.0 — Rust TypeSupport-Codegen

ZeroDDS Vendor-Spec. Implementiert in `crates/idl-rust` (Codegen),
`crates/cdr` (Encoder/Decoder) und `crates/dcps` (Trait-Anchor).
Konformanz gegen
[`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md).

## §1 Motivation

Es existiert **keine OMG-DDS-Rust-PSM-Spec**. Rust-Bindings fuer
DDS-Vendoren (RustDDS, dust-dds) haben jeweils proprietaere Patterns.
ZeroDDS hat seit RC1 ein voll funktionsfaehiges `DdsType`-Trait;
diese Spec dokumentiert es **formal als normative Pflicht** fuer
idl-rust-Codegen und Konformanz.

Status vor v1.0: voll implementiert in `idl-rust`, aber ohne
formale Spec — diese Datei macht das verbindlich.

## §2 TypeSupport-Pattern

```rust
// crates/dcps/src/dds_type.rs (existing)
pub trait DdsType: Sized {
    /// DDS-Type-Name (Module::Sub::Struct).
    const TYPE_NAME: &'static str;

    /// XTypes Type-Identifier (Equivalence-Hash).
    const TYPE_IDENTIFIER: TypeIdentifier;

    /// Final / Appendable / Mutable.
    const EXTENSIBILITY: ExtensibilityKind;

    /// Hat mindestens ein @key-Member.
    const IS_KEYED: bool;

    /// Encoder. Default-Endian = LE.
    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError>;
    fn encode_be(&self, out: &mut Vec<u8>) -> Result<(), EncodeError>;

    /// Decoder.
    fn decode(bytes: &[u8]) -> Result<Self, DecodeError>;

    /// Key-Hash (16 Bytes MD5 ueber PlainCdr2BeKeyHolder).
    fn key_hash(&self) -> [u8; 16];
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExtensibilityKind { Final, Appendable, Mutable }
```

`Sized` ist Pflicht weil `Self`-Return in `decode`. `'static`-Trait-
Bounds halten Type-Names + Hash compile-time-konstant.

## §3 Required API-Surface

```rust
// Generierter Code fuer struct Point { long x; long y; }
use zerodds_dcps::*;
use zerodds_cdr::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl DdsType for Point {
    const TYPE_NAME: &'static str = "Point";
    const TYPE_IDENTIFIER: TypeIdentifier = /* compile-time hash */;
    const EXTENSIBILITY: ExtensibilityKind = ExtensibilityKind::Final;
    const IS_KEYED: bool = false;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        let mut writer = Xcdr2Writer::new(out, Endian::Le);
        writer.write_i32(self.x)?;
        writer.write_i32(self.y)?;
        Ok(())
    }

    fn encode_be(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        let mut writer = Xcdr2Writer::new(out, Endian::Be);
        writer.write_i32(self.x)?;
        writer.write_i32(self.y)?;
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut reader = Xcdr2Reader::new(bytes, Endian::Le);
        Ok(Point {
            x: reader.read_i32()?,
            y: reader.read_i32()?,
        })
    }

    fn key_hash(&self) -> [u8; 16] {
        [0u8; 16] // !is_keyed
    }
}
```

## §4 Codegen-Pflicht (idl-rust)

Pro IDL-`struct` MUSS `idl-rust` emittieren:

1. `pub struct Point` mit Public-Felder (existiert).
2. `impl DdsType for Point` mit allen 4 Konstanten + 4 Methoden
   (existiert).
3. **Pflicht hier**: `key_holder_be()` fuer `@key`-Members in BE-
   PlainCDR2 (existiert via `PlainCdr2BeKeyHolder::write_<type>`).
4. **Pflicht**: Compile-Zeit-`TYPE_IDENTIFIER` aus IDL-AST via
   const-fn-Hashing (existiert in `crates/idl-rust/src/type_identifier.rs`).

Generierter Code lebt im Modul-Pfad der dem IDL-Modul-Pfad entspricht
(z.B. `module Outer { struct S }` → `pub mod outer { pub struct S {
... } impl DdsType for S }`).

## §5 Wire-Type-Mapping

| IDL | Rust | Wire (XCDR2 LE) |
|-----|------|-----------------|
| `boolean` | `bool` | 1 Byte |
| `octet` | `u8` | 1 Byte |
| `char` | `u8` (ASCII) | 1 Byte |
| `wchar` | `u16` (UTF-16 code-unit) | 2 Byte LE |
| `short` / `int16` | `i16` | 2 Byte LE Align(2) |
| `unsigned short` / `uint16` | `u16` | 2 Byte LE Align(2) |
| `long` / `int32` | `i32` | 4 Byte LE Align(4) |
| `unsigned long` / `uint32` | `u32` | 4 Byte LE Align(4) |
| `long long` / `int64` | `i64` | 8 Byte LE Align(8) |
| `unsigned long long` / `uint64` | `u64` | 8 Byte LE Align(8) |
| `float` | `f32` | 4 Byte IEEE-754 LE |
| `double` | `f64` | 8 Byte IEEE-754 LE |
| `string` | `String` | uint32 length+1 + UTF-8 + NUL |
| `wstring` | `String` (UTF-16 on wire) | uint32 length + UTF-16-LE |
| `sequence<T>` | `Vec<T>` | uint32 count + T[] |
| `T[N]` | `[T; N]` | T[] N Elemente |
| nested `struct U` | `U` | rekursiv `<U as DdsType>::encode(out)` |
| `enum E` | `enum E { A=0, B=1 }` mit `#[repr(i32)]` | int32 LE |
| `@optional T` | `Option<T>` | M-Flag (Mutable) / 1-Byte present |
| `@external T` | `Box<T>` | wie Plain-Member |

## §6 Extensibility

```rust
// @final
const EXTENSIBILITY: ExtensibilityKind = ExtensibilityKind::Final;
// kein DHEADER, kein EMHEADER

// @appendable (default)
const EXTENSIBILITY: ExtensibilityKind = ExtensibilityKind::Appendable;
// DHEADER prefixed via zerodds_cdr::struct_enc::encode_appendable

// @mutable
const EXTENSIBILITY: ExtensibilityKind = ExtensibilityKind::Mutable;
// PL_CDR2 mit EMHEADER pro Member via encode_mutable
```

`zerodds_cdr::struct_enc` haelt die drei Mode-Encoder bereits implementiert.

## §7 Key-Extraction

```rust
fn key_hash(&self) -> [u8; 16] {
    let mut holder = PlainCdr2BeKeyHolder::new();
    self.encode_key_holder_be(&mut holder);
    holder.finalize_md5()
}

fn encode_key_holder_be(&self, h: &mut PlainCdr2BeKeyHolder) {
    h.write_i32(self.id); // @key
}
```

`PlainCdr2BeKeyHolder` in `crates/cdr` haelt Big-Endian-Plain-CDR2-
Buffer + finalize → MD5 (RFC 1321 via `md5`-crate, optional-disabled
fuer no_std-Targets).

## §8 Helper-Library

| Crate | Inhalt |
|-------|--------|
| `crates/cdr` | XCDR2-Encoder/Decoder (`struct_enc`, `Xcdr2Writer`, `Xcdr2Reader`, `PlainCdr2BeKeyHolder`) |
| `crates/dcps` | Trait `DdsType`, `EncodeError`, `DecodeError`, `ExtensibilityKind` |
| `crates/idl-rust` | Codegen `crate::struct_emit`, `crate::enum_emit`, `crate::bitset_emit`, `crate::type_identifier` |

Alle Crates sind no_std-fest mit `alloc` (volle Compatibility mit
embedded Targets).

## §9 Conformance

L1-L4 gegen [`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md):

- L1 (Wire): `crates/cdr/tests/xcdr2_wire_vectors.rs` prueft V-1..V-12.
- L2 (Codegen): `crates/idl-rust/tests/snapshots/` mit generierten
  `*.rs`-Files.
- L3 (Cross-Lang): `crates/conformance/tests/cross_language_xcdr2.rs`
  ruft `cargo run --bin rust-xcdr2-runner`.
- L4 (Cross-Vendor): bereits live ueber `crates/discovery/tests/cyclone_*.rs`.

## §10 Examples

`crates/dcps/examples/hello_dds_publisher.rs` ist Referenz-Smoke
(generierter `Chatter`-Type + Pub/Sub-Loop).

## §11 Errata + Open-Questions

- **§11.1 Derive-Macro**: `#[derive(DdsType)]` waere ergonomisch.
  Aktueller Code-Generator emittiert Hand-implementierten `impl
  DdsType` weil Derive-Macros const-fn-Hash-Berechnung erschweren.
  Folge-Sprint moeglich; v1.0-Spec bleibt bei codegen-emittierten
  expliziten Impls.
- **§11.2 `bytes::Bytes`**: Encoder schreibt nach `Vec<u8>` (alloc-
  managed). Zero-Copy-Pfad via `&mut [u8]` ist optional (XTypes-
  konform aber Sprach-spezifisches Add-on).
- **§11.3 `serde`-Bridge**: Optional-Feature `serde-bridge` waere
  hilfreich aber nicht Bestandteil von v1.0.
- **§11.4 const-Generic-Bounds**: `sequence<T, N>`-Bounds via
  `ConstSize<N>`-Trait waeren ideal, aber stable-Rust unterstuetzt
  `const-generic-exprs` noch nicht voll. Codegen prueft Bound zur
  Laufzeit, nicht zur Compile-Zeit.
