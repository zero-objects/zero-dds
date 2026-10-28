# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `TK_NONE` to `TK_MAP` and `TypeKindValue` (Annex C), the typed sequences
  `Int32Seq` to `WstringSeq` of the `DynamicData` operations, the operations
  of Annex C under their IDL names on `DynamicType` (`get_descriptor`,
  `get_name`, `get_kind`, `get_all_members`, `get_member_count`,
  `get_annotation*`, `get_verbatim_text*`), `DynamicTypeMember`
  (`get_id`, `get_name`, `get_descriptor`, `get_annotation*`,
  `get_verbatim_text*`) and `TypeBuilder` (`get_all_members_by_name`,
  `get_all_members`).

- `AnnotationType::members`, `member_count`, `get_member_by_index`,
  `get_member_by_name`, `get_member`, `get_all_members_by_name`,
  `get_all_members` and `AnnotationMember` (Table 51, `ANNOTATION_TYPE`).

- `type_object_v1`: the `TypeObject` and the `_TypeId` of DDS-XTypes 1.1 Annex B (`TypeObjectV1`, `TypeIdV1`
  of the built-in topic data of XTypes 1.3 Annex D): the model, its XCDR1 serialization (parameter lists for
  the mutable structures, plain CDR for the extensible ones), the type ids of §7.3.4.1.2 (the low 64 bits of
  the MD5 of the big-endian serialization), and `convert::from_complete` / `convert::to_complete` between a
  library and Complete TypeObjects. A type that refers to itself, `int8` and a bit set with bit fields have no
  form in Annex B and are refused by name; `try_construct`, `@must_understand`, `@verbatim` and the other
  additions of 1.3 are not carried.
- The `@verbatim` of a type is written into the `AppliedBuiltinTypeAnnotations` of the `CompleteTypeDetail`
  (DDS-XTypes 1.3 Annex B, §7.3.4.3) and read back onto the `XTypesFacet`; a type with more than one `@verbatim`
  is refused by name. `VerbatimPlacement` has the six placements of §7.2.2.4.8.2 (`BeginDeclaration`,
  `EndDeclaration` are new) and is written with those strings; the `string<32>` bounds of `placement` and
  `language` are checked.
- `CompleteAnnotationParameterSeq` is ordered by `member_index`, the declaration order (§7.3.4.5); the reading
  of Annex B (by name) is `AnnotationParameterOrder::Name`, set with `set_annotation_parameter_order`
  (setting `xtypes.annotation_parameter_order`). The `annotation_typeid` of an annotation type whose
  declaration order is not the order of its names changes.
- `union_inheritance` (DDS-XTypes 1.3 §7.2.2.4.5): `check_union_derivation`
  checks the five conditions for a union that inherits from another one
  (extensibility, discriminator type, discriminator not a key, no default member
  in an ancestor, one member per discriminator value in the expanded union) and
  `expand_union` builds the union of Table 19's "expanded" members. No Type
  Representation carries a base union, so the functions take the objects of the
  ancestors and of the derived union.
- `assignability::key_erased` is public (§7.2.2.4.6).
- Enumerator values outside `i32` are refused by the IDL front end (Table 4) as
  they were by `from_ast` (`FromAstError::OutOfRange`).

- `DynamicTypeBuilderFactory::create_annotation_type` and
  `AnnotationTypeBuilder` (DDS-XTypes 1.3 §7.5.2.2.6, §7.5.2.3.4): an
  annotation type is built from a name and its attributes
  (`add_member(name, type, default)`, and `add_member_descriptor` for the
  descriptor of Table 55), and `build` gives the
  `AnnotationType` an `AnnotationDescriptor` applies. An `@annotation`
  declaration is not a `TypeNode` of the model, so `create_type` with the
  kind `ANNOTATION_TYPE` still refuses and names this operation.

- `DynamicType::equals` and `DynamicTypeMember::equals` compare the
  applied annotations and `@verbatim` texts (§7.5.2.8.4, Tables 52 and
  54), in any order; the built-in annotations that restate a property
  already compared (`@key`, `@id`, `@final`, …) count once.
  `DynamicType::equals_ignoring_annotations` is the comparison without
  them, for a check that two types are compatible.

- `DynamicSample::share`, `is_same` and `snapshot`.

- `TypeDescriptor::is_consistent` checks the name (an identifier, or
  identifiers separated by `::`, for a named kind) and
  `MemberDescriptor::is_consistent_in` the member name (§7.5.2.4.10,
  §7.5.2.7.9); `TypeBuilder::get_member`, `get_member_by_name`,
  `get_member_by_index`, `all_members_by_name` and `all_members_by_id`.

- `DynamicSample` and `DynamicDataFactory` (DDS-XTypes 1.3 §7.5.2.10,
  §7.5.2.11, Tables 57 and 58, Annex C): a data sample with its type.
  `get_member_id_by_name`, `get_member_id_at_index`, `get_item_count`,
  `get_descriptor`, `equals`, `clone`, `clear_value`, `clear_all_values`,
  `clear_nonkey_values`, `loan_value` and `return_loaned_value`; the
  getters and setters of Annex C for every primitive, `string`, `wstring`,
  `complex` and the plural forms for sequences and arrays, with the
  promotions of §7.5.2.11.9 (a smaller type reads and sets as a larger one,
  an enumeration as a signed integer, a bitmask as an unsigned one, an alias
  as its base type); the flags of a bitmask as Boolean values by bit, a map
  by key, a union member selects itself by being set, a multi-dimensional
  array is flattened.

- `DynamicTypeBuilderFactory` and `TypeBuilder` (DDS-XTypes 1.3 §7.5.2.2,
  §7.5.2.9, Tables 48 and 56): `get_instance`, `delete_instance`,
  `get_primitive_type`, `create_type`, `create_type_copy`,
  `create_type_w_type_object`, `create_string_type`,
  `create_wstring_type`, `create_sequence_type`, `create_array_type`,
  `create_map_type`, `create_bitmask_type` and `delete_type`; and on the
  builder `add_member` (with the constraints of Table 55),
  `apply_annotation`, `apply_annotation_to_member`, `build` and the
  queries of the type as it stands. A type named by a `DynamicType`
  argument is copied into the builder's model with everything it reaches
  (`DynamicTypeBuilder::import`, recursive types included).
  `create_type_w_uri` and `create_type_w_document` are in
  `zerodds-xml-compose`.
- `MemberDescriptor::is_consistent_for_add`, and the `default_value` of
  the descriptor of an enumerator or a flag carries its value or position.
- `ModelFromTypeObjects::into_model`.

- `AnnotationDescriptor`, `AnnotationType` and `VerbatimTextDescriptor`
  (DDS-XTypes 1.3 §7.5.2.3, Annex C): the annotations and `@verbatim` texts
  of a type or a member as descriptors (`DynamicType::annotation`,
  `annotation_count`, `verbatim_text`, `verbatim_text_count` and the same on
  `DynamicTypeMember`), with `get_value`, `get_all_value`, `set_value`,
  `copy_from`, `equals` and `is_consistent`. The annotations the model keeps
  in typed fields (`@key`, `@id`, `@mutable`, …) come back as built-in
  annotation types with string values.
- `TypeDescriptor::copy_from` and `equals`, `MemberDescriptor::copy_from`,
  `equals` and `is_consistent_in`, and `MemberDescriptor::default_value`,
  the `@default` as the string §7.5.2.7.3 describes.
- `DynamicType::get_member`, `get_member_by_name`, `get_member_by_index`
  (`DynamicError::BadParameter` for no match), `all_members_by_name` and
  `all_members_by_id` (Annex C), and the `DynamicError` variants
  `BadParameter`, `NoData`, `PreconditionNotMet`, `AlreadyDeleted` with
  `DynamicError::return_code`.

- `dynamic::TypeKind::value` and `TypeKind::from_value`: the numbers of
  Annex C's `TypeKind` constants (`TK_NONE = 0x00` to `TK_MAP = 0x62`).

- `TypeInformation::from_component` (DDS-XTypes 1.3 §7.6.3.2.1): the
  TypeInformation of a topic type that belongs to a strongly connected
  component. The type's own `TI_STRONGLY_CONNECTED_COMPONENT` identifier
  with the size of its object on both sides, the `SCCIdentifier` of the
  component first among the dependents, then the hash identifiers of the
  other types and the `SCCIdentifier` of every other component the type
  reaches. `TypeRegistry` files the MINIMAL component beside the COMPLETE
  one (`insert_minimal_component`, `insert_type_component`,
  `TypeComponent`, `from_ast::type_component`), resolves a component
  identifier of either kind to its minimal member
  (`component_minimal_member`), files the components a `getTypes` reply
  carries whole or not at all (`insert_components_of`) and reports a
  component it lacks as unresolved (`holds_component`,
  `first_unresolved_dependency`). `to_minimal` projects a complete
  component identifier onto its minimal one.
- Assignability of strongly connected components: a component member is
  compared through its minimal object, and a pair met again while it is
  being decided is taken as the answer it is being given, so two evolutions
  of a recursive type are compared member by member (the pair of
  identifiers that is `No("kinds do not match")` before).
- A declared annotation on the element of a collection, on a map's key and
  on a union's discriminator (DDS-XTypes 1.3 §7.2.2.6, §7.3.1.2.5).
  `from_ast` writes it into the `ann_custom` of `CompleteCollectionElement`
  and `CompleteDiscriminatorMember`. A collection with such an annotation
  is a non-plain type in the COMPLETE form (§7.3.4.1): it gets a
  `CompleteSequenceType`, `CompleteArrayType` or `CompleteMapType`, named
  `<containing type>.<member>` as §7.3.4.9.2 step 4b names an anonymous
  type, and the identifier is the hash of that object; the MINIMAL form
  stays the plain collection. `register_all` and `dependency_type_objects`
  file the object and the annotation's declaration, and the projection of
  a complete object onto its minimal form turns the collection's hash back
  into the plain identifier. `dynamic::to_ast` reads the applications back
  into `XTypesFacet::element_annotations`, `key_annotations` and
  `discriminator_annotations`; it refused an annotation on a collection
  element before, and merged the discriminator's into the union's.

### Changed

- The TypeLookup codecs and the `TypeObject` codecs follow the encoding version of the stream they
  are given (`framing::EncodingVersion`, read off the alignment cap: `xcdr1_writer`/`xcdr1_reader`
  eight, `xcdr2_writer`/`xcdr2_reader` four). Under XCDR1 an appendable aggregate has no `DHEADER`,
  a mutable one is a PL_CDR1 list ending in `PID_LIST_END`, and an `@optional` member of a final or
  appendable structure has a parameter header instead of a presence octet (XTypes 1.3 §7.4.1). An
  eight-aligned `Writer::new`/`Reader::new` is now read as XCDR1; a `TypeObject` stream names its
  XCDR2 buffer. An extended (`default:`) Annex B arm has no XCDR1 form and is refused under it.
- `TypeLookupEncoding` (`Xcdr2`, the default: CDR2_LE `{0x00,0x07}`, DDS-XTypes 1.4 Beta 1
  §7.6.3.3.3; `Xcdr1`: CDR_LE `{0x00,0x01}`),
  `TypeLookupEncapsulation::read` (the identifier names the encoding), `TypeLookupRequest::to_payload`
  and `TypeLookupReplyMessage::to_payload`. A sample ends four-aligned and its options count the
  padding (XTypes 1.3 §7.6.3.1.2).

- `TypeDescriptor::bound` of an unbounded string, sequence or map is the one
  value `LENGTH_UNLIMITED` (Table 50: "the length of the property value is
  one"); it was an empty list. A descriptor with an empty list is still read
  as unbounded.

- `DynamicTypeMember::equals` also compares the types the members belong to
  (Table 52).

- `DynamicSample` has reference semantics (§7.5.2.11, "DynamicData objects
  have reference semantics"). `get_complex_value` and `loan_value` return
  a handle on the value inside the sample, and a setter on it is seen
  through the sample and the other way round; `clone` is the copy of
  §7.5.2.11.4. A character of a string, a flag of a bitmask and an
  unselected member of a union have no storage and are copies, the last
  `RETCODE_NO_DATA`; a handle whose value was removed reads and writes
  `RETCODE_NO_DATA`. `return_loaned_value` ends the loan and writes
  nothing back. `DynamicSample::value` and `into_value` return
  `Result<DynamicData, DynamicError>`: a copy of what the sample holds.

- Dynamic Language Binding (DDS-XTypes 1.3 §7.5.2). `TypeDescriptor`: an
  alias names its target in `base_type` and has no `element_type`; a string
  has Char8 or Char16 as `element_type`; a bitmask has its bound in `bound`
  and Boolean as `element_type`, and `bit_bound` is the enumeration's only.
  `MemberDescriptor::label` is `Vec<i64>` (Table 53), and a union label the
  model cannot evaluate is an error where it was dropped. `id()` of an
  enumerator or a bit is `MEMBER_ID_INVALID` (§7.5.2.7.5); their value and
  position are `DynamicTypeMember::literal_value`. The flags of a bitmask
  have Boolean type; an alias has the members of its base type; a bitset has
  none (Table 51); `member_by_id` finds members of aggregated types only.
  `DynamicType::element_type` of an alias is `None`, its target is
  `base_type`. `TypeDescriptor::is_consistent` checks every property
  against the kinds that carry it.
- `DynamicTypeBuilder::add_member` and `add_case` give a member with no id
  one more than the maximum in the type (§7.5.2.7.5), take an alias to its
  base type (Table 55), and have `add_member_at` and `add_case_at` for an
  index (§7.5.2.7.6). `bitmask` refuses a bound outside 1 to 64;
  `LENGTH_UNLIMITED` is no bound for a string, a sequence and a map
  (§7.5.2.2).

### Fixed

- The collection TypeObjects are framed as XTypes 1.3 Annex B has them: `CompleteCollectionElement`,
  `CompleteCollectionHeader`, `CompleteArrayHeader`, `CompleteArrayType`, `MinimalCollectionElement`,
  `MinimalCollectionHeader` and `MinimalArrayHeader` are appendable (a `DHEADER` each under XCDR2),
  and the `detail` of a sequence's and a map's header is `@optional` and absent for an anonymous
  type (`CompleteSequenceType::detail`, `CompleteMapType::detail` are `Option`). The codecs wrote
  the members inline and the detail always. Byte-verified against the serializer of Cyclone DDS
  11.0.1. The COMPLETE hash of a collection with annotated elements changes with it.
- The `default:` arms of Annex B (`ExtendedTypeDefn`, `ExtendedAnnotationParameterValue`,
  `CompleteExtendedType`, `MinimalExtendedType`) have their XCDR1 form, a parameter list that
  `PID_LIST_END` closes; they were refused under XCDR1.

- `DynamicType::values_equal` is the equality of two data samples of
  DDS-XTypes 1.3 §7.5.2.11.5: members without regard to their order, a
  non-optional member that one sample leaves out equal to its default, an
  optional member present in one sample only a difference, a map and a
  bitmask unordered. `DynamicData`'s `==` knows no type and compares in order.
- `check_data_repr_extensibility` accepts XCDR version 1 with APPENDABLE, which
  Table 60 lists (`CDR_BE`, `CDR_LE`); it refused the pairing.
- `builtin_topic_types` declares the four built-in types as Annex E does:
  `@extensibility(APPENDABLE)`, the member `value`, without `@nested`
  (DDS-XTypes 1.3 §7.6.5). They were `@final` with the member `data`.
- `TypeConsistencyEnforcement::not_announced` is the policy of an endpoint
  that announces none: DISALLOW_TYPE_COERCION (DDS-XTypes 1.3 §7.6.3.4.1).
- `TypeMatcher` under DISALLOW_TYPE_COERCION requires equivalent types
  (DDS-XTypes 1.3 §7.6.3.4.2 Step 1); it ran the assignability check that
  ALLOW_TYPE_COERCION uses. `prevent_type_widening`, `ignore_sequence_bounds`,
  `ignore_string_bounds` and `ignore_member_names` apply only under
  ALLOW_TYPE_COERCION and read as true/false/false/false otherwise
  (§7.6.3.4.1). `force_type_validation` no longer changes the assignability
  configuration; it fails Step 2 only.
- `TypeRegistry::component_members` returns all members of a strongly
  connected component in step 4b order (used by the getTypes answer).
- `TypeInformation::from_registry` fills `dependent_typeid_count` and the
  dependent lists of both sides with the types the topic type depends on,
  followed transitively, each with its serialized size, and `-1` when the
  registry lacks one (DDS-XTypes 1.3 §7.6.3.2.1). `from_minimal_and_complete`
  wrote 0 and an empty list for a type that has dependencies.
- The literals of an enumerated type are written in increasing value order
  and the flags of a bitmask in increasing position order, in the minimal
  and the complete form (DDS-XTypes 1.3 §7.3.4.5); the hash of the type is
  taken over them. `from_ast` and the builders wrote them in declaration
  order.
- Strongly connected components (DDS-XTypes 1.3 §7.3.4.9): the temporary
  identifier of step 4b.i carries `sc_component_id = {EK, hash = 0}` where
  it carried `TK_NONE`; the hash of steps 4c to 4e is the MD5 of the
  XCDR2 serialisation of the `TypeObjectSeq` (a `DHEADER`, the count, the
  objects on four-octet boundaries), where the count was written without
  a `DHEADER` and the objects were not aligned; an `SCCIdentifier` with
  `scc_index = 0` (§7.3.4.9.3) is written and read.
- The `CollectionElementFlag` of a plain collection's `TypeIdentifier`
  carries the element's `@external` (`IS_EXTERNAL`, bit 2; DDS-XTypes 1.3
  §7.3.1.2.1.4) and, for a sequence and an array, its `@try_construct`
  behaviour, which `from_ast` wrote as DISCARD whatever the declaration
  said. `dynamic::to_ast` reads both back. The IDL front end carries
  `sequence<@external Foo, 42>` and `map<string, @external Foo, 42>`
  instead of refusing them.
- Minimal masks, name limits, strong assignability: see the entries above.
- Assignability follows Table 17, Table 18 and Table 19 of DDS-XTypes 1.3
  where it did not:
  - strong assignability (§7.2.4.3) for the element of an array and a
    sequence, the key and element of a map, the member type of the same
    index in an `@appendable` or `@final` struct and the discriminator of
    a union; delimited types are computed after §7.2.4.2
    (`AssignabilityConfig::appendable_is_delimited` says whether an
    `@appendable` type is delimited, true for XCDR2);
  - members of the same index in an `@appendable` or `@final` struct agree
    on `optional`; a member that is `@optional` is not one that may not be
    missing, `@must_understand` or not;
  - member types are compared as `KeyErased` types, and a key member of
    structure type is compared by `KeyHolder`, one of union type also by
    the discriminator-value rule;
  - an enumeration whose flag word names no extensibility is APPENDABLE and
    is compared as such.
- The limits of DDS-XTypes 1.3 Annex B are enforced when a `TypeObject` is
  written and when it is read: `MEMBER_NAME_MAX_LENGTH` and
  `TYPE_NAME_MAX_LENGTH` (256) for member, parameter, type and annotation
  names, `ANNOTATION_STR_VALUE_MAX_LEN` (128) for the string and wstring
  values of an annotation parameter. `ANNOTATION_OCTETSEC_VALUE_MAX_LEN` is
  declared. A longer value is a `WireError::OutOfRange`.
- `MemberFlagMinimalMask` (`0x003f`) and `TypeFlagMinimalMask` (`0x0007`)
  exist (`type_object::flags`) and are applied to MINIMAL objects, in
  `from_ast`, `builder` and `to_minimal`: `IS_NESTED` and `IS_AUTOID_HASH`
  are no longer in the flags of a minimal struct or union. `IS_DEFAULT` stays
  on union members and enumeration literals; Annex B's comment selects it
  and its value does not, and the minimal form has no other way to name the
  default case.
- `AnnotationParameterValue` has all cases of the Annex B union:
  `Float128Bits` (`TK_FLOAT128`), `Char16` (`TK_CHAR16`), `String16`
  (`TK_STRING16`) and `Extended` (the `default` case,
  `ExtendedAnnotationParameterValue`). An annotation declaration with a
  `long double`, `wchar` or `wstring` parameter builds a `TypeObject`; a
  discriminator without a case is read as `Extended` and not refused.
  `zerodds-float128` is a non-optional dependency for it.
- `TypeConsistencyKind` has the two values of DDS-XTypes 1.3 Annex D
  (`DisallowTypeCoercion`, `AllowTypeCoercion`); the third variant
  `ForceTypeValidation` is gone, `force_type_validation` stays the field of
  the policy. `TypeConsistencyKind::from_u32` returns a `Result`, and
  `TypeConsistencyEnforcement::try_from_bytes` refuses a `kind` other than
  0 and 1.

### Changed

- Ported from `zerodds-cdr` to `zerodds-wire`. One `WireError` where there
  were a `DecodeError` and an `EncodeError`; `wire`'s writer is infallible,
  so every write lost its `?` while `encode_into` kept its `Result` for the
  one thing that can still fail — a length that does not fit a `u32`.
  `struct_enc::encode_appendable` / `decode_appendable` are now `framing.rs`
  over `Writer::write_dheader` and `Reader::read_dheader`.
- `builder::Extensibility` and `builder::TryConstruct` are re-exports of
  `zerodds_ast::Extensibility` and `zerodds_ast::TryConstruct`. They were
  variant-for-variant copies, which is a second answer to a question that
  must have one.

### Added

- `from_ast` — the `TypeObject` and `TypeIdentifier` of a type in
  `zerodds-ast`'s model, minimal and complete. Refuses a recursive type and
  a construct §7.3 has no kind for, with the declaration named.
- Table 19 rows that were missing: union (§7.2.4.4.4.5), bitmask
  (§7.2.4.4.4.6), bitset (§7.2.4.4.4.7), alias (§7.2.4.4.3) and the
  sequence, array and map objects. `check_minimal_types` handled structures
  and enumerations and sent everything else to "type kinds do not match",
  which was not true — the kinds matched.

### Fixed

- **The annotation `TypeObject`s are Annex B's.** `MinimalAnnotationType`
  and `CompleteAnnotationType` wrote a `member_id` that
  `CommonAnnotationParameter` does not declare, the parameter's full name
  where Annex B has a `NameHash`, a `CompleteTypeDetail` where
  `CompleteAnnotationType` has a `CompleteAnnotationHeader` with
  `annotation_name` alone, the default value behind a length of its own
  instead of as the `AnnotationParameterValue` union, no `DHEADER` on the
  appendable parameters and headers, and the parameters in declaration
  order where §7.3.4.5 orders the minimal sequence by name hash and the
  complete one by name. A parameter the declaration gives no `default` for
  now takes the default value of its type (§7.3.1.2.4, Table 9), since
  Annex B does not declare `default_value` `@optional` and the wire has no
  octet for "no default". An object of this kind reaches the wire only when
  a peer asks for an annotation type over TypeLookup (§7.6.3.3), which no
  vendor capture has been made of: the reference for
  `tests/typeobject_annotation_annexb.rs` is the Annex B IDL, and its
  doc comment says so. The type hash of an annotation type changes with
  this; no other type kind's does.
- `APPENDABLE` assignability refused a reader that had grown a tail member.
  §7.2.4.4.4.4 compares the shorter member sequence as a prefix of the
  longer, either way round.
- `MUTABLE` assignability read `@optional` where the clause reads
  `must_understand`. `@optional` decides whether a member may be absent
  from a *sample*; `must_understand` decides whether it may be absent from
  the *type*.
- A writer member marked `@must_understand` that the reader does not
  declare was not checked at all and is now refused, as §7.2.4.4.4.4
  requires.

## [1.0.0-rc.1]

Initial release materialization of the `zerodds-types` crate.

### Spec references

- **OMG XTypes 1.3** §7.3 — TypeIdentifier union, TypeObject (minimal + complete), TypeInformation, TypeLookup service, hashing.
- **OMG XTypes 1.3** §7.5 — DynamicType / DynamicData reflection API.
- **OMG XTypes 1.3** §7.6 — wire encoding + discovery integration + bridge.
- **OMG XTypes 1.3** §7.2.4 — assignability + type-compatibility rules.
- **OMG XTypes 1.3** §7.3.1.2 — EquivalenceHash (MD5 → 14 byte).
- **OMG XTypes 1.3** §7.6.8 — KeyHash with endpoint-relative computation.
- **OMG DDS 1.4** §2.2.3 — TypeConsistencyEnforcement QoS policy.

### Public-API

**TypeIdentifier** (`type_identifier`):

- `TypeIdentifier::{None, Primitive, String8Small, String8Large, String16Small, String16Large, PlainSequence{Small,Large}, PlainArray{Small,Large}, PlainMap{Small,Large}, StronglyConnectedComponentRef, EquivalenceHash}`.
- `EquivalenceHash` ([u8; 14]), `EquivalenceKind`, `PrimitiveKind`, `PlainCollectionHeader`, `StronglyConnectedComponentId`.

**TypeObject** (`type_object`):

- `TypeObject::{Minimal, Complete}`.
- `MinimalTypeObject` + `CompleteTypeObject` with 10 variants: `Alias`, `Annotation`, `Struct`, `Union`, `Bitset`, `Sequence`, `Array`, `Map`, `Enumerated`, `Bitmask`.
- Per kind: `CompleteX` / `MinimalX` structures with header + members + flags.

**TypeInformation** (`type_information`):

- `TypeInformation`, `TypeIdentifierWithDependencies`, `TypeIdentifierWithSize`.

**TypeLookup** (`type_lookup`): `getTypes` / `getTypeDependencies` IDL service wire format.

**Builder** (`builder`): programmatic builder for all kinds (struct/union/enum/bitmask/alias/bitset/annotation + collections).

**Hashing** (`hash`):

- `compute_hash`, `compute_minimal_hash`, `compute_complete_hash`.
- `to_hashed_type_identifier`.

**Resolve** (`resolve`): `TypeRegistry` + alias resolution + DoS caps (`DEFAULT_MAX_RESOLVE_DEPTH`).

**Assignability** (`assignability`):

- `is_assignable(writer, reader, registry, config) -> Assignable`.
- `flatten_inheritance` for struct/union with a base type.
- `AssignabilityConfig`, `Assignable`, `InheritanceError`.

**TypeMatcher** (`type_matcher`):

- `TypeMatcher::new(&TypeConsistencyEnforcement)`.
- `match_types(writer_ti, reader_ti, registry) -> TypeMatchResult`.
- `TypeMatchResult::{Matches, Mismatch{reason}}`.

**QoS** (`qos`): `TypeConsistencyEnforcement` (all fields + default), `TypeConsistencyKind`, `DataRepresentation`.

**Dynamic** (`dynamic`):

- `DynamicType` / `DynamicTypeMember` / `DynamicData` reflection API (XTypes §7.5).
- `DynamicTypeBuilderFactory` + `DynamicTypeBuilder` for programmatic type construction.
- `TypeKind`, `TypeDescriptor`, `MemberDescriptor`, `ExtensibilityKind`, `TryConstructKind`.
- `DynamicError` with `Unsupported` / `Inconsistent` / `IllegalOperation` / `BuilderConflict` / `PreconditionNotMet`.
- **Bridge** (`dynamic::bridge`): `DynamicType::to_type_object` for **Struct, Union, Enumeration, Bitmask, Alias** (spec §7.6.3). Collection kinds (array/sequence/map) are TypeIdentifier-exclusive; bitset/annotation need MemberDescriptor extensions.

### Implementation

- `forbid(unsafe_code)`.
- `#![cfg_attr(not(feature = "std"), no_std)]` with mandatory `alloc`.
- 350+ tests green (281 unit + 9 dynamic + 40 compliance_typeobject + 5 fuzz_smoke + 5 proptest_assignability + 8 type_lookup_service).
- proptest suite for assignability rules.
- compliance_typeobject golden vectors for TypeObject wire encoding.
- DynamicType bridge with 5 implemented kinds (struct + union + enum + bitmask + alias) plus 5 explicitly classified (collection-3 + bitset + annotation).

### Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std`   | ✅      | std re-exports, implies `alloc` |
| `alloc` | ✅      | mandatory (Vec/String/BTreeMap) |
