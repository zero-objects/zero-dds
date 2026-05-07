# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Added

- **XCDR2 wire-vector conformance suite** in
  `tests/xcdr2_wire_vectors.rs`. Validates the encoder + decoder
  byte-exact against the master conformance vectors V-1..V-12 of
  `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6:
  V-1 empty `@final`, V-2 two int32, V-3 mixed primitives,
  V-4 string, V-5 sequence<long>, V-6 sequence<string>,
  V-7 nested-modules struct, V-8 keyed payload + KeyHash zero-pad,
  V-9 `@appendable`, V-10 `@mutable` two members,
  V-11a/b `@mutable @optional` present/absent,
  V-12 empty `@mutable` (DHEADER=0). 16 tests, all byte-exact pass
  against the existing encoder.

### Notes

- This release does not modify encoder/decoder behavior; it
  formalizes the existing `zerodds-cdr` implementation as
  spec-conform. See `docs/spec-coverage/zerodds-xcdr2-rust-1.0.md`
  §11 for two documented deltas vs the master-spec sample text
  (KeyHash follows OMG XTypes §7.6.8.4 zero-pad rule; EMHEADER
  bytes are LE per `CDR2_LE = 0x0010`).

## [1.0.0-rc.1]

Initiale Release-Materialisierung der `zerodds-cdr`-Crate.

### Spec-Referenzen

- **OMG XTypes 1.3** §7.4 — Wire-Encoding für XCDR1, Plain CDR2, Delimited CDR2, PL_CDR2.
- **OMG XTypes 1.3** §7.4.1.2 — PL_CDR1 Member-Codec (Standard-Header + Extended-Header + PID_LIST_END Sentinel).
- **OMG XTypes 1.3** §7.4.2 — XCDR2 Plain/Delimited/Parameter-List Encodings.
- **OMG XTypes 1.3** §7.4.4 — Composite Types (String, Sequence, Array, Optional).
- **OMG XTypes 1.3** §7.4.5 — Struct-Extensibility (`final`, `appendable`, `mutable`).
- **OMG XTypes 1.3** §7.6.8 — KeyHash (PlainCdr2BeKeyHolder mit MD5-Fallback bei `max_size > 16 byte`).
- **DDSI-RTPS 2.5** §10 — Wire-Encapsulation, RepresentationIdentifier-Bytes (CDR_LE/CDR_BE/PL_CDR_LE/PL_CDR_BE).
- **OMG IDL 4.2** §7.4.13 — `fixed<P, S>` Decimal-Type (BCD-Encoding).
- **RFC 1321** — MD5 (via `zerodds-foundation::md5`) für KeyHash + EquivalenceHash.

### Public-API

**Buffer-I/O:**
- `BufferReader<'a>` — Alignment-tracking Byte-Reader mit `read_u8`/`read_u16`/`read_u32`/`read_u64`/`read_i*`/`read_f*`/`read_bytes`/`read_string`.
- `BufferWriter` — Alignment-tracking Byte-Writer mit `write_*`-Familie und `into_bytes`.
- `Endianness::{Little, Big}` — Endianness-Marker für alle CDR-Operationen.

**Trait-Familie:**
- `CdrEncode` / `CdrDecode` — Serializer-Traits für alle XCDR2-Wire-Primitives.

**Composite-Type-Impls** (`composite`-Modul, `alloc`-Feature):
- `impl CdrEncode for str / String / Vec<T> / [T; N] / Option<T>` (XTypes §7.4.4).

**Struct-Extensibility-Encoder** (`struct_enc`-Modul):
- `encode_final` / `decode_final` — XCDR2 Plain CDR2 (§7.4.2.1).
- `encode_appendable` / `decode_appendable` — XCDR2 Delimited CDR2 (§7.4.2.2, DHEADER + body).
- `MutableStructEncoder` — XCDR2 Parameter-List Encoder mit Required-Members-Validierung (§7.4.2.4).
- `encode_mutable_member` / `encode_mutable_member_lc` — Low-Level EMHEADER-Emit.
- `read_mutable_member` / `read_all_mutable_members` — EMHEADER-Decode mit Length-Code-Switch.
- `MutableMember<'a>` — geparsten Member-Slice.
- `LengthCode` — XCDR2 Length-Code (LC0–LC7) gemäß Tabelle 7-19.

**XCDR1 / PL_CDR1** (`xcdr1`-Modul):
- `encode_pl_cdr1_member` — Standard-Header + Extended-Header automatisch wählend (§7.4.1.2.2).
- `read_pl_cdr1_member` / `read_all_pl_cdr1_members` — Decoder mit Sentinel-Erkennung.
- `write_pl_cdr1_sentinel` — `PID_LIST_END (0x3F02)` Terminator.
- Konstanten: `PID_LIST_END`, `PID_EXTENDED`, `PID_EXTENDED_THRESHOLD`.
- `PlCdr1Member` — geparsten Member.

**Fixed-Decimal** (`fixed`-Modul, `alloc`-Feature):
- `Fixed<const P: u32, const S: u32>` — IDL-`fixed<P, S>`-Type mit BCD-Wire-Format (IDL 4.2 §7.4.13).

**KeyHash** (`key_hash`-Modul, `alloc`-Feature):
- `compute_key_hash(holder: &[u8], max_size: usize) -> [u8; 16]` — XTypes 1.3 §7.6.8.
- `PlainCdr2BeKeyHolder` — CDR_BE-encoded Key-Holder-Struktur.
- `KEY_HASH_LEN` — `16` (Konstante).

**Error-Familie:**
- `EncodeError::{BufferFull, ValueOutOfRange, MissingNonOptionalMember}` mit Offset/Member-ID-Kontext.
- `DecodeError::{UnexpectedEof, InvalidString, LengthExceeded, InvalidEnum, InvalidBoolean, InvalidEncapsulation}` mit Offset-Kontext.

### Implementierung

- `forbid(unsafe_code)` über die ganze Crate.
- `#![no_std]` + opt-in `alloc`-Feature für composite/fixed/key_hash/struct_enc/xcdr1.
- Alignment-Tracking via Position-Counter + `pad_to`-Berechnung; Padding wird vor jedem Multibyte-Field automatisch eingefügt.
- 199 Tests grün (170 unit + 1 compliance_xcdr2 + 7 fuzz_smoke + 7 integration_topic + 15 proptest_roundtrip + 1 doc-test).
- Bench-Suite `encode_decode_hotpaths` (criterion) für u32/string/sequence/struct.
- Fuzz-Targets im `fuzz/`-Verzeichnis (libFuzzer): `read_pl_cdr1_member`, `read_all_pl_cdr1_members`, plus zentrale composite/struct_enc-Targets.

### Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std`   | ✅      | Re-exports + std-Error-Impl. Implies `alloc`. |
| `alloc` | ✅      | Aktiviert `composite`/`fixed`/`key_hash`/`struct_enc`/`xcdr1` (die alle `Vec`/`String` brauchen). |
