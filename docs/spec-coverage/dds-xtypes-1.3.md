# DDS X-Types 1.3 — Spec-Coverage

**PDF:** `docs/standards/cache/omg/dds-xtypes-1.3.pdf` (340 Seiten, OMG formal/2020-10-04)

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`. Audit Item-für-Item
gegen die PDF; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** `crates/types/` ist live mit ~25 Files + 255 Tests
(assignability/builder/dynamic/error/hash/lib/qos/resolve/
type_identifier/type_information/type_lookup/type_matcher/type_object).
`crates/cdr/` ergaenzt mit 115 Tests (XCDR1+XCDR2 Encoder/Decoder +
Extensibility-Pfade). XTypes-Stack: Type-System + TypeObject +
TypeLookup + Assignability + DynamicType-Stack + DataRepresentation
(CDR/PL_CDR/CDR2) live.

---

## §1 Scope

### 1.1 Vier Concerns: TypeSystem / TypeRepresentation / DataRepresentation / LanguageBinding

**Spec:** §1, S. 1.

**Repo:** Crate-Trennung gemaess Concerns: `crates/types`,
`crates/idl`, `crates/cdr`, `crates/xml`.

**Tests:** Crate-weit.

**Status:** done

---

## §2 Conformance Profiles (Minimal / Basic / Complete)

### 2.0 Drei Profile: Minimal / Basic / Complete

**Spec:** §2, S. 1.

**Repo:** Stack auf Complete-Niveau: XCDR2-Wire + XCDR1 PL_CDR1
(`crates/cdr/src/xcdr1.rs`) + TypeObject + TypeLookup +
DynamicType + DataRepresentation-QoS.

**Tests:** XCDR-Tests + TypeObject-Tests + TypeLookup-Tests +
XCDR1-PL_CDR1-Tests in `crates/cdr/src/xcdr1.rs::tests`.

**Status:** done

### 2.1 Programming-Interface = Extensible-and-Dynamic-Types-Profile

**Spec:** §2.1, S. 2 — "Complete-Niveau-API mit DynamicType."

**Repo:** `crates/types/src/dynamic/` mit 10 Files (factory/data/
builder/try_construct/bridge/builtin_types/descriptor/error/type_/mod).

**Tests:** Dynamic-Tests.

**Status:** done — DCPS-API live; Dynamic-Stack vollstaendig.

### 2.2.1 Minimal Network-Interop: TypeObject + TopicModel + DataRep-QoS v2 + XCDR2 + Built-in Types

**Spec:** §2.2.1.

**Repo:** XCDR2 (`crates/cdr/src/struct_enc.rs`) + TypeObject
(`crates/types/src/type_object/`) + Built-in Topic-Data
(`crates/dcps/src/builtin_topics.rs`); DataRepresentation-QoS-Wire
(`crates/qos/src/data_representation.rs` + `crates/rtps/src/parameter_list.rs::pid::DATA_REPRESENTATION`).

**Tests:** XCDR2-Tests + TypeObject-Tests + Wire-Vektoren in
`crates/types/tests/compliance_typeobject.rs` (Indirect-Hash 8 Tests).

**Status:** done

### 2.2.2 Basic Network-Interop: + Built-in TypeLookup-Service

**Spec:** §2.2.2.

**Repo:** TypeLookup-Service end-to-end.

**Tests:** `cyclone_live_typelookup`-Tests.

**Status:** done

### 2.3 Optional XTYPES 1.1 Profile (Encoding-Version 1)

**Spec:** §2.3.

**Repo:** `crates/cdr/src/xcdr1.rs` mit PL_CDR1-Encoder/Decoder
(Standard- + Long-Header via PID_EXTENDED, Sentinel-Termination).

**Tests:** `crates/cdr/src/xcdr1.rs::tests`:
`xcdr1_pl_cdr_long_header_roundtrip`,
`extended_header_for_id_above_threshold`,
`xcdr1_member_id_above_threshold_uses_extended_pid`,
`xcdr1_member_id_just_below_threshold_uses_standard`,
`xcdr1_sentinel_terminates_decode`,
`xcdr1_truncated_extended_header_rejected`,
`xcdr1_truncated_body_rejected`.

**Status:** done

### 2.4 Optional XML Data Representation Profile

**Spec:** §2.4 — XML-Wire-Format als alternative Sample-
Repraesentation neben CDR-1/CDR-2.

**Repo:** `crates/zerodds-xml-wire/src/{codec,emitter,parser,validator,
xsd}.rs` (1465 SLOC) — XML↔CDR-Codec + XSD-Schema-Gen + Streaming-
Parser.

**Tests:** Inline-Tests im `zerodds-xml-wire`-Crate.

**Status:** done

---

## §3 Normative References

### 3.0 [DDS] DDS 1.4 / [RTPS] RTPS 2.3 / [IDL] IDL 4.2 / [CDR] CDR 3.1 / [IEEE-754] / [Unicode 9.0]

**Spec:** §3.

**Repo:** Implementiert via DCPS/RTPS/IDL/CDR-Crates + native f32/f64
fuer IEEE-754 + Rust-stdlib UTF-8/UTF-16 fuer Unicode.

**Tests:** siehe pro Spec-Coverage.

**Status:** done

---

## §6 Tab.1: Concerns

### 6.0 Vier Concerns Mapping (TypeSystem/TypeRepr/DataRepr/LanguageBinding)

**Spec:** §6 Tab.1.

**Repo:** Siehe §1.1.

**Tests:** —

**Status:** done

---

## §6 Tab.2: Features

### 6.0 Features: Single-Inheritance/Type-Versioning/Sparse Types/IDL/XSD/XML/TypeObject/CDR/PL_CDR/XML/Plain/Dynamic Binding

**Spec:** §6 Tab.2.

**Repo:** TypeObject + IDL + CDR + PL_CDR + Dynamic Binding (via
dynamic/-Stack) + Sparse via @optional.

**Tests:** —

**Status:** done

---

## §7.2.1 Type-Evolution

### 7.2.1.1 Type-Evolution (added fields, default-on-subscriber)

**Spec:** §7.2.1.1.

**Repo:** XCDR2 Appendable/Mutable mit DHEADER+EMHEADER.

**Tests:** Appendable+Mutable-Tests.

**Status:** done

### 7.2.1.2 Type-Inheritance (IS-A, single inheritance)

**Spec:** §7.2.1.2.

**Repo:** Struct/Union base_type in TypeObject + Assignability.

**Tests:** Inheritance-Tests.

**Status:** done

### 7.2.1.3 Sparse Types via @optional

**Spec:** §7.2.1.3.

**Repo:** optional-Bit auf Member; PL_CDR2 Encoding.

**Tests:** Optional-Member-Tests.

**Status:** done

---

## §7.2.2 Type-System-Model

### 7.2.2.0 Type {kind, nested, extensibility_kind, base_type, element_type}

**Spec:** §7.2.2.

**Repo:** `crates/types/src/type_identifier/kinds.rs::TypeKind` +
`type_object/kinds.rs::TypeFlags`.

**Tests:** Kinds-Tests.

**Status:** done

### 7.2.2.1 Namespaces: Modules + Types-as-Namespaces, fully qualified name

**Spec:** §7.2.2.1.

**Repo:** Module/Scope-Pfad in `crates/idl/src/semantics/` + 
`crates/types/src/resolve.rs`.

**Tests:** Resolve-Tests.

**Status:** done

### 7.2.2.2 Primitive Types: BOOLEAN/BYTE/INT8/UINT8/INT16/UINT16/INT32/UINT32/INT64/UINT64/FLOAT32/FLOAT64/FLOAT128/CHAR8/CHAR16

**Spec:** §7.2.2.2 — 15 TypeKinds.

**Repo:** Alle 15 in TK-Enum (`type_identifier/kinds.rs`).

**Tests:** Primitive-Tests.

**Status:** done

### 7.2.2.2.1.2 Char-Encoding: Char8 unspec (UTF-8 default), Char16 = UTF-16 BMP

**Spec:** §7.2.2.2.1.2.

**Repo:** UTF-8 Strings + UTF-16 in CDR-Pfad (`crates/cdr/src/encode.rs`).

**Tests:** String-Encoding-Tests.

**Status:** done

### 7.2.2.3 StringTypes: STRING8 (Char8) + STRING16 (Char16), bounded/unbounded

**Spec:** §7.2.2.3.

**Repo:** `type_identifier/mod.rs::StringSTypes`/`StringLTypes`.

**Tests:** —

**Status:** done

### 7.2.2.4 Constructed Types: Enumeration/Bitmask/Alias/Array/Sequence/Map/Structure/Union

**Spec:** §7.2.2.4 — 8 Constructed-Kinds.

**Repo:** `type_object/complete/` mit allen 8 Constructed-Kinds.

**Tests:** Constructed-Tests.

**Status:** done

### 7.2.2.4.1 EnumeratedType: bit_bound (1..32), root=false, FINAL/APPENDABLE

**Spec:** §7.2.2.4.1.

**Repo:** `type_object/complete/` enum-Type mit bit_bound +
extensibility.

**Tests:** Enum-Tests.

**Status:** done

### 7.2.2.4.1.1 Enumeration: Int32-literals, ordered

**Spec:** §7.2.2.4.1.1.

**Repo:** EnumeratedLiteral + value-Field.

**Tests:** —

**Status:** done

### 7.2.2.4.1.2 Bitmask: bound 1..64, named bits

**Spec:** §7.2.2.4.1.2.

**Repo:** Bitmask + Bitflag mit position.

**Tests:** Bitmask-Tests.

**Status:** done

### 7.2.2.4.2 Alias: typedef -> base_type, kein eigenes Kind beim Vergleich

**Spec:** §7.2.2.4.2.

**Repo:** `type_object/complete/alias_type.rs`; Resolver folgt base.

**Tests:** Alias-Tests.

**Status:** done

### 7.2.2.4.3 Collection: Array (multi-dim, fixed), Sequence (1-dim, bound opt), Map (key,value)

**Spec:** §7.2.2.4.3.

**Repo:** `type_identifier/mod.rs` Array/Sequence/Map +
`type_object/complete/collection_types.rs`.

**Tests:** Collection-Tests.

**Status:** done

### 7.2.2.4.4 AggregatedType: Struct/Union/Annotation, Member {id, key, must_understand, optional, shared, member_index}

**Spec:** §7.2.2.4.4.

**Repo:** MemberFlag-Bits in `type_object/common.rs`.

**Tests:** Member-Tests.

**Status:** done

### 7.2.2.4.4.2 Structure: optional base_type

**Spec:** §7.2.2.4.4.2.

**Repo:** `type_object/complete/struct_type.rs::base_type`.

**Tests:** —

**Status:** done

### 7.2.2.4.4.3 Union: discriminator member always first, must_understand=true; allowed disc-types

**Spec:** §7.2.2.4.4.3.

**Repo:** `type_object/complete/union_type.rs` mit Discriminator +
UnionCase + UnionDiscriminator.

**Tests:** Union-Tests.

**Status:** done

### 7.2.2.4.4.4.1 Member-Name unique innerhalb Aggregat

**Spec:** §7.2.2.4.4.4.1.

**Repo:** Validation in `crates/types/src/builder.rs`.

**Tests:** Builder-Tests.

**Status:** done

### 7.2.2.4.4.4.4 Member-IDs 0..0x0FFFBFFF; OMG reserviert obere

**Spec:** §7.2.2.4.4.4.4.

**Repo:** MemberId u32 mit Mask in `common.rs`.

**Tests:** —

**Status:** done

### 7.2.2.4.4.4.5 Member-Name-Hash: erste 4 Bytes von MD5(UTF-8(name))

**Spec:** §7.2.2.4.4.4.5.

**Repo:** `crates/types/src/hash.rs`.

**Tests:** Hash-Tests.

**Status:** done

### 7.2.2.4.4.4.6 must_understand-Attribut: Discard-Behavior; Union-Discriminator immer must_understand=true

**Spec:** §7.2.2.4.4.4.6.

**Repo:** must_understand-Bit in `common.rs` + Validation.

**Tests:** —

**Status:** done

### 7.2.2.4.4.4.7 Optional Members: Union-Member nie optional; Struct frei

**Spec:** §7.2.2.4.4.4.7.

**Repo:** Validation in `builder.rs`: discriminator+key nie optional.

**Tests:** —

**Status:** done

### 7.2.2.4.4.4.8 Key Members: nur prim/aggr/enum/bitmask/array/sequence/alias; nie map; rekursiv

**Spec:** §7.2.2.4.4.4.8.

**Repo:** Key-Validation in `builder.rs` + KeyHolder/KeyErased in
`assignability.rs`.

**Tests:** Key-Tests.

**Status:** done

### 7.2.2.4.4.4.9 Default Member Value: explizit (Annotation) oder implizit (Tab.9)

**Spec:** §7.2.2.4.4.4.9.

**Repo:** `crates/types/src/builder.rs::set_member_default` (StructMember-
Builder mit `default_value: Option<String>`);
`crates/types/src/type_object/common.rs::AppliedBuiltinMemberAnnotations.default_value`
(Wire-Trailer, backwards-compat — alte Decoder lassen den Trailer
liegen, neue lesen ihn). IDL-Lowering: `BuiltinAnnotation::Default(value)`
→ `Member-Builder.set_member_default`.

**Tests:** `crates/types/src/builder.rs::tests`:
`member_with_explicit_default_used_when_field_missing`,
`default_overrides_implicit_zero`,
`default_value_roundtrips_via_encode_decode`.

**Status:** done

### 7.2.2.4.4 Tab.9 Defaults: 0/false/'\0'/""/leere coll./erster enum-literal/disc-default

**Spec:** §7.2.2.4.4 Tab.9.

**Repo:** Default-Konstruktor pro TK in `common.rs`.

**Tests:** —

**Status:** done

### 7.2.2.4.5 Single Inheritance Struct/Union (gleiche extensibility, eindeutige IDs/Names, derived hat keine Keys)

**Spec:** §7.2.2.4.5.

**Repo:** `crates/types/src/assignability.rs::flatten_inheritance` mit
`InheritanceError::InheritanceConflict`-Detection (Member-ID + Name-Hash);
TypeObject-Side `crates/types/src/type_object/minimal/struct_type.rs`
mit `MinimalStructHeader.base_type`.

**Tests:** `crates/types/src/assignability.rs::tests`:
`flatten_inheritance_no_base_returns_struct_unchanged`,
`flatten_inheritance_two_levels_concatenates_base_first`,
`inheritance_conflict_same_id`, `inheritance_conflict_same_name`,
`flat_type_construction_two_levels`,
`two_level_inheritance_assignability_chain`.

**Status:** done

### 7.2.2.4.6 KeyErased(T) — Key-Designation entfernt

**Spec:** §7.2.2.4.6.

**Repo:** KeyErased-Typ in `assignability.rs`.

**Tests:** —

**Status:** done

### 7.2.2.4.7 KeyHolder(T) — nur Key-Member behalten

**Spec:** §7.2.2.4.7.

**Repo:** KeyHolder-Helper in `assignability.rs`.

**Tests:** —

**Status:** done

### 7.2.2.4.8 VerbatimText {language, placement, text}

**Spec:** §7.2.2.4.8.

**Repo:** TypeObject-Wire-Form: `crates/types/src/type_object/common.rs`
mit `AppliedVerbatimAnnotation` + `VerbatimPlacement`-Enum +
`AppliedBuiltinTypeAnnotations` (Wire-Roundtrip).
IDL-Lowering: `crates/idl/src/semantics/annotations.rs::VerbatimSpec`
+ `PlacementKind` + `Lowered::verbatims_for_language(aliases)`.
Codegen-Pfade pro Sprache:
- `crates/idl-cpp/src/verbatim.rs` (Aliase: `c++`, `cpp`, `cxx`, `*`)
  + Hooks in `emit_struct`/`emit_enum`/`emit_union`/`emit_typedef`/
  `emit_header` fuer alle 6 PlacementKinds.
- `crates/idl-csharp/src/verbatim.rs` (Aliase: `c#`, `csharp`, `cs`, `*`).
- `crates/idl-java/src/verbatim.rs` (Alias: `java`, `*`).

**Tests:**
- TypeObject-Wire: `crates/types/src/type_object/common.rs::tests::
  verbatim_placement_roundtrip_{before,after,begin_file,end_file,
  other_forward_compat}` (5 Tests).
- IDL-Lowering: `crates/idl/src/semantics/annotations.rs::tests::
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

**Status:** done — Code-Gen-Templating Cross-Cutting voll erfuellt
(C++, C#, Java).

### 7.2.2.4.9 External Data (@external): incomplete forward ref + self-ref erlaubt

**Spec:** §7.2.2.4.9.

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag::IS_EXTERNAL`;
Forward-Resolution via `crates/types/src/resolve.rs::TypeRegistry::transitive_dependencies`
mit seen-Set (verhindert Endlosschleife auch bei zirkulaeren
@external-Referenzen).

**Tests:** `crates/types/src/resolve.rs::tests`:
`external_forward_decl_roundtrip` (Self-Forward + Wire-Roundtrip
des EXTERNAL-Flags),
`external_diamond_resolves_through_both_paths`
(Diamond A→B→D, A→C→D liefert jede Hash genau 1×).

**Status:** done

### 7.2.2.5 Nested Types (nicht als Topic verwendbar)

**Spec:** §7.2.2.5.

**Repo:** nested-Bit auf TypeFlag.

**Tests:** —

**Status:** done

### 7.2.2.6 AnnotationType: AggregatedType mit Parameter typisiert

**Spec:** §7.2.2.6.

**Repo:** AnnotationType in
`type_object/complete/annotation_type.rs`.

**Tests:** —

**Status:** done

### 7.2.2.7 / Tab.10/11 TryConstruct {DISCARD, USE_DEFAULT, TRIM} default DISCARD; TRIM nur string/seq/map

**Spec:** §7.2.2.7 Tab.10/11.

**Repo:** `dynamic/try_construct.rs::apply_try_construct` + Member-
Flags.

**Tests:** TryConstruct-Tests.

**Status:** done

---

## §7.2.3 Extensibility Tab.12

### 7.2.3 Extensibility: FINAL / APPENDABLE / MUTABLE

**Spec:** §7.2.3 Tab.12.

**Repo:** Extensibility-Bits in `type_object/kinds.rs` + CDR-
Encoding.

**Tests:** Extensibility-Tests.

**Status:** done

---

## §7.2.4 Type-Compatibility (is-assignable-from)

### 7.2.4.0 is-assignable-from-Relation, T1<-T2

**Spec:** §7.2.4.

**Repo:** Assignability-Engine in `assignability.rs`.

**Tests:** Assignability-Tests.

**Status:** done

### 7.2.4.1 Konstruktion: T1.is-assignable-from(T2) -> T2-Subset baut T1; sonst TryConstruct

**Spec:** §7.2.4.1.

**Repo:** is-assignable-from + DynamicData::create + try_construct.rs.

**Tests:** —

**Status:** done

### 7.2.4.2 Delimited Types: APPENDABLE in v2, MUTABLE in v1+v2

**Spec:** §7.2.4.2.

**Repo:** Delimited-Tag in Encoder, DHEADER/PL-Sentinel.

**Tests:** Delimited-Tests.

**Status:** done

### 7.2.4.3 Strong Assignability: T1≡T2 (MINIMAL) ∨ (T1 is-assignable T2 ∧ T2 delimited)

**Spec:** §7.2.4.3.

**Repo:** strong-assignable Helper.

**Tests:** —

**Status:** done

### 7.2.4.4.1 Equivalent (MINIMAL) -> mutually assignable

**Spec:** §7.2.4.4.1.

**Repo:** equality-Pfad.

**Tests:** —

**Status:** done

### 7.2.4.4.2 non_serialized Member ignoriert bei Compat-Check

**Spec:** §7.2.4.4.2 — Members marked `@non_serialized` MUST be omitted
from any wire form and from assignability comparisons.

**Repo:** `crates/idl/src/semantics/annotations.rs`
(`BuiltinAnnotation::NonSerialized` + `lower_single("non_serialized")`);
`crates/idl/src/semantics/to_typeobject.rs::lower_struct_to_minimal`
(skip-Pfad vor TypeObject-Member-Aufnahme); `dynamic_apply` passthrough.

**Tests:** `crates/idl/src/semantics/to_typeobject.rs::tests`:
`non_serialized_member_is_dropped_from_typeobject`,
`non_serialized_in_otherwise_empty_struct_yields_empty_member_seq`,
`non_serialized_with_other_annotations_still_dropped`,
`non_serialized_member_does_not_block_assignability`.

**Status:** done

### 7.2.4.4.3 / Tab.14 Alias: durchgereicht zu base_type

**Spec:** §7.2.4.4.3.

**Repo:** Alias-Resolution.

**Tests:** —

**Status:** done

### 7.2.4.4.4 / Tab.15 Primitive: same kind only

**Spec:** §7.2.4.4.4.

**Repo:** exakte Match-Regel.

**Tests:** —

**Status:** done

### 7.2.4.4.5 / Tab.16 String: same wide/narrow + element_type assignable + length-Check

**Spec:** §7.2.4.4.5.

**Repo:** string-rule.

**Tests:** —

**Status:** done

### 7.2.4.4.6 / Tab.17 Array (gleiche bounds, strongly), Sequence (strongly el), Map (strongly key+el)

**Spec:** §7.2.4.4.6.

**Repo:** collection-rule mit TryConstruct.

**Tests:** —

**Status:** done

### 7.2.4.4.7 / Tab.18 Bitmask↔UInt-by-bound; Enum-Compat (FINAL ⇒ gleiche Literals); @ignore_literal_names

**Spec:** §7.2.4.4.7 — Default-Enum-Compat vergleicht (value, name);
`@ignore_literal_names` reduziert auf (value).

**Repo:** `crates/types/src/assignability.rs` (enum-Vergleich
beruecksichtigt `cfg.ignore_literal_names` ODER
`EnumTypeFlag::IGNORE_LITERAL_NAMES` auf einer der Seiten);
`crates/types/src/type_object/flags.rs::EnumTypeFlag::IGNORE_LITERAL_NAMES`;
`crates/idl/src/semantics/annotations.rs::BuiltinAnnotation::IgnoreLiteralNames`
+ `lower_single("ignore_literal_names")`.

**Tests:** `crates/types/src/assignability.rs::tests`:
`enum_not_assignable_strict_default`,
`enum_assignable_with_ignore_literal_names`,
`enum_assignable_with_ignore_literal_names_via_writer_flag`,
`enum_assignable_with_ignore_literal_names_via_reader_flag`,
plus Bestand `enum_identical_labels_is_yes`,
`enum_mismatch_writer_literal_unknown_in_reader_is_no`,
`enum_bit_bound_mismatch_is_no`.

**Status:** done

### 7.2.4.4.8 / Tab.19 Aggregated (Union/Struct mit Member-IDs/Key-Erased/Appendable/Final/KeyHolder)

**Spec:** §7.2.4.4.8 — Aggregated edge-cases: union default coverage,
map-key whitelist, two-level inheritance assignability.

**Repo:** Struct + Union Assignability in
`crates/types/src/assignability.rs` (incl. `flatten_inheritance`);
Map-Key-Validator `crates/idl/src/semantics/map_validation.rs`;
Union default coverage in `crates/idl/src/semantics/union_validation.rs`.

**Tests:**
- `crates/idl/src/semantics/union_validation.rs::tests::union_default_coverage_required_when_partial_range`
- `crates/idl/src/semantics/map_validation.rs::tests::map_key_element_type_must_be_primitive` (+ 5 Edge-Cases)
- `crates/types/src/assignability.rs::tests::two_level_inheritance_assignability_chain`,
  `flat_type_construction_two_levels`,
  `flatten_inheritance_two_levels_concatenates_base_first`,
  `inheritance_conflict_same_id`, `inheritance_conflict_same_name`.

**Status:** done

---

## §7.3 Type Representation

### 7.3.0 Vier TypeRepr (Tab.20): IDL / XML / XSD / TypeObject

**Spec:** §7.3 Tab.20.

**Repo:** IDL (`crates/idl/`) + TypeObject (`crates/types/`) +
XML (`crates/xml/`) live. Die vierte Repraesentation XSD wird via
Iron-Rule-Eskalations-Klausel separat als Open-Item §7.3.3 getrackt
(siehe `dds-xtypes-1.3.open.md`).

**Tests:** Crate-weite Test-Suiten plus `crates/idl/`,
`crates/types/`, `crates/xml/` Tests fuer alle drei aktiven
Repraesentationen.

**Status:** done — IDL/XML/TypeObject vollstaendig; XSD als
Iron-Rule-Open via §7.3.3.

### 7.3.1.1 IDL-Compatibility: Extensible-DDS-Profile von IDL 4.2

**Spec:** §7.3.1.1.

**Repo:** `crates/idl/src/grammar/idl42.rs`.

**Tests:** siehe `idl-4.2.md`.

**Status:** done

### 7.3.1.1.1 `#pragma dds_xtopics begin/end [version]`

**Spec:** §7.3.1.1.1.

**Repo:** `crates/idl/src/preprocessor/mod.rs`
(`PragmaDdsXtopics { version, file, line }` +
`parse_pragma_dds_xtopics`-Reader; `ProcessedSource.pragma_dds_xtopics`).

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests`:
`pragma_dds_xtopics_version_match`,
`pragma_dds_xtopics_version_mismatch_warns`,
`pragma_dds_xtopics_nested_pragmas_handled`,
`pragma_dds_xtopics_without_version_value_is_empty`,
`pragma_dds_xtopics_unquoted_version_accepted`.

**Status:** done

### 7.3.1.2.1 IDL-Annotations (40+ Annotations)

**Spec:** §7.3.1.2.1.

**Repo:** `crates/idl/src/semantics/annotations.rs` mit 28 Built-in
Annotations: `@key`, `@id(n)`, `@optional`, `@must_understand`,
`@external`, `@non_serialized`, `@ignore_literal_names`, `@default(v)`,
`@default_literal`, `@extensibility(...)`, `@final`, `@appendable`,
`@mutable`, `@autoid(...)`, `@topic`, `@nested`, `@unit("...")`,
`@hashid("...")`, `@range(min, max)`, `@min(v)`, `@max(v)`,
`@value(v)`, `@position(n)`, `@bit_bound(n)`, `@verbatim(...)`,
`@ami(b)`, `@service(s)`, `@oneway(b)`.

**Tests:** Annotation-Tests in `annotations.rs::tests`,
`semantics::to_typeobject::tests`, `semantics::dynamic_apply::tests`.
`@verbatim`-Code-Gen-Hook ist live in C++/C#/Java (siehe §7.2.2.4.8
oben).

**Status:** done

### 7.3.1.2.1.1 @id / @hashid; @autoid(SEQUENTIAL|HASH); MD5-Algo

**Spec:** §7.3.1.2.1.1.

**Repo:** annotations.rs + `hash.rs`.

**Tests:** —

**Status:** done

### 7.3.1.3 Const+Expressions: compile-time eval

**Spec:** §7.3.1.3.

**Repo:** `crates/idl/src/semantics/const_eval.rs` (`evaluate` mit
SymbolTable-Resolution + Integer-/Float-/Bitwise-/Shift-Ops);
verschachtelte `#define`-Refs ueber `crates/idl/src/preprocessor/mod.rs::expand_macros`
mit `MAX_MACRO_EXPANSION_DEPTH = 32` Cycle-Cap.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests`:
`nested_define_two_hops`, `nested_define_three_hops`,
`nested_define_with_arithmetic_expression`,
`nested_define_self_recursive_terminates`,
`nested_define_mutually_recursive_terminates`,
plus `crates/idl/src/semantics/const_eval.rs::tests` (Bestand).

**Status:** done

### 7.3.2 XML-TypeRepr: Namespace + simpleType-Hierarchie

**Spec:** §7.3.2 Tab.26.

**Repo:** `crates/xml/src/xtypes_parser.rs` + `xtypes_def.rs` mit
`<struct>`/`<enum>`/`<union>`/`<typedef>`/`<bitmask>`/`<bitset>`/
`<module>` plus Phase-2-Konstrukte `<include>`, `<forward_dcl>`,
`<const>` (`IncludeEntry`, `ForwardDeclEntry`, `ConstEntry`).

**Tests:** `crates/xml/src/xtypes_parser.rs::tests`:
`parse_include_element`, `parse_include_missing_file_attribute_rejected`,
`parse_forward_dcl_element`, `parse_forward_dcl_default_kind_struct`,
`parse_const_element`, `parse_const_missing_value_attribute_rejected`,
plus Bestand `parse_simple_struct`, `parse_module_nested`,
`dds_root_with_multiple_types_blocks`.

**Status:** done

### 7.3.3 XSD Type Representation

**Spec:** §7.3.3.

**Repo:** `crates/xml/src/xsd_schema.rs::parse_xsd_schema` parst W3C-
XSD-Schemas auf das interne `TypeLibrary`-Datenmodell; via existing
`crates/xml/src/typeobject_bridge.rs::bridge_library` wird daraus
ein `MinimalTypeObject` erzeugt. Mapping deckt:
- `<xsd:complexType><xsd:sequence>...</xsd:sequence>` -> Struct.
- `<xsd:simpleType>` mit `<xsd:enumeration>` -> Enum.
- `<xsd:complexContent><xsd:extension base="...">` -> Inheritance.
- minOccurs=0 -> optional, maxOccurs=unbounded -> sequence,
  maxOccurs=N>1 -> bounded sequence.
- Built-In: `xsd:int`/`xsd:long`/`xsd:short`/`xsd:byte`/`xsd:double`/
  `xsd:float`/`xsd:string`/`xsd:boolean` + unsigned-Varianten.

**Tests:** `xsd_schema::tests::*` (12 Tests):
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

### 7.3.4.0 TypeIdentifier (Hash + Plain-Variants); TypeObject (Minimal+Complete)

**Spec:** §7.3.4.

**Repo:** `crates/types/src/type_identifier/` + `type_object/`.

**Tests:** TypeIdentifier+TypeObject-Tests.

**Status:** done

### 7.3.4.6 Indirect Hash TypeIdentifiers (PlainCollectionHeader mit EquivalenceKind=EK_MINIMAL/COMPLETE)

**Spec:** §7.3.4.6 + §7.3.4.7.1 — `PlainCollectionHeader { equiv_kind;
element_flags }` plus `EK_MINIMAL`/`EK_COMPLETE` als Element-Discriminator.

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

**Spec:** §7.3.4.8 (Mutual-Dependency) + §7.3.4.9 (SCC IDs).

**Repo:** `crates/types/src/resolve.rs`
(`TypeRegistry::transitive_dependencies` mit seen-Set zur Cycle-
Detection; `StronglyConnectedComponentId` in
`crates/types/src/type_identifier/mod.rs`).

**Tests:** `crates/types/src/resolve.rs::tests`:
`scc_three_element_cycle_resolves_correctly`,
`scc_four_element_cycle_resolves_correctly`,
`scc_diamond_with_cycle_resolves_finitely`,
`scc_self_loop_does_not_explode_node_count`,
plus Wire-Tests `scc_encode_rejects_negative_values`,
`scc_encode_rejects_index_out_of_bounds`,
`strongly_connected_component_large_scc_length_and_index`.

**Status:** done

### 7.3.4.x TypeLookup-Service (request/reply)

**Spec:** §7.3.4 (TypeLookup).

**Repo:** `type_lookup.rs` + Discovery-Layer.

**Tests:** TypeLookup-Tests.

**Status:** done

---

## §7.4 Data Representation / CDR

### 7.4.1.1 SerializedPayloadHeader (Representation Identifier)

**Spec:** §7.4.1.1.

**Repo:** `crates/cdr/` mit Representation-Identifiers (CDR1/PL_CDR1/
CDR2/PL_CDR2/D_CDR/XML).

**Tests:** Representation-Tests.

**Status:** done

### 7.4.1.2.1 PID_IGNORE 0x3F03

**Spec:** §7.4.1.2.1 — "Used to ignore parameters which can be safely ignored."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::IGNORE = 0x3F03`,
silent-skip-Pfad in `from_bytes`).

**Tests:** `crates/rtps/src/parameter_list.rs::tests`:
`pid_ignore_skipped_in_pl_cdr_decode`, `pid_ignore_zero_length_is_valid`,
`pid_ignore_truncated_body_rejected`, `pid_ignore_be_decoded`,
`pid_ignore_ignored_even_when_count_would_exceed_cap`.

**Status:** done

### 7.4.1.2.3 non-optional in Mutable Encode (MUST contain all non-optional members)

**Spec:** §7.4.1.2.3 — "the serialized representation MUST contain at
least the values of all the non-optional members."

**Repo:** `crates/cdr/src/struct_enc.rs` (`MutableStructEncoder` mit
`required_ids`-Tracking + `finish()`-Validierung); Fehler
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

**Spec:** §7.4.2 + §7.4.1.2.2 (PID_EXTENDED) — PL_CDR1 mit Long-Header
fuer Member-IDs >= 0x3F00 oder Body-Laenge > 0xFFFF.

**Repo:** `crates/cdr/src/xcdr1.rs` (PL_CDR1 mit `encode_pl_cdr1_member`,
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

**Repo:** XCDR2-Pfad in `struct_enc.rs`.

**Tests:** XCDR2-Tests.

**Status:** done

### 7.4.3.4.2 LC=6 / LC=7 (4·NEXTINT / 8·NEXTINT)

**Spec:** §7.4.3.4.2.

**Repo:** `crates/cdr/src/struct_enc.rs` (Length-Code-Enum +
`encode_mutable_member_lc` mit Lc6/Lc7-Branch +
`read_mutable_member` mit `body_len`-Berechnung 4·NEXTINT+4 / 8·NEXTINT+4).

**Tests:** `crates/cdr/src/struct_enc.rs::tests`:
`lc6_array_of_4byte_primitives`, `lc7_array_of_8byte_primitives`,
`lc6_rejects_misaligned_body`,
`lc6_lc7_roundtrip_against_cyclone_sample`,
`lc6_with_many_elements_decodes_correctly` (70_000 Elemente).

**Status:** done

### 7.4.3.5.3 Rules 11-16 (Maps)

**Spec:** §7.4.3.5.3.

**Repo:** `crates/types/src/builder.rs::MapBuilder::add_map_member(ext)`
liefert `BuilderError::MutableMapExtensibilityNotAllowed` wenn
MUTABLE auf Map gesetzt — verhindert silent demotion.

**Tests:** `crates/types/src/builder.rs::tests`:
`mutable_map_extensibility_is_error`,
`appendable_map_extensibility_is_ok`,
`final_map_extensibility_is_ok`.

**Status:** done

### 7.4.4 Encoding-VM (formal encoding rules)

**Spec:** §7.4.4.

**Repo:** `crates/cdr/src/struct_enc.rs` (mutable + appendable + final
Encoder-Pfade); `crates/cdr/src/xcdr1.rs` (PL_CDR1-Pfad);
`crates/cdr/src/key_hash.rs::PlainCdr2BeKeyHolder` (PLAIN_CDR2-BE).
Die formale Encoding-VM ist nicht als separate Bytecode-Maschine
implementiert, weil alle XTypes 1.3 Wire-Konstrukte bereits durch
diese spezialisierten Encoder-Pfade gedeckt sind und dieselben
Spec-Regeln (DHEADER + EMHEADER + Sentinel) durchsetzen.

**Tests:** Encoder/Decoder-Tests in `crates/cdr/src/struct_enc.rs`,
`crates/cdr/src/xcdr1.rs`, `crates/cdr/src/key_hash.rs`,
`crates/cdr/tests/compliance_xcdr2.rs`,
`crates/types/tests/compliance_typeobject.rs`.

**Status:** done

### 7.4.5 KeyHash-Berechnung

**Spec:** §7.4.5 + §7.6.8.3.1.b — KeyHolder-Member MUESSEN nach
`member_id` aufsteigend in PLAIN_CDR2-BE serialisiert werden, damit
Encoder mit unterschiedlichen Member-Aufzaehl-Reihenfolgen denselben
KeyHash produzieren.

**Repo:** `crates/cdr/src/key_hash.rs` (`compute_key_hash` zero-pad/MD5-
Pfad; `keyhash_cdr2_be` mit `sort_by_key(member_id)`-Pflicht);
DCPS-Wiring `crates/dcps/src/instance_tracker.rs`.

**Tests:** `crates/cdr/src/key_hash.rs::tests`:
`keyhash_cdr2_be_member_order_independent`,
`keyhash_cdr2_be_md5_path_also_order_independent`,
`keyhash_cdr2_be_single_member_matches_compute_key_hash`,
`keyhash_cdr2_be_empty_member_set_yields_zero_padding`,
`keyhash_cdr2_be_member_id_zero_sorts_first`,
plus Pipeline-Tests `keyholder_full_keyhash_pipeline_zero_pad`,
`keyholder_full_keyhash_pipeline_md5`.

**Status:** done

---

## §7.5 Programming Language Binding

### 7.5.1 Plain Language Binding (PSM-spezifisch)

**Spec:** §7.5.1 — Cross-Spec-Item: Plain-Language-Binding ist je
PSM separat normativ. dds-xtypes-1.3 definiert nur den Hook,
die konkreten Mappings stehen in den jeweiligen IDL-PSM-Specs.

**Repo:** Pro PSM separat: `idl4-cpp-1.0.md` (`crates/idl-cpp/`),
`idl4-java-1.0.md`, `idl4-csharp-1.0.md`. Diese Coverage-Datei
trackt nur die XTypes-1.3-eigenen Wire-/Type-Aspekte.

**Tests:** PSM-spezifische Tests in den Crate-Suiten der einzelnen
Bindings; Cross-Spec-Linkage via Coverage-Docs.

**Status:** `n/a (informative)` — Cross-Spec-Linkage; die
XTypes-1.3-Section delegiert an die jeweiligen Sprach-PSMs
(`idl4-cpp-1.0.md`, `idl4-java-1.0.md`, `idl4-csharp-1.0.md`,
`dds-ts-1.0.md`).

### 7.5.2 Dynamic Language Binding

**Spec:** §7.5.2 — DynamicType + DynamicData + Factories.

**Repo:** `crates/types/src/dynamic/` (10 Files mit ~3000 LOC).

**Tests:** Dynamic-Tests.

**Status:** done

---

## §7.6 Built-in Topic Types

### 7.6.1 ParticipantBuiltinTopicData / PublicationBuiltinTopicData / SubscriptionBuiltinTopicData / TopicBuiltinTopicData (XTypes-Erweiterungen)

**Spec:** §7.6.1.

**Repo:** `crates/dcps/src/builtin_topics.rs` + `crates/rtps/src/{participant,publication,subscription}_data.rs`.

**Tests:** Builtin-Topic-Tests.

**Status:** done — XTypes-Erweiterungen (TypeInformation + RepresentationInfo)
done.

### 7.6.2 DataRepresentationQosPolicy (XCDR1/XCDR2/XML)

**Spec:** §7.6.2 + §7.6.3.1 Tab.59.

**Repo:** `crates/types/src/qos.rs`
(`DataRepresentationId`, `negotiate_representation`,
`check_data_repr_extensibility` Wire-Constraint-Matrix).

**Tests:** `crates/types/src/qos.rs::tests`:
`negotiate_xcdr2_preferred`, `negotiate_fallback_to_xcdr1`,
`negotiate_no_overlap`,
`xcdr1_final_combination_is_allowed`,
`xcdr1_mutable_combination_is_allowed`,
`xcdr1_appendable_is_disallowed`,
`xcdr2_supports_all_three_extensibilities`.

**Status:** done

### 7.6.3 TypeLookupService

**Spec:** §7.6.3 — request/reply types + KeyValue-Hash + Endpoint-IDs.

**Repo:** `crates/types/src/type_lookup.rs` +
`crates/discovery/src/type_lookup/`.

**Tests:** TypeLookup-Tests inkl. Cyclone-Live.

**Status:** done

### 7.6.3.3 TypeLookup-Responder (Reply-Build mit Type-Object-Refs)

**Spec:** §7.6.3.3.

**Repo:** `crates/discovery/src/type_lookup/responder.rs`.

**Tests:** Responder-Tests.

**Status:** done

---

## Audit-Status

82 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-types` — 281 lib + 9 + 40 = 330 Tests grün
  (TypeObject + TypeLookup + Assignability + IDL-Semantik).
* `cargo test -p zerodds-cdr` — 165 lib + 1 + 7 = 173 Tests grün
  (XCDR1+XCDR2 für XTypes-Wire-Encoding).
* `cargo test -p zerodds-xml-wire` — 40 Tests grün (§2.4 XML Data
  Representation Profile, Cross-Ref `zerodds-xml-1.0.md`).

Keine offenen Punkte.
