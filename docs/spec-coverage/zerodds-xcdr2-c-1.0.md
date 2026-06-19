# `zerodds-xcdr2-c` 1.0 -- Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-c-1.0.md` (185 Zeilen) -- ZeroDDS C-FFI XCDR2-Encoding-Spec.

Implementation:

- `crates/zerodds-c-api/` — C-FFI XCDR2-TypeSupport-Encoding.

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

### §6 Caller/Callee-Vertrag für Encode/Decode/Free

**Spec:** §6, Tabelle 4 Einträge -- Caller bietet `out_buf` (oder NULL für Size-Probe), Callee schreibt `out_len`; Decode allokiert Strings/Sequences im Sample; `ts.sample_free` gibt heap-Pointer frei.

**Repo:** `crates/zerodds-c-api/src/xcdr2.rs::zerodds_xcdr2_encode` Size-Probe mit NULL `out_buf` (returns required size in `out_len`). Decode-Pfad allokiert via Codegen-emittierter `MyType_decode`.

**Tests:** Size-Probe-Test + Free-Path-Test in `xcdr2_c_compile.rs`.

**Status:** done

## §7 Conformance

### §7 L1 Wire (V-1..V-12 byte-genau via FFI)

**Spec:** §7 -- "L1 (Wire): `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` prüft V-1..V-12 byte-genau via FFI."

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

**Spec:** §7 -- "L4 (Cross-Vendor): C-FFI über RTPS gegen Cyclone DDS."

**Repo:** `tests/interop/xcdr2_cross_vendor.sh` orchestriert Cross-Vendor-Setup; Fixture-Tree `crates/discovery/tests/fixtures/cyclone-xcdr2/` hält V-1..V-12. C-FFI-Encoder dispatcht über dieselbe `crates/cdr`-Logik; daher deckt `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` Cyclone-Äquivalenz für C-FFI mit ab. Alle 12 Vektoren wurden live gegen Cyclone DDS 0.11 (erzwungenes XCDR2) auf dem Linux-Bench-Host aufgenommen und byte-genau verglichen: V-2..V-9 + V-11b byte-genau (V-3/V-8 belegen den XCDR2-64-Bit-Alignment-Cap §7.4.1.1.1, V-6 den `sequence<string>`-DHEADER §7.4.3.5); V-10/V-11a konforme LC-Divergenz (kein Bug, Spec §6 erlaubt, Decoder liest alle LCs). Beide aufgedeckten Gaps (Alignment + Sequence-DHEADER) sind über alle 6 Bindings gefixt; `v6.bin` korrigiert.

**Tests:** `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 Tests); Encoder-Seite `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` (13 Tests) gegen die korrigierten Vektoren grün (V-6 mit DHEADER, V-3/V-8 4-Byte-aligned).

**Status:** done -- alle deterministischen Vektoren (V-1..V-9, V-11b) byte-genau gegen Cyclone DDS 0.11; mutable V-10/V-11a konforme LC-Divergenz (spec-erlaubt, Roundtrip-Interop). Per-Capture-Verfahren auf dem Linux-Bench-Host reproduzierbar.

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

### §9.2 Sequence-Bound-Prüfung

**Spec:** §9.2 -- "Generierter decode prüft Bound aus IDL `sequence<T, N>`-Annotation und fällt bei Verletzung mit -7 zurück."

**Repo:** `crates/idl-cpp/src/c_mode.rs::emit_sequence_read` mit Bound-Check.

**Tests:** Bound-Edge-Case in `crates/idl-cpp/tests/edge_cases.rs`.

**Status:** done

### §9.3 C99 vs C++ ABI

**Spec:** §9.3 -- "Strukturen sind `extern C`-kompatibel; C++ Konsumenten linken direkt gegen C-FFI."

**Repo:** `#[repr(C)]`-Struct + `extern "C"`-Wrappers in `xcdr2.rs`.

**Tests:** `xcdr2_c_compile.rs` compiliert C99 + C++ Konsumenten gleichermaßen.

**Status:** done

### §9.4 enum-Width int32_t

**Spec:** §9.4 -- "C-Codegen emittiert enum-Typen mit explizitem `int32_t`-Storage (nicht `int`) für ABI-Stabilität."

**Repo:** `crates/idl-cpp/src/c_mode.rs` enum-Pfad emittiert `int32_t`-typedef.

**Tests:** Enum-Tests in `xcdr2_c_codegen.rs`.

**Status:** done

---

## Audit-Status

14 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-c-api` -- unittest 68 + smoke_ffi 1 + xcdr2_c_codegen 12 + xcdr2_c_compile 11 + xcdr2_wire_vectors 13 = 105 Tests grün, 0 failed; `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_3_c_ffi_binding` -- 1 Test grün; `cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` -- 15 Tests grün.
