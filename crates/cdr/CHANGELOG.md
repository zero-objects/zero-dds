# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

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

Initial release materialization of the `zerodds-cdr` crate.

### Spec references

- **OMG XTypes 1.3** §7.4 — wire encoding for XCDR1, Plain CDR2, Delimited CDR2, PL_CDR2.
- **OMG XTypes 1.3** §7.4.1.2 — PL_CDR1 member codec (standard header + extended header + PID_LIST_END sentinel).
- **OMG XTypes 1.3** §7.4.2 — XCDR2 Plain/Delimited/Parameter-List encodings.
- **OMG XTypes 1.3** §7.4.4 — composite types (String, Sequence, Array, Optional).
- **OMG XTypes 1.3** §7.4.5 — struct extensibility (`final`, `appendable`, `mutable`).
- **OMG XTypes 1.3** §7.6.8 — KeyHash (PlainCdr2BeKeyHolder with MD5 fallback when `max_size > 16 byte`).
- **DDSI-RTPS 2.5** §10 — wire encapsulation, RepresentationIdentifier bytes (CDR_LE/CDR_BE/PL_CDR_LE/PL_CDR_BE).
- **OMG IDL 4.2** §7.4.13 — `fixed<P, S>` decimal type (BCD encoding).
- **RFC 1321** — MD5 (via `zerodds-foundation::md5`) for KeyHash + EquivalenceHash.

### Public API

**Buffer I/O:**
- `BufferReader<'a>` — alignment-tracking byte reader with `read_u8`/`read_u16`/`read_u32`/`read_u64`/`read_i*`/`read_f*`/`read_bytes`/`read_string`.
- `BufferWriter` — alignment-tracking byte writer with the `write_*` family and `into_bytes`.
- `Endianness::{Little, Big}` — endianness marker for all CDR operations.

**Trait family:**
- `CdrEncode` / `CdrDecode` — serializer traits for all XCDR2 wire primitives.

**Composite-type impls** (`composite` module, `alloc` feature):
- `impl CdrEncode for str / String / Vec<T> / [T; N] / Option<T>` (XTypes §7.4.4).

**Struct-extensibility encoder** (`struct_enc` module):
- `encode_final` / `decode_final` — XCDR2 Plain CDR2 (§7.4.2.1).
- `encode_appendable` / `decode_appendable` — XCDR2 Delimited CDR2 (§7.4.2.2, DHEADER + body).
- `MutableStructEncoder` — XCDR2 Parameter-List encoder with required-members validation (§7.4.2.4).
- `encode_mutable_member` / `encode_mutable_member_lc` — low-level EMHEADER emit.
- `read_mutable_member` / `read_all_mutable_members` — EMHEADER decode with length-code switch.
- `MutableMember<'a>` — parsed member slice.
- `LengthCode` — XCDR2 length code (LC0–LC7) per Table 7-19.

**XCDR1 / PL_CDR1** (`xcdr1` module):
- `encode_pl_cdr1_member` — automatically selecting standard header + extended header (§7.4.1.2.2).
- `read_pl_cdr1_member` / `read_all_pl_cdr1_members` — decoder with sentinel detection.
- `write_pl_cdr1_sentinel` — `PID_LIST_END (0x3F02)` terminator.
- Constants: `PID_LIST_END`, `PID_EXTENDED`, `PID_EXTENDED_THRESHOLD`.
- `PlCdr1Member` — parsed member.

**Fixed decimal** (`fixed` module, `alloc` feature):
- `Fixed<const P: u32, const S: u32>` — IDL `fixed<P, S>` type with BCD wire format (IDL 4.2 §7.4.13).

**KeyHash** (`key_hash` module, `alloc` feature):
- `compute_key_hash(holder: &[u8], max_size: usize) -> [u8; 16]` — XTypes 1.3 §7.6.8.
- `PlainCdr2BeKeyHolder` — CDR_BE-encoded key-holder structure.
- `KEY_HASH_LEN` — `16` (constant).

**Error family:**
- `EncodeError::{BufferFull, ValueOutOfRange, MissingNonOptionalMember}` with offset/member-ID context.
- `DecodeError::{UnexpectedEof, InvalidString, LengthExceeded, InvalidEnum, InvalidBoolean, InvalidEncapsulation}` with offset context.

### Implementation

- `forbid(unsafe_code)` across the whole crate.
- `#![no_std]` + opt-in `alloc` feature for composite/fixed/key_hash/struct_enc/xcdr1.
- Alignment tracking via a position counter + `pad_to` computation; padding is inserted automatically before every multibyte field.
- 199 tests passing (170 unit + 1 compliance_xcdr2 + 7 fuzz_smoke + 7 integration_topic + 15 proptest_roundtrip + 1 doc-test).
- Bench suite `encode_decode_hotpaths` (criterion) for u32/string/sequence/struct.
- Fuzz targets in the `fuzz/` directory (libFuzzer): `read_pl_cdr1_member`, `read_all_pl_cdr1_members`, plus central composite/struct_enc targets.

### Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std`   | ✅      | Re-exports + std error impl. Implies `alloc`. |
| `alloc` | ✅      | Enables `composite`/`fixed`/`key_hash`/`struct_enc`/`xcdr1` (all of which need `Vec`/`String`). |
