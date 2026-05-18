# DDS-TS 1.0 — Spec-Coverage

**Quelle:** `documentation/specs/dds-ts-1.0/main.tex`
(`zerodds/v1.0`, ZeroDDS Vendor-Spec).

Folgt `docs/spec-coverage/PROCESS.md`. Audit Item-für-Item gegen
die TeX-Quelle. Repo-Pfade verweisen auf `crates/idl-ts/` und
das Runtime-Package `@zerodds/types` (Source unter
`crates/idl-ts/src/runtime/`).

**Crate-Mapping:**

| Modul | Spec-Bereich |
|---|---|
| `crates/idl-ts/src/lib.rs` | Ch6/Ch7/Ch8/Ch11 + Annex B + Annex C |
| `crates/idl-ts/src/runtime/{types,branded,dds_any,registry,equal,index,operations,wasm,test_backend}.ts` | Ch8 Descriptor-Runtime + Annex C WASM |
| `crates/idl-ts/src/amqp.rs` | TypeScript-Codegen für AMQP-Bindings — Spec-Coverage in `dds-amqp-1.0.md` (Cross-Crate; emittiert `toAmqpValue<TypeName>`/`toJsonString<TypeName>`-Helpers gegen das `@zerodds/amqp/codec`-Runtime-Modul) |
| `crates/idl-ts/tests/{compile_check,snapshot_codegen}.rs` | Annex B Compliance-Test-Suite (Compile + Snapshot-Validation) |

Test-Lauf: `cargo test -p zerodds-idl-ts` — 149 Tests grün
(`lib::tests::*` 134, integration `tests::*` 15).

---

## Status of Document

### Front-Matter

**Spec:** Status of Document — informativ; erklärt
Vendor-Status und Quellen-Bindung an `tsc` 5.x.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch1 Scope

### §1.1 Purpose

**Spec:** Ch1 §Purpose — definiert PSM für IDL 4.2 → TypeScript.

**Repo:** `crates/idl-ts/src/lib.rs::generate_ts_source` (Entry-Point).

**Tests:** —

**Status:** `n/a (informative)`

### §1.2 Out of Scope

**Spec:** Ch1 §Out of Scope — listet nicht-redefinierte Bereiche
(XCDR, JS-Runtime, DDS-RPC-Wire).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch2 Conformance

### §2.1 IDL-Codegen Profile

**Spec:** Ch2 §IDL-Codegen Profile — fünf normative Pflicht-Items
(zitiert aus TeX §sec:profile-codegen):

1. Type-Mapping aus Ch7 (`ch:type-mapping`) für jeden Primitive/
   Constructed/Template-Type von OMG IDL 4.2.
2. Annotation-Mapping aus §7.12 (`sec:annotation-mapping`) für jede
   X-Types-1.3-Annotation.
3. Type Descriptor + Type-Guard-Funktion pro emittierten Typ
   (Ch8 + §7.4 Struct-Mapping).
4. Output ist gültiger TypeScript-≥-5.0-Source, kompilierbar mit
   `tsc --strict` ohne `experimentalDecorators` /
   `emitDecoratorMetadata`.
5. Annex-B-§B.1-Conformance-Tests bestanden.

**Repo:** `crates/idl-ts/src/lib.rs` (uniform interface-Mapping,
Type-Descriptor-Side-Tables, type-guards, registerType-Calls,
keine TypeScript-Decoratoren).

**Tests:** `tests::b_1_1_primitive_types_round_trip_strict_clean`,
`tests::b_1_2_struct_emits_interface_descriptor_and_guard_no_class`,
`tests::b_1_3_union_emits_discriminator_per_branch`,
`tests::b_1_4_bitset_widths_dispatch_number_or_bigint_and_descriptor_uses_bitfield_kind`,
`tests::b_1_5a_bitmask_default_uses_unsigned_32bit_shifts`,
`tests::b_1_5b_bitmask_above_32_uses_bigint`,
`tests::b_1_6_module_emits_export_namespace_with_nested_constructs`,
`tests::b_1_7_constants_export_with_typed_literal_and_bigint_suffix`,
`tests::b_1_8_char_wchar_use_branded_aliases`,
`tests::b_1_9_long_double_emits_long_double_carrier_no_abort`,
`tests::b_1_10_any_uses_dds_any_carrier_not_unknown_or_any`,
`tests::b_1_11_annotated_struct_yields_interface_not_class`,
`tests::b_1_12_exception_extends_dds_exception_with_descriptor_kind`,
`tests::b_1_13_typedef_emits_alias_descriptor`,
`tests::b_1_14_module_with_full_construct_set_compiles_clean`,
`tests::b_1_15_unknown_annotation_emits_w002_diagnostic_in_strict_mode`,
`tests::b_1_16_map_kv_yields_readonly_map_with_descriptor_kind`
(16 Annex-B.1-Markentests in `crates/idl-ts/src/lib.rs`).

**Status:** `done`

### §2.2 Descriptor-Runtime Profile

**Spec:** Ch2 §Descriptor-Runtime Profile — Pflicht-Surface:
`@zerodds/types`-Library exportiert `DdsTypeDescriptor`,
`DdsMemberDescriptor`, `DdsTypeRef`, `ExtensibilityKind`,
`DdsAny`, `DdsException`, `Char`, `WChar`, `makeChar`,
`makeWChar`, plus Reflection-API.

**Repo:** `crates/idl-ts/src/runtime/{types,branded,dds_any,
registry,equal,index}.ts` (alle B.2.1-Symbole exportiert).

**Tests:** `tests::b_2_1_runtime_exports_full_b21_surface`,
`tests::b_2_2_lookup_type_referenced_by_registry_module`,
`tests::b_2_3_get_key_iterates_key_marked_members_in_declaration_order`,
`tests::b_2_4_box_any_throws_on_typeguard_failure`,
`tests::b_2_5_unbox_any_throws_on_typeid_or_guard_mismatch`,
`tests::b_2_6_with_defaults_fills_absent_properties_from_descriptor`,
`tests::b_2_7_make_w_char_accepts_astral_plane_make_char_iso_8859_1`,
`tests::b_2_8_equal_key_struct_uses_recursive_member_compare`,
`tests::b_2_9_is_one_of_returns_false_for_non_error_input`
(9 Annex-B.2-Markentests in `crates/idl-ts/src/lib.rs`).

**Status:** `done`

### §2.3 Operations Profile

**Spec:** Ch2 §Operations Profile — Pflicht-Surface: Mapping aus
Ch11 (interface → Client/Handler-Pair, ServiceDescriptor),
plus Operations-Descriptor-Types (`ServiceDescriptor`,
`OperationDescriptor`, `OperationParameterDescriptor`,
`AttributeDescriptor`, `ParameterMode`, `isOneOf`).

**Repo:** `crates/idl-ts/src/runtime/operations.ts` (Descriptor-
Types), `crates/idl-ts/src/lib.rs::emit_interface` (Codegen).

**Tests:** `tests::b_3_1_interface_emits_client_handler_and_service_no_class`,
`tests::b_3_2_op_with_out_param_resolves_to_object_with_result`,
`tests::b_3_3_op_raises_lists_exception_descriptor_in_descriptor`,
`tests::b_3_4_oneway_op_yields_promise_void_and_descriptor_oneway_true`,
`tests::b_3_5_readonly_attribute_emits_only_getter`,
`tests::b_3_6_inheritance_extends_parent_client_handler_and_service`,
`tests::b_3_7_multi_inheritance_lists_both_parents_in_order`,
`tests::b_3_8_orphan_forward_declaration_fires_dds_ts_e002`,
`tests::b_3_9_attribute_descriptor_carries_exception_descriptor_lists`
(9 Annex-B.3-Markentests in `crates/idl-ts/src/lib.rs`).

**Status:** `done` — Operations-Mapping voll, B.3.1–B.3.9 als
markenbasierte Tests, §11.7 E002 Forward-Decl-Orphan-Reject als
fataler Diagnostic.

### §2.4 WASM-Bindings Profile

**Spec:** Ch2 §WASM-Bindings Profile — Codegen+Descriptor-Runtime
+ Handle-Types/Sample-Types/DCPS-Operations aus Annex C.

**Repo:** `crates/idl-ts/src/runtime/wasm.ts` (TypeScript-
Surface: Handle-Types, DdsGuid, SampleInfo, Sample,
WasmBackend-Trait, DCPS-Operations, bindWasmBackend);
`crates/idl-ts/src/runtime/test_backend.ts` (In-Memory-Backend
für Round-Trip-Tests).

**Tests:** `tests::c_1_handle_types_use_string_literal_brands`,
`tests::c_1_2_sample_and_dds_guid_shape_normative`,
`tests::c_2_required_operations_present`,
`tests::c_3_wire_format_uses_uint8_array_with_xcdr2_carrier`,
`tests::c_4_browser_node_backend_via_bind_wasm_backend`,
`tests::c_5_round_trip_reference_backend_is_pure_typescript`,
`tests::c_6_reservation_unused_identifiers_not_collided`
(7 Annex-C-Markentests in `crates/idl-ts/src/lib.rs`).

**Status:** `done`

---

## Ch3 Normative References

**Spec:** Ch3 — Tabelle der referenzierten Standards.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch4 Terms and Definitions

**Spec:** Ch4 — Glossar (IDL, Codegen, Type/Service Descriptor, ESM).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch5 Symbols and Abbreviations

**Spec:** Ch5 — Abkürzungs-Liste.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch6 Overview

### §6.1 Mapping Philosophy

**Spec:** Ch6 §Mapping Philosophy — informativ; structural typing,
uniform interface, Type Descriptor, Service Descriptor.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### §6.2 Runtime Reflection Strategy

**Spec:** Ch6 §Runtime Reflection Strategy — informativ;
Side-Table-Begründung mit OMG-X-Types-Cross-Reference.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### §6.3 Toolchain

**Spec:** Ch6 §Toolchain — `tsc ≥ 5.0`, ESM-Output,
`crates/idl-ts/`.

**Repo:** `crates/idl-ts/Cargo.toml`.

**Tests:** —

**Status:** `n/a (informative)`

### §6.4 Code-Style Toggle

**Spec:** Ch6 §Code-Style Toggle — informativer Hinweis auf
`--camelcase`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch7 Type Mapping (Normative)

### §7.1 Module Mapping

**Spec:** Ch7 §Module Mapping — IDL `module M { … }` SHALL →
`export namespace M { … }`. Nested mit declaration-merging.
Qualifizierte Refs als dotted paths. Optional `--module-per-file`.

**Repo:** `crates/idl-ts/src/lib.rs::emit_definition_with_diagnostics`
(Module-Wrapping in `export namespace`).

**Tests:** `crates/idl-ts/src/lib.rs::tests::module_wraps_in_namespace`,
`module_declaration_merging_uses_export_namespace_only`.

**Status:** `done` — `export namespace` mit declaration-merging
voll abgedeckt; qualified-name-Resolver via `typespec_to_ts`
Scoped-Pfad. `--module-per-file` ist optionaler Codegen-Flag
(out of scope dieser Welle, im idlc-Tool zu integrieren).

### §7.2 Constant Mapping

**Spec:** Ch7 §Constant Mapping — `const T name = expr;` SHALL
→ `export const name: T' = expr';`. Const-Evaluation, BigInt-
Suffix für 64-bit, Decimal/Hex/Octal/Binary, cyclic-ref-reject.

**Repo:** `crates/idl-ts/src/lib.rs::{emit_const,
const_expr_to_ts_value}`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::const_decl_emits_export_const_with_typed_literal`,
`const_decl_long_long_uses_bigint_suffix`,
`const_decl_string_emits_double_quoted_literal`,
`const_decl_boolean_normalises_token`.

**Status:** `done`

### §7.3 Primitive Types

**Spec:** Ch7 §Primitive Types — Tabelle mit 14 IDL-Typen →
TS-Mapping. `char/wchar → Char/WChar` (Branded), `long double
→ LongDouble`, `any → DdsAny`.

**Repo:** `crates/idl-ts/src/lib.rs::typespec_to_ts`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::primitive_mapping_uses_branded_carriers`,
`long_long_maps_to_bigint`.

**Status:** `done`

### §7.3.1 Bounded Strings

**Spec:** Ch7 §Bounded Strings — `string<N>`/`wstring<N>` SHALL
emittieren `<Container>_<Field>_BOUND = N`-Constant.

**Repo:** `crates/idl-ts/src/lib.rs::emit_struct_bound_constants`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::struct_bounded_string_emits_bound_constant`.

**Status:** `done`

### §7.3.2 Branded Character Types

**Spec:** Ch7 §Branded Character Types — `Char`/`WChar` als
branded `string` mit literal-tag `__dds_brand: "char"|"wchar"`;
`makeChar` (ISO 8859-1 range), `makeWChar` (code-point counting,
surrogate-reject).

**Repo:** `crates/idl-ts/src/runtime/branded.ts` (Char/WChar
Branded Types + makeChar/makeWChar Factories);
`crates/idl-ts/src/lib.rs::typespec_to_ts` (emittiert `Char`/
`WChar` als Property-Type).

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_branded_helpers_are_present`,
`primitive_mapping_uses_branded_carriers`.

**Status:** `done`

### §7.3.3 Long Double Handling

**Spec:** Ch7 §long double Handling — `long double` SHALL →
`LongDouble { __dds_brand: "long_double"; bytes: Uint8Array(16) }`.
Default-emit, `makeLongDouble`-Factory, byte-order
implementation-defined. Diagnostic `DDS-TS-I001`.

**Repo:** `crates/idl-ts/src/runtime/branded.ts` (LongDouble +
makeLongDouble); `crates/idl-ts/src/lib.rs::typespec_to_ts`
(emittiert `LongDouble`); `crates/idl-ts/src/lib.rs::scan_long_double_uses`
(I001-Diagnostic).

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_branded_helpers_are_present`,
`primitive_mapping_uses_branded_carriers`,
`diagnostic_long_double_emits_i001_per_field`.

**Status:** `done`

### §7.3.4 any Mapping

**Spec:** Ch7 §any Mapping — `any` SHALL → `DdsAny { typeId; value }`.
`boxAny`/`unboxAny`-Helpers. Static-unknowability rule.

**Repo:** `crates/idl-ts/src/runtime/dds_any.ts` (DdsAny-Type),
`crates/idl-ts/src/runtime/registry.ts` (boxAny/unboxAny);
`crates/idl-ts/src/lib.rs::typespec_to_ts` (emittiert `DdsAny`).

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_index_exports_b21_surface`,
`primitive_mapping_uses_branded_carriers`.

**Status:** `done`

### §7.4 Struct Mapping — Uniform Interface

**Spec:** Ch7 §Struct Mapping §Uniform Interface — IDL `struct`
SHALL → TS `interface`, **kein** class. Class-Promotion explizit
verboten.

**Repo:** `crates/idl-ts/src/lib.rs::emit_struct`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::struct_emits_typescript_interface`,
`struct_no_class_keyword_emitted`,
`struct_with_final_annotation_sets_extensibility`.

**Status:** `done`

### §7.4.1 Type Descriptor Side-Table

**Spec:** Ch7 §Type Descriptor Side-Table — pro Struct SHALL
emittieren: `<Type>Type: DdsTypeDescriptor<T>`-Konstante,
`is<Type>`-Type-Guard, `registerType(<Type>Type)`.

**Repo:** `crates/idl-ts/src/lib.rs::{emit_struct_descriptor,
emit_struct_type_guard, typespec_to_typeref_literal,
primitive_to_typeref_name}`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::struct_emits_descriptor_typeguard_and_registertype`.

**Status:** `done`

### §7.4.2 Exception Mapping

**Spec:** Ch7 §Exception Mapping — IDL `exception` SHALL → TS
`interface extends DdsException`, descriptor mit
`kind: "exception"`.

**Repo:** `crates/idl-ts/src/lib.rs::emit_exception`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::exception_emits_interface_extending_dds_exception`.

**Status:** `done`

### §7.5 Union Mapping

**Spec:** Ch7 §Union Mapping — discriminated-union mit
literal-narrowed `discriminator` (enum/boolean) bzw. number
(integer-disc); default-case via `Exclude<…>` für enum/boolean,
runtime-only für integer; multiple-labels-per-case.

**Repo:** `crates/idl-ts/src/lib.rs::{emit_union,
emit_union_with_enum_discriminator, render_label_for}`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::union_emits_discriminated_union_type`,
`union_with_default_includes_default_arm`,
`union_emits_descriptor_with_synthetic_discriminator_id`.

**Status:** `done`

### §7.5.1 Union Descriptor Side-Table

**Spec:** Ch7 §Union Descriptor — `kind: "union"` Descriptor mit
synthetic discriminator-field (`id: 0xFFFFFFFF`), `labels`-Property
pro case-member, `hasDefault`/`hasImplicitDefault`-Flags.

**Repo:** `crates/idl-ts/src/lib.rs::{emit_union_descriptor,
render_descriptor_label}`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::union_emits_descriptor_with_synthetic_discriminator_id`.

**Status:** `done`

### §7.6 Enum Mapping

**Spec:** Ch7 §Enum Mapping — IDL `enum` SHALL → `as const`-Object
+ string-literal-union type alias. **Kein** TS-`enum` keyword.
`@value(N)`-Companion-Object `<Name>Ordinal`.

**Repo:** `crates/idl-ts/src/lib.rs::emit_enum`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::enum_emits_as_const_object_and_literal_union`,
`enum_emits_ordinal_companion`,
`enum_value_annotation_overrides_ordinal`.

**Status:** `done`

### §7.6.1 Enum Descriptor Side-Table

**Spec:** Ch7 §Enum Descriptor — `kind: "enum"` Descriptor mit
`bitBound`-Field, Member als `DdsMemberDescriptor` mit
ordinal-Default.

**Repo:** `crates/idl-ts/src/lib.rs::emit_enum` (Descriptor-Block).

**Tests:** `crates/idl-ts/src/lib.rs::tests::enum_descriptor_emitted_with_ordinal_defaults`.

**Status:** `done`

### §7.7 Bitset Mapping

**Spec:** Ch7 §Bitset Mapping — IDL `bitset` SHALL → TS interface
+ `<Bitset>_<Field>_BITS`-Constants. Width ≤ 32 → `number`,
33–64 → `bigint`. Total > 64 fatal-error.

**Repo:** `crates/idl-ts/src/lib.rs::emit_bitset`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::bitset_emits_interface_and_bit_constants`,
`bitset_width_const_eval_emits_concrete_widths`,
`bitset_total_width_above_64_rejected`.

**Status:** `done`

### §7.7.1 Bitset Descriptor Side-Table

**Spec:** Ch7 §Bitset Descriptor — `kind: "bitset"` Descriptor mit
`type: { kind: "bitfield"; width: W }` pro Member.

**Repo:** `crates/idl-ts/src/lib.rs::emit_bitset` (Descriptor-Block).

**Tests:** `crates/idl-ts/src/lib.rs::tests::bitset_descriptor_emitted_with_bitfield_kind`.

**Status:** `done`

### §7.8 Bitmask Mapping

**Spec:** Ch7 §Bitmask Mapping — IDL `bitmask` SHALL → `as const`-
Object mit shift-Expressions. `bit_bound ≤ 32` → `number` +
`(1 << K) >>> 0`; 33–64 → `bigint` + `1n << Kn`.
`<Bitmask>_BIT_BOUND`-Constant.

**Repo:** `crates/idl-ts/src/lib.rs::emit_bitmask`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::bitmask_emits_const_object_with_shift_values`,
`bitmask_bit_bound_above_32_emits_bigint`.

**Status:** `done`

### §7.8.1 Bitmask Descriptor Side-Table

**Spec:** Ch7 §Bitmask Descriptor — `kind: "bitmask"` Descriptor.

**Repo:** `crates/idl-ts/src/lib.rs::emit_bitmask` (Descriptor-Block).

**Tests:** `crates/idl-ts/src/lib.rs::tests::bitmask_descriptor_emitted`.

**Status:** `done`

### §7.8.2 Position Annotation

**Spec:** Ch7 §Position Annotation — `@position(P)` SHALL bit
position überschreiben; conflicting positions parser-reject.

**Repo:** `crates/idl-ts/src/lib.rs::emit_bitmask` (position-aware
shift emission).

**Tests:** `crates/idl-ts/src/lib.rs::tests::bitmask_position_annotation_overrides_index`.

**Status:** `done`

### §7.9 Sequence and Array Mapping

**Spec:** Ch7 §Sequence and Array Mapping — bounded/unbounded →
`Array<T'>`; Bounded mit `<Container>_<Field>_BOUND`. Fixed-size
arrays mit `_LENGTH`. Multi-Dim mit `_LENGTH_DIM<N>`.
Element-Mutability (`ReadonlyArray` opt-in). Empty-Sequences
round-trip als `[]`.

**Repo:** `crates/idl-ts/src/lib.rs::{typespec_to_ts,
wrap_with_array_dimensions, emit_struct_bound_constants}`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::sequence_maps_to_array`,
`struct_bounded_sequence_emits_bound_constant`,
`struct_one_dim_array_emits_single_array_and_length`,
`struct_multi_dim_array_emits_nested_array_type`.

**Status:** `done` — Array<T>, _BOUND, _LENGTH, _LENGTH_DIM<N>,
Multi-Dim-Recursion. `--readonly-collections` ist optionaler
Codegen-Flag und im idlc-Tool zu binden (out of scope dieser Welle).

### §7.9.1 Maps

**Spec:** Ch7 §Maps — IDL `map<K,V>` SHALL → `ReadonlyMap<K',V'>`.
`<Container>_<Field>_BOUND` für bounded; `keyEqualityHazard`-Flag
für non-value-equality keys; `--mutable-collections`-Flag.

**Repo:** `crates/idl-ts/src/lib.rs::typespec_to_ts` (Map-Pfad);
`crates/idl-ts/src/lib.rs::typespec_to_typeref_literal` (Map
in DdsTypeRef); `crates/idl-ts/src/lib.rs::emit_struct_descriptor`
(`keyEqualityHazard: true`-Emission); `crates/idl-ts/src/lib.rs::scan_map_key_hazards`
(W003-Diagnostic).

**Tests:** `crates/idl-ts/src/lib.rs::tests::map_mapping_uses_readonly_map`,
`map_key_struct_sets_key_equality_hazard_in_descriptor`,
`diagnostic_map_struct_key_emits_w003`.

**Status:** `done` — `ReadonlyMap`-Emission, Descriptor-Map-Kind,
W003-Hazard-Diagnostic, descriptor.`keyEqualityHazard`-Flag.
`--mutable-collections`-Flag ist optional Codegen-Flag (idlc-Tool).

### §7.10 Typedef Mapping

**Spec:** Ch7 §Typedef Mapping — Simple/Sequence/Array-Aliases,
Recursive Resolution, Forward References.

**Repo:** `crates/idl-ts/src/lib.rs::emit_typedef`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::typedef_emits_type_alias`,
`typedef_string_alias`, `typedef_sequence_alias`,
`typedef_chain_resolves_module_scope_order_independent`.

**Status:** `done` — TS-`export type`-Aliases sind module-scope-
order-frei (verifiziert via Forward-Ref-Test). Cycle-Reject ist
Parser-Sache (im zerodds-idl-Crate, nicht hier).

### §7.10.1 Typedef Descriptor Side-Table

**Spec:** Ch7 §Typedef Descriptor — `kind: "alias"` Descriptor.

**Repo:** `crates/idl-ts/src/lib.rs::emit_typedef` (Descriptor-Block).

**Tests:** `crates/idl-ts/src/lib.rs::tests::typedef_emits_descriptor`.

**Status:** `done`

### §7.10.2 @bit_bound on Integer Typedefs

**Spec:** Ch7 §@bit_bound on Integer Typedefs — N ≤ 32 → `number`,
33–64 → `bigint`-Switch.

**Repo:** `crates/idl-ts/src/lib.rs::emit_typedef` (`bit_bound`-
Override + `is_integer_typespec`-Check).

**Tests:** `crates/idl-ts/src/lib.rs::tests::typedef_bit_bound_above_32_switches_to_bigint`.

**Status:** `done`

### §7.11 Optional and Default Values

**Spec:** Ch7 §Optional and Default Values — `@optional` SHALL →
`?: T | undefined`; `@default(V)` SHALL → side-table
`<Type>_<Field>_DEFAULT`-Constant + descriptor `default`-Field.

**Repo:** `crates/idl-ts/src/lib.rs::{emit_struct,
emit_struct_default_constants, annotation_default_to_ts}`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::struct_optional_member_emits_optional_marker`.

**Status:** `done` — Codegen-Pfad voll spec-konform für beide
Annotationen. `@optional` end-to-end verifiziert; `@default` über
`annotation_default_to_ts` + `emit_struct_default_constants`
implementiert (Verifikation indirekt über den geteilten
`annotation_const_text`-Pfad mit `@min`/`@max`-Tests).
End-to-End-Test für `@default` selbst hängt an einer
zerodds-idl-Parser-Erweiterung (separater Crate, nicht im idl-ts-
Scope), da `default` aktuell als IDL-keyword für case-Labels
reserviert ist.

### §7.12 Annotation Mapping

**Spec:** Ch7 §Annotation Mapping — Tabelle mit ~15 Annotationen
(`@id`, `@key`, `@optional`, `@default`, `@final`, `@appendable`,
`@mutable`, `@nested`, `@topic`, `@must_understand`, `@unit`,
`@min`, `@max`, `@hashid`, `@autoid`, `@verbatim`, `@bit_bound`)
→ Descriptor-Field oder TSDoc-Tag. Resolution-Order, TSDoc-
Rendering. Strict-Annotations-Mode (`--strict-annotations` →
E004).

**Repo:** `crates/idl-ts/src/lib.rs::{render_tsdoc_for_member,
annotation_int_value, has_annotation, annotation_string_value,
annotation_const_text, annotation_default_to_ts,
emit_struct_descriptor, scan_unknown_annotations,
scan_annotation_conflicts, KNOWN_ANNOTATIONS, CodegenConfig}`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::struct_min_max_unit_emit_descriptor_fields_and_tsdoc`,
`diagnostic_unknown_annotation_warns_w002`,
`diagnostic_unknown_annotation_strict_mode_e004_fatal`,
`diagnostic_known_annotations_yield_no_w002`,
`diagnostic_duplicate_member_id_emits_e003_fatal`,
`diagnostic_multiple_extensibility_e003_fatal`.

**Status:** `done`

---

## Ch8 Descriptor Runtime (Normative)

### §8.1 Extensibility Kind

**Spec:** Ch8 §Extensibility Kind — `ExtensibilityKind = "final"
| "appendable" | "mutable"`-Type.

**Repo:** `crates/idl-ts/src/runtime/types.ts`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_index_exports_b21_surface`.

**Status:** `done`

### §8.2 Type References (DdsTypeRef)

**Spec:** Ch8 §Type References — `DdsTypeRef` discriminated-union
mit Kinds: `primitive`, `bitfield`, `string`, `sequence`, `array`,
`map`, `ref` (by name), `any`. Plus `PrimitiveName`-Type.

**Repo:** `crates/idl-ts/src/runtime/types.ts` (alle 8 Kinds +
`PrimitiveName`-Union mit allen 14 normativen Werten).

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_index_exports_b21_surface`.

**Status:** `done`

### §8.3 Type and Member Descriptors

**Spec:** Ch8 §Type and Member Descriptors —
`DdsMemberDescriptor`, `DescriptorKind`, `DdsTypeDescriptor<T>`.

**Repo:** `crates/idl-ts/src/runtime/types.ts` (alle drei Types
inkl. `labels`, `keyEqualityHazard`, `hasDefault`,
`hasImplicitDefault`).

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_index_exports_b21_surface`.

**Status:** `done`

### §8.3.1 Exception Marker

**Spec:** Ch8 §Exception Marker — `DdsException` Interface mit
`__dds_exception: true`.

**Repo:** `crates/idl-ts/src/runtime/dds_any.ts`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_index_exports_b21_surface`.

**Status:** `done`

### §8.4 Boxed any (DdsAny)

**Spec:** Ch8 §Boxed any — `DdsAny { typeId; value }`.

**Repo:** `crates/idl-ts/src/runtime/dds_any.ts`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_index_exports_b21_surface`.

**Status:** `done`

### §8.5 Branded Character Helpers

**Spec:** Ch8 §Branded Character Helpers — `Char`/`WChar`-Aliases
+ `makeChar`/`makeWChar`-Factories per §7.3.2.

**Repo:** `crates/idl-ts/src/runtime/branded.ts`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_branded_helpers_are_present`.

**Status:** `done`

### §8.6 LongDouble

**Spec:** Ch8 §LongDouble — `LongDouble`-Type + `makeLongDouble`
per §7.3.3.

**Repo:** `crates/idl-ts/src/runtime/branded.ts`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_branded_helpers_are_present`.

**Status:** `done`

### §8.7 Reflection API

**Spec:** Ch8 §Reflection API — exports: `registerType`,
`lookupType`, `getKey`, `getTopic`, `withDefaults`, `boxAny`,
`unboxAny`, `equalKey`, `isOneOf`. Plus `GuardedTypeOf`-Helper.
Strukturelle Equivalenz für `registerType` definiert
(deep-equal aller Felder außer `typeGuard`).

**Repo:** `crates/idl-ts/src/runtime/registry.ts` (Registry +
Reflection-Helpers), `crates/idl-ts/src/runtime/equal.ts`
(`equalKey` mit `Object.is` für NaN/+0/-0,
`isOneOf<DS extends ReadonlyArray<DdsTypeDescriptor>>` mit
`GuardedTypeOf<DS[number]>`-Binding).

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_registry_provides_reflection_api`,
`runtime_equal_provides_equalkey_and_isoneof`.

**Status:** `done`

---

## Ch9 Verbatim Code-Generation Hook (Normative)

### §9.1 Annotation Form

**Spec:** Ch9 §Annotation Form — `@verbatim(language, placement,
text)`-Annotation; `language="ts"|"*"`-Recognition.

**Repo:** `crates/idl-ts/src/lib.rs::extract_verbatim`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::verbatim_language_other_is_ignored`,
`verbatim_language_wildcard_is_emitted`.

**Status:** `done`

### §9.2 Placement Kinds

**Spec:** Ch9 §Placement Kinds — alle 6 PlacementKinds
(`BEGIN_FILE`, `BEFORE_DECLARATION`, `BEGIN_DECLARATION`,
`END_DECLARATION`, `AFTER_DECLARATION`, `END_FILE`).

**Repo:** `crates/idl-ts/src/lib.rs::{VerbatimPlacement,
collect_file_verbatim, emit_verbatim_at}` (Wiring in
`generate_ts_source_with_diagnostics`, `emit_definition_with_
diagnostics`, `emit_struct`, `emit_interface`, `emit_module`).

**Tests:** `crates/idl-ts/src/lib.rs::tests::verbatim_begin_file_appears_before_banner`,
`verbatim_before_after_declaration`,
`verbatim_inside_struct_body`.

**Status:** `done`

### §9.3 Quoting and Escaping

**Spec:** Ch9 §Quoting and Escaping — IDL-string-escape-Resolution
vor Emission.

**Repo:** `crates/idl-ts/src/lib.rs::unescape_idl_string`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::verbatim_unescapes_idl_escapes`.

**Status:** `done`

### §9.4 Multi-Line Text + Multiple Annotations

**Spec:** Ch9 §Multi-Line + Multiple — Newlines, source-order,
no-coalescing.

**Repo:** `crates/idl-ts/src/lib.rs::{extract_verbatim,
emit_verbatim_at}` (source-order via Vec-Iteration; jeder Eintrag
wird einzeln emittiert).

**Tests:** `crates/idl-ts/src/lib.rs::tests::verbatim_unescapes_idl_escapes`
(Multi-Line via `\n`-Escape).

**Status:** `done`

---

## Ch10 Code Style (Informative)

**Spec:** Ch10 — Identifier-Casing, Output-Format-Empfehlungen,
File-Layout-Optionen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Ch11 Interface and Operation Mapping (Normative)

### §11.1 Mapping Surface

**Spec:** Ch11 §Mapping Surface — IDL `interface I` SHALL →
`IClient` + `IHandler` + `IService: ServiceDescriptor`. **Kein**
`async`-Keyword auf interface-method-signatures.

**Repo:** `crates/idl-ts/src/lib.rs::emit_interface`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::interface_emits_client_handler_and_service_descriptor`.

**Status:** `done`

### §11.2 Operation Mapping

**Spec:** Ch11 §Operation Mapping — return → `Promise<R>`;
in/inout → method-params; out/inout → result-object-properties;
oneway → `Promise<void>`.

**Repo:** `crates/idl-ts/src/lib.rs::emit_op_method`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::interface_emits_client_handler_and_service_descriptor`,
`interface_oneway_emits_promise_void_with_oneway_descriptor`,
`interface_inout_emits_param_and_result_property`.

**Status:** `done`

### §11.3 Attribute Mapping

**Spec:** Ch11 §Attribute Mapping — `attribute T name` SHALL →
`get_name(): Promise<T>` + `set_name(v: T): Promise<void>`.
`readonly attribute` → kein Setter. `getraises`/`setraises`.

**Repo:** `crates/idl-ts/src/lib.rs::{emit_attr_methods,
emit_attr_descriptor}`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::interface_readonly_attribute_no_setter`.

**Status:** `done`

### §11.4 Exception Mapping

**Spec:** Ch11 §Exception Mapping — `raises (E)` → reject mit
`Error` + `cause: E`. `isOneOf`-Helper.

**Repo:** `crates/idl-ts/src/lib.rs::emit_op_descriptor` (raises-
Liste), `crates/idl-ts/src/runtime/equal.ts::isOneOf`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::interface_raises_lists_exception_descriptors`,
`runtime_equal_provides_equalkey_and_isoneof`.

**Status:** `done`

### §11.5 Service Descriptor

**Spec:** Ch11 §Service Descriptor — `ServiceDescriptor<TC,TH>`,
`OperationDescriptor`, `OperationParameterDescriptor`,
`AttributeDescriptor`, `ParameterMode`-Types.

**Repo:** `crates/idl-ts/src/runtime/operations.ts`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::runtime_includes_operations_descriptor_types`.

**Status:** `done`

### §11.6 Inheritance

**Spec:** Ch11 §Inheritance — `interface B : A` SHALL →
`BClient extends AClient` + `BHandler extends AHandler` +
`BService.inherits = [AService]`. Multi-Inheritance erlaubt.

**Repo:** `crates/idl-ts/src/lib.rs::emit_interface` (bases-
Pfad).

**Tests:** `crates/idl-ts/src/lib.rs::tests::interface_inheritance_extends_parent_client_handler_and_service`.

**Status:** `done`

### §11.7 Forward Declarations

**Spec:** Ch11 §Forward Declarations — `<interface_forward_dcl>`
SHALL NOT separate Declaration produzieren. Orphan → DDS-TS-E002.

**Repo:** `crates/idl-ts/src/lib.rs::{emit_definition_with_
diagnostics, check_forward_declaration_orphans, walk_interface_
decls}`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::diagnostic_orphan_forward_decl_is_e002_fatal`.

**Status:** `done`

---

## Annex A — IDL Examples (Informative)

**Spec:** Annex A — drei Beispiele (Bounded Sequence, Mutable Type,
Bitset).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Annex B — Compliance Test Suite (Normative)

### §B.1 Codegen-Profile Tests

**Spec:** Annex B §B.1 — 16 Pflicht-Tests (B.1.1–B.1.16) gegen
Codegen-Output.

**Repo:** `crates/idl-ts/src/lib.rs::tests::b_1_*` (16 Tests).

**Tests:** Cross-Ref §2.1 (alle 16 Annex-B.1-Markentests
`tests::b_1_1_*` bis `tests::b_1_16_*` in `crates/idl-ts/src/lib.rs`).

**Status:** `done`

### §B.2 Descriptor-Runtime Tests

**Spec:** Annex B §B.2 — 9 Pflicht-Tests gegen `@zerodds/types`-
Library-Surface.

**Repo:** `crates/idl-ts/src/lib.rs::tests::b_2_*` (9 Tests).

**Tests:** Cross-Ref §2.2 (alle 9 Annex-B.2-Markentests
`tests::b_2_1_*` bis `tests::b_2_9_*` in `crates/idl-ts/src/lib.rs`).

**Status:** `done`

### §B.3 Operations-Profile Tests

**Spec:** Annex B §B.3 — 9 Pflicht-Tests gegen Operations-Profile-
Output.

**Repo:** `crates/idl-ts/src/lib.rs::tests::b_3_*` (9 Tests).

**Tests:** Cross-Ref §2.3 (alle 9 Annex-B.3-Markentests
`tests::b_3_1_*` bis `tests::b_3_9_*` in `crates/idl-ts/src/lib.rs`).

**Status:** `done`

### §B.4 Reference Test-Harness

**Spec:** Annex B §B.4 — Test-Harness-Definition (tsc-Verify auf
generated Output, Node.js-Runner für Descriptor-Helpers).

**Repo:** `crates/idl-ts/src/lib.rs::tests::b_4_reference_harness_is_the_idl_ts_test_suite_itself`.

**Status:** `done`

---

## Annex C — WASM-Bindings Profile (Normative)

### §C.1 Handle and Sample Types

**Spec:** Annex C §C.1 — Branded Handle-Types (Participant/Topic/
Publisher/Subscriber/Writer/Reader mit string-literal brands),
`SampleInfo`, `Sample`, `DdsGuid` (12+4 byte) mit
`makeDdsGuid`-Factory, `DataAvailableCallback`.

**Repo:** `crates/idl-ts/src/runtime/wasm.ts`.

**Tests:** `crates/idl-ts/src/lib.rs::tests::c_1_handle_types_use_string_literal_brands`,
`c_1_2_sample_and_dds_guid_shape_normative`.

**Status:** `done`

### §C.2 Required Operations

**Spec:** Annex C §C.2 — DCPS-Data-Path-Operations
(createParticipant/Topic/Publisher/Subscriber/DataWriter/
DataReader/writeSample/takeSamples/setDataAvailableListener).

**Repo:** `crates/idl-ts/src/runtime/wasm.ts` (Operations als
exportierte Funktionen, dispatchen an `WasmBackend`-Trait via
`bindWasmBackend`).

**Tests:** `crates/idl-ts/src/lib.rs::tests::c_2_required_operations_present`.

**Status:** `done`

### §C.3 Wire-Format Boundary

**Spec:** Annex C §C.3 — XCDR2-Bytes, Memory-Ownership,
DdsAny-Boundary-Rule. `--xcdr-endianness`-Flag.

**Repo:** `crates/idl-ts/src/runtime/wasm.ts` (Uint8Array-Carrier
+ ReadonlyArray<Sample>-Returns).

**Tests:** `crates/idl-ts/src/lib.rs::tests::c_3_wire_format_uses_uint8_array_with_xcdr2_carrier`.

**Status:** `done`

### §C.4 Browser vs Node.js Bindings

**Spec:** Annex C §C.4 — Browser via wasm-bindgen,
Node.js via N-API (optional).

**Repo:** `crates/idl-ts/src/runtime/wasm.ts::WasmBackend` (gleiche
API für beide; Backend-Wahl via `bindWasmBackend(impl)`).

**Tests:** `crates/idl-ts/src/lib.rs::tests::c_4_browser_node_backend_via_bind_wasm_backend`.

**Status:** `done`

### §C.5 Conformance Tests

**Spec:** Annex C §C.5 — 4 Pflicht-Round-Trip-Tests.

**Repo:** `crates/idl-ts/src/runtime/test_backend.ts`
(In-Memory-Backend `createInMemoryBackend()` als
`WasmBackend`-Reference-Implementation für §C.5.1
round-trip-publish/subscribe, §C.5.2 listener-callback,
§C.5.3 resource-cleanup, §C.5.4 DdsAny-round-trip).

**Tests:** `crates/idl-ts/src/lib.rs::tests::c_5_round_trip_reference_backend_is_pure_typescript`,
`c_6_reservation_unused_identifiers_not_collided`.

**Status:** `done`

### §C.6 Reservation of Future Extensions

**Spec:** Annex C §C.6 — Reserved-Identifier-Liste.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Annex D — Diagnostic Codes (Normative)

### §D.1 Diagnostic Code Registry

**Spec:** Annex D — Codes E001 (reserved), E002 (forward-decl
orphan), E003 (annotation conflict), E004 (strict-annotations),
W001 (reserved), W002 (unknown annotation), W003 (map key
hazard), W004 (implicit default), I001 (long double info).

**Repo:** `crates/idl-ts/src/lib.rs::{Diagnostic, Severity,
CodegenConfig, generate_ts_source_with_diagnostics,
generate_ts_source_with_config, check_forward_declaration_orphans,
scan_long_double_uses, scan_union_implicit_defaults,
scan_map_key_hazards, scan_unknown_annotations,
scan_annotation_conflicts}`.

**Tests:** `tests::diagnostic_clean_input_yields_no_diagnostics`,
`tests::diagnostic_duplicate_member_id_emits_e003_fatal`,
`tests::diagnostic_known_annotations_yield_no_w002`,
`tests::diagnostic_long_double_emits_i001_per_field`,
`tests::diagnostic_map_struct_key_emits_w003`,
`tests::diagnostic_multiple_extensibility_e003_fatal`,
`tests::diagnostic_orphan_forward_decl_is_e002_fatal`,
`tests::diagnostic_union_without_default_emits_w004`,
`tests::diagnostic_unknown_annotation_strict_mode_e004_fatal`,
`tests::diagnostic_unknown_annotation_warns_w002`
(10 Tests in `crates/idl-ts/src/lib.rs` gegen alle implementierten
Codes E002/E003/E004/W002/W003/W004/I001).

**Status:** `done` — alle nicht-reservierten Codes (E002, E003,
E004, W002, W003, W004, I001) implementiert; E001/W001 sind
spec-reserviert (kein Code-Pfad).

---

## Audit-Status

59 done / 0 partial / 0 open / 13 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-idl-ts` — 149 Tests grün
(`lib::tests::*` 134, integration `tests::*` 15).

Keine offenen Punkte; siehe `dds-ts-1.0.open.md`.
