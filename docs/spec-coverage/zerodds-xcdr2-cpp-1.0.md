# `zerodds-xcdr2-cpp` 1.0 -- Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-cpp-1.0.md` (213 Zeilen) -- ZeroDDS C++17 TypeSupport-Codegen-Spec.

## §1 Motivation

### §1 OMG-DDS-PSM-Cxx-Luecke fuer TypeSupport

**Spec:** §1 -- "OMG DDS-PSM-Cxx 1.0 mandatiert TypeSupport-Registrierung in `Participant::create_topic`, spezifiziert aber kein konkretes `topic_type_support<T>`-Trait."

**Repo:** Motivations-Text der Vendor-Spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport-Pattern

### §2 `topic_type_support<T>`-Forward in `dds::topic`

**Spec:** §2 -- "template <typename T> struct topic_type_support; // forward (Spezialisierung pro T)" in Namespace `dds::topic`.

**Repo:** `crates/cpp/include/dds/topic/TopicTraits.hpp` deklariert das Forward-Template; Spezialisierungen pro IDL-`struct` werden von idl-cpp-Codegen emittiert (`crates/idl-cpp/src/emitter.rs::emit_topic_type_support_specs`).

**Tests:** `crates/idl-cpp/tests/snapshot_codegen.rs` (10 tests) verifiziert die emittierte Form.

**Status:** done

## §3 Required Methods

### §3 type_name() / encode() / encode_be() / decode() / key_hash() / is_keyed() / extensibility()

**Spec:** §3 -- C++-Method-Signaturen: `static const char* type_name()`, `static std::vector<uint8_t> encode(const T&)`, `static std::vector<uint8_t> encode_be(const T&)`, `static T decode(const uint8_t*, size_t)`, `static std::array<uint8_t,16> key_hash(const T&)`, `static constexpr bool is_keyed()`, `static constexpr ::dds::core::policy::DataRepresentationKind extensibility()`.

**Repo:** `crates/idl-cpp/src/emitter.rs::emit_topic_type_support_for` (Zeile 1429ff) emittiert exakt diese 7 Methoden inkl. der `constexpr` fuer is_keyed/extensibility.

**Tests:** `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` (15 V-tests), `crates/idl-cpp/tests/snapshot_codegen.rs`, `crates/idl-cpp/tests/spec_conformance.rs` (30 tests).

**Status:** done

## §4 Codegen-Pflicht (idl-cpp)

### §4 Spezialisierung pro IDL-`struct` mit FQN

**Spec:** §4 -- "Pro IDL-`struct` (Top-Level oder Modul-nested) MUSS idl-cpp eine `topic_type_support<FQN>`-Spezialisierung emittieren. FQN ist `::Module::Sub::Struct`."

**Repo:** `crates/idl-cpp/src/emitter.rs::emit_topic_type_support_specs` iteriert ueber alle structs der TU; FQN-Emission via `cpp_fqn`-Variable in `emit_topic_type_support_for`.

**Tests:** `crates/idl-cpp/tests/fixtures.rs` (13 tests), `crates/idl-cpp/tests/snapshot_codegen.rs` (10 tests).

**Status:** done

### §4 Plain-Members + @optional + @key Codegen-Pflicht

**Spec:** §4 -- "Pflicht-Members (aus AST): Alle Plain-Members (Primitive, String, Sequence, Nested-Struct, Enum, Array). `@optional` / `@shared` ueber EMHEADER M-Flag (Mutable) bzw. Present-Flag-Byte (Final/Appendable). `@key`-Members in Key-Hash-Generation."

**Repo:** `crates/idl-cpp/src/emitter.rs` -- `emit_encode_fn`, `emit_decode_fn`, Key-Hash-Generierung. `crates/idl-cpp/src/c_mode.rs::emit_key_hash_body`.

**Tests:** V-8 (key), V-9/V-10 (extensibility), V-11 (optional) in `xcdr2_wire_vectors.rs`.

**Status:** done

### §4 Type-Name ohne fuehrendes `::`

**Spec:** §4 -- "Type-Name-Form: `Module::Sub::Struct` ohne fuehrendes `::`."

**Repo:** `crates/idl-cpp/src/emitter.rs::short_name` strip leading `::`.

**Tests:** V-7 `Outer::Inner::S` Assert in wire-vector-Test.

**Status:** done

## §5 Wire-Type-Mapping

### §5 IDL-zu-C++17-Typen + Wire-Layout

**Spec:** §5, Tabelle 18 IDL-Typen → C++ → XCDR2 LE.

**Repo:** `crates/idl-cpp/src/type_map.rs` mappt IDL-Primitive auf C++-Typen. `crates/cpp/include/dds/topic/xcdr2.hpp` `write_le<T>/read_le<T>` Templates.

**Tests:** V-3 Mixed Primitives, V-4 String, V-5 Sequence<int32>, V-6 Sequence<string>, sowie `compile_check.rs` (16 tests) und `edge_cases.rs` (15 tests).

**Status:** done

## §6 Extensibility

### §6 Final / Appendable / Mutable Mode-Encoder

**Spec:** §6 -- "Codegen-Default: @appendable. Helper-Library liefert pro Mode dedicated Writer-Klassen: `xcdr2::FinalWriter`, `xcdr2::AppendableWriter`, `xcdr2::MutableWriter`."

**Repo:** `crates/cpp/include/dds/topic/xcdr2.hpp` haelt FinalWriter/AppendableWriter/MutableWriter. `crates/idl-cpp/src/emitter.rs::struct_extensibility` -> emit_encode_fn dispatch.

**Tests:** V-1..V-8 (Final), V-9 (Appendable), V-10/V-11 (Mutable) in `xcdr2_wire_vectors.rs`.

**Status:** done

## §7 Key-Extraction

### §7 PlainCdr2BeKeyHolder + MD5

**Spec:** §7 -- "Per XTypes §7.6.8: `PlainCdr2BeKeyHolder` (Big-Endian Plain-CDR2 ueber nur `@key`-Members), MD5 davon, 16 Bytes. Wenn is_keyed()==false: 16 Null-Bytes."

**Repo:** `crates/cpp/include/dds/topic/xcdr2_md5.hpp` (RFC-1321 MD5). `crates/idl-cpp/src/emitter.rs` emittiert key_hash() mit holder-write_be und md5().

**Tests:** V-8 in `xcdr2_wire_vectors.rs` (Key-Hash zero-padded fuer Holder ≤ 16 Byte; MD5-Pfad fuer >16 Byte ueber Helper-Self-Tests).

**Status:** done

## §8 Helper-Library

### §8 TopicTraits.hpp + xcdr2.hpp + xcdr2_md5.hpp

**Spec:** §8, Tabelle Header/Inhalt -- 3 Eintraege: `TopicTraits.hpp` (Forward + ByteSeq/string Defaults), `xcdr2.hpp` (Primitive-Helpers, Padding, DHEADER, EMHEADER), `xcdr2_md5.hpp` (RFC 1321).

**Repo:** `crates/cpp/include/dds/topic/TopicTraits.hpp`, `crates/cpp/include/dds/topic/xcdr2.hpp`, `crates/cpp/include/dds/topic/xcdr2_md5.hpp`.

**Tests:** `crates/idl-cpp/tests/clang_roundtrip.rs` (1 test, ignored on systems ohne clang); generierter Code wird gegen Header gelinkt.

**Status:** done

### §8 Pure C++17 header-only

**Spec:** §8 -- "Pure C++17, header-only. Kein Linking gegen Rust-Layer; Cross-Compile-fest."

**Repo:** Drei .hpp-Files ohne .cpp-Sibling. Keine Rust-FFI-Abhaengigkeit.

**Tests:** `compile_check.rs` (16 tests) compiliert generierten Output.

**Status:** done

## §9 Conformance

### §9 L1 Wire (V-1..V-12 byte-genau)

**Spec:** §9 -- "L1 (Wire): `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` prueft alle V-1..V-12 byte-genau."

**Repo:** `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` (15 #[test]-Funktionen).

**Tests:** v1_empty_final_struct, v2_plain_primitives_final, v3_mixed_primitives_final, v4_string_final, v5_sequence_int_final, v6_sequence_string_final, v7_nested_modules_final, v8_keyed_struct_final, v9_appendable_struct, v10_mutable_struct + 5 weitere.

**Status:** done

### §9 L2 Codegen Snapshots

**Spec:** §9 -- "L2 (Codegen): `crates/idl-cpp/tests/snapshots/` enthaelt Snapshot pro Vektor."

**Repo:** `crates/idl-cpp/tests/snapshots/` Dir mit Snapshot-Files; Treiber `crates/idl-cpp/tests/snapshot_codegen.rs` (10 tests).

**Tests:** `snapshot_codegen.rs` validiert generierte Outputs fuer alle V-Type-IDL-Inputs.

**Status:** done

### §9 L3 Cross-Lang Runner

**Spec:** §9 -- "L3 (Cross-Lang): `crates/conformance/tests/cross_language_xcdr2.rs` ruft `cpp_runner` auf."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs` haelt den Multi-Sprach-Driver inkl. C++-Binding-Aufruf via Subprocess gegen die idl-cpp Wire-Vector-Suite.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_2_cpp_binding`.

**Status:** done

### §9 L4 Cross-Vendor Cyclone

**Spec:** §9 -- "L4 (Cross-Vendor): `crates/discovery/tests/cyclone_xcdr2_cpp.rs` Roundtrip vs. Cyclone."

**Repo:** `tests/interop/xcdr2_cross_vendor.sh` orchestriert Cross-Vendor-Setup; Fixture-Tree `crates/discovery/tests/fixtures/cyclone-xcdr2/` haelt V-1..V-12 spec-derived + V-2 als recorded Cyclone-Capture (`v2_cyclone_recorded.bin`). idl-cpp-Encoder dispatcht ueber identische `crates/cdr`-Logik wie Rust; daher deckt `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` Cyclone-Equivalenz fuer C++ mit ab. Eine xcdr2-spezifische C++-Live-Roundtrip-Datei (`cyclone_xcdr2_cpp.rs`) ist noch nicht angelegt.

**Tests:** `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 Tests, Encoder-Logik shared mit idl-cpp).

**Status:** partial -- V-2 Cyclone-recorded; V-3..V-12 spec-derived; kein dedizierter cyclone_xcdr2_cpp.rs.

## §10 Examples

### §10 topic_typed_smoke.cpp

**Spec:** §10 -- "`crates/cpp/examples/topic_typed_smoke.cpp` ist der Referenz-Smoke (generierter `topic_type_support<Point>` + Pub/Sub-Loop)."

**Repo:** `crates/cpp/examples/topic_typed_smoke.cpp` (Smoke-Demo).

**Tests:** `compile_check.rs` deckt Compile-Pfad; ein Lauf-Test ist Sample-only.

**Status:** done

## §11 Errata + Open-Questions

### §11.1 wchar plattform-abhaengig

**Spec:** §11.1 -- "C++ wchar_t ist plattformabhaengig (4 Byte unter Linux, 2 Byte unter Windows). Codegen MUSS via `static_cast<uint16_t>` truncieren und bei Decode auf-extenden."

**Repo:** `crates/idl-cpp/src/emitter.rs` emit_member_write nutzt explicit u16-Cast fuer wchar.

**Tests:** wchar-Smoke in `crates/idl-cpp/tests/edge_cases.rs`.

**Status:** done

### §11.2 long double

**Spec:** §11.2 -- "XCDR2 spezifiziert kein long double-Wire. Codegen lehnt long double-Member mit Fehler ab."

**Repo:** `crates/idl-cpp/src/type_map.rs` lehnt long double per Validator.

**Tests:** Edge-Case-Test in `edge_cases.rs`.

**Status:** done

### §11.3 std::variant fuer union

**Spec:** §11.3 -- "idl-cpp emittiert IDL-`union` als `std::variant<...>`. Encoder schreibt Discriminator + selected-case nach XTypes §7.4.4.5."

**Repo:** `crates/idl-cpp/src/emitter.rs` union-Pfad.

**Tests:** Union-Tests in `crates/idl-cpp/tests/spec_conformance.rs`.

**Status:** done

---

## Audit-Status

17 done / 1 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-idl-cpp` -- 12 Test-Binaries gruen, 0 failed (139 unit + 16+15+13+11+15+0+30+10+21+7+15 in integration tests); `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_2_cpp_binding` -- 1 Test gruen; `cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` -- 15 Tests gruen.

Offene Items: `zerodds-xcdr2-cpp-1.0.open.md`.
