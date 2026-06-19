# `zerodds-xcdr2-bindings-conformance` 1.0 -- Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` (462 Zeilen) -- ZeroDDS Cross-Language-Master-Spec, Single-Source-of-Truth für V-1..V-12-Wire-Vektoren und L1-L4-Konformanz-Levels.

Implementation:

- `crates/cdr/` — XCDR2 Cross-Language-Wire-Vektoren (V-1..V-12) + L1-L4-Konformanz.

## §1 Conformance-Levels

### §1 L1 -- Wire byte-genau

**Spec:** §1, Tabelle Level/Anforderung -- "Encoder/Decoder produziert/konsumiert XCDR2 §7.4 byte-genau. Pflicht-Prüfung gegen die §6-Wire-Test-Vektoren."

**Repo:** Pro Sprache eigener L1-Pfad. `crates/cdr/src/struct_enc.rs` (`encode_appendable`, `encode_mutable_member`), `crates/cdr/src/key_hash.rs` (`PlainCdr2BeKeyHolder`), `crates/idl-cpp/src/emitter.rs` (`emit_topic_type_support_for`), `crates/zerodds-c-api/src/xcdr2.rs`, `crates/idl-csharp/src/typesupport.rs`, `crates/idl-java/src/typesupport.rs`, `crates/idl-ts/src/lib.rs` (`emit_struct_typesupport`), `crates/idl-rust/src/struct_emit.rs`.

**Tests:** `crates/cdr/tests/xcdr2_wire_vectors.rs` (16 tests), `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` (15 tests), `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` (13 tests), `crates/idl-csharp/tests/snapshot_xcdr2_vectors.rs` (11 tests), `crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs` (13 tests in V-Reihe), `crates/java-omgdds/java/src/test/java/org/zerodds/cdr/Xcdr2WireVectorsTest.java` (16 tests), `crates/ts-node/test/xcdr2-wire-vectors.test.ts` (13 V-tests + 2 md5).

**Status:** done

### §1 L2 -- Codegen pro Sprache

**Spec:** §1 -- "Sprach-Codegen (idl-cpp/csharp/java/ts/rust) emittiert pro IDL-`struct` eine TypeSupport-Spezialisierung mit allen §3-Methoden."

**Repo:** `crates/idl-cpp/src/emitter.rs` (Funktion `emit_topic_type_support_specs`), `crates/idl-cpp/src/c_mode.rs` (C-FFI Codegen via `--c-mode`), `crates/idl-csharp/src/typesupport.rs`, `crates/idl-java/src/typesupport.rs`, `crates/idl-ts/src/lib.rs` (`emit_struct_typesupport`), `crates/idl-rust/src/struct_emit.rs` (`emit_dds_type_impl`).

**Tests:** `crates/idl-cpp/tests/snapshot_codegen.rs` (10 tests), `crates/idl-csharp/tests/snapshot_codegen.rs` (5 tests), `crates/idl-java/tests/snapshot_codegen.rs` (11 tests), `crates/idl-ts/tests/` (snapshot via lib unit tests, 0 dedicated -- Codegen-Tests in Snapshot-Crate-Walks integriert), `crates/idl-rust/tests/snapshots` (Codegen lib-internal).

**Status:** done

### §1 L3 -- Cross-Language Roundtrip

**Spec:** §1 -- "Bytes von Sprache A werden von Sprache B byte-identisch round-trippt für alle §6-Test-Types." §7 verlangt `crates/conformance/tests/xcdr2_cross_language/` mit pro-Sprache-Runners.

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs` hält den Multi-Sprach-Driver. Sprach-Bindings dispatcht über Subprocess-Aufrufe der jeweiligen Test-Suiten (`cargo`, `dotnet`, `mvn`, `npx tsx`) gegen die V-1..V-12-Hex-Fixtures aus §6.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_0_cross_language_equivalence_documented`, `l3_1_rust_reference_encoder`, `l3_2_cpp_binding`, `l3_3_c_ffi_binding`, `l3_4_csharp_binding`, `l3_5_java_binding`, `l3_6_typescript_binding`.

**Status:** done

### §1 L4 -- Cross-Vendor Cyclone DDS

**Spec:** §1 -- "Bytes von ZeroDDS werden von Cyclone DDS akzeptiert und vice versa. Pflicht für L3-zertifizierte Bindings." §8 referenziert `tests/interop/xcdr2_cross_vendor.sh` und `crates/discovery/tests/fixtures/cyclone-xcdr2/*.bin`.

**Repo:** Skript `tests/interop/xcdr2_cross_vendor.sh` + Fixture-Tree `crates/discovery/tests/fixtures/cyclone-xcdr2/*.bin`. **Alle 12 Vektoren** wurden live gegen Cyclone DDS 0.11 (erzwungenes XCDR2-DataRepresentation, tcpdump-Capture + RTPS-Payload-Extraktion) auf dem Linux-Bench-Host aufgenommen und byte-genau verglichen. Der Live-Vergleich deckte zwei Conformance-Gaps auf, beide gefixt: (1) 64-Bit-Primitive richteten auf 8 statt XCDR2-4 aus (§7.4.1.1.1; V-3/V-8), (2) Sequenzen/Arrays mit nicht-primitiven Elementen fehlte der DHEADER (§7.4.3.5; V-6 `sequence<string>`). V-2..V-9 + V-11b byte-genau gegen Cyclone; V-10/V-11a konforme LC-Divergenz (Spec §6 erlaubt, Decoder liest alle LCs). Fix + Byte-Verifikation über alle 6 Bindings.

**Tests:** `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 Tests); Encoder/Wire-Vektoren je Binding ausgeführt + byte-verifiziert: cdr/C/C++ (compile+run, Cyclone-Bytes), C# `Xcdr2WireVectorsTests` (dotnet 13/13), Java `Xcdr2WireVectorsTest` (mvn 16/16), TS `xcdr2-wire-vectors.test.ts` (node 19/19).

**Status:** done -- alle 12 Vektoren live gegen Cyclone DDS 0.11 verglichen; deterministische V-1..V-9/V-11b byte-genau, mutable V-10/V-11a konforme LC-Divergenz (Roundtrip-Interop). Capture-Verfahren auf dem Linux-Bench-Host reproduzierbar.

## §2 Gemeinsames TypeSupport-Schema

### §2 type_name()

**Spec:** §2, Tabelle -- "Liefert den DDS-Type-Name als String (Convention: `Module::Sub::Struct`, ASCII, max 256 Bytes)."

**Repo:** Rust `crates/dcps/src/dds_type.rs::TYPE_NAME`. C++ `crates/idl-cpp/src/emitter.rs::emit_topic_type_support_for` schreibt `static const char* type_name()`. C-FFI `crates/zerodds-c-api/src/xcdr2.rs` (`zerodds_typesupport_t.type_name`). C# `crates/idl-csharp/src/typesupport.rs::TypeName`. Java `crates/idl-java/src/typesupport.rs::getTypeName`. TS `crates/idl-ts/src/lib.rs` (`typeName`).

**Tests:** Type-Name-Strings gegen V-7-Wire-Vektor (`Outer::Inner::S`) in den `xcdr2_wire_vectors`-Test-Files je Sprache.

**Status:** done

### §2 encode(v) -> bytes

**Spec:** §2 -- "XCDR2 §7.4 Big-Endian (default) oder Little-Endian (Option). Ohne RTPS-Header, nur Payload."

**Repo:** Rust `DdsType::encode` / `encode_be` (`crates/dcps/src/dds_type.rs`). C++ `topic_type_support<T>::encode/encode_be` (`crates/idl-cpp/src/emitter.rs`). C `zerodds_xcdr2_encode` (`crates/zerodds-c-api/src/xcdr2.rs`). C# `Xcdr2Writer` + `Encode(sample, EndianMode)` (`crates/cs/csharp/ZeroDDS.Cdr/Xcdr2Writer.cs`). Java `Xcdr2Writer` + `encode(s, EndianMode)` (`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Writer.java`). TS `Xcdr2Writer` (`crates/ts-node/src/cdr/writer.ts`).

**Tests:** V-1..V-12 in den sprach-spezifischen wire-vector-Test-Files.

**Status:** done

### §2 decode(bytes) -> v

**Spec:** §2 -- "Inverse zu encode. Gibt strukturiertes Sample-Object zurück."

**Repo:** Rust `DdsType::decode`. C++ `topic_type_support<T>::decode`. C `zerodds_xcdr2_decode`. C# `Xcdr2Reader` (`crates/cs/csharp/ZeroDDS.Cdr/Xcdr2Reader.cs`). Java `Xcdr2Reader` (`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Xcdr2Reader.java`). TS `Xcdr2Reader` (`crates/ts-node/src/cdr/reader.ts`).

**Tests:** Roundtrip-Asserts in den V-Tests je Sprache.

**Status:** done

### §2 key_hash(v) -> [u8; 16]

**Spec:** §2 -- "MD5 über `PlainCdr2BeKeyHolder` der `@key`-Felder; voll auf Null wenn kein Key (XTypes §7.6.8). MD5 ist NICHT unconditional -- konditional pro Holder-Größe, siehe §3 / §7.6.8.4."

**Repo:** Rust `PlainCdr2BeKeyHolder::finalize_md5` (`crates/cdr/src/key_hash.rs`). C++ `xcdr2::md5` (`crates/cpp/include/dds/topic/xcdr2_md5.hpp`). C# `Md5` (`crates/cs/csharp/ZeroDDS.Cdr/Md5.cs`). Java `Md5` (`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/Md5.java`). TS `md5` (`crates/ts-node/src/cdr/md5.ts`). C-FFI über Rust-Layer.

**Tests:** V-8 Keyed Struct in jedem Sprach-Test, MD5-self-checks (`Md5Tests.cs`, ts-md5-tests).

**Status:** done

### §2 is_keyed()

**Spec:** §2 -- "true falls mindestens ein Member `@key` trägt."

**Repo:** Rust `IS_KEYED` const. C++ `static constexpr bool is_keyed()`. C `zerodds_typesupport_t.is_keyed`. C# `IsKeyed`. Java `isKeyed()`. TS `isKeyed`.

**Tests:** V-8-Asserts über alle Sprachen.

**Status:** done

### §2 extensibility_kind()

**Spec:** §2 -- "Final / Appendable / Mutable (XTypes §7.2.2.4.4)."

**Repo:** Rust `EXTENSIBILITY` (`crates/dcps/src/dds_type.rs`). C++ `extensibility()`. C `zerodds_typesupport_t.extensibility`. C# `Extensibility`. Java `getExtensibility()`. TS `extensibility`.

**Tests:** V-9 (Appendable), V-10 (Mutable), V-1..V-8 (Final) in jedem Sprach-Test.

**Status:** done

## §3 Wire-Format-Anker

### §3.1 XCDR2 §7.4 Encoding-Algorithmus

**Spec:** §3 -- "Alle Sprach-Encoder produzieren XCDR2 gemäß OMG XTypes 1.3 §7.4 (formal/2025-04-04)."

**Repo:** `crates/cdr/src/encode.rs`, `crates/cdr/src/struct_enc.rs` -- zentrale Reference-Implementation. Sprach-Bindings dispatchen auf identische Algorithmen.

**Tests:** V-1..V-12, plus `crates/cdr/tests/proptest_roundtrip.rs` (15 tests), `crates/cdr/tests/compliance_xcdr2.rs`.

**Status:** done

### §3.2 §7.4.1.5 Padding origin-relative

**Spec:** §3 -- "§7.4.1.5 Padding-Regeln (Alignment relativ zum Buffer-Start)."

**Repo:** `crates/cdr/src/buffer.rs` (BufferWriter::pad_to), Sprach-Helpers (`Xcdr2Writer.align(...)`).

**Tests:** V-3 Mixed Primitives (48 Byte mit Pad@6 und Pad@36), wire-vector-Test je Sprache.

**Status:** done

### §3.3 §7.4.2 PL_CDR2 EMHEADER

**Spec:** §3 -- "§7.4.2 PL_CDR2 für Mutable-Types (EMHEADER)."

**Repo:** `crates/cdr/src/struct_enc.rs::encode_mutable_member_lc`, Sprach-Writer `WriteEmHeader/writeEmHeader`.

**Tests:** V-10 Mutable Struct über alle Sprachen.

**Status:** done

### §3.4 §7.4.4.4 DHEADER für Appendable

**Spec:** §3 -- "§7.4.4.4 DHEADER für Appendable-Types."

**Repo:** `crates/cdr/src/struct_enc.rs::encode_appendable`, Sprach-Helpers `BeginAppendable/beginAppendable`.

**Tests:** V-9 Appendable Struct.

**Status:** done

### §3.5 §7.6.8 Key-Hash mit PlainCdr2BeKeyHolder

**Spec:** §3 -- "§7.6.8.4 Pflicht: Wenn der Holder ≤ 16 octets groß ist, ist Key_Hash der Holder-Inhalt mit zero-padding auf 16 octets. Sonst ist Key_Hash der MD5 des Holder-Inhalts. MD5 ist NICHT unconditional -- konditional pro Holder-Größe."

**Repo:** `crates/cdr/src/key_hash.rs::PlainCdr2BeKeyHolder::finalize_md5` enthält die Holder-Size-Prüfung. Sprach-Bindings rufen identische Logik (C++ `xcdr2_md5.hpp`, C# `Md5`, Java `Md5`, TS `md5`).

**Tests:** V-8 Keyed Struct (Holder=4 Byte → zero-padded, kein MD5).

**Status:** done

### §3.6 §7.4.3 Encoding-Header (4-Byte)

**Spec:** §3 -- "§7.4.3 Encoding-Header (4-Byte) für Wire-Frames mit Encapsulation. Default-Encoding: PLAIN_CDR2 LE (0x00 0x01 0x00 0x00)."

**Repo:** RTPS-Layer prepended Header in `crates/rtps/`; xcdr2-Bindings emittieren payload-only. Encoder-Bytes folgen ohne Header per Spec-Konvention §6.

**Tests:** V-Test-Vektoren sind explizit "ohne 4-Byte-Encoding-Header"; RTPS-Header-Komposition ist außerhalb dieser Spec.

**Status:** done

## §4 Annotations-Mapping

### §4 @final / @appendable / @mutable / @key / @id / @optional / @bit_bound / @external / @must_understand

**Spec:** §4, Tabelle Annotation/Wire-Effekt/Konformanz-Pflicht -- 9 Einträge.

**Repo:** Rust `crates/idl-rust/src/annotations.rs` + `struct_emit.rs`. C++ `crates/idl-cpp/src/emitter.rs::struct_extensibility`. C# `crates/idl-csharp/src/annotations.rs`. Java `crates/idl-java/src/annotations.rs`. TS `crates/idl-ts/src/lib.rs::struct_extensibility`. Pro-Sprache-Spec §4 listet die volle Sprach-Tabelle.

**Tests:** V-9/V-10/V-11 Coverage je Sprache (Final/Appendable/Mutable/Optional). `@key` via V-8.

**Status:** done

## §5 Type-Name-Konvention

### §5 Module::Sub::Struct mit `::`-Trennzeichen

**Spec:** §5 -- "Trennzeichen MUSS `::` sein (kein `/`, `.`, `_`). Encoding ASCII oder UTF-8 (XTypes §7.3.1.1.1 erlaubt UTF-8)."

**Repo:** Pro Sprach-Codegen: `idl-rust` `compose_type_name`, `idl-cpp` `short_name`, `idl-csharp` Module-Path-Join, `idl-java` Module-Path-Join, `idl-ts` Module-Path-Join.

**Tests:** V-7 `Outer::Inner::S` in jedem Sprach-Test.

**Status:** done

## §6 Wire-Test-Vektoren

### §6 V-1 Empty Final Struct

**Spec:** §6, V-1 -- IDL `@final struct Empty {};`, Wire 0 Bytes, Type-Name `"Empty"`.

**Repo:** Codegen-Pfad pro Sprache; identische Hex-Fixtures.

**Tests:** Rust `crates/cdr/tests/xcdr2_wire_vectors.rs::v1_empty_final_struct`, C++ `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` (V1-test), C `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs`, C# `Xcdr2WireVectorsTests.cs::V1_EmptyFinal_EncodesToZeroBytes`, Java `Xcdr2WireVectorsTest.java`, TS `xcdr2-wire-vectors.test.ts::V-1`.

**Status:** done

### §6 V-2 Plain Primitives Final

**Spec:** §6, V-2 -- `Point{x=1,y=-2}` → 8 Bytes `01 00 00 00 FE FF FF FF`.

**Repo:** dito, primitive write_le i32.

**Tests:** alle 7 Sprach-Test-Files V-2-Test.

**Status:** done

### §6 V-3 Mixed Primitives Final

**Spec:** §6, V-3 -- 48-Byte-Layout mit Pad@6 + Pad@36, dokumentierte Hex-Sequenz.

**Repo:** Padding via origin-relative Alignment in jedem Writer.

**Tests:** alle Sprach-V3-Tests.

**Status:** done

### §6 V-4 String Final

**Spec:** §6, V-4 -- `text="hello"` mit length-with-NUL=6.

**Repo:** `write_string` in jedem Writer (uint32 length+1 + bytes + NUL).

**Tests:** alle Sprach-V4-Tests.

**Status:** done

### §6 V-5 Sequence<int32> Final

**Spec:** §6, V-5 -- `[1,2,3]` als count + i32[].

**Repo:** `write_sequence` in jedem Writer.

**Tests:** alle Sprach-V5-Tests.

**Status:** done

### §6 V-6 Sequence<string> Final

**Spec:** §6, V-6 -- `["a","bc"]` mit 2-Byte-Pad zwischen den Elementen für Align(4).

**Repo:** Sequence-Writer + String-Writer mit element-pad.

**Tests:** alle Sprach-V6-Tests.

**Status:** done

### §6 V-7 Nested Modules Final

**Spec:** §6, V-7 -- `Outer::Inner::S{x=1234}` → 4 Bytes `D2 04 00 00`, Type-Name `"Outer::Inner::S"`.

**Repo:** Sprach-Modul-Verschachtelung im Codegen.

**Tests:** alle Sprach-V7-Tests.

**Status:** done

### §6 V-8 Keyed Struct (Final)

**Spec:** §6, V-8 -- `Sensor{id=42,value=3.14}` mit 16-Byte-Wire + Key-Hash zero-padded auf 16 Bytes (Holder-Größe=4 ≤ 16, daher kein MD5).

**Repo:** `key_hash` in jedem Sprach-Binding mit Holder-Size-Check.

**Tests:** alle Sprach-V8-Tests inkl. Key-Hash-Assert.

**Status:** done

### §6 V-9 Appendable Struct

**Spec:** §6, V-9 -- DHEADER=8 + 2 i32 Body.

**Repo:** `encode_appendable` / Sprach-Aequivalente.

**Tests:** alle Sprach-V9-Tests.

**Status:** done

### §6 V-10 Mutable Struct

**Spec:** §6, V-10 -- DHEADER=27, Reference-Encoder LC=4 universell mit NEXTINT-Prefix; Decoder akzeptiert LC 0-7.

**Repo:** `encode_mutable_member_lc` mit `LengthCode::Lc4` als Default.

**Tests:** alle Sprach-V10-Tests.

**Status:** done

### §6 V-11 Optional Member (Mutable)

**Spec:** §6, V-11 -- Sample-A `Some(7)` LC=4-Variante DHEADER=12; Sample-A LC=2-Alternative DHEADER=8; Sample-B `None` DHEADER=0.

**Repo:** Encoder-Wahl pro Sprache (Reference LC=4); Decoder akzeptiert beide Varianten.

**Tests:** alle Sprach-V11-Tests (Some + None).

**Status:** done

### §6 V-12 Mutable Sentinel End-Marker

**Spec:** §6, V-12 -- "XCDR2-Bindings DUERFEN keinen expliziten Sentinel emittieren -- die DHEADER-Größe begrenzt das Lesen."

**Repo:** Encoder schreiben kein PID_LIST_END; nur DHEADER-Bound.

**Tests:** alle Sprach-V12-Tests prüfen Abwesenheit des Sentinels.

**Status:** done

## §7 Cross-Language-Roundtrip-Tests

### §7 Test-Matrix `crates/conformance/tests/xcdr2_cross_language/`

**Spec:** §7 -- "tests/xcdr2_cross_language/ hält eine deklarative Test-Matrix mit `vectors.json` + `runner_*`-Skripten pro Sprache."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs` (Single-File-Driver, ruft pro-Sprache-Test-Suite per Subprocess; Hex-Fixtures aus §6). V-1..V-12-Bytes als Inline-Konstanten im Driver, kein separater `vectors.json`-File.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs` (7 Tests).

**Status:** done

## §8 Cross-Vendor (Cyclone DDS)

### §8 Cyclone-Cross-Vendor Skript + Fixtures

**Spec:** §8 -- "Pflicht ab L4. Test-Skript `tests/interop/xcdr2_cross_vendor.sh`. Cyclone-Test-Vektoren liegen in `crates/discovery/tests/fixtures/cyclone-xcdr2/*.bin`."

**Repo:** `tests/interop/xcdr2_cross_vendor.sh` als Orchestrator-Skript. `crates/discovery/tests/fixtures/cyclone-xcdr2/` hält die Fixtures (`v1.bin`..`v12.bin`). Alle 12 Vektoren wurden live gegen Cyclone DDS 0.11 (erzwungenes XCDR2) auf dem Linux-Bench-Host aufgenommen und byte-genau verglichen; der Vergleich deckte den 64-Bit-Alignment-Cap (§7.4.1.1.1) und den Sequence-DHEADER für nicht-primitive Elemente (§7.4.3.5) auf — beide gefixt, `v6.bin` korrigiert. V-2..V-9/V-11b byte-genau; V-10/V-11a konforme LC-Divergenz.

**Tests:** `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 Tests). Capture-Verfahren auf dem Linux-Bench-Host reproduzierbar.

**Status:** done -- alle 12 Vektoren live gegen Cyclone DDS 0.11 verglichen (V-1..V-9/V-11b byte-genau, V-10/V-11a konforme LC-Divergenz).

## §9 Errata + Edge-Cases

### §9.1 wstring UTF-16-LE

**Spec:** §9.1 -- "wstring als UTF-16-LE auf der Wire mit 2-Byte-Code-Units. Sprach-Bindings MUESSEN UTF-16 emittieren, nicht UTF-8."

**Repo:** `write_wstring` Sprach-Helper in jedem Binding (Rust `crates/cdr/src/encode.rs`, C++ `xcdr2.hpp`, C# `Xcdr2Writer`, Java `Xcdr2Writer`, TS `writer.ts`).

**Tests:** wstring-Smoke in Unit-Tests der Helper-Crates (`crates/cdr/src/lib.rs` 170 Unit-Tests inkl. wstring-Encode-Decode).

**Status:** done

### §9.2 Empty Mutable

**Spec:** §9.2 -- "DHEADER = 0 ist legal; kein EMHEADER folgt."

**Repo:** `encode_mutable` mit empty body schreibt DHEADER=0.

**Tests:** V-11B (Sample-B None) deckt DHEADER=0 für Mutable explizit ab.

**Status:** done

### §9.3 Sequence-Bound-Overflow

**Spec:** §9.3 -- "Decoder MUSS `bound`-Annotation prüfen und bei Verletzung Fehler werfen (XTypes §7.2.2.4.4.4.10)."

**Repo:** `crates/cdr/src/encode.rs` decode_sequence prüfen Bound; `crates/idl-cpp/src/emitter.rs` emittiert Bound-Check; analog C/C#/Java/TS Codegen.

**Tests:** Bound-Overflow-Asserts in `crates/cdr/tests/proptest_roundtrip.rs` und Codegen-Edge-Case-Files (`crates/idl-cpp/tests/edge_cases.rs`, `crates/idl-csharp/tests/edge_cases.rs`, `crates/idl-java/tests/edge_cases.rs`).

**Status:** done

### §9.4 Cycle-Sample @external

**Spec:** §9.4 -- "Self-referenzierende Types (`@external`) brauchen Heap-Indirektion; Wire-Format identisch zu Plain-Member."

**Repo:** Rust `Box<T>`, C++ `std::shared_ptr<T>`, C# / Java reference-types nativ. Codegen-Mapping in jedem Sprach-Emitter.

**Tests:** `@external`-Smoke in Codegen-Edge-Case-Tests; Wire-Identität implizit aus Plain-Member-Pfad.

**Status:** done

## §10 Versioning

### §10 1.0 Vendor-Spec

**Spec:** §10 -- "Diese Spec ist 1.0. Inkompatible Aenderungen → 2.0. Backward-Compatible (z.B. neue Test-Vektoren V-13ff) → 1.x."

**Repo:** Spec-File `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md`.

**Tests:** --

**Status:** n/a (informative)

## §11 Cross-Reference

### §11 Sprach-Spec-Dateien

**Spec:** §11, Tabelle Sprach-Spec/Datei -- 6 Einträge (cpp, c, csharp, java, ts, rust).

**Repo:** `docs/specs/zerodds-xcdr2-cpp-1.0.md`, `-c-1.0.md`, `-csharp-1.0.md`, `-java-1.0.md`, `-ts-1.0.md`, `-rust-1.0.md`.

**Tests:** --

**Status:** n/a (informative)

---

## Audit-Status

36 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-cdr -p zerodds-idl-rust -p zerodds-idl-cpp -p zerodds-c-api -p zerodds-idl-csharp -p zerodds-idl-java -p zerodds-idl-ts -p zerodds-conformance --test cross_language_xcdr2` -- 41 Test-Binaries grün, 0 failed; `dotnet test` ZeroDDS.Cdr.Tests -- 28 Tests grün; `npm run test:wire` -- 15 Tests grün; `mvn test` java-omgdds -- 34 Tests grün.
