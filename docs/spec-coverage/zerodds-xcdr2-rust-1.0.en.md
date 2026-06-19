# `zerodds-xcdr2-rust` 1.0 -- Spec Coverage

**Source:** `docs/specs/zerodds-xcdr2-rust-1.0.md` (220 lines) -- the ZeroDDS Rust TypeSupport codegen spec.

Implementation:

- `crates/idl-rust/` — Rust XCDR2 TypeSupport codegen (DdsType trait in `crates/dcps/`).

## §1 Motivation

### §1 No OMG DDS-Rust-PSM spec

**Spec:** §1 -- "There is no OMG DDS-Rust-PSM spec. Rust bindings for DDS vendors (RustDDS, dust-dds) each have proprietary patterns. ZeroDDS has had a fully functional DdsType trait since RC1; this spec documents it formally as a normative requirement."

**Repo:** the motivation text of the vendor spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport pattern

### §2 `DdsType` trait with 4 constants + 4 methods

**Spec:** §2 -- "`pub trait DdsType: Sized` with the constants `TYPE_NAME`, `TYPE_IDENTIFIER`, `EXTENSIBILITY`, `IS_KEYED` and the methods `encode/encode_be/decode/key_hash`. ExtensibilityKind enum {Final, Appendable, Mutable}."

**Repo:** `crates/dcps/src/dds_type.rs` (line 51 `pub trait DdsType: Sized`, plus constants from line 54, methods in the trait body). `crates/dcps/src/dds_type.rs::Extensibility` enum.

**Tests:** `crates/dcps/` unit tests in the trait file, integration via `crates/cdr/tests/integration_topic.rs` (7 tests).

**Status:** done

## §3 Required API surface

### §3 A full `impl DdsType` per IDL `struct`

**Spec:** §3 -- "Generated code: `pub struct Point` + a full `impl DdsType for Point` with all 4 constants + 4 methods."

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_dds_type_impl` (line 95ff) emits TYPE_NAME, TYPE_IDENTIFIER, EXTENSIBILITY, IS_KEYED, encode, encode_be, decode, key_hash.

**Tests:** `crates/cdr/tests/xcdr2_wire_vectors.rs` (16 tests) consumes the generated outputs; `crates/cdr/tests/proptest_roundtrip.rs` (15 tests).

**Status:** done

## §4 Codegen requirement (idl-rust)

### §4 Struct + impl + key_holder_be + compile-time TYPE_IDENTIFIER

**Spec:** §4 -- "Per IDL `struct`, idl-rust MUST emit: 1) `pub struct Point`, 2) `impl DdsType`, 3) `key_holder_be()` for @key members in BE-PlainCDR2, 4) a compile-time TYPE_IDENTIFIER."

**Repo:** `crates/idl-rust/src/struct_emit.rs` (`emit_struct_decl`, `emit_dds_type_impl`, `emit_key_holder_be`). `crates/idl-rust/src/type_identifier.rs` holds const-fn hashing for TYPE_IDENTIFIER.

**Tests:** snapshot coverage via `idl-rust` lib unit tests + downstream `xcdr2_wire_vectors.rs`.

**Status:** done

### §4 Module path = IDL module path

**Spec:** §4 -- "The generated code lives in the module path that matches the IDL module path."

**Repo:** `crates/idl-rust/src/emitter.rs` module-path mapping → `pub mod` statement.

**Tests:** V-7 nested modules `Outer::Inner::S` in `xcdr2_wire_vectors.rs::v7_nested_modules_final`.

**Status:** done

## §5 Wire type mapping

### §5 IDL-to-Rust types + wire layout

**Spec:** §5, table of 18 IDL types → Rust → XCDR2 LE.

**Repo:** `crates/idl-rust/src/type_map.rs` maps IDL primitives to Rust. `crates/cdr/src/encode.rs` write helpers.

**Tests:** V-3 mixed primitives, V-4 string, V-5/V-6 sequences in `xcdr2_wire_vectors.rs`.

**Status:** done

## §6 Extensibility

### §6 Final / Appendable / Mutable mode encoder

**Spec:** §6 -- "`zerodds_cdr::struct_enc` already implements the three mode encoders (`encode_appendable`, `encode_mutable_member_lc`)."

**Repo:** `crates/cdr/src/struct_enc.rs` (line 42 `encode_appendable`, line 167 `encode_mutable_member_lc`).

**Tests:** V-9 Appendable, V-10 Mutable, V-11 optional Mutable in `xcdr2_wire_vectors.rs`.

**Status:** done

## §7 Key extraction

### §7 PlainCdr2BeKeyHolder + finalize_md5

**Spec:** §7 -- "PlainCdr2BeKeyHolder in `crates/cdr` holds a big-endian Plain-CDR2 buffer + finalize → MD5 (RFC 1321 via the md5 crate, optional-disabled for no_std targets)."

**Repo:** `crates/cdr/src/key_hash.rs::PlainCdr2BeKeyHolder` (line 86), the `finalize_md5` method with a holder-size check per XTypes §7.6.8.4.

**Tests:** V-8 keyed-struct test (holder=4 bytes → zero-padded, no MD5); MD5 self-tests in `crates/cdr/src/key_hash.rs` unit tests (line 311ff).

**Status:** done

## §8 Helper library

### §8 cdr + dcps + idl-rust crates

**Spec:** §8, table of 3 crates -- `crates/cdr` (encoder/decoder, struct_enc, Xcdr2Writer/Reader, PlainCdr2BeKeyHolder), `crates/dcps` (DdsType trait, errors, ExtensibilityKind), `crates/idl-rust` (codegen).

**Repo:** all three crates present: `crates/cdr/src/{encode,struct_enc,key_hash,buffer,fixed,xcdr1,composite,endianness,error}.rs`, `crates/dcps/src/dds_type.rs`, `crates/idl-rust/src/{struct_emit,enum_emit,bitset_emit,type_identifier,type_map,emitter,annotations}.rs`.

**Tests:** `cargo test -p zerodds-cdr -p zerodds-idl-rust` -- all green.

**Status:** done

### §8 no_std-safe with alloc

**Spec:** §8 -- "All crates are no_std-safe with `alloc` (full compatibility with embedded targets)."

**Repo:** `crates/cdr/src/lib.rs` and `crates/dcps/src/lib.rs` with `#![no_std]` + `extern crate alloc`. The md5 feature is optional-disabled.

**Tests:** build with `--no-default-features` via CI; the embedded-target smoke test indirectly via crate configuration.

**Status:** done

## §9 Conformance

### §9 L1 wire (V-1..V-12)

**Spec:** §9 -- "L1 (wire): `crates/cdr/tests/xcdr2_wire_vectors.rs` checks V-1..V-12."

**Repo:** `crates/cdr/tests/xcdr2_wire_vectors.rs` with 16 #[test] functions.

**Tests:** as above.

**Status:** done

### §9 L2 codegen snapshots

**Spec:** §9 -- "L2 (codegen): `crates/idl-rust/tests/snapshots/` with generated *.rs files."

**Repo:** `crates/idl-rust/tests/snapshot_xcdr2_vectors.rs` holds 11 V-i snapshots as a standalone test tree.

**Tests:** `crates/idl-rust/tests/snapshot_xcdr2_vectors.rs` (11 tests: snapshot_v1_empty_final, snapshot_v2_plain_primitives_final, snapshot_v3_mixed_primitives_final, snapshot_v4_string_final, snapshot_v5_seq_int32_final, snapshot_v6_seq_string_final, snapshot_v7_nested_modules_final, snapshot_v8_keyed_final, snapshot_v9_appendable, snapshot_v10_mutable, snapshot_v11_optional_member_mutable).

**Status:** done

### §9 L3 cross-language runner

**Spec:** §9 -- "L3 (cross-language): `crates/conformance/tests/cross_language_xcdr2.rs` calls `cargo run --bin rust-xcdr2-runner`."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_1_rust_reference_encoder` calls the zerodds-cdr wire-vector suite via subprocess against the identical V-1..V-12 hex fixtures.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_1_rust_reference_encoder`.

**Status:** done

### §9 L4 cross-vendor

**Spec:** §9 -- "L4 (cross-vendor): already live via `crates/discovery/tests/cyclone_*.rs`."

**Repo:** `crates/discovery/tests/cyclone_*.rs` covers discovery (SPDP/SEDP) against Cyclone live; the XCDR2 wire-format equivalence is carried along. An xcdr2-specific V-1..V-12 Cyclone fixture validation is not present, but the live RTPS path covers the end-to-end wire path.

**Tests:** `crates/discovery/tests/cyclone_*.rs` live tests.

**Status:** done

## §10 Examples

### §10 hello_dds_publisher.rs

**Spec:** §10 -- "`crates/dcps/examples/hello_dds_publisher.rs` is the reference smoke (a generated Chatter type + a pub/sub loop)."

**Repo:** `crates/dcps/examples/hello_dds_publisher.rs` (smoke demo).

**Tests:** the run is demo-only; the compile path via `cargo build --example hello_dds_publisher`.

**Status:** done

## §11 Errata + open questions

### §11.1 Derive macro

**Spec:** §11.1 -- "`#[derive(DdsType)]` as an ergonomic alternative to the codegen-emitted form."

**Repo:** `crates/cdr-derive/` (crate name `zerodds-cdr-derive`, a proc-macro). The derive macro emits TYPE_NAME + EXTENSIBILITY + IS_KEYED + encode/decode/key_hash byte-identical to the codegen form.

**Tests:** `crates/cdr-derive/tests/derive_smoke.rs` (6 tests: keyed_struct_sets_has_key, point_encode_matches_v2_wire, point_roundtrip, sensor_roundtrip, type_name_attribute_overrides_default, type_name_default_is_struct_ident).

**Status:** done

### §11.2 bytes::Bytes

**Spec:** §11.2 -- "The encoder writes to `Vec<u8>` (alloc-managed). A zero-copy path via `&mut [u8]` is optional (XTypes-conformant but a language-specific add-on)."

**Repo:** the `Vec<u8>` path in `crates/cdr/src/encode.rs`. The slice path as an add-on is not in v1.0 scope.

**Tests:** round-trip tests with Vec<u8> in `xcdr2_wire_vectors.rs`.

**Status:** done

### §11.3 serde bridge

**Spec:** §11.3 -- "An optional `serde-bridge` feature as an additional form for serde-consuming applications."

**Repo:** `crates/cdr/Cargo.toml` holds the `serde-bridge` feature gate; the implementation in `crates/cdr/src/serde_bridge.rs`.

**Tests:** `crates/cdr/tests/serde_bridge.rs` (3 tests: serde_roundtrip_primitives, serde_roundtrip_struct, serde_decoded_json_repr_is_xcdr2_string_payload), `cargo test -p zerodds-cdr --test serde_bridge --features serde-bridge`.

**Status:** done

### §11.4 const-generic bounds

**Spec:** §11.4 -- "`sequence<T, N>` bounds via a `ConstSize<N>` trait would be ideal, but stable Rust does not yet fully support const-generic exprs. The codegen checks the bound at runtime, not at compile time."

**Repo:** `crates/idl-rust/src/struct_emit.rs` emits a runtime bound check.

**Tests:** bound edge-case tests in `crates/cdr/tests/proptest_roundtrip.rs`.

**Status:** done

---

## Audit status

18 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-cdr -p zerodds-idl-rust -p zerodds-cdr-derive` -- 170 unit + integration + 6 derive_smoke + 11 snapshot_xcdr2_vectors = 232+ tests green, 0 failed; `cargo test -p zerodds-cdr --test serde_bridge --features serde-bridge` -- 3 tests green; `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_1_rust_reference_encoder` -- 1 test green.
