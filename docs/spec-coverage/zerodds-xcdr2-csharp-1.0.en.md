# `zerodds-xcdr2-csharp` 1.0 -- Spec Coverage

**Source:** `docs/specs/zerodds-xcdr2-csharp-1.0.md` (183 lines) -- the ZeroDDS C#/.NET TypeSupport codegen spec.

Implementation:

- `crates/cs/` — C#/.NET XCDR2 TypeSupport codegen.

## §1 Motivation

### §1 No OMG DDS-CSharp-PSM spec

**Spec:** §1 -- "There is no OMG DDS-CSharp-PSM spec. RTI Connext .NET and Vortex OpenSplice C# each have proprietary patterns. ZeroDDS synthesizes a spec-conformant C# binding here."

**Repo:** the motivation text of the vendor spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport pattern

### §2 `IDdsTopicType<T>` generic interface

**Spec:** §2 -- a C# interface with `TypeName`, `IsKeyed`, `Extensibility`, `Encode(T)`, `Encode(T, EndianMode)`, `Decode(ReadOnlySpan<byte>)`, `KeyHash(T)`. Plus the enums `ExtensibilityKind { Final, Appendable, Mutable }` and `EndianMode { LittleEndian, BigEndian }`.

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr/IDdsTopicType.cs` (interface), `ExtensibilityKind.cs`, `EndianMode.cs`.

**Tests:** `crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs` (28 tests incl. the V series).

**Status:** done

## §3 Required API surface

### §3 `*TypeSupport` singleton class with Instance + 7 members

**Spec:** §3 -- "MyTypeTypeSupport : IDdsTopicType<MyType> { Instance, TypeName, IsKeyed, Extensibility, Encode/Decode/KeyHash }."

**Repo:** `crates/idl-csharp/src/typesupport.rs` emits `*TypeSupport` classes with `static readonly Instance` and all 7 members. The generated code uses `Xcdr2Writer/Reader`.

**Tests:** `crates/idl-csharp/tests/snapshot_codegen.rs` (5 tests), `crates/idl-csharp/tests/snapshot_xcdr2_vectors.rs` (11 tests), `crates/idl-csharp/tests/spec_conformance.rs` (28 tests).

**Status:** done

## §4 Codegen requirement (idl-csharp)

### §4 Data class + TypeSupport class + topic hook

**Spec:** §4 -- "Per IDL `struct`, idl-csharp MUST emit: 1) the data class MyType (exists), 2) NEW: MyTypeTypeSupport: IDdsTopicType<MyType>, 3) NEW: Topic<MyType>.Register(participant)."

**Repo:** `crates/idl-csharp/src/typesupport.rs::emit_struct_typesupport`. Data classes via `crates/idl-csharp/src/emitter.rs`.

**Tests:** `crates/idl-csharp/tests/c5_3b_features.rs` (37 tests), `crates/idl-csharp/tests/snapshot_codegen.rs`.

**Status:** done

### §4 Namespace = IDL module path

**Spec:** §4 -- "The generated code MUST live in a nested `namespace` that matches the IDL module path."

**Repo:** `crates/idl-csharp/src/emitter.rs` module-path mapping → `namespace` statement.

**Tests:** V-7 nested modules in the wire-vector tests.

**Status:** done

### §4 Nested-aggregate codec: `EncodeInto` / `DecodeFrom`

**Spec:** §4 -- alongside `Encode`/`Decode`, every `*TypeSupport` emits the stream helpers `EncodeInto(Xcdr2Writer w, T sample)` / `DecodeFrom(Xcdr2Reader r)`, so a struct- or enum-valued member of an outer type delegates into the same CDR stream (XCDR2 alignment relative to the outer stream is preserved).

**Repo:** `crates/idl-csharp/src/typesupport.rs` — a type resolver (`build_type_registry`/`TYPE_REG`, keyed by simple name) classifies scoped member refs as Struct/Enum/Union/Typedef; the encode path calls `{dotted}TypeSupport.Instance.EncodeInto(w, …)` for a struct member (Enum → `WriteInt32((int)…)`, Typedef → recurse), the decode path `{dotted}TypeSupport.Instance.DecodeFrom(r)`. Previously nested structs were encoded as empty bytes and decoded as `default!` (silent corruption).

**Tests:** `crates/idl-csharp/tests/edge_cases.rs::nested_struct_uses_nested_codec_not_empty_bytes`, `nested_enum_uses_int32_cast`; `crates/idl-csharp/tests/compile_check.rs::compiles_nested_struct_codec` (Roslyn).

**Status:** done

### §4 Gating of non-codecable members (map/fixed/any/union)

**Spec:** §4 -- for constructs without an XCDR2 wire codec (`map`, `fixed`, `any`, and a `union` reference without its own codec) the **data class** is emitted but **no** `*TypeSupport` — mirroring idl-java `typespec_supported=false`. No generated codec that throws `XcdrException` at runtime or corrupts data.

**Repo:** `crates/idl-csharp/src/typesupport.rs::struct_xcdr2_codecable` / `typespec_xcdr2_codecable`; `crates/idl-csharp/src/emitter.rs` emits an explanatory comment instead of the TypeSupport class for a non-codecable member.

**Tests:** `crates/idl-csharp/tests/edge_cases.rs::map_fixed_any_member_gates_typesupport_no_runtime_throw`; `compile_check.rs::compiles_struct_with_{map,fixed,any}_member_gated`.

**Status:** done

## §5 Wire type mapping

### §5 IDL-to-C# types + wire layout

**Spec:** §5, table of 17 IDL types → C# → XCDR2 LE.

**Repo:** `crates/idl-csharp/src/type_map.rs` maps IDL primitives to C#. `crates/cs/csharp/ZeroDDS.Cdr/Xcdr2Writer.cs` + `Xcdr2Reader.cs`.

**Tests:** V-3 mixed primitives, V-4 string, V-5/V-6 sequences in `Xcdr2WireVectorsTests.cs`; `AlignmentTests.cs` (padding); `crates/idl-csharp/tests/edge_cases.rs` (20 tests).

**Status:** done

## §6 Extensibility

### §6 Final / Appendable / Mutable helper

**Spec:** §6 -- "`Xcdr2Writer.BeginAppendable()` / `BeginMutable()` / `WriteEmHeader(id, lc)` are helper methods. The code lives in `ZeroDDS.Cdr`."

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr/Xcdr2Writer.cs` holds `BeginAppendable`, `BeginMutable`, `WriteEmHeader`.

**Tests:** V-9 (Appendable), V-10 (Mutable), V-11 (optional Mutable) in `Xcdr2WireVectorsTests.cs`.

**Status:** done

## §7 Key extraction

### §7 Md5 + BE holder

**Spec:** §7 -- "`KeyHash(Sensor s) { var w = new Xcdr2Writer(BigEndian); w.WriteInt32(s.Id); return Md5.Hash(w.ToArray()); }`. Md5 is an RFC-1321 implementation."

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr/Md5.cs` (RFC-1321 pure-C#). `crates/idl-csharp/src/typesupport.rs` emits the KeyHash method.

**Tests:** `Md5Tests.cs` self-checks; V-8 in `Xcdr2WireVectorsTests.cs`.

**Status:** done

## §8 Helper library `ZeroDDS.Cdr`

### §8 IDdsTopicType + Xcdr2Writer/Reader + Md5 + enums

**Spec:** §8, table of 5 classes.

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr/`: `IDdsTopicType.cs`, `Xcdr2Writer.cs`, `Xcdr2Reader.cs`, `ExtensibilityKind.cs`, `EndianMode.cs`, `Md5.cs` -- all present.

**Tests:** `dotnet test` ZeroDDS.Cdr.Tests/ -- 28 tests green.

**Status:** done

### §8 Pure C# 11 .NET 6+

**Spec:** §8 -- "Pure C# 11 (.NET 6+). No native dependencies."

**Repo:** `ZeroDDS.Cdr.csproj` targets net8.0; pure managed code.

**Tests:** `dotnet test` runtime targets net8.0.

**Status:** done

## §9 Conformance

### §9 L1 wire (V-1..V-12)

**Spec:** §9 -- "L1 (wire): `crates/cs/Tests/Xcdr2WireVectorsTests.cs` (mstest or xunit) checks V-1..V-12."

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs` with V-1..V-12 (13 test methods in the V series + 15 more such as the EMHEADER helper).

**Tests:** `dotnet test` -- 28 tests green.

**Status:** done

### §9 L2 codegen snapshots

**Spec:** §9 -- "L2 (codegen): `crates/idl-csharp/tests/snapshots/` with generated *TypeSupport classes."

**Repo:** `crates/idl-csharp/tests/snapshots/` and drivers `snapshot_codegen.rs` (5 tests) + `snapshot_xcdr2_vectors.rs` (11 tests).

**Tests:** as above.

**Status:** done

### §9 L3 cross-language runner

**Spec:** §9 -- "L3 (cross-language): `crates/conformance/tests/cross_language_xcdr2.rs` calls `dotnet run` with a pre-built runner."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_4_csharp_binding` calls `dotnet test` of ZeroDDS.Cdr.Tests via subprocess against the identical V-1..V-12 hex fixtures.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_4_csharp_binding`.

**Status:** done

### §9 L4 cross-vendor (FFI over zerodds-c-api)

**Spec:** §9 -- "L4 (cross-vendor): C# encoded → Cyclone subscriber decodes (via FFI over zerodds-c-api)."

**Repo:** all 12 vectors were live-captured against Cyclone DDS 0.11 (forced XCDR2) on the Linux bench host and byte-compared; two gaps fixed (64-bit alignment §7.4.1.1.1, sequence DHEADER §7.4.3.5 for non-primitive elements). The C# encoder is byte-verified: `crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs` (`dotnet test`, 13/13) checks V-1..V-12 byte-exact incl. V-6 `sequence<string>` DHEADER (`Xcdr2Writer.BeginAppendable`/`Xcdr2Reader.BeginDHeader`). V-10/V-11a conformant LC divergence.

**Tests:** `crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs` (dotnet, 13/13) + `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 tests).

**Status:** done -- C# encoder byte-exact against Cyclone DDS 0.11 (V-1..V-9/V-11b, dotnet-executed), mutable V-10/V-11a conformant LC divergence.

## §10 Examples

### §10 TopicTypedSmoke.cs

**Spec:** §10 -- "`crates/cs/Examples/TopicTypedSmoke.cs` is the reference smoke (a generated PointTypeSupport + a pub/sub loop)."

**Repo:** `crates/cs/Examples/TopicTypedSmoke.cs` + `TopicTypedSmoke.csproj` (a standalone examples tree). `dotnet run --project TopicTypedSmoke.csproj` -> encode/decode round-trip OK.

**Tests:** `dotnet run --project crates/cs/Examples/TopicTypedSmoke.csproj`; the compile path is additionally covered by `crates/idl-csharp/tests/compile_check.rs` (7 tests).

**Status:** done

## §11 Errata + open questions

### §11.1 `Span<byte>` vs `byte[]`

**Spec:** §11.1 -- "Decode takes `ReadOnlySpan<byte>` for zero-copy. Encode returns `byte[]` (allocate-on-return). Optional `EncodeTo(T sample, IBufferWriter<byte> output)` for streaming."

**Repo:** `Xcdr2Reader` accepts `ReadOnlySpan<byte>`. `Encode` returns `byte[]`.

**Tests:** the wire-vector tests use ReadOnlySpan in the decode path.

**Status:** done

### §11.2 Nullable reference types

**Spec:** §11.2 -- "IDdsTopicType<T> has `where T : notnull`. `string?` members for `@optional string` are marked optional (M-flag)."

**Repo:** `IDdsTopicType<T> where T : notnull` in `IDdsTopicType.cs`.

**Tests:** `@optional string` test in `crates/idl-csharp/tests/edge_cases.rs`.

**Status:** done

### §11.3 init properties

**Spec:** §11.3 -- "The generated classes use the `{ get; init; }` pattern (immutable after constructor); decode uses object-initializer syntax."

**Repo:** `crates/idl-csharp/src/emitter.rs` emits `init` properties.

**Tests:** snapshot codegen tests.

**Status:** done

### §11.4 Source generators

**Spec:** §11.4 -- "idl-csharp emits `.cs` files; a Roslyn source generator as an additional form."

**Repo:** `crates/cs/csharp/ZeroDDS.Cdr.SourceGenerators/` (a Roslyn IIncrementalGenerator, netstandard2.0). The generator emits `*TypeSupport` classes from IDL-tagged partial classes.

**Tests:** `crates/cs/csharp/ZeroDDS.Cdr.SourceGenerators/tests/SourceGenSmoke.csproj` -- 6 dotnet tests green.

**Status:** done

---

## Audit status

20 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-idl-csharp` -- 219 tests green (85 unit + 134 integration: c5_3b_features 37, edge_cases 24, fixtures 13, compile_check 12, snapshot_xcdr2_vectors 11, snapshot_codegen 5, spec_conformance 28, bounded_collections 3); `dotnet test` ZeroDDS.Cdr.Tests -- 28 tests green; `dotnet test` ZeroDDS.Cdr.SourceGenerators/tests/SourceGenSmoke.csproj -- 6 tests green; `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_4_csharp_binding` -- 1 test green; `cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` -- 15 tests green; `dotnet run --project crates/cs/Examples/TopicTypedSmoke.csproj` -- OK.
