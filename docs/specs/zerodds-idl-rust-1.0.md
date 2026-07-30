# zerodds-idl-rust 1.0 — IDL4 → Rust Mapping

> **Vendor-Spec.** OMG hat keine offizielle IDL-Rust-PSM. Diese Spec definiert wie `zerodds-idl-rust` IDL4-Konstrukte auf Rust-Code abbildet — vergleichbar zu OMG-IDL-CPP-Mapping (formal/2018-07-01) für C++17.
>
> **Konformitäts-Träger:** OMG IDL4 (formal/2018-07-01), OMG XTypes 1.3 (formal/2024-04-04), OMG DDS 1.4 (formal/2015-04-10). Diese Spec ist ein Mapping ON TOP, kein Ersatz.
>
> **Output-Konsument:** `zerodds-cdr`/`zerodds-dcps`-Stack (Rust-DataType-Pfad).

## §1 Scope

`zerodds-idl-rust` ist der Build-Zeit-Codegen, der IDL4-Definitionen in Rust-DataType-Code übersetzt. Der erzeugte Code:

- impl-iert `zerodds_dcps::DdsType` für jeden IDL-`struct`,
- impl-iert `zerodds_cdr::CdrEncode`/`CdrDecode` für IDL-`enum`,
- exportiert `pub type` für IDL-`typedef`,
- bildet IDL-`union` auf tagged Rust-`enum` ab,
- bildet IDL-`module` auf Rust-`pub mod` ab,
- handhabt `@key`/`@id`/`@final`/`@appendable`/`@mutable`/`@nested`/`@must_understand`/`@optional` Annotations,
- emittiert `field_value` für SQL-Filter-Evaluation in QueryCondition / ContentFilteredTopic.

**Out-of-Scope:** IDL-`bitset`/`bitmask`/`fixed`/`map`/`any`/`valuetype`/`interface`/`component`/`home`. CORBA-Konstrukte werden vom Rust-DataType-Pfad nicht abgedeckt.

## §2 Type-Mapping

### §2.1 Primitive Types

| IDL | Rust | Wire-Size (XCDR2) |
|---|---|---|
| `boolean` | `bool` | 1 byte |
| `octet` | `u8` | 1 byte |
| `int8` | `i8` | 1 byte |
| `uint8` | `u8` | 1 byte |
| `short`, `int16` | `i16` | 2 byte (alignment 2) |
| `unsigned short`, `uint16` | `u16` | 2 byte |
| `long`, `int32` | `i32` | 4 byte (alignment 4) |
| `unsigned long`, `uint32` | `u32` | 4 byte |
| `long long`, `int64` | `i64` | 8 byte (alignment 8) |
| `unsigned long long`, `uint64` | `u64` | 8 byte |
| `float` | `f32` | 4 byte |
| `double`, `long double` | `f64` | 8 byte |
| `char` | `u8` | 1 byte (8-bit, CDR §9.3.1.5 / XCDR2 §7.4.7) |
| `wchar` | `u16` | 2 byte LE (UTF-16 code-unit) |

`long double` mappt auf `f64` (kein `f128` in stable Rust); LongDouble-IDL-Wire ist 16 byte, dieser Mapping verliert Präzision aber bleibt wire-decodier-bar.

### §2.2 String

| IDL | Rust |
|---|---|
| `string` | `String` |
| `string<N>` | `String` (Bound wird nicht statisch geprüft, Wire-Encoding folgt XCDR2 §7.4.4) |
| `wstring`, `wstring<N>` | `String` (UTF-8 statt UTF-16 — Rust-idiomatisch) |

### §2.3 Composite Types

| IDL | Rust |
|---|---|
| `sequence<T>` | `Vec<T>` |
| `sequence<T, N>` | `Vec<T>` (Bound nicht statisch geprüft) |
| `T[N]` | `[T; N]` |
| `T[N1][N2]` | `[[T; N2]; N1]` |
| `@optional T` | `Option<T>` (Wire-Format-Tag; Field-Type bleibt `T`) |

### §2.4 Constructed Types

| IDL | Rust |
|---|---|
| `struct S { … }` | `pub struct S { pub …, pub …, … }` + `impl DdsType for S` |
| `enum E { A, B, C }` | `pub enum E { A = 0, B = 1, C = 2 }` `#[repr(i32)]` + `from_wire` + `impl CdrEncode/CdrDecode` |
| `union U switch (T) { case 1: A x; … }` | `pub enum U { X(A), … }` (tagged enum, Discriminator implizit) |
| `typedef T Alias` | `pub type Alias = T;` |
| `typedef T Alias[N]` | `pub type Alias = [T; N];` |

### §2.5 Module-Hierarchie

```idl
module M { module N { struct S { long x; }; }; };
```

→

```rust
pub mod M {
    pub mod N {
        pub struct S { pub x: i32 }
        impl zerodds_dcps::DdsType for S { … }
    }
}
```

Nested types werden via `pub mod`-Hierarchie referenzierbar (`M::N::S`).

## §3 Annotation-Mapping

### §3.1 Extensibility

| Annotation | Wire-Form | Codegen |
|---|---|---|
| `@final` | XCDR2 final encoding (kein DHEADER) | direkter encode in deklarations-Reihenfolge |
| `@appendable` (default) | XCDR2 appendable (DHEADER + body, XTypes §7.4.3.4.5) | `zerodds_cdr::struct_enc::encode_appendable(writer, |w| { … })` |
| `@mutable` | XCDR2 mutable (DHEADER + member-id-tagged members, §7.4.3.4.4) | `zerodds_cdr::struct_enc::MutableStructEncoder::new(...).encode(|enc| { enc.member(id, must_understand, |w| { … })?; … })` |
| `@extensibility(FINAL\|APPENDABLE\|MUTABLE)` | wie obige | gleichwertig zu spezifischer Annotation |

**Default-Wahl:** ZeroDDS-Codegen nimmt `appendable` als Default (XTypes-1.3-spec-konform, §7.3.3.1). Wer den kompakteren `final`-Wire ohne DHEADER will, annotiert explizit `@final` oder setzt `--default-extensibility final` (bzw. das Cargo-Feature `ext-default-final`).

### §3.2 @key

`@key` markiert Member als Teil der Topic-Instance-Identity (DDS 1.4 §2.2.3, XTypes 1.3 §7.6.8).

Codegen-Effekt:
- `const HAS_KEY: bool = true`
- `const KEY_HOLDER_MAX_SIZE: Option<usize>` wird statisch berechnet — Summe der Wire-Sizes aller `@key`-Member, falls alle fixed-size sind. String/Sequence/etc. → `None` (MD5-Pfad in `compute_key_hash`).
- `fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder)` schreibt die Key-Member in Big-Endian, member-id-sortiert (Spec §7.6.8.3.1.b).

### §3.3 @id(N)

Setzt explizite Member-ID für mutable extensibility und @key-Multi-Member-Sortierung. Default: positional (Index in Deklarations-Reihenfolge).

### §3.4 @must_understand

Markiert mutable-Member als pflicht — Decoder verwirft die Message wenn ein must_understand-Member-ID nicht erkannt wird.

### §3.5 @nested

Markiert Struct als „nicht topic-fähig" (kann nicht direkt für DDS-Topics registriert werden). Im Codegen aktuell informell — wird in `is_nested`-Property zurückgegeben aber nicht durchgesetzt.

### §3.6 @optional

Markiert Member als optional. Im Wire: führender bool für present-Flag (XTypes §7.4.5.1.4). Der Codegen emittiert `Option<T>` als Field-Type.

## §4 DdsType-Trait-Impl

Pro IDL-`struct` emittiert der Codegen:

```rust
impl zerodds_dcps::DdsType for <Name> {
    const TYPE_NAME: &'static str = "<Name>";
    const HAS_KEY: bool = …;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = …;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), zerodds_dcps::EncodeError> { … }
    fn decode(bytes: &[u8]) -> Result<Self, zerodds_dcps::DecodeError> { … }
    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) { … } // wenn HAS_KEY
    fn field_value(&self, path: &str) -> Option<zerodds_sql_filter::Value> { … }
}
```

`TYPE_NAME` ist der unqualifizierte Type-Name (gleich der Struct-Identifier). Bei Module-nested Types muss der Caller im DDS-Topic-Register-Aufruf den fully-qualified Pfad selbst angeben — der Codegen registriert keinen Module-Prefix.

## §5 Wire-Format-Konformität

Der Codegen delegiert die Wire-Encoding-Logik an `zerodds-cdr`:

- Primitives + Composite: `zerodds_cdr::CdrEncode`/`CdrDecode` Trait.
- Final-Struct: direkter encode/decode in deklarations-Reihenfolge.
- Appendable-Struct: `zerodds_cdr::struct_enc::encode_appendable` / `decode_appendable` (DHEADER-Wrap).
- Mutable-Struct: `zerodds_cdr::struct_enc::MutableStructEncoder` (Encode mit member-id + LengthCode); der Decode nutzt aktuell einen `decode_appendable`-Wrap als Vereinfachung — eine Erweiterung auf einen `read_mutable_member`-Loop mit beliebiger Member-Reihenfolge ist möglich.
- Enum: `zerodds_cdr::CdrEncode for i32` mit Discriminator-Wert (XTypes §7.4.5.1).

## §6 Naming-Konventionen

Da OMG IDL Identifier nicht 1:1 auf Rust-Identifier abbilden:

- IDL-`Type`-Identifier ($\to$ Rust-Type) bleiben unverändert (z.B. `Pose` $\to$ `Pose`).
- IDL-`field`/`enumerator`-Identifier bleiben unverändert (z.B. `sensor_id` $\to$ `sensor_id`).
- Rust-Reserved-Words als IDL-Identifier (`type`, `match`, `mod`, `fn`, …) werden aktuell nicht escaped — Caller muss IDL anpassen oder den Codegen erweitern (künftig: raw-identifier `r#…`).
- Module-Identifier bleiben unverändert (z.B. `module geom` $\to$ `pub mod geom`).

## §7 Out-of-Scope

| IDL-Konstrukt | Out-of-Scope-Begründung |
|---|---|
| `bitset` / `bitmask` (§7.4.7) | Nicht typisch für DDS-Topics; bei Bedarf, nicht in v1.0 |
| `fixed` (§7.4.4.5) | Financial-Domain; CORBA-Pfad reicht |
| `map<K, V>` (§7.4.4.6) | BTreeMap-Default; HashMap-Variante optional via Annotation |
| `any` (§7.4.4.7) | Type-Erasure passt nicht zu Rust-Generics; CORBA-Pfad |
| `valuetype` (§7.4.5.4) | CORBA-Konstrukt |
| `interface` (§7.4.6.4) | CORBA-Konstrukt; DDS-RPC-Service-Codegen läuft über `zerodds-rpc` |
| `component` / `home` (§7.4.8/§7.4.9) | CCM-Konstrukte; CORBA-Stack |

## §8 Konformitäts-Test

`zerodds-idl-rust` ist konform wenn:

1. **Snapshot-Identität** — für jede IDL-Quelle ist der emittierte Code reproduzierbar (deterministisch).
2. **Compile-Korrektheit** — der emittierte Code kompiliert gegen `zerodds-cdr`/`zerodds-dcps`/`zerodds-sql-filter` ohne Errors/Warnings.
3. **Wire-Roundtrip** — encode + decode = identity für alle in §2 spezifizierten Type-Mappings.
4. **Wire-Konformität** — die XCDR2-Bytes der encode-Funktion folgen XTypes 1.3 §7.4.

Tests dafür liegen in:
- `crates/idl-rust/tests/snapshot_codegen.rs` (Konformität §8.1)
- `crates/idl-rust/tests/compile_check.rs` (Konformität §8.2)
- `crates/idl-rust/tests/wire_roundtrip.rs` (Konformität §8.3 + §8.4)
