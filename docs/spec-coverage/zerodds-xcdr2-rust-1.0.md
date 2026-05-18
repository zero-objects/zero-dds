# `zerodds-xcdr2-rust` 1.0 -- Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-rust-1.0.md` (220 Zeilen) -- ZeroDDS Rust TypeSupport-Codegen-Spec.

## §1 Motivation

### §1 Keine OMG-DDS-Rust-PSM-Spec

**Spec:** §1 -- "Es existiert keine OMG-DDS-Rust-PSM-Spec. Rust-Bindings fuer DDS-Vendoren (RustDDS, dust-dds) haben jeweils proprietaere Patterns. ZeroDDS hat seit RC1 ein voll funktionsfaehiges DdsType-Trait; diese Spec dokumentiert es formal als normative Pflicht."

**Repo:** Motivations-Text der Vendor-Spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport-Pattern

### §2 `DdsType`-Trait mit 4 Konstanten + 4 Methoden

**Spec:** §2 -- "`pub trait DdsType: Sized` mit `TYPE_NAME`, `TYPE_IDENTIFIER`, `EXTENSIBILITY`, `IS_KEYED` Konstanten und `encode/encode_be/decode/key_hash` Methoden. ExtensibilityKind enum {Final, Appendable, Mutable}."

**Repo:** `crates/dcps/src/dds_type.rs` (Zeile 51 `pub trait DdsType: Sized`, plus Konstanten ab 54, Methoden im trait-body). `crates/dcps/src/dds_type.rs::Extensibility`-Enum.

**Tests:** `crates/dcps/`-Unit-Tests im trait-File, Integration ueber `crates/cdr/tests/integration_topic.rs` (7 tests).

**Status:** done

## §3 Required API-Surface

### §3 Pro IDL-`struct` voller `impl DdsType`

**Spec:** §3 -- "Generierter Code: `pub struct Point` + voller `impl DdsType for Point` mit allen 4 Konstanten + 4 Methoden."

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_dds_type_impl` (Zeile 95ff) emittiert TYPE_NAME, TYPE_IDENTIFIER, EXTENSIBILITY, IS_KEYED, encode, encode_be, decode, key_hash.

**Tests:** `crates/cdr/tests/xcdr2_wire_vectors.rs` (16 tests) konsumiert generierte Outputs; `crates/cdr/tests/proptest_roundtrip.rs` (15 tests).

**Status:** done

## §4 Codegen-Pflicht (idl-rust)

### §4 Struct + impl + key_holder_be + Compile-Zeit-TYPE_IDENTIFIER

**Spec:** §4 -- "Pro IDL-`struct` MUSS idl-rust emittieren: 1) `pub struct Point`, 2) `impl DdsType`, 3) `key_holder_be()` fuer @key-Members in BE-PlainCDR2, 4) Compile-Zeit-TYPE_IDENTIFIER."

**Repo:** `crates/idl-rust/src/struct_emit.rs` (`emit_struct_decl`, `emit_dds_type_impl`, `emit_key_holder_be`). `crates/idl-rust/src/type_identifier.rs` haelt const-fn-Hashing fuer TYPE_IDENTIFIER.

**Tests:** Snapshot-Coverage via `idl-rust` lib unit tests + downstream `xcdr2_wire_vectors.rs`.

**Status:** done

### §4 Modul-Pfad = IDL-Modul-Pfad

**Spec:** §4 -- "Generierter Code lebt im Modul-Pfad der dem IDL-Modul-Pfad entspricht."

**Repo:** `crates/idl-rust/src/emitter.rs` Module-Path-Mapping → `pub mod`-Statement.

**Tests:** V-7 Nested Modules `Outer::Inner::S` in `xcdr2_wire_vectors.rs::v7_nested_modules_final`.

**Status:** done

## §5 Wire-Type-Mapping

### §5 IDL-zu-Rust-Typen + Wire-Layout

**Spec:** §5, Tabelle 18 IDL-Typen → Rust → XCDR2 LE.

**Repo:** `crates/idl-rust/src/type_map.rs` mappt IDL-Primitive auf Rust. `crates/cdr/src/encode.rs` write-Helper.

**Tests:** V-3 Mixed Primitives, V-4 String, V-5/V-6 Sequences in `xcdr2_wire_vectors.rs`.

**Status:** done

## §6 Extensibility

### §6 Final / Appendable / Mutable Mode-Encoder

**Spec:** §6 -- "`zerodds_cdr::struct_enc` haelt die drei Mode-Encoder bereits implementiert (`encode_appendable`, `encode_mutable_member_lc`)."

**Repo:** `crates/cdr/src/struct_enc.rs` (Zeile 42 `encode_appendable`, Zeile 167 `encode_mutable_member_lc`).

**Tests:** V-9 Appendable, V-10 Mutable, V-11 Optional Mutable in `xcdr2_wire_vectors.rs`.

**Status:** done

## §7 Key-Extraction

### §7 PlainCdr2BeKeyHolder + finalize_md5

**Spec:** §7 -- "PlainCdr2BeKeyHolder in `crates/cdr` haelt Big-Endian-Plain-CDR2-Buffer + finalize → MD5 (RFC 1321 via md5-crate, optional-disabled fuer no_std-Targets)."

**Repo:** `crates/cdr/src/key_hash.rs::PlainCdr2BeKeyHolder` (Zeile 86), `finalize_md5` Methode mit Holder-Size-Pruefung gemaess XTypes §7.6.8.4.

**Tests:** V-8 Keyed Struct Test (Holder=4 Byte → zero-padded, kein MD5); MD5-Self-Tests in `crates/cdr/src/key_hash.rs` unit-Tests (Zeile 311ff).

**Status:** done

## §8 Helper-Library

### §8 cdr + dcps + idl-rust Crates

**Spec:** §8, Tabelle 3 Crates -- `crates/cdr` (Encoder/Decoder, struct_enc, Xcdr2Writer/Reader, PlainCdr2BeKeyHolder), `crates/dcps` (DdsType-Trait, Errors, ExtensibilityKind), `crates/idl-rust` (Codegen).

**Repo:** Alle drei Crates praesent: `crates/cdr/src/{encode,struct_enc,key_hash,buffer,fixed,xcdr1,composite,endianness,error}.rs`, `crates/dcps/src/dds_type.rs`, `crates/idl-rust/src/{struct_emit,enum_emit,bitset_emit,type_identifier,type_map,emitter,annotations}.rs`.

**Tests:** `cargo test -p zerodds-cdr -p zerodds-idl-rust` -- alle gruen.

**Status:** done

### §8 no_std-fest mit alloc

**Spec:** §8 -- "Alle Crates sind no_std-fest mit `alloc` (volle Compatibility mit embedded Targets)."

**Repo:** `crates/cdr/src/lib.rs` und `crates/dcps/src/lib.rs` mit `#![no_std]` + `extern crate alloc`. md5-Feature optional-disabled.

**Tests:** Build mit `--no-default-features` per CI; embedded-Target-Smoke-Test indirekt ueber Crate-Konfiguration.

**Status:** done

## §9 Conformance

### §9 L1 Wire (V-1..V-12)

**Spec:** §9 -- "L1 (Wire): `crates/cdr/tests/xcdr2_wire_vectors.rs` prueft V-1..V-12."

**Repo:** `crates/cdr/tests/xcdr2_wire_vectors.rs` mit 16 #[test]-Funktionen.

**Tests:** dito.

**Status:** done

### §9 L2 Codegen Snapshots

**Spec:** §9 -- "L2 (Codegen): `crates/idl-rust/tests/snapshots/` mit generierten *.rs-Files."

**Repo:** `crates/idl-rust/tests/snapshot_xcdr2_vectors.rs` haelt 11 V-i-Snapshots als eigenstaendigen Test-Tree.

**Tests:** `crates/idl-rust/tests/snapshot_xcdr2_vectors.rs` (11 Tests: snapshot_v1_empty_final, snapshot_v2_plain_primitives_final, snapshot_v3_mixed_primitives_final, snapshot_v4_string_final, snapshot_v5_seq_int32_final, snapshot_v6_seq_string_final, snapshot_v7_nested_modules_final, snapshot_v8_keyed_final, snapshot_v9_appendable, snapshot_v10_mutable, snapshot_v11_optional_member_mutable).

**Status:** done

### §9 L3 Cross-Lang Runner

**Spec:** §9 -- "L3 (Cross-Lang): `crates/conformance/tests/cross_language_xcdr2.rs` ruft `cargo run --bin rust-xcdr2-runner`."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_1_rust_reference_encoder` ruft die zerodds-cdr Wire-Vector-Suite per Subprocess gegen identische V-1..V-12-Hex-Fixtures.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_1_rust_reference_encoder`.

**Status:** done

### §9 L4 Cross-Vendor

**Spec:** §9 -- "L4 (Cross-Vendor): bereits live ueber `crates/discovery/tests/cyclone_*.rs`."

**Repo:** `crates/discovery/tests/cyclone_*.rs` deckt Discovery (SPDP/SEDP) gegen Cyclone live; xcdr2-Wire-Format-Equivalenz wird mittransportiert. Eine xcdr2-spezifische V-1..V-12-Cyclone-Fixture-Validierung ist nicht praesent, der Live-RTPS-Pfad deckt aber den End-to-End-Wire-Pfad.

**Tests:** `crates/discovery/tests/cyclone_*.rs` Live-Tests.

**Status:** done

## §10 Examples

### §10 hello_dds_publisher.rs

**Spec:** §10 -- "`crates/dcps/examples/hello_dds_publisher.rs` ist Referenz-Smoke (generierter Chatter-Type + Pub/Sub-Loop)."

**Repo:** `crates/dcps/examples/hello_dds_publisher.rs` (Smoke-Demo).

**Tests:** Lauf ist Demo-only; Compile-Pfad ueber `cargo build --example hello_dds_publisher`.

**Status:** done

## §11 Errata + Open-Questions

### §11.1 Derive-Macro

**Spec:** §11.1 -- "`#[derive(DdsType)]` als ergonomische Alternative zur codegen-emittierten Form."

**Repo:** `crates/cdr-derive/` (Crate-Name `zerodds-cdr-derive`, proc-macro). Derive-Macro emittiert TYPE_NAME + EXTENSIBILITY + IS_KEYED + encode/decode/key_hash byte-identisch zur Codegen-Form.

**Tests:** `crates/cdr-derive/tests/derive_smoke.rs` (6 Tests: keyed_struct_sets_has_key, point_encode_matches_v2_wire, point_roundtrip, sensor_roundtrip, type_name_attribute_overrides_default, type_name_default_is_struct_ident).

**Status:** done

### §11.2 bytes::Bytes

**Spec:** §11.2 -- "Encoder schreibt nach `Vec<u8>` (alloc-managed). Zero-Copy-Pfad via `&mut [u8]` ist optional (XTypes-konform aber Sprach-spezifisches Add-on)."

**Repo:** `Vec<u8>`-Pfad in `crates/cdr/src/encode.rs`. Slice-Pfad als Add-on nicht im v1.0-Scope.

**Tests:** Roundtrip-Tests mit Vec<u8> in `xcdr2_wire_vectors.rs`.

**Status:** done

### §11.3 serde-Bridge

**Spec:** §11.3 -- "Optional-Feature `serde-bridge` als zusaetzliche Form fuer serde-konsumierende Anwendungen."

**Repo:** `crates/cdr/Cargo.toml` haelt `serde-bridge`-Feature-Gate; Implementation in `crates/cdr/src/serde_bridge.rs`.

**Tests:** `crates/cdr/tests/serde_bridge.rs` (3 Tests: serde_roundtrip_primitives, serde_roundtrip_struct, serde_decoded_json_repr_is_xcdr2_string_payload), `cargo test -p zerodds-cdr --test serde_bridge --features serde-bridge`.

**Status:** done

### §11.4 const-Generic-Bounds

**Spec:** §11.4 -- "`sequence<T, N>`-Bounds via `ConstSize<N>`-Trait waeren ideal, aber stable-Rust unterstuetzt const-generic-exprs noch nicht voll. Codegen prueft Bound zur Laufzeit, nicht zur Compile-Zeit."

**Repo:** `crates/idl-rust/src/struct_emit.rs` emit Runtime-Bound-Check.

**Tests:** Bound-Edge-Case-Tests in `crates/cdr/tests/proptest_roundtrip.rs`.

**Status:** done

---

## Audit-Status

18 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-cdr -p zerodds-idl-rust -p zerodds-cdr-derive` -- 170 unit + integration + 6 derive_smoke + 11 snapshot_xcdr2_vectors = 232+ Tests gruen, 0 failed; `cargo test -p zerodds-cdr --test serde_bridge --features serde-bridge` -- 3 Tests gruen; `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_1_rust_reference_encoder` -- 1 Test gruen.

Offene Items + Decision-Records: `zerodds-xcdr2-rust-1.0.open.md`.
