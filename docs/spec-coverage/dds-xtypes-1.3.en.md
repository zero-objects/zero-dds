# DDS X-Types 1.3 — Spec Coverage

**Spec:** [OMG DDS-XTypes 1.3](https://www.omg.org/spec/DDS-XTypes/1.3/PDF) (340 pages, OMG formal/2020-10-04)

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

**Context:** XTypes is spread across two crates:

- `crates/types/` — type system, live with ~25 files + 255 tests (assignability/builder/dynamic/error/hash/lib/qos/resolve/type_identifier/type_information/type_lookup/type_matcher/type_object)
- `crates/cdr/` — DataRepresentation (XCDR1+XCDR2 encoder/decoder + extensibility paths), 115 tests

XTypes stack: type system + TypeObject + TypeLookup + assignability +
DynamicType stack + DataRepresentation (CDR/PL_CDR/CDR2) live.

---

## §1 Scope

### 1.1 Four concerns: TypeSystem / TypeRepresentation / DataRepresentation / LanguageBinding

**Spec:** §1, p. 1.

**Repo:** crate separation by concern: `crates/types`,
`crates/idl`, `crates/cdr`, `crates/xml`.

**Tests:** crate-wide.

**Status:** done

---

## §2 Conformance Profiles (Minimal / Basic / Complete)

### 2.0 Three profiles: Minimal / Basic / Complete

**Spec:** §2, p. 1.

**Repo:** stack at Complete level: XCDR2 wire + XCDR1 PL_CDR1
(`crates/cdr/src/xcdr1.rs`) + TypeObject + TypeLookup +
DynamicType + DataRepresentation QoS.

**Tests:** XCDR tests + TypeObject tests + TypeLookup tests +
XCDR1-PL_CDR1 tests in `crates/cdr/src/xcdr1.rs::tests`.

**Status:** done

### 2.1 Programming interface = Extensible-and-Dynamic-Types profile

**Spec:** §2.1, p. 2 — "Complete-level API with DynamicType."

**Repo:** `crates/types/src/dynamic/` with 10 files (factory/data/
builder/try_construct/bridge/builtin_types/descriptor/error/type_/mod).

**Tests:** dynamic tests.

**Status:** done — DCPS API live; dynamic stack complete.

### 2.2.1 Minimal network interop: TypeObject + TopicModel + DataRep-QoS v2 + XCDR2 + built-in types

**Spec:** §2.2.1.

**Repo:** XCDR2 (`crates/cdr/src/struct_enc.rs`) + TypeObject
(`crates/types/src/type_object/`) + built-in topic data
(`crates/dcps/src/builtin_topics.rs`); DataRepresentation-QoS wire
(`crates/qos/src/data_representation.rs` + `crates/rtps/src/parameter_list.rs::pid::DATA_REPRESENTATION`).

**Tests:** XCDR2 tests + TypeObject tests + wire vectors in
`crates/types/tests/compliance_typeobject.rs` (indirect-hash 8 tests).

**Status:** done

### 2.2.2 Basic network interop: + built-in TypeLookup service

**Spec:** §2.2.2.

**Repo:** TypeLookup service end-to-end.

**Tests:** `cyclone_live_typelookup` tests.

**Status:** done

### 2.3 Optional XTYPES 1.1 profile (encoding version 1)

**Spec:** §2.3.

**Repo:** `crates/cdr/src/xcdr1.rs` with PL_CDR1 encoder/decoder
(standard + long header via PID_EXTENDED, sentinel termination).

**Tests:** `crates/cdr/src/xcdr1.rs::tests`:
`xcdr1_pl_cdr_long_header_roundtrip`,
`extended_header_for_id_above_threshold`,
`xcdr1_member_id_above_threshold_uses_extended_pid`,
`xcdr1_member_id_just_below_threshold_uses_standard`,
`xcdr1_sentinel_terminates_decode`,
`xcdr1_truncated_extended_header_rejected`,
`xcdr1_truncated_body_rejected`.

**Status:** done

### 2.4 Optional XML Data Representation profile

**Spec:** §2.4 — XML wire format as an alternative sample
representation alongside CDR-1/CDR-2.

**Repo:** `crates/zerodds-xml-wire/src/{codec,emitter,parser,validator,
xsd}.rs` (1465 SLOC) — XML↔CDR codec + XSD schema gen + streaming
parser.

**Tests:** inline tests in the `zerodds-xml-wire` crate.

**Status:** done

---

## §3 Normative References

### 3.0 [DDS] DDS 1.4 / [RTPS] RTPS 2.3 / [IDL] IDL 4.2 / [CDR] CDR 3.1 / [IEEE-754] / [Unicode 9.0]

**Spec:** §3.

**Repo:** implemented via DCPS/RTPS/IDL/CDR crates + native f32/f64
for IEEE-754 + Rust stdlib UTF-8/UTF-16 for Unicode.

**Tests:** see per spec coverage.

**Status:** done

---

## §6 Tab.1: Concerns

### 6.0 Four concerns mapping (TypeSystem/TypeRepr/DataRepr/LanguageBinding)

**Spec:** §6 Tab.1.

**Repo:** see §1.1.

**Tests:** —

**Status:** done

---

## §6 Tab.2: Features

### 6.0 Features: Single-Inheritance/Type-Versioning/Sparse Types/IDL/XSD/XML/TypeObject/CDR/PL_CDR/XML/Plain/Dynamic Binding

**Spec:** §6 Tab.2.

**Repo:** TypeObject + IDL + CDR + PL_CDR + dynamic binding (via
dynamic/ stack) + sparse via @optional.

**Tests:** —

**Status:** done

---

## §7.2.1 Type Evolution

### 7.2.1.1 Type evolution (added fields, default-on-subscriber)

**Spec:** §7.2.1.1.

**Repo:** XCDR2 appendable/mutable with DHEADER+EMHEADER.

**Tests:** appendable+mutable tests.

**Status:** done

### 7.2.1.2 Type inheritance (IS-A, single inheritance)

**Spec:** §7.2.1.2.

**Repo:** struct/union base_type in TypeObject + assignability.

**Tests:** inheritance tests.

**Status:** done

### 7.2.1.3 Sparse types via @optional

**Spec:** §7.2.1.3.

**Repo:** optional bit on member; PL_CDR2 encoding.

**Tests:** optional-member tests.

**Status:** done

---

## §7.2.2 Type System Model

### 7.2.2.0 Type {kind, nested, extensibility_kind, base_type, element_type}

**Spec:** §7.2.2.

**Repo:** `crates/types/src/type_identifier/kinds.rs::TypeKind` +
`type_object/kinds.rs::TypeFlags`.

**Tests:** kinds tests.

**Status:** done

### 7.2.2.1 Namespaces: modules + types-as-namespaces, fully qualified name

**Spec:** §7.2.2.1.

**Repo:** module/scope path in `crates/idl/src/semantics/` +
`crates/types/src/resolve.rs`.

**Tests:** resolve tests.

**Status:** done

### 7.2.2.2 Primitive types: BOOLEAN/BYTE/INT8/UINT8/INT16/UINT16/INT32/UINT32/INT64/UINT64/FLOAT32/FLOAT64/FLOAT128/CHAR8/CHAR16

**Spec:** §7.2.2.2 — 15 TypeKinds.

**Repo:** all 15 in the TK enum (`type_identifier/kinds.rs`).

**Tests:** primitive tests.

**Status:** done

### 7.2.2.2.1.2 Char encoding: Char8 unspec (UTF-8 default), Char16 = UTF-16 BMP

**Spec:** §7.2.2.2.1.2.

**Repo:** UTF-8 strings + UTF-16 in the CDR path (`crates/cdr/src/encode.rs`).

**Tests:** string-encoding tests.

**Status:** done

### 7.2.2.3 StringTypes: STRING8 (Char8) + STRING16 (Char16), bounded/unbounded

**Spec:** §7.2.2.3.

**Repo:** `type_identifier/mod.rs::StringSTypes`/`StringLTypes`.

**Tests:** —

**Status:** done

### 7.2.2.4 Constructed types: Enumeration/Bitmask/Alias/Array/Sequence/Map/Structure/Union

**Spec:** §7.2.2.4 — 8 constructed kinds.

**Repo:** `type_object/complete/` with all 8 constructed kinds.

**Tests:** constructed tests.

**Status:** done

### 7.2.2.4.1 EnumeratedType: bit_bound (1..32), root=false, FINAL/APPENDABLE

**Spec:** §7.2.2.4.1.

**Repo:** `type_object/complete/` enum type with bit_bound +
extensibility.

**Tests:** enum tests.

**Status:** done

### 7.2.2.4.1.1 Enumeration: Int32 literals, ordered

**Spec:** §7.2.2.4.1.1.

**Repo:** EnumeratedLiteral + value field.

**Tests:** —

**Status:** done

### 7.2.2.4.1.2 Bitmask: bound 1..64, named bits

**Spec:** §7.2.2.4.1.2.

**Repo:** Bitmask + Bitflag with position.

**Tests:** bitmask tests.

**Status:** done

### 7.2.2.4.2 Alias: typedef -> base_type, no own kind in comparison

**Spec:** §7.2.2.4.2.

**Repo:** `type_object/complete/alias_type.rs`; resolver follows base.

**Tests:** alias tests.

**Status:** done

### 7.2.2.4.3 Collection: Array (multi-dim, fixed), Sequence (1-dim, bound opt), Map (key,value)

**Spec:** §7.2.2.4.3.

**Repo:** `type_identifier/mod.rs` Array/Sequence/Map +
`type_object/complete/collection_types.rs`.

**Tests:** collection tests.

**Status:** done

### 7.2.2.4.3 Collection — bound enforcement in the typed codegen encode

**Spec:** §7.2.2.4.3 + §7.4.3 — a bounded `sequence<T,N>` / `string<N>` /
`wstring<N>` / `map<K,V,N>` with more than `N` elements is a bound violation at
serialization; strict vendors reject it on the wire (OpenDDS: `CORBA::BAD_PARAM`
on `write`).

**Repo:** The typed codegen encode enforces the bound in all four language
codegens:
- `crates/idl-rust/src/struct_emit.rs` (`emit_bound_checks` /
  `emit_bound_checks_decl` / `type_has_bounds`) + `union_emit.rs` — seq
  (top-level + nested + array element), narrow string (UTF-8 byte), wstring
  (UTF-16 unit), map (size + keys + values), union arms.
- `crates/idl-java` + `crates/idl-csharp` TypeSupport encode — seq, narrow
  string, wstring (`IllegalArgumentException` / `ArgumentException`).
- `crates/idl-cpp/src/emitter.rs` (both value emitters) — seq (incl. nested
  via recursion), narrow string, wstring (`std::length_error`, conditional
  `<stdexcept>`).

**Tests:** `idl-rust/tests/bounded_sequence.rs` (10),
`idl-java/tests/bounded_collections.rs` (3),
`idl-csharp/tests/bounded_collections.rs` (3),
`idl-cpp/tests/bounded_collections.rs` (3).

**Status:** done — all four language codegens enforce the bound: idl-rust/
idl-java/idl-csharp + idl-cpp (seq incl. nested via recursion, narrow string,
wstring), plus map/union arms where the codegen supports the type.

### 7.2.2.4.4 AggregatedType — idl-cpp XCDR2 encode of sequence<@appendable/@mutable struct>

**Spec:** §7.2.2.4.4 + §7.4.3.

**Repo:** `crates/idl-cpp/src/emitter.rs` — the whole idl-cpp XCDR2 codegen is
complete. Nested @appendable/@mutable struct *members* are encoded
(`scoped_struct` splice), and the former remaining case —
`sequence<@appendable struct>` / `sequence<@mutable struct>` — is now closed:
`typespec_supported`'s Sequence arm accepts struct elements of any
extensibility, and each non-final element is 4-aligned + spliced/sub-decoded
through its own DHEADER (encode + decode, including the @mutable-member path via
`__body_origin`).

**Tests:** `v_sequence_of_appendable_struct` (byte-exact `1C…` seq DHEADER +
per-element Pt DHEADER + roundtrip) + `v_mutable_member_seq_of_appendable_struct`
(roundtrip in the @mutable-member path); member case still green
(`v_nested_appendable_struct_byte_exact`, `v_nested_mutable_struct_roundtrips`).

**Status:** done — the idl-cpp XCDR2 encoder covers the full type system, no
remaining gap.

### 7.2.2.4.4 AggregatedType: Struct/Union/Annotation, Member {id, key, must_understand, optional, shared, member_index}

**Spec:** §7.2.2.4.4.

**Repo:** MemberFlag bits in `type_object/common.rs`.

**Tests:** member tests.

**Status:** done

### 7.2.2.4.4.2 Structure: optional base_type

**Spec:** §7.2.2.4.4.2.

**Repo:** `type_object/complete/struct_type.rs::base_type`.

**Tests:** —

**Status:** done

### 7.2.2.4.4.3 Union: discriminator member always first, must_understand=true; allowed disc types

**Spec:** §7.2.2.4.4.3.

**Repo:** `type_object/complete/union_type.rs` with Discriminator +
UnionCase + UnionDiscriminator.

**Tests:** union tests.

**Status:** done

### 7.2.2.4.4.4.1 Member name unique within aggregate

**Spec:** §7.2.2.4.4.4.1.

**Repo:** validation in `crates/types/src/builder.rs`.

**Tests:** builder tests.

**Status:** done

### 7.2.2.4.4.4.4 Member IDs 0..0x0FFFBFFF; OMG reserves the upper range

**Spec:** §7.2.2.4.4.4.4.

**Repo:** MemberId u32 with mask in `common.rs`.

**Tests:** —

**Status:** done

### 7.2.2.4.4.4.5 Member name hash: first 4 bytes of MD5(UTF-8(name))

**Spec:** §7.2.2.4.4.4.5.

**Repo:** `crates/types/src/hash.rs`.

**Tests:** hash tests.

**Status:** done

### 7.2.2.4.4.4.6 must_understand attribute: discard behavior; union discriminator always must_understand=true

**Spec:** §7.2.2.4.4.4.6.

**Repo:** must_understand bit in `common.rs` + validation.

**Tests:** —

**Status:** done

### 7.2.2.4.4.4.7 Optional members: union member never optional; struct free

**Spec:** §7.2.2.4.4.4.7.

**Repo:** validation in `builder.rs`: discriminator+key never optional.

**Tests:** —

**Status:** done

### 7.2.2.4.4.4.8 Key members: only prim/aggr/enum/bitmask/array/sequence/alias; never map; recursive

**Spec:** §7.2.2.4.4.4.8.

**Repo:** key validation in `builder.rs` + KeyHolder/KeyErased in
`assignability.rs`.

**Tests:** key tests.

**Status:** done

### 7.2.2.4.4.4.9 Default member value: explicit (annotation) or implicit (Tab.9)

**Spec:** §7.2.2.4.4.4.9.

**Repo:** `crates/types/src/builder.rs::set_member_default` (StructMember
builder with `default_value: Option<String>`);
`crates/types/src/type_object/common.rs::AppliedBuiltinMemberAnnotations.default_value`
(wire trailer, backwards-compat — old decoders leave the trailer
in place, new ones read it). IDL lowering: `BuiltinAnnotation::Default(value)`
→ `Member-Builder.set_member_default`.

**Tests:** `crates/types/src/builder.rs::tests`:
`member_with_explicit_default_used_when_field_missing`,
`default_overrides_implicit_zero`,
`default_value_roundtrips_via_encode_decode`.

**Status:** done

### 7.2.2.4.4 Tab.9 Defaults: 0/false/'\0'/""/empty coll./first enum literal/disc default

**Spec:** §7.2.2.4.4 Tab.9.

**Repo:** default constructor per TK in `common.rs`.

**Tests:** —

**Status:** done

### 7.2.2.4.5 Single inheritance Struct/Union (same extensibility, unique IDs/names, derived has no keys)

**Spec:** §7.2.2.4.5.

**Repo:** `crates/types/src/assignability.rs::flatten_inheritance` with
`InheritanceError::InheritanceConflict` detection (member ID + name hash);
TypeObject side `crates/types/src/type_object/minimal/struct_type.rs`
with `MinimalStructHeader.base_type`.

**Tests:** `crates/types/src/assignability.rs::tests`:
`flatten_inheritance_no_base_returns_struct_unchanged`,
`flatten_inheritance_two_levels_concatenates_base_first`,
`inheritance_conflict_same_id`, `inheritance_conflict_same_name`,
`flat_type_construction_two_levels`,
`two_level_inheritance_assignability_chain`.

**Status:** done

### 7.2.2.4.6 KeyErased(T) — key designation removed

**Spec:** §7.2.2.4.6.

**Repo:** KeyErased type in `assignability.rs`.

**Tests:** —

**Status:** done

### 7.2.2.4.7 KeyHolder(T) — keep only key members

**Spec:** §7.2.2.4.7.

**Repo:** KeyHolder helper in `assignability.rs`.

**Tests:** —

**Status:** done

### 7.2.2.4.8 VerbatimText {language, placement, text}

**Spec:** §7.2.2.4.8.

**Repo:** TypeObject wire form: `crates/types/src/type_object/common.rs`
with `AppliedVerbatimAnnotation` + `VerbatimPlacement` enum +
`AppliedBuiltinTypeAnnotations` (wire roundtrip).
IDL lowering: `crates/idl/src/semantics/annotations.rs::VerbatimSpec`
+ `PlacementKind` + `Lowered::verbatims_for_language(aliases)`.
Codegen paths per language:

- `crates/idl-cpp/src/verbatim.rs` (aliases: `c++`, `cpp`, `cxx`, `*`)
  + hooks in `emit_struct`/`emit_enum`/`emit_union`/`emit_typedef`/
  `emit_header` for all 6 PlacementKinds.

- `crates/idl-csharp/src/verbatim.rs` (aliases: `c#`, `csharp`, `cs`, `*`).
- `crates/idl-java/src/verbatim.rs` (alias: `java`, `*`).

**Tests:**
- TypeObject wire: `crates/types/src/type_object/common.rs::tests::
  verbatim_placement_roundtrip_{before,after,begin_file,end_file,
  other_forward_compat}` (5 tests).
- IDL lowering: `crates/idl/src/semantics/annotations.rs::tests::
  {verbatim_no_params_uses_defaults, verbatim_compact_form_takes_text,
  verbatim_named_params_full, verbatim_all_placement_kinds,
  verbatim_unknown_placement_falls_back_to_default}`.
- Codegen C++: `crates/idl-cpp/tests/spec_conformance.rs::
  {verbatim_annotation_with_cpp_language_inlines_text,
  verbatim_annotation_with_after_declaration_placement,
  verbatim_annotation_wildcard_language_applies,
  verbatim_annotation_other_language_skipped}`.
- Codegen C#: `crates/idl-csharp/tests/spec_conformance.rs::
  {verbatim_annotation_with_csharp_language_inlines_text,
  verbatim_annotation_csharp_alias_cs_matches,
  verbatim_annotation_other_language_not_emitted_in_csharp}`.
- Codegen Java: `crates/idl-java/tests/spec_conformance.rs::
  {verbatim_annotation_with_java_language_inlines_text,
  verbatim_annotation_with_after_declaration_placement,
  verbatim_annotation_other_language_skipped_in_java}`.

**Status:** done — code-gen templating cross-cutting fully fulfilled
(C++, C#, Java).

### 7.2.2.4.9 External Data (@external): incomplete forward ref + self-ref allowed

**Spec:** §7.2.2.4.9.

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag::IS_EXTERNAL`;
forward resolution via `crates/types/src/resolve.rs::TypeRegistry::transitive_dependencies`
with a seen-set (prevents infinite loop even with circular
@external references).

**Tests:** `crates/types/src/resolve.rs::tests`:
`external_forward_decl_roundtrip` (self-forward + wire roundtrip
of the EXTERNAL flag),
`external_diamond_resolves_through_both_paths`
(diamond A→B→D, A→C→D yields each hash exactly once).

**Status:** done

### 7.2.2.5 Nested types (not usable as a topic)

**Spec:** §7.2.2.5.

**Repo:** nested bit on TypeFlag.

**Tests:** —

**Status:** done

### 7.2.2.6 AnnotationType: AggregatedType with typed parameter

**Spec:** §7.2.2.6.

**Repo:** AnnotationType in
`type_object/complete/annotation_type.rs`.

**Tests:** —

**Status:** done

### 7.2.2.7 / Tab.10/11 TryConstruct {DISCARD, USE_DEFAULT, TRIM} default DISCARD; TRIM only string/seq/map

**Spec:** §7.2.2.7 Tab.10/11.

**Repo:** `dynamic/try_construct.rs::apply_try_construct` + member
flags.

**Tests:** TryConstruct tests.

**Status:** done

---

## §7.2.3 Extensibility Tab.12

### 7.2.3 Extensibility: FINAL / APPENDABLE / MUTABLE

**Spec:** §7.2.3 Tab.12.

**Repo:** extensibility bits in `type_object/kinds.rs` + CDR
encoding.

**Tests:** extensibility tests.

**Status:** done

---

## §7.2.4 Type Compatibility (is-assignable-from)

### 7.2.4.0 is-assignable-from relation, T1<-T2

**Spec:** §7.2.4.

**Repo:** assignability engine in `assignability.rs`.

**Tests:** assignability tests.

**Status:** done

### 7.2.4.1 Construction: T1.is-assignable-from(T2) -> T2 subset builds T1; else TryConstruct

**Spec:** §7.2.4.1.

**Repo:** is-assignable-from + DynamicData::create + try_construct.rs.

**Tests:** —

**Status:** done

### 7.2.4.2 Delimited types: APPENDABLE in v2, MUTABLE in v1+v2

**Spec:** §7.2.4.2.

**Repo:** delimited tag in encoder, DHEADER/PL sentinel.

**Tests:** delimited tests.

**Status:** done

### 7.2.4.3 Strong assignability: T1≡T2 (MINIMAL) ∨ (T1 is-assignable T2 ∧ T2 delimited)

**Spec:** §7.2.4.3.

**Repo:** strong-assignable helper.

**Tests:** —

**Status:** done

### 7.2.4.4.1 Equivalent (MINIMAL) -> mutually assignable

**Spec:** §7.2.4.4.1.

**Repo:** equality path.

**Tests:** —

**Status:** done

### 7.2.4.4.2 non_serialized member ignored in compat check

**Spec:** §7.2.4.4.2 — members marked `@non_serialized` MUST be omitted
from any wire form and from assignability comparisons.

**Repo:** `crates/idl/src/semantics/annotations.rs`
(`BuiltinAnnotation::NonSerialized` + `lower_single("non_serialized")`);
`crates/idl/src/semantics/to_typeobject.rs::lower_struct_to_minimal`
(skip path before TypeObject member inclusion); `dynamic_apply` passthrough.

**Tests:** `crates/idl/src/semantics/to_typeobject.rs::tests`:
`non_serialized_member_is_dropped_from_typeobject`,
`non_serialized_in_otherwise_empty_struct_yields_empty_member_seq`,
`non_serialized_with_other_annotations_still_dropped`,
`non_serialized_member_does_not_block_assignability`.

**Status:** done

### 7.2.4.4.3 / Tab.14 Alias: passed through to base_type

**Spec:** §7.2.4.4.3.

**Repo:** alias resolution.

**Tests:** —

**Status:** done

### 7.2.4.4.4 / Tab.15 Primitive: same kind only

**Spec:** §7.2.4.4.4.

**Repo:** exact match rule.

**Tests:** —

**Status:** done

### 7.2.4.4.5 / Tab.16 String: same wide/narrow + element_type assignable + length check

**Spec:** §7.2.4.4.5.

**Repo:** string rule.

**Tests:** —

**Status:** done

### 7.2.4.4.6 / Tab.17 Array (same bounds, strongly), Sequence (strongly el), Map (strongly key+el)

**Spec:** §7.2.4.4.6.

**Repo:** collection rule with TryConstruct.

**Tests:** —

**Status:** done

### 7.2.4.4.7 / Tab.18 Bitmask↔UInt-by-bound; Enum compat (FINAL ⇒ same literals); @ignore_literal_names

**Spec:** §7.2.4.4.7 — default enum compat compares (value, name);
`@ignore_literal_names` reduces to (value).

**Repo:** `crates/types/src/assignability.rs` (enum comparison
considers `cfg.ignore_literal_names` OR
`EnumTypeFlag::IGNORE_LITERAL_NAMES` on one of the sides);
`crates/types/src/type_object/flags.rs::EnumTypeFlag::IGNORE_LITERAL_NAMES`;
`crates/idl/src/semantics/annotations.rs::BuiltinAnnotation::IgnoreLiteralNames`
+ `lower_single("ignore_literal_names")`.

**Tests:** `crates/types/src/assignability.rs::tests`:
`enum_not_assignable_strict_default`,
`enum_assignable_with_ignore_literal_names`,
`enum_assignable_with_ignore_literal_names_via_writer_flag`,
`enum_assignable_with_ignore_literal_names_via_reader_flag`,
plus existing `enum_identical_labels_is_yes`,
`enum_mismatch_writer_literal_unknown_in_reader_is_no`,
`enum_bit_bound_mismatch_is_no`.

**Status:** done

### 7.2.4.4.8 / Tab.19 Aggregated (Union/Struct with member IDs/Key-Erased/Appendable/Final/KeyHolder)

**Spec:** §7.2.4.4.8 — aggregated edge cases: union default coverage,
map-key whitelist, two-level inheritance assignability.

**Repo:** struct + union assignability in
`crates/types/src/assignability.rs` (incl. `flatten_inheritance`);
map-key validator `crates/idl/src/semantics/map_validation.rs`;
union default coverage in `crates/idl/src/semantics/union_validation.rs`.

**Tests:**

- `crates/idl/src/semantics/union_validation.rs::tests::union_default_coverage_required_when_partial_range`
- `crates/idl/src/semantics/map_validation.rs::tests::map_key_element_type_must_be_primitive` (+ 5 edge cases)
- `crates/types/src/assignability.rs::tests::two_level_inheritance_assignability_chain`,
  `flat_type_construction_two_levels`,
  `flatten_inheritance_two_levels_concatenates_base_first`,
  `inheritance_conflict_same_id`, `inheritance_conflict_same_name`.

**Status:** done

---

## §7.3 Type Representation

### 7.3.0 Four TypeReprs (Tab.20): IDL / XML / XSD / TypeObject

**Spec:** §7.3 Tab.20.

**Repo:** IDL (`crates/idl/`) + TypeObject (`crates/types/`) +
XML (`crates/xml/`) live. The fourth representation, XSD, is a deliberately
deferred Iron-Rule extension (§7.3.3) — three of the four representations are
implemented.

**Tests:** crate-wide test suites plus `crates/idl/`,
`crates/types/`, `crates/xml/` tests for all three active
representations.

**Status:** done — IDL/XML/TypeObject complete; XSD as an
Iron-Rule open item via §7.3.3.

### 7.3.1.1 IDL compatibility: Extensible-DDS profile of IDL 4.2

**Spec:** §7.3.1.1.

**Repo:** `crates/idl/src/grammar/idl42.rs`.

**Tests:** see `idl-4.2.md`.

**Status:** done

### 7.3.1.1.1 `#pragma dds_xtopics begin/end [version]`

**Spec:** §7.3.1.1.1.

**Repo:** `crates/idl/src/preprocessor/mod.rs`
(`PragmaDdsXtopics { version, file, line }` +
`parse_pragma_dds_xtopics` reader; `ProcessedSource.pragma_dds_xtopics`).

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests`:
`pragma_dds_xtopics_version_match`,
`pragma_dds_xtopics_version_mismatch_warns`,
`pragma_dds_xtopics_nested_pragmas_handled`,
`pragma_dds_xtopics_without_version_value_is_empty`,
`pragma_dds_xtopics_unquoted_version_accepted`.

**Status:** done

### 7.3.1.2.1 IDL annotations (40+ annotations)

**Spec:** §7.3.1.2.1.

**Repo:** `crates/idl/src/semantics/annotations.rs` with 28 built-in
annotations: `@key`, `@id(n)`, `@optional`, `@must_understand`,
`@external`, `@non_serialized`, `@ignore_literal_names`, `@default(v)`,
`@default_literal`, `@extensibility(...)`, `@final`, `@appendable`,
`@mutable`, `@autoid(...)`, `@topic`, `@nested`, `@unit("...")`,
`@hashid("...")`, `@range(min, max)`, `@min(v)`, `@max(v)`,
`@value(v)`, `@position(n)`, `@bit_bound(n)`, `@verbatim(...)`,
`@ami(b)`, `@service(s)`, `@oneway(b)`.

**Tests:** annotation tests in `annotations.rs::tests`,
`semantics::to_typeobject::tests`, `semantics::dynamic_apply::tests`.
The `@verbatim` code-gen hook is live in C++/C#/Java (see §7.2.2.4.8
above).

**Status:** done

### 7.3.1.2.1.1 @id / @hashid; @autoid(SEQUENTIAL|HASH); MD5 algo

**Spec:** §7.3.1.2.1.1.

**Repo:** annotations.rs + `hash.rs`.

**Tests:** —

**Status:** done

### 7.3.1.3 Const+Expressions: compile-time eval

**Spec:** §7.3.1.3.

**Repo:** `crates/idl/src/semantics/const_eval.rs` (`evaluate` with
SymbolTable resolution + integer/float/bitwise/shift ops);
nested `#define` refs via `crates/idl/src/preprocessor/mod.rs::expand_macros`
with `MAX_MACRO_EXPANSION_DEPTH = 32` cycle cap.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests`:
`nested_define_two_hops`, `nested_define_three_hops`,
`nested_define_with_arithmetic_expression`,
`nested_define_self_recursive_terminates`,
`nested_define_mutually_recursive_terminates`,
plus `crates/idl/src/semantics/const_eval.rs::tests` (existing).

**Status:** done

### 7.3.2 XML TypeRepr: namespace + simpleType hierarchy

**Spec:** §7.3.2 Tab.26.

**Repo:** `crates/xml/src/xtypes_parser.rs` + `xtypes_def.rs` with
`<struct>`/`<enum>`/`<union>`/`<typedef>`/`<bitmask>`/`<bitset>`/
`<module>` plus the constructs `<include>`, `<forward_dcl>`,
`<const>` (`IncludeEntry`, `ForwardDeclEntry`, `ConstEntry`).

**Tests:** `crates/xml/src/xtypes_parser.rs::tests`:
`parse_include_element`, `parse_include_missing_file_attribute_rejected`,
`parse_forward_dcl_element`, `parse_forward_dcl_default_kind_struct`,
`parse_const_element`, `parse_const_missing_value_attribute_rejected`,
plus existing `parse_simple_struct`, `parse_module_nested`,
`dds_root_with_multiple_types_blocks`.

**Status:** done

### 7.3.3 XSD Type Representation

**Spec:** §7.3.3.

**Repo:** `crates/xml/src/xsd_schema.rs::parse_xsd_schema` parses W3C
XSD schemas onto the internal `TypeLibrary` data model; via the existing
`crates/xml/src/typeobject_bridge.rs::bridge_library` a
`MinimalTypeObject` is produced from it. The mapping covers:
- `<xsd:complexType><xsd:sequence>...</xsd:sequence>` -> struct.
- `<xsd:simpleType>` with `<xsd:enumeration>` -> enum.
- `<xsd:complexContent><xsd:extension base="...">` -> inheritance.
- minOccurs=0 -> optional, maxOccurs=unbounded -> sequence,
  maxOccurs=N>1 -> bounded sequence.
- Built-in: `xsd:int`/`xsd:long`/`xsd:short`/`xsd:byte`/`xsd:double`/
  `xsd:float`/`xsd:string`/`xsd:boolean` + unsigned variants.

**Tests:** `xsd_schema::tests::*` (12 tests):
`complex_content_extension_yields_inheritance`,
`complex_type_maps_to_struct_with_primitive_members`,
`empty_schema_yields_empty_library`,
`max_occurs_bounded_yields_bounded_sequence`,
`max_occurs_unbounded_yields_sequence`,
`min_occurs_zero_yields_optional_member`,
`non_schema_root_is_rejected`,
`simple_type_with_enumeration_maps_to_dds_enum`,
`unsigned_xsd_types_map_to_unsigned_dds_primitives`,
`user_type_reference_yields_named_typeref`,
`xsd_long_maps_to_dds_longlong`,
`xsd_to_typeobject_via_bridge_produces_minimal_struct`.

**Status:** done

---

## §7.3.4 TypeIdentifier / TypeObject

### 7.3.4.0 TypeIdentifier (hash + plain variants); TypeObject (Minimal+Complete)

**Spec:** §7.3.4.

**Repo:** `crates/types/src/type_identifier/` + `type_object/`.

**Tests:** TypeIdentifier+TypeObject tests.

**Status:** done

### 7.3.4.6 Indirect Hash TypeIdentifiers (PlainCollectionHeader with EquivalenceKind=EK_MINIMAL/COMPLETE)

**Spec:** §7.3.4.6 + §7.3.4.7.1 — `PlainCollectionHeader { equiv_kind;
element_flags }` plus `EK_MINIMAL`/`EK_COMPLETE` as element discriminator.

**Repo:** `crates/types/src/type_identifier/mod.rs`
(`PlainCollectionHeader`, `EquivalenceHash`, `encode_into`/`decode_from`).

**Tests:** `crates/types/tests/compliance_typeobject.rs`:
`indirect_hash_plain_sequence_small_minimal_wire_vector`,
`indirect_hash_plain_sequence_large_complete_wire_vector`,
`indirect_hash_plain_array_small_minimal_wire_vector`,
`indirect_hash_plain_array_large_complete_wire_vector`,
`indirect_hash_plain_map_small_minimal_keys_complete_values_wire_vector`,
`indirect_hash_plain_map_large_complete_keys_minimal_values_wire_vector`,
`indirect_hash_nested_sequence_of_sequence_minimal_wire_roundtrip`,
`indirect_hash_invalid_equiv_kind_in_header_decodes_as_unknown_ek_byte`.

**Status:** done

### 7.3.4.8/9 Mutual-Dependency / SCC

**Spec:** §7.3.4.8 (mutual dependency) + §7.3.4.9 (SCC IDs).

**Repo:** `crates/types/src/resolve.rs`
(`TypeRegistry::transitive_dependencies` with a seen-set for cycle
detection; `StronglyConnectedComponentId` in
`crates/types/src/type_identifier/mod.rs`).

**Tests:** `crates/types/src/resolve.rs::tests`:
`scc_three_element_cycle_resolves_correctly`,
`scc_four_element_cycle_resolves_correctly`,
`scc_diamond_with_cycle_resolves_finitely`,
`scc_self_loop_does_not_explode_node_count`,
plus wire tests `scc_encode_rejects_negative_values`,
`scc_encode_rejects_index_out_of_bounds`,
`strongly_connected_component_large_scc_length_and_index`.

**Status:** done

### 7.3.4.x TypeLookup service (request/reply)

**Spec:** §7.3.4 (TypeLookup).

**Repo:** `type_lookup.rs` + discovery layer.

**Tests:** TypeLookup tests.

**Status:** done

---

## §7.4 Data Representation / CDR

### 7.4.1.1 SerializedPayloadHeader (Representation Identifier)

**Spec:** §7.4.1.1.

**Repo:** `crates/cdr/` with representation identifiers (CDR1/PL_CDR1/
CDR2/PL_CDR2/D_CDR/XML).

**Tests:** representation tests.

**Status:** done

### 7.4.1.2.1 PID_IGNORE 0x3F03

**Spec:** §7.4.1.2.1 — "Used to ignore parameters which can be safely ignored."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::IGNORE = 0x3F03`,
silent-skip path in `from_bytes`).

**Tests:** `crates/rtps/src/parameter_list.rs::tests`:
`pid_ignore_skipped_in_pl_cdr_decode`, `pid_ignore_zero_length_is_valid`,
`pid_ignore_truncated_body_rejected`, `pid_ignore_be_decoded`,
`pid_ignore_ignored_even_when_count_would_exceed_cap`.

**Status:** done

### 7.4.1.2.3 non-optional in mutable encode (MUST contain all non-optional members)

**Spec:** §7.4.1.2.3 — "the serialized representation MUST contain at
least the values of all the non-optional members."

**Repo:** `crates/cdr/src/struct_enc.rs` (`MutableStructEncoder` with
`required_ids` tracking + `finish()` validation); error
`EncodeError::MissingNonOptionalMember { member_id }` in
`crates/cdr/src/error.rs`.

**Tests:** `crates/cdr/src/struct_enc.rs::tests`:
`mutable_encode_omitting_non_optional_member_errors`,
`mutable_struct_encoder_succeeds_when_all_required_emitted`,
`mutable_encode_first_missing_id_is_reported`,
`mutable_encode_optional_only_with_no_required_succeeds`,
`mutable_encode_extra_optional_emitted_does_not_break_finish`,
`mutable_encode_with_lc_variant_tracks_id`.

**Status:** done

### 7.4.2 XCDR Version 1 (Plain CDR1 + PL_CDR1)

**Spec:** §7.4.2 + §7.4.1.2.2 (PID_EXTENDED) — PL_CDR1 with long header
for member IDs >= 0x3F00 or body length > 0xFFFF.

**Repo:** `crates/cdr/src/xcdr1.rs` (PL_CDR1 with `encode_pl_cdr1_member`,
`read_pl_cdr1_member`, `PID_EXTENDED = 0x3F01`,
`PID_LIST_END = 0x3F02`, `PID_EXTENDED_THRESHOLD = 0x3F00`).

**Tests:** `crates/cdr/src/xcdr1.rs::tests`:
`standard_header_for_small_id_and_length`,
`extended_header_for_id_above_threshold`,
`extended_header_for_large_body_length`,
`xcdr1_pl_cdr_long_header_roundtrip`,
`xcdr1_member_id_above_threshold_uses_extended_pid`,
`xcdr1_member_id_just_below_threshold_uses_standard`,
`xcdr1_sentinel_terminates_decode`,
`xcdr1_truncated_extended_header_rejected`,
`xcdr1_truncated_body_rejected`.

**Status:** done

### 7.4.3 XCDR Version 2 (Plain CDR2 + Delimited + PL_CDR2)

**Spec:** §7.4.3.

**Repo:** XCDR2 path in `struct_enc.rs`.

**Tests:** XCDR2 tests.

**Status:** done

### 7.4.3.4.2 LC=6 / LC=7 (4·NEXTINT / 8·NEXTINT)

**Spec:** §7.4.3.4.2.

**Repo:** `crates/cdr/src/struct_enc.rs` (length-code enum +
`encode_mutable_member_lc` with Lc6/Lc7 branch +
`read_mutable_member` with `body_len` computation 4·NEXTINT+4 / 8·NEXTINT+4).

**Tests:** `crates/cdr/src/struct_enc.rs::tests`:
`lc6_array_of_4byte_primitives`, `lc7_array_of_8byte_primitives`,
`lc6_rejects_misaligned_body`,
`lc6_lc7_roundtrip_against_cyclone_sample`,
`lc6_with_many_elements_decodes_correctly` (70_000 elements).

**Status:** done

### 7.4.3.5.3 Rules 11-16 (Maps)

**Spec:** §7.4.3.5.3.

**Repo:** `crates/types/src/builder.rs::MapBuilder::add_map_member(ext)`
returns `BuilderError::MutableMapExtensibilityNotAllowed` when
MUTABLE is set on a map — prevents silent demotion.

**Tests:** `crates/types/src/builder.rs::tests`:
`mutable_map_extensibility_is_error`,
`appendable_map_extensibility_is_ok`,
`final_map_extensibility_is_ok`.

**Status:** done

### 7.4.4 Encoding VM (formal encoding rules)

**Spec:** §7.4.4.

**Repo:** `crates/cdr/src/struct_enc.rs` (mutable + appendable + final
encoder paths); `crates/cdr/src/xcdr1.rs` (PL_CDR1 path);
`crates/cdr/src/key_hash.rs::PlainCdr2BeKeyHolder` (PLAIN_CDR2-BE).
The formal encoding VM is not implemented as a separate bytecode
machine, because all XTypes 1.3 wire constructs are already covered by
these specialized encoder paths and enforce the same
spec rules (DHEADER + EMHEADER + sentinel).

**Tests:** encoder/decoder tests in `crates/cdr/src/struct_enc.rs`,
`crates/cdr/src/xcdr1.rs`, `crates/cdr/src/key_hash.rs`,
`crates/cdr/tests/compliance_xcdr2.rs`,
`crates/types/tests/compliance_typeobject.rs`.

**Status:** done

### 7.4.5 KeyHash computation

**Spec:** §7.4.5 + §7.6.8.3.1.b — KeyHolder members MUST be serialized
in ascending `member_id` order in PLAIN_CDR2-BE, so that
encoders with different member enumeration orders produce the same
KeyHash.

**Repo:** `crates/cdr/src/key_hash.rs` (`compute_key_hash` zero-pad/MD5
path; `keyhash_cdr2_be` with mandatory `sort_by_key(member_id)`);
DCPS wiring `crates/dcps/src/instance_tracker.rs`.

**Tests:** `crates/cdr/src/key_hash.rs::tests`:
`keyhash_cdr2_be_member_order_independent`,
`keyhash_cdr2_be_md5_path_also_order_independent`,
`keyhash_cdr2_be_single_member_matches_compute_key_hash`,
`keyhash_cdr2_be_empty_member_set_yields_zero_padding`,
`keyhash_cdr2_be_member_id_zero_sorts_first`,
plus pipeline tests `keyholder_full_keyhash_pipeline_zero_pad`,
`keyholder_full_keyhash_pipeline_md5`.

**Status:** done

---

## §7.5 Programming Language Binding

### 7.5.1 Plain Language Binding (PSM-specific)

**Spec:** §7.5.1 — cross-spec item: plain language binding is normative
separately per PSM. dds-xtypes-1.3 defines only the hook;
the concrete mappings live in the respective IDL-PSM specs.

**Repo:** per PSM separately: `idl4-cpp-1.0.md` (`crates/idl-cpp/`),
`idl4-java-1.0.md`, `idl4-csharp-1.0.md`. This coverage file
tracks only the XTypes-1.3 wire/type aspects.

**Tests:** PSM-specific tests in the crate suites of the individual
bindings; cross-spec linkage via the coverage docs.

**Status:** `n/a (informative)` — cross-spec linkage; the
XTypes-1.3 section delegates to the respective language PSMs
(`idl4-cpp-1.0.md`, `idl4-java-1.0.md`, `idl4-csharp-1.0.md`,
`dds-ts-1.0.md`).

### 7.5.2 Dynamic Language Binding

**Spec:** §7.5.2 — DynamicType + DynamicData + factories.

**Repo:** `crates/types/src/dynamic/` (10 files with ~3000 LOC).

**Tests:** dynamic tests.

**Status:** done

---

## §7.6 Built-in Topic Types

### 7.6.1 ParticipantBuiltinTopicData / PublicationBuiltinTopicData / SubscriptionBuiltinTopicData / TopicBuiltinTopicData (XTypes extensions)

**Spec:** §7.6.1.

**Repo:** `crates/dcps/src/builtin_topics.rs` + `crates/rtps/src/{participant,publication,subscription}_data.rs`.

**Tests:** built-in topic tests.

**Status:** done — XTypes extensions (TypeInformation + RepresentationInfo)
done.

### 7.6.2 DataRepresentationQosPolicy (XCDR1/XCDR2/XML)

**Spec:** §7.6.2 + §7.6.3.1 Tab.59.

**Repo:** `crates/types/src/qos.rs`
(`DataRepresentationId`, `negotiate_representation`,
`check_data_repr_extensibility` wire-constraint matrix).

**Tests:** `crates/types/src/qos.rs::tests`:
`negotiate_xcdr2_preferred`, `negotiate_fallback_to_xcdr1`,
`negotiate_no_overlap`,
`xcdr1_final_combination_is_allowed`,
`xcdr1_mutable_combination_is_allowed`,
`xcdr1_appendable_is_disallowed`,
`xcdr2_supports_all_three_extensibilities`.

**Status:** done

### 7.6.3 TypeLookupService

**Spec:** §7.6.3 — request/reply types + KeyValue hash + endpoint IDs.

**Repo:** `crates/types/src/type_lookup.rs` +
`crates/discovery/src/type_lookup/`.

**Tests:** TypeLookup tests incl. Cyclone live.

**Status:** done

### 7.6.3.3 TypeLookup responder (reply build with type-object refs)

**Spec:** §7.6.3.3.

**Repo:** `crates/discovery/src/type_lookup/responder.rs`.

**Tests:** responder tests.

**Status:** done

---

## Audit status

84 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-types` — 281 lib + 9 + 40 = 330 tests green
  (TypeObject + TypeLookup + assignability + IDL semantics).
* `cargo test -p zerodds-cdr` — 165 lib + 1 + 7 = 173 tests green
  (XCDR1+XCDR2 for XTypes wire encoding).
* `cargo test -p zerodds-xml-wire` — 40 tests green (§2.4 XML Data
  Representation profile, cross-ref `zerodds-xml-1.0.md`).

No open items: the idl-cpp XCDR2 encoder covers the full type system
(including `sequence<@appendable/@mutable struct>`, §7.2.2.4.4).
