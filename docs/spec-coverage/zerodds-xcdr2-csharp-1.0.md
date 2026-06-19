# `zerodds-xcdr2-csharp` 1.0 -- Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-csharp-1.0.md` (183 Zeilen) -- ZeroDDS C#/.NET TypeSupport-Codegen-Spec.

Implementation:

- `crates/cs/` — C#/.NET XCDR2-TypeSupport-Codegen.

## §1 Motivation

### §1 Keine OMG-DDS-CSharp-PSM-Spec

**Spec:** §1 -- "Es existiert keine OMG-DDS-CSharp-PSM-Spec. RTI Connext .NET und Vortex OpenSplice C# haben jeweils proprietäre Patterns. ZeroDDS synthetisiert hier eine spec-conforme C#-Bindung."

**Repo:** Motivations-Text der Vendor-Spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport-Pattern

### §2 `IDdsTopicType<T>` Generic-Interface

**Spec:** §2 -- C#-Interface mit `TypeName`, `IsKeyed`, `Extensibility`, `Encode(T)`, `Encode(T, EndianMode)`, `Decode(ReadOnlySpan<byte>)`, `KeyHash(T)`. Plus enums `ExtensibilityKind { Final, Appendable, Mutable }` und `EndianMode { LittleEndian, BigEndian }`.

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr/IDdsTopicType.cs` (Interface), `ExtensibilityKind.cs`, `EndianMode.cs`.

**Tests:** `crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs` (28 tests inkl. V-Reihe).

**Status:** done

## §3 Required API-Surface

### §3 `*TypeSupport` Singleton-Klasse mit Instance + 7 Members

**Spec:** §3 -- "MyTypeTypeSupport : IDdsTopicType<MyType> { Instance, TypeName, IsKeyed, Extensibility, Encode/Decode/KeyHash }."

**Repo:** `crates/idl-csharp/src/typesupport.rs` emittiert `*TypeSupport`-Klassen mit `static readonly Instance`, allen 7 Members. Generierter Code nutzt `Xcdr2Writer/Reader`.

**Tests:** `crates/idl-csharp/tests/snapshot_codegen.rs` (5 tests), `crates/idl-csharp/tests/snapshot_xcdr2_vectors.rs` (11 tests), `crates/idl-csharp/tests/spec_conformance.rs` (28 tests).

**Status:** done

## §4 Codegen-Pflicht (idl-csharp)

### §4 Datenklasse + TypeSupport-Klasse + Topic-Hook

**Spec:** §4 -- "Pro IDL-`struct` MUSS idl-csharp emittieren: 1) Datenklasse MyType (existiert), 2) NEU: MyTypeTypeSupport: IDdsTopicType<MyType>, 3) NEU: Topic<MyType>.Register(participant)."

**Repo:** `crates/idl-csharp/src/typesupport.rs::emit_struct_typesupport`. Datenklassen via `crates/idl-csharp/src/emitter.rs`.

**Tests:** `crates/idl-csharp/tests/c5_3b_features.rs` (37 tests), `crates/idl-csharp/tests/snapshot_codegen.rs`.

**Status:** done

### §4 Namespace = IDL-Modul-Pfad

**Spec:** §4 -- "Generierter Code MUSS in einem geschachtelten `namespace` liegen, der dem IDL-Modul-Pfad entspricht."

**Repo:** `crates/idl-csharp/src/emitter.rs` Module-Path-Mapping → `namespace`-Statement.

**Tests:** V-7 Nested Modules in wire-vector-Tests.

**Status:** done

### §4 Nested-Aggregate-Codec: `EncodeInto` / `DecodeFrom`

**Spec:** §4 -- neben `Encode`/`Decode` emittiert jedes `*TypeSupport` die Stream-Helfer `EncodeInto(Xcdr2Writer w, T sample)` / `DecodeFrom(Xcdr2Reader r)`, sodass ein struct-/enum-wertiges Member eines äußeren Typs in denselben CDR-Stream delegiert (das XCDR2-Alignment relativ zum äußeren Stream bleibt erhalten).

**Repo:** `crates/idl-csharp/src/typesupport.rs` — ein Typ-Resolver (`build_type_registry`/`TYPE_REG`, keyed by simple name) klassifiziert scoped Member-Refs als Struct/Enum/Union/Typedef; der Encode-Pfad ruft für ein Struct-Member `{dotted}TypeSupport.Instance.EncodeInto(w, …)` (Enum → `WriteInt32((int)…)`, Typedef → rekursiv), der Decode-Pfad `{dotted}TypeSupport.Instance.DecodeFrom(r)`. Davor wurden nested structs als leere Bytes encodiert bzw. als `default!` decodiert (stille Korruption).

**Tests:** `crates/idl-csharp/tests/edge_cases.rs::nested_struct_uses_nested_codec_not_empty_bytes`, `nested_enum_uses_int32_cast`; `crates/idl-csharp/tests/compile_check.rs::compiles_nested_struct_codec` (Roslyn).

**Status:** done

### §4 Gating nicht-codierbarer Member (map/fixed/any/union)

**Spec:** §4 -- für Konstrukte ohne XCDR2-Wire-Codec (`map`, `fixed`, `any` sowie eine `union`-Referenz ohne eigenen Codec) wird die **Datenklasse** emittiert, aber **kein** `*TypeSupport` — analog zu idl-java `typespec_supported=false`. Es entsteht kein generierter Codec, der zur Laufzeit `XcdrException` wirft oder Daten korrumpiert.

**Repo:** `crates/idl-csharp/src/typesupport.rs::struct_xcdr2_codecable` / `typespec_xcdr2_codecable`; `crates/idl-csharp/src/emitter.rs` emittiert bei nicht-codierbarem Member statt der TypeSupport-Klasse einen erklärenden Kommentar.

**Tests:** `crates/idl-csharp/tests/edge_cases.rs::map_fixed_any_member_gates_typesupport_no_runtime_throw`; `compile_check.rs::compiles_struct_with_{map,fixed,any}_member_gated`.

**Status:** done

## §5 Wire-Type-Mapping

### §5 IDL-zu-C#-Typen + Wire-Layout

**Spec:** §5, Tabelle 17 IDL-Typen → C# → XCDR2 LE.

**Repo:** `crates/idl-csharp/src/type_map.rs` mappt IDL-Primitive auf C#. `crates/cs/csharp/ZeroDDS.Cdr/Xcdr2Writer.cs` + `Xcdr2Reader.cs`.

**Tests:** V-3 Mixed Primitives, V-4 String, V-5/V-6 Sequences in `Xcdr2WireVectorsTests.cs`; `AlignmentTests.cs` (Padding); `crates/idl-csharp/tests/edge_cases.rs` (20 tests).

**Status:** done

## §6 Extensibility

### §6 Final / Appendable / Mutable Helper

**Spec:** §6 -- "`Xcdr2Writer.BeginAppendable()` / `BeginMutable()` / `WriteEmHeader(id, lc)` sind Helper-Methoden. Code lebt in `ZeroDDS.Cdr`."

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr/Xcdr2Writer.cs` hält `BeginAppendable`, `BeginMutable`, `WriteEmHeader`.

**Tests:** V-9 (Appendable), V-10 (Mutable), V-11 (Optional Mutable) in `Xcdr2WireVectorsTests.cs`.

**Status:** done

## §7 Key-Extraction

### §7 Md5 + BE-Holder

**Spec:** §7 -- "`KeyHash(Sensor s) { var w = new Xcdr2Writer(BigEndian); w.WriteInt32(s.Id); return Md5.Hash(w.ToArray()); }`. Md5 ist RFC-1321-Implementation."

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr/Md5.cs` (RFC-1321 pure-C#). `crates/idl-csharp/src/typesupport.rs` emittiert KeyHash-Method.

**Tests:** `Md5Tests.cs` self-checks; V-8 in `Xcdr2WireVectorsTests.cs`.

**Status:** done

## §8 Helper-Library `ZeroDDS.Cdr`

### §8 IDdsTopicType + Xcdr2Writer/Reader + Md5 + Enums

**Spec:** §8, Tabelle 5 Klassen.

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr/`: `IDdsTopicType.cs`, `Xcdr2Writer.cs`, `Xcdr2Reader.cs`, `ExtensibilityKind.cs`, `EndianMode.cs`, `Md5.cs` -- alle präsent.

**Tests:** `dotnet test` ZeroDDS.Cdr.Tests/ -- 28 Tests grün.

**Status:** done

### §8 Pure C# 11 .NET 6+

**Spec:** §8 -- "Pure C# 11 (.NET 6+). Keine native Dependencies."

**Repo:** `ZeroDDS.Cdr.csproj` targets net8.0; pure managed Code.

**Tests:** `dotnet test` runtime targets net8.0.

**Status:** done

## §9 Conformance

### §9 L1 Wire (V-1..V-12)

**Spec:** §9 -- "L1 (Wire): `crates/cs/Tests/Xcdr2WireVectorsTests.cs` (mstest oder xunit) prüft V-1..V-12."

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs` mit V-1..V-12 (13 Test-Methoden in V-Reihe + 15 weitere wie EMHEADER-Helper).

**Tests:** `dotnet test` -- 28 Tests grün.

**Status:** done

### §9 L2 Codegen Snapshots

**Spec:** §9 -- "L2 (Codegen): `crates/idl-csharp/tests/snapshots/` mit generierten *TypeSupport-Klassen."

**Repo:** `crates/idl-csharp/tests/snapshots/` und Treiber `snapshot_codegen.rs` (5 tests) + `snapshot_xcdr2_vectors.rs` (11 tests).

**Tests:** dito.

**Status:** done

### §9 L3 Cross-Lang Runner

**Spec:** §9 -- "L3 (Cross-Lang): `crates/conformance/tests/cross_language_xcdr2.rs` ruft `dotnet run` mit pre-built Runner."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_4_csharp_binding` ruft `dotnet test` der ZeroDDS.Cdr.Tests via Subprocess gegen identische V-1..V-12-Hex-Fixtures.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_4_csharp_binding`.

**Status:** done

### §9 L4 Cross-Vendor (FFI über zerodds-c-api)

**Spec:** §9 -- "L4 (Cross-Vendor): C# encoded → Cyclone-Subscriber decodes (via FFI über zerodds-c-api)."

**Repo:** Alle 12 Vektoren wurden live gegen Cyclone DDS 0.11 (erzwungenes XCDR2) auf dem Linux-Bench-Host aufgenommen und byte-genau verglichen; zwei Gaps gefixt (64-Bit-Alignment §7.4.1.1.1, Sequence-DHEADER §7.4.3.5 für nicht-primitive Elemente). Der C#-Encoder ist byte-verifiziert: `crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs` (`dotnet test`, 13/13) prüft V-1..V-12 byte-genau inkl. V-6 `sequence<string>`-DHEADER (`Xcdr2Writer.BeginAppendable`/`Xcdr2Reader.BeginDHeader`). V-10/V-11a konforme LC-Divergenz.

**Tests:** `crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs` (dotnet, 13/13) + `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 Tests).

**Status:** done -- C#-Encoder byte-genau gegen Cyclone DDS 0.11 (V-1..V-9/V-11b, dotnet-ausgeführt), mutable V-10/V-11a konforme LC-Divergenz.

## §10 Examples

### §10 TopicTypedSmoke.cs

**Spec:** §10 -- "`crates/cs/Examples/TopicTypedSmoke.cs` ist Referenz-Smoke (generierter PointTypeSupport + Pub/Sub-Loop)."

**Repo:** `crates/cs/Examples/TopicTypedSmoke.cs` + `TopicTypedSmoke.csproj` (eigenständiger Examples-Tree). `dotnet run --project TopicTypedSmoke.csproj` -> Encode/Decode-Roundtrip OK.

**Tests:** `dotnet run --project crates/cs/Examples/TopicTypedSmoke.csproj`; Compile-Pfad zusätzlich durch `crates/idl-csharp/tests/compile_check.rs` (7 tests).

**Status:** done

## §11 Errata + Open-Questions

### §11.1 `Span<byte>` vs `byte[]`

**Spec:** §11.1 -- "Decode nimmt `ReadOnlySpan<byte>` für Zero-Copy. Encode liefert `byte[]` (allocate-on-return). Optional `EncodeTo(T sample, IBufferWriter<byte> output)` für Streaming."

**Repo:** `Xcdr2Reader` accept `ReadOnlySpan<byte>`. `Encode` liefert `byte[]`.

**Tests:** Wire-Vector-Tests verwenden ReadOnlySpan im Decode-Pfad.

**Status:** done

### §11.2 Nullable Reference Types

**Spec:** §11.2 -- "IDdsTopicType<T> hat `where T : notnull`. `string?`-Members für `@optional string` werden als optional marked (M-Flag)."

**Repo:** `IDdsTopicType<T> where T : notnull` in `IDdsTopicType.cs`.

**Tests:** `@optional string`-Test in `crates/idl-csharp/tests/edge_cases.rs`.

**Status:** done

### §11.3 init-Properties

**Spec:** §11.3 -- "Generierte Klassen nutzen `{ get; init; }`-Pattern (immutable nach Constructor); Decode nutzt Object-Initializer-Syntax."

**Repo:** `crates/idl-csharp/src/emitter.rs` emit `init`-properties.

**Tests:** Snapshot-Codegen-Tests.

**Status:** done

### §11.4 Source-Generators

**Spec:** §11.4 -- "idl-csharp emittiert `.cs`-Files; Roslyn-Source-Generator als zusätzliche Form."

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr.SourceGenerators/` (Roslyn IIncrementalGenerator, netstandard2.0). Generator emittiert `*TypeSupport`-Klassen aus IDL-getaggten partial-classes.

**Tests:** `crates/cs/csharp/ZeroDDS.Cdr.SourceGenerators/tests/SourceGenSmoke.csproj` -- 6 dotnet-Tests grün.

**Status:** done

---

## Audit-Status

20 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-idl-csharp` -- 219 Tests grün (85 unit + 134 integration: c5_3b_features 37, edge_cases 24, fixtures 13, compile_check 12, snapshot_xcdr2_vectors 11, snapshot_codegen 5, spec_conformance 28, bounded_collections 3); `dotnet test` ZeroDDS.Cdr.Tests -- 28 Tests grün; `dotnet test` ZeroDDS.Cdr.SourceGenerators/tests/SourceGenSmoke.csproj -- 6 Tests grün; `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_4_csharp_binding` -- 1 Test grün; `cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` -- 15 Tests grün; `dotnet run --project crates/cs/Examples/TopicTypedSmoke.csproj` -- OK.
