# Language-Adapter Stub Elimination

**Status: complete.** All items (1, 1b, 2, 2c, 3, 4, 5, 6) are done. A final
cross-adapter scan finds no generated codec/marshalling that throws at runtime,
corrupts data, or drops input: every adapter either does the real work or fails
fast at codegen (idl-cpp/idl-java pattern). `long double` (f128) remains a
documented hard language blocker, and the dynamic `Any` codec is a tracked
future feature (gated at codegen, not a runtime stub). Verified against the real
toolchains — `javac` (java-omgdds + idl-java/runtime, no stubs), `dotnet`/Roslyn,
`tsc`.

Tracking list for the cross-adapter stub sweep: placeholder/skeleton codegen in
the language adapters that does not compile, silently drops input, or throws at
runtime instead of doing the work. Each item: analysis → impl → unit test →
e2e/compile test → docs. Worked serially, crate by crate.

Discovery method: the real-runtime `javac` compile test for idl-java
(`crates/idl-java/tests/compile_check.rs`) exposed item 1; a cross-adapter scan
for `/* args */`, `not implemented` runtime throws, `_ => {}` drops, and
template placeholders surfaced the rest.

## Items

### 1. idl-java — RPC Requester/Replier marshalling (CONFIRMED, does not compile)

`crates/idl-java/src/rpc.rs` emits a **concrete** `XRequester`/`XReplier` whose
bodies are placeholders: `handler.m(/* args */)`, `requester.sendRequest(new
Object[] { /* args */ })` (untyped `CompletableFuture<Object>`), JNI
`sendRequest(/* xcdr2_encode(args) */ new byte[0], …)`, and a sync wrapper
`return asyncName(...).get()` returning `Object` instead of the return type.
Proven non-compiling against the real runtime (`compile_check.rs` tests
`compiles_service_against_real_rpc_runtime`,
`compiles_service_with_struct_params_against_real_runtime`).

**Status:** done. The default (non-JNI) requester/replier now marshal a
type-erased tuple through the runtime `Requester`/`Replier`
(request `Object[]` of IN+INOUT, reply `Object[]{ ret, INOUT+OUT… }`), with
holder write-back and typed return casts. `compile_check.rs` rewritten to
compile generated code against the **real** runtime (java-omgdds +
idl-java/runtime, no stubs); the two service tests are now active and green
(15/15). Fixed in `rpc.rs` (`emit_requester_async_impl`, replier dispatch,
sync void+inout `return`, `request_args_array`).

### 1b. idl-java — JNI codegen XCDR2 arg marshalling (removed)

`crates/idl-java/src/rpc.rs` under `cfg!(feature = "jni")` emitted
`requesterFfi.sendRequest(/* xcdr2_encode(args) */ new byte[0], …)` and
`.thenApply(reply -> /* xcdr2_decode<TOut>(reply) */ null)`. The `jni` feature
was **off by default and enabled by nobody** (no crate/CI). It was first made
fail-fast (codegen `UnsupportedConstruct`), then removed outright.

**Status:** done (removed). The `jni` cargo feature, its `cfg!` codegen branch,
the orphaned `requester_with_jni_feature_*` test, the `cfg(not(feature="jni"))`
test gates, and the dead `crates/idl-java/runtime/jni/` Java scaffold (a JNI
binding loading the never-built `dds_java_jni` lib) are all gone. The Java PSM is
pure-Java by design (see `dds-java-psm-1.0`: an earlier JNI bridge was already
removed), so the leftover scaffold contradicted the chosen architecture. Only the
real type-erased RPC marshalling (item 1) remains; default path unaffected
(272 idl-java tests green).

### 2. idl-csharp — embedded type decls in an interface dropped

`crates/idl-csharp/src/emitter.rs` — `_ => {} // embedded types in a C#
interface — not implemented.` Nested type declarations inside an interface were
silently skipped.

**Status:** done. `emit_interface_stub` now emits embedded
`struct`/`enum`/`const`/`exception` nested in the C# interface (C# 8+ allows
nested types + constants), and `collect_in_def` walks interface exports so the
runtime usings (`Omg.Types`, `ZeroDDS.Cdr`) are pulled in. Verified against
Roslyn (.NET 8): `compile_check::compiles_interface_with_embedded_types` +
`edge_cases::interface_embedded_types_are_emitted_not_dropped`. 213 idl-csharp
tests green. Coverage idl4-csharp §7.5 (DE+EN+HTML) corrected (was falsely
"non-service interface unsupported / out of scope").

### 2c. idl-ts — nested struct/union encoded as int32 (silent corruption)

`crates/idl-ts/src/lib.rs` — for `TypeSpec::Scoped` the XCDR2 encode emits
`w.writeInt32(s.field as unknown as number)` and decode `r.readInt32() as
unknown as never`. Correct only for enums; for a **nested struct/union/typedef**
this silently writes one int32 instead of the nested body — wrong wire output,
no error. Root cause: TS codegen has no type-name→kind resolver. Worst kind of
stub (looks done, corrupts data).

**Status:** done. Added a type-kind resolver (`build_type_registry`/`TYPE_REG`)
and `encodeInto(w,…)`/`decodeFrom(r)` on the `DdsTopicType` runtime interface;
the Scoped arm now routes struct/union through the nested codec (shared CDR
stream → alignment-correct), resolves typedefs, and keeps enum/bitmask/bitset
as int32. tsc compile_check green (8/8); snapshots updated; regression tests
`tests/nested_codec.rs` (5). 165 idl-ts tests green.

### 3. idl-ts — fixed-point XCDR2 encode/decode runtime throw

`crates/idl-ts/src/lib.rs` — `TypeSpec::Fixed(_)` emits
`throw new Error("fixed-point XCDR2 encode/decode not implemented in codegen")`
instead of the wire codec.

**Status:** done (gated, consistent with idl-java). A struct/union with a
`fixed` member is gated — the data type is emitted, the TypeSupport is skipped
(comment in output) instead of a codec that throws at runtime. The Fixed arms
in encode/decode are now codegen errors (unreachable; defensive). Also fixed a
regression from item 2c: unions have no TS XCDR2 codec, so a struct containing a
union member is gated too (was emitting a reference to a non-existent
`UnionTypeSupport`). Tests: `nested_codec::struct_with_union_member_is_gated_not_broken`,
`compile_check::compiles_struct_with_{union,fixed,any}_member_gated`.

### 4. idl-ts — DDS-Any XCDR2 encode/decode runtime throw

`crates/idl-ts/src/lib.rs` — `TypeSpec::Any` emits a runtime throw instead of
the TypeIdentifier+value codec.

**Status:** done (gated, same mechanism as item 3 — `any` member → struct gated,
no runtime-throwing codec; Any arms are defensive codegen errors). A full
dynamic Any codec needs the TS dynamic-type runtime (TypeIdentifier+value) and
is tracked separately, not as a stub.

### 5. idl-csharp — Map/Fixed/Any XCDR2 codec runtime throw

`crates/idl-csharp/src/typesupport.rs` — encode emits
`throw new XcdrException("unsupported codegen TypeSpec…")` and decode
`throw new XcdrException("decode unsupported type")` for `Map`/`Fixed`/`Any`
(and an unsupported map-key throw). Same dishonest pattern as TS — generated
code that throws at runtime instead of gating at codegen.

**Status:** done (gated, consistent with idl-java/idl-ts). A struct with a
map/fixed/any member is gated in `emitter.rs` — the data type is emitted
(map→IDictionary, fixed→decimal, any→Omg.Types.Any), the TypeSupport is skipped
(comment in output) instead of a codec that throws `XcdrException`. The
throw arms in `typesupport.rs` are now unreachable defensive safety-nets.
Tests: `edge_cases::map_fixed_any_member_gates_typesupport_no_runtime_throw`,
`compile_check::compiles_struct_with_{map,fixed,any}_member_gated` (real
`dotnet`/Roslyn). 216 idl-csharp tests green.

### 6. idl-csharp — nested struct/union encoded as empty / decoded as default (silent corruption)

`crates/idl-csharp/src/typesupport.rs` — a nested struct member encodes as
`w.WriteBytes(((sample.X) as object) is byte[] ? … : Array.Empty<byte>())`
(always the empty branch → the nested data is dropped), and decodes via
`TypeSpec::Scoped(_) => "default!"` (returns a default value). The C# analogue
of item 2c — silent round-trip corruption of nested composites, masked by the
string-assert/compile-only tests. Root cause: no type-name→kind resolver in the
C# codegen.

**Status:** done. Added a C# type-kind resolver (`build_type_registry`/`TYPE_REG`)
+ `EncodeInto(w, sample)`/`DecodeFrom(r)` methods on each generated TypeSupport;
Encode/Decode delegate to them. Nested struct members route through
`<Name>TypeSupport.Instance.EncodeInto/DecodeFrom` (shared CDR stream), enums via
`(int)`/`(Enum)` int32 cast, typedefs resolve, unions gate the containing struct.
No `IDdsTopicType` interface change needed (the singleton `Instance` is the
concrete type). Also covers sequence-of-struct elements. Verified: edge_cases
(nested struct + enum asserts), compile_check::compiles_nested_struct_codec (real
Roslyn). 219 idl-csharp tests green.

## Honest-limitation policy (not stubs)

The cross-adapter approach for genuinely-unsupported XCDR2 constructs should be
the idl-cpp/idl-java pattern: a **codegen-time** rejection (`Err(unsupported)`)
or a typed gate — never generated code that throws at runtime or corrupts data.
`long double` is a legitimate hard language blocker (f128) and stays gated
(documented). The `Any` dynamic codec needs a per-language dynamic-type runtime;
if not implemented, it must fail at codegen, not at runtime.

## Not stubs (checked, no action)

- `crates/py` — pyo3 binding gated behind the `pyo3`/`extension-module` feature;
  without it the crate is a pure-Rust lib by design. Real API behind the flag.
- `crates/idl-cpp` `emit_interface_stub` — emits a real abstract C++ interface
  class (CORBA client-stub idiom); methods are pure virtual, compiles.
- `crates/idl-cpp` RPC templates — abstract typed `send_request_async = 0`
  facade delegating to the Rust RPC runtime; compiles, deliberate design.
