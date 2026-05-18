# `zerodds-xcdr2-c` 1.0 -- Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-c-1.0.md` (185 Zeilen) -- ZeroDDS C-FFI XCDR2-Encoding-Spec.

## §1 Motivation

### §1 Keine OMG-DDS-C-PSM-Spec

**Spec:** §1 -- "Es gibt keine OMG-DDS-C-PSM-Spec. Die existierende zerodds-c-api-1.0 deckt Entity-Lifecycle und QoS ab, aber nicht typisiertes Encoding."

**Repo:** Motivations-Text der Vendor-Spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport-Pattern

### §2 `zerodds_typesupport_t`-Struct mit Function-Table

**Spec:** §2 -- C-Struct mit `type_hash[16]`, `type_name`, `is_keyed`, `extensibility`, plus Function-Pointer `encode/decode/key_hash/sample_free`.

**Repo:** `crates/zerodds-c-api/src/xcdr2.rs` definiert `zerodds_typesupport_t` als `#[repr(C)]`-Struct in `crates/zerodds-c-api/include/zerodds/typesupport.h`.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_c_compile.rs` (11 tests) compilet generierten C-Code gegen Header.

**Status:** done

## §3 Required FFI-Functions

### §3 Topic + Writer/Reader + Standalone Encode/Decode

**Spec:** §3 -- 6 FFI-Funktionen: `zerodds_topic_create_typed`, `zerodds_writer_write_typed`, `zerodds_reader_take_typed`, `zerodds_xcdr2_encode`, `zerodds_xcdr2_decode`. Return-Codes 0=OK, -7=BAD_PARAMETER, -13=BUFFER_TOO_SMALL, -3=UNSUPPORTED.

**Repo:** `crates/zerodds-c-api/src/xcdr2.rs` exportiert die FFI-Funktionen. Status-Codes via zentralem `zerodds-c-api`-Status-Mapping.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` (13 tests) ruft die FFI-Encoders/Decoder; `crates/zerodds-c-api/tests/smoke_ffi.rs`.

**Status:** done

## §4 Codegen-Pflicht

### §4 idl-cpp `--c-mode` Codegen

**Spec:** §4 -- "Pro IDL-`struct` muss ein C-Codegen (idl-c, falls existent, oder als Aufgabe von idl-cpp via `extern C`-Wrapper) bereitstellen: Datenstruktur `MyType_t` + `extern const zerodds_typesupport_t MyType_typesupport`."

**Repo:** `crates/idl-cpp/src/c_mode.rs` (`emit_struct`, `emit_encode_body`, `emit_decode_body`, `emit_key_hash_body`, `emit_free_body`) -- volle C-Codegen-Path. `MyType_typesupport`-Static-Tabelle in emit_struct.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_c_codegen.rs` (12 tests) verifiziert generierten C-Output.

**Status:** done

## §5 Wire-Type-Mapping

### §5 IDL-zu-C99-Typen + Wire-Layout

**Spec:** §5, Tabelle 16 IDL-Typen → C99 → XCDR2 LE. Strings + Sequences sind heap-allocated; sample_free() MUSS sie freigeben.

**Repo:** `crates/idl-cpp/src/c_mode.rs::emit_primitive_write`, `emit_sequence_write`, `emit_member_write`. Heap-Allocation in `emit_decode_body` mit malloc.

**Tests:** V-4 (string), V-5/V-6 (sequence) in `xcdr2_wire_vectors.rs`; Memory-Free in `xcdr2_c_compile.rs`.

**Status:** done

## §6 Memory-Ownership

### §6 Caller/Callee-Vertrag fuer Encode/Decode/Free

**Spec:** §6, Tabelle 4 Eintraege -- Caller bietet `out_buf` (oder NULL fuer Size-Probe), Callee schreibt `out_len`; Decode allokiert Strings/Sequences im Sample; `ts.sample_free` gibt heap-Pointer frei.

**Repo:** `crates/zerodds-c-api/src/xcdr2.rs::zerodds_xcdr2_encode` Size-Probe mit NULL `out_buf` (returns required size in `out_len`). Decode-Pfad allokiert via Codegen-emittierter `MyType_decode`.

**Tests:** Size-Probe-Test + Free-Path-Test in `xcdr2_c_compile.rs`.

**Status:** done

## §7 Conformance

### §7 L1 Wire (V-1..V-12 byte-genau via FFI)

**Spec:** §7 -- "L1 (Wire): `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` prueft V-1..V-12 byte-genau via FFI."

**Repo:** `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` (13 tests).

**Tests:** dito.

**Status:** done

### §7 L2 Codegen

**Spec:** §7 -- "L2 (Codegen): C-Codegen ist Teil von idl-cpp (`--c-mode`-Flag) ODER separate idl-c-Crate."

**Repo:** `crates/idl-cpp/src/c_mode.rs` als Bestandteil von idl-cpp. Treiber `crates/zerodds-c-api/tests/xcdr2_c_codegen.rs` (12 tests).

**Tests:** dito.

**Status:** done

### §7 L3 Cross-Lang

**Spec:** §7 -- "L3 (Cross-Lang): C-Encoder vs Rust-Decoder, C-Decoder vs Rust-Encoder."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_3_c_ffi_binding` ruft die zerodds-c-api Wire-Vector-Test-Suite per Subprocess gegen identische V-1..V-12-Hex-Fixtures.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_3_c_ffi_binding`.

**Status:** done

### §7 L4 Cross-Vendor

**Spec:** §7 -- "L4 (Cross-Vendor): C-FFI ueber RTPS gegen Cyclone DDS."

**Repo:** `tests/interop/xcdr2_cross_vendor.sh` orchestriert Cross-Vendor-Setup; Fixture-Tree `crates/discovery/tests/fixtures/cyclone-xcdr2/` haelt V-1..V-12 spec-derived + V-2 als recorded Cyclone-Capture (`v2_cyclone_recorded.bin`). C-FFI-Encoder dispatcht ueber dieselbe `crates/cdr`-Logik; daher deckt `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` Cyclone-Equivalenz fuer C-FFI mit ab. V-3..V-12 ohne live Cyclone-Capture.

**Tests:** `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 Tests, Encoder-Logik shared mit zerodds-c-api).

**Status:** partial -- V-2 Cyclone-recorded; V-3..V-12 spec-derived ohne Cyclone-Live-Capture.

## §8 Examples

### §8 C-Smoke-Demo

**Spec:** §8 -- "`#include zerodds.h` + generated `MyType_typesupport`, Topic-Create-Typed + Writer."

**Repo:** Smoke-Demo-Snippet im Spec-Body (illustrativ).

**Tests:** Compile-Pfad in `xcdr2_c_compile.rs`.

**Status:** done

## §9 Errata + Edge-Cases

### §9.1 const-Strings via malloc/free

**Spec:** §9.1 -- "MyType_t.text ist `char*` (mutable); decode allokiert per malloc und sample_free ruft free."

**Repo:** `crates/idl-cpp/src/c_mode.rs::emit_decode_body` malloc-Pfad; `emit_free_body` free-Pfad.

**Tests:** String-Member-Test in `xcdr2_c_codegen.rs`.

**Status:** done

### §9.2 Sequence-Bound-Pruefung

**Spec:** §9.2 -- "Generierter decode prueft Bound aus IDL `sequence<T, N>`-Annotation und faellt bei Verletzung mit -7 zurueck."

**Repo:** `crates/idl-cpp/src/c_mode.rs::emit_sequence_read` mit Bound-Check.

**Tests:** Bound-Edge-Case in `crates/idl-cpp/tests/edge_cases.rs`.

**Status:** done

### §9.3 C99 vs C++ ABI

**Spec:** §9.3 -- "Strukturen sind `extern C`-kompatibel; C++ Konsumenten linken direkt gegen C-FFI."

**Repo:** `#[repr(C)]`-Struct + `extern "C"`-Wrappers in `xcdr2.rs`.

**Tests:** `xcdr2_c_compile.rs` compiliert C99 + C++ Konsumenten gleichermassen.

**Status:** done

### §9.4 enum-Width int32_t

**Spec:** §9.4 -- "C-Codegen emittiert enum-Typen mit explizitem `int32_t`-Storage (nicht `int`) fuer ABI-Stabilitaet."

**Repo:** `crates/idl-cpp/src/c_mode.rs` enum-Pfad emittiert `int32_t`-typedef.

**Tests:** Enum-Tests in `xcdr2_c_codegen.rs`.

**Status:** done

---

## Audit-Status

13 done / 1 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-c-api` -- unittest 68 + smoke_ffi 1 + xcdr2_c_codegen 12 + xcdr2_c_compile 11 + xcdr2_wire_vectors 13 = 105 Tests gruen, 0 failed; `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_3_c_ffi_binding` -- 1 Test gruen; `cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` -- 15 Tests gruen.

Offene Items: `zerodds-xcdr2-c-1.0.open.md`.
