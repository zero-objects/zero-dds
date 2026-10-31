# DDS-TS 1.0 — Spec Coverage

**Source:** `documentation/specs/dds-ts-1.0/main.tex`
(`zerodds/v1.0`, ZeroDDS vendor spec).

Audit item-by-item against the TeX source.

Implementation:

- `crates/lang-rules/` — the TypeScript rules, materialiser and emitter;
  the runtime under `runtime/typescript/src/`.

**The mechanism is a different one than when this spec was written.**
`crates/idl-ts/` existed until commit `d045e43ea86`: a codegen per language feeding a
descriptor runtime (`@zerodds/types` with a registry, type guards, `DdsAny`) that was
reflected against at run time. Today a shared walk
(`crates/lang-rules/src/emit/{encode,decode}.rs`) emits reader and writer code
directly, and the language file supplies only the language decisions. The generated
TypeScript imports `Reader`, `Writer` and the framing policies from
`runtime/typescript/src/`.

The descriptor runtime stands again, as the second half beside the codec rather than
in its place: `runtime/typescript/src/{types,branded,dds_any,registry,equal}.ts`
carries the Ch8 symbols, `wasm.ts` and `test_backend.ts` the Annex C profile, and
`crates/lang-rules/src/adapter/typescript.rs::ts_descriptors` emits the side table,
the type guard and the `registerType` call for every IDL type. The generated code
still imports `Reader` and `Writer` for the encoding; the descriptor describes the
type, it does not encode it.

**Module mapping:**

| Module | Spec area |
|---|---|
| `crates/lang-rules/src/rule/typescript.rs` | Ch7 naming, reserved words, type mapping |
| `crates/lang-rules/src/adapter/typescript.rs` | Ch7 declaring the generated types; §7.4.1–§7.8.1 descriptor side tables |
| `crates/lang-rules/src/emit/typescript.rs` | Ch7 rendering; the shared walk's language decisions |
| `crates/lang-rules/src/default_typescript.rs` | §7.10 default values |
| `crates/lang-rules/src/emit/{encode,decode}.rs` | Ch9 wire format — shared across languages |
| `runtime/typescript/src/{reader,writer,endianness,policy,float128,error,index}.ts` | the XCDR runtime |
| `runtime/typescript/src/{types,branded,dds_any,registry,equal}.ts` | Ch8 descriptor runtime |
| `runtime/typescript/src/{wasm,test_backend}.ts` | Annex C WASM bindings |
| `tools/idlc/` | Ch6 toolchain |

Test run: `cargo test -p zerodds-lang-rules typescript` and
`runtime/typescript/run-tests.sh`.

---

## Status of Document

### Front matter

**Spec:** Status of Document — informative; explains
vendor status and source binding to `tsc` 5.x.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch1 Scope

### §1.1 Purpose

**Spec:** Ch1 §Purpose — defines a PSM for IDL 4.2 → TypeScript.

**Repo:** `tools/idlc/`, `crates/lang-rules/src/{rule,adapter,emit}/typescript.rs`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `n/a (informative)`

### §1.2 Out of Scope

**Spec:** Ch1 §Out of Scope — lists non-redefined areas
(XCDR, JS runtime, DDS-RPC wire).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch2 Conformance

### §2.1 IDL-Codegen Profile

**Spec:** Ch2 §IDL-Codegen Profile — five normative mandatory items
(quoted from TeX §sec:profile-codegen):

1. Type mapping from Ch7 (`ch:type-mapping`) for every primitive/
   constructed/template type of OMG IDL 4.2.
2. Annotation mapping from §7.12 (`sec:annotation-mapping`) for every
   X-Types-1.3 annotation.
3. Type descriptor + type-guard function per emitted type
   (Ch8 + §7.4 struct mapping).
4. Output is valid TypeScript-≥-5.0 source, compilable with
   `tsc --strict` without `experimentalDecorators` /
   `emitDecoratorMetadata`.
5. Annex-B §B.1 conformance tests passed.

**Repo:** `crates/lang-rules/src/rule/typescript.rs` (naming, reserved words),
`crates/lang-rules/src/adapter/typescript.rs` (`ts_items` for the types,
`ts_descriptors` for the side tables, `ts_items_with_descriptors` for both in one
namespace), `crates/lang-rules/src/emit/typescript.rs` (rendering),
`crates/lang-rules/src/generate.rs` (the driver path, which writes both halves plus
`descriptor_import`).

Points 1, 2 and 4 hold: the type mapping covers the construct set, the X-Types
annotations take effect through the shared walk, and the generated source compiles
under `tsc --strict` without decorator options — measured in
`crates/lang-rules/tests/typescript_descriptors.rs::the_generated_types_type_check`
over nine shapes. Point 3 holds: `ts_descriptors` writes `<Type>Type`, `is<Type>` and
`registerType(<Type>Type)` for every type.

Point 5 holds: every one of the sixteen Annex B §B.1 marks has a test, the
class-freedom clause of B.1.2 included — `pair` writes `interface P` beside
`function newP(): P` and no class.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::the_generated_types_type_check`,
`::b_1_1_every_primitive_round_trips`,
`::b_1_2_a_struct_gets_an_interface_a_descriptor_and_a_guard`;
`crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`,
`crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`,
`crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`.

**Status:** `done`

### §2.2 Descriptor-Runtime Profile

**Spec:** Ch2 §Descriptor-Runtime Profile — mandatory surface:
`@zerodds/types` library exports `DdsTypeDescriptor`,
`DdsMemberDescriptor`, `DdsTypeRef`, `ExtensibilityKind`,
`DdsAny`, `DdsException`, `Char`, `WChar`, `makeChar`,
`makeWChar`, plus a reflection API.

**Repo:** `runtime/typescript/src/types.ts` (`DdsTypeDescriptor`,
`DdsMemberDescriptor`, `DdsTypeRef`, `ExtensibilityKind`, `DescriptorKind`,
`PrimitiveName`, `DdsAny`, `DdsException`, `GuardedTypeOf`),
`runtime/typescript/src/branded.ts` (`Char`, `WChar`, `makeChar`, `makeWChar`,
`LongDouble`, `makeLongDouble`), `runtime/typescript/src/registry.ts`
(`registerType`, `lookupType`, `getKey`, `getTopic`, `withDefaults`, `isOneOf`),
`runtime/typescript/src/dds_any.ts` (`boxAny`, `unboxAny`),
`runtime/typescript/src/equal.ts` (`equalKey`, and the structural equivalence
`registerType` needs); all re-exported from `runtime/typescript/src/index.ts`.

The import path is `runtime/typescript/src/` rather than `@zerodds/types`. Ch8 calls
the package name vendor-specific in as many words and binds only the symbol names;
those are unchanged and unprefixed.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — the nine marks `"B.2.1"`, `"B.2.2"`, `"B.2.3"`, `"B.2.4"`, `"B.2.5"`, `"B.2.6"`, `"B.2.7"`, `"B.2.8"`, `"B.2.9"`, driven by `runtime/typescript/run-tests.sh`.

**Status:** `done`

### §2.3 Operations Profile

**Spec:** Ch2 §Operations Profile — mandatory surface: mapping from
Ch11 (interface → client/handler pair, ServiceDescriptor),
plus operations descriptor types (`ServiceDescriptor`,
`OperationDescriptor`, `OperationParameterDescriptor`,
`AttributeDescriptor`, `ParameterMode`, `isOneOf`).

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs::ts_interface_items` (a client/handler pair per interface), `runtime/typescript/src/operations.ts` (`ServiceDescriptor`, `OperationDescriptor`, `OperationParameterDescriptor`, `AttributeDescriptor`, `ParameterMode`, `hasAggregatedResult`)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_maps_an_interface_to_a_client_a_handler_and_a_descriptor`, `crates/lang-rules/tests/typescript_operations_profile.rs::the_generated_operations_profile_type_checks`

**Status:** done

### §2.4 WASM-Bindings Profile

**Spec:** Ch2 §WASM-Bindings Profile — codegen + descriptor runtime
+ handle types/sample types/DCPS operations from Annex C.

**Repo:** `runtime/typescript/src/wasm.ts` (`ParticipantHandle`, `TopicHandle`,
`PublisherHandle`, `SubscriberHandle`, `DataWriterHandle`, `DataReaderHandle`,
`DdsGuid`, `makeDdsGuid`, `SampleInfo`, `Sample`, `DataAvailableCallback`,
`WasmBackend`, `DdsBindings`, `bindWasmBackend`),
`runtime/typescript/src/test_backend.ts` (`createTestBackend`).

The codegen and descriptor-runtime profiles §2.4 sits on are §2.1 and §2.2. The
XCDR2 carrier across the boundary is `Uint8Array` and nothing else: `writeSample`
takes octets, `takeSamples` returns octets.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` — `"C.5.1"`, `"C.5.2"`, `"C.5.3"`, `"C.5.4"`, driven by `runtime/typescript/run-tests.sh`.

**Status:** `done`

---

## Ch3 Normative References

**Spec:** Ch3 — table of referenced standards.

**Repo:** —

None of the handle types, `DdsGuid`, `SampleInfo`, `Sample`, the `WasmBackend` trait, the DCPS operations or `bindWasmBackend` remain. The profile sits on the descriptor runtime and went with it.

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch4 Terms and Definitions

**Spec:** Ch4 — glossary (IDL, codegen, type/service descriptor, ESM).

**Repo:** —

None of the handle types, `DdsGuid`, `SampleInfo`, `Sample`, the `WasmBackend` trait, the DCPS operations or `bindWasmBackend` remain. The profile sits on the descriptor runtime and went with it.

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch5 Symbols and Abbreviations

**Spec:** Ch5 — abbreviation list.

**Repo:** —

None of the handle types, `DdsGuid`, `SampleInfo`, `Sample`, the `WasmBackend` trait, the DCPS operations or `bindWasmBackend` remain. The profile sits on the descriptor runtime and went with it.

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch6 Overview

### §6.1 Mapping Philosophy

**Spec:** Ch6 §Mapping Philosophy — informative; structural typing,
uniform interface, type descriptor, service descriptor.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### §6.2 Runtime Reflection Strategy

**Spec:** Ch6 §Runtime Reflection Strategy — informative;
side-table rationale with an OMG X-Types cross-reference.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### §6.3 Toolchain

**Spec:** Ch6 §Toolchain — `tsc ≥ 5.0`, ESM output,
`crates/idl-ts/` (until `d045e43ea86`, now `tools/idlc`).

**Repo:** `tools/idlc/`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `n/a (informative)`

### §6.4 Code-Style Toggle

**Spec:** Ch6 §Code-Style Toggle — informative note on
`--camelcase`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch7 Type Mapping (Normative)

### §7.1 Module Mapping

**Spec:** Ch7 §Module Mapping — IDL `module M { … }` SHALL →
`export namespace M { … }`. Nested with declaration merging.
Qualified refs as dotted paths. Optional `--module-per-file`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{module_chain, module_handle, scope_decls}` (module as `export namespace`, nested along the chain), `crates/lang-rules/src/emit/typescript.rs::module_prefix` (qualified names as dotted paths)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done` — `export namespace` with declaration merging
fully covered; qualified-name resolver via `typespec_to_ts`
scoped path. `--module-per-file` is an optional codegen flag
(out of scope of this wave, to be integrated in the idlc tool).

### §7.2 Constant Mapping

**Spec:** Ch7 §Constant Mapping — `const T name = expr;` SHALL
→ `export const name: T' = expr';`. Const evaluation, BigInt
suffix for 64-bit, decimal/hex/octal/binary, cyclic-ref reject.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{const_object, declare}`, `crates/lang-rules/src/emit/typescript.rs::{constant, literal, wide_int}`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.3 Primitive Types

**Spec:** Ch7 §Primitive Types — table with 14 IDL types →
TS mapping. `char/wchar → Char/WChar` (branded), `long double
→ LongDouble`, `any → DdsAny`.

**Repo:** `crates/lang-rules/src/emit/typescript.rs::{scalar, int, unsigned_int, wide_int}`, `crates/lang-rules/src/adapter/typescript.rs::ts_member_type`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.3.1 Bounded Strings

**Spec:** Ch7 §Bounded Strings — `string<N>`/`wstring<N>` SHALL
additionally emit an exported constant
`<Container>_<Field>_BOUND` carrying `N`; a standalone
bounded-string typedef emits `<Alias>_BOUND`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{bound_constants, side_constant}`,
called from `declare` (the structure, union and alias arms) and from
`ts_pending` (exception members). No alias is followed: a member
declared with a named typedef takes that typedef's own constant.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_3_1_a_bounded_string_states_its_bound`,
`::the_side_tables_and_their_codecs_type_check` (case `side-strings`)

**Status:** `done`

### §7.3.2 Branded Character Types

**Spec:** Ch7 §Branded Character Types — `Char`/`WChar` as
branded `string` with literal tag `__dds_brand: "char"|"wchar"`;
`makeChar` (ISO 8859-1 range), `makeWChar` (code-point counting,
surrogate reject).

**Repo:** `crates/lang-rules/src/emit/typescript.rs::code_point`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.3.3 Long Double Handling

**Spec:** Ch7 §long double Handling — `long double` SHALL →
`LongDouble { __dds_brand: "long_double"; bytes: Uint8Array(16) }`.
Default emit, `makeLongDouble` factory, byte order
implementation-defined. Diagnostic `DDS-TS-I001`.

**Repo:** `crates/lang-rules/src/emit/typescript.rs` and `runtime/typescript/src/float128.ts`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.3.4 any Mapping

**Spec:** Ch7 §any Mapping — `any` SHALL → `DdsAny { typeId; value }`.
`boxAny`/`unboxAny` helpers. Static-unknowability rule.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs` (`ANY_BOX`; `prelude`
declares `DdsAny` in the generated file, `type_at` maps `TypeKind::Any` onto it, and
`type_ref_expr` writes `{ kind: "any" }`), `runtime/typescript/src/types.ts`
(`DdsAny`), `runtime/typescript/src/dds_any.ts` (`boxAny`, `unboxAny`).

`prelude` declares `DdsAny` rather than importing it — TypeScript is structural, the
declaration and the runtime's are one type, and a generated file without descriptors
stays compilable with no import.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_10_an_any_member_is_a_box_and_never_any_or_unknown`,
`crates/lang-rules/src/adapter/typescript.rs::tests::an_any_member_takes_the_box_and_not_a_typescript_keyword`;
`runtime/typescript/tests/descriptor-runtime.ts` (B.2.4, B.2.5).

**Status:** `done`

### §7.4 Struct Mapping — Uniform Interface

**Spec:** Ch7 §Struct Mapping §Uniform Interface — IDL `struct`
SHALL → TS `interface`, **not** a class. Class promotion explicitly
forbidden.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{ts_items, declare, pair, FACTORY_PREFIX, factory_path}` (struct as `interface` beside `function new<Name>(): <Name>`; annotations reach the side table and never the shape of the interface)

Both clauses hold. The shape of the interface depends on no annotation, and no IDL
data type produces a TypeScript class: what a value needs to start from comes from a
factory function returning an object literal with every required member at its
Table 9 value. The read path's defaulter
(`crates/lang-rules/src/default_typescript.rs::new_local`) calls it through
`factory_path`. An `@optional` member gets no entry — §7.2.2.4.4.4.8.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_2_no_class_reaches_the_output_for_any_idl_data_type`, `::b_1_11_annotations_change_the_side_table_and_not_the_interface`, `crates/lang-rules/src/adapter/typescript.rs::tests::a_struct_is_an_interface_and_a_factory_and_never_a_class`, `crates/lang-rules/tests/typescript_reader_generates.rs::the_generated_reader_type_checks`

**Status:** `done`

### §7.4.1 Type Descriptor Side-Table

**Spec:** Ch7 §Type Descriptor Side-Table — per struct SHALL
emit: `<Type>Type: DdsTypeDescriptor<T>` constant,
`is<Type>` type guard, `registerType(<Type>Type)`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{ts_descriptors, ts_items_with_descriptors, push_descriptor, descriptor_object, field_object, guard_expr}`; `crates/lang-rules/src/generate.rs` (the driver path, which writes both halves plus `descriptor_import`)

The guard checks the presence and the type of every required member, does not require
an `@optional` one, and checks neither `@unit`, `@min` nor `@max`. The registration is
a module-scope statement (`zerodds_lang_cst::typescript::Decl::Statement`), after the
constant it names.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_2_a_struct_gets_an_interface_a_descriptor_and_a_guard`, `::b_1_11_annotations_change_the_side_table_and_not_the_interface`, `::the_generated_types_type_check`

**Status:** `done`

### §7.4.2 Exception Mapping

**Spec:** Ch7 §Exception Mapping — IDL `exception` SHALL → TS
`interface extends DdsException`, descriptor with
`kind: "exception"`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::declare`, `runtime/typescript/src/error.ts`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.5 Union Mapping

**Spec:** Ch7 §Union Mapping — discriminated union with
literal-narrowed `discriminator` (enum/boolean) resp. number
(integer disc); default case via `Exclude<…>` for enum/boolean,
runtime-only for integer; multiple labels per case.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{declare, discriminant_type, discriminant_type_of, discriminant_expr, excluding, explicit_labels}`
and the constant `DISCRIMINANT_PROPERTY` (= `discriminator`);
`crates/lang-rules/src/emit/typescript.rs::{discriminant, case_label}`
through the seams
`crates/lang-rules/src/emit/mod.rs::CodecTarget::{discriminant, case_label}`.

The property is named `discriminator`. The `default:` arm carries
`Exclude<DiscType, ExplicitLabels>`; for an enum or `boolean`
discriminator `Exclude` really removes the labels, and for an integer
discriminator the type stays the number — which is what the paragraph
"Compile-time exclusivity is partial" says. An enum discriminator
narrows to `typeof Tag.TAG_A`.

The interop suite's canonical JSON dialect still spells a discriminant
`_d`; that is `crates/wire-probe/src/canonical.rs` and not this
mapping. The octets written are unchanged (measured, see the closing
note).

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_5_a_union_arm_names_its_discriminator`,
`::s_7_5_an_enumeration_discriminator_narrows_on_the_enumerator`,
`::the_side_tables_and_their_codecs_type_check` (cases `side-union-integer`, `side-enum-union`),
`crates/lang-rules/src/adapter/typescript.rs::tests::{a_union_is_a_discriminated_union_narrowed_on_a_literal, a_union_discriminant_takes_the_discriminators_own_literal_form}`

**Status:** `done`

### §7.5.1 Union Descriptor Side-Table

**Spec:** Ch7 §Union Descriptor — `kind: "union"` descriptor with
synthetic discriminator field (`id: 0xFFFFFFFF`), `labels` property
per case member, `hasDefault`/`hasImplicitDefault` flags.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{type_spec, DISCRIMINATOR_ID, DISCRIMINANT_PROPERTY, exhausts_discriminator}`

The synthetic discriminator member comes first and carries `id: 4294967295`. Its
`name` is the property the arms actually have (`discriminator`, §7.5), so a consumer
reads the side table instead of knowing the name twice. `hasImplicitDefault` is computed from the
coverage for `boolean` and enum discriminators; an integer discriminator never counts
as covered.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_3_a_union_carries_its_labels_and_its_default_flags`, `::b_1_3_a_union_without_a_default_and_without_coverage_says_so`, `::b_1_3_an_enum_discriminator_the_cases_exhaust_has_no_implicit_default`

**Status:** `done`

### §7.6 Enum Mapping

**Spec:** Ch7 §Enum Mapping — IDL `enum` SHALL → `as const` object
+ string-literal-union type alias. **No** TS `enum` keyword.
`@value(N)` companion object `<Enum>Ordinal`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{declare, enum_alias_type, enum_ordinals, ordinal_path, enum_default_literal, enumerator_path}`;
`crates/lang-rules/src/emit/typescript.rs::enumerator_ordinal` through
the seam
`crates/lang-rules/src/emit/mod.rs::CodecTarget::enumerator_ordinal`.

What is emitted is `export const Colour = { RED: "RED", … } as const;`,
`export type Colour = (typeof Colour)[keyof typeof Colour];`,
`export const ColourOrdinal: Readonly<Record<Colour, number>>` and
`export const ColourFromOrdinal: ReadonlyMap<number, Colour>`. The
ordinal table is emitted without `@value(N)` too, with the implicit
values. `@bit_bound(N)` stays a descriptor field and does not touch the
surface.

The codec reads the value through `<Enum>Ordinal`, because there is no
number in the variable any more. Without that seam `enum-sparse-values`
wrote the octets `70 79 87` — the first code unit of `FAIL`, `OK` and
`WARN` — where the ledger holds `65535 1 100`.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_6_an_enumeration_is_a_string_literal_union_with_its_ordinals`,
`::s_7_6_implicit_ordinals_are_emitted_too`,
`::the_side_tables_and_their_codecs_type_check` (case `side-enum`),
`crates/lang-rules/tests/typescript_descriptors.rs::b_1_4_an_enum_gets_a_const_object_and_a_descriptor`,
`crates/lang-rules/src/adapter/typescript.rs::tests::an_enumeration_is_an_as_const_object_beside_a_string_literal_union`

**Status:** `done`

### §7.6.1 Enum Descriptor Side-Table

**Spec:** Ch7 §Enum Descriptor — `kind: "enum"` descriptor with
`bitBound` field, members as `DdsMemberDescriptor` with
ordinal default.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::type_spec` (the `TypeKind::Enumeration` arm)

Every enumerator becomes a `DdsMemberDescriptor` with
`type: { kind: "primitive", name: "int32" }` — the 32 bits of X-Types 1.3 §7.4.3.5.3 —
and its ordinal as `default`. `bitBound` carries the effective bit bound, 32 by
default.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_4_an_enum_gets_a_const_object_and_a_descriptor`, `::b_1_4_an_explicit_value_reaches_the_descriptor`

**Status:** `done`

### §7.7 Bitset Mapping

**Spec:** Ch7 §Bitset Mapping — IDL `bitset` SHALL → TS interface
+ `<Bitset>_<Field>_BITS` constants. Width ≤ 32 → `number`,
33–64 → `bigint`. Total > 64 fatal error.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::declare`
(the `TypeKind::Bitset` arm, one `side_constant` per named bitfield)
and `unsigned_holder` for the anchor; the 64-bit limit is refused by
the parser. An anonymous bitfield is padding and gets no constant.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_7_a_bitset_states_the_width_of_every_bitfield`,
`::the_side_tables_and_their_codecs_type_check` (case `side-bits`)

**Status:** `done`

### §7.7.1 Bitset Descriptor Side-Table

**Spec:** Ch7 §Bitset Descriptor — `kind: "bitset"` descriptor with
`type: { kind: "bitfield"; width: W }` per member.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{type_spec, bitfield_ref}` (the `TypeKind::Bitset` arm)

Every bitfield becomes a `DdsMemberDescriptor` with
`type: { kind: "bitfield", width: W }`; `bitBound` carries the sum of every width,
anonymous padding fields included. The 64-bit limit is the parser's to refuse.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_5a_a_bitset_widths_pick_their_holders_and_the_descriptor_totals_them`

**Status:** `done`

### §7.8 Bitmask Mapping

**Spec:** Ch7 §Bitmask Mapping — IDL `bitmask` SHALL → `as const`
object with shift expressions. `bit_bound ≤ 32` → `number` +
`(1 << K) >>> 0`; 33–64 → `bigint` + `1n << Kn`.
`<Bitmask>_BIT_BOUND` constant.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::declare`
(the `TypeKind::Bitmask` arm; `side_constant` from
`ast.effective_bit_bound`), `bit_literal` for the anchor-typed value,
`crates/lang-rules/src/emit/typescript.rs::bitmask_value` for the codec.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_8_a_bitmask_states_its_bit_bound`,
`::the_side_tables_and_their_codecs_type_check` (case `side-bits`),
`crates/lang-rules/src/adapter/typescript.rs::tests::a_bitmask_writes_its_bit_as_the_number_and_not_as_a_shift`

**Status:** `done`

### §7.8.1 Bitmask Descriptor Side-Table

**Spec:** Ch7 §Bitmask Descriptor — `kind: "bitmask"` descriptor.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::type_spec` (the `TypeKind::Bitmask` arm)

Every bit value becomes a `DdsMemberDescriptor` with
`type: { kind: "bitfield", width: 1 }` and the bit position as `default`;
`@position(P)` reaches it because `effective_bit_position` has already resolved it.
`bitBound` carries the effective bound.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_5b_a_bitmask_carries_its_positions_and_its_bound`, `::b_1_5b_a_wide_bitmask_and_an_explicit_position`

**Status:** `done`

### §7.8.2 Position Annotation

**Spec:** Ch7 §Position Annotation — `@position(P)` SHALL override
the bit position; conflicting positions parser-reject.

**Repo:** `crates/lang-rules/src/emit/typescript.rs::bitmask_value`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.9 Sequence and Array Mapping

**Spec:** Ch7 §Sequence and Array Mapping — bounded/unbounded →
`Array<T'>`; bounded with `<Container>_<Field>_BOUND`. Fixed-size
arrays with `_LENGTH`. Multi-dim with `_LENGTH_DIM<N>`.
Element mutability (`ReadonlyArray` opt-in). Empty sequences
round-trip as `[]`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{array_of, type_at}`
for `T[]`, `bound_constants` for `_BOUND`, `_LENGTH` and
`_LENGTH_DIM1`…`_LENGTH_DIM<k>` in declaration order; `default_expr`
gives a sequence `[]` and a fixed-length member
`Array.from({ length: n }, …)`.

`ReadonlyArray` is a MAY of the clause ("An implementation MAY emit
`ReadonlyArray` … when configured with `--readonly-collections`") and
at the same time a SHALL NOT without that configuration; what is
emitted is `Array`.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_9_a_bounded_sequence_and_an_array_state_their_sizes`,
`::the_side_tables_and_their_codecs_type_check` (case `side-collections`),
`crates/lang-rules/src/adapter/typescript.rs::tests::a_sequence_and_an_array_are_both_typescript_arrays`,
`crates/lang-rules/tests/typescript_codec.rs::a_sequence_counts_and_loops`

**Status:** `done`

### §7.9.1 Maps

**Spec:** Ch7 §Maps — IDL `map<K,V>` SHALL → `ReadonlyMap<K',V'>`.
`<Container>_<Field>_BOUND` for bounded; `keyEqualityHazard` flag
for non-value-equality keys; `--mutable-collections` flag.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::type_at`
(the `TypeKind::Map` arm → `ReadonlyMap<K', V'>`), `default_expr`
(`new Map()`), `guard_check_at` (`instanceof Map`), `bound_constants`
for `_BOUND`, `map_key_hazard` for `keyEqualityHazard`;
`crates/lang-rules/src/emit/typescript.rs::{value_type_of, routine, reading_map_body, entries_of_map, map_of_entries}`.

Never a `Record<…>`: it loses key uniqueness and commits to an
enumeration order. The codec still works on the entry sequence XTypes
1.3 §7.4.1.1.4 puts on the wire — a `Map` has no half-built entry
object for the read direction to fill key and value into one after the
other — and the conversion sits at the two ends of the generated
function. The octets written are unchanged (measured, see the closing
note).

`--mutable-collections` is a MAY of the clause and explicitly
non-default; what is emitted is `ReadonlyMap`.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_9_1_a_map_is_a_readonly_map_and_states_its_bound`,
`::the_side_tables_and_their_codecs_type_check` (cases `side-map`, `side-map-struct-key`, `side-map-in-sequence`),
`crates/lang-rules/src/adapter/typescript.rs::tests::a_map_member_is_a_readonly_map_over_a_staged_entry_type`,
`crates/lang-rules/tests/typescript_descriptors.rs::b_1_16_a_bounded_map_carries_its_bound_and_a_struct_key_its_hazard`

**Status:** `done`

### §7.10 Typedef Mapping

**Spec:** Ch7 §Typedef Mapping — simple/sequence/array aliases,
recursive resolution, forward references.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{module_chain, module_handle, scope_decls}` (module as `export namespace`, nested along the chain), `crates/lang-rules/src/emit/typescript.rs::module_prefix` (qualified names as dotted paths)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done` — TS `export type` aliases are module-scope-
order-free (verified via forward-ref test). Cycle reject is a
matter of the IDL front end (`zerodds-idl-lower` / `zerodds-idl-compose`, not here).

### §7.10.1 Typedef Descriptor Side-Table

**Spec:** Ch7 §Typedef Descriptor — `kind: "alias"` descriptor with a
single member named `value`; the type guard delegates to the referenced
type's guard.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{type_spec, GuardShape}` (the `TypeKind::Alias` arm; `GuardShape::Delegate` carries the target's guard)

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_13_a_typedef_is_an_alias_with_one_member_named_value`, `::b_1_13_a_wide_bit_bound_switches_the_alias_to_bigint`

**Status:** `done`

### §7.10.2 @bit_bound on Integer Typedefs

**Spec:** Ch7 §@bit_bound on Integer Typedefs — N ≤ 32 → `number`,
33–64 → `bigint` switch.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{module_chain, module_handle, scope_decls}` (module as `export namespace`, nested along the chain), `crates/lang-rules/src/emit/typescript.rs::module_prefix` (qualified names as dotted paths)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §7.11 Optional and Default Values

**Spec:** Ch7 §Optional and Default Values — `@optional` SHALL →
`?: T | undefined`; `@default(V)` SHALL → side-table
`<Type>_<Field>_DEFAULT` constant + descriptor `default` field.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{pair, default_constant, member_field}`;
`crates/lang-rules/src/default_typescript.rs::{explicit_default, member_default}`.

An `@optional` member carries both halves: the `?:` suffix and the
`| undefined` arm — under `exactOptionalPropertyTypes`
`retry?: number` and `retry?: number | undefined` are different
declarations. A `@default(V)` produces
`export const <Type>_<Field>_DEFAULT: T' = V;` from the same function
the defaulter takes the value from, and the `default` property of the
`DdsMemberDescriptor` beside it.

**Tests:** `crates/lang-rules/tests/typescript_side_tables.rs::s_7_11_a_default_reaches_a_side_table_constant`,
`::s_7_11_an_optional_member_is_a_question_mark_and_undefined`,
`::the_side_tables_and_their_codecs_type_check` (case `side-defaults`),
`crates/lang-rules/src/adapter/typescript.rs::tests::an_optional_member_is_a_question_mark_and_not_a_null`

**Status:** `done`

### §7.12 Annotation Mapping

**Spec:** Ch7 §Annotation Mapping — table of ~15 annotations
(`@id`, `@key`, `@optional`, `@default`, `@final`, `@appendable`,
`@mutable`, `@nested`, `@topic`, `@must_understand`, `@unit`,
`@min`, `@max`, `@hashid`, `@autoid`, `@verbatim`, `@bit_bound`)
→ descriptor field or TSDoc tag. Resolution order, TSDoc
rendering. Strict-annotations mode (`--strict-annotations` →
E004).

**Repo:** `crates/lang-rules/src/adapter/typescript.rs::{module_chain, module_handle, scope_decls}` (module as `export namespace`, nested along the chain), `crates/lang-rules/src/emit/typescript.rs::module_prefix` (qualified names as dotted paths)

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

---

## Ch8 Descriptor Runtime (Normative)

### §8.1 Extensibility Kind

**Spec:** Ch8 §Extensibility Kind — `ExtensibilityKind = "final"
| "appendable" | "mutable"` type.

**Repo:** `runtime/typescript/src/types.ts` (`ExtensibilityKind`)

The three values in lower case, because the descriptor is
JSON-roundtrippable and these are its values; they correspond one for one
to `FINAL`, `APPENDABLE` and `MUTABLE` of X-Types 1.3 §7.2.2.4.4.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` (B.2.1 — the
type list is a tuple `tsc` checks), `crates/lang-rules/tests/typescript_descriptors.rs::b_1_11_an_unannotated_struct_is_appendable_and_carries_no_topic`

**Status:** `done`

### §8.2 Type References (DdsTypeRef)

**Spec:** Ch8 §Type References — `DdsTypeRef` discriminated union
with kinds: `primitive`, `bitfield`, `string`, `sequence`, `array`,
`map`, `ref` (by name), `any`. Plus `PrimitiveName` type.

**Repo:** `runtime/typescript/src/types.ts` (`DdsTypeRef`, `PrimitiveName`); `crates/lang-rules/src/adapter/typescript.rs::{type_ref_expr, primitive_ref, bitfield_ref, named_ref, primitive_ref_name}` (the producing side)

All eight kinds. `ref` carries the qualified IDL name and not a
descriptor reference — that keeps the value graph acyclic and
JSON-roundtrippable and is what mutually recursive types need.

`PrimitiveName` carries one name past the list in §8.2: `int8`. IDL 4.2
§7.4.1.4.4.2 has `int8` alongside `uint8`, the list names `octet` — which
is `uint8` — and nothing at all for the signed one. Additive: every value
§8.2 names still means what §8.2 says.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_1_every_primitive_round_trips`, `::b_1_16_a_map_member_is_described_as_a_map`, `::b_1_8_char_and_wchar_reach_the_descriptor_as_themselves`, `::the_generated_types_type_check`; `runtime/typescript/tests/descriptor-runtime.ts` (B.2.8)

**Status:** `done`

### §8.3 Type and Member Descriptors

**Spec:** Ch8 §Type and Member Descriptors —
`DdsMemberDescriptor`, `DescriptorKind`, `DdsTypeDescriptor<T>`.

**Repo:** `runtime/typescript/src/types.ts` (`DdsMemberDescriptor`, `DescriptorKind`, `DdsTypeDescriptor`)

`labels` additionally admits `bigint`: a `union U switch (long long)` is
valid IDL 4.2, its case labels reach past 2^53, and a label list that
could not carry one would disagree with the type it describes. Additive,
like `int8` in §8.2.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` (B.2.1 through B.2.6), `crates/lang-rules/tests/typescript_descriptors.rs::the_generated_types_type_check`

**Status:** `done`

### §8.3.1 Exception Marker

**Spec:** Ch8 §Exception Marker — `DdsException` interface with
`__dds_exception: true`.

**Repo:** `runtime/typescript/src/types.ts` (`DdsException`)

A property and not a class: TypeScript's structural typing has nothing
else that separates an exception payload from an ordinary struct at
compile time.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — `"B.2.1"` (the export is in the list) and `"B.2.9"`
(`NotFound extends DdsException`).

**Status:** `done`

### §8.4 Boxed any (DdsAny)

**Spec:** Ch8 §Boxed any — `DdsAny { typeId; value }`.

**Repo:** `runtime/typescript/src/types.ts` (`DdsAny`), `runtime/typescript/src/dds_any.ts` (`boxAny`, `unboxAny`)

Both functions check the descriptor's type guard — on the way in and on
the way out. A box whose `typeId` does not describe its `value` is worse
than no box, because every consumer downstream trusts it.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — `"B.2.4"` (`boxAny`) and `"B.2.5"` (`unboxAny`).

**Status:** `done`

### §8.5 Branded Character Helpers

**Spec:** Ch8 §Branded Character Helpers — `Char`/`WChar` aliases
+ `makeChar`/`makeWChar` factories per §7.3.2.

**Repo:** `runtime/typescript/src/branded.ts` (`Char`, `WChar`, `makeChar`, `makeWChar`)

`makeChar` takes exactly one code unit in U+0000..U+00FF; `makeWChar`
counts code points rather than code units, so it admits the astral plane
and rejects unpaired surrogates.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — `"B.2.7"`.

**Status:** `done`

### §8.6 LongDouble

**Spec:** Ch8 §LongDouble — `LongDouble` type + `makeLongDouble`
per §7.3.3.

**Repo:** `runtime/typescript/src/branded.ts` (`LongDouble`, `makeLongDouble`)

The length check is the whole check: a binary128 has no invalid bit
pattern, and a factory that inspected the exponent would refuse values
IEEE 754 defines. The conversion from a decimal to those sixteen octets
is in `runtime/typescript/src/float128.ts`.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` (B.2.7), `crates/lang-rules/tests/typescript_descriptors.rs::b_1_9_a_long_double_is_never_a_number`

**Status:** `done`

### §8.7 Reflection API

**Spec:** Ch8 §Reflection API — exports: `registerType`,
`lookupType`, `getKey`, `getTopic`, `withDefaults`, `boxAny`,
`unboxAny`, `equalKey`, `isOneOf`. Plus `GuardedTypeOf` helper.
Structural equivalence for `registerType` defined
(deep-equal of all fields except `typeGuard`).

**Repo:** `runtime/typescript/src/registry.ts` (`registerType`, `lookupType`, `getKey`, `getTopic`, `withDefaults`, `isOneOf`), `runtime/typescript/src/dds_any.ts` (`boxAny`, `unboxAny`), `runtime/typescript/src/equal.ts` (`equalKey`, `descriptorsEqual`), `runtime/typescript/src/types.ts` (`GuardedTypeOf`)

`registerType` is silent on a structurally equivalent re-registration and
throws on a structurally different one; `descriptorsEqual` compares every
field deeply and `typeGuard` by reference identity. `equalKey` follows a
`ref` through `lookupType` and compares member by member, `NaN` equal to
itself and `+0` equal to `-0`; a map, a sequence and an array are not IDL
key types and throw rather than answer. `isOneOf` never throws.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — `"B.2.1"` through `"B.2.9"`; the reflection itself in `"B.2.2"`
(`lookupType`) and `"B.2.3"` (`getKey`).

**Status:** `done`

---

## Ch9 Verbatim Code-Generation Hook (Normative)

### §9.1 Annotation Form

**Spec:** Ch9 §Annotation Form — `@verbatim(language, placement,
text)` annotation; `language="ts"|"*"` recognition.

**Repo:** `crates/lang-rules/src/verbatim.rs::{language_matches, declaration_slots, file_slots}`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §9.2 Placement Kinds

**Spec:** Ch9 §Placement Kinds — all 6 PlacementKinds
(`BEGIN_FILE`, `BEFORE_DECLARATION`, `BEGIN_DECLARATION`,
`END_DECLARATION`, `AFTER_DECLARATION`, `END_FILE`).

**Repo:** `crates/lang-rules/src/verbatim.rs::{declaration_slots, file_slots}`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §9.3 Quoting and Escaping

**Spec:** Ch9 §Quoting and Escaping — IDL-string-escape resolution
before emission.

**Repo:** `crates/lang-rules/src/verbatim.rs::gather`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

### §9.4 Multi-Line Text + Multiple Annotations

**Spec:** Ch9 §Multi-Line + Multiple — newlines, source order,
no coalescing.

**Repo:** `crates/lang-rules/src/verbatim.rs::gather`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `done`

---

## Ch10 Code Style (Informative)

**Spec:** Ch10 — identifier casing, output-format recommendations,
file-layout options.

**Repo:** `crates/lang-rules/src/verbatim.rs::gather`

**Tests:** `crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`, `crates/lang-rules/tests/typescript_reader_generates.rs::both_directions`, `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`

**Status:** `n/a (informative)`

---

## Ch11 Interface and Operation Mapping (Normative)

### §11.1 Mapping Surface

**Spec:** Ch11 §Mapping Surface — IDL `interface I` SHALL →
`IClient` + `IHandler` + `IService: ServiceDescriptor`. **No**
`async` keyword on interface method signatures.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs::{ts_interface_items, decls_of}` — one `<I>Client`, one `<I>Handler` and the descriptor per interface

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_maps_an_interface_to_a_client_a_handler_and_a_descriptor`

**Status:** done

### §11.2 Operation Mapping

**Spec:** Ch11 §Operation Mapping — return → `Promise<R>`;
in/inout → method params; out/inout → result-object properties;
oneway → `Promise<void>`.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs::{signatures, operation_signature}` — `out` and `inout` parameters move into the resolved value of the `Promise`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_aggregates_out_parameters_into_the_resolved_value`

**Status:** done

### §11.3 Attribute Mapping

**Spec:** Ch11 §Attribute Mapping — `attribute T name` SHALL →
`get_name(): Promise<T>` + `set_name(v: T): Promise<void>`.
`readonly attribute` → no setter. `getraises`/`setraises`.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs::attribute_signatures` — `get_`/`set_`; a `readonly attribute` gets only the reader

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_maps_an_attribute_to_a_getter_and_a_setter`

**Status:** done

### §11.4 Exception Mapping

**Spec:** Ch11 §Exception Mapping — `raises (E)` → reject with
`Error` + `cause: E`. `isOneOf` helper.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs` (lines 395 and 420–424: the `raises` list per operation and per attribute direction in the descriptor), `runtime/typescript/src/operations.ts` (`isOneOf` inspects the caught rejection)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_writes_the_service_descriptor_of_eleven_five`

**Status:** done

### §11.5 Service Descriptor

**Spec:** Ch11 §Service Descriptor — `ServiceDescriptor<TC,TH>`,
`OperationDescriptor`, `OperationParameterDescriptor`,
`AttributeDescriptor`, `ParameterMode` types.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs::{descriptor, operation_descriptor}`, `runtime/typescript/src/operations.ts::ServiceDescriptor`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_writes_the_service_descriptor_of_eleven_five`

**Status:** done

### §11.6 Inheritance

**Spec:** Ch11 §Inheritance — `interface B : A` SHALL →
`BClient extends AClient` + `BHandler extends AHandler` +
`BService.inherits = [AService]`. Multi-inheritance allowed.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs` (lines 141 and 156: `extends` on the client and the handler; line 313: the descriptor's `inherits` array carries every parent in declaration order)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_maps_an_interface_to_a_client_a_handler_and_a_descriptor`

**Status:** done

### §11.7 Forward Declarations

**Spec:** Ch11 §Forward Declarations — `<interface_forward_dcl>`
SHALL NOT produce a separate declaration. Orphan → DDS-TS-E002.

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs` — a forward declaration without a definition is a fatal finding carrying the code `DDS-TS-E002`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::a_forward_declaration_without_a_definition_is_e002`

**Status:** done

---

## Annex A — IDL Examples (Informative)

**Spec:** Annex A — three examples (bounded sequence, mutable type,
bitset).

**Repo:** `crates/lang-rules/src/emit/interface_typescript.rs` — a forward declaration without a definition is a fatal finding carrying the code `DDS-TS-E002`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::a_forward_declaration_without_a_definition_is_e002`

**Status:** `n/a (informative)`

---

## Annex B — Compliance Test Suite (Normative)

### §B.1 Codegen-Profile Tests

**Spec:** Annex B §B.1 — 16 mandatory tests (B.1.1–B.1.16) against
codegen output.

**Repo:** `crates/lang-rules/tests/typescript_descriptors.rs` — 26 tests
named after the marks (`b_1_1_…` through `b_1_16_…`), plus
`the_generated_types_type_check`, which puts nine shapes through
`tsc --strict --noEmit` against the real runtime.

All sixteen marks hold. B.1.2 has two halves and both are measured: the
side table (`<Type>Type`, `is<Type>`, three typed members) and the clause
"No `class` keyword MUST appear in the generator output for any IDL data
type" — checked over eleven shapes with the comments stripped, because a
comment about the word is not a class.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::b_1_1_every_primitive_round_trips`,
`::b_1_2_a_struct_gets_an_interface_a_descriptor_and_a_guard`,
`::b_1_2_no_class_reaches_the_output_for_any_idl_data_type`,
`::b_1_3_a_union_carries_its_labels_and_its_default_flags`,
`::b_1_4_an_enum_gets_a_const_object_and_a_descriptor`,
`::b_1_5a_a_bitset_widths_pick_their_holders_and_the_descriptor_totals_them`,
`::b_1_5b_a_bitmask_carries_its_positions_and_its_bound`,
`::b_1_6_a_module_nests_its_declarations_and_its_descriptors`,
`::b_1_7_free_standing_constants_keep_their_types`,
`::b_1_8_char_and_wchar_reach_the_descriptor_as_themselves`,
`::b_1_9_a_long_double_is_never_a_number`,
`::b_1_10_an_any_member_is_a_box_and_never_any_or_unknown`,
`::b_1_11_annotations_change_the_side_table_and_not_the_interface`,
`::b_1_12_an_exception_is_described_as_one`,
`::b_1_13_a_typedef_is_an_alias_with_one_member_named_value`,
`::b_1_14_a_mixed_module_produces_every_side_table`,
`::b_1_15_an_unrecognised_annotation_does_not_stop_the_generator`,
`::b_1_16_a_map_member_is_described_as_a_map`,
`::the_generated_types_type_check`

**Status:** `done`

### §B.2 Descriptor-Runtime Tests

**Spec:** Annex B §B.2 — 9 mandatory tests against the `@zerodds/types`
library surface.

**Repo:** `runtime/typescript/tests/descriptor-runtime.ts` — B.2.1
through B.2.9, one block per mark. B.2.1 has two halves: the type list is
a tuple `tsc` checks, and the function list is checked at run time,
because a name that is exported and `undefined` compiles perfectly well.

**Tests:** `runtime/typescript/tests/descriptor-runtime.ts` — 9 of 9 marks: `"B.2.1"`, `"B.2.2"`, `"B.2.3"`, `"B.2.4"`, `"B.2.5"`, `"B.2.6"`, `"B.2.7"`, `"B.2.8"`, `"B.2.9"`; run via `runtime/typescript/run-tests.sh`.

**Status:** `done`

### §B.3 Operations-Profile Tests

**Spec:** Annex B §B.3 — 9 mandatory tests against operations-profile
output.

**Repo:** `crates/lang-rules/tests/interface_bindings.rs` (the six `typescript_*` markers) and `crates/lang-rules/tests/typescript_operations_profile.rs` (`tsc --strict` over the generated profile code)

**Tests:** `crates/lang-rules/tests/typescript_operations_profile.rs::the_generated_operations_profile_type_checks`

**Status:** done

### §B.4 Reference Test-Harness

**Spec:** Annex B §B.4 — test-harness definition (tsc verify on
generated output, Node.js runner for descriptor helpers).

**Repo:** `crates/lang-rules/tests/typescript_descriptors.rs` (the tsc
half: `--noEmit --strict` over the generated output),
`runtime/typescript/run-tests.sh` (the Node.js half: the descriptor
helpers and the WASM bindings under `node`).

The spec names the former codegen crate as the place; it is gone and both
halves of the harness are not. What §B.4 asks for — Rust tests against
the codegen output, `tsc --noEmit --strict` over it, and the runtime
helpers in a Node.js runner — stands in full.

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::the_generated_types_type_check`,
`runtime/typescript/tests/descriptor-runtime.ts`,
`runtime/typescript/tests/wasm-bindings.ts`

**Status:** `done`

---

## Annex C — WASM-Bindings Profile (Normative)

### §C.1 Handle and Sample Types

**Spec:** Annex C §C.1 — branded handle types (Participant/Topic/
Publisher/Subscriber/Writer/Reader with string-literal brands),
`SampleInfo`, `Sample`, `DdsGuid` (12+4 byte) with
`makeDdsGuid` factory, `DataAvailableCallback`.

**Repo:** `runtime/typescript/src/wasm.ts` (`ParticipantHandle`, `TopicHandle`, `PublisherHandle`, `SubscriberHandle`, `DataWriterHandle`, `DataReaderHandle`, `DdsGuid`, `makeDdsGuid`, `SampleInfo`, `Sample`, `DataAvailableCallback`)

The brands are string literals and not `unique symbol`s: a symbol brand
is nominal across module boundaries, and a handle from one vendor's
binding would then not fit another's parameter. `publicationHandle`
carries the full sixteen octets of the RTPS GUID; truncating to 64 bits
would lose the vendor and host components.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` — `"C.5.1"` checks `prefix.length === 12` and `sequenceNumber`.

**Status:** `done`

### §C.2 Required Operations

**Spec:** Annex C §C.2 — DCPS data-path operations
(createParticipant/Topic/Publisher/Subscriber/DataWriter/
DataReader/writeSample/takeSamples/setDataAvailableListener).

**Repo:** `runtime/typescript/src/wasm.ts` (`DdsBindings` names all fourteen operations of §C.2.1 through §C.2.4; `WasmBackend` is the same surface with bare numbers, as a Wasm export table has it; `bindWasmBackend` joins the two)

What `bindWasmBackend` enforces on top is not in the type system: handle
0 as the sentinel, liveness, the cascade on delete, borrowed buffers, and
listener delivery on the host event loop.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` — `"C.5.1"`, `"C.5.2"`, `"C.5.3"`, `"C.5.4"`.

**Status:** `done`

### §C.3 Wire-Format Boundary

**Spec:** Annex C §C.3 — XCDR2 bytes, memory ownership,
DdsAny boundary rule. `--xcdr-endianness` flag.

**Repo:** `runtime/typescript/src/wasm.ts` (`writeSample` and `takeSamples` take and return `Uint8Array`; `borrowed` copies what goes across and `takeSamples` copies what comes back), `tools/idlc/src/main.rs` (`--xcdr-endianness <BE|LE>`, default LE)

No JavaScript object graph crosses the boundary: the payload is the XCDR2
octet sequence and nothing else. A `DdsAny` is no exception — `boxAny` on
the producing side, the generated serialiser, the octets, the generated
deserialiser, `unboxAny` on the consuming side.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` (C.5.1 checks that an array changed after the write does not reach the reader; C.5.4 the `DdsAny` path), `tools/idlc/src/main.rs::tests::generate_takes_the_dds_ts_spelling_of_the_endianness_flag`

**Status:** `done`

### §C.4 Browser vs Node.js Bindings

**Spec:** Annex C §C.4 — browser via wasm-bindgen,
Node.js via N-API (optional).

**Repo:** `runtime/typescript/src/wasm.ts` (`WasmBackend`, `bindWasmBackend`)

One surface for both profiles. The browser supplies a `.wasm` module,
Node.js may additionally supply a `.node` module through N-API; both
satisfy `WasmBackend`, and `bindWasmBackend` returns the same
`DdsBindings` either way. An application changes its import line and
nothing else. The runtime itself depends on nothing from Node: it has to
run in a browser, and an `@types/node` among its dependencies would say
otherwise.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` — `"C.5.1"` binds against a backend that is neither the browser nor
the Node module; what is measured is the binding layer, not either module.

**Status:** `done`

### §C.5 Conformance Tests

**Spec:** Annex C §C.5 — 4 mandatory round-trip tests.

**Repo:** `runtime/typescript/tests/wasm-bindings.ts` (C.5.1 publish/subscribe round trip, C.5.2 listener, C.5.3 resource cleanup, C.5.4 `DdsAny` round trip), `runtime/typescript/src/test_backend.ts` (the in-memory data path they run against)

The reference backend is deliberately synchronous: it calls the listener
inside `writeSample`, which is exactly what §C.2.4 forbids a *binding*
from doing — and that is what makes the deferral in `bindWasmBackend`
measurable.

**Tests:** `runtime/typescript/tests/wasm-bindings.ts` — 4 of 4: `"C.5.1"`, `"C.5.2"`, `"C.5.3"`, `"C.5.4"`, via `runtime/typescript/run-tests.sh`.

**Status:** `done`

### §C.6 Reservation of Future Extensions

**Spec:** Annex C §C.6 — reserved-identifier list.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Annex D — Diagnostic Codes (Normative)

### §D.1 Diagnostic Code Registry

**Spec:** Annex D — codes E001 (reserved), E002 (forward-decl
orphan), E003 (annotation conflict), E004 (strict-annotations),
W001 (reserved), W002 (unknown annotation), W003 (map key
hazard), W004 (implicit default), I001 (long double info).

**Repo:** —

**Tests:** —

**Status:** open — the tree carries a stable code set, but a different one:
`crates/idl-parse/src/diagnostic.rs::DiagnosticCode` assigns `IDL0001`
through `IDL0018`. The Annex D codes `E001`-`E004`, `W001`-`W004` and `I001`
do not occur in the tree, and no test covers them.

---

## Audit status

58 done / 0 partial / 1 open / 13 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 50 test binaries,
1024 tests green, 0 failed; among them
`--test typescript_side_tables` (12), `--test typescript_descriptors` (26),
`--test typescript_reader_generates` (16), `--test typescript_codec` (3),
`--test typescript_names_its_runtime` (2) and
`--test typescript_operations_profile` (1).
`runtime/typescript/run-tests.sh` — 61 marks green (16 octet cases,
32 value cases, 9 descriptor-runtime marks, 4 WASM conformance tests),
0 failed.

Wire measurement for the surfaces of §7.5, §7.6 and §7.9.1: 119 cases of
the cell `matrix/zerodds-typescript/cyclone-xcdr2-le` re-run through
`tools/wireprobe/wireprobe-zerodds-typescript`, octets and read-back
values held against the recorded artefacts — 244 of 244 comparisons
equal, 0 different.

Open items: see `dds-ts-1.0.open.en.md`. Decision records: none.
