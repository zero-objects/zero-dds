# DDS-XTypes 1.3 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (340 pages, OMG formal/2020-02-04) —
[OMG DDS-XTypes 1.3](https://www.omg.org/spec/DDS-XTypes/1.3/)

Item-by-item audit against the PDF according to
`internal/spec-coverage/PROCESS.md`. Page numbers are PDF pages (PDF page =
printed page + 16). Every rule of type assignability, every table row, every
annotation, every serialization rule, every operation of the Dynamic Language
Binding and every field of the built-in topic extensions is an item of its
own. `done` stands only where code and test cover the requirement; a test
citation names the test function as `file::module::name`. A repository
citation names file, symbol and line (l.); the lines refer to the state with
which this file was checked in. Every statement about behaviour is read from
the code; no test was run and nothing was built for this audit.

**Context:** XTypes is spread over these crates in ZeroDDS:

- `crates/ast/` — the shared type model (`TypeKind`, `PrimitiveKind`,
  `XTypesFacet` with the XTypes annotations).
- `crates/idl-parse/` and `crates/idl-lower/` — the IDL type representation:
  parser, annotations, assignment of member IDs, validation rules.
- `crates/types/` — TypeIdentifier, TypeObject (MINIMAL and COMPLETE),
  EquivalenceHash, assignability, TypeLookup types, TypeInformation, the
  dynamic stack (`DynamicType`, `DynamicData`) and the negotiation of the data
  representation.
- `crates/ast-layout/` — the wire plan: alignment, DHEADER, EMHEADER, length
  codes, KeyHolder, the parameter list of XCDR1.
- `crates/wire/` and `crates/wire-codec/` — reader and writer of the octets,
  the plan-driven encoder and decoder, MD5 and TryConstruct.
- `crates/lang-rules/` — the code generators of the language bindings.
- `crates/xml/` — XML and XSD type representation and XML data.
- `crates/rtps/`, `crates/discovery/` and `crates/dcps/` — the use by DDS:
  parameters of the built-in topics, TypeLookup endpoints, matching with type
  consistency (`runtime.rs`), DataRepresentation QoS, built-in types.

The Rust API of `zerodds-dcps` is not a mapping of the IDL of Annex C:
operations return `Result<T, E>` instead of `DDS::ReturnCode_t` and
collections are `Vec`. Where an operation of the specification is missing in
the Rust API, that is listed as an item of its own.

The Plain Language Binding is checked for the three languages for which
§7.5.1 has tables (C, C++, Java), and for Rust. The code generators of the
other languages in `crates/lang-rules/` are not the subject of this audit.

**Negative list:** `dds-xtypes-1.3.gaps.md` is the output of the tool
`tools/xtypes-1.3-gap-scan/` and is not part of this audit.

**Open items:** none; every item is `done` or `n/a (informative)`.

---

## §1 Scope

### §1 Scope

**Spec:** §1, p. 17–18 — "The specification addresses four related concerns:
the type system, the representation of types, the representation of data, and
the language bindings used to access types and data."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure of the specification in four
packages; the requirements are in §7.2 to §7.6 and are listed there one by
one.

---

## §2 Conformance Criteria

### §2 Conformance levels Minimal, Basic, Complete

**Spec:** §2, p. 19 — "There are three conformance levels: Minimal conformance
with XTYPES version 1.2 requires conformance to the Programming Interface and
the Minimal Network Interoperability Profile. Basic conformance [...] the Basic
Network Interoperability Profile. Complete conformance [...] requires Basic
conformance as well as conformance to the two optional profiles."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — assignment of clauses to conformance
levels; whether a level is met follows from the status of the assigned items.
The levels name "XTYPES version 1.2" in a specification of version 1.3 (defect
of the specification, listed in the findings file).

### §2.1 Programming Interface Conformance

**Spec:** §2.1, p. 19 — The "Extensible and Dynamic Types" profile consists of
clause 7 up to and including 7.3, 7.5, 7.6 without 7.6.8, Annex B, Annex C and
"Annex D: Built-in Types".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — assignment of clauses to the profile; the
clauses are listed one by one. The list names "Annex D: Built-in Types"; in
the PDF Annex D is "DDS Built-in Topic Data Types" and Annex E is "Built-in
Types" (defect of the specification, listed in the findings file).

### §2.2 Network Interoperability Conformance

**Spec:** §2.2, p. 20 — "Regardless of profile, conformance with respect to
network interoperability requires conformance to the Real-Time
Publish-Subscribe Wire Protocol specification [RTPS]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference to DDSI-RTPS; its coverage is in
`ddsi-rtps-2.5.md`.

### §2.2.1 Minimal Network Interoperability Profile

**Spec:** §2.2.1, p. 20 — comprises 7.3.4, 7.6.1, 7.6.3 without 7.6.3.3 (with
7.6.3.1.1 "with support limited to version 2 encoding"), 7.6.8, 7.4.2, 7.4.3,
Annex B and Annex D.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — assignment of clauses to the profile; the
clauses are listed one by one.

### §2.2.2 Basic Network Interoperability Profile

**Spec:** §2.2.2, p. 20 — "Conformance with the Basic Network Interoperability
Profile requires conformance with the Minimal Network Interoperability profile
and the following clauses: 'Built-in TypeLookup service' (Clause 7.6.3.3)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — assignment; §7.6.3.3 is listed one by one.

### §2.3 Optional XTYPES 1.1 Interoperability Profile

**Spec:** §2.3, p. 20 — "requires conformance with the Basic Network
Interoperability profile and support of version 1 encoding in Clause 7.6.3.1.1".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — assignment; encoding version 1 is listed
one by one under §7.4.1.

### §2.4 Optional XML Data Representation Profile

**Spec:** §2.4, p. 21 — comprises 7.3.2, 7.3.3, 7.4.4 and the schemas of Annex
A.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — assignment; the clauses are listed one by
one.

---

## §3 Normative References

### §3 Normative References

**Spec:** §3, p. 23–24 — list of normative documents ([DDS] 1.4, [RTPS] 2.3,
[IDL] 4.2, [CDR] CORBA 3.1 Part 2 §9.3, language standards, language mappings,
[UNICODE] 9.0.0, [IEEE-754] and others).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — list of references.

---

## §4 Terms and Definitions

### §4 Terms and Definitions

**Spec:** §4, p. 25 — definitions of DCPS and DDS.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary.

---

## §5 Symbols

### §5 Symbols

**Spec:** §5, p. 27 — "No additional symbols are used in this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no symbols.

---

## §6 Additional Information

### §6.1 Data Distribution Service for Real-Time Systems (DDS)

**Spec:** §6.1, p. 29–31 — motivation; Table 1 (existing mechanisms) and
Table 2 (features of this specification).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — background; the features of Table 2 are
listed one by one in §7.2 to §7.6.

### §6.2 Acknowledgments

**Spec:** §6.2, p. 31 — list of submitting companies.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acknowledgments.

---

## §7.1 Overview

### §7.1 Overview

**Spec:** §7.1, p. 33–35 — Figure 2 and Figure 3: relationships between Type
System, Type Representation, Language Binding and Data Representation, using
the "AlarmType" example.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — conceptual model; Figure 3 is explicitly
"non-normative".

---

## §7.2 Type System

### §7.2 Type System — purpose

**Spec:** §7.2, p. 35 — "The Type System defines the data types that can be used
for DDS Topics and therefore the type of the data that can be published and
subscribed via DDS."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction; the model follows in §7.2.2.

### §7.2.1 Background (Non-Normative)

**Spec:** §7.2.1, p. 35–36 — list of the supported primitive and constructed
types; reference to type evolution, type inheritance and sparse types.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the heading says "Non-Normative".

### §7.2.1.1 Type Evolution Example

**Spec:** §7.2.1.1, p. 36–37 — example `VehicleLocationType` with four appended
members.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example under §7.2.1 (non-normative); the
rules are in §7.2.4.

### §7.2.1.2 Type Inheritance Example

**Spec:** §7.2.1.2, p. 37–38 — example `VehicleLocation3DType :
VehicleLocationType`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example under §7.2.1 (non-normative).

### §7.2.1.3 Sparse Types Example

**Spec:** §7.2.1.3, p. 38 — optional members and omitted values with a default.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example under §7.2.1 (non-normative); the
rules are in §7.2.2.4.4.4.7 and §7.4.

---

## §7.2.2 Type System Model

### §7.2.2 Type name: non-empty, unique in its namespace, a valid IDL identifier

**Spec:** §7.2.2, p. 39 — "A type has a non-empty name that is unique within its
namespace (see Clause 7.2.2.1). The set of valid names is the set of valid
identifiers defined by the OMG IDL specification [IDL]."

**Repo:** `crates/idl-lower/src/lower.rs::duplicate` (l. 3734);
`crates/ast/src/invariant.rs::NameCollidesOnlyInCase` (l. 99);
`crates/idl-lower/src/scope.rs::is_bound_here` (l. 160)

**Tests:** `crates/idl-lower/tests/lowering.rs::two_names_differing_only_by_case_collide`,
`crates/idl-lower/tests/lowering.rs::a_name_declared_in_two_openings_of_one_module_collides`,
`crates/idl-lower/tests/validation.rs::the_same_name_in_two_scopes_does_not_collide`

**Status:** done — the IDL front-end rules for identifiers apply; two
declarations of the same name in one scope (also when they differ only in case)
are refused with `DuplicateName`.

### §7.2.2 Type kind

**Spec:** §7.2.2, p. 39 — "A type has a kind that identifies which primitive
type it is or, if it is a constructed type, whether it is a structure, union,
sequence, etc."

**Repo:** `crates/ast/src/node/type_.rs::TypeKind` (l. 201);
`crates/types/src/dynamic/descriptor.rs::TypeKind` (l. 48);
`crates/types/src/type_object/kinds.rs::TK_STRUCTURE` (l. 26)

**Tests:** `crates/types/src/dynamic/descriptor.rs::tests::every_model_primitive_has_a_kind_and_comes_back`,
`crates/types/src/dynamic/descriptor.rs::tests::the_classifications_are_disjoint`

**Status:** done

### §7.2.2 Primitive and constructed types

**Spec:** §7.2.2, p. 39 — "The type system supports Primitive Types [...] whose
names are predefined. [...] The type system supports Constructed Types whose
names are explicitly provided as part of the type-definition process."

**Repo:** `crates/ast/src/node/type_.rs::PrimitiveKind` (l. 17);
`crates/ast/src/node/type_.rs::TypeNode` (l. 444, field `name`)

**Tests:** `crates/ast/tests/node_space.rs::every_primitive_of_the_spec_table_is_representable`,
`crates/idl-lower/tests/lowering.rs::a_struct_carries_its_members_in_order`

**Status:** done

### §7.2.2.1 Namespace: equal names in the same namespace are an error

**Spec:** §7.2.2.1, p. 39 — "it is an error for different elements within the
same namespace to have the same name. However, it is legal for different
elements within different namespaces to have the same name."

**Repo:** `crates/idl-lower/src/lower.rs::duplicate` (l. 3734);
`crates/ast/src/invariant.rs::DuplicateMemberName` (l. 56);
`crates/ast/src/invariant.rs::DuplicateEnumeratorName` (l. 63)

**Tests:** `crates/idl-lower/tests/validation.rs::two_members_of_one_aggregate_differing_only_by_case_collide`,
`crates/idl-lower/tests/validation.rs::the_same_name_in_two_scopes_does_not_collide`,
`crates/idl-lower/tests/lowering.rs::one_literal_name_in_two_enumerations_collides`

**Status:** done

### §7.2.2.1 Modules as namespaces, fully qualified name

**Spec:** §7.2.2.1, p. 40 — "Modules are namespaces whose contained named
elements are types. The concatenation of module names with the name of a type
inside of those modules is referred to as the type's 'fully qualified name.'"

**Repo:** `crates/idl-lower/src/scope.rs::qualify` (l. 199);
`crates/ast/src/node/type_.rs::TypeNode` (l. 444, fields `module`,
`scope`)

**Tests:** `crates/idl-lower/tests/lowering.rs::nested_modules_nest_in_the_model_too`,
`crates/idl-lower/tests/lowering.rs::a_module_reopened_across_two_files_converges_on_one_node`,
`crates/idl-lower/tests/reported_idl.rs::a_fully_qualified_scoped_name_resolves`

**Status:** done

### §7.2.2.1 Types as namespaces of their elements

**Spec:** §7.2.2.1, p. 40 — "Certain kinds of types are themselves namespaces
with respect to the elements inside of them."

**Repo:** `crates/ast/src/invariant.rs::DuplicateMemberName` (l. 56);
`crates/ast/src/invariant.rs::DuplicateEnumeratorName` (l. 63)

**Tests:** `crates/idl-lower/tests/validation.rs::two_members_of_one_aggregate_differing_only_by_case_collide`,
`crates/idl-lower/tests/validation.rs::a_case_member_named_like_its_union_collides`

**Status:** done — members of an aggregate and literals of an enumeration are
unique per type. The literals of an IDL enumeration live in the enclosing scope
under IDL 4.2; two enumerations of one scope with the same literal are refused
(`one_literal_name_in_two_enumerations_collides`).

---

## §7.2.2.2 Primitive Types

### §7.2.2.2 Primitive types outside any module, top-level names

**Spec:** §7.2.2.2, p. 43 — "The primitive types do not exist within any module;
their names are top-level names." Figure 6–8: `{nested = true}`,
`{extensibility_kind = <Not Applicable>}`.

**Repo:** `crates/ast/src/node/type_.rs::PrimitiveKind` (l. 17);
`crates/types/src/type_identifier/mod.rs::PrimitiveKind` (l. 315)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done — a primitive type is a discriminator without a body, without a
name and without an extensibility in the TypeIdentifier.

### §7.2.2.2 Table 3 — BOOLEAN_TYPE

**Spec:** §7.2.2.2, Table 3, p. 42 — "BOOLEAN_TYPE Boolean Boolean type. Data of
this type can only take two values: true and false."

**Repo:** `crates/ast/src/node/type_.rs::Boolean` (l. 19);
`crates/types/src/type_identifier/kinds.rs::TK_BOOLEAN` (l. 21);
`crates/types/src/from_ast.rs::primitive_kind` (l. 1479)

**Tests:** `crates/ast/tests/node_space.rs::every_primitive_of_the_spec_table_is_representable`,
`crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`,
`crates/types/src/dynamic/descriptor.rs::tests::every_model_primitive_has_a_kind_and_comes_back`

**Status:** done

### §7.2.2.2 Table 3 — BYTE_TYPE

**Spec:** §7.2.2.2, Table 3, p. 42 — "BYTE_TYPE Byte Single opaque byte. A Byte
value has no numeric value."

**Repo:** `crates/ast/src/node/type_.rs::Octet` (l. 21);
`crates/types/src/type_identifier/kinds.rs::TK_BYTE` (l. 23);
`crates/types/src/from_ast.rs::primitive_kind` (l. 1479, arm l. 1493)

**Tests:** `crates/types/src/from_ast.rs::primitive_kind_tests::octet_and_uint8_are_two_kinds`,
`crates/types/src/dynamic/descriptor.rs::tests::octet_and_uint8_are_two_kinds`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done — `octet` becomes `TK_BYTE` (0x02) and stays separate from
`uint8` (`TK_UINT8`).

### §7.2.2.2 Table 3 — INT_8_TYPE

**Spec:** §7.2.2.2, Table 3, p. 42 — "INT_8_TYPE Int8 Signed integer minimally
capable of representing values in the range -127 to +128."

**Repo:** `crates/ast/src/node/type_.rs::Int8` (l. 27);
`crates/types/src/type_identifier/kinds.rs::TK_INT8` (l. 43)

**Tests:** `crates/ast/tests/node_space.rs::every_primitive_of_the_spec_table_is_representable`,
`crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done — the value range is `i8` (−128 to +127). The table names "-127
to +128", which a signed 8-bit type cannot represent (defect of the
specification, listed in the findings file).

### §7.2.2.2 Table 3 — UINT_8_TYPE

**Spec:** §7.2.2.2, Table 3, p. 42 — "UINT_8_TYPE UInt8 Unsigned integer
minimally capable of representing values in the range 0 to +255."

**Repo:** `crates/ast/src/node/type_.rs::UInt8` (l. 29);
`crates/types/src/type_identifier/kinds.rs::TK_UINT8` (l. 45);
`crates/types/src/from_ast.rs::primitive_kind` (l. 1479, arm l. 1494)

**Tests:** `crates/types/src/from_ast.rs::primitive_kind_tests::octet_and_uint8_are_two_kinds`,
`crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — INT_16_TYPE

**Spec:** §7.2.2.2, Table 3, p. 42 — "INT_16_TYPE Int16 Signed integer minimally
capable of representing values in the range -32768 to +32767."

**Repo:** `crates/ast/src/node/type_.rs::Int16` (l. 31);
`crates/types/src/type_identifier/kinds.rs::TK_INT16` (l. 25)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — UINT_16_TYPE

**Spec:** §7.2.2.2, Table 3, p. 42 — "UINT_16_TYPE UInt16 Unsigned integer
minimally capable of representing values in the range 0 to +65535."

**Repo:** `crates/ast/src/node/type_.rs::UInt16` (l. 33);
`crates/types/src/type_identifier/kinds.rs::TK_UINT16` (l. 31)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — INT_32_TYPE

**Spec:** §7.2.2.2, Table 3, p. 42 — "INT_32_TYPE Int32 Signed integer minimally
capable of representing values in the range -2147483648 to +2147483647."

**Repo:** `crates/ast/src/node/type_.rs::Int32` (l. 35);
`crates/types/src/type_identifier/kinds.rs::TK_INT32` (l. 27)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — UINT_32_TYPE

**Spec:** §7.2.2.2, Table 3, p. 42 — "UINT_32_TYPE UInt32 Unsigned integer
minimally capable of representing values in the range 0 to +4294967295."

**Repo:** `crates/ast/src/node/type_.rs::UInt32` (l. 37);
`crates/types/src/type_identifier/kinds.rs::TK_UINT32` (l. 33)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — INT_64_TYPE

**Spec:** §7.2.2.2, Table 3, p. 43 — "INT_64_TYPE Int64 Signed integer minimally
capable of supporting values in the range -9223372036854775808 to
+9223372036854775807."

**Repo:** `crates/ast/src/node/type_.rs::Int64` (l. 39);
`crates/types/src/type_identifier/kinds.rs::TK_INT64` (l. 29)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — UINT_64_TYPE

**Spec:** §7.2.2.2, Table 3, p. 43 — "UINT_64_TYPE UInt64 Unsigned integer
minimally capable of supporting values in the range 0 to +18446744073709551617."

**Repo:** `crates/ast/src/node/type_.rs::UInt64` (l. 41);
`crates/types/src/type_identifier/kinds.rs::TK_UINT64` (l. 35)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done — the value range is `u64` (0 to 18446744073709551615). The
table names 18446744073709551617, i.e. 2^64 + 1, as the upper bound (defect of
the specification, listed in the findings file).

### §7.2.2.2 Table 3 — FLOAT_32_TYPE

**Spec:** §7.2.2.2, Table 3, p. 43 — "FLOAT_32_TYPE Float32 Floating point
number minimally capable of supporting the range and precision of an IEEE 754
single-precision floating point value."

**Repo:** `crates/ast/src/node/type_.rs::Float32` (l. 43);
`crates/types/src/type_identifier/kinds.rs::TK_FLOAT32` (l. 37)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — FLOAT_64_TYPE

**Spec:** §7.2.2.2, Table 3, p. 43 — "FLOAT_64_TYPE Float64 Floating point
number minimally capable of supporting the range and precision of an IEEE 754
double-precision floating point value."

**Repo:** `crates/ast/src/node/type_.rs::Float64` (l. 45);
`crates/types/src/type_identifier/kinds.rs::TK_FLOAT64` (l. 39)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — FLOAT_128_TYPE

**Spec:** §7.2.2.2, Table 3, p. 43 — "FLOAT_128_TYPE Float128 Floating point
number minimally capable of supporting the range and precision of an IEEE 754
quadruple-precision floating point value."

**Repo:** `crates/ast/src/node/type_.rs::Float128` (l. 51);
`crates/types/src/type_identifier/kinds.rs::TK_FLOAT128` (l. 41);
`crates/types/src/from_ast.rs::primitive_kind` (l. 1479, arm l. 1506)

**Tests:** `crates/ast/tests/node_space.rs::every_primitive_of_the_spec_table_is_representable`,
`crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done — `long double` is present in the type model and in the
TypeIdentifier; the value is carried by `zerodds_float128::Float128` (see
§7.4.1.1.1, Table 31).

### §7.2.2.2 Table 3 — CHAR_8_TYPE

**Spec:** §7.2.2.2, Table 3, p. 43 — "CHAR_8_TYPE Char8 8-bit character type.
There is no encoding specified, it may be ASCII, ISO-8859-1, or used to hold a
byte of a multi-byte-encoded character set."

**Repo:** `crates/ast/src/node/type_.rs::Char` (l. 23);
`crates/types/src/type_identifier/kinds.rs::TK_CHAR8` (l. 47)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — CHAR_16_TYPE

**Spec:** §7.2.2.2, Table 3, p. 43 — "CHAR_16_TYPE Char16 16-bit character type
capable of supporting the Basic Multilingual Plane (BMP) encoded in UTF-16."

**Repo:** `crates/ast/src/node/type_.rs::WChar` (l. 25);
`crates/types/src/type_identifier/kinds.rs::TK_CHAR16` (l. 49)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2.1.1 Design Rationale (Non-Normative)

**Spec:** §7.2.2.2.1.1, p. 43 — rationale for two character types Char8 and
Char16.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the heading says "Non-Normative".

### §7.2.2.2.1.2.1 Use of Unicode

**Spec:** §7.2.2.2.1.2.1, p. 43–44 — "This specification uses the Unicode
Standard (version 9.0, June 2016) as the means to represent characters and
strings." Explanations of code points, planes, UTF-8 and UTF-16 follow.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition of terms; the requirements follow
in §7.2.2.2.1.2.2 to .7.

### §7.2.2.2.1.2.2 CHAR_8_TYPE: no encoding, 8 bits

**Spec:** §7.2.2.2.1.2.2, p. 44 — "This specification does not define an
encoding for the CHAR_8_TYPE. The only constraint is that it shall be
representable using 8 bits."

**Repo:** `crates/wire/src/writer.rs::write_char` (l. 1524);
`crates/wire/src/reader.rs::read_char` (l. 410)

**Tests:** `crates/wire/tests/char_width.rs::every_code_point_a_char_can_hold_round_trips`,
`crates/wire/tests/char_width.rs::a_char_above_u00ff_is_refused_and_not_narrowed`

**Status:** done — a `char` is one octet. A `char` value outside Latin-1 is
refused by the writer (§7.4.1.1.1).

### §7.2.2.2.1.2.3 Array or sequence of CHAR_8_TYPE: no encoding

**Spec:** §7.2.2.2.1.2.3, p. 44 — "This specification does not define an
encoding for the CHAR_8_TYPE that appears as an element of an array or sequence
of CHAR_8_TYPE."

**Repo:** `crates/wire/src/writer.rs::write_char` (l. 1524)

**Tests:** `crates/wire/tests/char_width.rs::every_code_point_a_char_can_hold_round_trips`,
`crates/wire/tests/char_width.rs::a_char_above_u00ff_is_refused_and_not_narrowed`

**Status:** done — the elements are written one octet each; no encoding across
the elements is assumed.

### §7.2.2.2.1.2.4 String<Char8>: UTF-8 in the data representation

**Spec:** §7.2.2.2.1.2.4, p. 44–45 — "The default encoding for String<Char8>
shall be UTF-8. This encoded shall be used for the externalized Data
Representation (see clause 7.4). Language bindings [...] If this is different
from UTF-8 the language binding shall manage the transformation to/from UTF-8
external Data Representation."

**Repo:** `crates/wire/src/writer.rs::write_string` (l. 1881);
`crates/wire/src/reader.rs::read_string` (l. 927)

**Tests:** `crates/wire/tests/wire.rs::a_string_of_bytes_that_are_not_utf8_is_refused`,
`crates/wire/tests/wire.rs::a_string_round_trips_with_its_terminator_consumed`

**Status:** done — the writer writes the UTF-8 octets of a Rust `str`, the
reader refuses octets that are not UTF-8 (`WireError::NotUtf8`).

### §7.2.2.2.1.2.5 CHAR_16_TYPE: BMP, UTF-16

**Spec:** §7.2.2.2.1.2.5, p. 45 — "The CHAR_16_TYPE shall be restricted to
representing Unicode codepoints in the Basic Multilingual Plane. [...] The
CHAR_16_TYPE encoding shall be UTF-16."

**Repo:** `crates/wire/src/writer.rs::write_wchar` (l. 1559)

**Tests:** `crates/wire/tests/char_width.rs::a_wchar_above_the_bmp_is_refused_and_not_narrowed`,
`crates/wire/tests/char_width.rs::the_reader_refuses_a_surrogate_rather_than_returning_something_else`

**Status:** done — a `wchar` outside the BMP is refused by the writer ("a BMP
character, not U+…").

### §7.2.2.2.1.2.6 Array or sequence of CHAR_16_TYPE: UTF-16, BMP

**Spec:** §7.2.2.2.1.2.6, p. 45 — "The representation of each CHAR_16_TYPE
element of an array or sequence of CHAR_16_TYPE shall be UTF-16 and shall be
restricted to being in the Basic Multilingual Plane".

**Repo:** `crates/wire/src/writer.rs::write_wchar` (l. 1559)

**Tests:** `crates/wire/tests/char_width.rs::a_wchar_above_the_bmp_is_refused_and_not_narrowed`,
`crates/wire/tests/char_width.rs::the_reader_refuses_a_surrogate_rather_than_returning_something_else`

**Status:** done — every element goes through the same writer as a single
`wchar`.

### §7.2.2.2.1.2.7 String<Char16>: UTF-16

**Spec:** §7.2.2.2.1.2.7, p. 45 — "The encoding for String<Char16> shall be
UTF-16. This encoded shall be used for the externalized Data Representation".

**Repo:** `crates/wire/src/writer.rs::write_wstring` (l. 1980);
`crates/wire/src/reader.rs::read_wstring` (l. 1027)

**Tests:** `crates/wire/tests/wstring.rs::every_cell_round_trips_under_every_unit_order`,
`crates/wire/tests/wstring.rs::an_unpaired_surrogate_is_refused_and_not_replaced`

**Status:** done — the default `WstringLengthPolicy::SPECIFICATION` writes two
octets per UTF-16 code unit. The vendor variants (four octets per unit, UTF-8)
can be selected through `WstringLengthPolicy` and are not the default. The
sentence ends with "If this is different from UTF-8" instead of UTF-16 (defect
of the specification, listed in the findings file).

### §7.2.2.3 String types: element Char8 or Char16

**Spec:** §7.2.2.3, p. 45–46 — "the element type of a string must be either
Char8 or Char16 (or an alias to one of these); other element types are
undefined." Figure 9: String8 `{element_type = Char8}`, String16 `{element_type
= Char16}`.

**Repo:** `crates/ast/src/node/type_.rs::CharWidth` (l. 120);
`crates/types/src/type_identifier/kinds.rs::TI_STRING8_SMALL` (l. 63)

**Tests:** `crates/types/src/assignability.rs::tests::string8_vs_string16_not_assignable`,
`crates/types/src/assignability.rs::tests::string_small_and_large_interchangeable`

**Status:** done — a string has only the width `Char` or `WChar` in the model;
other element types cannot be represented.

### §7.2.2.3 String types: variable length up to the bound

**Spec:** §7.2.2.3, p. 45 — "StringTypes are variable-sized; objects of a given
string type can have different numbers of elements [...] the length of a given
string object may change between zero and the string type's 'bound'".

**Repo:** `crates/ast/src/node/type_.rs::Bound` (l. 132);
`crates/wire/src/writer.rs::write_string_bounded` (l. 1915)

**Tests:** `crates/idl-lower/tests/completion.rs::a_bounded_string_and_sequence_carry_their_bound_expressions`,
`crates/idl-lower/tests/completion.rs::an_unbounded_collection_says_so_rather_than_carrying_a_zero`

**Status:** done

---

## §7.2.2.4 Constructed Types

### §7.2.2.4 Constructed types, recursive definitions

**Spec:** §7.2.2.4, p. 46–47 — "These other types may be either primitive types
or other constructed types: type definitions may be recursive to an arbitrary
depth. Constructed types are explicitly defined by a user [...] and are assigned
a name when they are defined."

**Repo:** `crates/types/src/from_ast.rs::structure` (l. 2082);
`crates/types/src/resolve.rs::transitive_dependencies` (l. 246)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_self_referential_type_gets_a_component_identifier`,
`crates/types/tests/strongly_connected_components.rs::a_mutually_recursive_pair_is_one_component_of_two`

**Status:** done — recursive types through sequences and `@external` are built;
their TypeIdentifier is a strongly connected component (§7.3.4.9).

### §7.2.2.4 Figure 10 — kinds of constructed types

**Spec:** §7.2.2.4, Figure 10, p. 47 — "There are several kinds of Constructed
Types: Collections, Aggregated types, Aliases, and Enumerated types. Collections
are homogeneous [...] Aggregated types are heterogeneous [...] Aliases introduce
a new name for another type. Enumerated types define a finite set of possible
integer values".

**Repo:** `crates/ast/src/node/type_.rs::TypeKind` (l. 201);
`crates/types/src/type_object/kinds.rs::TK_ALIAS` (l. 15)

**Tests:** `crates/types/src/dynamic/descriptor.rs::tests::the_classifications_are_disjoint`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::every_constructed_form_can_be_defined_inside_a_typedef`

**Status:** done

---

## §7.2.2.4.1 Enumerated Types

### §7.2.2.4.1 EnumeratedType: bit_bound

**Spec:** §7.2.2.4.1, Figure 11, p. 48 — `EnumeratedType` has "+ bit_bound:
Int32"; Enumeration and Bitmask are subclasses.

**Repo:** `crates/ast/src/query.rs::effective_bit_bound` (l. 319);
`crates/ast/src/lib.rs::DEFAULT_BIT_BOUND` (l. 66)

**Tests:** `crates/idl-lower/tests/completion.rs::an_enum_without_a_bit_bound_is_thirty_two_bits`,
`crates/idl-lower/tests/completion.rs::a_written_bit_bound_is_what_the_rule_reports`

**Status:** done — the limits of `@bit_bound` are listed under §7.3.1.2.1.9.

### §7.2.2.4.1 EnumeratedType: root = false

**Spec:** §7.2.2.4.1, Figure 11, p. 48 — Constraint `{root = false}`.

**Repo:** `crates/ast/src/node/type_.rs::Enumeration` (l. 258);
`crates/ast/src/node/type_.rs::Bitmask` (l. 263)

**Tests:** `crates/idl-lower/tests/lowering.rs::an_enum_carries_its_enumerators_in_order`,
`crates/idl-lower/tests/lowering.rs::a_bitmask_carries_its_bits`

**Status:** done — Enumeration and Bitmask have no field for a base type in the
model; inheritance cannot be represented.

### §7.2.2.4.1 EnumeratedType: extensibility FINAL or APPENDABLE

**Spec:** §7.2.2.4.1, Figure 11, p. 48 — constraint `{extensibility_kind = FINAL
or APPENDABLE}`; likewise Table 12, p. 74: "These types may be final or
appendable on a type-by-type basis."

**Repo:** `crates/idl-lower/src/annotation.rs::set_extensibility` (l. 1036)

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/idl-lower/tests/validation.rs::an_enum_or_bitmask_is_never_mutable`

**Status:** done — `@final` and `@appendable` reach an enumeration and a bitmask; the front end refuses `@mutable` on both (`validate.rs::extensibility_is_final_or_appendable`).

### §7.2.2.4.1 EnumeratedType: @mutable is refused

**Spec:** §7.2.2.4.1, Figure 11, p. 48 — `{extensibility_kind = FINAL or
APPENDABLE}`.

**Repo:** `crates/idl-lower/src/validate.rs::extensibility_is_final_or_appendable`

**Tests:** `crates/idl-lower/tests/validation.rs::an_enum_or_bitmask_is_never_mutable`

**Status:** done — `crates/idl-lower/src/validate.rs::extensibility_is_final_or_appendable` refuses `@mutable` on `enum` and `bitmask`; the model carries no `IS_MUTABLE` on an enumeration.

### §7.2.2.4.1.1 Table 4 — ENUMERATION_TYPE: one or more literals

**Spec:** §7.2.2.4.1.1, Table 4, p. 49 — "An enumerated type defines a closed
set of one or more literal objects of that type."

**Repo:** `crates/idl-lower/src/validate.rs::body_is_not_empty` (l. 175)

**Tests:** `crates/idl-lower/tests/validation.rs::an_enumeration_with_no_enumerator_is_refused`

**Status:** done

### §7.2.2.4.1.1 Table 4 — Name unique per literal

**Spec:** §7.2.2.4.1.1, Table 4, p. 49 — "Each object of a given enumerated type
has a name and an Int32 value that are each unique within that type."

**Repo:** `crates/ast/src/invariant.rs::DuplicateEnumeratorName` (l. 63)

**Tests:** `crates/idl-lower/tests/lowering.rs::one_literal_name_in_two_enumerations_collides`

**Status:** done

### §7.2.2.4.1.1 Table 4 — Value unique per literal

**Spec:** §7.2.2.4.1.1, Table 4, p. 49 — "[...] a name and an Int32 value that
are each unique within that type."

**Repo:** `crates/idl-lower/src/validate.rs::enumerator_values_are_unique`

**Tests:** `crates/idl-lower/tests/validation.rs::two_enumerators_of_one_value_are_rejected`

**Status:** done — `validate.rs::enumerator_values_are_unique` refuses two literals of one value (`enum E { @value(1) A, @value(1) B };`).

### §7.2.2.4.1.1 Table 4 — Value is Int32

**Spec:** §7.2.2.4.1.1, Table 4, p. 49 — "[...] has a name and an Int32 value".

**Repo:** `crates/idl-lower/src/validate.rs::enumerator_values_fit_int32`,
`crates/types/src/from_ast.rs::enumeration`

**Tests:** `crates/idl-lower/tests/validation.rs::an_enumerator_value_outside_int32_is_rejected`,
`crates/types/tests/type_system_rules.rs::an_enumerator_value_outside_int32_is_no_type_object`,
`crates/idl-lower/tests/completion.rs::an_explicit_enumerator_value_shifts_the_ones_after_it`

**Status:** `done` — the front end refuses `@value` outside `i32` (`enumerator_values_fit_int32`, also the value the implicit numbering reaches behind `2147483647`); `from_ast::enumeration` reports `FromAstError::OutOfRange` when the diagnostic is not heeded.

### §7.2.2.4.1.1 Table 4 — Value outside Int32: no test

**Spec:** §7.2.2.4.1.1, Table 4, p. 49 — an Int32 value per literal.

**Repo:** `crates/idl-lower/src/validate.rs::enumerator_values_fit_int32`,
`crates/types/src/from_ast.rs::enumeration`

**Tests:** `crates/idl-lower/tests/validation.rs::an_enumerator_value_outside_int32_is_rejected`,
`crates/types/tests/type_system_rules.rs::an_enumerator_value_outside_int32_is_no_type_object`

**Status:** `done` — refusal in the front end (`validate.rs`) and in the TypeObject builder (`from_ast.rs`), each with a test; both forms (`Minimal`, `Complete`) report `OutOfRange`.

### §7.2.2.4.1.1 Table 4 — The order of the literals is significant

**Spec:** §7.2.2.4.1.1, Table 4, p. 49 — "The order in which the literals of an
enumerated type are defined is significant to the definition of that type. For
example, some type representations may base the numeric values of the literals
on their order of definition."

**Repo:** `crates/ast/src/node/type_.rs::Enumeration` (l. 258, field `enumerators` in declaration order);
`crates/ast/src/query.rs::effective_enumerator_value` (l. 559)

**Tests:** `crates/idl-lower/tests/lowering.rs::an_enum_carries_its_enumerators_in_order`,
`crates/idl-lower/tests/completion.rs::enumerators_without_values_run_from_zero_upwards`

**Status:** done

### §7.2.2.4.1.2 Table 5 — BITMASK_TYPE: bound greater than 0 and at most 64

**Spec:** §7.2.2.4.1.2, Table 5, p. 50 — "A bitmask defines a bound—the maximum
number of bits in the set—and identifies by name certain bits within the set.
The bound must be greater than zero and no greater than 64."

**Repo:** `crates/idl-lower/src/complete.rs::check_bitmask_bounds` (l. 176)

**Tests:** `crates/idl-lower/tests/validation.rs::a_bit_bound_over_sixty_four_is_refused`,
`crates/idl-lower/tests/validation.rs::the_bit_forms_the_specification_admits_still_get_through`

**Status:** done

### §7.2.2.4.1.2 Bitmask: bits with name and index 0 to bound − 1

**Spec:** §7.2.2.4.1.2, p. 50–51 — "Each bit in this subset is identified by
name and by an index, numbered from 0 to (bound – 1). The bitmask need not
identify every bit it reserves. Furthermore, the bits it does identify need not
be contiguous."

**Repo:** `crates/idl-lower/src/complete.rs::check_bitmask_bounds` (l. 176)

**Tests:** `crates/idl-lower/tests/validation.rs::a_bit_position_outside_the_bitmasks_size_is_refused`,
`crates/idl-lower/tests/validation.rs::a_bitmask_with_more_values_than_its_size_is_refused`,
`crates/idl-lower/tests/completion.rs::bit_positions_run_from_zero_and_an_explicit_one_shifts_the_rest`

**Status:** done

### §7.2.2.4.1.2 Bitmask: no requirement on the storage form

**Spec:** §7.2.2.4.1.2, p. 51 — "It does not actually constrain such
representations to represent each 'bit' in the set as a single memory bit or to
align the bitmask in any particular way."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — note without a requirement; the data
representation is in §7.4.1.1.3.

### §7.2.2.4.1.2.1 Design Rationale (Non-Normative)

**Spec:** §7.2.2.4.1.2.1, p. 51 — rationale for bitmasks.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the heading says "Non-Normative".

---

## §7.2.2.4.2 Alias Types

### §7.2.2.4.2 Table 6 — ALIAS_TYPE: a second name for a type

**Spec:** §7.2.2.4.2, Table 6, p. 52 — "An alias type—also referred to as a
typedef [...]—applies an additional name to an already-existing type. [...] an
alias/typedef does not introduce a distinct type."

**Repo:** `crates/ast/src/node/type_.rs::Alias` (l. 275);
`crates/types/src/from_ast.rs::alias` (l. 2658);
`crates/types/src/resolve.rs::resolve_alias_chain`

**Tests:** `crates/idl-lower/tests/lowering.rs::a_typedef_becomes_an_alias_to_its_target`,
`crates/types/src/resolve.rs::tests::resolve_follows_single_alias_to_target`,
`crates/types/src/resolve.rs::tests::resolve_chained_aliases_two_deep`

**Status:** done — for assignability and keys an alias is resolved to its target
type; in the TypeObject it is a `TK_ALIAS` object of its own, as Annex B
provides.

### §7.2.2.4.2 Figure 14 — Alias: nested = base_type.nested

**Spec:** §7.2.2.4.2, Figure 14, p. 52 — constraint `{nested =
base_type.nested}`; annotations on the alias and on the base type
(`+base_annotation`).

**Repo:** `crates/types/src/from_ast.rs::alias` (l. 2658)

**Tests:** `crates/idl-lower/tests/annotations.rs::an_alias_supplies_a_default_that_the_use_site_overrides`

**Status:** done — an alias is not a topic type of a kind of its own; the
annotations on the base type (`@try_construct`) take effect through the alias.

---

## §7.2.2.4.3 Collection Types

### §7.2.2.4.3 Collection: element type at definition

**Spec:** §7.2.2.4.3, p. 53 — "The type of the element might be any other type,
primitive or constructed [...] and must be specified when the collection type is
defined." Figure 15: `{extensibility_kind = <Not Applicable>}`, `{nested =
true}`, `external: Boolean = false`.

**Repo:** `crates/ast/src/node/type_.rs::Sequence` (l. 219);
`crates/ast/src/node/type_.rs::Array` (l. 226);
`crates/ast/src/node/type_.rs::Map` (l. 233)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_sequence_member_carries_its_element_type`

**Status:** done

### §7.2.2.4.3 Table 7 — ARRAY_TYPE: fixed size, multi-dimensional

**Spec:** §7.2.2.4.3, Table 7, p. 54 — "Fixed-size multi-dimensional collection.
Arrays are of a fixed size in that all objects of a given array type will have
the same number of elements. [...] array types of higher dimensionality are
distinct from arrays of arrays of lower dimensionality."

**Repo:** `crates/ast/src/node/type_.rs::Array` (l. 226,
`dims`);
`crates/types/src/type_identifier/kinds.rs::TI_PLAIN_ARRAY_SMALL` (l. 77)

**Tests:** `crates/idl-lower/tests/completion.rs::a_multi_dimensional_array_keeps_its_dimensions_in_order`,
`crates/idl-lower/tests/lowering.rs::dimensions_belong_to_the_declarator_and_not_to_the_typedef`,
`crates/types/src/assignability.rs::tests::plain_array_diff_bounds_is_no`

**Status:** done — a multi-dimensional array carries all dimensions in one
`array_bounds`; an array of arrays through a `typedef` is a different
TypeIdentifier.

### §7.2.2.4.3 Table 7 — SEQUENCE_TYPE: variable length up to the bound

**Spec:** §7.2.2.4.3, Table 7, p. 54 — "Variable-size single-dimensional
collection. [...] the length of a given sequence object may change between zero
and the sequence type's 'bound' [...]. Elements are addressed by a single
index."

**Repo:** `crates/ast/src/node/type_.rs::Sequence` (l. 219);
`crates/types/src/type_identifier/kinds.rs::TI_PLAIN_SEQUENCE_SMALL` (l. 72)

**Tests:** `crates/idl-lower/tests/completion.rs::a_bounded_string_and_sequence_carry_their_bound_expressions`,
`crates/lang-rules/tests/codec.rs::a_bounded_sequence_checks_its_bound_and_an_unbounded_one_has_none`

**Status:** done

### §7.2.2.4.3 Table 7 — MAP_TYPE: key unique per map

**Spec:** §7.2.2.4.3, Table 7, p. 54 — "'Map value' elements are addressed by a
'map key' object, the value of which must be unique within a given map object.
The types of both of these are homogeneous within a given map type".

**Repo:** `crates/ast/src/node/type_.rs::Map` (l. 233);
`crates/wire/src/reader.rs::read_map` (l. 1764)

**Tests:** `crates/wire/tests/wire.rs::the_ordered_map_reader_refuses_a_stream_that_repeats_a_key`,
`crates/wire-conformance/tests/map_bytes.rs::a_stream_that_repeats_a_key_keeps_every_entry`

**Status:** done

### §7.2.2.4.3 Map: key types integer and string

**Spec:** §7.2.2.4.3, p. 55 — "Implementers of this specification need only
support key elements of signed and unsigned integer types and of narrow and wide
string types; the behavior of maps with other key element types is undefined and
may not be portable."

**Repo:** `crates/ast-layout/src/xcdr.rs::map_key_ordering` (l. 1586);
`crates/ast-layout/src/xcdr.rs::key_ordering` (l. 1605)

**Tests:** `crates/wire-conformance/tests/map_bytes.rs::the_recorded_row_round_trips_back_into_the_sample`

**Status:** done — integer and string keys are written and read. Keys of other
types are accepted; only under `MapEntryOrder::AscendingKey` does
`map_key_ordering` return no order for floating-point, `fixed` and constructed
keys, and the caller refuses.

### §7.2.2.4.3 Collection: bound greater than 0, unbounded for sequence, string, map

**Spec:** §7.2.2.4.3, p. 55 — "Bound: The maximum number of elements the
collection may contain (inclusively); it must be greater than zero. In the cases
of sequences, strings, and maps, the bound parameter may be omitted. [...] (All
arrays must be bounded.)"

**Repo:** `crates/idl-lower/src/validate.rs::bound_is_positive` (l. 842);
`crates/ast/src/node/type_.rs::Bound` (l. 132)

**Tests:** `crates/idl-lower/tests/validation.rs::a_zero_string_bound_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_negative_sequence_bound_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_zero_array_dimension_is_rejected`,
`crates/idl-lower/tests/completion.rs::an_unbounded_collection_says_so_rather_than_carrying_a_zero`

**Status:** done

### §7.2.2.4.3 Collection: overall bound of an array

**Spec:** §7.2.2.4.3, p. 55 — "Arrays have independent bounds on each of their
dimensions; they can also be said to have an overall bound, which is the product
of all of their dimensions' bounds."

**Repo:** `crates/ast/src/node/type_.rs::Array` (l. 226,
`dims`)

**Tests:** `crates/idl-lower/tests/completion.rs::a_multi_dimensional_array_keeps_its_dimensions_in_order`

**Status:** done

### §7.2.2.4.3 Collection: different types per kind, element type and bound

**Spec:** §7.2.2.4.3, p. 55 — "a one-dimensional array of 10 integers, a
one-dimensional array of 10 short integers, a sequence of at most 10 integers,
and a sequence of an unspecified number of integers are all of different types.
However, all one-dimensional arrays of 10 integers are of the same type."

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier`

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_array_small_3d_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done — an anonymous collection type is determined by its plain
TypeIdentifier from kind, bound and element type; equal values give the same
identifier.

---

## §7.2.2.4.4 Aggregated Types

### §7.2.2.4.4.1 Member: name, type, ID, index, boolean properties

**Spec:** §7.2.2.4.4.1, p. 55–56 — "Each member has a string name, a type, an
integer ID, an integer index, and a set of boolean properties". Figure 16: `id:
UInt32`, `key: Boolean = false`, `member_index: UInt32`, `must_understand:
Boolean = false`, `optional: Boolean = false`, `shared: Boolean = false`.

**Repo:** `crates/ast/src/node/member.rs::MemberNode` (l. 18);
`crates/ast/src/facet/xtypes.rs::XTypesFacet` (l. 242);
`crates/types/src/type_object/flags.rs::StructMemberFlag` (l. 39)

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`,
`crates/ast-layout/tests/layout.rs::must_understand_is_set_exactly_when_the_facet_says_so`

**Status:** done — `key`, `must_understand`, `optional` and `external` are
fields of the facet and bits in `StructMemberFlag`; the UML attribute `shared`
corresponds to `@external` (`IS_EXTERNAL`).

### §7.2.2.4.4.1 Table 8 — kinds of aggregates

**Spec:** §7.2.2.4.4.1, p. 56 — "There are three kinds of Aggregated Types:
structures, unions, and annotations." Figure 16 names `STRUCTURE_TYPE`,
`UNION_TYPE`, `BITSET_TYPE`; Table 8 names `UNION_TYPE` and `STRUCTURE_TYPE`.

**Repo:** `crates/types/src/type_object/kinds.rs::TK_STRUCTURE` (l. 26);
`crates/types/src/type_object/kinds.rs::TK_UNION` (l. 28);
`crates/types/src/type_object/kinds.rs::TK_ANNOTATION` (l. 23);
`crates/types/src/type_object/kinds.rs::TK_BITSET` (l. 30)

**Tests:** `crates/types/src/builder.rs::tests::struct_builder_basic_with_two_members`,
`crates/types/src/builder.rs::tests::union_builder_minimal_with_two_cases_and_default`,
`crates/types/tests/type_object_annotations.rs::an_annotation_declaration_has_a_type_object`,
`crates/types/src/builder.rs::tests::bitset_builder_two_fields`

**Status:** done — all four kinds have a TypeObject. The text, Figure 16 and
Table 8 name different sets of kinds (contradiction in the specification, listed
in the findings file; DDSXTY14-63 and DDSXTY15-5 on bitset, see
`docs/omg-xtypes-issues.md` §A2).

### §7.2.2.4.4.1 Table 8 — UNION_TYPE

**Spec:** §7.2.2.4.4.1, Table 8, p. 56 — "Discriminated exclusive aggregation of
members. Unions define a well-known discriminator member and a set of
type-specific members."

**Repo:** `crates/ast/src/node/type_.rs::Union` (l. 249);
`crates/types/src/from_ast.rs::union` (l. 2178)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_union_over_an_enumeration_lowers_with_its_labels_evaluated`,
`crates/types/src/builder.rs::tests::union_builder_minimal_with_two_cases_and_default`

**Status:** done

### §7.2.2.4.4.1 Table 8 — STRUCTURE_TYPE

**Spec:** §7.2.2.4.4.1, Table 8, p. 56 — "Non-exclusive aggregation of members.
A type designer may declare any number of members within a structure. Unlike in
a union, there are no implicit members in a structure, and values for multiple
members may coexist."

**Repo:** `crates/ast/src/node/type_.rs::Structure` (l. 242);
`crates/types/src/from_ast.rs::structure` (l. 2082)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_struct_carries_its_members_in_order`,
`crates/types/src/builder.rs::tests::struct_builder_basic_with_two_members`

**Status:** done

### §7.2.2.4.4.2 Structure: optional base type, collision of name or ID

**Spec:** §7.2.2.4.4.2, p. 57 — "A structure can optionally extend one other
structure, its 'base_type.' In the event that there is a name or ID collision
between a structure and its base type, the definition of the derived structure
is ill-formed."

**Repo:** `crates/ast/src/node/type_.rs::Structure` (l. 242, field `base`);
`crates/idl-lower/src/validate.rs::inheritance_rules` (l. 415)

**Tests:** `crates/idl-lower/tests/keys_and_inheritance.rs::a_member_name_of_the_base_chain_cannot_be_declared_again`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_member_id_of_the_base_chain_cannot_be_given_again`

**Status:** done

### §7.2.2.4.4.3 Union: name "discriminator" reserved for members

**Spec:** §7.2.2.4.4.3, p. 57 — "The name of the discriminator member is always
'discriminator'; that name is reserved for union types and is not permitted for
type-specific union members."

**Repo:** `crates/idl-lower/src/validate.rs::union_member_rules`

**Tests:** `crates/idl-lower/tests/validation.rs::a_union_member_is_not_named_discriminator`

**Status:** done — `validate.rs::union_member_rules` refuses a union member named `discriminator`. Figure 18 names the discriminator `disc` (contradiction in the specification, kept in the findings file).

### §7.2.2.4.4.3 Union: the discriminator is the first member

**Spec:** §7.2.2.4.4.3, p. 57 — "The discriminator member is always considered
to be the first member of a union."

**Repo:** `crates/types/src/from_ast.rs::union` (l. 2178, branches numbered from 1)

**Tests:** `crates/types/src/builder.rs::tests::union_builder_minimal_with_two_cases_and_default`

**Status:** done — the branches get the member IDs from 1 on, the discriminator
counts as member 0.

### §7.2.2.4.4.3 Union: no label for two members

**Spec:** §7.2.2.4.4.3, p. 58 — "it is not allowed for multiple members to
explicitly identify the same discriminator value."

**Repo:** `crates/idl-lower/src/validate.rs::union_rules` (l. 185)

**Tests:** `crates/idl-lower/tests/validation.rs::a_repeated_case_label_is_rejected`,
`crates/idl-lower/tests/validation.rs::distinct_case_labels_are_accepted`

**Status:** done

### §7.2.2.4.4.3 Union: at most one default member

**Spec:** §7.2.2.4.4.3, p. 58 — "At most one member of the union may be
identified as the 'default' member; any discriminator value that does not
explicitly identify another member is considered to identify the default
member."

**Repo:** `crates/ast/src/invariant.rs::MultipleDefaultCases` (l. 84);
`crates/ast/src/node/type_.rs::Union` (l. 249,
`default_case`)

**Tests:** `crates/idl-lower/tests/validation.rs::two_default_cases_are_rejected`,
`crates/lang-rules/src/default_python.rs::tests::a_unions_default_case_is_what_an_unlisted_discriminator_selects`

**Status:** done

### §7.2.2.4.4.3 Union: only the selected member exists

**Spec:** §7.2.2.4.4.3, p. 58 — "The value of the member associated with the
current value of the discriminator is the only member value considered to exist
in a given object of a union type at a given moment in time. [...] the initial
value of the newly observed member is undefined by the type system".

**Repo:** `crates/lang-rules/src/rust_union.rs`

**Tests:** `crates/lang-rules/tests/a_decoded_union_has_only_its_selected_case.rs::every_case_is_cleared_before_the_selected_one_is_read`

**Status:** done — a union is generated in Rust as an `enum` with one variant
per branch; a decoded value holds only the selected branch.

### §7.2.2.4.4.3 Union: permitted discriminator types

**Spec:** §7.2.2.4.4.3, p. 58–59 — "The discriminator of a union must be of one
of the following types: Boolean, Byte, Char8, Char16, Int8, UInt8, Int16,
UInt16, Int32, UInt32, Int64, UInt64, Any enumerated type, Any alias type that
resolves, directly or indirectly, to one of the aforementioned types."

**Repo:** `crates/idl-lower/src/validate.rs::is_legal_discriminator` (l. 244)

**Tests:** `crates/idl-lower/tests/validation.rs::each_legal_discriminator_type_is_accepted`,
`crates/idl-lower/tests/validation.rs::an_enum_discriminator_is_accepted`,
`crates/idl-lower/tests/validation.rs::a_typedef_to_a_legal_discriminator_is_accepted`,
`crates/idl-lower/tests/validation.rs::a_float_discriminator_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_string_discriminator_is_rejected`

**Status:** done — "Any enumerated type" includes bitmask by §7.2.2.4.1;
`is_legal_discriminator` accepts only `enum`. A bitmask as discriminator is
refused (IDL 4.2 does not admit it).

### §7.2.2.4.4.4.1 Member name: unique in the aggregate

**Spec:** §7.2.2.4.4.4.1, p. 59 — "Each member of an Aggregated type is
identified by a string name. The name shall be unique within the scope of the
Aggregated type".

**Repo:** `crates/ast/src/invariant.rs::DuplicateMemberName` (l. 56)

**Tests:** `crates/idl-lower/tests/validation.rs::two_members_of_one_aggregate_differing_only_by_case_collide`,
`crates/idl-lower/tests/lowering.rs::two_names_differing_only_by_case_collide`

**Status:** done

### §7.2.2.4.4.4.2 Member type

**Spec:** §7.2.2.4.4.4.2, p. 59 — "Each member of an Aggregated type has an
associated member type."

**Repo:** `crates/ast/src/node/member.rs::MemberNode` (l. 18, field `type_`)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_sequence_member_carries_its_element_type`

**Status:** done

### §7.2.2.4.4.4.3 Member index: from 0, without gaps

**Spec:** §7.2.2.4.4.4.3, p. 59 — "The member index indicates the declaration
order of the member within the Aggegated type. [...] Member indices shall start
at zero and increment by one with the highest index being equal to the number of
members minus one."

**Repo:** `crates/ast/src/node/type_.rs::Structure` (l. 242,
`members` in declaration order);
`crates/ast/src/query.rs::all_members_of` (l. 151)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_struct_carries_its_members_in_order`,
`crates/types/src/assignability.rs::tests::flatten_inheritance_two_levels_concatenates_base_first`

**Status:** done — the index is the position in the member vector; for a derived
structure the members of the base count first.

### §7.2.2.4.4.4.4 Member IDs: 28-bit range

**Spec:** §7.2.2.4.4.4.4, p. 59 — "Member IDs are unsigned and have a range that
can be represented in 28 bits: from zero to 268,435,455 (0x0FFFFFFF)."

**Repo:** `crates/idl-lower/src/complete.rs::hash_id`,
`crates/idl-lower/src/annotation.rs::id_annotation`,
`crates/wire/src/writer.rs::write_emheader`

**Tests:** `crates/idl-lower/tests/validation.rs::a_member_id_is_at_most_28_bits`,
`crates/idl-lower/tests/completion.rs::a_hashed_id_stays_within_the_28_bit_member_id_space`,
`crates/idl-lower/src/complete.rs::tests::a_hashed_id_stays_inside_the_28_bit_space`,
`crates/wire/tests/wire.rs::an_emheader_member_id_past_28_bits_is_refused`

**Status:** `done` — `id_annotation` refuses an id above 0x0FFFFFFF (`MAX_MEMBER_ID`), hashed ids are masked to 28 bits, `write_emheader` refuses an id above 0x0FFFFFFF when writing.

### §7.2.2.4.4.4.4 Member IDs: @id above 0x0FFFFFFF is refused

**Spec:** §7.2.2.4.4.4.4, p. 59 — 28-bit range of the member IDs.

**Repo:** `crates/idl-lower/src/annotation.rs::id_annotation`

**Tests:** `crates/idl-lower/tests/validation.rs::a_member_id_is_at_most_28_bits`

**Status:** done — The front end refuses `@id` above `0x0FFFFFFF` (28 bits, Annex B `MemberId`).

### §7.2.2.4.4.4.4 Member IDs: range for applications

**Spec:** §7.2.2.4.4.4.4, p. 59 — "The remaining part of the member ID
range—from 0 to 268,402,687 (0x0FFFBFFF)—is available for use by
application-defined types compliant with this specification."

**Repo:** `crates/idl-lower/src/annotation.rs::id_annotation`

**Tests:** `crates/idl-lower/tests/validation.rs::an_application_member_id_in_the_available_range_is_accepted`

**Status:** `done` — every id from 0 to 0x0FFFBFFF is accepted. The sentence names what is available to applications and does not forbid the rest; 0x0FFFC000 to 0x0FFFFFFF are accepted as well because they lie within the 28 bits and the vendor corpus writes `@id(268435455)`. The sentence refers to a part no text delimits, and gives 268,402,687 in decimal for 0x0FFFBFFF in hexadecimal (DDSXTY14-8, `docs/omg-xtypes-issues.md` §A4).

### §7.2.2.4.4.4.5 Member name hashes: first 4 octets of MD5(UTF-8)

**Spec:** §7.2.2.4.4.4.5, p. 59 — "the 4-byte hashes computed from strings shall
consist of the first 4 bytes of the MD5 hash of the UTF-8 encoding of the
string, without any 'nul' terminator. For example, computing the 4-byte Hash of
the member name 'color' shall result on the 4 octets {0x70, 0xDD, 0xA5, 0xDF}."

**Repo:** `crates/types/src/type_object/common.rs::from_name` (l. 34);
`crates/idl-lower/src/complete.rs::hash_id` (l. 1165);
`crates/wire/src/digest.rs::md5`

**Tests:** `crates/types/src/builder.rs::tests::member_id_from_name_matches_known_md5_vectors`,
`crates/idl-lower/src/complete.rs::tests::the_digest_matches_the_published_vectors`,
`crates/types/tests/compliance_typeobject.rs::md5_empty_string_first_14_bytes_golden_vector`

**Status:** done — the test checks "color" as the masked ID `0x0FA5DD70`, i.e.
the octets `70 DD A5` and the masked fourth octet.

### §7.2.2.4.4.4.6 Must understand: discarding the sample for an unknown member

**Spec:** §7.2.2.4.4.4.6, p. 60 — "If the attribute is true, a data consumer,
upon identifying a member it does not recognize, must discard the entire data
sample to which the member belongs. If the attribute is false, the consumer is
permitted to process the sample, omitting the value of the unrecognized member."

**Repo:** `crates/wire-codec/src/decode.rs` (XCDR2 l. 1485–1498, XCDR1 l. 1573–1589);
`crates/wire/src/reader.rs::read_emheader` (l. 1265)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_unknown_mutable_member_is_skipped_and_a_must_understand_one_is_not`,
`crates/wire-probe/tests/decode_round_trip.rs::an_unknown_must_understand_parameter_discards_the_sample_under_xcdr1`

**Status:** done — the detection per data representation is under §7.4.

### §7.2.2.4.4.4.6 Must understand: structure members independent

**Spec:** §7.2.2.4.4.4.6, p. 60 — "In a structure type, each member may have the
'must understand' attribute set to true or false independently."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `must_understand` l. 337);
`crates/types/src/from_ast.rs::member_bits` (l. 1633)

**Tests:** `crates/ast-layout/tests/layout.rs::must_understand_is_set_exactly_when_the_facet_says_so`

**Status:** done

### §7.2.2.4.4.4.6 Must understand: union discriminator always true

**Spec:** §7.2.2.4.4.4.6, p. 60 — "In a union type, the discriminator member
shall always have the 'must understand' attribute set to true."

**Repo:** `crates/types/src/from_ast.rs::union`,
`crates/ast-layout/src/xcdr.rs::plan_at (Diskriminator)`,
`crates/ast-layout/src/key.rs::MustUnderstandPolicy::DeclaredKeyAndDiscriminator`

**Tests:** `crates/types/tests/type_system_rules.rs::the_discriminator_of_a_union_is_must_understand_in_both_forms`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_discriminator_of_a_mutable_union_is_must_understand`,
`crates/ast-layout/tests/vendor_gate.rs::each_dialect_holds_what_its_implementation_was_measured_writing`

**Status:** `done` — the TypeObject sets `IS_MUST_UNDERSTAND` on the discriminator (minimal and complete). On the wire `Dialect::Specification` (`MustUnderstandPolicy::DeclaredKeyAndDiscriminator`) sets `M_FLAG` (XCDR2) and `FLAG_MUST_UNDERSTAND` (XCDR1) on the member header of the discriminator of a `@mutable` union. The dialects `Rti` and `OpenDds` leave the bit clear, as measured (`union-mutable`, `vendor-corpus.tsv`); the reader takes both forms.

### §7.2.2.4.4.4.6 Must understand: no test of the discriminator bit

**Spec:** §7.2.2.4.4.4.6, p. 60 — Diskriminator immer must understand.

**Repo:** `crates/types/src/from_ast.rs::union`,
`crates/ast-layout/src/xcdr.rs::plan_at (Diskriminator)`

**Tests:** `crates/types/tests/type_system_rules.rs::the_discriminator_of_a_union_is_must_understand_in_both_forms`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_discriminator_of_a_mutable_union_is_must_understand`

**Status:** `done` — both tests ask for the bit: `IS_MUST_UNDERSTAND` in the minimal and complete TypeObject, `0xA0000000` (XCDR2) and `0x4000` (XCDR1) in the member header; `Rti` writes `0x20000000`.

### §7.2.2.4.4.4.6 Must understand: detection per data representation

**Spec:** §7.2.2.4.4.4.6, p. 60 — "The ability of a consumer to detect the
presence of an unrecognized member depends on the Data Representation. Each
representation shall therefore define the means by which such detection occurs."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — requirement on the data representations of the
specification; the means (EMHEADER M flag, parameter header) are listed under
§7.4.

### §7.2.2.4.4.4.7 Optional: every object has a value for each non-optional member

**Spec:** §7.2.2.4.4.4.7, p. 60 — "Every object of a given type shall be
considered to contain a value for every non-optional member defined by that
type. In the event that no explicit value for such a member is ever provided in
a Data Representation of that object, that member is considered to nevertheless
have the default 'zero' value defined in Table 9 below."

**Repo:** `crates/wire-codec/src/default.rs::member` (l. 100);
`crates/lang-rules/src/default_rust.rs::member_default` (l. 48)

**Tests:** `crates/lang-rules/tests/codec.rs::an_appendable_tail_the_writer_did_not_have_takes_its_default`,
`crates/lang-rules/tests/c_end_to_end.rs::a_mutable_member_that_did_not_arrive_takes_its_default`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — BYTE: 0x00

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 60 — "BYTE 0x00".

**Repo:** `crates/wire-codec/src/default.rs::primitive` (l. 237);
`crates/lang-rules/src/default_rust.rs::primitive_default` (l. 349)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/tests/c_end_to_end.rs::every_table_nine_row_has_a_c_default`,
`crates/lang-rules/tests/cpp_reader_generates.rs::every_kind_table_9_answers_for_gets_a_defaulter`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — BOOLEAN: False

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 60 — "BOOLEAN False".

**Repo:** `crates/wire-codec/src/default.rs::primitive` (l. 237);
`crates/lang-rules/src/default_rust.rs::primitive_default` (l. 349)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/tests/c_end_to_end.rs::every_table_nine_row_has_a_c_default`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — integer and floating-point types: 0

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 60 — "INT_8_TYPE, UINT_8_TYPE,
INT_16_TYPE, UINT_16_TYPE, INT_32_TYPE, UINT_32_TYPE, INT_64_TYPE, UINT_64_TYPE,
FLOAT_32_TYPE, FLOAT_64_TYPE, FLOAT_128_TYPE 0".

**Repo:** `crates/wire-codec/src/default.rs::primitive` (l. 237);
`crates/lang-rules/src/default_rust.rs::primitive_default` (l. 349)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/tests/c_end_to_end.rs::every_table_nine_row_has_a_c_default`,
`crates/lang-rules/src/default_python.rs::tests::a_long_double_is_a_float128_and_never_a_float`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — CHAR_8_TYPE, CHAR_16_TYPE: '\0'

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 61 — "CHAR_8_TYPE, CHAR_16_TYPE '\0'".

**Repo:** `crates/wire-codec/src/default.rs::primitive` (l. 237);
`crates/lang-rules/src/default_rust.rs::primitive_default` (l. 349)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_char_is_a_one_character_string_and_not_an_integer`,
`crates/lang-rules/tests/cpp_reader_generates.rs::every_kind_table_9_answers_for_gets_a_defaulter`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — STRING_TYPE: ""

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 61 — "STRING_TYPE \"\"".

**Repo:** `crates/wire-codec/src/default.rs::at_depth` (l. 152, arm `TypeKind::String`)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/tests/codec.rs::an_appendable_tail_the_writer_did_not_have_takes_its_default`,
`crates/lang-rules/tests/c_end_to_end.rs::every_table_nine_row_has_a_c_default`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — ARRAY_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 61 — "An array of the same dimensions and
same element type whose elements take the default value for their corresponding
type."

**Repo:** `crates/wire-codec/src/default.rs::array` (l. 304)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_fixed_size_array_is_that_many_elements`,
`crates/lang-rules/src/default_python.rs::tests::a_two_dimensional_array_is_one_tuple_per_dimension`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — ALIAS_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 61 — "The default type of the alias's base
type."

**Repo:** `crates/wire-codec/src/default.rs::at_depth` (l. 152, arm `TypeKind::Alias`);
`crates/lang-rules/src/default_rust.rs::resolve_alias` (l. 516)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_typedef_and_its_target_are_one_function_and_it_does_not_call_itself`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — SEQUENCE_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 61 — "A zero-length sequence of the same
element type."

**Repo:** `crates/wire-codec/src/default.rs::at_depth` (l. 152, arm `TypeKind::Sequence`)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_sequence_is_an_empty_list_at_every_bound`,
`crates/lang-rules/tests/c_end_to_end.rs::every_table_nine_row_has_a_c_default`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — MAP_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 61 — "An empty map of the same element
type."

**Repo:** `crates/wire-codec/src/default.rs::at_depth` (l. 152, arm `TypeKind::Map` l. 202)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — ENUM_TYPE: first literal

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 61 — "The first literal in the
enumeration."

**Repo:** `crates/wire-codec/src/default.rs::first_enumerator` (l. 270);
`crates/lang-rules/src/default_rust.rs::default_enumerator` (l. 370)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/tests/cpp_reader_generates.rs::an_enumeration_whose_first_literal_is_not_zero`,
`crates/lang-rules/src/default_python.rs::tests::an_enumeration_takes_its_first_literal_by_name`

**Status:** done — with `@default_literal` the literal so marked applies
(`an_enumeration_with_a_default_literal_takes_that_one`); Table 9 and
§7.3.1.2.1.12.5 contradict each other here (`docs/omg-xtypes-issues.md` §B5).

### §7.2.2.4.4.4.7 Table 9 — UNION_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 61 — "A union with the discriminator set
to the default value for the discriminator type. If this selects a branch, then
the selected member is also set to the default value for the member type.
Otherwise the value of the union is fully specified by the discriminator value."

**Repo:** `crates/wire-codec/src/default.rs::union` (l. 366);
`crates/lang-rules/src/default_rust.rs::discriminator_default_value` (l. 393)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_union_whose_discriminators_default_selects_a_case_sets_both`,
`crates/lang-rules/src/default_python.rs::tests::a_union_whose_discriminators_default_selects_nothing_is_the_discriminator_alone`,
`crates/lang-rules/tests/c_end_to_end.rs::a_union_member_the_stream_does_not_carry_takes_its_default`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — STRUCTURE_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 61 — "A structure without any of the
optional members and with other members set to their default values based on
their corresponding types."

**Repo:** `crates/wire-codec/src/default.rs::structure` (l. 330)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_structure_of_primitives_names_every_required_field`,
`crates/lang-rules/src/default_python.rs::tests::an_optional_member_is_absent_from_the_constructor_call`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — Bitmask and bitset without a row

**Spec:** §7.2.2.4.4.4.7, Table 9, p. 60–61 — Table 9 has no row for
`BITMASK_TYPE` and `BITSET_TYPE`.

**Repo:** `crates/wire-codec/src/default.rs::at_depth` (l. 152, arms `TypeKind::Bitmask`,
`TypeKind::Bitset`)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/wire-conformance/tests/bitmask_bytes.rs::table_9_gives_a_bitmask_no_bit_and_a_bitset_no_field`,
`crates/lang-rules/src/default_python.rs::tests::a_bitset_is_the_dataclass_with_every_field_at_zero`

**Status:** done — the tree takes "no bit set" and "every field 0". The missing
row is a gap in the specification (DDSXTY14-60, listed in the findings file).

### §7.2.2.4.4.4.7 Optional: omitted value without a default

**Spec:** §7.2.2.4.4.4.7, p. 61 — "An object may omit a value for any optional
member(s) defined by its type. [...] Implementations shall not provide a default
value in such a case."

**Repo:** `crates/lang-rules/src/default_rust.rs::member_default` (l. 48)

**Tests:** `crates/lang-rules/src/default_python.rs::tests::an_optional_member_is_absent_from_the_constructor_call`

**Status:** done — an `@optional` member becomes `None`, not Table 9.

### §7.2.2.4.4.4.7 Optional: union members never optional

**Spec:** §7.2.2.4.4.4.7, p. 61 — "Union members, including the discriminator,
shall never be optional."

**Repo:** `crates/idl-lower/src/validate.rs::union_member_rules` (l. 768)

**Tests:** `crates/idl-lower/tests/validation.rs::no_union_member_is_optional`

**Status:** done — an `@optional` on the discriminator is not a permitted
use-site annotation position and is reported by `apply_use_site`.

### §7.2.2.4.4.4.7 Optional: structure members chosen per member

**Spec:** §7.2.2.4.4.4.7, p. 61 — "Structure members may be optional. The
designer of a structure can choose which members are optional on a
member-by-member basis."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `optional` l. 335);
`crates/types/src/from_ast.rs::member_bits` (l. 1633)

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`

**Status:** done

### §7.2.2.4.4.4.7 Optional and must understand independent

**Spec:** §7.2.2.4.4.4.7, p. 61 — "The value of a member's 'optional' attribute
is unrelated to the value of its 'must understand' attribute."

**Repo:** `crates/types/src/from_ast.rs::member_bits` (l. 1633)

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`

**Status:** done

### §7.2.2.4.4.4.8 Key members: the key of the type is the key of the topic

**Spec:** §7.2.2.4.4.4.8, p. 61 — "The type's key will become the key of any DDS
Topic that is constructed using the aforementioned aggregated type as the
Topic's type. If a given type has no members designated as key members, then the
type [...] has no key."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_members` (l. 254);
`crates/zerodds-build/src/dds_type.rs`

**Tests:** `crates/ast-layout/tests/vendor_gate.rs::a_keyless_topic_type_still_reports_no_key_members`,
`crates/ast-layout/tests/wire_vectors.rs::a_type_with_no_key_member_has_no_key_members_and_a_whole_type_holder`

**Status:** done

### §7.2.2.4.4.4.8 Key members: never optional

**Spec:** §7.2.2.4.4.4.8, p. 61 — "Key members shall never be optional".

**Repo:** `crates/idl-lower/src/validate.rs::member_rules` (l. 327, check l. 348)

**Tests:** `crates/idl-lower/tests/validation.rs::a_key_member_must_not_be_optional`

**Status:** done

### §7.2.2.4.4.4.8 Key members: must understand always true

**Spec:** §7.2.2.4.4.4.8, p. 61 — "[...] and they shall always have their 'must
understand' attribute set to true."

**Repo:** `crates/types/src/from_ast.rs::member_bits`,
`crates/types/src/assignability.rs::must_not_be_missing`,
`crates/ast-layout/src/xcdr.rs::member_must_understand`

**Tests:** `crates/types/tests/type_system_rules.rs::a_key_member_is_must_understand_in_both_forms`,
`crates/types/tests/table19_key_members.rs::a_writer_enum_key_literal_the_reader_lacks_is_not_assignable`,
`crates/wire-probe/tests/opendds_mutable_framing.rs::the_key_rows_are_reproduced`,
`crates/wire-probe/tests/opendds_mutable_framing.rs::the_clause_reaches_only_the_specification_dialect`

**Status:** `done` — the TypeObject sets `IS_MUST_UNDERSTAND` on every key member (minimal and complete), the assignability treats a key member like a member with must understand. On the wire the default `Dialect::Specification` sets `M_FLAG` on key members as well; `Cyclone`, `Rti` and `FastDds` do not, as measured (`docs/omg-xtypes-issues.md` §B3).

### §7.2.2.4.4.4.8 Key members: no test of the bit in the TypeObject

**Spec:** §7.2.2.4.4.4.8, p. 61 — must understand an Key-Membern.

**Repo:** `crates/types/src/from_ast.rs::member_bits`

**Tests:** `crates/types/tests/type_system_rules.rs::a_key_member_is_must_understand_in_both_forms`

**Status:** `done` — the test asks for `IS_KEY` and `IS_MUST_UNDERSTAND` on key and non-key members in the minimal and complete TypeObject. A comparison with the TypeObjects of other vendors (same `EquivalenceHash`) is not measured.

### §7.2.2.4.4.4.8 Key members: permitted types, no map

**Spec:** §7.2.2.4.4.4.8, p. 62 — "A type's key can only include members of the
following types: primitive, aggregation, enumeration, bitmask, array, and
sequence. Aliases to one of the previous types can also be used as key members.
Members of type map cannot be included as part of the key."

**Repo:** `crates/idl-lower/src/validate.rs::key_member_type_rules` (l. 683);
`crates/idl-lower/src/validate.rs::map_in_key` (l. 714)

**Tests:** `crates/idl-lower/tests/validation.rs::a_map_cannot_be_part_of_a_key`,
`crates/idl-lower/tests/validation.rs::a_map_that_is_not_in_the_key_is_accepted`,
`crates/idl-lower/tests/validation.rs::a_string_key_is_accepted_although_the_clause_does_not_list_it`

**Status:** done — `string` is missing from the list of the specification but is
the key of its own example (§7.6.8) and of the ShapeType; the tree accepts it
(contradiction in the specification, listed in the findings file).

### §7.2.2.4.4.4.8 Key members: structure — any members

**Spec:** §7.2.2.4.4.4.8, p. 62 — "In a structure type, the key designation can
be applied to any member and to any number of members."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `key` l. 334)

**Tests:** `crates/idl-lower/tests/validation.rs::key_on_its_own_is_accepted`,
`crates/ast-layout/tests/wire_vectors.rs::key_members_come_back_in_member_id_order_not_declaration_order`

**Status:** done

### §7.2.2.4.4.4.8 Key members: union — only the discriminator

**Spec:** §7.2.2.4.4.4.8, p. 62 — "In a union type, only the discriminator is
permitted to be a key member. The union discriminator is marked as a key by
annotating the discriminator itself with the @key annotation".

**Repo:** `crates/idl-lower/src/validate.rs::union_member_rules`,
`crates/idl-lower/src/lower.rs::KeyOnUnionDiscriminator`,
`crates/ast-layout/src/keyhash.rs::union_key_discriminator`

**Tests:** `crates/idl-lower/tests/validation.rs::only_a_unions_discriminator_may_be_a_key`,
`crates/idl-lower/tests/annotations.rs::a_key_on_a_union_discriminator_is_carried_by_default`,
`crates/idl-lower/tests/annotations.rs::a_key_on_a_union_discriminator_reaches_the_facet_when_carried`,
`crates/types/tests/table19_key_members.rs::one_keyed_union_discriminator_and_one_unkeyed_are_not_assignable`

**Status:** `done` — `@key` on a branch is refused. `@key` on the discriminator is carried by default (`KeyOnUnionDiscriminator::Carried` is the default); `Refused` is the named setting for tools that have to follow the measured vendors. `zerodds-build` sets `Refused` for the generated Rust code because the union codec there generates no key holder (language binding, not this package).

### §7.2.2.4.4.4.8 Key members: @key on the discriminator is carried

**Spec:** §7.2.2.4.4.4.8, p. 62 — `union MyCommand switch (@key CommandKind)`.

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::a_key_on_a_union_discriminator_is_carried_by_default`,
`crates/xml-compose/tests/key_on_union_discriminator.rs::the_default_carries_the_declaration`,
`crates/xml-compose/tests/loader.rs::a_key_on_a_union_discriminator_is_refused_under_the_refusing_setting_and_says_why`

**Status:** done — `KeyOnUnionDiscriminator::default()` is `Carried`: `@key` on the discriminator reaches the model (`zerodds-idlc` and `zerodds-xml-compose`). `Refused` stays as a named setting for vendors that refuse the declaration.

### §7.2.2.4.4.4.8 Key members: array and sequence, length ahead of the elements

**Spec:** §7.2.2.4.4.4.8, p. 62 — "If a member of type array or sequence is
marked as a key member of an aggregated type T, all the elements in the array or
sequence shall be considered part of the key of T. In the case of a sequence,
the length of the sequence is also considered as part of the key ahead of the
sequence elements."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_holder_plan` (l. 41)

**Tests:** `crates/ast-layout/tests/key_maximum.rs::sequence_and_map_bounds_include_prefix_and_each_entry`,
`crates/ast-layout/tests/key_maximum.rs::nested_keys_and_arrays_do_not_double_count_or_add_trailing_padding`

**Status:** done — the KeyHolder serializes the whole member including the
length of the sequence.

### §7.2.2.4.4.4.8 Key members: key of a nested type

**Spec:** §7.2.2.4.4.4.8, p. 62 — "In the event that the type K of a key member
of a given type T itself defines key members, only the key of K, and not any
other of its members, shall be considered part of the key of T. This
relationship is recursive".

**Repo:** `crates/ast-layout/src/keyhash.rs::member_key` (l. 161);
`crates/ast-layout/src/keyhash.rs::holder_keeps_only_keys` (l. 133)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::nested_keyed_members_contribute_their_own_keys`,
`crates/ast-layout/tests/vendor_gate.rs::a_key_member_of_a_keyless_type_contributes_all_of_its_members`,
`crates/ast-layout/tests/vendor_gate.rs::step_3_recursion_is_unlimited_except_in_rtis_xcdr1_holder`

**Status:** done

### §7.2.2.4.4.4.9 Default member value: explicit or implicit

**Spec:** §7.2.2.4.4.4.9, p. 63 — "The default value for each member may be
defined explicitly or implicitly: An explicit definition associates a default
value with the member at the time where the aggregated type is defined. [...]
The implicit default value for a member is defined to match the default value of
the member type. See Table 9."

**Repo:** `crates/wire-codec/src/default.rs::member` (l. 100);
`crates/lang-rules/src/default_rust.rs::member_default` (l. 48)

**Tests:** `crates/lang-rules/tests/codec.rs::an_appendable_tail_takes_an_explicit_default_over_table_nine`,
`crates/lang-rules/tests/codec.rs::an_explicit_default_beats_table_nine_at_the_member_position`,
`crates/wire-probe/tests/evolution.rs::an_explicit_default_beats_table_nine`

**Status:** done

### §7.2.2.4.4.4.9 Default member value: no explicit default with @optional

**Spec:** §7.2.2.4.4.4.9, p. 63 — "Optional members are not allowed to associate
an explicit default value for the member."

**Repo:** `crates/wire-codec/src/default.rs::member_at_depth` (l. 110, check l. 123);
`crates/types/src/dynamic/builder.rs`

**Tests:** `crates/types/src/dynamic/builder.rs::tests::optional_with_a_default_is_refused`,
`crates/types/src/dynamic/builder.rs::tests::optional_with_a_default_is_refused_on_a_union_case`,
`crates/idl-lower/tests/validation.rs::an_optional_member_has_no_default`

**Status:** done — The `DynamicTypeBuilder` and the reflective decoder refuse `@optional` with `@default`; the IDL front end does so too (`validate.rs::member_rules`).

### §7.2.2.4.4.4.9 Default member value: the IDL front end refuses @optional @default

**Spec:** §7.2.2.4.4.4.9, p. 63 — no explicit default for optional members.

**Repo:** `crates/idl-lower/src/validate.rs::member_rules`

**Tests:** `crates/idl-lower/tests/validation.rs::an_optional_member_has_no_default`

**Status:** done — `struct S { @optional @default(3) long a; };` is refused by the front end.

---

## §7.2.2.4.5 Inheritance of Aggregated Types

### §7.2.2.4.5 Single inheritance

**Spec:** §7.2.2.4.5, p. 63 — "The Type System supports single inheritance of
Aggegated Types."

**Repo:** `crates/ast/src/node/type_.rs::Structure` (l. 242,
`base: Option<Ref<TypeNode>>`);
`crates/types/src/assignability.rs::flatten_inheritance` (l. 58)

**Tests:** `crates/types/src/assignability.rs::tests::flatten_inheritance_two_levels_concatenates_base_first`,
`crates/types/src/assignability.rs::tests::two_level_inheritance_assignability_chain`

**Status:** done — for structures; unions see below.

### §7.2.2.4.5 Structure: extensibility unspecified or equal to the base

**Spec:** §7.2.2.4.5, p. 63 — "The derived structure either has the
extensibility kind unspecified, or else, it has the same extensibility kind as
the base type."

**Repo:** `crates/idl-lower/src/validate.rs::inheritance_rules` (l. 415, check l. 431–445);
`crates/ast/src/query.rs::effective_extensibility` (l. 290)

**Tests:** `crates/idl-lower/tests/keys_and_inheritance.rs::a_derived_structure_with_another_extensibility_than_its_base_is_refused`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_derived_structure_that_repeats_its_bases_extensibility_is_accepted`,
`crates/idl-lower/tests/completion.rs::a_derived_structure_that_said_nothing_has_its_bases_extensibility`

**Status:** done

### §7.2.2.4.5 Structure: names and member IDs unique across the chain

**Spec:** §7.2.2.4.5, p. 63 — "The derived type does not have a member with the
same name or with the same memberId as the base type or any of the base
structure ancestor types. [...] The members of the hypothetical structure must
have unique names and member IDs."

**Repo:** `crates/idl-lower/src/validate.rs::inheritance_rules` (l. 415, checks l. 473 and l. 483);
`crates/types/src/assignability.rs::flatten_inheritance` (l. 58)

**Tests:** `crates/idl-lower/tests/keys_and_inheritance.rs::a_member_name_of_the_base_chain_cannot_be_declared_again`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_member_id_of_the_base_chain_cannot_be_given_again`,
`crates/types/src/assignability.rs::tests::inheritance_conflict_same_id`,
`crates/types/src/assignability.rs::tests::inheritance_conflict_same_name`

**Status:** done

### §7.2.2.4.5 Structure: no key fields in the derived type

**Spec:** §7.2.2.4.5, p. 63 — "The derived type does not define any key fields.
This ensures the key fields of the derived type are the same as those of the
base root type."

**Repo:** `crates/idl-lower/src/validate.rs::inheritance_rules` (l. 415, check l. 499)

**Tests:** `crates/idl-lower/tests/keys_and_inheritance.rs::a_key_written_in_a_derived_structure_is_refused`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_keylist_that_gives_a_derived_structure_its_own_key_is_refused`

**Status:** done

### §7.2.2.4.5 Union inheritance: conditions

**Spec:** §7.2.2.4.5, p. 63–64 — "A Union Type may inherit from another Union
Type as long as the following conditions are met: The derived Union either has
the extensibility kind unspecified, or else it has the same extensibility kind
as the base type. The derived Union has the same discriminator type as the base
Union. The derived Union discriminator does is not defined as key. The base
Union, or any of its ancestors have not explicitly defined a 'default' member of
the Union. Any given discriminator value identifies at most one member of the
derived Union or any of the base Union ancestor types."

**Repo:** `crates/types/src/union_inheritance.rs::check_union_derivation`,
`crates/types/src/union_inheritance.rs::expand_union`

**Tests:** `crates/types/src/union_inheritance.rs::tests::a_derived_union_that_meets_the_five_conditions_is_accepted`,
`crates/types/src/union_inheritance.rs::tests::a_derived_union_of_another_extensibility_kind_is_refused`,
`crates/types/src/union_inheritance.rs::tests::a_derived_union_with_another_discriminator_type_is_refused`,
`crates/types/src/union_inheritance.rs::tests::the_discriminator_of_a_derived_union_is_not_a_key`,
`crates/types/src/union_inheritance.rs::tests::no_ancestor_defines_a_default_member`,
`crates/types/src/union_inheritance.rs::tests::a_discriminator_value_selects_at_most_one_member_of_the_hypothetical_union`,
`crates/types/src/union_inheritance.rs::tests::the_hypothetical_union_has_unique_member_ids_and_names`,
`crates/types/src/union_inheritance.rs::tests::the_members_of_the_ancestors_are_expanded_ahead_of_the_derived_ones`

**Status:** `done` — `check_union_derivation` checks the five conditions on the objects of the ancestors and of the derived union (one test per condition and for the chain of ancestors), `expand_union` builds Table 19's hypothetical union. No Type Representation carries a base union: IDL 4.2 has no syntax, the XSD gives `baseType` to `structDecl` and `annotationTypeDecl` only, `MinimalUnionHeader` and `CompleteUnionHeader` are empty (findings file, contradictions). A representation that adds a base calls the functions.

---

## §7.2.2.4.6 Related Key-Erased Type
### §7.2.2.4.6 KeyErased(T)

**Spec:** §7.2.2.4.6, p. 64–65 — "For each Aggergated Type 'T' the type system
defines a related Aggregated Type KeyErased(T) obtainied from 'T' by removing
the key designation from any member that had it [...]. In the case the type 'T'
had no members designated key members then KeyErased(T) is the same type 'T'."

**Repo:** `crates/types/src/assignability.rs::key_erased`

**Tests:** `crates/types/tests/type_system_rules.rs::key_erased_removes_the_key_designation_and_nothing_else`,
`crates/types/tests/type_system_rules.rs::the_types_of_members_are_compared_with_their_keys_erased`

**Status:** `done` — `assignability::key_erased` is public and builds KeyErased(T) for a structure and a union after the clause's example (`Aircraft`, `Coordinates`, `Command`); a type without keys and an enumeration are left as they are. Table 19 compares the types of members as KeyErased (`member_types_assignable`).

---

## §7.2.2.4.7 Related Key-Holder Type
### §7.2.2.4.7 KeyHolder(T): type with a key — only the key members

**Spec:** §7.2.2.4.7, p. 66 — "If 'T' has any members designated as key members
[...], then KeyHolder(T) removes any members of 'T' that are not designated as
key members."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_holder_plan` (l. 41);
`crates/ast-layout/src/keyhash.rs::holder_keeps_only_keys` (l. 133)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v8_keyed_sensor_plans_a_four_byte_zero_padded_key_holder`,
`crates/ast-layout/tests/wire_vectors.rs::key_members_come_back_in_member_id_order_not_declaration_order`

**Status:** done

### §7.2.2.4.7 KeyHolder(T): structure without a key — all members as key

**Spec:** §7.2.2.4.7, p. 66 — "If 'T' is a structure with no key members, then
KeyHolder(T) adds a key designator to each member."

**Repo:** `crates/ast-layout/src/keyhash.rs::holder_keeps_only_keys` (l. 133);
`crates/ast-layout/src/keyhash.rs::member_key` (l. 161)

**Tests:** `crates/ast-layout/tests/vendor_gate.rs::a_key_member_of_a_keyless_type_contributes_all_of_its_members`,
`crates/ast-layout/tests/wire_vectors.rs::a_type_with_no_key_member_has_no_key_members_and_a_whole_type_holder`

**Status:** done — for a nested type without a key. A topic type without a key
has no KeyHash; `key_members` reports an empty list for it
(`a_keyless_topic_type_still_reports_no_key_members`).

### §7.2.2.4.7 KeyHolder(T): union without a key discriminator is T

**Spec:** §7.2.2.4.7, p. 66 — "If T' is a union and the discriminator is not
marked as key, then KeyHolder(T) is the same type T."

**Repo:** `crates/ast-layout/src/keyhash.rs::union_key_discriminator`,
`crates/ast-layout/src/key_maximum.rs`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::a_union_as_the_type_of_a_key_member_is_in_the_holder_whole`,
`crates/ast-layout/tests/key_maximum.rs::union_takes_largest_branch_not_sum`

**Status:** `done` — a union whose discriminator is not a key goes into the holder whole (discriminator and selected member, PLAIN_CDR2 big endian); `key_maximum` counts the discriminator and the largest branch. A union with `@key` on the discriminator is the discriminator alone in the holder.

### §7.2.2.4.7 KeyHolder(T): union as the type of a key member, no test

**Spec:** §7.2.2.4.7, p. 66 — KeyHolder of a union without a key.

**Repo:** `crates/ast-layout/src/keyhash.rs::collect_key_members`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::a_union_as_the_type_of_a_key_member_is_in_the_holder_whole`

**Status:** `done` — the test checks the octets of the KeyHash for two branches of the union and that a non-key member does not move the value.

---

## §7.2.2.4.8 Verbatim Text
### §7.2.2.4.8 VerbatimText: language, placement, text

**Spec:** §7.2.2.4.8, p. 67–68 — "A VerbatimText object defines three
properties; each is a string: language [...] placement [...] text".

**Repo:** `crates/ast/src/facet/xtypes.rs::VerbatimDirective` (l. 218);
`crates/idl-lower/src/annotation.rs::verbatim_annotation` (l. 934)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_verbatim_directive_keeps_its_language_placement_and_text`,
`crates/idl-lower/tests/annotations.rs::repeated_verbatim_directives_accumulate_in_declaration_order`

**Status:** done

### §7.2.2.4.8.1 Property: Language — languages and "*", case-insensitive

**Spec:** §7.2.2.4.8.1, p. 68 — "When a Type Representation compiler generates
code for the programming language named (case-insensitively) by this property,
it shall copy the string contained in the text property into its output. The
string 'c' [...] 'c++' [...] 'java' [...] The string '*' (an asterisk) shall
indicate that text applies to all programming languages."

**Repo:** `crates/lang-rules/src/verbatim.rs::language_matches` (l. 105);
`crates/lang-rules/src/verbatim.rs::aliases_of` (l. 324)

**Tests:** `crates/lang-rules/src/verbatim.rs::tests::the_wildcard_concerns_every_backend_and_names_compare_caselessly`,
`crates/lang-rules/src/verbatim.rs::tests::a_directive_for_another_language_is_left_to_that_backend`

**Status:** done

### §7.2.2.4.8.2 Property: Placement — six values, case-insensitive

**Spec:** §7.2.2.4.8.2, p. 68 — "It shall be interpreted in a case-insensitive
manner. All Type Representation compilers shall recognize the following
placement strings [...] begin-declaration-file, before-declaration,
begin-declaration, end-declaration, after-declaration, end-declaration-file".

**Repo:** `crates/idl-lower/src/annotation.rs::verbatim_placement` (l. 1103)

**Tests:** `crates/idl-lower/tests/annotations.rs::an_unknown_verbatim_placement_is_reported`,
`crates/idl-lower/tests/annotations.rs::a_verbatim_placement_is_read_without_regard_to_case_and_in_the_xtypes_spelling`,
`crates/lang-rules/src/verbatim.rs::tests::every_placement_lands_in_its_own_slot`

**Status:** done — The names of IDL 4.2 §8.3.5.1 are accepted, the six of the specification (`before-declaration`, `begin-declaration`, `end-declaration`, `after-declaration`, `begin-file`, `end-file`) and `begin-declaration-file` and `end-declaration-file`, with no distinction of case.

### §7.2.2.4.8.2 Placement: begin-declaration-file, end-declaration-file, spelling

**Spec:** §7.2.2.4.8.2, p. 68 — placement values case-insensitive, among them
`begin-declaration-file` and `end-declaration-file`.

**Repo:** `crates/idl-lower/src/annotation.rs::verbatim_placement`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_verbatim_placement_is_read_without_regard_to_case_and_in_the_xtypes_spelling`

**Status:** done — `verbatim_placement` knows `begin-declaration-file` and `end-declaration-file` and compares with no distinction of case.

### §7.2.2.4.8.2 Placement: behaviour per value

**Spec:** §7.2.2.4.8.2, p. 68 — start of the file before all type declarations,
immediately before the declaration, in the body before all members, in the body
after all members, immediately after the declaration, end of the file.

**Repo:** `crates/lang-rules/src/verbatim.rs::declaration_slots` (l. 129);
`crates/lang-rules/src/verbatim.rs::file_slots` (l. 167);
`crates/lang-rules/src/verbatim.rs::weave` (l. 245)

**Tests:** `crates/lang-rules/src/verbatim.rs::tests::every_placement_lands_in_its_own_slot`,
`crates/lang-rules/tests/verbatim_end_to_end.rs::cpp_places_all_six_slots_in_spec_order`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`

**Status:** done — a generator whose tree cannot hold text at `begin`/`end` (C
`struct`, `enum`) writes it next to the declaration; a verbatim on a member is
lifted into the `begin` slot of the aggregate.

### §7.2.2.4.8.3 Property: Text — copied unchanged

**Spec:** §7.2.2.4.8.3, p. 69 — "The Type Representation compiler shall copy the
string contained in this property into its output as described above."

**Repo:** `crates/lang-rules/src/verbatim.rs::weave` (l. 245)

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::multi_line_text_arrives_line_by_line`,
`crates/lang-rules/tests/verbatim_every_column.rs::the_wildcard_reaches_every_column`

**Status:** done

---

## §7.2.2.4.9 External Data

### §7.2.2.4.9 External: member referring to an incomplete type

**Spec:** §7.2.2.4.9, p. 69 — "An external member of an aggregated type shall be
permitted to refer to a type whose definition is incomplete (i.e., is identified
only by a forward declaration) at the time of the member's declaration."

**Repo:** `crates/types/src/resolve.rs::transitive_dependencies` (l. 246)

**Tests:** `crates/types/src/resolve.rs::tests::external_forward_decl_roundtrip`

**Status:** done

### §7.2.2.4.9 External: member referring to the containing type

**Spec:** §7.2.2.4.9, p. 69 — "An external member of an aggregated type shall be
permitted to refer to the member's containing type."

**Repo:** `crates/lang-rules/tests/external_is_an_indirection.rs`;
`crates/ast-layout/src/plan.rs`

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value`,
`crates/types/src/resolve.rs::tests::external_diamond_resolves_through_both_paths`

**Status:** done

### §7.2.2.4.9 External: every member except the discriminator

**Spec:** §7.2.2.4.9, p. 69 — "Each member of an aggregated type—with the
exception of the discriminator of a union type—may be optionally marked as
external."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `external` l. 336);
`crates/idl-lower/src/validate.rs::external_is_a_member_annotation` (l. 134)

**Tests:** `crates/idl-lower/tests/validation.rs::external_on_a_member_and_on_a_branch_is_accepted`,
`crates/idl-lower/tests/validation.rs::external_on_a_type_declaration_is_refused_by_name`

**Status:** done

### §7.2.2.4.9 External: elements of a collection

**Spec:** §7.2.2.4.9, p. 69 — "Likewise, the elements of a collection type may
be optionally marked as external."

**Repo:** `crates/ast/src/query.rs::element_is_external`,
`crates/ast/src/query.rs::key_is_external`,
`crates/idl-parse/src/decl.rs::parse_declarator_list`,
`crates/idl-lower/src/lower.rs::declarator_type`

**Tests:** `crates/idl-lower/tests/annotations.rs::external_on_an_element_a_map_key_or_a_map_value_is_carried`,
`crates/idl-lower/tests/annotations.rs::external_after_an_array_declarators_name_is_the_elements`,
`crates/types/tests/collection_element_flags.rs::the_element_of_a_collection_carries_its_external_and_its_try_construct`

**Status:** done — `@external` on an element, a map key or a map value (`sequence<@external Foo, 42>`) and after the name of an array declarator (`Foo array_of_foo @external [42];`) applies to the elements: the model has `element_is_external` and `key_is_external`, the element identifier carries `IS_EXTERNAL` and `TRY_CONSTRUCT`.

### §7.2.2.4.9 External: no object graphs

**Spec:** §7.2.2.4.9, p. 69 — "Note that this attribute does not provide a means
for modeling object graphs."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — note.

---

## §7.2.2.5 Nested Types

### §7.2.2.5 Nested Types

**Spec:** §7.2.2.5, p. 69 — "Types that are not intended to describe topic data
are referred to as nested types." Ein IDL-Compiler "need not generate typed
DataWriter, DataReader, and TypeSupport classes for types that are not intended
to type topics."

**Repo:** `crates/ast/src/query.rs::effective_nested` (l. 484);
`crates/types/src/from_ast.rs::aggregate_flags` (l. 1614,
`IS_NESTED`)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_nested_annotation_reaches_the_type`,
`crates/idl-lower/tests/annotations.rs::default_nested_on_a_module_reaches_the_types_in_it`

**Status:** done — the annotations `@nested` and `@default_nested` are listed
under §7.3.1.2.1.7.

---

## §7.2.2.6 Annotations

### §7.2.2.6 Annotation on type, element, member, literal, related_type

**Spec:** §7.2.2.6, p. 69 — "An annotation describes a piece of metadata
attached to a type or an element/member/literal of an
aggregated/collection/enumerated type. Annotations can also be attached to the
related_type of an alias type."

**Repo:** `crates/ast/src/facet/applied.rs`;
`crates/idl-lower/src/annotation.rs::apply_user_annotation` (l. 417)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::an_annotation_applied_to_the_type_itself_reaches_the_object`,
`crates/types/tests/type_object_custom_annotations.rs::every_position_with_a_complete_member_detail_is_read`,
`crates/types/tests/type_object_custom_annotations.rs::an_annotation_on_a_typedef_reaches_the_alias_object`,
`crates/types/tests/annotations_on_uses.rs::an_annotation_on_an_element_is_in_the_complete_collection_object`

**Status:** done — the annotation reaches the type, the member, the literal, a collection element and the `typedef` declaration (`CompleteAliasType`, header `ann_custom`).

### §7.2.2.6 An annotation on a collection element is kept

**Spec:** §7.2.2.6, p. 69 — annotations on an element of a collection type.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_use_site`,
`crates/types/src/from_ast.rs::collection_object`,
`crates/types/src/type_object/to_minimal.rs::plain_collection`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_declared_annotation_on_an_element_a_key_or_an_array_is_kept`,
`crates/types/tests/annotations_on_uses.rs::an_annotation_on_an_element_is_in_the_complete_collection_object`,
`crates/types/tests/annotations_on_uses.rs::the_minimal_form_is_the_one_the_unannotated_declaration_has`,
`crates/types/tests/annotations_on_uses.rs::the_complete_object_projects_onto_the_same_minimal_object`,
`crates/types/tests/annotations_on_uses.rs::the_applications_come_back_into_the_model`,
`crates/types/tests/annotations_on_uses.rs::an_undeclared_annotation_leaves_the_collection_plain`

**Status:** done — A user-declared annotation on a collection element, a map key or after an array name is kept in the model on the collection (`XTypesFacet::element_annotations`, `key_annotations`) and written into the `ann_custom` of `CompleteCollectionElement`. The collection is then a type with a `TypeObject` (§7.3.4.1): `CompleteSequenceType`, `CompleteArrayType` or `CompleteMapType`, in the hash identifier of the COMPLETE form; the MINIMAL form stays the plain collection.

### §7.2.2.6 AnnotationType: parameters with a default

**Spec:** §7.2.2.6, p. 69–70 — "An AnnotationType defines the structure of the
metadata as a set of AnnotationParameters that can be assigned values when the
annotation is applied. [...] The definition of an AnnotationType can specify the
default value of each AnnotationParameter."

**Repo:** `crates/types/src/from_ast.rs::annotation_type_object` (l. 537);
`crates/idl-lower/src/annotation.rs::check_annotation_application` (l. 468)

**Tests:** `crates/types/tests/type_object_annotations.rs::an_annotation_declaration_has_a_type_object`,
`crates/types/tests/type_object_custom_annotations.rs::the_parameter_value_is_the_value_that_was_applied`,
`crates/types/tests/type_object_custom_annotations.rs::an_unapplied_member_writes_ann_custom_absent`

**Status:** done

### §7.2.2.6 AnnotationParameter: permitted types

**Spec:** §7.2.2.6, p. 70 — "The types permitted for an AnnotationParameter are:
Primitive types, String types of Char8 or Char16 elements, Enumerated types".

**Repo:** `crates/types/src/from_ast.rs::annotation_type_object` (l. 537);
`crates/types/src/type_object/common.rs::AnnotationParameterValue` (l. 488)

**Tests:** `crates/types/tests/annotation_parameter_value_union.rs::an_annotation_with_long_double_wchar_and_wstring_parameters_builds`,
`crates/types/tests/type_object_custom_annotations.rs::an_enumerated_annotation_parameter_is_in_the_closure`,
`crates/idl-lower/tests/validation.rs::an_annotation_member_is_a_primitive_a_string_or_an_enum`

**Status:** done — Parameters of type Boolean, Byte, Int8 to UInt64, Float32, Float64, Float128, Char8, Char16, enumeration, String8 and String16 are represented; `AnnotationParameterValue` has every case of the union of Annex B and the extended case.

### §7.2.2.6 AnnotationParameter: long double, wchar, wstring are represented

**Spec:** §7.2.2.6, p. 70 — primitive types and strings of Char16.

**Repo:** `crates/types/src/type_object/common.rs::AnnotationParameterValue`

**Tests:** `crates/types/tests/annotation_parameter_value_union.rs::a_wchar_value_is_tk_char16_and_two_octets`,
`crates/types/tests/annotation_parameter_value_union.rs::a_long_double_value_is_tk_float128_and_sixteen_octets`,
`crates/types/tests/annotation_parameter_value_union.rs::a_wstring_value_is_tk_string16_with_an_octet_length`,
`crates/types/tests/annotation_parameter_value_union.rs::an_unknown_discriminator_is_the_extended_value`

**Status:** done — `AnnotationParameterValue` has `Float128Bits`, `Char16`, `String16` and the extended case (with a DHEADER).

### §7.2.2.6 AnnotationParameter: a type that is not permitted is refused

**Spec:** §7.2.2.6, p. 70 — only primitive types, strings and enumerated types.

**Repo:** `crates/idl-lower/src/validate.rs::annotation_member_types`

**Tests:** `crates/idl-lower/tests/validation.rs::an_annotation_member_is_a_primitive_a_string_or_an_enum`

**Status:** done — `validate.rs::annotation_member_types` refuses an annotation member whose type is neither a primitive, a string nor an enumeration.

---

## §7.2.2.7 Try Construct behavior

### §7.2.2.7 Background and Table 10

**Spec:** §7.2.2.7, p. 70–72 — motivation with `STRUCT1024`/`STRUCT128` and
`SEQ1024`/`SEQ128` (Table 10).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example; the rules follow in Table 11 and
§7.2.4.

### §7.2.2.7 Construction is recursive

**Spec:** §7.2.2.7, p. 72 — "Object construction for collection and aggregated
types is done recursively. [...] failure to construct a nested element/member
can prevent the construction of the collection/aggregated type."

**Repo:** `crates/wire-codec/src/decode.rs`

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/wire-probe/tests/try_construct.rs::an_element_annotation_trims_the_element_and_not_the_collection`

**Status:** done

### §7.2.2.7 TryConstruct on array and sequence elements

**Spec:** §7.2.2.7, p. 72 — "Array and Sequence collection types may explicitly
declare that their element has one of three kinds of TryConstruct behavior".

**Repo:** `crates/ast/src/facet/xtypes.rs::XTypesFacet` (l. 242, field `element_try_construct`);
`crates/ast/src/query.rs::element_try_construct` (l. 413)

**Tests:** `crates/idl-lower/tests/annotations.rs::try_construct_reaches_every_position_the_clause_names`,
`crates/wire-probe/tests/try_construct.rs::an_element_annotation_trims_the_element_and_not_the_collection`

**Status:** done

### §7.2.2.7 TryConstruct on map key and map value

**Spec:** §7.2.2.7, p. 72 — "Map collection types may explicitly declare that
their 'key' and or 'value' element has one of three kinds of TryConstruct
behavior".

**Repo:** `crates/ast/src/query.rs::key_try_construct` (l. 449);
`crates/idl-lower/src/annotation.rs::apply_use_site` (l. 775)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_map_key_may_only_be_discard`,
`crates/wire-probe/tests/try_construct.rs::a_map_value_trims_and_the_entry_survives`

**Status:** done — the map value takes all three kinds; the map key only
`DISCARD`, because Table 17 provides no other for the key. §7.2.2.7 names three
kinds for the key too (contradiction in the specification, listed in the
findings file).

### §7.2.2.7 TryConstruct on structure and union members, discriminator

**Spec:** §7.2.2.7, p. 72 — "Structure and Union types may explicitly declare
member has one of three kinds of TryConstruct behavior [...]. In the case of
Unions this extends to the discriminator member."

**Repo:** `crates/idl-lower/src/annotation.rs::try_construct_annotation` (l. 725);
`crates/ast/src/query.rs::discriminator_try_construct` (l. 508)

**Tests:** `crates/idl-lower/tests/annotations.rs::try_construct_reaches_every_position_the_clause_names`,
`crates/wire-probe/tests/try_construct.rs::a_defaulted_discriminator_defaults_the_member_it_now_selects`

**Status:** done

### §7.2.2.7 The default is DISCARD

**Spec:** §7.2.2.7, p. 72 — "The default behavior unless otherwise specified
using the TryConstruct annotation is DISCARD."

**Repo:** `crates/ast/src/facet/xtypes.rs::ABSENT` (l. 88)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_bare_try_construct_is_use_default_and_an_absent_one_is_discard`,
`crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/types/src/builder.rs::tests::member_default_try_construct_is_discard_bits`

**Status:** done

### §7.2.2.7 Table 11 — DISCARD

**Spec:** §7.2.2.7, Table 11, p. 72 — "Failure to construct an element or member
propagates to the collection or aggregated type that contains it."

**Repo:** `crates/wire/src/try_construct.rs::fit_string` (l. 89);
`crates/wire/src/try_construct.rs::fit_sequence` (l. 131)

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/wire-probe/tests/try_construct.rs::a_discard_and_a_malformed_stream_are_not_the_same_error`,
`crates/wire-conformance/tests/map_bytes.rs::an_over_long_map_is_discarded_where_the_position_says_discard`

**Status:** done

### §7.2.2.7 Table 11 — USE_DEFAULT

**Spec:** §7.2.2.7, Table 11, p. 72 — "If an element or member cannot be
constructed, the element/member shall be set to its default value (according to
its type as described in Table 9) and does not cause the collection/aggregated
object to fail its construction."

**Repo:** `crates/wire/src/try_construct.rs::Narrowed` (l. 61)

**Tests:** `crates/wire-probe/tests/try_construct.rs::use_default_substitutes_table_nines_value_and_keeps_the_sample`,
`crates/wire-conformance/tests/map_bytes.rs::an_over_long_map_takes_the_default_where_the_position_says_use_default`

**Status:** done

### §7.2.2.7 Table 11 — TRIM: only string, wstring, sequence, map

**Spec:** §7.2.2.7, Table 11, p. 73 — "This option only applies to elements or
members of type string, wide string, sequence, or map. The behavior when applied
to other element/member types is unspecified and may be treated as an error."

**Repo:** `crates/idl-lower/src/validate.rs::refuse_trim` (l. 594);
`crates/ast/src/facet/xtypes.rs::trim_is_defined_for` (l. 131)

**Tests:** `crates/idl-lower/tests/annotations.rs::trim_on_a_type_that_cannot_be_trimmed_is_refused`

**Status:** done — treated as an error.

### §7.2.2.7 Table 11 — TRIM: cut to the length of the receiver, order preserved

**Spec:** §7.2.2.7, Table 11, p. 73 — "In this situation the element or member
is constructed trimming the received object to the length that can be
accommodated by the receiving member type. The order of the characters in the
string or elements in the sequence or map is preserved."

**Repo:** `crates/wire/src/try_construct.rs::fit_string` (l. 89);
`crates/wire/src/try_construct.rs::fit_sequence` (l. 131);
`crates/wire/src/try_construct.rs::fit_map` (l. 169);
`crates/wire/src/try_construct.rs::fit_wstring` (l. 209)

**Tests:** `crates/wire-probe/tests/try_construct.rs::trim_cuts_the_string_to_the_bound_and_keeps_the_sample`,
`crates/wire-probe/tests/try_construct.rs::trim_cuts_a_sequence_to_its_bound_keeping_the_first_elements`,
`crates/wire-probe/tests/try_construct.rs::a_trim_stops_on_a_character_boundary`,
`crates/wire-conformance/tests/map_bytes.rs::an_over_long_map_is_trimmed_where_the_position_says_trim`

**Status:** done — a string is not cut inside a character; for a map the entries
with the smallest keys remain.

---

## §7.2.3 Type Extensibility and Mutability

### §7.2.3 Three extensibility kinds: FINAL, APPENDABLE, MUTABLE

**Spec:** §7.2.3, p. 73 — "the type system introduces the concept of type
extensibility and defines three extensibility kinds: final, appendable and
mutable." FINAL: "the range of its possible data values is strictly defined";
APPENDABLE: "two types, where one contains all of the elements/members of the
other plus additional elements/members appended to the end, may remain
assignable"; MUTABLE: "two types may differ from one another in the additional,
removal, and/or transposition of elements/members while remaining assignable."

**Repo:** `crates/ast/src/facet/xtypes.rs::Extensibility` (l. 16);
`crates/types/src/type_object/flags.rs::IS_FINAL` (l. 14)

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/types/src/assignability.rs::tests::final_struct_with_extra_writer_field_is_not_assignable`,
`crates/types/src/assignability.rs::tests::appendable_struct_with_extra_writer_field_is_assignable`,
`crates/types/src/assignability.rs::tests::mutable_struct_member_id_matching`

**Status:** done

### §7.2.3 Extensibility affects the encoding

**Spec:** §7.2.3, p. 74 — "This attribute may be used by the Data
Representations to modify the encoding of the type in order to support its
extensibility."

**Repo:** `crates/ast-layout/src/xcdr.rs::extensibility_of` (l. 1689)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_gives_a_dheader_to_anything_that_is_not_final`,
`crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`

**Status:** done — the rules are under §7.4.

### §7.2.3 Table 12 — Aggregated Types

**Spec:** §7.2.3, Table 12, p. 74 — "Aggregated types may be final, appendable,
or mutable on a type-by-type basis. However, the extensibility kind of a
structure type with a base type must match that of the base type. It shall not
be permitted for a subtype to change the extensibility kind of its base type.
Any members marked as keys must be present in all variants of the type."

**Repo:** `crates/idl-lower/src/validate.rs::inheritance_rules` (l. 415);
`crates/types/src/assignability.rs::must_not_be_missing` (l. 1054)

**Tests:** `crates/idl-lower/tests/keys_and_inheritance.rs::a_derived_structure_with_another_extensibility_than_its_base_is_refused`,
`crates/types/tests/table19_key_members.rs::a_writer_enum_key_literal_the_reader_lacks_is_not_assignable`,
`crates/types/src/assignability.rs::tests::a_mutable_writer_member_the_reader_lacks_and_must_understand_is_no`

**Status:** done — a key member missing on one side makes two variants
non-assignable.

### §7.2.3 Table 12 — Collection Types: no effect

**Spec:** §7.2.3, Table 12, p. 74 — "ARRAY_TYPE, SEQUENCE_TYPE, MAP_TYPE: For
these types the extensibility kind has no effect."

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (l. 89)

**Tests:** `crates/ast-layout/tests/layout.rs::an_array_of_aggregates_reuses_its_dheader_as_the_nextint`,
`crates/ast-layout/tests/layout.rs::a_map_frames_as_lc5_only_when_it_carries_a_dheader`

**Status:** done — whether a collection carries a DHEADER depends on the element
type and on the encoding version, not on an extensibility of the collection
(DDSXTY14-25, `docs/omg-xtypes-issues.md` §A5).

### §7.2.3 Table 12 — Enumerated Types: final or appendable

**Spec:** §7.2.3, Table 12, p. 74 — "ENUMERATION_TYPE, BITMASK_TYPE: These types
may be final or appendable on a type-by-type basis."

**Repo:** `crates/idl-lower/src/validate.rs::extensibility_is_final_or_appendable`,
`crates/types/src/assignability.rs::check_enumerations`

**Tests:** `crates/idl-lower/tests/validation.rs::an_enum_or_bitmask_is_never_mutable`,
`crates/types/src/assignability.rs::tests::a_final_enum_has_the_same_literals_on_both_sides`,
`crates/types/src/assignability.rs::tests::an_enum_that_grew_a_literal_is_still_assignable`

**Status:** `done` — `@mutable` on an enumeration or a bitmask is refused (`@final` and `@appendable` are accepted).

### §7.2.3 Table 12 — String Types: no effect

**Spec:** §7.2.3, Table 12, p. 74 — "STRING8_TYPE, STRING16_TYPE: For these
types the extensibility kind has no effect."

**Repo:** `crates/ast-layout/src/xcdr.rs::extensibility_of` (l. 1689)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v6_final_sequence_of_string_is_unbounded_and_frames_compactly_as_lc5`

**Status:** done — a string has no DHEADER.

### §7.2.3 Table 12 — ALIAS_TYPE: extensibility of the base type

**Spec:** §7.2.3, Table 12, p. 74 — "Since aliases are semantically equivalent
to their base types, the extensibility kind of an alias is always equal to that
of its base type."

**Repo:** `crates/types/src/resolve.rs::resolve_alias_chain`;
`crates/ast-layout/src/plan.rs`

**Tests:** `crates/types/src/resolve.rs::tests::resolve_chained_aliases_two_deep`

**Status:** done — layout and assignability resolve the alias.

### §7.2.3 Table 12 — Primitive types: no effect

**Spec:** §7.2.3, Table 12, p. 74 — "Primitive types: For these types the
extensibility kind has no effect."

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (l. 89)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v5_final_sequence_of_int32_is_unbounded`

**Status:** done

---

## §7.2.4 Type Compatibility

### §7.2.4 Relation is-assignable-from between every pair of types

**Spec:** §7.2.4, p. 74–75 — "the type system defines the 'is-assignable-from'
directed binary relationship between every pair of types in the Type System.
[...] T1 is-assignable-from T2".

**Repo:** `crates/types/src/assignability.rs::is_assignable` (l. 274);
`crates/types/src/assignability.rs::check_direct` (l. 321)

**Tests:** `crates/types/tests/evolution_assignability.rs::a_type_is_assignable_to_itself`,
`crates/types/tests/evolution_assignability.rs::every_evolution_case_gets_a_verdict_and_the_verdicts_are_recorded`

**Status:** done — `is_assignable(w, r, …)` takes the writer type (T2) first and
the reader type (T1) as second argument.

### §7.2.4.1 Construction: subset of the T2 objects, the rest detectable

**Spec:** §7.2.4.1, p. 75 — "a meaningful subset of T2 objects can construct T1
objects without misinterpretation and [...] the remaining objects of T2—which
cannot construct T1 objects—can be detected as such [...]. these per-object
'can-construct' rules are designed such that their enforcement does not require
any inspection of a data producer's type definition."

**Repo:** `crates/wire-codec/src/decode.rs`;
`crates/wire/src/try_construct.rs`

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/wire-probe/tests/try_construct.rs::an_enumerator_the_reader_does_not_declare_is_a_construction_failure`

**Status:** done — the decoder reads with the type of the reader; the type of
the writer is not consulted when a sample is read.

### §7.2.4.1 Construction: the default is discard, TryConstruct on the member

**Spec:** §7.2.4.1, p. 75 — "In the case T1 is-assignable-from T2 but an object
O2 of type T2 is encountered that cannot construct any object of type T1, the
default behavior is to discard the O2 object to avoid misinterpretation. This
behavior can be altered when the object O2 is a member of an Aggregated type
[...] the behavior is determined by the TryConstruct behavior specified for the
member."

**Repo:** `crates/wire/src/try_construct.rs::TryConstruct` (l. 42)

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/wire-probe/tests/try_construct.rs::use_default_substitutes_table_nines_value_and_keeps_the_sample`

**Status:** done

### §7.2.4.1 Table 13 — Example sequence of 10 / sequence of 5

**Spec:** §7.2.4.1, Table 13, p. 75 — a sequence of 10 integers and a sequence
of 5 integers are mutually assignable; only objects with at most 5 elements
construct T2.

**Repo:** `crates/types/src/assignability.rs::check_direct` (l. 321, arm Sequence)

**Tests:** `crates/types/src/assignability.rs::tests::sequence_bounds_ignored_when_policy_allows`,
`crates/lang-rules/tests/codec.rs::a_bounded_sequence_checks_its_bound_and_an_unbounded_one_has_none`

**Status:** done — with the default `ignore_sequence_bounds = true` both
directions are assignable; the decoder checks the length per sample.

### §7.2.4.2 Delimited: primitive and enumerated types

**Spec:** §7.2.4.2, p. 76 — "Primitive and Enumerated types (Enumeration and
Bitmask) are delimited types as their serialized size is fixed."

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (l. 89)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v5_final_sequence_of_int32_is_unbounded`

**Status:** done — as a property of the encoding. The term "delimited" is used
in the tree only for the choice of the DHEADER; for strong assignability see
§7.2.4.3.

### §7.2.4.2 Delimited: strings

**Spec:** §7.2.4.2, p. 76 — "Strings and wide strings are delimited types
because the serialization starts with a size from which it is possible to derive
the overall serialized length of the string."

**Repo:** `crates/wire/src/writer.rs::write_string` (l. 1881);
`crates/wire/src/writer.rs::write_wstring` (l. 1980)

**Tests:** `crates/wire/tests/wire.rs::a_string_length_counts_its_terminator`

**Status:** done

### §7.2.4.2 Delimited: collections with delimited element and key type

**Spec:** §7.2.4.2, p. 76 — "Collection types (arrays, sequences, maps) are
delimited if the collection element type is delimited. In the case of a map
collection the key type must also be delimited. Otherwise the collection is not
delimited."

**Repo:** `crates/types/src/assignability.rs::is_delimited`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::a_collection_of_delimited_elements_is_delimited`,
`crates/types/tests/strong_assignability_and_table19.rs::appendable_elements_are_delimited_under_xcdr2_only`

**Status:** done — `assignability.rs::is_delimited` answers §7.2.4.2: a type is delimited if it is MUTABLE, APPENDABLE under XCDR2 (`AssignabilityConfig::appendable_is_delimited`) or a collection with delimited element and key types.

### §7.2.4.2 Delimited: FINAL aggregates not delimited

**Spec:** §7.2.4.2, p. 76 — "Other than the types mentioned above all other
types with extensibility kind FINAL are not delimited."

**Repo:** `crates/ast-layout/src/xcdr.rs::extensibility_of` (l. 1689)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v1_empty_final_struct_plans_zero_bytes_and_no_dheader`

**Status:** done — as a property of the encoding: a FINAL aggregate carries no
DHEADER.

### §7.2.4.2 Delimited: APPENDABLE only under encoding version 2

**Spec:** §7.2.4.2, p. 76 — "Types with extensibility kind APPENDABLE are
delimited if serialized with encoding version 2 (DELIMITED_CDR). [...] They are
not delimited if serialized with encoding version 1."

**Repo:** `crates/ast-layout/src/xcdr.rs::extensibility_of` (l. 1689)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v9_appendable_plans_a_dheader`,
`crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`

**Status:** done

### §7.2.4.2 Delimited: MUTABLE under both encoding versions

**Spec:** §7.2.4.2, p. 76 — "Mutable types are also delimited with both encoding
version 1 and encoding version 2. [...] PL_CDR is a list of length-encoded
elements ended by a sentinel [...] PL_CDR2 starts with a delimiter header".

**Repo:** `crates/ast-layout/src/xcdr.rs` (Parameterliste and DHEADER)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v12_empty_mutable_plans_a_dheader_and_no_members`,
`crates/ast-layout/tests/xcdr1_gate.rs::plain_cdr1_plans_no_sentinel`

**Status:** done

### §7.2.4.3 Strong Assignability

**Spec:** §7.2.4.3, p. 76 — "If types T1 and T2 are equivalent using the MINIMAL
relation (see Clause 7.3.4.7), or alternatively if T1 is assignable-from T2 and
T2 is a delimited type, then T1 is said to be 'strongly' assignable from T2."

**Repo:** `crates/types/src/assignability.rs::strongly_assignable`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::a_sequence_of_final_structs_needs_equivalent_elements`,
`crates/types/tests/strong_assignability_and_table19.rs::array_elements_and_map_keys_and_values_are_strongly_assignable`,
`crates/types/tests/strong_assignability_and_table19.rs::members_of_appendable_and_final_types_are_strongly_assignable`

**Status:** done — `assignability.rs::strongly_assignable` checks strong assignability (Table 17: elements of array, sequence and map; Table 19: members at the same index of APPENDABLE and FINAL structs): a type is delimited or MINIMAL-equivalent.

### §7.2.4.4.1 Equivalent types mutually assignable

**Spec:** §7.2.4.4.1, p. 76–77 — "If two types T1 and T2 are equivalent
according to the MINIMAL relation (see Clause 7.3.4.7), then they are mutually
assignable [...]. The reverse is not always true."

**Repo:** `crates/types/src/assignability.rs::is_assignable` (l. 274, check `w == r` l. 301)

**Tests:** `crates/types/src/assignability.rs::tests::identical_type_identifiers_short_circuit_yes`,
`crates/types/src/assignability.rs::tests::equal_minimal_hash_is_assignable_with_empty_registry`,
`crates/types/src/assignability.rs::tests::equivalence_hash_identical_short_circuits_to_yes`

**Status:** done — `TK_NONE` is excepted; two endpoints with the invalid
identifier are not assignable (`tk_none_is_not_assignable_from_itself`).

### §7.2.4.4.2 Non-serialized members are ignored

**Spec:** §7.2.4.4.2, p. 77 — "Members that are marked as non-serialized, see
Sub Clause 7.3.1.2.1.14, shall be ignored during type compatibility checking."

**Repo:** `crates/types/src/from_ast.rs::structure` (l. 2082, omission l. 2105–2123)

**Tests:** `crates/types/tests/table19_key_members.rs::a_non_serialized_member_does_not_enter_the_comparison`

**Status:** done — a `@non_serialized` member does not go into the TypeObject.

### §7.2.4.4.3 Table 14 — ALIAS T1, non-alias T2

**Spec:** §7.2.4.4.3, Table 14, p. 77 — "ALIAS_TYPE | Any non ALIAS_TYPE type
kind T2 if and only if T1.base_type is-assignable-from T2".

**Repo:** `crates/types/src/assignability.rs::is_assignable` (l. 274,
`resolve_alias_chain` l. 306–311)

**Tests:** `crates/types/src/resolve.rs::tests::resolve_follows_single_alias_to_target`,
`crates/types/tests/evolution_assignability.rs::every_evolution_case_gets_a_verdict_and_the_verdicts_are_recorded`

**Status:** done

### §7.2.4.4.3 Table 14 — Non-alias T1, ALIAS T2

**Spec:** §7.2.4.4.3, Table 14, p. 77 — "Any non ALIAS_TYPE type kind |
ALIAS_TYPE T2 if and only if T1 is-assignable-from T2.base_type".

**Repo:** `crates/types/src/assignability.rs::is_assignable` (l. 274)

**Tests:** `crates/types/src/resolve.rs::tests::resolve_chained_aliases_two_deep`

**Status:** done

### §7.2.4.4.3 Table 14 — ALIAS T1, ALIAS T2

**Spec:** §7.2.4.4.3, Table 14, p. 77 — "ALIAS_TYPE | ALIAS_TYPE if and only if
T1.base_type is-assignable-from T2.base_type".

**Repo:** `crates/types/src/assignability.rs::check_minimal_types` (l. 725, arm Alias)

**Tests:** `crates/types/src/resolve.rs::tests::resolve_chained_aliases_two_deep`

**Status:** done

### §7.2.4.4.3 Aliases fully resolved

**Spec:** §7.2.4.4.3, p. 77 — "For the purpose of evaluating the
is-assignable-from relationship, aliases are considered to be fully resolved to
their ultimate base types."

**Repo:** `crates/types/src/resolve.rs::resolve_alias_chain`;
`crates/types/src/assignability.rs::key_members_fit` (l. 1260, resolution l. 1291)

**Tests:** `crates/types/src/resolve.rs::tests::resolve_chained_aliases_two_deep`

**Status:** done

### §7.2.4.4.4 Table 15 — A primitive type only from the same primitive type

**Spec:** §7.2.4.4.4, Table 15, p. 77 — "Any Primitive Type | The same Primitive
Type | Copy the primitive object."

**Repo:** `crates/types/src/assignability.rs::primitive_compatible` (l. 698)

**Tests:** `crates/types/src/assignability.rs::tests::only_the_identical_primitive_kind_is_assignable`,
`crates/types/src/assignability.rs::tests::primitive_widening_is_rejected_per_table_15`,
`crates/types/src/assignability.rs::tests::primitive_narrowing_and_cross_family_are_rejected`,
`crates/types/src/type_matcher_vectors.rs::uint8_to_uint32_widening_does_not_match`

**Status:** done — `octet` and `uint8` are different primitive types and not
mutually assignable; the OMG ShapeType IDL in `interop/shapes/omg-dds-rtps.idl`
therefore declares `sequence<uint8>`.

### §7.2.4.4.4 Table 15 — UINT8_TYPE from BITMASK with bound 1 to 8

**Spec:** §7.2.4.4.4, Table 15, p. 78 — "UINT8_TYPE | BITMASK_TYPE if and only
if T2.bound is between 1 and 8, inclusive."

**Repo:** `crates/types/src/assignability.rs::bitmask_uint_compatible` (l. 560)

**Tests:** `crates/types/src/assignability.rs::tests::bitmask_is_assignable_to_the_uint_of_its_bound`,
`crates/types/src/assignability.rs::tests::bitmask_is_not_assignable_to_a_wider_uint`

**Status:** done

### §7.2.4.4.4 Table 15 — UINT16_TYPE from BITMASK with bound 9 to 16

**Spec:** §7.2.4.4.4, Table 15, p. 78 — "UINT16_TYPE | BITMASK_TYPE if and only
if T2.bound is between 9 and 16, inclusive."

**Repo:** `crates/types/src/assignability.rs::bitmask_uint_compatible` (l. 560)

**Tests:** `crates/types/src/assignability.rs::tests::bitmask_is_assignable_to_the_uint_of_its_bound`

**Status:** done

### §7.2.4.4.4 Table 15 — UINT32_TYPE from BITMASK with bound 17 to 32

**Spec:** §7.2.4.4.4, Table 15, p. 78 — "UINT32_TYPE | BITMASK_TYPE if and only
if T2.bound is between 17 and 32, inclusive."

**Repo:** `crates/types/src/assignability.rs::bitmask_uint_compatible` (l. 560)

**Tests:** `crates/types/src/assignability.rs::tests::bitmask_is_assignable_to_the_uint_of_its_bound`

**Status:** done

### §7.2.4.4.4 Table 15 — UINT64_TYPE from BITMASK with bound 33 to 64

**Spec:** §7.2.4.4.4, Table 15, p. 78 — "UINT64_TYPE | BITMASK_TYPE if and only
if T2.bound is between 33 and 64, inclusive."

**Repo:** `crates/types/src/assignability.rs::bitmask_uint_compatible` (l. 560)

**Tests:** `crates/types/src/assignability.rs::tests::bitmask_is_assignable_to_the_uint_of_its_bound`

**Status:** done

### §7.2.4.4.4 Table 15 — Construction of a UINT from a BITMASK

**Spec:** §7.2.4.4.4, Table 15, p. 78 — "For each bitflag that is set in the
bitmask construct the integer value (1 << position) using the position of that
bitflag. Add all those integer values to obtain the resulting object O1 of type
T1".

**Repo:** `crates/types/src/assignability.rs::bitmask_uint_compatible`,
`crates/wire-codec/src/decode.rs::read_bitmask`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::an_unsigned_integer_is_built_from_the_flags_of_a_bitmask`,
`crates/types/tests/type_system_rules.rs::an_unsigned_integer_is_assignable_from_a_bitmask_of_its_width_only`

**Status:** `done` — Table 33 writes a bitmask as the unsigned integer of its bound; a reader of type `uint8` to `uint64` reads the octets and gets the sum of `1 << position` of the flags that are set (four bounds, both byte orders, both versions). Assignability holds for the width of the bound only. The reverse (bitmask from integer) is accepted by the code as well, as OpenDDS does; Table 15 does not list it.

### §7.2.4.4.5 Table 16 — STRING_TYPE: element type assignable

**Spec:** §7.2.4.4.5, Table 16, p. 78 — "STRING_TYPE | STRING_TYPE if and only
if T1.element_type is-assignable-from T2.element_type".

**Repo:** `crates/types/src/assignability.rs::check_direct` (l. 321, arms String8 and String16)

**Tests:** `crates/types/src/assignability.rs::tests::string8_vs_string16_not_assignable`,
`crates/types/src/assignability.rs::tests::string_small_and_large_interchangeable`,
`crates/types/src/assignability.rs::tests::string16_small_and_large_interchangeable`

**Status:** done — with the default `ignore_string_bounds = true` assignability
does not depend on the bound.

### §7.2.4.4.5 Table 16 — Construction: O2.length <= T1.length

**Spec:** §7.2.4.4.5, Table 16, p. 78 — "An object O2 of type T2 can-construct
an object of type T1 if and only if O2.length <= T1.length. Copy each character.
O1.length is set to O2.length."

**Repo:** `crates/wire/src/try_construct.rs::fit_string` (l. 89);
`crates/wire/src/try_construct.rs::fit_wstring` (l. 209)

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/wire/tests/wire.rs::a_value_longer_than_its_string_bound_is_refused_and_not_truncated`

**Status:** done — the bound of a `string<N>` counts octets, that of a
`wstring<N>` UTF-16 code units.

### §7.2.4.4.5.1 Example: Strings

**Spec:** §7.2.4.4.5.1, p. 78–79 — narrow and wide strings are not mutually
assignable; a string that is too long constructs no T1 object.

**Repo:** `crates/types/src/assignability.rs::check_direct`,
`crates/wire-codec/src/decode.rs (TryConstruct)`

**Tests:** `crates/types/tests/type_system_rules.rs::narrow_and_wide_strings_are_not_assignable_to_each_other`,
`crates/wire-probe/tests/xtypes_enc_items.rs::a_string_longer_than_the_bound_of_the_reader_constructs_no_object`

**Status:** `done` — narrow and wide strings are not assignable to each other, strings of one kind are whatever the bound; a string beyond the bound of the reader constructs no object (`ProbeError::Discarded`), one within the bound does.

### §7.2.4.4.6 Table 17 — ARRAY_TYPE: equal bounds, element strongly assignable

**Spec:** §7.2.4.4.6, Table 17, p. 79 — "ARRAY_TYPE | ARRAY_TYPE if and only if:
T1.bounds[] == T2.bounds[]; T1.element_type is strongly assignable from
T2.element_type".

**Repo:** `crates/types/src/assignability.rs::strongly_assignable`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::array_elements_and_map_keys_and_values_are_strongly_assignable`,
`crates/types/tests/strong_assignability_and_table19.rs::a_sequence_of_final_structs_needs_equivalent_elements`

**Status:** done — The element type (for a map key and value) is checked with `strongly_assignable`.

### §7.2.4.4.6 Table 17 — ARRAY_TYPE: construction per element with TryConstruct

**Spec:** §7.2.4.4.6, Table 17, p. 79 — "Each element of the T1 array shall be
constructed from the corresponding element of the O2 array. If an element of T1
cannot be constructed from the O2 element, the result depends on the
TryConstruct behavior associated with T1 element type."

**Repo:** `crates/ast/src/query.rs::element_try_construct` (l. 413);
`crates/wire-codec/src/decode.rs`

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_element_annotation_trims_the_element_and_not_the_collection`

**Status:** done

### §7.2.4.4.6 Table 17 — SEQUENCE_TYPE: element strongly assignable

**Spec:** §7.2.4.4.6, Table 17, p. 80 — "SEQUENCE_TYPE | SEQUENCE_TYPE if and
only if T1.element_type is strongly assignable from T2.element_type".

**Repo:** `crates/types/src/assignability.rs::strongly_assignable`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::array_elements_and_map_keys_and_values_are_strongly_assignable`,
`crates/types/tests/strong_assignability_and_table19.rs::a_sequence_of_final_structs_needs_equivalent_elements`

**Status:** done — The element type (for a map key and value) is checked with `strongly_assignable`.

### §7.2.4.4.6 Table 17 — SEQUENCE_TYPE: construction O2.length <= T1.length

**Spec:** §7.2.4.4.6, Table 17, p. 80 — "An object O2 of type T2 can construct
T1 if and only if O2.length <= T1.length. [...] If an element of O2 cannot
construct T1.element_type, the result depends on the TryConstruct behavior".

**Repo:** `crates/wire/src/try_construct.rs::fit_sequence` (l. 131)

**Tests:** `crates/wire-probe/tests/try_construct.rs::trim_cuts_a_sequence_to_its_bound_keeping_the_first_elements`,
`crates/lang-rules/tests/codec.rs::a_bounded_sequence_checks_its_bound_and_an_unbounded_one_has_none`

**Status:** done

### §7.2.4.4.6 Table 17 — MAP_TYPE: key and value strongly assignable

**Spec:** §7.2.4.4.6, Table 17, p. 80 — "MAP_TYPE | MAP_TYPE if and only if:
T1.key_element_type is strongly assignable from T2.key_element_type;
T1.element_type is strongly assignable from T2.element_type."

**Repo:** `crates/types/src/assignability.rs::strongly_assignable`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::array_elements_and_map_keys_and_values_are_strongly_assignable`,
`crates/types/tests/strong_assignability_and_table19.rs::a_sequence_of_final_structs_needs_equivalent_elements`

**Status:** done — The element type (for a map key and value) is checked with `strongly_assignable`.

### §7.2.4.4.6 Table 17 — MAP_TYPE: construction

**Spec:** §7.2.4.4.6, Table 17, p. 80 — "An object O2 of type T2 can construct
T1 if and only if O2.length <= T1.length [...] If a key element of O2 cannot
construct the corresponding key type of T1 the entire map O2 cannot construct
any object of type T1."

**Repo:** `crates/wire/src/try_construct.rs::fit_map` (l. 169)

**Tests:** `crates/wire-conformance/tests/map_bytes.rs::an_over_long_map_is_discarded_where_the_position_says_discard`,
`crates/wire-conformance/tests/map_bytes.rs::an_over_long_map_is_trimmed_where_the_position_says_trim`,
`crates/idl-lower/tests/annotations.rs::a_map_key_may_only_be_discard`

**Status:** done

### §7.2.4.4.7 Table 18 — BITMASK from BITMASK: equal bound

**Spec:** §7.2.4.4.7, Table 18, p. 81 — "BITMASK_TYPE | BITMASK_TYPE if and only
if T1.bound == T2.bound".

**Repo:** `crates/types/src/assignability.rs::check_minimal_types` (l. 725, arm Bitmask l. 813–844)

**Tests:** `crates/types/tests/table19_key_members.rs::two_bitmasks_of_the_same_bound_are_assignable_whatever_their_flags`

**Status:** done

### §7.2.4.4.7 Table 18 — BITMASK from UINT_8/16/32/64

**Spec:** §7.2.4.4.7, Table 18, p. 81 — "UINT_32_TYPE if and only if T1.bound is
between 17 and 32, inclusive. UINT_16_TYPE [...] 9 and 16 [...] UINT_64_TYPE
[...] 33 and 64 [...] UINT_8_TYPE [...] 1 and 8, inclusive."

**Repo:** `crates/types/src/assignability.rs::check_direct` (l. 321, arm primitive against hash l. 516–519);
`crates/types/src/assignability.rs::bitmask_uint_compatible` (l. 560)

**Tests:** `crates/types/src/assignability.rs::tests::bitmask_is_assignable_to_the_uint_of_its_bound`,
`crates/types/src/assignability.rs::tests::a_struct_against_a_uint_is_still_no_row`

**Status:** done

### §7.2.4.4.7 Table 18 — BITMASK: construction per common bit

**Spec:** §7.2.4.4.7, Table 18, p. 81 — "Preserve bit values by index for all
bits identified in both T1 and T2."

**Repo:** `crates/wire-codec/src/decode.rs::read_bitmask` (l. 2499)

**Tests:** `crates/wire-codec/tests/bitmask_unnamed_bits.rs::a_bit_inside_the_bound_that_the_type_does_not_name_is_not_read`,
`crates/wire/tests/wire.rs::a_bitmask_read_keeps_only_the_declared_bits`,
`crates/wire-conformance/tests/bitmask_bytes.rs::a_bit_the_type_does_not_declare_is_not_part_of_the_value`

**Status:** done — Common bits are kept; a set bit that the reader does not name is not part of the value and is dropped on reading (the Rust reader and the decoders `lang-rules` generates).

### §7.2.4.4.7 Table 18 — BITMASK: a bit only the writer names is dropped

**Spec:** §7.2.4.4.7, Table 18, p. 81 — construction only over the bits that
both types name.

**Repo:** `crates/wire-codec/src/decode.rs::read_bitmask`

**Tests:** `crates/wire-codec/tests/bitmask_unnamed_bits.rs::a_bit_inside_the_bound_that_the_type_does_not_name_is_not_read`,
`crates/wire-codec/tests/bitmask_unnamed_bits.rs::the_declared_bits_beside_an_unnamed_one_are_kept`

**Status:** done — `read_bitmask` drops a bit that the type does not name, with no error; the named bits beside an unnamed one are kept.

### §7.2.4.4.7 Table 18 — ENUMERATION: equal extensibility

**Spec:** §7.2.4.4.7, Table 18, p. 81 — "ENUMERATION_TYPE if an only if:
T1.extensibility == T2.extensibility".

**Repo:** `crates/types/src/assignability.rs::check_enumerations`

**Tests:** `crates/types/src/assignability.rs::tests::a_final_enum_has_the_same_literals_on_both_sides`,
`crates/types/tests/strong_assignability_and_table19.rs::an_enum_without_an_extensibility_flag_is_appendable`

**Status:** done — An enumeration without an extensibility flag is APPENDABLE (§7.3.1.2.1.8), and the extensibility is always compared: FINAL against APPENDABLE is not assignable.

### §7.2.4.4.7 Table 18 — ENUMERATION: the extensibility comparison is tested

**Spec:** §7.2.4.4.7, Table 18, p. 81 — `T1.extensibility == T2.extensibility`.

**Repo:** —

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::an_enum_without_an_extensibility_flag_is_appendable`

**Status:** done — The test checks FINAL against APPENDABLE and against an enumeration without a flag, in both directions.

### §7.2.4.4.7 Table 18 — ENUMERATION: same name, same value and vice versa

**Spec:** §7.2.4.4.7, Table 18, p. 81 — "Any literals that have the same name in
T1 and T2 also have the same value, and any literals that have the same value in
T1 and T2 also have the same name. This behavior may be modified with the
@ignore_literal_names annotation".

**Repo:** `crates/types/src/assignability.rs::check_enumerations` (l. 964, loop l. 1025–1038)

**Tests:** `crates/types/src/assignability.rs::tests::enum_literals_that_disagree_are_not_assignable`,
`crates/types/src/assignability.rs::tests::an_enum_that_grew_a_literal_is_still_assignable`,
`crates/types/src/assignability.rs::tests::ignore_literal_names_on_both_sides_drops_the_name_rule`,
`crates/types/tests/table19_key_members.rs::ignore_literal_names_needs_the_annotation_on_both_types`

**Status:** done — in addition the code requires the same `bit_bound` of both
enumerations (DDSXTY14-34), which Table 18 does not name.

### §7.2.4.4.7 Table 18 — ENUMERATION: FINAL requires the same literals

**Spec:** §7.2.4.4.7, Table 18, p. 81 — "If extensibility is final T1 and T2
have the same literals."

**Repo:** `crates/types/src/assignability.rs::check_enumerations` (l. 964, branch l. 1009–1023)

**Tests:** `crates/types/src/assignability.rs::tests::a_final_enum_has_the_same_literals_on_both_sides`

**Status:** done

### §7.2.4.4.7 Table 18 — ENUMERATION: construction

**Spec:** §7.2.4.4.7, Table 18, p. 81 — "Choose the corresponding T1 literal if
it exists. If the name or value of the T2 object does not exist in T1, the
object cannot construct any object of type T1."

**Repo:** `crates/wire-codec/src/decode.rs` (enumeration, l. 2457–2473)

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_enumerator_the_reader_does_not_declare_is_a_construction_failure`

**Status:** done — the decision is then made by the `@try_construct` of the
position.

### §7.2.4.4.8 Aggregated: correspondence by member ID and name

**Spec:** §7.2.4.4.8, p. 81 — "For aggregated types, is-assignable-from is based
on the same relationship between the types' members. The correspondence between
members in the two types is established based on their respective member IDs and
on their respective member names."

**Repo:** `crates/types/src/assignability.rs::check_minimal_types` (l. 725)

**Tests:** `crates/types/src/assignability.rs::tests::mutable_struct_member_id_matching`,
`crates/types/src/assignability.rs::tests::a_member_name_at_two_different_ids_is_refused`

**Status:** done — the name is compared as a 4-octet hash in the MINIMAL
TypeObject.

### §7.2.4.4.8 Table 19 — UNION: equal extensibility

**Spec:** §7.2.4.4.8, Table 19, p. 82 — "T1.extensibility == T2.extensibility."

**Repo:** `crates/types/src/assignability.rs::check_unions`

**Tests:** `crates/types/tests/type_system_rules.rs::two_unions_of_different_extensibility_are_not_assignable`,
`crates/types/src/assignability.rs::tests::a_final_union_has_a_default_member_on_both_sides_or_neither`

**Status:** `done` — two unions of the same extensibility (`@final`, `@appendable`, `@mutable`) are assignable, two of different extensibility are not (all six pairings).

### §7.2.4.4.8 Table 19 — UNION: extensibility comparison without a test

**Spec:** §7.2.4.4.8, Table 19, p. 82 — gleiche Extensibility.

**Repo:** `crates/types/src/assignability.rs::check_unions`

**Tests:** `crates/types/tests/type_system_rules.rs::two_unions_of_different_extensibility_are_not_assignable`

**Status:** `done` — the test covers every pairing of different extensibility.

### §7.2.4.4.8 Table 19 — UNION: discriminator strongly assignable

**Spec:** §7.2.4.4.8, Table 19, p. 82 — "T1.discriminator.type
is-strongly-assignable-from T2.discriminator.type."

**Repo:** `crates/types/src/assignability.rs::check_unions` (l. 1536, call l. 1589–1597)

**Tests:** `crates/types/src/assignability.rs::tests::two_unions_need_a_case_label_in_common`

**Status:** done — a discriminator is primitive or an enumeration and so
delimited; `is_assignable` coincides with strong assignability here.

### §7.2.4.4.8 Table 19 — UNION: both discriminators keys or neither

**Spec:** §7.2.4.4.8, Table 19, p. 82 — "Either the discriminators of both T1
and T2 are keys or neither are keys."

**Repo:** `crates/types/src/assignability.rs::check_unions` (l. 1536, check l. 1580–1587)

**Tests:** `crates/types/tests/table19_key_members.rs::one_keyed_union_discriminator_and_one_unkeyed_are_not_assignable`

**Status:** done

### §7.2.4.4.8 Table 19 — UNION: same name, same ID and vice versa

**Spec:** §7.2.4.4.8, Table 19, p. 83 — "Any members in T1 and T2 that have the
same name also have the same ID and any members with the same ID also have the
same name."

**Repo:** `crates/types/src/assignability.rs::union_names_and_ids_agree` (l. 1121)

**Tests:** `crates/types/src/assignability.rs::tests::a_shared_union_label_may_carry_different_member_ids`

**Status:** done — can be switched off through `ignore_member_names`.

### §7.2.4.4.8 Table 19 — UNION: a label of T2 that selects a member in T1

**Spec:** §7.2.4.4.8, Table 19, p. 82 — "For all non-default labels in T2 that
select some member in T1 (including selecting the member in T1's default label),
the type of the selected member in T1 is assignable from the type of the T2
member."

**Repo:** `crates/types/src/assignability.rs::check_unions` (l. 1536, loop l. 1666–1712)

**Tests:** `crates/types/src/assignability.rs::tests::a_label_the_other_side_lacks_is_checked_against_its_default_member`

**Status:** done

### §7.2.4.4.8 Table 19 — UNION: a label of T1 that selects the default of T2

**Spec:** §7.2.4.4.8, Table 19, p. 82 — "If any non-default labels in T1 that
select the default member in T2, the type of the member in T1 is assignable from
the type of the T2 default member."

**Repo:** `crates/types/src/assignability.rs::check_unions` (l. 1536, l. 1734–1749)

**Tests:** `crates/types/src/assignability.rs::tests::a_label_the_other_side_lacks_is_checked_against_its_default_member`

**Status:** done

### §7.2.4.4.8 Table 19 — UNION: both with a default member

**Spec:** §7.2.4.4.8, Table 19, p. 82 — "If T1 and T2 both have default labels,
the type associated with T1 default member is assignable from the type
associated with T2 default member."

**Repo:** `crates/types/src/assignability.rs::check_unions` (l. 1536, l. 1751–1754)

**Tests:** `crates/types/src/assignability.rs::tests::a_final_union_has_a_default_member_on_both_sides_or_neither`

**Status:** done

### §7.2.4.4.8 Table 19 — UNION: FINAL equal labels, otherwise one in common

**Spec:** §7.2.4.4.8, Table 19, p. 83 — "If T1 (and therefore T2) extensibility
is final then the set of labels is identical. Otherwise, they have at least one
common label other than the default label."

**Repo:** `crates/types/src/assignability.rs::check_unions` (l. 1536, l. 1653–1664 and l. 1714–1728)

**Tests:** `crates/types/src/assignability.rs::tests::two_unions_need_a_case_label_in_common`,
`crates/types/src/assignability.rs::tests::a_default_case_is_not_a_case_label_in_common`

**Status:** done — for FINAL the code additionally requires that either both
sides or neither has a default member.

### §7.2.4.4.8 Table 19 — UNION: construction

**Spec:** §7.2.4.4.8, Table 19, p. 82–83 — discriminator from O2 or according to
TryConstruct (DEFAULT); selected member m1 from m2 or according to TryConstruct;
if the value selects no member in O1, m2 is truncated; if it selects none in O2
but one in O1, m1 takes its default.

**Repo:** `crates/wire-codec/src/decode.rs`;
`crates/ast/src/query.rs::discriminator_try_construct` (l. 508)

**Tests:** `crates/wire-probe/tests/try_construct.rs::a_defaulted_discriminator_defaults_the_member_it_now_selects`,
`crates/lang-rules/tests/c_end_to_end.rs::a_union_member_the_stream_does_not_carry_takes_its_default`

**Status:** done — the column names TryConstruct "DEFAULT"; Table 11 names the
kind USE_DEFAULT (naming in the specification, listed in the findings file).

### §7.2.4.4.8 Table 19 — STRUCTURE: equal extensibility

**Spec:** §7.2.4.4.8, Table 19, p. 83 — "T1 and T2 have the same extensibility
kind."

**Repo:** `crates/types/src/assignability.rs::check_minimal_types` (l. 725, check l. 738–742)

**Tests:** `crates/types/src/assignability.rs::tests::extensibility_mismatch_fails`,
`crates/types/src/assignability.rs::tests::minimal_and_complete_hash_still_reject_an_extensibility_mismatch`

**Status:** done

### §7.2.4.4.8 Table 19 — STRUCTURE: same name, same ID and vice versa

**Spec:** §7.2.4.4.8, Table 19, p. 83 — "Any members in T1 and T2 that have the
same name also have the same ID and any members with the same ID also have the
same name."

**Repo:** `crates/types/src/assignability.rs::struct_names_and_ids_agree` (l. 1100)

**Tests:** `crates/types/src/assignability.rs::tests::a_member_name_at_two_different_ids_is_refused`

**Status:** done — can be switched off through `ignore_member_names`.

### §7.2.4.4.8 Table 19 — STRUCTURE: at least one common member ID

**Spec:** §7.2.4.4.8, Table 19, p. 83 — "There is at least one member 'm1' of T1
and one corresponding member 'm2' of T2 such that m1.id == m2.id."

**Repo:** `crates/types/src/assignability.rs::at_least_one_common_member_id` (l. 1155)

**Tests:** `crates/types/src/assignability.rs::tests::two_mutable_structs_need_a_member_id_in_common`

**Status:** done — two structures without members count as assignable; the rule
presupposes members.

### §7.2.4.4.8 Table 19 — STRUCTURE: KeyErased of the member types assignable

**Spec:** §7.2.4.4.8, Table 19, p. 84 — "For any member 'm2' in T2, if there is
a member 'm1' in T1 with the same member ID, then the type KeyErased(m1.type)
is-assignable-from the type KeyErased(m2.type)."

**Repo:** `crates/types/src/assignability.rs::key_erased`,
`crates/types/src/assignability.rs::check_member_pair`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::a_nested_types_key_bound_does_not_count_for_a_non_key_member`,
`crates/types/src/assignability.rs::tests::mutable_struct_member_id_matching`

**Status:** done — The member types are compared as `KeyErased` (`assignability.rs::key_erased`): the length of a key member of a nested type does not count for a non-key member.

### §7.2.4.4.8 Table 19 — STRUCTURE: non-optional must-understand members on both sides

**Spec:** §7.2.4.4.8, Table 19, p. 84 — "Members for which both optional is
false and must_understand is true in either T1 or T2 appear (i.e., have a
corresponding member of the same member ID) in both T1 and T2."

**Repo:** `crates/types/src/assignability.rs::must_not_be_missing` (l. 1054);
`crates/types/src/assignability.rs::check_appendable_structs` (l. 1414);
`crates/types/src/assignability.rs::check_mutable_structs` (l. 1474)

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::an_optional_must_understand_member_may_be_missing`,
`crates/types/src/assignability.rs::tests::an_appendable_tail_member_that_must_be_understood_is_no`,
`crates/types/src/assignability.rs::tests::a_mutable_reader_member_the_writer_lacks_is_yes_unless_it_must_be_understood`,
`crates/types/src/assignability.rs::tests::a_mutable_writer_member_the_reader_lacks_and_must_understand_is_no`

**Status:** done — `must_not_be_missing` reads `IS_OPTIONAL`: a member `@optional @must_understand` that is missing on one side leaves the pair assignable.

### §7.2.4.4.8 Table 19 — STRUCTURE: an optional must-understand member may be missing

**Spec:** §7.2.4.4.8, Table 19, p. 84 — "both optional is false and
must_understand is true".

**Repo:** `crates/types/src/assignability.rs::must_not_be_missing`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::an_optional_must_understand_member_may_be_missing`

**Status:** done — `must_not_be_missing` reads `IS_OPTIONAL`.

### §7.2.4.4.8 Table 19 — STRUCTURE: key members on both sides

**Spec:** §7.2.4.4.8, Table 19, p. 84 — "Members marked as key in either T1 or
T2 appear (i.e., have a corresponding member of the same member ID) in both T1
and T2."

**Repo:** `crates/types/src/assignability.rs::must_not_be_missing`

**Tests:** `crates/types/tests/type_system_rules.rs::a_key_member_that_one_side_lacks_makes_the_structures_not_assignable`,
`crates/types/tests/table19_key_members.rs::a_writer_enum_key_literal_the_reader_lacks_is_not_assignable`,
`crates/types/src/assignability.rs::tests::a_mutable_writer_member_the_reader_lacks_and_must_understand_is_no`

**Status:** `done` — a key member that one side lacks (reader or writer) makes the structures not assignable; a member present on both sides and a key on one only does not.

### §7.2.4.4.8 Table 19 — STRUCTURE: missing key member, no test

**Spec:** §7.2.4.4.8, Table 19, p. 84 — key members on both sides.

**Repo:** `crates/types/src/assignability.rs::must_not_be_missing`

**Tests:** `crates/types/tests/type_system_rules.rs::a_key_member_that_one_side_lacks_makes_the_structures_not_assignable`

**Status:** `done` — the test leaves the key member out on the reader side and on the writer side.

### §7.2.4.4.8 Table 19 — STRUCTURE: string key m1.length >= m2.length

**Spec:** §7.2.4.4.8, Table 19, p. 84 — "For any string key member m2 in T2, the
m1 member of T1 with the same member ID verifies m1.type.length >=
m2.type.length."

**Repo:** `crates/types/src/assignability.rs::key_members_fit` (l. 1260, check l. 1301–1308)

**Tests:** `crates/types/tests/table19_key_members.rs::a_shorter_string_key_in_the_reader_is_not_assignable`,
`crates/types/tests/table19_key_members.rs::a_shorter_string_member_that_is_not_a_key_is_still_assignable`

**Status:** done — can be switched off through `KeyMemberBounds`; the default is
the rule of the specification. The OMG test suite expects the opposite for four
`struct_key_*` cases (`docs/omg-xtypes-issues.md` §C1).

### §7.2.4.4.8 Table 19 — STRUCTURE: enumerated key: all literals of m2 in m1

**Spec:** §7.2.4.4.8, Table 19, p. 84 — "For any enumerated key member m2 in T2,
the m1 member of T1 with the same member ID verifies that all literals in
m2.type appear as literals in m1.type."

**Repo:** `crates/types/src/assignability.rs::key_members_fit` (l. 1260, check l. 1313–1329)

**Tests:** `crates/types/tests/table19_key_members.rs::a_writer_enum_key_literal_the_reader_lacks_is_not_assignable`

**Status:** done — the comparison goes by the value of the literal.

### §7.2.4.4.8 Table 19 — STRUCTURE: sequence/map key m1.length >= m2.length

**Spec:** §7.2.4.4.8, Table 19, p. 84 — "For any sequence or map key member m2
in T2, the m1 member of T1 with the same member ID verifies m1.type.length >=
m2.type.length."

**Repo:** `crates/types/src/assignability.rs::key_members_fit` (l. 1260);
`crates/types/src/assignability.rs::key_length_of` (l. 1198)

**Tests:** `crates/types/tests/table19_key_members.rs::a_shorter_sequence_key_in_the_reader_is_not_assignable`

**Status:** done — a map is not permitted as a key member by §7.2.2.4.4.4.8; the
check covers it nevertheless.

### §7.2.4.4.8 Table 19 — STRUCTURE: structure or union key through KeyHolder

**Spec:** §7.2.4.4.8, Table 19, p. 84 — "For any structure or union key member
m2 in T2, the m1 member of T1 with the same member ID verifies that
KeyHolder(m1.type) is-assignable-from KeyHolder(m2.type)."

**Repo:** `crates/types/src/assignability.rs::key_holder_assignable`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::a_nested_types_key_bound_does_not_count_for_a_non_key_member`,
`crates/types/tests/strong_assignability_and_table19.rs::every_value_of_a_union_key_member_must_select_a_member_in_the_reader`

**Status:** done — A key member of structure or union type is compared through `KeyHolder` (`key_holder_struct`, `key_holder_assignable`).

### §7.2.4.4.8 Table 19 — STRUCTURE: union key, KeyHolder per selected member

**Spec:** §7.2.4.4.8, Table 19, p. 84–85 — "For any union key member m2 in T2,
the m1 member of T1 with the same member ID verifies that: For every
discriminator value of m2.type that selects a member m22 in m2.type, the
discriminator value selects a member m11 in m1.type that verifies
KeyHolder(m11.type) is-assignable-from KeyHolder(m22.type)."

**Repo:** `crates/types/src/assignability.rs::union_key_values_fit`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::every_value_of_a_union_key_member_must_select_a_member_in_the_reader`

**Status:** done — For a key member of union type, every value of the writer's discriminator must select a member in the reader (`union_key_values_fit`).

### §7.2.4.4.8 Table 19 — STRUCTURE: APPENDABLE — same index, same ID, optional, strongly assignable

**Spec:** §7.2.4.4.8, Table 19, p. 85 — "AND if T1 is appendable, then members
with the same member_index have the same member ID, the same setting for the
'optional' attribute and the T1 member type is strongly assignable from the T2
member type."

**Repo:** `crates/types/src/assignability.rs::check_appendable_structs` (l. 1414)

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::members_of_appendable_and_final_types_are_strongly_assignable`,
`crates/types/tests/strong_assignability_and_table19.rs::members_of_the_same_index_agree_on_optional`,
`crates/types/src/assignability.rs::tests::appendable_struct_with_extra_writer_field_is_assignable`,
`crates/types/src/assignability.rs::tests::appendable_struct_reader_longer_than_writer_is_yes`

**Status:** done — Member ID, type (strongly assignable) and `optional` at the same index are compared.

### §7.2.4.4.8 Table 19 — STRUCTURE: APPENDABLE and FINAL — optional is compared

**Spec:** §7.2.4.4.8, Table 19, p. 85 — "the same setting for the 'optional'
attribute".

**Repo:** —

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::members_of_the_same_index_agree_on_optional`

**Status:** done — `check_appendable_structs` and `check_final_structs` compare `optional` of the members at the same index.

### §7.2.4.4.8 Table 19 — STRUCTURE: FINAL — in addition the same set of IDs

**Spec:** §7.2.4.4.8, Table 19, p. 85 — "AND if T1 is final, then they meet the
same condition as for T1 being appendable and in addition T1 and T2 have the
same set of member IDs."

**Repo:** `crates/types/src/assignability.rs::check_final_structs` (l. 1370)

**Tests:** `crates/types/src/assignability.rs::tests::final_struct_with_extra_writer_field_is_not_assignable`,
`crates/types/src/assignability.rs::tests::final_struct_member_count_mismatch_is_no`,
`crates/types/tests/strong_assignability_and_table19.rs::members_of_appendable_and_final_types_are_strongly_assignable`

**Status:** done — Same count, same ID per position, same `optional` and strong assignability are checked.

### §7.2.4.4.8 Table 19 — STRUCTURE: base type members expanded

**Spec:** §7.2.4.4.8, Table 19, p. 85 — "For the purposes of the above
conditions, members belonging to base types of T1 or T2 shall be considered
'expanded' inside T1 or T2 respectively, as if they had been directly defined as
part of the sub-type."

**Repo:** `crates/types/src/assignability.rs::check_minimal_types` (l. 725, flattening l. 757–780);
`crates/types/src/assignability.rs::flatten_inheritance` (l. 58)

**Tests:** `crates/types/src/assignability.rs::tests::two_level_inheritance_assignability_chain`,
`crates/types/src/assignability.rs::tests::flat_type_construction_two_levels`

**Status:** done

### §7.2.4.4.8 Table 19 — STRUCTURE: construction

**Spec:** §7.2.4.4.8, Table 19, p. 83–84 — every member of T1 takes the value of
the T2 member with the same ID or the same name; non-optional missing members
take their default, optional ones no value; a present must-understand member of
T2 without a counterpart in T1 prevents the construction, independent of
TryConstruct; a member that cannot be constructed follows its TryConstruct.

**Repo:** `crates/wire-codec/src/decode.rs` (l. 1485–1498, l. 1573–1589);
`crates/wire-codec/src/default.rs::member` (l. 100)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_unknown_mutable_member_is_skipped_and_a_must_understand_one_is_not`,
`crates/lang-rules/tests/c_end_to_end.rs::a_mutable_member_that_did_not_arrive_takes_its_default`,
`crates/wire-probe/tests/try_construct.rs::use_default_substitutes_table_nines_value_and_keeps_the_sample`

**Status:** done

### §7.2.4.4.8 Note on the key rules

**Spec:** §7.2.4.4.8, Table 19, p. 85 — "Note: The rules regarding key members
ensure that the key of T2 can be transformed faithfully into the key of T1
without aliasing or loss of information."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale of the key rules.

### §7.2.4.4.8.1 Example: Type Truncation

**Spec:** §7.2.4.4.8.1, p. 85–86 — `Coordinate2D` is assignable from
`Coordinate3D`.

**Repo:** `crates/types/src/assignability.rs::check_appendable_structs`

**Tests:** `crates/types/tests/type_system_rules.rs::coordinate2d_is_assignable_from_coordinate3d`

**Status:** `done` — `Coordinate2D` is assignable from `Coordinate3D` (the example's IDL, unannotated, hence `@appendable`).

### §7.2.4.4.8.2 Example: Type Inheritance

**Spec:** §7.2.4.4.8.2, p. 86 — "LandVehicle is assignable from Vehicle. Any
consumer of the latter that receives an instance of the former will observe the
value of the member km_per_hour and ignore the member num_wheels."

**Repo:** `crates/types/src/assignability.rs::flatten_inheritance`

**Tests:** `crates/types/tests/type_system_rules.rs::a_structure_and_its_derived_structure_are_assignable`

**Status:** `done` — `Vehicle` and `LandVehicle` are assignable in both directions (the first sentence of the example swaps the direction, the second names it correctly; findings file).

### §7.2.4.4.8.3 Example: Type Refactoring

**Spec:** §7.2.4.4.8.3, p. 86–87 — `Giraffe` before and after introducing
`Mammal` are mutually assignable, because the members are compared expanded.

**Repo:** `crates/types/src/assignability.rs::flatten_inheritance`

**Tests:** `crates/types/tests/type_system_rules.rs::both_versions_of_giraffe_are_assignable_to_one_another`

**Status:** `done` — both versions of `Giraffe` are mutually assignable, `Giraffe` and `Snake` are not.

---

## §7.3 Type Representation
### §7.3 Purpose and Table 20

**Spec:** §7.3, p. 87–89 — "This specification introduces multiple equivalent
Type Representations. [...] These representations are all equivalent because
they describe the same Type System." Table 20 compares IDL, TypeObject, XML and
XSD.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale; the four representations are listed
one by one under §7.3.1 to §7.3.4.

---

## §7.3.1 IDL Type Representation

### §7.3.1.1 IDL Compatibility — two aspects

**Spec:** §7.3.1.1, p. 89 — backward compatibility of existing type definitions
and forward compatibility with IDL compilers without XTypes.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure; the requirements follow in
§7.3.1.1.1 and §7.3.1.1.2.

### §7.3.1.1.1 Extensible DDS Profile of IDL 4.2

**Spec:** §7.3.1.1.1, p. 90 — "it uses the Extensible DDS Profile (Sub Clause
9.3.2 [IDL]), which is composed of [...] Core Data Types [...] Extended Data
Types [...] Anonymous Types [...] Annotations [...] General Purpose [...] Data
Modeling [...] Data Implementation [...] Code Generation".

**Repo:** `crates/idl-parse/src/parser.rs`;
`crates/idl-lower/src/lower.rs::lower` (l. 358)

**Tests:** `crates/idl-lower/tests/conformance.rs::the_xtypes_specifications_own_idl_lowers_completely`,
`crates/idl-lower/tests/conformance.rs::every_fixture_lowers_to_exactly_the_diagnostics_it_should`

**Status:** done — the building blocks themselves are listed in `idl-4.2.md`.

### §7.3.1.1.1 Constructs outside the profile: ignore or warn

**Spec:** §7.3.1.1.1, p. 90 — "These constructs remain legal for use in IDL
files provided to IDL compilers compliant with this specification. However,
their meanings are undefined with respect to this specification. Compilers that
do not support them shall ignore them or issue a warning rather than halting
with an error."

**Repo:** `crates/idl-lower/src/lower.rs::lower`,
`crates/ast/src/profile/outside.rs::drop_outside_profile`,
`tools/idlc/src/main.rs::run_generate`,
`crates/zerodds-build/src/lib.rs::leave_out_what_xtypes_has_no_type_for`

**Tests:** `crates/idl-lower/tests/lowering.rs::an_any_member_reaches_the_model_as_a_type_rather_than_a_broken_reference`,
`crates/idl-lower/tests/lowering.rs::a_native_declaration_becomes_a_native_type`,
`crates/idl-lower/tests/outside_profile.rs::what_holds_an_any_is_dropped_and_the_rest_of_the_file_stays`,
`tools/idlc/tests/outside_profile.rs::the_rest_of_the_file_is_generated_and_the_two_declarations_are_named`,
`tools/idlc/tests/outside_profile.rs::deny_warnings_turns_it_back_into_a_failure`

**Status:** done — The IDL front end takes `any`, `native`, `valuetype` and interfaces into the model without stopping the run. `zerodds-idlc generate` and `zerodds-build` leave out a declaration that reaches `any`, `ValueBase`, `native` or an object reference (`Ast::drop_outside_profile`), report it as a warning with name, construct and paragraph, and generate the rest of the file. `--deny-warnings` makes it an error again.

### §7.3.1.1.1 Constructs outside the profile: leaving out and reporting instead of refusing

**Spec:** §7.3.1.1.1, p. 90 — "shall ignore them or issue a warning rather than
halting with an error."

**Repo:** —

**Tests:** `crates/idl-lower/tests/outside_profile.rs::what_holds_an_any_is_dropped_and_the_rest_of_the_file_stays`,
`crates/idl-lower/tests/outside_profile.rs::the_report_names_the_construct_and_the_clause`,
`crates/idl-lower/tests/outside_profile.rs::native_and_an_object_reference_are_dropped_too`,
`crates/idl-lower/tests/outside_profile.rs::a_type_an_operation_names_is_kept`,
`crates/idl-lower/tests/outside_profile.rs::a_profile_that_admits_any_keeps_it`,
`crates/zerodds-build/src/lib.rs::tests::a_struct_holding_an_any_is_left_out_and_the_rest_of_the_unit_builds`,
`crates/ast-layout/tests/layout.rs::any_is_refused_at_planning_and_the_reason_names_the_construct`

**Status:** done — `Ast::drop_outside_profile(Profile::XTypes13)` removes every data type that reaches `any`, `ValueBase`, `native` or an object reference (member, case, alias, collection element, map key and value, base, discriminator), and reports it. A type that a constant, an operation, a parameter, an attribute, an annotation member or a topic names stays in the model; for it the layout planner refuses the construct by name as before (`LayoutError::UnsupportedForEncoding`).

### §7.3.1.1.2 #pragma dds_xtopics begin and end

**Spec:** §7.3.1.1.2, p. 90 — "#pragma dds_xtopics begin [<version_number>] //
IDL definitions #pragma dds_xtopics end [<version_number>]".

**Repo:** `crates/idl-lower/src/lower.rs::dds_xtopics` (l. 825)

**Tests:** `crates/idl-lower/tests/dialects.rs::the_clauses_own_example_is_read_without_a_word`,
`crates/idl-lower/tests/dialects.rs::a_dds_xtopics_region_is_kept_verbatim_and_reaches_no_declaration`,
`crates/idl-lower/tests/dialects.rs::a_dds_xtopics_that_is_neither_begin_nor_end_is_reported`

**Status:** done

### §7.3.1.1.2 Version number case-insensitive, spaces as underscores

**Spec:** §7.3.1.1.2, p. 91 — "It shall be interpreted without respect to case,
and any spaces (for example, in '1.0 Beta 1') shall be replaced with
underscores."

**Repo:** `crates/idl-lower/src/lower.rs::xtopics_version` (l. 4451)

**Tests:** `crates/idl-lower/tests/dialects.rs::the_clauses_own_example_is_read_without_a_word`

**Status:** done

### §7.3.1.1.2 Nesting: the innermost version applies, end version as begin

**Spec:** §7.3.1.1.2, p. 91 — "In the event that such pragma declarations are
nested within one another, the innermost version number specified, if any, shall
be in effect. If version numbers are used with 'end' declarations, those version
numbers should be the same as those used with the matching 'begin'
declarations."

**Repo:** `crates/idl-lower/src/lower.rs::dds_xtopics` (l. 825)

**Tests:** `crates/idl-lower/tests/dialects.rs::the_inner_end_closes_the_inner_begin`,
`crates/idl-lower/tests/dialects.rs::a_version_that_does_not_match_its_begin_is_reported`,
`crates/idl-lower/tests/dialects.rs::an_end_with_no_begin_is_reported`

**Status:** done

### §7.3.1.1.2 begin without end runs to the end of the input

**Spec:** §7.3.1.1.2, p. 91 — "In the event that such a pragma 'begin'
declaration is not matched with a subsequent closing 'end' declaration, the
'begin' declaration shall be considered to continue until the end of the IDL
input."

**Repo:** `crates/idl-lower/src/lower.rs::dds_xtopics` (l. 825)

**Tests:** `crates/idl-lower/tests/dialects.rs::a_begin_with_no_end_runs_to_the_end_of_the_input`

**Status:** done

### §7.3.1.1.2 Behaviour of the compiler unspecified

**Spec:** §7.3.1.1.2, p. 91 — "The above declarations are informative only. The
behavior of an IDL compiler upon encountering them is unspecified".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the specification itself calls the
declarations "informative only".

### §7.3.1.2 Annotation Language

**Spec:** §7.3.1.2, p. 91 — "This specification makes use of different standard
annotation groups defined in [IDL]. It also proposes an alternative annotation
syntax for pre-existing IDL compilers."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction.

### §7.3.1.2.1 Applicability to constructed types, bitmask constants, literals, members

**Spec:** §7.3.1.2.1, p. 92 — "This specification restricts the applicability of
annotations to constructed types, bitmask constants, enumerated type literals,
and members of aggregated types."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_annotations` (l. 295)

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_outside_its_row_of_table_21_is_refused`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_on_its_row_of_table_21_is_accepted`

**Status:** done — the annotations reach types, members, literals and bits; a built-in annotation outside its row of Table 21 is refused (own items under §7.3.1.2.2, all `done`).

### §7.3.1.2.1.1 Member IDs: @id or @hashid, at most one

**Spec:** §7.3.1.2.1.1, p. 92 — "To assign the member ID explicitly the member
shall have either the @id or the @hashid annotation. At most one of these two
annotations may appear on a member. Otherwise it shall be considered an error."

**Repo:** `crates/idl-lower/src/annotation.rs::reject_two_member_ids` (l. 312)

**Tests:** `crates/idl-lower/tests/annotations.rs::an_id_and_a_hashid_on_one_member_are_rejected`

**Status:** done

### §7.3.1.2.1.1 @id sets the member ID to the value

**Spec:** §7.3.1.2.1.1, p. 92 — "The @id annotation [...] shall assign the
member ID to the value specified in the annotation parameter."

**Repo:** `crates/idl-lower/src/annotation.rs::id_annotation` (l. 582)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_declared_id_records_that_it_was_declared`,
`crates/idl-lower/tests/annotations.rs::an_id_that_is_not_an_integer_is_reported_rather_than_ignored`

**Status:** done — for the value range see §7.2.2.4.4.4.4.

### §7.3.1.2.1.1 @hashid: from the member name or the parameter, an empty parameter means the member name

**Spec:** §7.3.1.2.1.1, p. 92 — "@annotation hashid { string value default \"\";
}; [...] If the annotation is used without any parameter or with the empty
string as a parameter, then the Member ID shall be computed from the member
name."

**Repo:** `crates/idl-lower/src/annotation.rs::hashid_annotation` (l. 613)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_hashid_records_the_name_it_hashes`,
`crates/idl-lower/tests/annotations.rs::an_empty_hashid_argument_is_the_member_name`,
`crates/idl-lower/tests/completion.rs::a_hashid_uses_the_name_it_names_rather_than_the_members_own`

**Status:** done

### §7.3.1.2.1.1 Explicit ID before automatic

**Spec:** §7.3.1.2.1.1, p. 92 — "The explicit assignment of member IDs takes
precedence over the automatic mechanisms."

**Repo:** `crates/idl-lower/src/complete.rs::member_ids` (l. 1051)

**Tests:** `crates/idl-lower/tests/completion.rs::a_declared_id_wins_over_the_aggregates_autoid`,
`crates/types/src/builder.rs::tests::explicit_id_wins_over_hashid`

**Status:** done

### §7.3.1.2.1.1 @autoid: SEQUENTIAL or HASH, also on containing modules

**Spec:** §7.3.1.2.1.1, p. 92 — "If there is no explicit member ID specified,
member IDs are automatically assigned either sequentially or as a hash of the
member name. The decision is controlled by the @autoid annotation [...]. If no
@autoid annotation has been specified for the type or any of the containing
modules, then the default shall be to use a sequential approach".

**Repo:** `crates/idl-lower/src/complete.rs::member_ids` (l. 1051);
`crates/idl-lower/src/annotation.rs::autoid_annotation` (l. 635)

**Tests:** `crates/idl-lower/tests/completion.rs::autoid_sequential_and_autoid_hash_differ`,
`crates/idl-lower/tests/annotations.rs::both_autoid_policies_are_distinguished`,
`crates/idl-lower/tests/annotations.rs::autoid_on_a_module_numbers_the_members_of_its_types`

**Status:** done — `@autoid` on the aggregate and on a containing module takes effect (`ast::query::effective_autoid`); without an annotation the numbering is sequential.

### §7.3.1.2.1.1 @autoid on a containing module takes effect

**Spec:** §7.3.1.2.1.1, p. 92 — "for the type or any of the containing modules".

**Repo:** `crates/ast/src/query.rs::effective_autoid`,
`crates/idl-lower/src/complete.rs::member_ids`

**Tests:** `crates/idl-lower/tests/annotations.rs::autoid_on_a_module_numbers_the_members_of_its_types`

**Status:** done — `member_ids` asks `effective_autoid`, which asks the containing modules up to the top level.

### §7.3.1.2.1.1 Sequential: previous ID plus one, first ID 0

**Spec:** §7.3.1.2.1.1, p. 92–93 — "The value of the member ID assigned to a
member using the sequential approach shall equal to the member ID of the
previous member of the same Aggregated Type plus one [...] In the case of the
first member of Aggregated Type that does not have any base type the member ID
shall be set to zero."

**Repo:** `crates/idl-lower/src/complete.rs::next_id` (l. 1144)

**Tests:** `crates/idl-lower/tests/completion.rs::members_without_annotations_number_from_zero_upwards`,
`crates/idl-lower/tests/completion.rs::a_declared_id_shifts_the_numbering_of_what_follows`

**Status:** done

### §7.3.1.2.1.1 Sequential: a derived type continues after the last base member

**Spec:** §7.3.1.2.1.1, p. 92–93 — "In the case an aggregated type inherits from
another aggregated type the first member of the derived type gets the member ID
of the last member in the base type plus one." Example 1: ID(s1_m1)=0,
ID(s1_m2)=1, ID(s2_m1)=2, ID(s3_m1)=3.

**Repo:** `crates/idl-lower/src/complete.rs::last_base_member_id` (l. 1036)

**Tests:** `crates/idl-lower/tests/completion.rs::example_1_of_the_member_id_clause_numbers_a_derived_chain_0_1_2_3`,
`crates/idl-lower/tests/validation.rs::a_declared_id_colliding_with_a_derived_one_is_rejected_and_names_both_origins`

**Status:** done — the first id of the derived type is the last id of the base chain plus one; Example 1 gives 0, 1, 2, 3.

### §7.3.1.2.1.1 Example 1 without a test

**Spec:** §7.3.1.2.1.1, p. 93 — Example 1 with three levels.

**Repo:** `crates/idl-lower/src/complete.rs::last_base_member_id`

**Tests:** `crates/idl-lower/tests/completion.rs::example_1_of_the_member_id_clause_numbers_a_derived_chain_0_1_2_3`

**Status:** done — the test lowers `MyStruct1` to `MyStruct3` with `@autoid(SEQUENTIAL)` and checks ID(s1_m1)=0, ID(s1_m2)=1, ID(s2_m1)=2, ID(s3_m1)=3.

### §7.3.1.2.1.1 Hash computation: MD5, little endian, & 0x0FFFFFFF

**Spec:** §7.3.1.2.1.1, p. 94 — "1. Compute a 4-byte hash of the string as
specified in 7.2.2.4.4.4.5. 2. Interpret the resulting 4-byte has as a Little
Endian unsigned 32-bit integer. 3. Perform a bitwise AND operation with the
integer 0x0FFFFFFF to zero the most significant 4-bits of the integer."

**Repo:** `crates/idl-lower/src/complete.rs::hash_id` (l. 1165);
`crates/types/src/type_object/common.rs::member_id_from_name` (l. 53)

**Tests:** `crates/types/src/builder.rs::tests::member_id_from_name_matches_known_md5_vectors`,
`crates/idl-lower/src/complete.rs::tests::a_hashed_id_stays_inside_the_28_bit_space`

**Status:** done

### §7.3.1.2.1.1 Example 2: color and @hashid("shapesize")

**Spec:** §7.3.1.2.1.1, p. 94 — "ID(color) = 0x0FA5DD70, ID(size) = 0x047790DA."

**Repo:** `crates/idl-lower/src/complete.rs::hash_id` (l. 1165)

**Tests:** `crates/idl-lower/tests/completion.rs::example_2_of_the_member_id_clause_gives_the_two_published_ids`,
`crates/types/src/builder.rs::tests::member_id_from_name_matches_known_md5_vectors`

**Status:** done — `@autoid(HASH)` gives `color` the id 0x0FA5DD70 and `@hashid("shapesize") long size` the id 0x047790DA.

### §7.3.1.2.1.1 Example 2: shapesize without a test

**Spec:** §7.3.1.2.1.1, p. 94 — `ID(size) = 0x047790DA`.

**Repo:** `crates/idl-lower/src/complete.rs::hash_id`

**Tests:** `crates/idl-lower/tests/completion.rs::example_2_of_the_member_id_clause_gives_the_two_published_ids`

**Status:** done — the test lowers `MyStruct1` of the example and `hashed_member_id("shapesize")`; both give 0x047790DA.

### §7.3.1.2.1.2 Optional Members: @optional

**Spec:** §7.3.1.2.1.2, p. 94 — "To declare a member optional, users shall apply
the @optional annotation [...] It is an error to declare the same member as both
optional and as a key."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `optional` l. 335);
`crates/idl-lower/src/validate.rs::member_rules` (l. 327)

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`,
`crates/idl-lower/tests/validation.rs::a_key_member_must_not_be_optional`,
`crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — `@optional` and `@optional(TRUE)` make the member optional, `@optional(FALSE)` does not; `@key @optional` is refused.

### §7.3.1.2.1.3 Key Members: @key

**Spec:** §7.3.1.2.1.3, p. 94 — "To declare a member as part of the key, users
shall apply the @key annotation [...] It is an error to declare the same member
as both optional and as a key."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `key` l. 334);
`crates/idl-lower/src/validate.rs::member_rules` (l. 327)

**Tests:** `crates/idl-lower/tests/validation.rs::a_key_member_must_not_be_optional`,
`crates/idl-lower/tests/validation.rs::key_on_its_own_is_accepted`,
`crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — `@key` and `@key(TRUE)` set the key, `@key(FALSE)` does not.

### §7.3.1.2.1.4 External Data: member always present, key permitted

**Spec:** §7.3.1.2.1.4, p. 94 — "Unless also annotated as Optional, external
members shall always be present and therefore the pointer [...] to non-optional
external members cannot be NULL. Non-optional external members can be annotated
as Key."

**Repo:** `crates/lang-rules/tests/external_is_an_indirection.rs`

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_non_optional_external_member_gets_table_9_behind_the_indirection`,
`crates/lang-rules/tests/d_end_to_end.rs::an_optional_external_member_is_one_pointer_and_not_a_nullable_pointer`

**Status:** done — a non-optional `@external` member is a non-empty indirection
in the generated code. A test with `@key @external` is missing; the validation
rules do not refuse the combination.

### §7.3.1.2.1.4 External: no preservation of graphs, diamond, cycle

**Spec:** §7.3.1.2.1.4, p. 95 — "If a conforming implementation encounters a
graph [...], it is not required to maintain the graph structure through
serialization/deserialization." Non-normative note on tree, diamond, cycle.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement on an implementation; the note
is "non-normative".

### §7.3.1.2.1.4 External: syntax on the member, also as suffix

**Spec:** §7.3.1.2.1.4, p. 95 — "@external long my_aggregation_member; or: long
my_aggregation_member; //@external".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `external` l. 336);
`crates/idl-parse/src/suffix.rs`

**Tests:** `crates/idl-lower/tests/validation.rs::external_on_a_member_and_on_a_branch_is_accepted`,
`crates/idl-lower/tests/annotations.rs::the_prefix_and_the_suffix_form_produce_the_same_model`

**Status:** done

### §7.3.1.2.1.4 External: syntax for elements of sequence, array, map

**Spec:** §7.3.1.2.1.4, p. 95 — "sequence<@external Foo, 42> sequence_of_foo;
[...] Foo array_of_foo @external [42]; [...] map<string, @external Foo, 42>
map_of_string_to_foo;"

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::external_on_an_element_a_map_key_or_a_map_value_is_carried`,
`crates/idl-lower/tests/annotations.rs::external_after_an_array_declarators_name_is_the_elements`,
`crates/idl-lower/tests/annotations.rs::an_annotation_after_the_name_of_a_declarator_with_no_dimensions_is_reported`

**Status:** done — `@external` on an element of a sequence, on a map key and a map value and the array form `Foo a @external [42]` are read; the annotation after the name applies to the elements of the array, a declarator with no dimensions reports it.

### §7.3.1.2.1.5 Enumerated Literal Values: @bit_bound 1 to 32 on enumerations

**Spec:** §7.3.1.2.1.5, p. 95 — "the value member of the annotation may take any
value from 1 to 32, inclusive, when this annotation is applied to an enumerated
type."

**Repo:** `crates/idl-lower/src/annotation.rs::bit_bound_annotation` (l. 710);
`crates/ast/src/query.rs::effective_bit_bound` (l. 319)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_bit_bound_reaches_the_type`,
`crates/idl-lower/tests/completion.rs::an_enum_without_a_bit_bound_is_thirty_two_bits`,
`crates/idl-lower/tests/annotations.rs::an_enumerations_bit_bound_is_from_1_to_32`

**Status:** done — `@bit_bound` reaches the enumeration, the default is 32; a value outside 1 to 32 on an enumeration is refused (`complete.rs::check_enumeration_bound`).

### §7.3.1.2.1.5 @bit_bound on an enumeration outside 1 to 32 is refused

**Spec:** §7.3.1.2.1.5, p. 95 — value range 1 to 32.

**Repo:** `crates/idl-lower/src/complete.rs::check_enumeration_bound`

**Tests:** `crates/idl-lower/tests/annotations.rs::an_enumerations_bit_bound_is_from_1_to_32`

**Status:** done — `@bit_bound(0)` and `@bit_bound(48)` on an `enum` are refused.

### §7.3.1.2.1.5 @value and implicit progression

**Spec:** §7.3.1.2.1.5, p. 95 — "this specification allows enumerated literals
to be given explicit custom values [...] by means of the @value annotation [...]
implicit values are assigned in a progression starting from the most-recently
specified value (or an implicit value of zero for the first literal, if there is
no previous specified value) and adding one with each successive literal."

**Repo:** `crates/idl-lower/src/annotation.rs::value_annotation` (l. 660);
`crates/ast/src/query.rs::effective_enumerator_value` (l. 559)

**Tests:** `crates/idl-lower/tests/completion.rs::enumerators_without_values_run_from_zero_upwards`,
`crates/idl-lower/tests/completion.rs::an_explicit_enumerator_value_shifts_the_ones_after_it`,
`crates/idl-lower/tests/annotations.rs::an_explicit_enumerator_value_keeps_the_expression_it_was_written_as`

**Status:** done

### §7.3.1.2.1.6 Bitmask Positions: size 32, @bit_bound up to 64, @position

**Spec:** §7.3.1.2.1.6, p. 96 — "By default, the size of a bit mask is 32-bit.
This behavior may be amended with the use of the @bit_bound annotation, which
may set the size of the whole bit mask to a value lower or equal to 64 [...] a
bit value may be set explicitly by means of the @position annotation".

**Repo:** `crates/idl-lower/src/complete.rs::check_bitmask_bounds` (l. 176);
`crates/idl-lower/src/annotation.rs::value_annotation` (l. 660, arm `BitValue`)

**Tests:** `crates/idl-lower/tests/annotations.rs::an_explicit_bit_position_reaches_the_bit`,
`crates/idl-lower/tests/validation.rs::a_bit_bound_over_sixty_four_is_refused`,
`crates/idl-lower/tests/completion.rs::bit_positions_run_from_zero_and_an_explicit_one_shifts_the_rest`

**Status:** done

### §7.3.1.2.1.7 Nested Types: not nested by default, @nested

**Spec:** §7.3.1.2.1.7, p. 96 — "By default, aggregated types and aliases to
aggregated types defined in IDL are not considered to be nested types. This
designation may be changed by applying the IDL @nested annotation to a type
definition".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `nested` l. 339);
`crates/ast/src/query.rs::effective_nested` (l. 484)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_nested_annotation_reaches_the_type`,
`crates/idl-lower/tests/annotations.rs::nested_false_and_topic_override_the_default_nested_of_the_module`

**Status:** done — `@nested`, `@nested(TRUE)` and `@nested(FALSE)` are told apart: the model carries `nested_explicit` beside `is_nested`, an own `false` takes precedence over the default of the module.

### §7.3.1.2.1.7 @nested(FALSE) in a module with @default_nested(TRUE) holds

**Spec:** §7.3.1.2.1.7, p. 96 — `@nested` on the type definition changes the
designation.

**Repo:** `crates/ast/src/query.rs::effective_nested`

**Tests:** `crates/idl-lower/tests/annotations.rs::nested_false_and_topic_override_the_default_nested_of_the_module`

**Status:** done — `effective_nested` gives an own `false` precedence over `@default_nested(TRUE)` of the module.

### §7.3.1.2.1.7 @default_nested on modules, inherited from the enclosing module, default FALSE

**Spec:** §7.3.1.2.1.7, p. 96 — "@annotation default_nested { boolean value
default TRUE; }; The @default_nested annotation may be applied to modules. If
not present on a module, the value defaults to that of the enclosing module. If
a top-level module is not annotated, the default is FALSE."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `default_nested` l. 347);
`crates/ast/src/query.rs::effective_nested` (l. 484)

**Tests:** `crates/idl-lower/tests/annotations.rs::default_nested_on_a_module_reaches_the_types_in_it`

**Status:** done

### §7.3.1.2.1.7 Compiler option for the default of non-annotated top-level modules holds

**Spec:** §7.3.1.2.1.7, p. 96 — "In addition to the above annotations, IDL
compilers shall provide the means to change the default value for non-annotated
top-level modules."

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::the_default_for_unannotated_top_level_modules_is_an_option`

**Status:** done — `Options::default_nested` (compiler option) sets the default for non-annotated top-level modules.

### §7.3.1.2.1.8 @extensibility on aggregates

**Spec:** §7.3.1.2.1.8, p. 96 — "The extensibility kind of a type may be defined
by means of a @extensibility annotation [...] This annotation may be applied to
the definitions of aggregated types."

**Repo:** `crates/idl-lower/src/annotation.rs::extensibility_argument` (l. 879)

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/idl-lower/tests/annotations.rs::an_extensibility_argument_that_is_not_one_of_the_three_is_reported`

**Status:** done — for the application to other kinds of type see Table 21.

### §7.3.1.2.1.8 @extensibility applied multiple times is an error and is reported

**Spec:** §7.3.1.2.1.8, p. 96 — "It shall be considered an error for it to be
applied to the same type multiple times."

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::the_extensibility_is_given_once`

**Status:** done — A second `@extensibility`, `@final`, `@appendable` or `@mutable` on a type is reported as an error.

### §7.3.1.2.1.8 Appendable when unspecified, overridable

**Spec:** §7.3.1.2.1.8, p. 96 — "In the event that the representation of a given
type does not indicate the type's extensibility kind, the type shall be
considered appendable. Implementations may provide a mechanism to override this
default behavior".

**Repo:** `crates/ast/src/lib.rs::DEFAULT_EXTENSIBILITY` (l. 62);
`crates/idl-lower/src/lower.rs::Options` (l. 50, field `default_extensibility` l. 73);
`crates/ast-layout/src/key.rs::default_extensibility` (l. 5640)

**Tests:** `crates/idl-lower/tests/completion.rs::a_type_that_said_nothing_is_appendable_by_rule_and_silent_in_the_facet`,
`crates/idl-lower/tests/completion.rs::the_callers_default_extensibility_does_not_override_a_bases`

**Status:** done — the default is APPENDABLE; the `Cyclone` dialect of the
layout planner takes FINAL. Which default applied cannot be seen on the wire
(`docs/omg-xtypes-issues.md` §B6).

### §7.3.1.2.1.8 Shortcuts @final, @appendable, @mutable

**Spec:** §7.3.1.2.1.8, p. 96 — "IDL compilers shall also implement the shortcut
annotations for the different extensibility kinds. That is, @final, @appendable,
and @mutable".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arms l. 353–355)

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/idl-lower/tests/annotations.rs::every_spelling_of_one_builtin_says_the_same_thing`

**Status:** done

### §7.3.1.2.1.9 @must_understand

**Spec:** §7.3.1.2.1.9, p. 97 — "If the @must_understand annotation is set to
true in particular member M2 of a type T2, then the assignment to an object of
type T1 shall fail if the type T1 does not define such a member."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `must_understand` l. 337);
`crates/types/src/assignability.rs::check_mutable_structs` (l. 1474)

**Tests:** `crates/types/src/assignability.rs::tests::a_mutable_writer_member_the_reader_lacks_and_must_understand_is_no`,
`crates/wire-conformance/tests/bytes.rs::an_unknown_mutable_member_is_skipped_and_a_must_understand_one_is_not`,
`crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — acts in assignability and when reading; `@must_understand(FALSE)` does not set the attribute.

### §7.3.1.2.1.10 @default_literal

**Spec:** §7.3.1.2.1.10, p. 97 — "The annotation @default_literal allows this
behavior to be changed. @annotation default_literal {};"

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `default_literal` l. 351);
`crates/types/src/from_ast.rs::enumeration` (l. 2361)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_default_literal_marks_the_enumerator_and_not_the_enum`,
`crates/lang-rules/src/default_python.rs::tests::an_enumeration_with_a_default_literal_takes_that_one`

**Status:** done

### §7.3.1.2.1.11 @ignore_literal_names: absent FALSE, without parameter TRUE

**Spec:** §7.3.1.2.1.11, p. 97 — "@annotation ignore_literal_names { boolean
value default TRUE; }; If the @ignore_literal_names annotation is not present it
is interpreted as having the value FALSE. If it is present without a parameter,
it is interpreted as having the value TRUE."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `ignore_literal_names` l. 352)

**Tests:** `crates/types/tests/table19_key_members.rs::ignore_literal_names_needs_the_annotation_on_both_types`,
`crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — absent is FALSE, without a parameter and with TRUE it is TRUE, `@ignore_literal_names(FALSE)` is FALSE.

### §7.3.1.2.1.11 @ignore_literal_names: TRUE on both sides

**Spec:** §7.3.1.2.1.11, p. 97 — "If the @ignore_literal_names annotation has
the value TRUE in both enumerated types, then the literal names are ignored for
the purpose of checking the assignability of the two Enumerated Types."

**Repo:** `crates/types/src/assignability.rs::check_enumerations` (l. 964, l. 1005–1007);
`crates/types/src/from_ast.rs::enumeration` (l. 2361)

**Tests:** `crates/types/tests/table19_key_members.rs::ignore_literal_names_needs_the_annotation_on_both_types`,
`crates/types/src/assignability.rs::tests::one_sided_ignore_literal_names_does_not_drop_the_name_rule`

**Status:** done — the introduction of the clause says Table 18 requires equal
default literals; Table 18 does not say so (contradiction in the specification,
listed in the findings file).

### §7.3.1.2.1.12 @try_construct: declaration and default USE_DEFAULT

**Spec:** §7.3.1.2.1.12, p. 98–99 — "enum TryConstructFailAction { DISCARD,
USE_DEFAULT, TRIM }; @annotation try_construct { TryConstructFailAction value
default USE_DEFAULT; }; [...] if the @try_construct annotation is not used it is
the same as if it had been explicitly set to DISCARD [...] If the annotation is
specified without a value [...] then the behavior is set to DEFAULT".

**Repo:** `crates/idl-lower/src/annotation.rs::try_construct_behaviour` (l. 746);
`crates/ast/src/facet/xtypes.rs::BARE` (l. 99)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_bare_try_construct_is_use_default_and_an_absent_one_is_discard`,
`crates/idl-lower/tests/annotations.rs::each_try_construct_behaviour_is_distinguished`,
`crates/wire-probe/tests/try_construct.rs::a_bare_annotation_is_use_default_and_not_discard`

**Status:** done

### §7.3.1.2.1.12 @try_construct: permitted positions

**Spec:** §7.3.1.2.1.12, p. 99 — "the TryConstruct annotation may be used in
structure and union members, the union discriminator, the elements of arrays and
sequences, and the key and/or values of map types."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_use_site` (l. 775);
`crates/ast/src/facet/xtypes.rs::XTypesFacet` (l. 242, fields `try_construct`,
`element_try_construct`,
`key_try_construct`,
`discriminator_try_construct`)

**Tests:** `crates/idl-lower/tests/annotations.rs::try_construct_reaches_every_position_the_clause_names`

**Status:** done — in addition the front end accepts `@try_construct` on a
`typedef`; a use without its own setting inherits it
(`an_alias_supplies_a_default_that_the_use_site_overrides`). Table 21 names only
structure and union members (contradiction in the specification,
`docs/omg-xtypes-issues.md` §B4).

### §7.3.1.2.1.12.1 TryConstruct Example 1

**Spec:** §7.3.1.2.1.12.1, p. 99–100 — `@try_construct(USE_DEFAULT) string<5>`
against "Hello World!": `O1.member` is the empty string.

**Repo:** `crates/wire/src/try_construct.rs::fit_string` (l. 89)

**Tests:** `crates/wire-probe/tests/try_construct.rs::use_default_substitutes_table_nines_value_and_keeps_the_sample`

**Status:** done — the example names the type `String4 (string<5>)` (naming
defect of the specification, listed in the findings file).

### §7.3.1.2.1.12.2 TryConstruct Example 2

**Spec:** §7.3.1.2.1.12.2, p. 100 — `@try_construct(TRIM) string<5>` against
"Hello World!": "Hello".

**Repo:** `crates/wire/src/try_construct.rs::fit_string` (l. 89)

**Tests:** `crates/wire-probe/tests/try_construct.rs::trim_cuts_the_string_to_the_bound_and_keeps_the_sample`

**Status:** done

### §7.3.1.2.1.12.3 TryConstruct Example 3

**Spec:** §7.3.1.2.1.12.3, p. 100 — `@try_construct(TRIM) sequence<long,4>`
against eight elements: the first four.

**Repo:** `crates/wire/src/try_construct.rs::fit_sequence` (l. 131)

**Tests:** `crates/wire-probe/tests/try_construct.rs::trim_cuts_a_sequence_to_its_bound_keeping_the_first_elements`

**Status:** done

### §7.3.1.2.1.12.4 TryConstruct Example 4

**Spec:** §7.3.1.2.1.12.4, p. 101 — `sequence<@try_construct(TRIM) String5,
10>`: the element is trimmed, the sequence stays.

**Repo:** `crates/ast/src/query.rs::element_try_construct` (l. 413)

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_element_annotation_trims_the_element_and_not_the_collection`

**Status:** done

### §7.3.1.2.1.12.5 TryConstruct Example 5

**Spec:** §7.3.1.2.1.12.5, p. 101–102 — `union T1 switch (@try_construct
T1Enum)`: a discriminator ENUM3 becomes the default ENUM2.

**Repo:** `crates/ast/src/query.rs::discriminator_try_construct` (l. 508)

**Tests:** `crates/wire-probe/tests/try_construct.rs::a_defaulted_discriminator_defaults_the_member_it_now_selects`

**Status:** done — the example ends with "O1.e1_value would be constructed from
O2.e3_value", although ENUM2 selects `e2_value` (defect of the specification,
listed in the findings file).

### §7.3.1.2.1.13 @verbatim

**Spec:** §7.3.1.2.1.13, p. 102 — "Verbatim Text objects associated with a
constructed type declaration shall be indicated using the following @verbatim
annotation defined in Sub Clause 8.3.5.1 of [IDL]."

**Repo:** `crates/idl-lower/src/annotation.rs::verbatim_annotation` (l. 934)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_verbatim_directive_keeps_its_language_placement_and_text`,
`crates/idl-lower/tests/annotations.rs::a_verbatim_directive_defaults_language_and_placement`

**Status:** done — for the placement values see §7.2.2.4.8.2.

### §7.3.1.2.1.14 @non_serialized

**Spec:** §7.3.1.2.1.14, p. 103 — "@annotation non_serialized { boolean value
default TRUE; }; It is an error to declare the same member as both
non_serialized and as a key."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330, arm `non_serialized` l. 338);
`crates/idl-lower/src/validate.rs::member_rules` (l. 327, check l. 357)

**Tests:** `crates/idl-lower/tests/validation.rs::a_key_member_must_not_be_non_serialized`,
`crates/types/tests/table19_key_members.rs::a_non_serialized_member_does_not_enter_the_comparison`,
`crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — the annotation and the refusal together with `@key`; `@non_serialized(FALSE)` does not set the attribute.

### §7.3.1.2.1.15 @data_representation: bitmask XCDR1, XML, XCDR2

**Spec:** §7.3.1.2.1.15, p. 103 — "@bit_bound(32) bitmask DataRepresentationMask
{ @position(0) XCDR1, @position(1) XML, @posiiton(2) XCDR2 } @annotation
data_representation { DataRepresentationMask allowed_kinds; };"

**Repo:** `crates/idl-lower/src/annotation.rs::data_representation_annotation` (l. 998);
`crates/ast/src/facet/xtypes.rs::DataRepresentationMask` (l. 428)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_data_representation_annotation_is_a_mask_not_one_value`,
`crates/idl-lower/tests/annotations.rs::an_unknown_data_representation_is_reported`

**Status:** done — the code accepts `XCDR1` and `XCDR`; the example of the
specification writes `XCDR | XCDR2`, the bitmask is named `XCDR1`, and
`@posiiton` is a typo (defect of the specification, listed in the findings
file).

### §7.3.1.2.1.15 @data_representation: absent means 0xffffffff

**Spec:** §7.3.1.2.1.15, p. 104 — "If the @data_representation annotation is not
present it is interpreted as if the DataRepresentationMask value was set to
0xffffffff."

**Repo:** `crates/ast/src/query.rs::data_representation_mask`;
`crates/ast/src/facet/xtypes.rs::DataRepresentationMask`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_type_without_data_representation_has_the_mask_0xffffffff`,
`crates/idl-lower/tests/annotations.rs::a_data_representation_annotation_is_a_mask_not_one_value`

**Status:** done — `Ast::data_representation_mask` gives a type with neither its own mask nor a module mask `DataRepresentationMask::ALL` (`ALL_BITS` = 0xffffffff); a declared mask stays what it says (XCDR2 = bit 2, XCDR1 = bit 0, XML = bit 1).

### §7.3.1.2.1.15 @data_representation on type and module declarations

**Spec:** §7.3.1.2.1.15, p. 104 — "This annotation may be applied to type
declarations and module declarations."

**Repo:** `crates/idl-lower/src/annotation.rs::data_representation_annotation`,
`crates/ast/src/query.rs::effective_data_representation`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_data_representation_annotation_is_a_mask_not_one_value`,
`crates/idl-lower/tests/annotations.rs::data_representation_on_a_module_is_the_default_of_its_types`

**Status:** done — The mask reaches types and modules; a type without its own mask inherits that of its module (`ast::query::effective_data_representation`).

### §7.3.1.2.1.15 @data_representation on a module takes effect on its types

**Spec:** §7.3.1.2.1.15, p. 104 — application to module declarations.

**Repo:** `crates/ast/src/query.rs::effective_data_representation`

**Tests:** `crates/idl-lower/tests/annotations.rs::data_representation_on_a_module_is_the_default_of_its_types`

**Status:** done — `effective_data_representation` reads the facet of the containing modules; the module takes effect on the types in it.

### §7.3.1.2.1.16 @topic: name, platform

**Spec:** §7.3.1.2.1.16, p. 104 — "@annotation topic { string name default \"\";
string platform default \"*\"; }; [...] The @topic annotation overrides the
@nested and @default_nested annotations."

**Repo:** `crates/idl-lower/src/annotation.rs::topic_annotation` (l. 979);
`crates/ast/src/facet/xtypes.rs::TopicAnnotation` (l. 416)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_topic_annotation_keeps_both_of_its_parameters`,
`crates/idl-lower/tests/annotations.rs::nested_false_and_topic_override_the_default_nested_of_the_module`

**Status:** done — the parameters are kept and `@topic` overrides `@nested` and `@default_nested`: a topic type is never nested.

### §7.3.1.2.1.16 @topic overrides @nested and @default_nested

**Spec:** §7.3.1.2.1.16, p. 104 — "The @topic annotation overrides the @nested
and @default_nested annotations."

**Repo:** `crates/ast/src/query.rs::effective_nested`

**Tests:** `crates/idl-lower/tests/annotations.rs::nested_false_and_topic_override_the_default_nested_of_the_module`

**Status:** done — `@topic` overrides `@nested` and `@default_nested`: a topic type is never nested (`effective_nested`).

### §7.3.1.2.1.16 @topic only on legal topic types is checked

**Spec:** §7.3.1.2.1.16, p. 104 — "The @topic annotation can only be used on
types that are legal as DDS Topic types, see 7.6.2."

**Repo:** `crates/idl-lower/src/validate.rs::topic_is_on_a_topic_type`

**Tests:** `crates/idl-lower/tests/annotations.rs::topic_on_a_type_that_is_no_topic_type_is_refused`

**Status:** done — `validate.rs::topic_is_on_a_topic_type` refuses `@topic` on a type that is no topic type (enumeration, bitmask, bitset, alias, sequence, map).

---

## §7.3.1.2.2 Using Built-in Annotations (Table 21)

### §7.3.1.2.2 Table 21 — @optional, @must_understand, @non_serialized: structure members

**Spec:** §7.3.1.2.2, Table 21, p. 104 — "@optional, @must_understand,
@non_serialized | Structure Members".

**Repo:** `crates/idl-lower/src/annotation.rs::place_of`

**Tests:** `crates/idl-lower/tests/validation.rs::no_union_member_is_optional`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_outside_its_row_of_table_21_is_refused`,
`crates/idl-lower/tests/annotations.rs::must_understand_on_a_union_branch_is_refused`

**Status:** done — `@optional`, `@must_understand` and `@non_serialized` apply to structure members (Table 21); on a union member or a type declaration the front end refuses them.

### §7.3.1.2.2 Table 21 — the place of the annotations is checked

**Spec:** §7.3.1.2.2, p. 104 — "The application of the annotations listed above
is restricted to the type elements specified in Table 21."

**Repo:** `crates/idl-lower/src/annotation.rs::place_of`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_builtin_annotation_outside_its_row_of_table_21_is_refused`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_on_its_row_of_table_21_is_accepted`

**Status:** done — `annotation.rs::place_of` assigns each built-in annotation to the places of Table 21; on another place it is an error (`@bit_bound` on a structure, `@position` on a member, `@autoid` on an enumeration).

### §7.3.1.2.2 Boolean parameters of @key, @optional, @external, @must_understand, @non_serialized, @ignore_literal_names are read

**Spec:** §7.3.1.2.1.11, p. 97, and §7.3.1.2.1.14, p. 103 — the annotations have
`boolean value default TRUE`; IDL 4.2 §8.3 declares `@key`, `@optional`,
`@external` and `@must_understand` the same way.

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — The parameters of `@key`, `@optional`, `@external`, `@must_understand`, `@non_serialized` and `@ignore_literal_names` are read: `@key(FALSE)` makes no key member. An argument that cannot be evaluated or has no value is an error.

### §7.3.1.2.2 Table 21 — @id, @hashid, @external, @try_construct: structure and union members

**Spec:** §7.3.1.2.2, Table 21, p. 104 — "@id, @hashid, @external,
@try_construct | Structure Members, Union members (except union discriminator)".

**Repo:** `crates/idl-lower/src/validate.rs::external_is_a_member_annotation` (l. 134);
`crates/idl-lower/src/annotation.rs::apply_use_site` (l. 775)

**Tests:** `crates/idl-lower/tests/validation.rs::external_on_a_type_declaration_is_refused_by_name`,
`crates/idl-lower/tests/validation.rs::external_on_a_member_and_on_a_branch_is_accepted`,
`crates/idl-lower/tests/annotations.rs::try_construct_reaches_every_position_the_clause_names`

**Status:** done — `@try_construct` is also permitted on the discriminator and
on collection elements, as §7.3.1.2.1.12 requires; Table 21 and §7.3.1.2.1.12
contradict each other (`docs/omg-xtypes-issues.md` §B4).

### §7.3.1.2.2 Table 21 — @key: structure members, union discriminator

**Spec:** §7.3.1.2.2, Table 21, p. 105 — "@key | Structure Members, Union
discriminator".

**Repo:** `crates/idl-lower/src/validate.rs::union_member_rules` (l. 768);
`crates/idl-lower/src/annotation.rs::apply_use_site` (l. 775)

**Tests:** `crates/idl-lower/tests/validation.rs::only_a_unions_discriminator_may_be_a_key`,
`crates/idl-lower/tests/annotations.rs::a_key_on_a_union_discriminator_is_carried_by_default`

**Status:** done — `@key` applies to structure members and the union discriminator; on the discriminator it is carried by default (`KeyOnUnionDiscriminator::Carried`), `Refused` stays as a setting.

### §7.3.1.2.2 Table 21 — @bit_bound: enumeration, bitmask

**Spec:** §7.3.1.2.2, Table 21, p. 105 — "@bit_bound | Enumerated Types, Bit
Mask Types".

**Repo:** `crates/idl-lower/src/annotation.rs::bit_bound_annotation` (l. 710)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_bit_bound_reaches_the_type`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_outside_its_row_of_table_21_is_refused`

**Status:** done — `@bit_bound` on an enumeration and a bitmask; on another place the front end refuses it. Extension of the project: also on an alias and a bitset (`@bit_bound(40) typedef`).

### §7.3.1.2.2 Table 21 — @extensibility, @mutable, @appendable, @final, @nested: type declarations

**Spec:** §7.3.1.2.2, Table 21, p. 105 — "@extensibility, @mutable, @appendable,
@final, @nested | Type declarations".

**Repo:** `crates/idl-lower/src/annotation.rs::set_extensibility` (l. 1036)

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/idl-lower/tests/annotations.rs::a_nested_annotation_reaches_the_type`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_outside_its_row_of_table_21_is_refused`

**Status:** done — `@extensibility`, `@mutable`, `@appendable`, `@final` and `@nested` take effect on type declarations; on members the front end refuses them.

### §7.3.1.2.2 Table 21 — @default_literal, @value: literals

**Spec:** §7.3.1.2.2, Table 21, p. 105 — "@default_literal, @value | Enumerated
Literals".

**Repo:** `crates/idl-lower/src/annotation.rs::value_annotation` (l. 660)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_default_literal_marks_the_enumerator_and_not_the_enum`,
`crates/idl-lower/tests/annotations.rs::an_explicit_enumerator_value_keeps_the_expression_it_was_written_as`

**Status:** done — `@value` on a target other than a literal or a bit is
reported (`value_annotation`, last arm); `@default_literal` at other positions
is not (open above).

### §7.3.1.2.2 Table 21 — @position: bitmask values

**Spec:** §7.3.1.2.2, Table 21, p. 105 — "@position | Bitmask Values".

**Repo:** `crates/idl-lower/src/annotation.rs::value_annotation` (l. 660, arm `BitValue`)

**Tests:** `crates/idl-lower/tests/annotations.rs::an_explicit_bit_position_reaches_the_bit`

**Status:** done — reported on another target.

### §7.3.1.2.2 Table 21 — @autoid: module, structure, union

**Spec:** §7.3.1.2.2, Table 21, p. 105 — "@autoid | Module declarations,
Structure declarations, Union declarations".

**Repo:** `crates/idl-lower/src/annotation.rs::autoid_annotation` (l. 635)

**Tests:** `crates/idl-lower/tests/annotations.rs::both_autoid_policies_are_distinguished`,
`crates/idl-lower/tests/annotations.rs::autoid_on_a_module_numbers_the_members_of_its_types`

**Status:** done — `@autoid` takes effect on structure, union and module.

### §7.3.1.2.2 Table 21 — @verbatim: all elements

**Spec:** §7.3.1.2.2, Table 21, p. 105 — "@verbatim | All elements".

**Repo:** `crates/idl-lower/src/annotation.rs::verbatim_annotation` (l. 934);
`crates/lang-rules/src/verbatim.rs::declaration_slots` (l. 129)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_verbatim_directive_on_a_member_reaches_the_member`,
`crates/lang-rules/src/verbatim.rs::tests::a_member_directive_is_hoisted_into_the_aggregate_and_taken_once`

**Status:** done

### §7.3.1.2.2 Table 21 — missing rows

**Spec:** §7.3.1.2.2, Table 21, p. 104–105 — the table does not name
`@default_nested`, `@ignore_literal_names`, `@data_representation`, `@topic`,
`@unit`, `@range`, `@min`, `@max` and `@default`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — gap in the specification (DDSXTY14-50 for
`@range`, `@min`, `@max`; listed in the findings file); no requirement on an
implementation.

### §7.3.1.2.3 Alternative Annotation Syntax: //@ as suffix

**Spec:** §7.3.1.2.3, p. 105 — "it is legal to follow the declaration with a
single-line comment containing the annotation string. To distinguish such
comments from regular comments, there must be no space in between the double
slash ('//') and the at-sign ('@')."

**Repo:** `crates/idl-parse/src/suffix.rs`;
`crates/idl-parse/src/preprocess/mod.rs`

**Tests:** `crates/idl-parse/tests/parse.rs::a_suffix_annotation_reaches_the_tree_as_an_annotation`,
`crates/idl-parse/tests/parse.rs::only_the_exact_slash_slash_at_marker_is_the_suffix_form`,
`crates/idl-lower/tests/annotations.rs::the_prefix_and_the_suffix_form_produce_the_same_model`

**Status:** done

### §7.3.1.2.3 Alternative syntax: several annotations

**Spec:** §7.3.1.2.3, p. 105 — "If multiple annotations are to be applied to the
same element, the at-sign of each shall be preceded by a double slash and no
white space."

**Repo:** `crates/idl-parse/src/suffix.rs`

**Tests:** `crates/idl-lower/tests/annotations.rs::several_suffix_annotations_apply_to_the_same_element`,
`crates/idl-lower/tests/annotations.rs::two_suffix_annotations_on_one_line_both_apply`

**Status:** done

### §7.3.1.2.4 Defining Annotations: @annotation

**Spec:** §7.3.1.2.4, p. 106 — "An annotation type is defined using the new
token @annotation [...] Annotation identifiers are orthogonal to any other kind
of type and therefore do not conflict with other types that may use the same
identifier name even when defined in the same module."

**Repo:** `crates/idl-lower/src/lower.rs` (annotation declaration, around l. 2600);
`crates/ast/src/node/annotation.rs`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_user_declared_annotation_is_kept_whole`,
`crates/types/tests/type_object_annotations.rs::an_annotation_declaration_has_a_type_object`

**Status:** done — an annotation declaration is an `AnnotationDefNode` and not a
`TypeNode`; a type of the same name does not collide.

### §7.3.1.2.4 Table 22 — declaration as struct with //@annotation is read

**Spec:** §7.3.1.2.4, Table 22, p. 106 — "struct <ann_identifier> '{'
<ann_members> '};' //@annotation | The 'struct' <ann_identifier> is actually an
annotation type".

**Repo:** `crates/idl-lower/src/lower.rs::annotation_from_struct`

**Tests:** `crates/idl-lower/tests/annotations.rs::table_22s_declaration_form_declares_an_annotation_type`

**Status:** done — The form `struct … //@annotation` (Table 22) declares an annotation type (`lower.rs::annotation_from_struct`).

### §7.3.1.2.4 Table 23 — Annotation members with default

**Spec:** §7.3.1.2.4, Table 23, p. 106–107 — "[ <pre_annotations>] <member_type>
<member_name> [ default <member_value> ]; [ <post_annotations> ]" and "The
specified default value may be any legal IDL literal compatible with the
declared member type."

**Repo:** `crates/idl-lower/src/lower.rs` (annotation member, around l. 2647);
`crates/idl-lower/src/annotation.rs::check_annotation_application` (l. 468)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_parameter_value_is_the_value_that_was_applied`,
`crates/types/tests/type_object_custom_annotations.rs::an_unapplied_member_writes_ann_custom_absent`

**Status:** done — annotations on annotation members (`<pre_annotations>`) are
refused in the TypeObject, because Annex B has no place for them
(`annotation_type_object`).

### §7.3.1.2.5 Applying Annotations: prefix, several, multiple

**Spec:** §7.3.1.2.5, p. 107 — "Annotations may be applied to any type
definition or type member definition. [...] More than one annotation may be
applied to the same element, and multiple instances of the same annotation may
be applied to the same element."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_annotations` (l. 295)

**Tests:** `crates/idl-lower/tests/annotations.rs::several_annotations_on_one_member_all_apply`,
`crates/types/tests/type_object_custom_annotations.rs::one_declaration_applied_twice_is_one_node`

**Status:** done

### §7.3.1.2.5 Table 24 — Prefix and suffix form equivalent

**Spec:** §7.3.1.2.5, Table 24, p. 107 — "{ '@' <annotation_type_name> [ '('
<arguments> ')' ] }* [...] Alternately and equivalently [...] { '//@'
<annotation_type_name> [ '(' <arguments> ')' ] }*".

**Repo:** `crates/idl-parse/src/suffix.rs`

**Tests:** `crates/idl-lower/tests/annotations.rs::the_prefix_and_the_suffix_form_produce_the_same_model`,
`crates/idl-lower/tests/annotations.rs::a_suffix_annotation_carries_its_arguments`

**Status:** done

### §7.3.1.2.5 Annotation on the discriminator type in the union header

**Spec:** §7.3.1.2.5, p. 107 — "Annotations can be applied to the implicit
discriminator member of a union type by applying them to the discriminator type
declaration in the header of the union type's definition: union MyUnion switch
(@MyAnnotation long)".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_use_site` (l. 775)

**Tests:** `crates/idl-lower/tests/annotations.rs::try_construct_reaches_every_position_the_clause_names`,
`crates/idl-lower/tests/annotations.rs::a_declared_annotation_on_a_discriminator_is_kept`

**Status:** done — `@try_construct`, `@key` (under `Carried`) and a user-declared annotation on the discriminator type in the `switch` are taken over.

### §7.3.1.2.5 A user annotation on the discriminator is kept

**Spec:** §7.3.1.2.5, p. 107 — `union MyUnion switch (@MyAnnotation long)`.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_use_site`,
`crates/types/src/from_ast.rs::union`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_declared_annotation_on_a_discriminator_is_kept`,
`crates/types/tests/annotations_on_uses.rs::an_annotation_on_the_discriminator_is_in_the_discriminator_member`,
`crates/types/tests/annotations_on_uses.rs::the_applications_come_back_into_the_model`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_with_no_meaning_on_a_use_is_still_reported`

**Status:** done — A user-declared annotation on the discriminator (`union MyUnion switch (@MyAnnotation long)`) is in `XTypesFacet::discriminator_annotations` and in the `ann_custom` of `CompleteDiscriminatorMember`; the union's own list stays empty. A built-in name with no meaning there is still reported.

### §7.3.1.2.5 Names case-insensitive

**Spec:** §7.3.1.2.5, p. 108 — "As with any IDL identifier, the name of an
annotation and its members are not case-sensitive."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (l. 330)

**Tests:** `crates/idl-lower/tests/annotations.rs::annotation_names_are_matched_without_regard_to_case`,
`crates/idl-lower/tests/annotations.rs::every_spelling_of_one_builtin_says_the_same_thing`,
`crates/idl-lower/tests/validation.rs::the_names_of_annotation_members_are_not_case_sensitive`

**Status:** done — The names of the built-in annotations are recognised with no distinction of case, also qualified (`@::key`); the names of annotation members likewise.

### §7.3.1.2.5 @Key is recognized as @key

**Spec:** §7.3.1.2.5, p. 108 — names of annotations are not case-sensitive.

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::annotation_names_are_matched_without_regard_to_case`

**Status:** done — `@Key` is recognised as `@key` (XTypes 1.3 §7.3.1.2.5). IDL 4.2 §8.2 requires the spelling of the declaration for annotation names; for the built-in annotations the rule of XTypes applies here (contradiction between the two specifications, kept in the findings file).

### §7.3.1.2.5 Values as name = value, empty list, defaults, short form

**Spec:** §7.3.1.2.5, p. 108 — "each list item is of the form 'member_name =
member_value.' Each value must be a compile-time constant. [...] An annotation
with an empty list of member values is equivalent to a member list that is
omitted altogether. [...] If a value for a given member is omitted, and that
member has a defined default value, it will take that value. If an omitted
member does not have a specified default, it will take the default value
specified for its type [...] the member name may be omitted when applying the
annotation."

**Repo:** `crates/idl-lower/src/annotation.rs::check_annotation_application` (l. 468);
`crates/types/src/from_ast.rs::annotation_type_object` (l. 537)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_short_form_comes_back_as_the_named_one`,
`crates/types/tests/type_object_custom_annotations.rs::a_parameterless_annotation_is_written_without_a_param_seq`,
`crates/idl-lower/tests/validation.rs::an_annotation_member_with_no_default_takes_the_default_of_its_type`

**Status:** done — Named values, short form, defaults and an omitted member are implemented; a member without a default that is given no value takes the default of its type.

### §7.3.1.2.5 Missing value without a default: the type default

**Spec:** §7.3.1.2.5, p. 108 — "If an omitted member does not have a specified
default, it will take the default value specified for its type".

**Repo:** —

**Tests:** `crates/idl-lower/tests/validation.rs::an_annotation_member_with_no_default_takes_the_default_of_its_type`

**Status:** done — `check_annotation_application` reports no error for a member without a default and without a value; the value is the default of its type (§7.3.1.2.5). IDL 4.2 §7.4.15.4.2 requires a value instead (contradiction between the two specifications, kept in the findings file).

### §7.3.1.3 Constants and Expressions

**Spec:** §7.3.1.3, p. 109 — "Such declarations and expressions remain legal
IDL. However, they are not reflected directly in the Type System specified here,
which assumes that all compile-time-constant values have already been
evaluated."

**Repo:** `crates/idl-lower/src/expr.rs`;
`crates/ast/src/node/constant.rs`

**Tests:** `crates/ast/tests/node_space.rs::evaluation_yields_the_folded_value_without_mutating_the_tree`,
`crates/idl-lower/tests/completion.rs::an_array_dimension_is_the_evaluated_constant_expression`

**Status:** done

### §7.3.1.4 Table 25 — IDL primitive type mapping

**Spec:** §7.3.1.4, Table 25, p. 109 — Int8 `int8`, UInt8 `uint8`, Int16
`short`, UInt16 `unsigned short`, Int32 `long`, UInt32 `unsigned long`, Int64
`long long`, UInt64 `unsigned long long`, Float32 `float`, Float64 `double`,
Float128 `long double`, Char8 `char`, Char16 `wchar`, Boolean `boolean`, Byte
`octet`.

**Repo:** `crates/idl-lower/src/lower.rs::primitive_ref`;
`crates/types/src/from_ast.rs::primitive_kind` (l. 1479)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/from_ast.rs::primitive_kind_tests::octet_and_uint8_are_two_kinds`

**Status:** done — `uint8` becomes `TK_UINT8`, `octet` becomes `TK_BYTE`. The
OMG ShapeType IDL `interop/shapes/omg-dds-rtps.idl` has been changed to
`sequence<uint8>` so that it matches the type of the interoperability suite.

### §7.3.1.5 Alias Types

**Spec:** §7.3.1.5, p. 109 — "Aliases as described in this specification are
fully compatible with the IDL typedef construct."

**Repo:** `crates/idl-lower/src/lower.rs`;
`crates/ast/src/node/type_.rs::Alias` (l. 275)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_typedef_becomes_an_alias_to_its_target`,
`crates/idl-lower/tests/lowering.rs::one_typedef_may_declare_several_names`

**Status:** done

### §7.3.1.6 Array and Sequence Types

**Spec:** §7.3.1.6, p. 109 — "Arrays and sequences as described in this
specification are fully compatible with the IDL constructs of the same names."

**Repo:** `crates/ast/src/node/type_.rs::Sequence` (l. 219);
`crates/ast/src/node/type_.rs::Array` (l. 226)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_sequence_member_carries_its_element_type`,
`crates/idl-lower/tests/lowering.rs::a_typedef_declarator_keeps_its_array_dimensions`

**Status:** done

### §7.3.1.7 String Types: string and wstring, bound retained

**Spec:** §7.3.1.7, p. 109 — "Strings of Char8 shall be represented by the IDL
type string. Strings of Char16 shall be represented by the IDL type wstring. In
either case, any bound shall be retained."

**Repo:** `crates/idl-lower/src/lower.rs` (`type_ref_of`, arms `StringType`,
`WStringType`)

**Tests:** `crates/idl-lower/tests/completion.rs::a_bounded_string_and_sequence_carry_their_bound_expressions`

**Status:** done

### §7.3.1.8 Enumerated Types

**Spec:** §7.3.1.8, p. 109 — "Enumerations and bitmasks as described in this
specification are fully compatible with the IDL constructs of the same name."

**Repo:** `crates/ast/src/node/type_.rs::Enumeration` (l. 258);
`crates/ast/src/node/type_.rs::Bitmask` (l. 263)

**Tests:** `crates/idl-lower/tests/lowering.rs::an_enum_carries_its_enumerators_in_order`,
`crates/idl-lower/tests/lowering.rs::a_bitmask_carries_its_bits`

**Status:** done

### §7.3.1.9 Map Types

**Spec:** §7.3.1.9, p. 110 — "Map types as described in this specification are
fully compatible with the IDL constructs of the same name defined in the
Extended Data-Types Building Block of [IDL]."

**Repo:** `crates/ast/src/node/type_.rs::Map` (l. 233)

**Tests:** `crates/types/src/builder.rs::tests::map_builder_string_to_int`,
`crates/wire-conformance/tests/map_bytes.rs::the_recorded_row_round_trips_back_into_the_sample`

**Status:** done

### §7.3.1.10 Structure Types

**Spec:** §7.3.1.10, p. 110 — "Structures as described in this specification are
fully compatible with the IDL constructs of the same name."

**Repo:** `crates/ast/src/node/type_.rs::Structure` (l. 242)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_struct_carries_its_members_in_order`,
`crates/idl-lower/tests/lowering.rs::a_struct_with_a_base_records_it`

**Status:** done

### §7.3.1.11 Union Types: octet and wchar as discriminator

**Spec:** §7.3.1.11, p. 110 — "Compliant IDL parsers shall implement the
Building Block Extended Data-Types of [IDL], which adds support for Byte (octet)
and Char16 (wchar) type discriminators."

**Repo:** `crates/idl-lower/src/validate.rs::is_legal_discriminator` (l. 244)

**Tests:** `crates/idl-lower/tests/validation.rs::each_legal_discriminator_type_is_accepted`

**Status:** done

---

## §7.3.2 XML Type Representation

### §7.3.2 XML Type Representation: XSD and namespace http://www.omg.org/dds

**Spec:** §7.3.2, p. 110 — "This format is defined by an XML schema document
(XSD) and a set of semantic rules [...] The XML namespace of the XML Type
Representation shall be http://www.omg.org/dds."

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_type_libraries` (l. 34);
`crates/xml-compose/src/lower.rs::lower_all` (l. 79)

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_simple_struct`,
`crates/xml/src/xtypes_parser.rs::tests::dds_root_with_multiple_types_blocks`,
`crates/xml-compose/tests/loader.rs::a_malformed_document_is_an_error_and_not_an_empty_model`

**Status:** done — two paths: `zerodds-xml` reads the types into a `TypeLibrary`
of its own and from there into a `MinimalTypeObject`; `zerodds-xml-compose`
reads them into the shared model. The entries below cite the second path where
it carries the construct. Whether the namespace is checked is not checked.

### §7.3.2 Design Rationale (non-normative)

**Spec:** §7.3.2, p. 110 — the built-in annotations are constructs of their own
in XML.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — "non-normative".

### §7.3.2.1.1 File Inclusion

**Spec:** §7.3.2.1.1, p. 110–111 — "A file inclusion specified as in this Type
Representation shall be considered equivalent to an IDL #include of the same
file."

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_include`,
`crates/xml-compose/src/lower.rs::load_units`,
`crates/xml-compose/src/lower.rs::load_one`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_include_element`,
`crates/xml/src/xtypes_parser.rs::tests::parse_include_missing_file_attribute_rejected`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_include_brings_the_named_document_into_the_model_once`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_include_that_cannot_be_followed_is_reported`

**Status:** done — `zerodds-xml` reads `<include file=…/>` as an entry. `zerodds-xml-compose` follows the entry: the file lies beside the document that names it, is loaded into the model once however often it is included, and comes ahead of the including document. A file that cannot be read as XML and a file of another representation are an error (§7.3.2.1.1 does not require including other representations).

### §7.3.2.1.1 File Inclusion in the model path

**Spec:** §7.3.2.1.1, p. 110 — inclusion equivalent to `#include`.

**Repo:** `crates/xml-compose/src/lower.rs::load_units`,
`crates/xml-compose/src/lower.rs::load_one`

**Tests:** `crates/xml-compose/tests/constants_and_annotations.rs::an_include_brings_the_named_document_into_the_model_once`

**Status:** done — `load_units` and `load_one` in `zerodds-xml-compose` load the named documents ahead of the including one; a type of the included document is in the model equivalent to one that comes from IDL by `#include`. The include depth is limited to `MAX_INCLUDE_DEPTH`.

### §7.3.2.1.1 Including other representations not required

**Spec:** §7.3.2.1.1, p. 111 — "Conformant Type Representation compilers need
not support the inclusion of files of other Type Representations from within an
XML Type Representation document."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — exemption.

### §7.3.2.1.2 Forward Declarations

**Spec:** §7.3.2.1.2, p. 111 — "A forward declaration as described in this Type
Representation shall be considered semantically equivalent to an IDL forward
declaration."

**Repo:** `crates/xml-compose/src/lower.rs::forward_declaration` (l. 630);
`crates/xml/src/xtypes_parser.rs::parse_forward_dcl` (l. 167)

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_forward_dcl_element`,
`crates/xml-compose/tests/idl_parity.rs::forward_and_enclosing_scope_references_reach_the_same_type`

**Status:** done

### §7.3.2.1.3 Constants: value with const_exp, at top level, in a module, in a structure

**Spec:** §7.3.2.1.3, p. 111–112 — "the string specified in the value attribute
described below shall have the same syntax as the <const_exp> production in the
IDL grammar [IDL]. Constants can appear at the top level of a Type
Representation file, within a module, or [...] within a structure declaration."

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_const`,
`crates/xml-compose/src/lower.rs::constant`,
`crates/xml-compose/src/expr.rs::parse`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_const_element`,
`crates/xml/src/xtypes_parser.rs::tests::parse_const_missing_value_attribute_rejected`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_constant_inside_a_structure_is_in_its_scope`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_constant_is_found_whichever_side_of_its_use_it_is_declared`,
`crates/xml-compose/src/expr.rs::tests::the_examples_of_the_specification_parse`,
`crates/xml-compose/src/expr.rs::tests::the_precedence_is_that_of_idl`,
`crates/xml-compose/src/expr.rs::tests::literals_are_read_as_idl_reads_them`,
`crates/xml-compose/src/expr.rs::tests::what_is_not_an_expression_is_refused_with_a_position`

**Status:** done — `<const name type value/>` appears at the top, in a module and in a structure. `value` is a `<const_exp>` after IDL 4.2 §7.4.1.4 with the operator precedence of §7.4.1.4.2; `zerodds-xml-compose` parses it (`expr::parse`) and stores the expression tree in the model. Names are resolved from the inside out, `S::C` reaches a constant in a structure, a name with no match is an error. `zerodds-xml` reads `<const>` as an entry.

### §7.3.2.1.3 Constants and expressions in the model path

**Spec:** §7.3.2.1.3, p. 111 — `const_exp`-Syntax.

**Repo:** `crates/xml-compose/src/lower.rs::bound`,
`crates/xml-compose/src/lower.rs::dimensions`,
`crates/xml-compose/src/lower.rs::const_exp`,
`crates/xml-compose/src/expr.rs::parse_list`

**Tests:** `crates/xml-compose/tests/constants_and_annotations.rs::bounds_are_const_expressions_over_constants`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_bound_naming_no_constant_is_reported`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_bound_keeps_the_constant_it_names`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_enumerator_value_is_a_const_expression`,
`crates/xml-compose/src/expr.rs::tests::the_examples_of_the_specification_parse`,
`crates/xml-compose/src/expr.rs::tests::the_precedence_is_that_of_idl`,
`crates/xml-compose/src/expr.rs::tests::literals_are_read_as_idl_reads_them`,
`crates/xml-compose/src/expr.rs::tests::what_is_not_an_expression_is_refused_with_a_position`

**Status:** done — Bounds, array dimensions and enumerator values are `<const_exp>`: `"2 + MY_CONSTANT"`, `"6 * 3"` and `" 2, MY_CONSTANT + 3 "` from the examples of the spec are read and evaluated (`Ast::eval_integral`). A name that is declared further on stays in the tree and is evaluated when the number is needed. An expression that does not parse names the character where it stops.

### §7.3.2.2 Basic Types: type, nonBasicTypeName

**Spec:** §7.3.2.2, p. 112 — "A 'type' attribute, typed by an enumeration
allTypeKind, indicates whether the type is 'basic' [...] A 'non-basic type name'
attribute indicates the name of the type if it is a non-basic type. It is an
error to include this attribute if the type attribute does not indicate a
non-basic type."

**Repo:** `crates/xml-compose/src/lower.rs::type_reference`;
`crates/xml/src/xtypes_parser.rs::parse_type_attribute`

**Tests:** `crates/xml/tests/annex_a_types.rs::a_non_basic_type_is_named_by_nonbasictypename_on_every_element_that_has_a_type`,
`crates/xml/tests/annex_a_types.rs::nonbasictypename_beside_a_basic_type_is_an_error`,
`crates/xml-compose/tests/annex_a_declarations.rs::nonbasictypename_names_a_type_only_beside_nonbasic`,
`crates/xml-compose/tests/idl_parity.rs::every_primitive_reaches_the_same_type_from_both_front_ends`

**Status:** done — Both readers read `type="nonBasic"` with `nonBasicTypeName`. `nonBasicTypeName` beside any other `type` is an error that names the attribute: on a member, a typedef, a discriminator, an annotation member and (as `mapKeyNonBasicTypeName`) on a map key.

### §7.3.2.2 Basic Types: nonBasicTypeName in the zerodds-xml path

**Spec:** §7.3.2.2, p. 112 — attribute for the name of a non-basic type.

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_type_attribute`

**Tests:** `crates/xml/tests/annex_a_types.rs::a_non_basic_type_is_named_by_nonbasictypename_on_every_element_that_has_a_type`,
`crates/xml/tests/annex_a_types.rs::nonbasictypename_beside_a_basic_type_is_an_error`

**Status:** done — `zerodds-xml` reads `nonBasicTypeName` on members, typedefs, discriminators and annotation members and checks the name against `identifierName` (Annex A).

### §7.3.2.2 Table 26 — names of the primitive types and strings

**Spec:** §7.3.2.2, Table 26, p. 112–113 — boolean, byte, char8, char16, int32,
uint32, int8, uint8, int16, uint16, int64, uint64, float32, float64, float128,
string, wstring.

**Repo:** `crates/xml-compose/src/lower.rs::primitive_of` (l. 1449)

**Tests:** `crates/xml-compose/src/lower.rs::tests::the_two_primitive_vocabularies_agree`,
`crates/xml-compose/src/lower.rs::tests::octet_and_uint8_are_two_kinds`,
`crates/xml-compose/tests/idl_parity.rs::every_primitive_reaches_the_same_type_from_both_front_ends`

**Status:** done — in addition the IDL names (`long`, `octet`, …) are accepted.

### §7.3.2.3 String Types: stringMaxLength

**Spec:** §7.3.2.3, p. 113 — "The stringMaxLength attribute, if present,
indicates the string's bound. If the attribute is omitted, the string shall be
considered unbounded. The presence of this attribute is legal only when a
member's type is a string, a wide string, or an alias to string or wide string."

**Repo:** `crates/xml-compose/src/lower.rs::type_reference` (l. 753, arm `string`/`wstring`)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::string_bounds_reach_the_same_type_from_both_front_ends`

**Status:** done — whether `stringMaxLength` on a non-string member is reported
is not checked.

### §7.3.2.4 Collection Types: MaxLength attributes, "-1" unbounded

**Spec:** §7.3.2.4, p. 113 — "Collection bounds are indicated by attributes
named according to the convention <collection>MaxLength [...] the values of
these attributes may be any constant expression [...] The literal expression
'-1' shall indicate an unbounded collection; no other 'negative' value is
permitted."

**Repo:** `crates/xml-compose/src/lower.rs::bound`,
`crates/xml-compose/src/lib.rs::ComposeOptions`

**Tests:** `crates/xml-compose/tests/loader.rs::a_bound_of_zero_is_refused_by_default`,
`crates/xml-compose/tests/loader.rs::a_length_unlimited_bound_is_an_unbounded_collection`,
`crates/xml-compose/tests/annex_a_attributes.rs::a_negative_bound_other_than_minus_one_is_refused`,
`crates/xml-compose/tests/constants_and_annotations.rs::bounds_are_const_expressions_over_constants`

**Status:** done — "-1" is unbounded, any other negative bound is an error, a bound is a `<const_exp>` and is evaluated (`lower.rs::bound`). The value `0` is an error: the clause names only "-1", and a bound in IDL is a positive integer (`positive_int_const`). The suite writes "0 or -1" in `sequences.xml`; `ComposeOptions::zero_bound_is_unbounded` (default `false`) reads `0` as unbounded, and the driver of the suite (`tools/omg-xtypes-test-main`) sets it.

### §7.3.2.4 Collection types: other negative bounds are refused

**Spec:** §7.3.2.4, p. 113 — "no other 'negative' value is permitted."

**Repo:** `crates/xml-compose/src/lower.rs::bound`

**Tests:** `crates/xml-compose/tests/annex_a_attributes.rs::a_negative_bound_other_than_minus_one_is_refused`

**Status:** done — `bound` refuses a negative value other than -1 with an error.

### §7.3.2.4 Collection types: external as an attribute applies to the elements

**Spec:** §7.3.2.4, p. 113 — "The element_external property of the Type System
Model shall be represented by an attribute external."

**Repo:** `crates/xml-compose/src/lower.rs::member_spec`

**Tests:** `crates/xml-compose/tests/annex_a_attributes.rs::external_on_a_collection_member_is_the_elements`

**Status:** done — `external="true"` on a collection member applies to the elements (`element_is_external`, for a map also `key_is_external`).

### §7.3.2.4.1 Array Types: arrayDimensions

**Spec:** §7.3.2.4.1, p. 113–114 — "The presence of the arrayDimensions
attribute shall indicate that given member is an array. Array dimensions are
represented as a comma-delimited list [...] Whitespace is allowed around each
bound".

**Repo:** `crates/xml-compose/src/lower.rs::dimensions`,
`crates/xml-compose/src/lower.rs::collections_around`,
`crates/xml-compose/src/expr.rs::parse_list`

**Tests:** `crates/xml-compose/tests/idl_parity.rs::arrays_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/constants_and_annotations.rs::bounds_are_const_expressions_over_constants`,
`crates/xml-compose/src/expr.rs::tests::a_list_splits_at_the_commas_outside_parentheses_and_literals`

**Status:** done — The dimensions are a list of `<const_exp>`, separated by commas outside parentheses and literals; whitespace around a bound does not count. A dimension below 1 and a list with no bound are an error.

### §7.3.2.4.2 Sequence Types: sequenceMaxLength

**Spec:** §7.3.2.4.2, p. 114 — "The sequenceMaxLength attribute, if present,
shall indicate that the member is of a sequence type. This attribute is required
for all sequence types. In case of an unbounded sequence, sequenceMaxLength
shall be set to '-1'."

**Repo:** `crates/xml-compose/src/lower.rs::collections_around` (l. 816)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::sequences_reach_the_same_type_from_both_front_ends`

**Status:** done

### §7.3.2.4.3 Map Types: mapMaxLength, mapKeyType, mapKeyNonBasicTypeName, mapKeyStringMaxLength

**Spec:** §7.3.2.4.3, p. 114–115 — mapMaxLength is required, "-1" unbounded;
mapKeyType is required; mapKeyNonBasicTypeName only with a non-basic key;
mapKeyStringMaxLength only with a string key.

**Repo:** `crates/xml-compose/src/lower.rs::collections_around`

**Tests:** `crates/xml-compose/tests/constants_and_annotations.rs::map_key_attributes_are_checked_against_the_key_type`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_map_without_map_max_length_is_reported`,
`crates/xml-compose/tests/constants_and_annotations.rs::the_collection_attributes_nest_sequence_then_array_then_map`

**Status:** done — `mapMaxLength` missing: error. `mapKeyNonBasicTypeName` beside a basic key and `mapKeyStringMaxLength` beside a key that is no string: error. A map from XML is lowered into the model and its bounds are evaluated. A non-basic key that may be an alias of a string is not judged for `mapKeyStringMaxLength`.

### §7.3.2.4.4 Combinations: sequence before array before map

**Spec:** §7.3.2.4.4, p. 115 — "The following list is ordered from
most-tightly-binding to least-tightly-binding: Sequence designations [...] Array
designations [...] Map designations".

**Repo:** `crates/xml-compose/src/lower.rs::collections_around`

**Tests:** `crates/xml-compose/tests/constants_and_annotations.rs::the_collection_attributes_nest_sequence_then_array_then_map`

**Status:** done — `collections_around` binds the sequence first, then the array, then the map: `sequenceMaxLength`, `arrayDimensions` and `mapKeyType` together give a map whose values are arrays of sequences. Before, the array lay around the map and the sequence was the map bound. Another order needs an alias (§7.3.2.4.4).

### §7.3.2.5 Aggregated Types: properties as attributes

**Spec:** §7.3.2.5, p. 116 — "the XML Type Definition is able to express these
properties directly. For example, structures and unions may indicate whether
they are appendable/mutable and/or nested types: <struct name=\"MyStructure\"
extensibility=\"mutable\" nested=\"true\">".

**Repo:** `crates/xml-compose/src/lower.rs::record` (l. 1145)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::keys_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/idl_parity.rs::member_ids_reach_the_same_numbers_from_both_front_ends`

**Status:** done

### §7.3.2.5 Aggregated Types: option for the default extensibility

**Spec:** §7.3.2.5, p. 116 — "type representation compilers shall provide
configuration options to allow users to specify whether types of unspecified
extensibility will be considered final, appendable, or mutable."

**Repo:** `crates/xml-compose/src/lower.rs::lower_all` (l. 79)

**Tests:** `crates/xml-compose/tests/loader.rs::the_default_extensibility_only_reaches_types_the_document_left_open`

**Status:** done

### §7.3.2.5.1 Structures: annotations, verbatim, members, constants

**Spec:** §7.3.2.5.1, p. 116 — "Structures contain four kinds of declarations:
Applied annotations, Verbatim text, Members, Constants".

**Repo:** `crates/xml-compose/src/lower.rs::structure`

**Tests:** `crates/xml-compose/tests/idl_parity.rs::keys_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_constant_inside_a_structure_is_in_its_scope`,
`crates/xml-compose/tests/constants_and_annotations.rs::verbatim_text_reaches_the_type`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_annotation_is_declared_and_applied`

**Status:** done — `structure` reads the four kinds of children: `<annotate>`, `<verbatim>`, `<member>` and `<const>`. Constants first, so that a bound may name them ahead of the declaration; names inside begin the search at the structure.

### §7.3.2.5.1.1 Verbatim Text and §7.3.2.9 Annotations in XML

**Spec:** §7.3.2.5.1.1, p. 116–117, and §7.3.2.9, p. 119 — `<verbatim language=…
placement=…>`, `<annotation name=…>` with members and `<annotate name=…>` with
values.

**Repo:** `crates/xml-compose/src/lower.rs::verbatim_directives`,
`crates/xml-compose/src/lower.rs::annotation_declaration`,
`crates/xml-compose/src/lower.rs::applied_annotations`,
`crates/xml-compose/src/lower.rs::annotate_enumerated`

**Tests:** `crates/xml-compose/tests/constants_and_annotations.rs::verbatim_text_reaches_the_type`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_verbatim_placement_that_is_none_of_the_six_is_reported`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_annotation_is_declared_and_applied`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_annotate_of_an_unknown_or_a_built_in_annotation_is_reported`

**Status:** done — `<verbatim language placement>` sits on a structure, union, enumeration and bitset, the text as content of the element or as a `<text>` child after Annex A; the placement is one of the six spellings of IDL 4.2 §8.3.5.1 (default `before-declaration`), the language defaults to `*`. `<annotation>` declares an annotation type with members, `<annotate>` applies it to a type, member, discriminator, enumeration and literal, the values are `<const_exp>`. A built-in annotation written as `<annotate>` (`key`, `optional`) is an error pointing at the attribute. `zerodds-xml` reads none of the three elements.

### §7.3.2.5.1.2 Members: key, optional

**Spec:** §7.3.2.5.1.2, p. 117 — "Each structure type shall include one or more
members. Each member of a structure type can indicate individually whether or
not it is a key member and whether or not it is an optional member."

**Repo:** `crates/xml-compose/src/lower.rs::member_spec` (l. 667)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::keys_reach_the_same_type_from_both_front_ends`

**Status:** done — "one or more members" is not enforced; IDL 4.2 permits an
empty structure (contradiction in the specification, listed in the findings
file).

### §7.3.2.5.1.3 Inheritance: baseType

**Spec:** §7.3.2.5.1.3, p. 117 — "A structure declaration's baseType attribute
indicates the name of the structure's base type, if any".

**Repo:** `crates/xml-compose/src/lower.rs::structure` (l. 257,
`baseType` l. 274);
`crates/xml/src/xtypes_parser.rs::parse_struct` (l. 196)

**Tests:** `crates/xml/tests/types_and_samples.rs::t04_struct_inheritance_basetype`,
`crates/xml/tests/types_and_samples.rs::t18_struct_with_inheritance_resolves`

**Status:** done — the tests cover the `zerodds-xml` path; for
`zerodds-xml-compose` a test with `baseType` is missing (not checked whether the
result equals the IDL path).

### §7.3.2.5.2 Unions: discriminator first, case, caseDiscriminator, default

**Spec:** §7.3.2.5.2, p. 117–118 — "exactly one discriminator member [...] and
one or more cases [...] The discriminator member must be declared before the
others. Each case of a union contains one or more discriminator values
(caseDiscriminator elements) and one data member. [...] The literal 'default' is
also allowed".

**Repo:** `crates/xml-compose/src/lower.rs::union` (l. 308);
`crates/xml-compose/src/lower.rs::case_label` (l. 992)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::unions_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/loader.rs::a_bitmask_discriminated_union_keeps_its_cases`

**Status:** done

### §7.3.2.6 Aliases: typedef

**Spec:** §7.3.2.6, p. 118 — "Alias definitions are defined in typedef elements.
They have syntax very similar to that of structure members."

**Repo:** `crates/xml-compose/src/lower.rs::alias` (l. 588);
`crates/xml/src/xtypes_parser.rs::parse_typedef` (l. 356)

**Tests:** `crates/xml/tests/types_and_samples.rs::t15_typedef_simple_and_array`

**Status:** done — the test covers the `zerodds-xml` path; for
`zerodds-xml-compose` a test with `typedef` is missing.

### §7.3.2.7.1 Enumerations: enumerator, value, bitBound

**Spec:** §7.3.2.7.1, p. 118 — "Enumerations consist of a list of enumeration
literals, each of which has a name and a value. The syntax of the value shall be
defined by the <const_exp> production [...] If the value is omitted, it shall be
assigned automatically."

**Repo:** `crates/xml-compose/src/lower.rs::enumeration` (l. 459)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::a_narrow_bit_bound_reaches_the_same_type_from_both_front_ends`

**Status:** done — for `value` as `const_exp` see §7.3.2.1.3.

### §7.3.2.7.2 Bitmasks: flag, position, bitBound

**Spec:** §7.3.2.7.2, p. 119 — "<bitmask name=\"MyBitmask\" bitBound=\"64\">
<flag name=\"FIRST_BIT\" position=\"0\"/>".

**Repo:** `crates/xml-compose/src/lower.rs::bitmask` (l. 534)

**Tests:** `crates/xml-compose/tests/loader.rs::both_bitmask_spellings_reach_the_same_bits`,
`crates/xml-compose/tests/table_15_bitmask.rs::a_bitmask_discriminator_matches_the_uint_of_its_bound`

**Status:** done

### §7.3.2.8 Modules

**Spec:** §7.3.2.8, p. 119 — "A module groups type declarations and serves as a
namespace for those definitions."

**Repo:** `crates/xml-compose/src/lower.rs::module` (l. 226)

**Tests:** `crates/xml-compose/tests/loader.rs::two_documents_reopen_one_module`,
`crates/xml/src/xtypes_parser.rs::tests::parse_module_nested`

**Status:** done

---

## §7.3.3 XSD Type Representation

### §7.3.3 XSD by the IDL-to-XSD mapping

**Spec:** §7.3.3, p. 120 — "An XSD Representation of a given type shall be as if
the OMG-standard IDL mapping to XSD were applied to the IDL Representation of
the type as defined in Clause 7.3.1. [...] Valid documents conforming to such
schemas [...] must respect such namespaces, if any."

**Repo:** `crates/xml/src/xsd_schema.rs::parse_xsd_schema`;
`crates/xml/src/xsd_writer.rs::type_library_to_xsd`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::the_examples_of_the_mapping_are_read_as_the_idl_they_come_from`,
`crates/xml/tests/xsd_type_representation.rs::the_writer_writes_what_the_mapping_writes`,
`crates/xml/tests/xsd_type_representation.rs::intermediate_types_with_other_dimensions_get_a_suffix`,
`crates/xml/tests/xsd_type_representation.rs::the_element_of_a_collection_may_still_be_named_element`,
`crates/xml/tests/xsd_type_representation.rs::a_library_survives_its_xsd_representation`,
`crates/xml/tests/xsd_type_representation.rs::the_written_schema_compiles_and_validates_sample_documents`

**Status:** done — The document of the mapping (CORBA to WSDL/SOAP Interworking 1.2.1, formal/2008-08-03, clauses 4.1.6 to 4.1.7.6) has been read (https://www.omg.org/spec/C2WSDL/1.2.1/PDF). Read and written: structure (4.1.7.2; a string member is `nillable`), alias as a `simpleType` restriction or, for a complex type, as a `complexContent` restriction that states the content again (4.1.7.3), union (4.1.7.4), sequence with the element `item` (4.1.7.5), array (4.1.7.6: a field of an array type has the type `<type>.<field>_ArrayOf<element>`, several dimensions nest as for `long matrix[5][3]`, the first dimension is the innermost type, a taken name `ArrayOf<element>` with other dimensions gets the suffix `_n`), bounded string with `maxLength fixed` (4.1.6), constants as the substituted value. `element` (DDS-XML 1.0) is still read. The SOAP-encoding form of sequences and arrays (`_SE_`, `soapenc:Array`, 4.1.7.5) is skipped on reading and not written; the WS-I form is the one written. Object references, interfaces, value types, `any`, `TypeCode` and `fixed` (4.1.5, 4.1.7.7 to 4.1.7.10, 4.1.8) map constructs the type model of XTypes does not have. The rules for module names with `#pragma prefix`, `version` and `id` (4.1.4) presuppose pragmas the library does not know; the name is the qualified name with `.`. The check against a second validator runs on the Linux test host (`xmllint`, log in `.agent/REPORT.md`).

### §7.3.3 Mapping of union, bitmask and collection by the text of the IDL-to-XSD mapping

**Spec:** §7.3.3, p. 120 — "as if the OMG-standard IDL mapping to XSD were applied to the IDL Representation of the type".

**Repo:** `crates/xml/src/xsd_schema.rs::parse_xsd_schema`;
`crates/xml/src/xsd_writer.rs::type_library_to_xsd`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::the_examples_of_the_mapping_are_read_as_the_idl_they_come_from`,
`crates/xml/tests/xsd_type_representation.rs::the_writer_writes_what_the_mapping_writes`,
`crates/xml/tests/xsd_type_representation.rs::intermediate_types_with_other_dimensions_get_a_suffix`,
`crates/xml/tests/xsd_type_representation.rs::the_element_of_a_collection_may_still_be_named_element`,
`crates/xml/tests/xsd_type_representation.rs::a_library_survives_its_xsd_representation`,
`crates/xml/tests/xsd_type_representation.rs::the_written_schema_compiles_and_validates_sample_documents`

**Status:** done — Union (`sequence` of `discriminator` and `choice`, 4.1.7.4) and collections (4.1.7.5, 4.1.7.6) follow the text of the mapping. The labels of a union are in the mapping as an XML comment only (`<!-- case 2, 3 -->`), which a reader does not see; here they are in the annotation `case` of the branch (§7.3.3.1.2), and a branch without it is an error. A bitmask has no form in the mapping (the document has no `bitmask`) and §7.3.3 gives none; the `list` of a restriction with the flag names is this library's choice, with the annotation `position` per flag.

### §7.3.3.1.1 Annotation types as complexType with appInfo Definition

**Spec:** §7.3.3.1.1, p. 120–121 — "Any complexType definition immediately
containing an XSD Annotation with an appInfo element having a source attribute
value of http://www.omg.org/Type/Annotation/Definition shall be considered to be
an OMG Annotation." Table 27.

**Repo:** `crates/xml/src/xsd_schema.rs::annotation_definition`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::an_annotation_type_is_a_complex_type_with_the_definition_appinfo`

**Status:** done — A `complexType` whose `xsd:annotation` holds an `appinfo` with the `source` `http://www.omg.org/Type/Annotation/Definition` is an annotation type; each `xsd:attribute` is a member with its type and `default` (Table 27). The spelling `appInfo` of the clause text is read like `appinfo`; the writer writes annotation types in this form.

### §7.3.3.1.2 Application through appInfo Usage and annotate

**Spec:** §7.3.3.1.2, p. 121–122 — "OMG Annotations shall be applied to a
definition by declaring, immediately within that definition's XML element, an
XSD Annotation containing an appInfo with its source attribute set to
http://www.omg.org/Type/Annotation/Usage [...] The appInfo element shall contain
an element annotate for each OMG Annotation to be applied."

**Repo:** `crates/xml/src/xsd_schema.rs::applied_to`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::annotations_are_applied_through_the_usage_appinfo`

**Status:** done — An `appinfo` with the `source` `http://www.omg.org/Type/Annotation/Usage` in the definition holds one `annotate` per annotation; its `type` (also as `omg:type`) names the annotation, the other attributes are the values of its members. Read on `complexType`, `element`, `simpleType` and `enumeration`.

### §7.3.3.1.3 Built-in Annotations in XSD

**Spec:** §7.3.3.1.3, p. 122 — "those Type System concepts represented with
built-in annotations in the IDL Type Representation shall be represented by
equivalent built-in annotations in this Type Representation."

**Repo:** `crates/xml/src/xsd_schema.rs::struct_type`;
`crates/xml/src/xsd_schema.rs::member_of`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::built_in_annotations_are_read_from_their_equivalents`

**Status:** done — The built-in annotations of the IDL Type Representation are read and written under their IDL names: `key`, `id`, `optional`, `must_understand`, `non_serialized`, `external`, `try_construct`, `nested`, `autoid`, `final`, `appendable`, `mutable`, `extensibility`, `bit_bound`, `value`, `default_literal`, `position`, `verbatim`. `minOccurs="0"` is `@optional` (§7.3.3.2.2).

### §7.3.3.2.1 Inheritance: complexContent extension

**Spec:** §7.3.3.2.1, p. 122 — "The subtype shall extend its base type using an
XSD complexContent element." Table 28.

**Repo:** `crates/xml/src/xsd_schema.rs::complex_type`, `crates/xml/src/xsd_schema.rs::struct_type`

**Tests:** `crates/xml/src/xsd_schema.rs::tests::complex_content_extension_yields_inheritance`

**Status:** done

### §7.3.3.2.2 Optional Members: minOccurs 0

**Spec:** §7.3.3.2.2, p. 123 — "Optional members of an aggregated type shall be
indicated with a minOccurs attribute value of 0 instead of 1."

**Repo:** `crates/xml/src/xsd_schema.rs::member_of`, `crates/xml/src/xsd_schema.rs::occurs`

**Tests:** `crates/xml/src/xsd_schema.rs::tests::min_occurs_zero_yields_optional_member`

**Status:** done

### §7.3.3.3 Nested Types: root element per non-nested type

**Spec:** §7.3.3.3, p. 123 — "For each type T that is not a nested type, the
schema shall define an XML element of that type suitable for use as a document
root. The name of this element shall be the fully qualified name of T."

**Repo:** `crates/xml/src/xsd_schema.rs::parse_xsd_schema`;
`crates/xml/src/xsd_writer.rs::type_library_to_xsd`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::a_root_element_is_declared_for_each_type_that_is_not_nested`,
`crates/xml/tests/xsd_type_representation.rs::a_nested_type_with_a_root_element_is_refused`

**Status:** done — The writer declares a root element named by the fully qualified name (`MyModule.MyStructure`) for every structure and union that is not `nested`; a nested type has none. The reader recognizes the root elements; a type annotated nested that has a root element is an error. The reader does not conclude from a missing root element that a type is nested.

### §7.3.3.4 Maps: sequence with maxOccurs, key and value

**Spec:** §7.3.3.4, p. 123 — "the XSD sequence declaration specifies a maxOccurs
multiplicity equal to the bound of the map [...] The map elements are
represented by elements named key and value".

**Repo:** `crates/xml/src/xsd_schema.rs::wrapper_shape`;
`crates/xml/src/xsd_writer.rs::collection_body`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::a_map_is_a_sequence_with_max_occurs_and_the_elements_key_and_value`

**Status:** done — A `sequence` with `maxOccurs` (the bound, else `unbounded`) of the elements `key` and `value` is a map, as a named type (alias) and as the type of a member; the example of the clause text (`MyMap`, int to double, bound 32) is read, and the writer writes the same form.

---

## §7.3.4 Representing Types with TypeIdentifier and TypeObject

### §7.3.4 Every type has a TypeIdentifier, complex types a TypeObject

**Spec:** §7.3.4, p. 123 — "Any possible type within the XTYPES type system is
uniquely identified by a TypeIdentifier. In the case of simple types [...] the
TypeIdentifier completely describes the Type. For more complex types, the
TypeIdentifier only identifies the type and its full description uses a
TypeObject."

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (l. 197);
`crates/types/src/type_object/mod.rs::TypeObject` (l. 36);
`crates/types/src/from_ast.rs::type_identifier` (l. 389)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/src/hash.rs::tests::to_hashed_type_identifier_picks_correct_kind`

**Status:** done

### §7.3.4.1 Plain Types: collections without annotations other than @external and @try_construct

**Spec:** §7.3.4.1, p. 124 — "This specification uses the term Plain Collection
type to refer to anonymous collection types (array, sequence, and map) that have
no annotations beyond @external and @try_construct. [...] Plain types only have
a TypeIdentifier. Non-plain types have both a TypeIdentifier and a TypeObject."

**Repo:** `crates/types/src/from_ast.rs::is_plain` (l. 1261)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`,
`crates/types/tests/dependency_type_objects.rs::a_type_of_fully_descriptive_members_has_no_dependencies`

**Status:** done — an anonymous collection is always plain; the front end
refuses other annotations on it (§7.2.2.6).

### §7.3.4.2 Type Identifier: identifies an equivalence class

**Spec:** §7.3.4.2, p. 124 — "More precisely it identifies each equivalence
class of types, see Clause 7.3.4.6. [...] TypeIdentifier is a discriminated
union allowing the format of the identifier to vary depending on the type. Table
21 below lists the TypeIdentifier discriminator values".

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (l. 197)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::primitive_none_roundtrips`

**Status:** done — the text refers to "Table 21" instead of Table 29 and to
§7.3.4.6 instead of §7.3.4.7 (defect of the specification, listed in the
findings file).

### §7.3.4.2 Table 29 — TK_NONE: invalid identifier

**Spec:** §7.3.4.2, Table 29, p. 124 — "TK_NONE | N/A | Invalid identifier".

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_NONE` (l. 19);
`crates/types/src/assignability.rs::is_assignable` (l. 274, check l. 295)

**Tests:** `crates/types/tests/table19_key_members.rs::tk_none_is_not_assignable_from_itself`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_none_roundtrips`

**Status:** done

### §7.3.4.2 Table 29 — primitive types: discriminator without a body

**Spec:** §7.3.4.2, Table 29, p. 124 — "TK_BOOLEAN, TK_BYTE, TK_INT8, TK_UINT8,
TK_INT16, TK_INT32, TK_INT64, TK_UINT16, TK_UINT32, TK_UINT64, TK_FLOAT32,
TK_FLOAT64, TK_FLOAT128, TK_CHAR8, TK_CHAR16 | Primitive Types | Plain Type. No
TypeObject. Fully described by the discriminator."

**Repo:** `crates/types/src/type_identifier/mod.rs::PrimitiveKind` (l. 315)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.3.4.2 Table 29 — TI_STRING8_SMALL, TI_STRING8_LARGE

**Spec:** §7.3.4.2, Table 29, p. 124 — "Fully described by the discriminator and
the bound of the string. The SMALL discriminators have a bound represented as an
octet. It is used for unbounded strings or strings with bounds smaller than
256."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING8_SMALL` (l. 63);
`crates/types/src/type_identifier/kinds.rs::TI_STRING8_LARGE` (l. 65)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string8_small_roundtrips`,
`crates/types/src/type_identifier/mod.rs::tests::string8_large_roundtrips`

**Status:** done — the threshold in the code is `bound <= 255` for SMALL; the
specification says "smaller than 256", which is the same set.

### §7.3.4.2 Table 29 — TI_STRING16_SMALL, TI_STRING16_LARGE

**Spec:** §7.3.4.2, Table 29, p. 125 — "Wide String types | Plain Type. No
TypeObject. Fully described by the discriminator and the bound of the string."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING16_SMALL` (l. 67);
`crates/types/src/type_identifier/kinds.rs::TI_STRING16_LARGE` (l. 69)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string16_small_and_large_roundtrip`

**Status:** done

### §7.3.4.2 Table 29 — TI_PLAIN_SEQUENCE_SMALL, TI_PLAIN_SEQUENCE_LARGE

**Spec:** §7.3.4.2, Table 29, p. 125 — "TypeIdentifier contains maximum length
of sequence and the TypeIdentifier of element. SMALL and LARGE indicate
representation of maximum length."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_SEQUENCE_SMALL` (l. 72);
`crates/types/src/type_identifier/kinds.rs::TI_PLAIN_SEQUENCE_LARGE` (l. 74)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_sequence_of_int32_roundtrips`,
`crates/types/src/type_identifier/mod.rs::tests::plain_sequence_large_of_string_roundtrips`

**Status:** done

### §7.3.4.2 Table 29 — TI_PLAIN_ARRAY_SMALL, TI_PLAIN_ARRAY_LARGE

**Spec:** §7.3.4.2, Table 29, p. 125 — "TypeIdentifier contains array dimensions
and the TypeIdentifier of element. SMALL and LARGE indicate representation of
dimensions."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_ARRAY_SMALL` (l. 77);
`crates/types/src/type_identifier/kinds.rs::TI_PLAIN_ARRAY_LARGE` (l. 79)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_array_small_3d_roundtrips`,
`crates/types/src/type_identifier/mod.rs::tests::plain_array_large_roundtrips`

**Status:** done

### §7.3.4.2 Table 29 — TI_PLAIN_MAP_SMALL, TI_PLAIN_MAP_LARGE

**Spec:** §7.3.4.2, Table 29, p. 125 — "TypeIdentifier contains length of map
and the TypeIdentifier of key and element."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_MAP_SMALL` (l. 82);
`crates/types/src/type_identifier/kinds.rs::TI_PLAIN_MAP_LARGE` (l. 84)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_map_small_roundtrips`,
`crates/types/src/type_identifier/mod.rs::tests::plain_map_large_with_nested_map_value_roundtrips`

**Status:** done

### §7.3.4.2 Table 29 — TI_STRONGLY_CONNECTED_COMPONENT

**Spec:** §7.3.4.2, Table 29, p. 125 — "Types with mutual dependencies on other
types | Not plain type. Has TypeObject. Uses a Hash computed on the TypeObjects
of the set of mutually-dependent types."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRONGLY_CONNECTED_COMPONENT` (l. 87);
`crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId` (l. 176)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::strongly_connected_component_large_scc_length_and_index`,
`crates/types/tests/strongly_connected_components.rs::a_self_referential_type_gets_a_component_identifier`

**Status:** done — for the computation of the hash see §7.3.4.9.2.

### §7.3.4.2 Table 29 — EK_COMPLETE

**Spec:** §7.3.4.2, Table 29, p. 125 — "Not mutually dependent on other types |
Not plain type. Has TypeObject. Uses a Hash of the Complete TypeObject that
describes the type."

**Repo:** `crates/types/src/type_identifier/kinds.rs::EK_COMPLETE` (l. 92);
`crates/types/src/hash.rs::compute_complete_hash` (l. 78)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::the_complete_hash_is_the_one_rti_and_cyclone_both_report`

**Status:** done

### §7.3.4.2 Table 29 — EK_MINIMAL

**Spec:** §7.3.4.2, Table 29, p. 125 — "Uses a Hash of the Minimal TypeObject
that describes the type."

**Repo:** `crates/types/src/type_identifier/kinds.rs::EK_MINIMAL` (l. 90);
`crates/types/src/hash.rs::compute_minimal_hash` (l. 53)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_minimal_object_is_cyclones_octet_for_octet`,
`crates/types/src/hash.rs::tests::minimal_and_complete_same_semantic_differ_in_hash`

**Status:** done

### §7.3.4.2 Table 29 — TK_ANNOTATION

**Spec:** §7.3.4.2, Table 29, p. 126 — "TK_ANNOTATION | Annotation Declaration |
Not plain type. Has TypeObject. Uses Hash of the TypeObject representation of
the Annotation declaration".

**Repo:** `crates/types/src/from_ast.rs::annotation_type_object` (l. 537)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_typeid_carries_the_hash_of_the_declarations_own_object`

**Status:** done — the annotation declaration gets a TypeObject, and its
identifier is that object's hash (`EK_MINIMAL`/`EK_COMPLETE`). `TK_ANNOTATION`
as a discriminator of a TypeIdentifier of its own is not accepted, because the
`union TypeIdentifier` of Annex B has no `case` for it
(`docs/omg-xtypes-issues.md` §B9; contradiction in the specification).

### §7.3.4.2 Table 29 — TI_EXTENDED

**Spec:** §7.3.4.2, Table 29, p. 126 — "TI_EXTENDED | | Reserved for future
extensions".

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (l. 197, variant `Extended`)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::an_extended_arm_with_a_member_list_round_trips_verbatim`

**Status:** done — Annex B has no constant `TI_EXTENDED` but the branch
`default: ExtendedTypeDefn`; the tree reads an unknown discriminator into this
branch (contradiction in the specification, listed in the findings file). Under XCDR1 the arm is a parameter list that `PID_LIST_END` closes (the type is MUTABLE and empty); an unknown member in it is stepped over, a must-understand member refuses the value, and members read under XCDR2 for a type of a later revision have no XCDR1 form (`320c16503d2`, tests in `crates/types/tests/type_lookup_xcdr1.rs`).

### §7.3.4.3 Complete TypeObject: back to IDL without loss

**Spec:** §7.3.4.3, p. 126 — "Any non-plain type represented in IDL can be
converted to the Complete TypeObject representation and back to IDL with no
information loss, other than formatting".

**Repo:** `crates/types/src/from_ast.rs::complete_type_object`;
`crates/types/src/from_ast.rs::type_builtin_annotations`;
`crates/types/src/dynamic/to_ast.rs::apply_type_verbatim`

**Tests:** `crates/types/tests/type_verbatim_round_trip.rs::a_types_verbatim_is_in_its_complete_object_after_the_wire`,
`crates/types/tests/type_verbatim_round_trip.rs::the_verbatim_comes_back_into_the_rebuilt_model`,
`crates/types/tests/type_verbatim_round_trip.rs::more_than_one_verbatim_on_a_type_is_refused_by_name`,
`crates/types/tests/type_object_annotations.rs::the_annotations_come_back_into_the_rebuilt_model`,
`crates/types/tests/type_object_bridge.rs::a_type_survives_its_own_wire_form`,
`crates/types/tests/strongly_connected_components.rs::a_component_rebuilds_into_a_model`

**Status:** done — Structure, union, enumeration, bitmask, bitset, alias, annotations (parameters of type `long double`, `wchar`, `wstring` too) and the `@verbatim` of a type come back; it is in the `AppliedBuiltinTypeAnnotations` of the `CompleteTypeDetail` (placement as §7.2.2.4.8.2 spells it, `string<32>` checked). Annex B has room for one `@verbatim`, §7.2.2.4.8 allows several: a type with more than one is refused by name and not written without the others (contradiction in the findings file). The MINIMAL object holds no `@verbatim`.

### §7.3.4.4 Minimal TypeObject: only the information relevant for assignability

**Spec:** §7.3.4.4, p. 126 — "This representation does not include information
on the type that would not impact type assignability. For example user-defined
annotations or the order of members for types with extensibility kind MUTABLE."

**Repo:** `crates/types/src/from_ast.rs::minimal_type_object`;
`crates/types/src/type_object/to_minimal.rs`

**Tests:** `crates/types/tests/minimal_object_7_3_4_4.rs::user_defined_annotations_are_not_in_the_minimal_object`,
`crates/types/tests/minimal_object_7_3_4_4.rs::the_members_of_a_mutable_type_are_in_declaration_order`,
`crates/types/tests/minimal_object_7_3_4_4.rs::a_minimal_member_has_the_hash_of_its_name`,
`crates/types/tests/type_object_custom_annotations.rs::the_minimal_form_leaves_the_applied_annotation_out`,
`crates/types/tests/typeobject_annotation_annexb.rs::the_minimal_object_is_annex_bs_octet_for_octet`

**Status:** done — The MINIMAL object holds no user-defined annotation, no `@verbatim` and no names (only the `NameHash`); equal types with and without an annotation have the same MINIMAL object and the same MINIMAL identifier. The members of a MUTABLE type are in declaration order (`member_index`, §7.3.4.5 with the ordering rules of the serialization); the example sentence in §7.3.4.4 says the opposite (contradiction in the findings file). Two camps of vendors compute different hashes for a MINIMAL struct with a nested aggregate; the tree chooses through `minimal_member_trailing_octet` (`docs/omg-xtypes-issues.md` §C2, a named setting).

### §7.3.4.5 Serialization: XCDR2, little endian

**Spec:** §7.3.4.5, p. 126 — "The serialization of a TypeObject shall happen in
accordance to its IDL declaration and the general serialization rules defined in
this specification (see Clause 7.4) for XCDR encoding version 2. [...] The
serialization shall use Little Endian encoding."

**Repo:** `crates/types/src/type_object/mod.rs::to_bytes_le` (l. 115)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/tests/typeobject_vendor_dheader.rs::the_first_four_octets_announce_everything_after_them`,
`crates/types/tests/typeobject_annotation_annexb.rs::every_appendable_and_sequence_carries_its_own_length`

**Status:** done

### §7.3.4.5 Order: AnnotationParameterSeq by paramname_hash

**Spec:** §7.3.4.5, p. 126 — "The elements in AnnotationParameterSeq shall be
ordered in increasing values of their paramname_hash _typeid."

**Repo:** `crates/types/src/from_ast.rs::applied_parameters` (l. 2011, sorting l. 2078)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_parameters_are_ordered_by_name_hash`

**Status:** done — "paramname_hash _typeid" is a typo of the specification
(listed in the findings file).

### §7.3.4.5 Order: AppliedAnnotationSeq by annotation_typeid

**Spec:** §7.3.4.5, p. 126 — "The elements in AppliedAnnotationSeq shall be
ordered in increasing values of their annotation_typeid."

**Repo:** `crates/types/src/from_ast.rs::applied_annotations` (l. 1958, sorting l. 1985)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_applications_are_ordered_by_annotation_typeid`

**Status:** done

### §7.3.4.5 Order: struct and union members by member_index

**Spec:** §7.3.4.5, p. 126–127 — "The elements in CompleteStructMemberSeq [...]
MinimalStructMemberSeq [...] CompleteUnionMember [...] MinimalUnionMember shall
be ordered in increasing values of the member_index."

**Repo:** `crates/types/src/from_ast.rs::structure` (l. 2082);
`crates/types/src/from_ast.rs::union` (l. 2178)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/src/assignability.rs::tests::flatten_inheritance_two_levels_concatenates_base_first`

**Status:** done — the members are written in declaration order.

### §7.3.4.5 Order: annotation members

**Spec:** §7.3.4.5, p. 127 — "The elements in CompleteAnnotationMemberSeq shall
be ordered in increasing values of the member_index. The elements in
MinimalAnnotationMemberSeq shall be ordered in increasing values of the
member_name hash."

**Repo:** `crates/types/src/type_object/complete/annotation_type.rs::AnnotationParameterOrder`;
`crates/types/src/type_object/minimal/annotation_type.rs::ordered_by_name_hash`

**Tests:** `crates/types/tests/annotation_parameter_order.rs::the_parameters_follow_member_index_and_the_annex_b_comment_is_a_setting`,
`crates/types/tests/annotation_parameter_order.rs::the_minimal_form_is_ordered_by_the_hash_of_the_name_in_both_readings`,
`crates/types/tests/typeobject_annotation_annexb.rs::each_sequence_is_ordered_by_its_own_key`

**Status:** done — MINIMAL by the hash of the name; COMPLETE by `member_index`, the declaration order (§7.3.4.5, the normative text; the maintainer's decision). The reading of Annex B ("Ordered by CompleteAnnotationParameter.name") is the named setting `xtypes.annotation_parameter_order: name`, environment variable `ZERODDS_ANNOTATION_PARAMETER_ORDER`; the order is in the object and so in the `annotation_typeid` of every application. The contradiction is in the findings file.

### §7.3.4.5 Order: CompleteAnnotationMemberSeq by member_index

**Spec:** §7.3.4.5, p. 127 — `member_index`.

**Repo:** `crates/types/src/from_ast.rs::annotation_type_object`;
`crates/types/src/type_object/complete/annotation_type.rs::annotation_parameter_order`

**Tests:** `crates/types/tests/annotation_parameter_order.rs::the_parameters_follow_member_index_and_the_annex_b_comment_is_a_setting`,
`crates/types/tests/annotation_parameter_order.rs::the_setting_has_a_spelling_that_reads_back`,
`crates/types/tests/type_object_annotations.rs::an_annotation_declaration_has_a_type_object`

**Status:** done — `CompleteAnnotationParameterSeq` is ordered by `member_index`, the position of the parameter in the declaration (`CommonAnnotationParameter` has no member id); the test declares `zebra`, `alpha`, `mid` and expects that order, and under the setting `name` the order of the names. The hash of the annotation type changes with the setting. Expectation for Cyclone DDS, Fast DDS and OpenDDS: none of them writes `ann_custom` with annotation types; not measured.

### §7.3.4.5 Order: EnumeratedLiteralSeq by numeric value

**Spec:** §7.3.4.5, p. 127 — "The elements in CompleteEnumeratedLiteralSeq [...]
MinimalEnumeratedLiteralSeq shall be ordered in increasing values of their
numeric value."

**Repo:** `crates/types/src/from_ast.rs::enumeration`

**Tests:** `crates/types/tests/literal_and_flag_order.rs::enumerated_literals_are_written_in_increasing_value_order`,
`crates/types/src/type_object/mod.rs::tests::minimal_enum_color_3_literals`

**Status:** done — The literals are written in increasing order of value, also when `@value` reverses the declaration order.

### §7.3.4.5 Order: literals are sorted by value

**Spec:** §7.3.4.5, p. 127 — ascending by numeric value.

**Repo:** —

**Tests:** `crates/types/tests/literal_and_flag_order.rs::enumerated_literals_are_written_in_increasing_value_order`

**Status:** done — `enum E { @value(7) A, @value(0) B };` has `B` before `A` in the TypeObject and in the hashed stream.

### §7.3.4.5 Order: BitflagSeq by position

**Spec:** §7.3.4.5, p. 127 — "The elements in CompleteBitflagSeq [...]
MinimalBitflagSeq shall be ordered in increasing values of their position."

**Repo:** `crates/types/src/from_ast.rs::bitmask`

**Tests:** `crates/types/tests/literal_and_flag_order.rs::bitmask_flags_are_written_in_increasing_position_order`,
`crates/types/src/type_object/mod.rs::tests::minimal_bitmask_3_flags`

**Status:** done — The flags are written in increasing order of position.

### §7.3.4.5 Order: bitflags are sorted by position

**Spec:** §7.3.4.5, p. 127 — ascending by position.

**Repo:** —

**Tests:** `crates/types/tests/literal_and_flag_order.rs::bitmask_flags_are_written_in_increasing_position_order`

**Status:** done — With `@position` in descending order the flags are sorted by position all the same.

### §7.3.4.5 Order: BitfieldSeq by position

**Spec:** §7.3.4.5, p. 127 — "The elements in CompleteBitfieldSeq [...]
MinimalBitfieldSeq shall be ordered in increasing values of their position."

**Repo:** `crates/types/src/from_ast.rs` (Bitset, positions consecutive l. 2549–2600)

**Tests:** `crates/types/src/builder.rs::tests::bitset_builder_two_fields`

**Status:** done — positions are assigned consecutively, so the sequence is
ascending.

### §7.3.4.6.1 Fully-descriptive TypeIdentifiers, EK_BOTH

**Spec:** §7.3.4.6.1, p. 127 — "The TypeIdentifiers for Primitive and String
types. The TypeIdentifiers of plain collections where the element (and key)
TypeIdentifer a fully descriptive TypeIdentifier. They are recognized by the
contained PlainCollectionHeader having EquivalenceKind set to EK_BOTH."

**Repo:** `crates/types/src/from_ast.rs` (header, l. 1443–1456);
`crates/types/src/type_identifier/kinds.rs::EK_BOTH` (l. 98)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::ek_both_is_not_a_type_identifier_discriminator`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_nested_sequence_of_sequence_minimal_wire_roundtrip`

**Status:** done

### §7.3.4.6.2 Hash TypeIdentifiers

**Spec:** §7.3.4.6.2, p. 127–128 — "Those with discriminator EK_MINIMAL,
EK_COMPLETE, or TI_STRONG_COMPONENT. Those with discriminator
TI_PLAIN_SEQUENCE_SMALL, [...] TI_PLAIN_MAP_LARGE where the contained
PlainCollectionHeader has EquivalenceKind EK_MINIMAL or EK_COMPLETE."

**Repo:** `crates/types/src/type_identifier/mod.rs::PlainCollectionHeader` (l. 122)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_large_complete_wire_vector`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_map_small_minimal_keys_complete_values_wire_vector`

**Status:** done — the specification writes `TI_STRONG_COMPONENT` here, Annex B
`TI_STRONGLY_CONNECTED_COMPONENT` (listed in the findings file).

### §7.3.4.6.3 Direct Hash TypeIdentifiers

**Spec:** §7.3.4.6.3, p. 128 — "These are the HASH TypeIdentifiers with
discriminator EK_MINIMAL, EK_COMPLETE, or TI_STRONG_COMPONENT."

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (l. 197, variants `EquivalenceHashMinimal`,
`EquivalenceHashComplete`,
`StronglyConnectedComponent`)

**Tests:** `crates/types/src/hash.rs::tests::to_hashed_type_identifier_picks_correct_kind`

**Status:** done

### §7.3.4.6.4 Indirect Hash TypeIdentifiers

**Spec:** §7.3.4.6.4, p. 128 — "These are the TypeIdentifiers for plain
collections with the element type identified using a HASH TypeIdentifier. [...]
Having the contained PlainCollectionHeader with EquivalenceKind EK_MINIMAL or
EK_COMPLETE."

**Repo:** `crates/types/src/type_identifier/mod.rs::PlainCollectionHeader` (l. 122)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_array_small_minimal_wire_vector`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_array_large_complete_wire_vector`

**Status:** done

### §7.3.4.6.5 Minimal Hash TypeIdentifiers

**Spec:** §7.3.4.6.5, p. 128 — EK_MINIMAL, TI_STRONG_COMPONENT with
TypeObjectHashId EK_MINIMAL, plain collections with EquivalenceKind EK_MINIMAL.

**Repo:** `crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId` (l. 176, field `equivalence_kind`)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### §7.3.4.6.6 Complete Hash TypeIdentifiers

**Spec:** §7.3.4.6.6, p. 129 — EK_COMPLETE, TI_STRONG_COMPONENT with
TypeObjectHashId EK_COMPLETE, plain collections with EquivalenceKind
EK_COMPLETE.

**Repo:** `crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId` (l. 176)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_large_complete_wire_vector`

**Status:** done

### §7.3.4.6 PlainCollectionHeader: unknown EquivalenceKind

**Spec:** §7.3.4.6.1–§7.3.4.6.6, p. 127–129 — the EquivalenceKind in the
PlainCollectionHeader is EK_BOTH, EK_MINIMAL or EK_COMPLETE.

**Repo:** `crates/types/src/type_identifier/mod.rs::EquivalenceKind` (l. 85)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_invalid_equiv_kind_in_header_decodes_as_unknown_ek_byte`

**Status:** done — another octet is read and kept unchanged.

### §7.3.4.7 Type Equivalence: Complete and Minimal

**Spec:** §7.3.4.7, p. 129–130 — "Two types are equivalent according to the
Complete equivalence relation if and only if either they have equal
Fully-Descriptive TypeIdentifiers, or else they have equal Complete
TypeIdentifiers. Two types are equivalent according to the Minimal equivalence
relation if and only if either they have equal Fully-Descriptive
TypeIdentifiers, or else they have equal Minimal TypeIdentifiers."

**Repo:** `crates/types/src/assignability.rs::is_assignable` (l. 274, check `w == r` l. 301);
`crates/types/src/hash.rs::compute_hash` (l. 37)

**Tests:** `crates/types/src/hash.rs::tests::different_type_objects_have_different_hashes`,
`crates/types/src/hash.rs::tests::hash_is_14_bytes_and_deterministic`,
`crates/types/src/assignability.rs::tests::equal_complete_hash_is_assignable_with_empty_registry`

**Status:** done

### §7.3.4.7 Equivalence relations: set theory, finer/coarser

**Spec:** §7.3.4.7, p. 129 — reflexive, symmetric, transitive; partition into
classes; "finer" and "coarser".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition of terms.

### §7.3.4.8 Types with mutual dependencies

**Spec:** §7.3.4.8, p. 130 — recursive types such as `TreeNode` with
`sequence<@external TreeNode>`; the simple hash algorithm fails at the circular
dependency.

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component` (l. 236)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_self_referential_type_gets_a_component_identifier`,
`crates/types/tests/strongly_connected_components.rs::a_mutually_recursive_pair_is_one_component_of_two`

**Status:** done — the example uses `sequence<@external TreeNode>`, a form the
front end refuses (open under §7.3.1.2.1.4); recursion through a sequence
without `@external` is built.

### §7.3.4.8.1 Graph theory

**Spec:** §7.3.4.8.1, p. 130–131 — dependency digraph, path, cycle,
reachability, strongly connected components, kernel DAG, Figure 21.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition of terms.

### §7.3.4.9.1 Introduction

**Spec:** §7.3.4.9.1, p. 131–132 — Figure 22; the algorithm is run once per
equivalence relation.

**Repo:** `crates/types/src/from_ast.rs::component_identifier` (l. 440)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_component_members_type_object_builds`

**Status:** done — `form` selects MINIMAL or COMPLETE.

### §7.3.4.9.2 Step 1: graph without a cycle — ordinary hash

**Spec:** §7.3.4.9.2, p. 132 — "If this graph has no cycles, then T is not
affected by mutual dependencies and the TypeIdentifier can use the regular
algorithm of hashing the serialized TypeObjects".

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component` (l. 236)

**Tests:** `crates/types/tests/strongly_connected_components.rs::an_ordinary_type_is_not_given_a_component_identifier`

**Status:** done

### §7.3.4.9.2 Steps 2 and 3: remove vertices without outgoing edges, determine components

**Spec:** §7.3.4.9.2, p. 133 — "Let ReducedDependencyDG(T) be the subdigraph
[...] where all the vertices that have no outgoing edges are removed. [...]
Identify the Strongly Connected Components of the ReducedDependencyDG(T)."

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component` (l. 236);
`crates/types/src/from_ast.rs::reachable` (l. 189)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_mutually_recursive_pair_is_one_component_of_two`,
`crates/types/tests/strongly_connected_components.rs::a_type_that_holds_a_component_member_from_outside_builds`

**Status:** done — the component is the set of types that reach `T` and are
reached by `T`; plain collections are no members.

### §7.3.4.9.2 Step 4a: component with one type

**Spec:** §7.3.4.9.2, p. 133 — "If the Strongly Connected Component (SCC) has a
single type, then use the regular algorithm to compute its TypeIdentifier".

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component`

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_type_that_holds_a_component_member_from_outside_builds`,
`crates/types/tests/strongly_connected_components.rs::an_ordinary_type_is_not_given_a_component_identifier`

**Status:** done — A type that is itself on no cycle and only reaches a component gets the ordinary hash; its TypeObject carries the SCC identifier of the member it reaches.

### §7.3.4.9.2 Step 4a: a self-referential type gets an SCC identifier

**Spec:** §7.3.4.9.2, p. 133 — a component with one type uses the ordinary
algorithm.

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component`

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_self_referential_type_gets_a_component_identifier`,
`crates/types/tests/strongly_connected_components.rs::a_component_members_type_object_builds`,
`crates/types/tests/strongly_connected_components.rs::the_component_hash_is_the_md5_of_the_xcdr2_sequence_of_its_objects`

**Status:** done — Step 4a: a component of one type uses the ordinary algorithm. A type that reaches itself has no hash that way, because its TypeObject would hold the identifier it yields; it is treated as a component of one type by steps 4b to 4f (`TI_STRONGLY_CONNECTED_COMPONENT` with `scc_length = 1`, `scc_index = 1`). The specification names no other way for this case (contradiction in the findings file). A comparison with the identifier of another vendor is not possible: none in the matrix computes an SCC identifier.

### §7.3.4.9.2 Step 4b: sorting by fully qualified name, index from 1

**Spec:** §7.3.4.9.2, p. 133 — "sort them using the lexicographic order of their
fully qualified type name. Let SCCIndex(U) be the sort index of each type U
belonging to the SCC starting with index 1 for the first type. For anonymous
types concatenate the fully-qualified name of the containing type with the
member name using '.' as the separator".

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component` (l. 236, sorting l. 265);
`crates/types/src/from_ast.rs::Descent` (l. 88, index from 1)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_mutually_recursive_pair_is_one_component_of_two`

**Status:** done — anonymous types do not occur as members (plain collections
are excluded), so the naming rule for them is not needed.

### §7.3.4.9.2 Step 4b.i: provisional identifier {EK, hash = 0}, scc_length, scc_index is set

**Spec:** §7.3.4.9.2, p. 133 — "Temporarily set the TypeIdentifier of each U
belonging to the SC to: discriminator = TI_STRONGLY_CONNECTED_COMPONENT,
sc_component_id = {discriminator=EK, hash= 0}, scc_length = Number of types in
SCC, scc_index = SCCIndex(U)".

**Repo:** `crates/types/src/from_ast.rs::component_hash`

**Tests:** `crates/types/tests/strongly_connected_components.rs::the_component_hash_is_the_md5_of_the_xcdr2_sequence_of_its_objects`

**Status:** done — The hash input of the component carries the provisional identifier with hash 0, `scc_length` and `scc_index` (step 4b.i).

### §7.3.4.9.2 Step 4b.ii: TypeObjects with temporary identifiers

**Spec:** §7.3.4.9.2, p. 133 — "Construct the TypeObject of all the types in the
SC using the temporary TypeIdentifier for references to other types in the SCC."

**Repo:** `crates/types/src/from_ast.rs::component_hash` (l. 466)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_component_members_type_object_builds`,
`crates/types/tests/strongly_connected_components.rs::the_component_hash_is_the_md5_of_the_xcdr2_sequence_of_its_objects`

**Status:** done — The TypeObjects of the component are built with the provisional identifiers (step 4b.ii).

### §7.3.4.9.2 Steps 4c to 4e: TypeObjectSeq, XCDR2 LE, MD5, 14 octets

**Spec:** §7.3.4.9.2, p. 133 — "Place computed TypeObjects [...] into a sequence
TypeObjectSeq in the order of their scc_index. Serialize the TypeObjectSeq using
the XCDR serialization for sequences with encoding version 2 and little endian.
Compute the MD5 hash of the serialized buffer. Let EquivalenceHash(SC) be the
first 14 bytes."

**Repo:** `crates/types/src/from_ast.rs::component_hash` (l. 466)

**Tests:** `crates/types/tests/strongly_connected_components.rs::the_component_hash_is_the_md5_of_the_xcdr2_sequence_of_its_objects`,
`crates/types/tests/strongly_connected_components.rs::the_component_hash_is_deterministic`

**Status:** done — The stream is the `sequence<TypeObject>` under XCDR2 little endian with a DHEADER and alignment to four; the hash is MD5, the first 14 octets count (steps 4c to 4e).

### §7.3.4.9.2 Hash input of the component with DHEADER and alignment

**Spec:** §7.3.4.9.2, p. 133 — XCDR2 serialization of the `TypeObjectSeq`.

**Repo:** —

**Tests:** `crates/types/tests/strongly_connected_components.rs::the_component_hash_is_the_md5_of_the_xcdr2_sequence_of_its_objects`

**Status:** done — `component_hash` writes the DHEADER and the alignment of the sequence. A comparison with the SCC hash of another vendor is not possible.

### §7.3.4.9.2 Step 4f: final identifier per type

**Spec:** §7.3.4.9.2, p. 133–134 — "Set the TypeIdentifier of each of the types
in SC to: discriminator = TI_STRONGLY_CONNECTED_COMPONENT, strong_component_id =
StronglyConnectedComponentId(SC), scc_index = SCCIndex(U)".

**Repo:** `crates/types/src/from_ast.rs::component_identifier` (l. 440)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_mutually_recursive_pair_is_one_component_of_two`

**Status:** done

### §7.3.4.9.2 Implementation notes

**Spec:** §7.3.4.9.2, p. 134 — forward and backward reachability,
Kosaraju-Sharir.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — "non-normative".

### §7.3.4.9.3 SCCIdentifier: scc_index = 0 can be encoded

**Spec:** §7.3.4.9.3, p. 134 — "There are situations where an SCC needs to be
identified without referencing a concrete type inside the SCC. In this situation
a TypeIdentifier is constructed the same way as for any of the types in the SCC
except the scc_index field is set to 0."

**Repo:** —

**Tests:** `crates/types/tests/strongly_connected_components.rs::an_scc_identifier_with_index_zero_is_written_and_read`

**Status:** done — `TypeIdentifier::to_bytes_le` and the reader accept `scc_index = 0`.

### §7.3.4.9.3 StronglyConnectedComponentId: scc_length and hash

**Spec:** §7.3.4.9.3, p. 134 — "The StronglyConnectedComponentId contains the
number of types in the strongly connected component (field scc_length) and a
hash of all the corresponding TypeObjects (field sc_component_id). [...] The
TypeIdentifiers of all the types belonging to the same SCC only differ on the
scc_index field, which always takes values from 1 to scc_length."

**Repo:** `crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId` (l. 176)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::scc_encode_rejects_index_out_of_bounds`,
`crates/types/src/type_identifier/mod.rs::tests::scc_encode_rejects_negative_values`

**Status:** done

---

## §7.4 Data Representation

### §7.4 Figure 23 — DataRepresentationId_t: XCDR 0, XML 1, XCDR2 2

**Spec:** §7.4, Figure 23, p. 135 — "XCDR_DATA_REPRESENTATION = 0,
XML_DATA_REPRESENTATION = 1, XCDR2_DATA_REPRESENTATION = 2".

**Repo:** `crates/types/src/qos.rs::DataRepresentationId` (l. 174)

**Tests:** `crates/types/src/qos.rs::tests::negotiate_xcdr2_preferred`

**Status:** done

### §7.4 Table 30 — Alternative Data Representations

**Spec:** §7.4, Table 30, p. 136 — Extended CDR and XML with their advantages
and disadvantages.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — comparison.

---

## §7.4.1 Extended CDR Representation (encoding version 1)

### §7.4.1 Two formats: PLAIN_CDR and PL_CDR

**Spec:** §7.4.1, p. 136 — "These extensions result in two encoding formats:
PLAIN_CDR and PL_CDR. [...] PLAIN_CDR introduces extensions to CDR in order to
handle optional members, bitmasks, and maps. PL_CDR leverages the RTPS Parameter
List encoding to handle mutable types."

**Repo:** `crates/ast-layout/src/key.rs::Encoding` (variants `PlainCdr1`,
`PlCdr1`)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::plain_cdr1_plans_no_sentinel`,
`crates/ast-layout/tests/xcdr1_gate.rs::a_mutable_struct_plans_the_pid_list_end_sentinel`

**Status:** done

### §7.4.1.1 PLAIN_CDR for final, appendable, strings, sequences, maps; PL_CDR for mutable

**Spec:** §7.4.1.1, p. 136 — "The PLAIN_CDR encoding shall be used for final and
appendable types, including (trivially) primitive types. It shall also be used
for all string, sequence, and map types. Aggregated types declared as mutable
shall use the PL_CDR encoding".

**Repo:** `crates/zerodds-build/src/lib.rs::xcdr1_encoding_for` (l. 978);
`crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/ast-layout/tests/vendor_gate.rs::appendable_adds_a_delimiter_under_xcdr2_and_nothing_under_xcdr1`

**Status:** done

### §7.4.1.1 Padding bytes at the end of the serialized data are zero

**Spec:** §7.4.1.1, p. 136 — "This XTYPES specification further requires that
any padding bytes added at the end of the serialized data shall be set to zero."

**Repo:** `crates/wire/src/writer.rs::align_to` (l. 1415);
`crates/wire-codec/src/value.rs::encapsulation_header`

**Tests:** `crates/wire/tests/padding_record.rs::every_recorded_octet_is_a_zero_the_writer_wrote`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_drops_the_padding_the_options_name`

**Status:** done — every alignment writes zero octets; the number of padding
bytes at the end is in the field `options` of the encapsulation header.

### §7.4.1.1.1 Primitive types: aligned to their size, byte swap by cendian

**Spec:** §7.4.1.1.1, p. 137 — "The serialized data shall be encoded at an
offset that aligned to the size of the primitive type. An endianness byte swap
shall be performed in case the native system endianness is different from the
one currently configured in the XCDR stream (XCDR.cendian)."

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (l. 456);
`crates/wire/src/writer.rs::align_to` (l. 1415)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`,
`crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`,
`crates/wire/tests/wire.rs::a_long_after_a_char_sits_at_four_not_at_one`

**Status:** done

### §7.4.1.1.1 Table 31 — Byte: 1 octet, alignment 1

**Spec:** §7.4.1.1.1, Table 31, p. 137 — "Byte | 1 | 1 | The byte value".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (l. 456)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`

**Status:** done

### §7.4.1.1.1 Table 31 — Boolean: 1 octet, 0 or 1

**Spec:** §7.4.1.1.1, Table 31, p. 137 — "Boolean | 1 | 1 | 0 for false, 1 for
true".

**Repo:** `crates/wire/src/writer.rs::write_bool` (l. 1460)

**Tests:** `crates/wire/tests/wire.rs::a_boolean_that_is_neither_zero_nor_one_is_refused`

**Status:** done — an octet other than 0 or 1 is refused when reading.

### §7.4.1.1.1 Table 31 — Char8: 1 octet

**Spec:** §7.4.1.1.1, Table 31, p. 137 — "Char8 | 1 | 1 | The character value
encoded as described in 7.2.2.2.1.2".

**Repo:** `crates/wire/src/writer.rs::write_char` (l. 1524)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_char_and_a_wchar_encode_to_one_byte_and_two`

**Status:** done

### §7.4.1.1.1 Table 31 — Char16: 2 octets, alignment 2

**Spec:** §7.4.1.1.1, Table 31, p. 137 — "Char16 | 2 | 2 | The character value
encoded as described in 7.2.2.2.1.2".

**Repo:** `crates/wire/src/writer.rs::write_wchar` (l. 1559);
`crates/ast-layout/src/xcdr.rs::primitive_layout` (l. 456)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_char_and_a_wchar_encode_to_one_byte_and_two`

**Status:** done — the width can be set through `WcharWidth` (RTI under XCDR1:
four octets); the default is the table.

### §7.4.1.1.1 Table 31 — Int8, UInt8: 1 octet

**Spec:** §7.4.1.1.1, Table 31, p. 137 — "Int8, UInt8 | 1 | 1 | The integer
value using two's complement notation".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (l. 456)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`

**Status:** done

### §7.4.1.1.1 Table 31 — Int16, UInt16: 2 octets

**Spec:** §7.4.1.1.1, Table 31, p. 137 — "Int16, UInt16 | 2 | 2".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (l. 456)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`

**Status:** done

### §7.4.1.1.1 Table 31 — Int32, UInt32: 4 octets

**Spec:** §7.4.1.1.1, Table 31, p. 137 — "Int32, UInt32 | 4 | 4".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (l. 456)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`

**Status:** done

### §7.4.1.1.1 Table 31 — Int64, UInt64: 8 octets, alignment 8

**Spec:** §7.4.1.1.1, Table 31, p. 137 — "Int64, UInt64 | 8 | 8".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (l. 456)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::xcdr1_aligns_sixty_four_bit_primitives_to_eight`

**Status:** done

### §7.4.1.1.1 Table 31 — Float32: 4 octets, IEEE single

**Spec:** §7.4.1.1.1, Table 31, p. 137 — "Float32 | 4 | 4 | IEEE standard for
normalized single-precision floating-point numbers".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (l. 456)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`

**Status:** done

### §7.4.1.1.1 Table 31 — Float64: 8 octets, alignment 8

**Spec:** §7.4.1.1.1, Table 31, p. 137 — "Float64 | 8 | 8".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (l. 456)

**Tests:** `crates/wire/tests/wire.rs::a_double_aligns_to_eight`

**Status:** done

### §7.4.1.1.1 Table 31 — Float128: 16 octets, alignment 8

**Spec:** §7.4.1.1.1, Table 31, p. 137 — "Float128 | 16 | 8 | IEEE standard for
normalized quadruple-precision floating-point numbers".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (l. 456);
`crates/wire/src/writer.rs::write_f128` (l. 1864);
`crates/wire/src/reader.rs::read_f128` (l. 766)

**Tests:** `crates/ast-layout/tests/layout.rs::long_double_is_sixteen_bytes`,
`crates/float128/tests/binary128.rs::one_and_a_half_is_the_pattern_the_vendors_write`,
`crates/wire-probe/tests/interop_corpus.rs::every_case_produces_the_bytes_its_readme_records_in_all_four_modes`

**Status:** done — the corpus case `primitive-longdouble` runs through
`write_f128` in both versions and both byte orders; the octets are the ones
recorded in the repository, not a measurement of another vendor.

### §7.4.1.1.2 Char8 as Byte

**Spec:** §7.4.1.1.2, p. 137 — "Objects of Char8 type shall not be interpreted
to have a specific encoding and shall be serialized as-is in the same way as the
Byte primitive type."

**Repo:** `crates/wire/src/writer.rs::write_char` (l. 1524)

**Tests:** `crates/wire/tests/char_width.rs::every_code_point_a_char_can_hold_round_trips`

**Status:** done — a Rust `char` above U+00FF is refused because it is not one
octet.

### §7.4.1.1.2 String<Char8>: UTF-8, length with NUL

**Spec:** §7.4.1.1.2, p. 137–138 — "Objects of String<Char8> type shall be
represented using the UTF-8 character encoding. The serialized length [...]
shall be the number of bytes in the CDR buffer taken by the String<Char8>
characters, including the terminating NUL character."

**Repo:** `crates/wire/src/writer.rs::write_string` (l. 1881)

**Tests:** `crates/wire/tests/wire.rs::a_string_length_counts_its_terminator`,
`crates/wire/tests/wire.rs::an_empty_string_is_a_length_of_one_and_a_nul`,
`crates/wire/tests/wire.rs::a_string_whose_terminator_is_not_a_nul_is_refused`

**Status:** done

### §7.4.1.1.2 String<Char16>: UTF-16, length in octets, without BOM, without NUL

**Spec:** §7.4.1.1.2, p. 138 — "Objects of String<Char16> type shall be
represented using the UTF-16 character encoding. The serialized length [...]
shall be the number of bytes in the CDR buffer taken by the String<Char16>
characters. [...] The UTF-16 representation [...] shall not include a Byte Order
Mark (BOM). The representation shall also not include any terminating NUL
character(s)."

**Repo:** `crates/wire/src/writer.rs::write_wstring` (l. 1980);
`crates/ast-layout/src/key.rs::WstringLengthPolicy`

**Tests:** `crates/wire-conformance/tests/wstring_bytes.rs::the_unbounded_octets_are_the_recorded_cyclone_and_opendds_rows`,
`crates/wire/tests/wstring.rs::every_cell_round_trips_under_every_unit_order`

**Status:** done — `WstringLengthPolicy::SPECIFICATION` is the default; counting
characters, a NUL and four octets per unit are dialects.

### §7.4.1.1.2 Rationale

**Spec:** §7.4.1.1.2, p. 138 — rationale for the length in octets, the missing
BOM and the missing NUL.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale.

### §7.4.1.1.3.1 Table 32 — Enumeration with bit_bound 1 to 8: Int8

**Spec:** §7.4.1.1.3.1, Table 32, p. 138 — "Int8 | 1-8".

**Repo:** `crates/ast-layout/src/xcdr.rs::enumeration_bit_bound` (l. 1317);
`crates/ast-layout/src/xcdr.rs::bit_bound_size` (l. 1257)

**Tests:** `crates/wire-conformance/tests/enumeration_bytes.rs::an_enumeration_member_is_its_holder_and_round_trips`,
`crates/xml-compose/tests/idl_parity.rs::a_narrow_bit_bound_reaches_the_same_type_from_both_front_ends`

**Status:** done — `EnumHolderWidth` permits the dialect that writes every
enumeration in four octets.

### §7.4.1.1.3.1 Table 32 — bit_bound 9 to 16: Int16

**Spec:** §7.4.1.1.3.1, Table 32, p. 138 — "Int16 | 9-16".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (l. 1257)

**Tests:** `crates/wire-conformance/tests/enumeration_bytes.rs::an_enumeration_member_is_its_holder_and_round_trips`

**Status:** done

### §7.4.1.1.3.1 Table 32 — bit_bound 17 to 32: Int32, default

**Spec:** §7.4.1.1.3.1, Table 32, p. 138 — "Int32 | 17-32 (32 bits is the
default size, and corresponds to all enumerated types prior to this
specification)".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (l. 1257);
`crates/ast/src/lib.rs::DEFAULT_BIT_BOUND` (l. 66)

**Tests:** `crates/idl-lower/tests/completion.rs::an_enum_without_a_bit_bound_is_thirty_two_bits`,
`crates/wire-conformance/tests/enumeration_bytes.rs::an_enumeration_member_is_its_holder_and_round_trips`

**Status:** done

### §7.4.1.1.3.2 Table 33 — Bitmask [1..8]: UInt8

**Spec:** §7.4.1.1.3.2, Table 33, p. 138 — "[1..8] | UInt8".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (l. 1257)

**Tests:** `crates/wire-conformance/tests/bitmask_bytes.rs::a_bitmask_takes_the_holder_its_bit_bound_names`

**Status:** done

### §7.4.1.1.3.2 Table 33 — Bitmask [9..16]: UInt16

**Spec:** §7.4.1.1.3.2, Table 33, p. 138 — "[9..16] | UInt16".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (l. 1257)

**Tests:** `crates/wire-conformance/tests/bitmask_bytes.rs::a_bitmask_takes_the_holder_its_bit_bound_names`,
`crates/wire-conformance/tests/bitmask_bytes.rs::a_bound_that_is_not_a_holder_width_still_takes_the_holder`

**Status:** done

### §7.4.1.1.3.2 Table 33 — Bitmask [17..32]: UInt32

**Spec:** §7.4.1.1.3.2, Table 33, p. 139 — "[17..32] | UInt32".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (l. 1257)

**Tests:** `crates/wire-conformance/tests/bitmask_bytes.rs::a_bitmask_takes_the_holder_its_bit_bound_names`

**Status:** done

### §7.4.1.1.3.2 Table 33 — Bitmask [33..64]: UInt64

**Spec:** §7.4.1.1.3.2, Table 33, p. 139 — "[33..64] | UInt64".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (l. 1257)

**Tests:** `crates/wire-conformance/tests/bitmask_bytes.rs::a_bitmask_takes_the_holder_its_bit_bound_names`

**Status:** done

### §7.4.1.1.3.2 Bitmask: bit index from the LSB, bits above the bound not part of the mask

**Spec:** §7.4.1.1.3.2, p. 139 — "Bit indexes are counted from zero starting at
the least-significant bit of the full byte size of the bitmask. In the case
where the bound of the bitmask is less than the number of bits in the
corresponding primitive type, the states of the remaining serialized bits are
not specified, and those bits are not considered to be part of the bitmask."

**Repo:** `crates/wire-codec/src/decode.rs::read_bitmask` (l. 2499)

**Tests:** `crates/wire-conformance/tests/bitmask_bytes.rs::the_bytes_are_the_c_and_go_columns`,
`crates/wire-conformance/tests/bitmask_bytes.rs::a_bit_the_type_does_not_declare_is_not_part_of_the_value`

**Status:** done — The bit index runs from the lowest bit; a bit above the bound or at a position not declared is not part of the value and is dropped on reading.

### §7.4.1.1.3.2 Bits above the bound are dropped

**Spec:** §7.4.1.1.3.2, p. 139 — "the states of the remaining serialized bits
are not specified, and those bits are not considered to be part of the bitmask."

**Repo:** `crates/wire-codec/src/decode.rs::read_bitmask`

**Tests:** `crates/wire-codec/tests/bitmask_unnamed_bits.rs::a_bit_inside_the_bound_that_the_type_does_not_name_is_not_read`,
`crates/wire/tests/wire.rs::a_bitmask_read_keeps_only_the_declared_bits`

**Status:** done — `read_bitmask` returns no error for such a bit; the reader in `lang-rules` masks likewise.

### §7.4.1.1.4 Map as a sequence of MapEntry

**Spec:** §7.4.1.1.4, p. 139 — "Objects of map types shall be represented
according to the following equivalent IDL: struct
MapEntry_<key_type>_<value_type> [...] { <key_type> key; <value_type> value; };
typedef sequence<MapEntry_...> Map_...;"

**Repo:** `crates/wire/src/writer.rs::write_map`;
`crates/wire/src/reader.rs::read_map` (l. 1764)

**Tests:** `crates/wire-conformance/tests/map_bytes.rs::the_length_word_is_the_entry_count_and_not_the_number_of_halves`,
`crates/wire-conformance/tests/map_bytes.rs::the_bytes_are_the_recorded_fast_dds_row`

**Status:** done

### §7.4.1.1.5 Structures by CDR

**Spec:** §7.4.1.1.5, p. 139 — "Objects of structure type shall be represented
as defined by the CDR specification [CDR], augmented as described below."

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_aggregate` (l. 1720)

**Tests:** `crates/ast-layout/tests/layout.rs::a_final_struct_pads_between_members`,
`crates/ast-layout/tests/vendor_gate.rs::point_is_sixteen_bytes_under_xcdr1_and_twelve_under_xcdr2`

**Status:** done

### §7.4.1.1.5.1 Inheritance: base members first

**Spec:** §7.4.1.1.5.1, p. 139 — "The members defined by the base type, if any,
shall be serialized before the members of their derived types. The
representation shall be exactly as if all of the members had been defined, in
the same order, in the most-derived type."

**Repo:** `crates/ast/src/query.rs::all_members_of` (l. 151);
`crates/ast-layout/src/xcdr.rs::plan_aggregate` (l. 1720)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::an_inherited_member_is_written_ahead_of_the_derived_ones`,
`crates/wire-probe/tests/vendor_bytes.rs::member_ids_continue_across_the_inheritance_boundary`,
`crates/ast-layout/tests/layout.rs::a_bitset_that_inherits_is_sized_over_the_whole_chain`

**Status:** done

### §7.4.1.1.5.2 Optional members: a parameter header ahead

**Spec:** §7.4.1.1.5.2, p. 139 — "Structure members marked as optional shall be
preceded by a parameter header as described in Clause 7.4.1.2".

**Repo:** `crates/ast-layout/src/plan.rs::MemberHeader` (l. 93, variant `OptionalCdr1`)

**Tests:** `crates/ast-layout/tests/layout.rs::an_optional_member_carries_a_header_in_every_extensibility`,
`crates/ast-layout/tests/wire_vectors.rs::v11_optional_member_plans_a_header_regardless_of_presence`

**Status:** done

### §7.4.1.2 PL_CDR: parameter header of ID and length, sentinel

**Spec:** §7.4.1.2, p. 140 — "Preceding each one is a parameter header
consisting of a two-byte parameter ID followed by a two-byte parameter length.
One parameter follows another until a list-terminating sentinel is reached."

**Repo:** `crates/ast-layout/src/plan.rs::MemberHeader` (l. 93, variant `PidCdr1`);
`crates/wire/src/reader.rs::read_pid` (l. 1344)

**Tests:** `crates/wire/tests/wire.rs::the_sentinel_ends_a_parameter_list`,
`crates/wire-conformance/tests/bytes.rs::a_mutable_xcdr1_struct_round_trips_through_its_sentinel`

**Status:** done

### §7.4.1.2 Parameter length without padding, alignment to 4

**Spec:** §7.4.1.2, p. 140 — "the value of the parameter length is the exact
length of the serialized member. It does not account for any padding bytes that
may follow the serialized member. Padding bytes may be added in order to start
the next parameterID at a 4 byte offset relative to the previous parameterID."

**Repo:** `crates/ast-layout/src/key.rs::PlCdr1HeaderPolicy`,
`crates/wire/src/writer.rs::DEFAULT`,
`crates/wire/src/reader.rs::check_declared_length`

**Tests:** `crates/wire/tests/wire.rs::the_default_parameter_length_is_the_exact_length_of_the_member`,
`crates/wire/tests/wire.rs::a_parameter_length_excludes_the_padding_that_follows_it`,
`crates/wire/tests/wire.rs::a_parameter_length_covers_the_padding_where_the_framing_says_so`

**Status:** `done` — the default (`PlCdr1HeaderPolicy::Specification`) writes the exact length of the member; the next parameter starts on a 4-octet boundary (padding behind the value). The length rounded up to four of DDSI-RTPS 2.5 §9.4.2.11 is the named setting `PlCdr1HeaderPolicy::ShortRoundedLength` (`--pl-cdr1-headers short-rounded-length`, runtime `wire.pl_cdr1_headers`). The specifications contradict each other here; the XTypes reading is the maintainer's decision.

### §7.4.1.2 The default writes and expects the rounded parameter length

**Spec:** §7.4.1.2, p. 140 — "the value of the parameter length is the exact
length of the serialized member. It does not account for any padding bytes that
may follow the serialized member."

**Repo:** `crates/ast-layout/src/key.rs::PlCdr1HeaderPolicy::pads_declared_length`,
`crates/wire/src/writer.rs::PlCdr1Framing::DEFAULT`,
`crates/wire/src/reader.rs::check_declared_length`

**Tests:** `crates/wire/tests/wire.rs::the_default_parameter_length_is_the_exact_length_of_the_member`,
`crates/wire/tests/wire.rs::a_default_reader_refuses_the_rounded_length_and_names_the_setting`,
`crates/wire-codec/src/value.rs::tests::the_default_framing_is_the_default_dialects`

**Status:** `done` — writer and reader of the default use the exact length; a stream with a rounded length is reported as `WireError::WrongFraming` naming `short-rounded-length`. The Rust constant `PlCdr1Framing::DEFAULT` and the default dialect agree (test). Measured vendors: Cyclone DDS and RTI Connext write the rounded length, Fast DDS and OpenDDS the exact one (`vendor_bytes.md`); their dialects keep that.

### §7.4.1.2 Mutable as a parameter list, one parameter per member

**Spec:** §7.4.1.2, p. 140 — "Any object of a mutable aggregated type shall be
serialized as a parameter list. Each of its members shall correspond to a single
parameter within that list."

**Repo:** `crates/ast-layout/src/xcdr.rs::member_header` (l. 2018)

**Tests:** `crates/ast-layout/tests/layout.rs::a_mutable_struct_frames_every_member_with_a_header`,
`crates/wire-conformance/tests/bytes.rs::an_xcdr1_parameter_list_is_read_by_its_ids_too`

**Status:** done

### §7.4.1.2 Optional in final/appendable: header with length 0, no sentinel

**Spec:** §7.4.1.2, p. 140 — "Any optional member of a final or appendable
structure shall be preceded by a parameter header describing that member. If the
member takes no value within that particular object, the data length indicated
by the header shall be zero. [...] the optional member shall not be followed by
list-terminating sentinel."

**Repo:** `crates/ast-layout/src/plan.rs::MemberHeader` (l. 93, variant `OptionalCdr1`)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_absent_optional_member_round_trips_as_the_absence_the_vendors_write`,
`crates/ast-layout/tests/xcdr1_gate.rs::plain_cdr1_plans_no_sentinel`

**Status:** done

### §7.4.1.2.1 Parameter ID: FLAG_IMPL_EXTENSION and FLAG_MUST_UNDERSTAND

**Spec:** §7.4.1.2.1, p. 140 — "The first bit of the bitmask [...] indicates
whether the parameter has an implementation-specific interpretation [...]
FLAG_IMPL_EXTENSION. The second bit [...] FLAG_MUST_UNDERSTAND. This bit shall
be set if and only if the must_understand property of the member being
encapsulated is set to true."

**Repo:** `crates/ast-layout/src/key.rs::PidFlagPolicy` (l. 2119);
`crates/ast-layout/src/xcdr.rs::member_must_understand` (l. 1287)

**Tests:** `crates/ast-layout/tests/layout.rs::must_understand_is_set_exactly_when_the_facet_says_so`,
`crates/wire/tests/wire.rs::a_flagged_sentinel_and_a_flagged_extended_header_are_recognised`

**Status:** done — under the default `Dialect::Specification` a key member sets
the flag as well (§7.2.2.4.4.4.8).

### §7.4.1.2.1 Reserved range 0x3F01 to 0x3FFF

**Spec:** §7.4.1.2.1, p. 140 — "this specification further reserves the largest
255 values—from 16,129 (0x3F01) to 16,383 (0x3FFF)—for use by the OMG [...] For
a parameter to be recognized as one of the well-known values in Table 34, the
FLAG_IMPL_EXTENSION bit must be set to zero."

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (l. 1357, field `reserved` l. 1486);
`crates/wire-codec/src/decode.rs::read_parameter_list` (l. 1526)

**Tests:** `crates/wire-probe/tests/decode_round_trip.rs::a_vendor_private_or_reserved_parameter_is_not_a_member_of_this_type`

**Status:** done — a parameter with `FLAG_IMPL_EXTENSION` or with an ID in the
reserved range is assigned to no member.

### §7.4.1.2.1 Table 34 — PID_EXTENDED 0x3F01, must understand

**Spec:** §7.4.1.2.1, Table 34, p. 141 — "PID_EXTENDED | 0x3F01 | Yes | Allows
the specification of large member ID and/or data length values".

**Repo:** `crates/ast-layout/src/xcdr.rs::PID_EXTENDED` (l. 49)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::the_pl_cdr1_constants_match_the_oracle`,
`crates/wire/tests/wire.rs::the_extended_parameter_form_carries_a_thirty_two_bit_id`

**Status:** done — written as `0x7F01` (with `FLAG_MUST_UNDERSTAND`) under the
default `PidFlagPolicy::SentinelMustUnderstand`; `PidFlagPolicy::Bare` writes
`0x3F01`, `PidFlagPolicy::ImplExtensionOnMembers` `0xFF01`.

### §7.4.1.2.1 Table 34 — PID_LIST_END 0x3F02, must understand

**Spec:** §7.4.1.2.1, Table 34, p. 141 — "PID_LIST_END | 0x3F02 | Yes |
Indicates the end of the parameter list data structure."

**Repo:** `crates/ast-layout/src/xcdr.rs::PID_LIST_END` (l. 53)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::a_mutable_struct_plans_the_pid_list_end_sentinel`,
`crates/ast-layout/tests/xcdr1_gate.rs::an_empty_mutable_struct_still_plans_the_sentinel`

**Status:** done — written as `0x7F02` under the default.

### §7.4.1.2.1 Table 34 — PID_LIST_END: Simple Discovery types with PID 1, accept 0x3F02 as well

**Spec:** §7.4.1.2.1, Table 34, p. 141 — "Simple Discovery types shall be
subject to a special limitation: member ID 1 shall not be used and parameter ID
1 shall terminate the parameter list [...]. Implementations shall be robust to
receiving parameter ID 0x3F02 to indicate the end of a list as well."

**Repo:** `crates/rtps/src/parameter_list.rs` (`SENTINEL`, Dekodierschleife)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::the_parameter_id_3f02_ends_a_list_as_well`,
`crates/rtps/src/parameter_list.rs::tests::pid_ignore_skipped_in_pl_cdr_decode`

**Status:** done — A parameter list ends at PID 1 and at `0x3F02` (`0x7F02` with the flags set).

### §7.4.1.2.1 Parameter list of the built-in topics: 0x3F02 is recognized as the end

**Spec:** §7.4.1.2.1, Table 34, p. 141 — "Implementations shall be robust to
receiving parameter ID 0x3F02 to indicate the end of a list as well."

**Repo:** —

**Tests:** `crates/rtps/src/parameter_list.rs::tests::the_parameter_id_3f02_ends_a_list_as_well`

**Status:** done — The decoding loop compares with `pid::SENTINEL` and with `pid::LIST_END`.

### §7.4.1.2.1 Table 34 — PID_IGNORE 0x3F03

**Spec:** §7.4.1.2.1, Table 34, p. 141 — "PID_IGNORE | 0x3F03 | No | All
consumers of this Data Representation shall ignore parameters with this ID."

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (l. 1357, field `ignore` l. 1487);
`crates/wire-codec/src/decode.rs::read_parameter_list` (l. 1526, check l. 1553);
`crates/rtps/src/parameter_list.rs` (`IGNORE`)

**Tests:** `crates/wire-probe/tests/decode_round_trip.rs::a_vendor_private_or_reserved_parameter_is_not_a_member_of_this_type`,
`crates/rtps/src/parameter_list.rs::tests::pid_ignore_skipped_in_pl_cdr_decode`,
`crates/rtps/src/parameter_list.rs::tests::pid_ignore_zero_length_is_valid`

**Status:** done — `PID_IGNORE` is skipped also with `FLAG_MUST_UNDERSTAND` set.

### §7.4.1.2.1 Table 34 — 0x3F04 to 0x3FFF reserved

**Spec:** §7.4.1.2.1, Table 34, p. 141 — "Reserved for OMG | 0x3F04-0x3FFF |
N/A".

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (l. 1357, field `reserved` l. 1486)

**Tests:** `crates/wire-probe/tests/decode_round_trip.rs::a_vendor_private_or_reserved_parameter_is_not_a_member_of_this_type`

**Status:** done — a reserved ID is assigned to no member; skipped without
`FLAG_MUST_UNDERSTAND`, with the bit the sample is discarded.

### §7.4.1.2.1 Writing by Table 34, reading robust to any FLAG_MUST_UNDERSTAND

**Spec:** §7.4.1.2.1, p. 141 — "When writing data, implementations of this
specification shall set the FLAG_MUST_UNDERSTAND bit as described in Table 34.
When reading data, implementations of this specification shall be robust to any
setting of the FLAG_MUST_UNDERSTAND bit and accept the parameter nevertheless."

**Repo:** `crates/wire/src/reader.rs::read_pid_as`,
`crates/wire/src/reader.rs::Reader::with_pl_cdr1_reading`

**Tests:** `crates/wire/tests/wire.rs::a_tolerant_reader_takes_any_flags_on_the_extended_marker`,
`crates/wire/tests/wire.rs::a_tolerant_reader_takes_any_flags_on_the_sentinel`,
`crates/wire/tests/wire.rs::a_flagged_sentinel_and_a_flagged_extended_header_are_recognised`

**Status:** `done` — writing per Table 34. Reading: `PlCdr1Reading::Tolerant` (`Reader::with_pl_cdr1_reading`, `LayoutKey::with_pl_cdr1_reading`, `zerodds_build::Config::pl_cdr1_reading`) accepts every setting of `FLAG_MUST_UNDERSTAND` and `FLAG_IMPL_EXTENSION` on `PID_EXTENDED` and `PID_LIST_END`. The default reader still reports the flags of another dialect as `WrongFraming`: maintainer decision, tolerant reading only behind a named setting. For the runtime: `wire.pl_cdr1_reading` (`ZERODDS_WIRE_PL_CDR1_READING`), process-wide; no `--` flag.

### §7.4.1.2.1 Reading: differing flags on PID_EXTENDED and PID_LIST_END refused

**Spec:** §7.4.1.2.1, p. 141 — "robust to any setting of the
FLAG_MUST_UNDERSTAND bit and accept the parameter nevertheless."

**Repo:** `crates/wire/src/reader.rs::read_pid_as`

**Tests:** `crates/wire/tests/wire.rs::a_tolerant_reader_takes_any_flags_on_the_extended_marker`,
`crates/wire/tests/wire.rs::a_tolerant_reader_takes_any_flags_on_the_sentinel`

**Status:** `done` — the test reads `0x3f02`, `0x7f02`, `0xbf02`, `0xff02` and `0x3f01`, `0x7f01`, `0xbf01`, `0xff01`: the default reader takes the form of its dialect only, the tolerant reader all of them.

### §7.4.1.2.1 Design rationale on PID_PAD

**Spec:** §7.4.1.2.1, footnote 5, p. 141 — "Design rationale (non-normative)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — "non-normative".

### §7.4.1.2.1 PID_EXTENDED: 0x7F01, eMemberHeader with four flags, llength

**Spec:** §7.4.1.2.1, p. 142 — "The value of the PID_EXTENDED with the must
understand flag set is 0x7F01 [...] The four bytes following the PID_EXTENDED
and length shall be a serialized UINT32 value 'eMemberHeader' [...] FLAG_1 &
FLAG_IMPL_EXTENSION + FLAG_2 & FLAG_MUST_UNDERSTAND + FLAG_3 & FLAG_UNSPECIFIED1
+ FLAG_4 & FLAG_UNSPECIFIED2 + memberId [...] The second four bytes [...]
llength [...] the exact length of the serialized member".

**Repo:** `crates/wire/src/writer.rs::promote_pid`;
`crates/wire/src/reader.rs::read_pid_as` (l. 1357)

**Tests:** `crates/wire/tests/wire.rs::the_extended_parameter_form_carries_a_thirty_two_bit_id`,
`crates/wire/tests/wire.rs::flag_two_of_an_extended_header_is_the_members_must_understand`,
`crates/wire/tests/wire.rs::a_promoted_header_moves_the_must_understand_bit_to_flag_two`

**Status:** done

### §7.4.1.2.1 PID_EXTENDED: slength = 8, member after 12 octets

**Spec:** §7.4.1.2.1, p. 142 — "The value of the 16-bit length associated with
the PID_EXTENDED (slength) shall be equal to eight. The serialized member shall
start immediately after the long member length (llength). That is exactly 12
bytes from the beginning of the PID_EXTENDED parameter." Figure 24.

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (l. 1357, check `length != 8` l. 1379)

**Tests:** `crates/wire/tests/wire.rs::the_extended_parameter_form_carries_a_thirty_two_bit_id`

**Status:** done

### §7.4.1.2.1 PID_EXTENDED: the flags of the 16-bit ID do not apply to the member

**Spec:** §7.4.1.2.1, p. 143 — "The setting of the FLAG_IMPL_EXTENSION and
FLAG_MUST_UNDERSTAND bits in the 16-bit parameter ID shall not be interpreted to
apply to the extended parameter as well. Instead, the first most-significant bit
of the four-bitmask [...] shall represent the value of FLAG_IMPL_EXTENSION [...]
The second [...] FLAG_MUST_UNDERSTAND [...] The remaining two bits [...] should
be set to zero."

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (l. 1357, l. 1413–1422)

**Tests:** `crates/wire/tests/wire.rs::flag_two_of_an_extended_header_is_the_members_must_understand`

**Status:** done

### §7.4.1.2.1 PID_EXTENDED: legal in mutable lists and ahead of optional members

**Spec:** §7.4.1.2.1, p. 143 — "These extended parameter headers [...] shall be
legal within the parameter list data structures used to serialize objects of
mutable aggregated types. They shall also be legal when preceding optional
members of final or appendable structures".

**Repo:** `crates/ast-layout/src/plan.rs::MemberHeader` (l. 93, field `extended` in `PidCdr1` and `OptionalCdr1`)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::the_planner_applies_the_extended_rule_to_a_real_member`,
`crates/ast-layout/tests/layout.rs::a_member_id_above_the_short_pid_range_needs_the_extended_form`

**Status:** done

### §7.4.1.2.1 PID_EXTENDED: alignment as for short parameters

**Spec:** §7.4.1.2.1, p. 143 — "The alignment rules for extended parameters
shall be the same as those for non-extended parameters, which are defined in
[RTPS] Clause 9.4.2.11."

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (l. 1357,
`align_to(4)` l. 1358)

**Tests:** `crates/wire/tests/wire.rs::a_parameter_value_counts_alignment_from_its_own_block`

**Status:** done

### §7.4.1.2.2 Member ID 0 to 0x3F00: short form

**Spec:** §7.4.1.2.2, p. 143 — "Member IDs from 0 to 16,128 (0x3F00) inclusive
shall be represented exactly in the lower 14 bits of the parameter ID."

**Repo:** `crates/ast-layout/src/xcdr.rs::PID_SHORT_MAX_ID` (l. 45);
`crates/ast-layout/src/xcdr.rs::needs_extended_pid` (l. 2110)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::the_last_id_the_short_header_holds_is_the_threshold_itself`,
`crates/ast-layout/tests/xcdr1_gate.rs::a_short_member_with_a_low_id_uses_the_standard_header`

**Status:** done

### §7.4.1.2.2 Other member IDs: extended form

**Spec:** §7.4.1.2.2, p. 144 — "All other member IDs must be expressed using the
extended parameter header format."

**Repo:** `crates/ast-layout/src/xcdr.rs::needs_extended_pid` (l. 2110)

**Tests:** `crates/ast-layout/tests/layout.rs::a_member_id_above_the_short_pid_range_needs_the_extended_form`,
`crates/ast-layout/tests/xcdr1_gate.rs::a_member_id_beyond_u16_still_produces_a_usable_plan`

**Status:** done

### §7.4.1.2.2 Short and extended form equivalent

**Spec:** §7.4.1.2.2, p. 144 — "if a parameter could be described using a short
parameter header or an extended header, the short and extended expressions of
that header shall be considered totally equivalent."

**Repo:** `crates/wire/src/reader.rs::read_pid_as`

**Tests:** `crates/wire/tests/wire.rs::a_tolerant_reader_takes_the_short_form_where_the_extended_one_is_expected`,
`crates/ast-layout/tests/xcdr1_gate.rs::the_always_extended_policy_takes_the_long_header_for_a_member_that_fits_the_short_one`,
`crates/wire/tests/wire.rs::a_short_parameter_header_becomes_extended_when_the_value_overruns_it`

**Status:** `done` — both forms are written and read; under `PlCdr1Reading::Tolerant` a reader configured for `AlwaysExtended` reads the short form as well (and a reader of the short form the extended one). By default the short form under `AlwaysExtended` stays a `WrongFraming` (maintainer decision).

### §7.4.1.2.2 Reader under AlwaysExtended refuses the short form

**Spec:** §7.4.1.2.2, p. 144 — "totally equivalent".

**Repo:** `crates/wire/src/reader.rs::read_pid_as`

**Tests:** `crates/wire/tests/wire.rs::a_tolerant_reader_takes_the_short_form_where_the_extended_one_is_expected`

**Status:** `done` — the test reads the short form under `require_extended` with the tolerant reader and checks that the default reader reports it as `WrongFraming`.

### §7.4.1.2.2 Members of user types never set FLAG_IMPL_EXTENSION

**Spec:** §7.4.1.2.2, p. 144 — "This mapping ensures that members of
user-defined data types will never set the FLAG_IMPL_EXTENSION bit."

**Repo:** `crates/ast-layout/src/key.rs::impl_extension_at` (l. 2192)

**Tests:** `crates/wire-probe/tests/opendds_mutable_framing.rs::the_key_rows_are_reproduced`

**Status:** done — only `PidFlagPolicy::ImplExtensionOnMembers` (OpenDDS, chosen
by no dialect) sets the bit.

### §7.4.1.2.3 Mutable: any order, members may be omitted

**Spec:** §7.4.1.2.3, p. 144 — "members of mutable structures may appear in any
order. Furthermore, from the receiver's point of view, any mutable structure
member's value may be omitted. [...] if the receiver's member is not optional,
it logically takes its default value. If the member is optional, it takes no
value at all."

**Repo:** `crates/wire-codec/src/decode.rs::read_parameter_list` (l. 1526);
`crates/wire-codec/src/default.rs::member` (l. 100)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_mutable_member_list_is_read_by_its_ids_and_not_by_its_order`,
`crates/wire-conformance/tests/bytes.rs::a_mutable_member_the_stream_omits_takes_its_default`,
`crates/wire-conformance/tests/bytes.rs::a_reordered_list_a_vendor_actually_wrote_reads_here_too`

**Status:** done

### §7.4.1.2.3 PL_CDR contains all non-optional members

**Spec:** §7.4.1.2.3, p. 144 — "When encoding data using Parameterized CDR, the
encoded data shall contain all non-optional members, even if a member value
matches the default value for the member."

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (l. 931)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_mutable_xcdr1_struct_round_trips_through_its_sentinel`

**Status:** done — the encoder writes every member of the plan; omitting a
member equal to its default is not provided for.

### §7.4.1.2.3 Final and appendable: no omission, no reordering

**Spec:** §7.4.1.2.3, p. 144 — "Objects of final or appendable structures are
not serialized as full parameter lists, even if some members are optional.
Therefore, the members of these types may not be omitted or reordered."

**Repo:** `crates/ast-layout/src/plan.rs::MemberHeader` (l. 93,
`writes_absence`)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_absent_optional_member_round_trips_as_the_absence_the_vendors_write`

**Status:** done

### §7.4.1.2.3 Union: discriminator first, never omitted

**Spec:** §7.4.1.2.3, p. 144 — "the value of the discriminator member must be
serialized before the value of the current non-discriminator member. The
discriminator value shall not be omitted."

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_at_inner` (l. 736, arm `TypeKind::Union`)

**Tests:** `crates/wire-probe/tests/decode_round_trip.rs::a_mutable_union_frames_its_discriminator_and_its_member`,
`crates/wire-probe/tests/vendor_bytes.rs::a_boolean_and_a_char_discriminator_are_one_octet_and_then_the_member`

**Status:** done

### §7.4.1.2.4 Nested parameter lists: IDs relative to the innermost type

**Spec:** §7.4.1.2.4, p. 144 — "parameter IDs are interpreted relative to the
innermost type definition. [...] an occurrence of PID_LIST_END indicates the
conclusion of the innermost parameter list."

**Repo:** `crates/wire-codec/src/decode.rs::read_parameter_list` (l. 1526)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::a_nested_parameter_list_numbers_and_terminates_per_level`,
`crates/wire-conformance/tests/bytes.rs::a_parameter_list_that_does_not_end_at_its_sentinel_is_refused`

**Status:** done — the test checks the octets of `Outer { @id(5) Inner }` with
`Inner { @id(5) long }`: every level has IDs of its own and a sentinel of its
own.

---

## §7.4.2 Extended CDR Representation (encoding version 2)

### §7.4.2 Three formats: PLAIN_CDR2, DELIMITED_CDR, PL_CDR2

**Spec:** §7.4.2, p. 144–145 — "This specification defines three encoding
formats used with encoding version 2: PLAIN_CDR2, DELIMITED_CDR, and PL_CDR2."

**Repo:** `crates/ast-layout/src/key.rs::Encoding` (variants `PlainCdr2`,
`DelimitedCdr`,
`PlCdr2`)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v9_appendable_plans_a_dheader`,
`crates/ast-layout/tests/wire_vectors.rs::v10_mutable_plans_the_oracle_member_id_and_length_code_sequence`

**Status:** done

### §7.4.2 PLAIN_CDR2: 8-octet types aligned to 4

**Spec:** §7.4.2, p. 145 — "PLAIN_CDR2 shall be used for all primitive, strings,
and enumerated types. It is also used for any type with extensibility kind
FINAL. The encoding is similar to PLAIN_CDR except that INT64, UINT64, FLOAT64,
and FLOAT128 are serialized into the CDR buffer at offsets that are aligned to 4
rather than 8".

**Repo:** `crates/ast-layout/src/key.rs::max_alignment` (l. 83)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_caps_alignment_at_four_and_leaves_sizes_alone`,
`crates/ast-layout/tests/vendor_gate.rs::point_is_sixteen_bytes_under_xcdr1_and_twelve_under_xcdr2`

**Status:** done

### §7.4.2 DELIMITED_CDR: DHEADER ahead of PLAIN_CDR2

**Spec:** §7.4.2, p. 145 — "DELIMITED_CDR shall be used for types with
extensibility kind APPENDABLE. It serializes a UINT32 delimiter header (DHEADER)
before serializing the object using PLAIN_CDR2. The delimiter encodes the
endianness and the length of the serialized object that follows."

**Repo:** `crates/ast-layout/src/xcdr.rs::opens_with_dheader` (l. 2378);
`crates/wire/src/writer.rs::write_dheader` (l. 2233)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_appendable_struct_writes_a_dheader_with_the_length_that_followed`,
`crates/ast-layout/tests/wire_vectors.rs::v9_appendable_plans_a_dheader`

**Status:** done — "encodes the endianness": the DHEADER is written in the
endianness of the stream; it carries no field of its own for it.

### §7.4.2 PL_CDR2: DHEADER and EMHEADER per member

**Spec:** §7.4.2, p. 145 — "PL_CDR2 shall be used for aggregated types with
extensibility kind MUTABLE. Similar to DELIMITED_CDR it also serializes a
DHEADER before serializing the object. In addition, it serializes a member
header (EMHEADER) ahead each serialized member."

**Repo:** `crates/ast-layout/src/xcdr.rs::member_header` (l. 2018);
`crates/wire/src/writer.rs::write_emheader` (l. 2261)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v10_mutable_plans_the_oracle_member_id_and_length_code_sequence`,
`crates/wire/tests/wire.rs::an_emheader_packs_its_id_code_and_flag_into_one_word`

**Status:** done

---

## §7.4.3 Extended CDR encoding virtual machine

### §7.4.3 Model of stream, state variables and operations

**Spec:** §7.4.3, p. 145 — "The encoding formats are specified using a virtual
machine that acts on an XCDR stream object. [...] The XCDR stream model consists
of: A linear byte buffer [...] A set of internal state variables [...] A set of
operations [...] A 'stream insertion' operation".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — means of description. The requirements are in
Table 35 to Table 39 and in the rules (1) to (30), listed one by one. The code
does not reproduce the machine; the plan (`zerodds-ast-layout`) and the `Writer`
(`zerodds-wire`) produce the same octets.

### §7.4.3.1 Table 35 — FINAL, version 1: PLAIN_CDR

**Spec:** §7.4.3.1, Table 35, p. 146 — "FINAL | 1 | PLAIN_CDR".

**Repo:** `crates/zerodds-build/src/lib.rs::xcdr1_encoding_for` (l. 978);
`crates/zerodds-build/src/dds_type.rs::xcdr1_encoding` (l. 685);
`crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.1 Table 35 — FINAL, version 2: PLAIN_CDR2

**Spec:** §7.4.3.1, Table 35, p. 146 — "FINAL | 2 | PLAIN_CDR2".

**Repo:** `crates/zerodds-build/src/lib.rs::encoding_for` (l. 983);
`crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.1 Table 35 — APPENDABLE, version 1: PLAIN_CDR

**Spec:** §7.4.3.1, Table 35, p. 146 — "APPENDABLE | 1 | PLAIN_CDR".

**Repo:** `crates/zerodds-build/src/dds_type.rs::xcdr1_encoding` (l. 685);
`crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/ast-layout/tests/vendor_gate.rs::appendable_adds_a_delimiter_under_xcdr2_and_nothing_under_xcdr1`

**Status:** done

### §7.4.3.1 Table 35 — APPENDABLE, version 2: DELIMITED_CDR

**Spec:** §7.4.3.1, Table 35, p. 146 — "APPENDABLE | 2 | DELIMITED_CDR".

**Repo:** `crates/zerodds-build/src/lib.rs::encoding_for` (l. 983);
`crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/ast-layout/tests/vendor_gate.rs::appendable_adds_a_delimiter_under_xcdr2_and_nothing_under_xcdr1`

**Status:** done

### §7.4.3.1 Table 35 — MUTABLE, version 1: PL_CDR

**Spec:** §7.4.3.1, Table 35, p. 146 — "MUTABLE | 1 | PL_CDR".

**Repo:** `crates/zerodds-build/src/dds_type.rs::xcdr1_encoding` (l. 685);
`crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.1 Table 35 — MUTABLE, version 2: PL_CDR2

**Spec:** §7.4.3.1, Table 35, p. 146 — "MUTABLE | 2 | PL_CDR2".

**Repo:** `crates/zerodds-build/src/lib.rs::encoding_for` (l. 983);
`crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.2.1 Table 36 — NENDIAN

**Spec:** §7.4.3.2.1, Table 36, p. 146 — "NENDIAN | Constant that represents the
native endianness used by the system."

**Repo:** `crates/wire/src/writer.rs::order4` (l. 2997)

**Tests:** `crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`

**Status:** done — the `Writer` assembles values through
`to_le_bytes`/`to_be_bytes` of the configured order; the native order of the
system does not enter.

### §7.4.3.2.1 Table 36 — cendian

**Spec:** §7.4.3.2.1, Table 36, p. 146 — "cendian | [...] the endianness that
will be used to serialize subsequent objects into the stream. It affects integer
types, floating-point types, enumerated types, and the Char16 type."

**Repo:** `crates/wire/src/writer.rs::Writer` (l. 1105, field `endianness`)

**Tests:** `crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`,
`crates/wire-probe/tests/decode_round_trip.rs::every_case_comes_back_the_value_that_went_in`

**Status:** done — the order holds for the whole stream; a change in the middle
of the stream is not provided for and required by no rule. The units of a
`wstring` follow the order of the stream under the default (`WstringUnitOrder`).

### §7.4.3.2.1 Table 36 — offset

**Spec:** §7.4.3.2.1, Table 36, p. 146 — "offset | [...] computed relative to
the beginning of the stream so that XCDR.offset counts the number of bytes
currently serialized into the stream."

**Repo:** `crates/wire/src/writer.rs::Writer` (l. 1105, field `out`);
`crates/wire/src/writer.rs::position`

**Tests:** `crates/wire/tests/wire.rs::a_long_after_a_char_sits_at_four_not_at_one`

**Status:** done

### §7.4.3.2.1 Table 36 — origin: alignment relative to the logical beginning

**Spec:** §7.4.3.2.1, Table 36, p. 146–147 — "origin | [...] used as the
'logical beginning of the stream' for alignment operations. [...] An object O of
type T shall be serialized at an offset that verifies: ((XCDR.offset -
XCDR.origin) % T.dalignment) == 0 [...] the serialization shall insert the
minimum 'padding bytes' needed".

**Repo:** `crates/wire/src/writer.rs::Writer` (l. 1105, field `origin`);
`crates/wire/src/writer.rs::align_to` (l. 1415);
`crates/wire/src/writer.rs::begin_parameter_value` (l. 2541)

**Tests:** `crates/wire/tests/wire.rs::a_parameter_value_counts_alignment_from_its_own_block`,
`crates/wire-conformance/tests/bytes.rs::a_parameter_value_is_its_own_alignment_origin_and_the_origin_is_put_back`

**Status:** done

### §7.4.3.2.1 Table 36 — eversion

**Spec:** §7.4.3.2.1, Table 36, p. 147 — "eversion | Octet state variable used
to identify the version of the encoding rules [...] {0x00} -- VERSION_NONE
{0x01} -- VERSION1 {0x02} -- VERSION2".

**Repo:** `crates/ast-layout/src/key.rs::Encoding` (l. 13)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_caps_alignment_at_four_and_leaves_sizes_alone`

**Status:** done — the version is part of `Encoding` (`PlainCdr1`/`PlCdr1`
against `PlainCdr2`/`DelimitedCdr`/`PlCdr2`). There is no value for
VERSION_NONE; a stream without a version does not occur.

### §7.4.3.2.1 Table 36 — maxalign: MALIGN = MIN(type alignment, maxalign)

**Spec:** §7.4.3.2.1, Table 36, p. 147 — "maxalign | [...] MALIGN(O) =
MIN(O.type.alignment, XCDR.maxalign) [...] XCDR.maxalign == MAXALIGN(
XCDR.eversion )".

**Repo:** `crates/wire/src/writer.rs::Writer` (l. 1105, field `max_align`);
`crates/wire/src/writer.rs::with_max_align` (l. 1341);
`crates/ast-layout/src/key.rs::max_alignment` (l. 83)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_caps_alignment_at_four_and_leaves_sizes_alone`,
`crates/wire/tests/wire.rs::a_double_aligns_to_eight`

**Status:** done

### §7.4.3.2.2 Table 37 — INIT

**Spec:** §7.4.3.2.2, Table 37, p. 147 — "INIT(V1=<nv1>, V2=<nv2>,…) |
Initializes (constructs) the XCDR stream and sets the state variables".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — operation of the descriptive machine; takes
effect through rule (1), listed below.

### §7.4.3.2.2 Table 37 — PUSH(VARIABLE=<newvalue>)

**Spec:** §7.4.3.2.2, Table 37, p. 147 — "Pushes the specified XCDR stream
variable VARIABLE into the stack and sets the current value to <newvalue>. [...]
This action is reverted by the POP() operation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — operation of the descriptive machine; takes
effect through rules (1), (24) and (25), listed below. The `Writer` saves and
restores `origin` per parameter (`begin_parameter_value`).

### §7.4.3.2.2 Table 37 — PUSH(V1=<nv1>,V2=<nv2>,…)

**Spec:** §7.4.3.2.2, Table 37, p. 147 — "A shortcut for calling PUSH() multiple
times".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — shorthand.

### §7.4.3.2.2 Table 37 — POP(VARIABLE)

**Spec:** §7.4.3.2.2, Table 37, p. 148 — "Replaces the XCDR stream variable
VARIABLE with the value for that variable that was pushed on the last PUSH()
operation".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — operation of the descriptive machine. Rules
(24) and (25) write a `PUSH(ORIGIN=0)` without a matching `POP`; the `Writer`
restores `origin` after every parameter
(`crates/wire/src/writer.rs::finish_pid`).

### §7.4.3.2.2 Table 37 — POP(V1, V2,…)

**Spec:** §7.4.3.2.2, Table 37, p. 148 — "A shortcut for calling POP() multiple
times".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — shorthand.

### §7.4.3.2.2 Table 37 — MAXALIGN: VERSION2 = 4, VERSION1 = 8, VERSION_NONE = 8

**Spec:** §7.4.3.2.2, Table 37, p. 148 — "MAXALIGN( VERSION2) = 4 MAXALIGN(
VERSION1) = 8 MAXALIGN( VERSION_NONE) = 8".

**Repo:** `crates/ast-layout/src/key.rs::max_alignment` (l. 83)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_caps_alignment_at_four_and_leaves_sizes_alone`,
`crates/ast-layout/tests/xcdr1_gate.rs::xcdr1_aligns_sixty_four_bit_primitives_to_eight`

**Status:** done

### §7.4.3.2.2 Table 37 — ALIGN(N): neededalign = MIN(N, maxalign)

**Spec:** §7.4.3.2.2, Table 37, p. 148 — "ALIGN(N) | [...] inserting 'padding
bytes' into the stream. The value of the padded bytes is left unspecified. [...]
neededalign = MIN(N, XCDR.maxalign) [...] (XCDR.offset - XCDR.origin) %
neededalign == 0".

**Repo:** `crates/wire/src/writer.rs::align_to` (l. 1415)

**Tests:** `crates/wire/tests/wire.rs::a_reader_skips_exactly_the_padding_the_writer_wrote`,
`crates/wire/tests/padding_record.rs::every_recorded_octet_is_a_zero_the_writer_wrote`

**Status:** done — the padding bytes are zero.

### §7.4.3.2.2 Table 37 — XCDR << { O : T }

**Spec:** §7.4.3.2.2, Table 37, p. 148 — "The 'append' stream operation.
Serializes [...] an object 'O' of type 'T' onto the XCDR stream starting at
offset XCDR.offset."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — notation of the rules.

### §7.4.3.2.3 Initialisation of the stream: empty buffer, endianness free to choose

**Spec:** §7.4.3.2.3, p. 148 — "The XCDR stream shall be initialized with an
empty buffer. The endianness shall be set as desired by the implementation,
although a common setting for best performance is the native system endianness
(NENDIAN)."

**Repo:** `crates/wire/src/writer.rs::with_max_align` (l. 1341);
`crates/dcps/src/runtime.rs::user_payload_encap` (l. 728, parameter `big_endian`)

**Tests:** `crates/dcps/src/runtime.rs::tests::big_endian_encap_routes_to_decode_be`,
`crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`

**Status:** done

### §7.4.3.2.3 Encoding version from the configuration of the DataWriter, 1 or 2

**Spec:** §7.4.3.2.3, p. 148 — "The encoding version (eversion) shall be set as
configured on the DataWriter. In this version of the DDS-XTypes specification it
may be set to 1 or 2."

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728, parameter `offer_first`)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done — the first element of the offered
`DataRepresentationQosPolicy` chooses the version.

### §7.4.3.2.3 The first two octets: ENC_HEADER of the outermost type

**Spec:** §7.4.3.2.3, p. 148 — "The first 2 octets in the XCDR stream shall be
the Encapsulation Header (ENC_HEADER) indicating the endianness, encoding
version, and encoding algorithm of the top-level type. See Table 39."

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header_in`,
`crates/dcps/src/runtime.rs::user_payload_encap`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_header_names_the_identifiers_of_table_39_when_asked`,
`crates/wire-probe/tests/vendor_bytes.rs::the_encapsulation_options_carry_the_padding_count`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** `done` — the first two octets are the ENC_HEADER of the outermost type with endianness, version and format; the identifier follows Table 60 (default) or Table 39 (`EncapsulationIds::Table39`, `RuntimeConfig::encapsulation_ids`).

### §7.4.3.3 Table 38 — AsFinal(T)

**Spec:** §7.4.3.3, Table 38, p. 149 — "This transformation only affects
Aggregated types. [...] AsFinal(T) is a new type which is declared the same as T
except that its extensibility kind is FINAL."

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_aggregate` (l. 1720)

**Tests:** `crates/ast-layout/tests/vendor_gate.rs::appendable_adds_a_delimiter_under_xcdr2_and_nothing_under_xcdr1`,
`crates/wire-conformance/tests/bytes.rs::an_appendable_struct_writes_a_dheader_with_the_length_that_followed`

**Status:** done — an `@appendable` structure has the DHEADER under XCDR2 and
behind it the members as a `@final` structure.

### §7.4.3.3 Table 38 — AsNested(T)

**Spec:** §7.4.3.3, Table 38, p. 149 — "This transformation treats the type as a
Nested type for serialization purposes."

**Repo:** `crates/wire-codec/src/encode.rs::encode` (l. 340)

**Tests:** `crates/wire-probe/tests/interop_corpus.rs::every_case_emits_a_keyhash_and_an_encapsulated_payload`

**Status:** done — the outermost type is written behind the header by the same
path as a nested value.

### §7.4.3.3 Table 38 — AsBytes(O)

**Spec:** §7.4.3.3, Table 38, p. 149 — "This transformation reinterprets the
primitive object as an array of bytes. The resulting bytes are ordered as they
appear in the processor memory according to the native Endianness".

**Repo:** `crates/wire/src/writer.rs::order4` (l. 2997)

**Tests:** `crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`

**Status:** done — `to_le_bytes`/`to_be_bytes` replace AsBytes and ESWAP
together; the result is the same.

### §7.4.3.3 Table 38 — ESWAP(B, <doit>) for 1, 2, 4, 8 octets

**Spec:** §7.4.3.3, Table 38, p. 149 — "Conditionally swaps the bytes [...] For
length 2: { B[1], B[0] } For length 4: { B[3], B[2], B[1], B[0] } For length 8:
{ B[7], ... B[0] }".

**Repo:** `crates/wire/src/writer.rs::order4` (l. 2997);
`crates/wire/src/writer.rs::write_f128` (l. 1864)

**Tests:** `crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`,
`crates/float128/tests/binary128.rs::every_pattern_round_trips_through_sixteen_bytes_in_both_orders`

**Status:** done — ESWAP is not defined for 16 octets, yet rule (2) applies it
to every primitive type, Float128 included (gap in the specification, listed in
the findings file). `write_f128` reverses all 16 octets (CORBA 3.3 part 2,
Figure 9.2, according to the comment in the code).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x00}: PLAIN_CDR, BIG_ENDIAN

**Spec:** §7.4.3.4, Table 39, p. 150 — "{0x00, 0x00} -- PLAIN_CDR, BIG_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header` (l. 527);
`crates/dcps/src/runtime.rs::user_encap_body` (l. 20061)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/src/runtime.rs::tests::big_endian_encap_routes_to_decode_be`

**Status:** done

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x01}: PLAIN_CDR, LITTLE_ENDIAN

**Spec:** §7.4.3.4, Table 39, p. 150 — "{0x00, 0x01} -- PLAIN_CDR,
LITTLE_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header` (l. 527);
`crates/dcps/src/runtime.rs::user_encap_body` (l. 20061)

**Tests:** `crates/dcps/src/runtime.rs::tests::strip_user_encap_xcdr1_le`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x02}: PL_CDR, BIG_ENDIAN

**Spec:** §7.4.3.4, Table 39, p. 150 — "{0x00, 0x02} -- PL_CDR, BIG_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header` (l. 527);
`crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done — `user_payload_encap` clears the lowest bit of the
little-endian identifier.

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x03}: PL_CDR, LITTLE_ENDIAN

**Spec:** §7.4.3.4, Table 39, p. 150 — "{0x00, 0x03} -- PL_CDR, LITTLE_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header` (l. 527);
`crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x10}: PLAIN_CDR2, BIG_ENDIAN

**Spec:** §7.4.3.4, Table 39, p. 150 — "{0x00, 0x10} -- PLAIN_CDR2, BIG_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::EncapsulationIds`,
`crates/wire-codec/src/value.rs::encapsulation_form`,
`crates/dcps/src/runtime.rs::RuntimeConfig::encapsulation_ids`,
`crates/dcps/src/runtime.rs::encap_in_table`,
`crates/dcps/src/runtime.rs::user_encap_body`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_header_names_the_identifiers_of_table_39_when_asked`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_default_header_is_table_60_and_both_tables_are_read`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`

**Status:** `done` — Reading: `user_encap_body` and `encapsulation_form` accept the identifier. Writing: `EncapsulationIds::Table39` (probe, `Emit::PayloadWithEncapsulation`) and `RuntimeConfig::encapsulation_ids = Table39` (runtime) write `{0x00, 0x10}` for PLAIN_CDR2 big endian. The default stays Table 60 (`{0x00, 0x06}` to `{0x00, 0x0b}`), which Cyclone DDS, RTI Connext and Fast DDS write; the tables contradict each other (findings file, contradictions). Runtime: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x11}: PLAIN_CDR2, LITTLE_ENDIAN

**Spec:** §7.4.3.4, Table 39, p. 150 — "{0x00, 0x11} -- PLAIN_CDR2,
LITTLE_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::EncapsulationIds`,
`crates/wire-codec/src/value.rs::encapsulation_form`,
`crates/dcps/src/runtime.rs::RuntimeConfig::encapsulation_ids`,
`crates/dcps/src/runtime.rs::encap_in_table`,
`crates/dcps/src/runtime.rs::user_encap_body`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_header_names_the_identifiers_of_table_39_when_asked`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_default_header_is_table_60_and_both_tables_are_read`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`

**Status:** `done` — Reading: `user_encap_body` and `encapsulation_form` accept the identifier. Writing: `EncapsulationIds::Table39` (probe, `Emit::PayloadWithEncapsulation`) and `RuntimeConfig::encapsulation_ids = Table39` (runtime) write `{0x00, 0x11}` for PLAIN_CDR2 little endian. The default stays Table 60 (`{0x00, 0x06}` to `{0x00, 0x0b}`), which Cyclone DDS, RTI Connext and Fast DDS write; the tables contradict each other (findings file, contradictions). Runtime: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x12}: PL_CDR2, BIG_ENDIAN

**Spec:** §7.4.3.4, Table 39, p. 150 — "{0x00, 0x12} -- PL_CDR2, BIG_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::EncapsulationIds`,
`crates/wire-codec/src/value.rs::encapsulation_form`,
`crates/dcps/src/runtime.rs::RuntimeConfig::encapsulation_ids`,
`crates/dcps/src/runtime.rs::encap_in_table`,
`crates/dcps/src/runtime.rs::user_encap_body`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_header_names_the_identifiers_of_table_39_when_asked`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_default_header_is_table_60_and_both_tables_are_read`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`

**Status:** `done` — Reading: `user_encap_body` and `encapsulation_form` accept the identifier. Writing: `EncapsulationIds::Table39` (probe, `Emit::PayloadWithEncapsulation`) and `RuntimeConfig::encapsulation_ids = Table39` (runtime) write `{0x00, 0x12}` for PL_CDR2 big endian. The default stays Table 60 (`{0x00, 0x06}` to `{0x00, 0x0b}`), which Cyclone DDS, RTI Connext and Fast DDS write; the tables contradict each other (findings file, contradictions). Runtime: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x13}: PL_CDR2, LITTLE_ENDIAN

**Spec:** §7.4.3.4, Table 39, p. 150 — "{0x00, 0x13} -- PL_CDR2, LITTLE_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::EncapsulationIds`,
`crates/wire-codec/src/value.rs::encapsulation_form`,
`crates/dcps/src/runtime.rs::RuntimeConfig::encapsulation_ids`,
`crates/dcps/src/runtime.rs::encap_in_table`,
`crates/dcps/src/runtime.rs::user_encap_body`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_header_names_the_identifiers_of_table_39_when_asked`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_default_header_is_table_60_and_both_tables_are_read`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`

**Status:** `done` — Reading: `user_encap_body` and `encapsulation_form` accept the identifier. Writing: `EncapsulationIds::Table39` (probe, `Emit::PayloadWithEncapsulation`) and `RuntimeConfig::encapsulation_ids = Table39` (runtime) write `{0x00, 0x13}` for PL_CDR2 little endian. The default stays Table 60 (`{0x00, 0x06}` to `{0x00, 0x0b}`), which Cyclone DDS, RTI Connext and Fast DDS write; the tables contradict each other (findings file, contradictions). Runtime: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x14}: DELIMIT_CDR, BIG_ENDIAN

**Spec:** §7.4.3.4, Table 39, p. 150 — "{0x00, 0x14} -- DELIMIT_CDR,
BIG_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::EncapsulationIds`,
`crates/wire-codec/src/value.rs::encapsulation_form`,
`crates/dcps/src/runtime.rs::RuntimeConfig::encapsulation_ids`,
`crates/dcps/src/runtime.rs::encap_in_table`,
`crates/dcps/src/runtime.rs::user_encap_body`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_header_names_the_identifiers_of_table_39_when_asked`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_default_header_is_table_60_and_both_tables_are_read`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`

**Status:** `done` — Reading: `user_encap_body` and `encapsulation_form` accept the identifier. Writing: `EncapsulationIds::Table39` (probe, `Emit::PayloadWithEncapsulation`) and `RuntimeConfig::encapsulation_ids = Table39` (runtime) write `{0x00, 0x14}` for DELIMIT_CDR big endian. The default stays Table 60 (`{0x00, 0x06}` to `{0x00, 0x0b}`), which Cyclone DDS, RTI Connext and Fast DDS write; the tables contradict each other (findings file, contradictions). Runtime: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x15}: DELIMIT_CDR, LITTLE_ENDIAN

**Spec:** §7.4.3.4, Table 39, p. 150 — "{0x00, 0x15} -- DELIMIT_CDR,
LITTLE_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::EncapsulationIds`,
`crates/wire-codec/src/value.rs::encapsulation_form`,
`crates/dcps/src/runtime.rs::RuntimeConfig::encapsulation_ids`,
`crates/dcps/src/runtime.rs::encap_in_table`,
`crates/dcps/src/runtime.rs::user_encap_body`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_header_names_the_identifiers_of_table_39_when_asked`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_default_header_is_table_60_and_both_tables_are_read`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`

**Status:** `done` — Reading: `user_encap_body` and `encapsulation_form` accept the identifier. Writing: `EncapsulationIds::Table39` (probe, `Emit::PayloadWithEncapsulation`) and `RuntimeConfig::encapsulation_ids = Table39` (runtime) write `{0x00, 0x15}` for DELIMIT_CDR little endian. The default stays Table 60 (`{0x00, 0x06}` to `{0x00, 0x0b}`), which Cyclone DDS, RTI Connext and Fast DDS write; the tables contradict each other (findings file, contradictions). Runtime: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x01, 0x00}: XML

**Spec:** §7.4.3.4, Table 39, p. 150 — "{0x01, 0x00} -- XML".

**Repo:** `crates/dcps/src/xml_data.rs::is_xml_payload`,
`crates/dcps/src/runtime.rs::is_xml_encap`,
`crates/dcps/src/runtime.rs::encap_in_table`

**Tests:** `crates/dcps/src/runtime.rs::tests::an_xml_sample_under_the_identifier_of_table_39_reaches_the_reader`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::an_xml_writer_sends_the_document_of_the_sample`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** `done` — a sample under `{0x01, 0x00}` is read as an XML document (`is_xml_payload`); under `RuntimeConfig::encapsulation_ids = Table39` an XML writer puts `{0x01, 0x00}` in front of the document of the sample and of the `serializedKey`. The default is `{0x00, 0x04}` (Table 60, DDSI-RTPS 2.5 Table 10.3). A vendor that writes `{0x01, 0x00}` is not measured.

### §7.4.3.4 Table 39 — EVERSION: 0x00 as version 1, 0x01, 0x02

**Spec:** §7.4.3.4, Table 39, p. 150 — "EVERSION is an octet used to identify
the version of the encoding rules [...] 0x00 -- unspecified version (understood
as version 1) 0x01 -- version 1 0x02 -- version 2".

**Repo:** `crates/ast-layout/src/key.rs::max_alignment` (l. 83)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_caps_alignment_at_four_and_leaves_sizes_alone`

**Status:** done — every encoding other than version 2 is treated as version 1
(`is_v1`). EVERSION appears only in the ENC_HEADER, not as an octet of its own.

### §7.4.3.4 Table 39 — DHEADER(O) = O.ssize

**Spec:** §7.4.3.4, Table 39, p. 151 — "DHEADER(O) = O.ssize Where O.ssize is
the number of bytes following the header that are required to hold the
serialized representation of O."

**Repo:** `crates/wire/src/writer.rs::finish_dheader` (l. 2241)

**Tests:** `crates/wire/tests/wire.rs::a_dheader_holds_the_length_of_what_followed_it`

**Status:** done

### §7.4.3.4 Table 39 — EMHEADER1 = (M_FLAG<<31) + (LC<<28) + M.id

**Spec:** §7.4.3.4, Table 39, p. 151 — "EMHEADER1 = (M_FLAG<<31) + (LC<<28) +
M.id".

**Repo:** `crates/wire/src/writer.rs::write_emheader` (l. 2261)

**Tests:** `crates/wire/tests/wire.rs::an_emheader_packs_its_id_code_and_flag_into_one_word`

**Status:** done

### §7.4.3.4 Table 39 — LC(M): 3-bit length code

**Spec:** §7.4.3.4, Table 39, p. 151 — "LC is a 3-bit length code used to
construct the EMHEADER1. It determines whether EMHEADER header has an additional
4 bytes (the NEXTINT)".

**Repo:** `crates/ast-layout/src/xcdr.rs::length_code` (l. 2149);
`crates/ast-layout/src/plan.rs::LengthCode` (l. 15)

**Tests:** `crates/ast-layout/tests/layout.rs::all_eight_length_codes_are_representable`

**Status:** done

### §7.4.3.4 Table 39 — NEXTINT only for LC ≥ 4

**Spec:** §7.4.3.4, Table 39, p. 151 — "NEXTINT is only present if LC(M)>=4."

**Repo:** `crates/wire/src/writer.rs::write_emheader` (l. 2261);
`crates/wire/src/reader.rs::read_emheader` (l. 1265)

**Tests:** `crates/wire/tests/wire.rs::a_length_code_below_four_carries_no_next_int`,
`crates/wire/tests/wire.rs::a_length_code_of_five_leaves_its_next_int_for_the_member`

**Status:** done

### §7.4.3.4.1 DHEADER ahead of DELIMITED_CDR and PL_CDR2, not counting itself

**Spec:** §7.4.3.4.1, p. 151 — "The DELIMITED_CDR and PL_CDR encoding formats
prepend a UInt32 delimiter header (DHEADER) ahead of the serialization of the
object content. The DHEADER encodes the serialized size of the object that
follows (not including the DHEADER itself)."

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_aggregate` (l. 1720);
`crates/wire/src/writer.rs::finish_dheader` (l. 2241)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_gives_a_dheader_to_anything_that_is_not_final`,
`crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`

**Status:** done — "PL_CDR" stands for PL_CDR2; PL_CDR (version 1) has no
DHEADER (slip in the specification, listed in the findings file).

### §7.4.3.4.1 O.ssize smaller than 2^32

**Spec:** §7.4.3.4.1, p. 151 — "In this expression, O.ssize is constrained to
being smaller than 4 Giga Bytes (2^32 Bytes)."

**Repo:** `crates/wire/src/reader.rs::read_dheader`,
`crates/wire/src/writer.rs::try_finish_dheader`,
`crates/wire/src/writer.rs::finish_dheader`,
`runtime/c/src/writer.c::zdw_finish_dheader`

**Tests:** `crates/wire/tests/wire.rs::a_delimited_sequence_cannot_run_past_its_delimiter`,
`crates/wire/tests/wire.rs::a_dheader_length_of_4_gib_or_more_is_refused`,
`crates/wire/src/writer.rs::tests::an_infallible_finish_dheader_over_4_gib_is_deferred_and_not_truncated`

**Status:** `done` — reading: a UInt32 cannot name a larger length, `read_dheader` refuses a length beyond the rest. Writing: `try_finish_dheader` returns the error, `finish_dheader` (no return value) records it (`Writer::deferred_error`, `check_deferred`, `finish_checked`) and does not write a length modulo 2^32; the `DdsType` glue of `zerodds-build` checks it before returning. The C runtime sets the status of the writer to `ZDW_RANGE`. The tests for 4 GiB run on the length as an argument, not on a buffer of that size. The other language runtimes (C++, Python, Lua, Julia, OCaml, Elixir) check the range in the patch call.

### §7.4.3.4.1 finish_dheader truncates a length from 2^32 on without an error

**Spec:** §7.4.3.4.1, p. 151 — "O.ssize is constrained to being smaller than 4
Giga Bytes".

**Repo:** `crates/wire/src/writer.rs::try_finish_dheader`,
`crates/wire/src/writer.rs::finish_dheader`

**Tests:** `crates/wire/tests/wire.rs::a_dheader_length_of_4_gib_or_more_is_refused`,
`crates/wire/src/writer.rs::tests::an_infallible_finish_dheader_over_4_gib_is_deferred_and_not_truncated`,
`crates/wire/src/writer.rs::tests::a_dheader_below_4_gib_is_patched_and_nothing_is_deferred`

**Status:** `done` — `finish_dheader` no longer writes a truncated length: the placeholder stays unpatched, the error is recorded and returned by `check_deferred` and `finish_checked`; `lang-rules` still generates `w.finish_dheader(..)`, the glue of `zerodds-build` checks afterwards.

### §7.4.3.4.1 DHEADER aligned to 4 relative to origin

**Spec:** §7.4.3.4.1, p. 151 — "The serialization of the DHEADER being a Uint32
type forces a 4-byte alignment relative to XCDR.origin, this may insert into the
stream up to 3 padding bytes prior to the DHEADER."

**Repo:** `crates/wire/src/writer.rs::write_dheader` (l. 2233);
`crates/wire/src/reader.rs::read_dheader` (l. 1213)

**Tests:** `crates/wire/tests/wire.rs::an_appending_writer_writes_what_a_fresh_one_appended_would`

**Status:** done

### §7.4.3.4.1 DHEADER in the current order of the stream

**Spec:** §7.4.3.4.1, p. 151 — "The serialization of the DHEADER uses the
endianness active in the XCDR stream at the time it is serialized
(XCDR.cendian)."

**Repo:** `crates/wire/src/writer.rs::finish_dheader` (l. 2241,
`order4`)

**Tests:** `crates/wire/tests/wire.rs::an_appending_writer_writes_what_a_fresh_one_appended_would`,
`crates/wire-probe/tests/interop_corpus.rs::every_case_produces_the_bytes_its_readme_records_in_all_four_modes`

**Status:** done

### §7.4.3.4.2 Member header of 4 or 8 octets

**Spec:** §7.4.3.4.2, p. 152 — "A member header precedes the serialization of
each member. The member header can be either 4 or 8 bytes."

**Repo:** `crates/wire/src/writer.rs::write_emheader` (l. 2261)

**Tests:** `crates/wire/tests/wire.rs::a_length_code_below_four_carries_no_next_int`,
`crates/ast-layout/tests/wire_vectors.rs::v10_mutable_plans_the_oracle_member_id_and_length_code_sequence`

**Status:** done

### §7.4.3.4.2 EMHEADER1 and NEXTINT in the order of the stream

**Spec:** §7.4.3.4.2, p. 152 — "EMHEADER1 shall be serialized using the XCDR
stream endianness current at the place the serialization occurs [...] NEXTINT.
It shall be serialized with the same endianness as EMHEADER1."

**Repo:** `crates/wire/src/writer.rs::write_emheader` (l. 2261);
`crates/wire/src/writer.rs::finish_emheader` (l. 2321)

**Tests:** `crates/wire/tests/wire.rs::shortening_a_header_keeps_its_id_and_its_flag_in_both_orders`,
`crates/wire/tests/wire.rs::an_emheader_next_int_that_cannot_be_true_is_refused`

**Status:** done

### §7.4.3.4.2 EMHEADER1 = (M_FLAG << 31) + (LC << 28) + (MemberId & 0x0fffffff)

**Spec:** §7.4.3.4.2, p. 152 — "EMHEADER1 = (M_FLAG << 31) + (LC << 28) +
(MemberId & 0x0fffffff)".

**Repo:** `crates/wire/src/writer.rs::write_emheader` (l. 2261);
`crates/wire/src/reader.rs::read_emheader` (l. 1265)

**Tests:** `crates/wire/tests/wire.rs::an_emheader_packs_its_id_code_and_flag_into_one_word`

**Status:** done — a member ID above `0x0FFFFFFF` is refused when writing
instead of masked.

### §7.4.3.4.2 M_FLAG = must understand of the member

**Spec:** §7.4.3.4.2, p. 152 — "The must understand flag (M_FLAG) shall be set
to 1 if the corresponding member must be understood by the receiver, see Clause
7.2.2.4.4.4.6. Otherwise it shall be set to zero."

**Repo:** `crates/ast-layout/src/xcdr.rs::member_must_understand`,
`crates/ast-layout/src/key.rs::MustUnderstandPolicy`

**Tests:** `crates/ast-layout/tests/layout.rs::must_understand_is_set_exactly_when_the_facet_says_so`,
`crates/wire-probe/tests/vendor_bytes.rs::the_must_understand_flag_is_the_one_each_vendor_writes_in_both_versions`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_discriminator_of_a_mutable_union_is_must_understand`

**Status:** `done` — set for `@must_understand`, by default (`Dialect::Specification`) for key members and for the discriminator of a `@mutable` union (`MustUnderstandPolicy::DeclaredKeyAndDiscriminator`); the vendor dialects write what was measured.

### §7.4.3.4.2 LC 0: 1 octet

**Spec:** §7.4.3.4.2, p. 152 — "LC = 0 = 0b000 indicates serialized member
length is 1 Byte".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (l. 1265);
`crates/wire/src/writer.rs::finish_emheader` (l. 2321)

**Tests:** `crates/ast-layout/tests/layout.rs::fixed_size_members_map_onto_lc0_through_lc3`

**Status:** done — `finish_emheader` refuses a value whose length does not match
code 0 to 3.

### §7.4.3.4.2 LC 1: 2 octets

**Spec:** §7.4.3.4.2, p. 152 — "LC = 1 = 0b001 indicates serialized member
length is 2 Bytes".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (l. 1265)

**Tests:** `crates/ast-layout/tests/layout.rs::fixed_size_members_map_onto_lc0_through_lc3`

**Status:** done

### §7.4.3.4.2 LC 2: 4 octets

**Spec:** §7.4.3.4.2, p. 152 — "LC = 2 = 0b010 indicates serialized member
length is 4 Bytes".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (l. 1265)

**Tests:** `crates/ast-layout/tests/layout.rs::fixed_size_members_map_onto_lc0_through_lc3`

**Status:** done

### §7.4.3.4.2 LC 3: 8 octets

**Spec:** §7.4.3.4.2, p. 152 — "LC = 3 = 0b011 indicates serialized member
length is 8 Bytes".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (l. 1265)

**Tests:** `crates/ast-layout/tests/layout.rs::fixed_size_members_map_onto_lc0_through_lc3`

**Status:** done

### §7.4.3.4.2 LC 4: length = NEXTINT

**Spec:** §7.4.3.4.2, p. 152 — "LC = 4 = 0b100 indicates serialized member
length is NEXTINT".

**Repo:** `crates/wire/src/writer.rs::write_emheader` (l. 2261);
`crates/wire/src/reader.rs::read_emheader` (l. 1265)

**Tests:** `crates/wire/tests/wire.rs::the_default_writer_leaves_an_lc4_header_alone`,
`crates/ast-layout/tests/layout.rs::a_union_member_frames_with_an_explicit_nextint`

**Status:** done

### §7.4.3.4.2 LC 5: length = NEXTINT, NEXTINT part of the member

**Spec:** §7.4.3.4.2, p. 152 — "LC = 5 = 0b101 indicates serialized member
length is also NEXTINT".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (l. 1265);
`crates/ast-layout/src/xcdr.rs::length_code` (l. 2149)

**Tests:** `crates/wire/tests/wire.rs::a_length_code_of_five_leaves_its_next_int_for_the_member`,
`crates/ast-layout/tests/layout.rs::a_string_member_frames_as_lc5`

**Status:** done — the reader takes `4 + NEXTINT` as the extent of the member:
the NEXTINT is the leading length of the member itself. The wording "serialized
member length is also NEXTINT" leaves open whether the four octets count (listed
in the findings file).

### §7.4.3.4.2 LC 6: length = 4 · NEXTINT

**Spec:** §7.4.3.4.2, p. 152 — "LC = 6 = 0b110 indicates serialized member
length is 4*NEXTINT".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (l. 1265);
`crates/ast-layout/src/xcdr.rs::length_code` (l. 2149)

**Tests:** `crates/ast-layout/tests/layout.rs::the_sequence_length_code_follows_the_element_width`,
`crates/wire-probe/tests/vendor_bytes.rs::an_aliased_sequence_element_takes_the_same_length_code_as_the_type_it_aliases`

**Status:** done — as for LC 5 the reader takes `4 + 4 · NEXTINT`.

### §7.4.3.4.2 LC 7: length = 8 · NEXTINT

**Spec:** §7.4.3.4.2, p. 152 — "LC = 7 = 0b111 indicates serialized member
length is 8*NEXTINT".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (l. 1265);
`crates/ast-layout/src/xcdr.rs::length_code` (l. 2149)

**Tests:** `crates/ast-layout/tests/layout.rs::the_sequence_length_code_follows_the_element_width`,
`crates/ast-layout/tests/layout.rs::all_eight_length_codes_are_representable`

**Status:** done — the reader takes `4 + 8 · NEXTINT`.

### §7.4.3.4.2 LC 5 to 7: NEXTINT becomes part of the member

**Spec:** §7.4.3.4.2, p. 152 — "EMHEADER1 with LC values 5 to 7 also affect the
serialization/deserialization virtual machine in that they cause NEXTINT to be
reused also as part of the serialized member."

**Repo:** `crates/wire/src/writer.rs::write_emheader` (l. 2261, no placeholder for LC 5 to 7);
`crates/wire/src/reader.rs::read_emheader` (l. 1265,
`peek_u32`)

**Tests:** `crates/wire/tests/wire.rs::a_length_code_of_five_leaves_its_next_int_for_the_member`,
`crates/wire-probe/tests/vendor_bytes.rs::every_length_code_matches_the_dialect_s_own_vendor`

**Status:** done — which code is chosen is decided by `LengthCodePolicy`; the
default `Compact` follows Cyclone DDS.

---

## §7.4.3.5 Encoding (serialization) rules

### §7.4.3.5 Form of the rules, first matching rule, entry point TOP_LEVEL_TYPE

**Spec:** §7.4.3.5, p. 153 — "When serializing an object each rule is evaluated
in sequence and the first one that has a matching version and criteria is
applied. [...] the entrypoint is a so-called 'Top Level' type [...] Other entry
points are possible if the intent is to simply serialize an object and not embed
it within an RTPS SerializedData."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — notation of the rules; the rules themselves
are listed one by one below.

### §7.4.3.5.1 Table 40 — Notation of the criteria

**Spec:** §7.4.3.5.1, Table 40, p. 154–157 — `O:T`, `TOP_LEVEL_TYPE`,
`PRIMITIVE_TYPE`, `STRING_TYPE`, `WSTRING_TYPE`, `ENUM_TYPE`, `BITMASK_TYPE`,
`ALIAS_TYPE`, `ARRAY_TYPE`, `FARRAY_TYPE`, `PARRAY_TYPE`, `SEQUENCE_TYPE`,
`PSEQUENCE_TYPE`, `FSEQUENCE_TYPE`, `MAP_TYPE`, `FMAP_TYPE`, `PMAP_TYPE`,
`UNION_TYPE`, `FUNION_TYPE`, `STRUCT_TYPE`, `FSTRUCT_TYPE`, `MSTRUCT_TYPE`,
`MEMBER`, `FMEMBER`, `OPT_FMEMBER`, `NOPT_FMEMBER`, `MMEMBER`, `FINAL_TYPE`,
`APPENDABLE_TYPE`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — notation. The `holder_type` of `ENUM_TYPE` and
`BITMASK_TYPE` are listed under Table 32 and Table 33, the order of
multi-dimensional arrays in the next entry. The rules use `MUNION_TYPE`, which
Table 40 does not define (listed in the findings file).

### §7.4.3.5.1 Table 40 — multi-dimensional arrays: the first index varies most slowly

**Spec:** §7.4.3.5.1, Table 40, p. 155 — "Multi-dimensional arrays are treated
for serialization purposes as a single dimensional array containing all the
elements ordered such that the index of the first dimension varies most slowly,
and the index of the last dimension varies most quickly."

**Repo:** `crates/wire-codec/src/encode.rs::write_array_dimension` (l. 884)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::a_typedef_of_an_array_matches_cyclone_and_rti`

**Status:** done

### §7.4.3.5.2 PLAIN_CDR: optional member with a ShortMemberHeader or a LongMemberHeader

**Spec:** §7.4.3.5.2, p. 158 — "PLAIN_CDR serializes optional members by
prepending either a ShortMemberHeader or a 12 byte LongMemberHeader. [...] The
associated size is set to zero if the optional member is not present [...] These
headers are serialized at a 4-byte offset relative to the current stream origin
(XCDR.origin) and adjust the alignment origin to zero for the serialization of
the member itself."

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (l. 931, arm `MemberHeader::OptionalCdr1`);
`crates/wire/src/writer.rs::begin_parameter_value` (l. 2541)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_absent_optional_member_round_trips_as_the_absence_the_vendors_write`,
`crates/ast-layout/tests/wire_vectors.rs::v11_optional_member_plans_a_header_regardless_of_presence`

**Status:** done

### §7.4.3.5.2 PLAIN_CDR2 and DELIMITED_CDR: Boolean is_present

**Spec:** §7.4.3.5.2, p. 158 — "PLAIN_CDR2 and DELIMITED_CDR serialize optional
members by first serializing a boolean (<is_present>) [...] set to 0 if the
member is not present and to 1 if it is. [...] If it is not present, the member
shall be omitted from the serialization."

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (l. 931, arm `MemberHeader::OptionalFlag`)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_optional_member_under_xcdr2_is_the_marker_byte_the_three_vendors_write`,
`crates/wire-probe/tests/decode_round_trip.rs::the_optional_marker_is_the_one_the_vendors_write_and_read`

**Status:** done

### §7.4.3.5.2 PL_CDR and PL_CDR2: absent optional member without a header

**Spec:** §7.4.3.5.2, p. 158 — "PL_CDR and PL_CDR2 serialize optional members as
it would with regular members except that if the optional member is not present,
then the corresponding member header and serialized member are omitted from the
serialized stream."

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::an_absent_optional_member_has_no_parameter_under_pl_cdr`,
`crates/wire-probe/tests/vendor_bytes.rs::an_absent_optional_member_is_not_written`,
`crates/wire-probe/src/decode.rs::tests::an_absent_optional_stays_absent_and_a_present_one_comes_back`

**Status:** `done` — PL_CDR2 and PL_CDR1 omit header and value of an absent `@optional` member and read it back as absent.

### §7.4.3.5.2 PL_CDR: no test for an absent optional member

**Spec:** §7.4.3.5.2, p. 158 — "if the optional member is not present, then the
corresponding member header and serialized member are omitted".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::an_absent_optional_member_has_no_parameter_under_pl_cdr`

**Status:** `done` — the test writes a `@mutable` structure with an absent and with a present `@optional` member under XCDR1 (`0000040007000000027f0000`, and with the parameter for `b`) and reads both back.

---

## §7.4.3.5.3 Complete Serialization Rules
### §7.4.3.5.3 Rule (1) — TOP_LEVEL_TYPE: ENC_HEADER, OPTIONS, object with ORIGIN=0

**Spec:** §7.4.3.5.3, p. 158 — "(1) XCDR << {O : TOP_LEVEL_TYPE} = XCDR << INIT(
OFFSET=0, ORIGIN=0, CENDIAN=<E>, EVERSION=<eversion> ) << { ENC_HEADER(<E>,
<eversion>, O.type) : Byte[2] } << PUSH( EVERSION = <eversion> ) << PUSH(
MAXALIGN = MAXALIGN(<eversion>) ) << PUSH( ORIGIN = 0 ) << { <OPTIONS> : Byte[2]
} << { O : AsNested(O.type) }".

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header_in`,
`crates/wire-codec/src/encode.rs::encode_with_padding`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_header_names_the_identifiers_of_table_39_when_asked`,
`crates/wire-probe/tests/vendor_bytes.rs::the_encapsulation_options_carry_the_padding_count`,
`crates/wire-probe/tests/vendor_bytes.rs::a_reader_that_is_told_the_padding_count_stops_at_the_data`

**Status:** `done` — four octets of header (ENC_HEADER, OPTIONS), alignment from the octet behind them (ORIGIN=0); ENC_HEADER per Table 60 (default) or Table 39, OPTIONS carry the number of padding octets (§7.6.3.1.2).

### §7.4.3.5.3 Rule (2) — PRIMITIVE_TYPE: ALIGN(ssize), ESWAP(AsBytes)

**Spec:** §7.4.3.5.3, p. 158 — "(2) XCDR << {O : PRIMITIVE_TYPE} = XCDR <<
ALIGN( O.ssize ) << ESWAP( AsBytes(O) )".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (l. 456);
`crates/wire/src/writer.rs::align_to` (l. 1415)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`,
`crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`

**Status:** done — Float128 aligns to 8 by Table 31, not to `O.ssize` = 16; both
rules together give MIN(16, MAXALIGN) = 8 or 4.

### §7.4.3.5.3 Rule (3) — STRING_TYPE: length with NUL, octets with NUL

**Spec:** §7.4.3.5.3, p. 159 — "(3) XCDR << {O : STRING_TYPE} = XCDR << {
O.ssize : UInt32 } // includes NUL << { O[i] : Byte }* // includes NUL".

**Repo:** `crates/wire/src/writer.rs::write_string` (l. 1881)

**Tests:** `crates/wire/tests/wire.rs::a_string_length_counts_its_terminator`,
`crates/wire-probe/tests/vendor_bytes.rs::a_string_member_matches_all_three`

**Status:** done

### §7.4.3.5.3 Rule (4) — WSTRING_TYPE: length in octets, without NUL

**Spec:** §7.4.3.5.3, p. 159 — "(4) XCDR << {O : WSTRING_TYPE} = XCDR << {
O.ssize : UInt32 } // No NUL << { O[i] : Char16 }* // No NUL".

**Repo:** `crates/wire/src/writer.rs::write_wstring` (l. 1980)

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_wstring_counts_bytes_and_writes_no_terminator`,
`crates/wire-probe/tests/spec_constructs.rs::a_wstring_is_the_same_under_both_encoding_versions`

**Status:** done

### §7.4.3.5.3 Rule (5) — ENUM_TYPE: value in the holder_type

**Spec:** §7.4.3.5.3, p. 159 — "(5) XCDR << {O : ENUM_TYPE} = XCDR << { O.value
: O.holder_type }".

**Repo:** `crates/wire-codec/src/encode.rs::write_enumeration` (l. 1550)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::an_enumeration_matches_all_three`,
`crates/wire-probe/tests/vendor_bytes.rs::a_negative_enumerator_is_the_twos_complement_rti_wrote`

**Status:** done

### §7.4.3.5.3 Rule (6) — BITMASK_TYPE: value in the holder_type

**Spec:** §7.4.3.5.3, p. 159 — "(6) XCDR << {O : BITMASK_TYPE} = XCDR << {
O.value : O.holder_type }".

**Repo:** `crates/wire-codec/src/encode.rs::write_bitmask` (l. 2040)

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_bitmask_is_the_integer_its_bit_bound_calls_for`

**Status:** done

### §7.4.3.5.3 Rule (7) — ALIAS_TYPE: as the base type

**Spec:** §7.4.3.5.3, p. 159 — "(7) XCDR << {O : ALIAS_TYPE} = XCDR << { O :
O.base_type }".

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_at_inner` (l. 736, arm `TypeKind::Alias`)

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_typedef_of_a_fixed_writes_what_the_target_writes`,
`crates/wire-probe/tests/vendor_bytes.rs::an_aliased_sequence_element_takes_the_same_length_code_as_the_type_it_aliases`

**Status:** done

### §7.4.3.5.3 Rule (8) — PARRAY_TYPE: only the elements

**Spec:** §7.4.3.5.3, p. 159 — "(8) XCDR << {O : PARRAY_TYPE} = XCDR << { O[i] :
O.element_type }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::array_is_delimited` (l. 199)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::an_array_of_primitives_matches_all_three`,
`crates/wire/tests/wire.rs::a_sequence_writes_a_count_and_an_array_does_not`

**Status:** done

### §7.4.3.5.3 Rule (9) — ARRAY_TYPE under version 2: DHEADER and elements

**Spec:** §7.4.3.5.3, p. 160 — "(9) XCDR[2] << {O : ARRAY_TYPE} = XCDR << {
DHEADER(O) : UINT32 } << { O[i] : O.element_type }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::array_is_delimited` (l. 199);
`crates/ast-layout/src/xcdr.rs::collection_is_delimited` (l. 89)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::an_array_of_structs_reproduces_each_vendor_under_its_policy`,
`crates/wire-probe/tests/spec_constructs.rs::a_bitset_in_a_collection_is_delimited_the_way_fast_dds_delimits_it`

**Status:** done — the default `CollectionDelimiterPolicy::Specification` sets
the DHEADER for every non-primitive element; an enumeration does not count as
primitive. An array of arrays is resolved down to the base type.

### §7.4.3.5.3 Rule (10) — ARRAY_TYPE under version 1: only the elements

**Spec:** §7.4.3.5.3, p. 160 — "(10) XCDR[1] << {O : ARRAY_TYPE} = XCDR << {
O[i] : O.element_type }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (l. 89)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`,
`crates/wire-probe/tests/spec_constructs.rs::a_bitset_in_a_collection_is_delimited_the_way_fast_dds_delimits_it`

**Status:** done

### §7.4.3.5.3 Arrays: APPENDABLE by (29)/(30), MUTABLE not allowed

**Spec:** §7.4.3.5.3, p. 160 — "// Arrays with extensibility APPENDABLE use
common APPENDABLE rules: (29)-(30) // Arrays with extensibility MUTABLE are not
allowed. Treated as APPENDABLE."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — comment of the rules. A collection has no
extensibility in the model; an array is no declaration to which IDL can attach
`@appendable` or `@mutable`.

### §7.4.3.5.3 Rule (11) — PSEQUENCE_TYPE: length and elements

**Spec:** §7.4.3.5.3, p. 160 — "(11) XCDR << { O : PSEQUENCE_TYPE } = XCDR << {
O.length : UInt32 } << { O[i] : O.element_type }*".

**Repo:** `crates/wire/src/writer.rs::write_sequence`

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::a_sequence_of_a_primitive_matches_all_three`,
`crates/wire/tests/wire.rs::a_sequence_writes_a_count_and_an_array_does_not`

**Status:** done

### §7.4.3.5.3 Rule (12) — SEQUENCE_TYPE under version 2: DHEADER, length, elements

**Spec:** §7.4.3.5.3, p. 160 — "(12) XCDR[2] << {O : SEQUENCE_TYPE} = XCDR << {
DHEADER(O) : UINT32 } << { O.length : UINT32 } << { O[i] : O.element_type }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (l. 89)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_sequence_of_sequences_writes_the_bytes_cyclone_and_fast_dds_write`,
`crates/wire-probe/tests/vendor_bytes.rs::a_collection_of_structs_reproduces_each_vendor_under_its_policy`

**Status:** done

### §7.4.3.5.3 Rule (13) — SEQUENCE_TYPE under version 1: length and elements

**Spec:** §7.4.3.5.3, p. 160–161 — "(13) XCDR[1] << {O : SEQUENCE_TYPE} = XCDR
<< { O.length : UInt32 } << { O[i] : O.element_type }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (l. 89)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`,
`crates/wire-probe/tests/spec_constructs.rs::a_bitset_in_a_collection_is_delimited_the_way_fast_dds_delimits_it`

**Status:** done

### §7.4.3.5.3 Rule (14) — PMAP_TYPE: length and pairs

**Spec:** §7.4.3.5.3, p. 161 — "(14) XCDR << {O : PMAP_TYPE} = XCDR << {
O.length : UInt32 } << { (O[i].key : O.key_type), (O[i].element :
O.element_type) }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_at_inner` (l. 736, arm `TypeKind::Map`);
`crates/ast-layout/src/key.rs::MapDelimiter` (l. 464)

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_map_of_primitives_is_a_length_and_the_pairs`,
`crates/wire-conformance/tests/map_bytes.rs::a_map_is_delimited_only_where_rule_14_does_not_exempt_it`

**Status:** done — under the default `MapDelimiter::EitherHalf` a non-primitive
key (`string`) puts the map under rule (15).

### §7.4.3.5.3 Rule (15) — MAP_TYPE under version 2: DHEADER and pairs

**Spec:** §7.4.3.5.3, p. 161 — "(15) XCDR[2] << { O : MAP_TYPE } = XCDR << {
DHEADER(O) : UINT32 } << { (O[i].key : O.key_type), (O[i].element :
O.element_type) }*".

**Repo:** `crates/wire-codec/src/encode.rs::write_map`,
`crates/wire-codec/src/value.rs::MapLengthPolicy`

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_map_with_a_structured_value_is_delimited_under_xcdr2`,
`crates/wire-probe/tests/spec_constructs.rs::the_two_readings_of_a_delimited_maps_length_differ_by_four_bytes`,
`crates/wire-probe/tests/spec_constructs.rs::each_reading_of_a_delimited_maps_length_is_the_bytes_of_one_implementation`

**Status:** `done` — the default is the reading of the recorded vendor rows: under XCDR2 a delimited map writes the entry count after the DHEADER (Fast DDS, `vendor-corpus.tsv`, 24 `map-*` rows; equivalence with `sequence<MapEntry>` of §7.4.1.1.4 with rule (12)). For OpenDDS, which omits the count, the corpus has no row (measured in `unspecified_constructs.md`). Rule (15) literally (DHEADER, pairs, no count) is the named setting `MapLengthPolicy::Rule15Omits` (`--map-length rule-15-omits`, `Dialect::OpenDds`). The reading chosen is in the findings file under DDSXTY14-72.

### §7.4.3.5.3 Rule (16) — MAP_TYPE under version 1: length and pairs

**Spec:** §7.4.3.5.3, p. 161 — "(16) XCDR[1] << {O : MAP_TYPE} = XCDR << {
O.length : UInt32 } << { (O[i].key : O.key_type), (O[i].element :
O.element_type) }*".

**Repo:** `crates/wire-codec/src/encode.rs::write_map` (l. 2221)

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_map_with_a_structured_value_is_delimited_under_xcdr2`,
`crates/wire-conformance/tests/map_bytes.rs::the_bytes_are_the_recorded_fast_dds_row`

**Status:** done

### §7.4.3.5.3 Rule (17) — FSTRUCT_TYPE: members in order

**Spec:** §7.4.3.5.3, p. 162 — "(17) XCDR << {O : FSTRUCT_TYPE} = XCDR << {
O.member[i] : FMEMBER }*".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (l. 931)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_final_struct_of_a_long_and_a_double_encodes_to_sixteen_bytes`,
`crates/wire-probe/tests/vendor_bytes.rs::point_matches_cyclone_rti_and_fastdds`

**Status:** done

### §7.4.3.5.3 Rule (18) — NOPT_FMEMBER: only the value

**Spec:** §7.4.3.5.3, p. 162 — "(18) XCDR << {M : NOPT_FMEMBER} = XCDR << {
M.value : M.value.type }".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (l. 931, arm `MemberHeader::NoHeader`)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::the_mixed_primitive_struct_matches_all_three`

**Status:** done

### §7.4.3.5.3 Rule (19) — OPT_FMEMBER under version 1: as MMEMBER

**Spec:** §7.4.3.5.3, p. 162 — "(19) XCDR[1] << {M : OPT_FMEMBER} = XCDR << { M
: MMEMBER }".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (l. 931, arm `MemberHeader::OptionalCdr1`)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_absent_optional_member_round_trips_as_the_absence_the_vendors_write`

**Status:** done — an absent member has a header with length 0 (§7.4.1.2).

### §7.4.3.5.3 Rule (20) — OPT_FMEMBER under version 2: is_present, then the value

**Spec:** §7.4.3.5.3, p. 162 — "(20) XCDR[2] << {M : OPT_FMEMBER} = XCDR << {
<is_present> : BOOLEAN } << IF (<is_present>) { M.value : M.value.type }".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (l. 931, arm `MemberHeader::OptionalFlag`)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_optional_member_under_xcdr2_is_the_marker_byte_the_three_vendors_write`

**Status:** done

### §7.4.3.5.3 Rule (21) — MSTRUCT_TYPE under version 2: DHEADER and MMEMBER

**Spec:** §7.4.3.5.3, p. 163 — "(21) XCDR[2] << {O : MSTRUCT_TYPE} = XCDR << {
DHEADER(O) : UInt32 } << { O.member[i] : MMEMBER }*".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (l. 931);
`crates/ast-layout/src/xcdr.rs::member_header` (l. 2018)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v10_mutable_plans_the_oracle_member_id_and_length_code_sequence`,
`crates/ast-layout/tests/wire_vectors.rs::v12_empty_mutable_plans_a_dheader_and_no_members`

**Status:** done

### §7.4.3.5.3 Rule (22) — MMEMBER under version 2: EMHEADER1, NEXTINT, value

**Spec:** §7.4.3.5.3, p. 163 — "(22) XCDR[2] << {M : MMEMBER} = XCDR << {
EMHEADER1(M) : UInt32 } << IF (LC(M)>=4) { NEXTINT(M) : UInt32 } << IF
(LC(M)>=5) XCDR.offset = XCDR.offset-4 << { M.value : M.value.type }".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (l. 931, arm `MemberHeader::EmHeader`);
`crates/wire/src/writer.rs::write_emheader` (l. 2261)

**Tests:** `crates/wire/tests/wire.rs::a_length_code_of_five_leaves_its_next_int_for_the_member`,
`crates/wire-probe/tests/vendor_bytes.rs::every_length_code_matches_the_dialect_s_own_vendor`

**Status:** done — on `M_FLAG` see §7.4.3.4.2.

### §7.4.3.5.3 Rule (23) — MSTRUCT_TYPE under version 1: parameters, then the sentinel

**Spec:** §7.4.3.5.3, p. 163 — "(23) XCDR[1] << {O : MSTRUCT_TYPE} = XCDR << {
O.member[i] : MMEMBER }* << { PID_SENTINEL : UInt16 } << { length = 0 : UInt16
}".

**Repo:** `crates/wire/src/writer.rs::write_sentinel` (l. 2664)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::a_mutable_struct_plans_the_pid_list_end_sentinel`,
`crates/wire/tests/wire.rs::the_sentinel_ends_a_parameter_list`

**Status:** done — `PID_LIST_END` (`0x3F02`) is written, by Table 34. The rule
names `PID_SENTINEL`, which has the value `0x0001` in DDSI-RTPS (contradiction
in the specification, listed in the findings file).

### §7.4.3.5.3 Rule (24) — MMEMBER under version 1, short form

**Spec:** §7.4.3.5.3, p. 163 — "// using short PL encoding when both M.id <=
2^14 and M.value.ssize <= 2^16 (24) XCDR[1] << {M : MMEMBER} = XCDR << ALIGN(4)
<< { FLAG_I + FLAG_M + M.id : UInt16 } << { M.value.ssize : UInt16 } << PUSH(
ORIGIN=0 ) << { M.value : M.value.type }".

**Repo:** `crates/ast-layout/src/xcdr.rs::needs_extended_pid` (l. 2110);
`crates/wire/src/writer.rs::write_pid` (l. 2422)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::the_last_id_the_short_header_holds_is_the_threshold_itself`,
`crates/wire/tests/wire.rs::a_value_that_fits_sixteen_bits_keeps_the_short_header`

**Status:** done — the short form holds up to ID `0x3F00` and length `0xFFFF`
(§7.4.1.2.2, width of the field). The comment of the rule names `2^14` and
`2^16`; `2^16` does not fit a UInt16, and IDs from `0x3F01` on are reserved
(contradiction in the specification, listed in the findings file). Under the
default the length is rounded up to four (open under §7.4.1.2).

### §7.4.3.5.3 Rule (25) — MMEMBER under version 1, long form

**Spec:** §7.4.3.5.3, p. 164 — "(25) XCDR[1] << {M : MMEMBER} = XCDR << ALIGN(4)
<< { FLAG_I + FLAG_M + PID_EXTENDED : UInt16 } << { slength=8 : UInt16 } << {
M.id : UInt32 } << { M.value.ssize : UInt32 } << PUSH( ORIGIN=0 ) << { M.value :
M.value.type }".

**Repo:** `crates/wire/src/writer.rs::write_pid_extended`;
`crates/wire/src/writer.rs::promote_pid` (l. 2628)

**Tests:** `crates/wire/tests/wire.rs::the_extended_parameter_form_carries_a_thirty_two_bit_id`,
`crates/wire/tests/wire.rs::a_short_parameter_header_becomes_extended_when_the_value_overruns_it`

**Status:** done — the 32-bit word carries the flags of §7.4.1.2.1 above the ID;
rule (25) writes `M.id` without them.

### §7.4.3.5.3 Rule (26) — FUNION_TYPE: discriminator, the selected member if any

**Spec:** §7.4.3.5.3, p. 164 — "(26) XCDR << {O : FUNION_TYPE} = XCDR << {
O.disc : NOPT_FMEMBER } << { O.selected_member : FMEMBER }?".

**Repo:** `crates/wire-codec/src/encode.rs::write_union` (l. 1167)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::a_union_in_a_struct_matches_all_three`,
`crates/wire-probe/src/decode.rs::tests::a_union_with_no_default_and_no_matching_label_carries_no_member`

**Status:** done

### §7.4.3.5.3 Rule (27) — MUNION_TYPE under version 2: DHEADER, discriminator, member

**Spec:** §7.4.3.5.3, p. 164 — "(27) XCDR[2] << {O : MUNION_TYPE} = XCDR << {
DHEADER(O) : UInt32 } << { O.disc : MMEMBER } << { O.selected_member : MMEMBER
}?".

**Repo:** `crates/wire-codec/src/encode.rs::write_union`,
`crates/wire-codec/src/encode.rs::write_mmember`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_discriminator_of_a_mutable_union_is_must_understand`,
`crates/wire-probe/tests/decode_round_trip.rs::a_mutable_union_frames_its_discriminator_and_its_member`,
`crates/wire-probe/tests/opendds_mutable_framing.rs::the_union_rows_are_reproduced`

**Status:** `done` — DHEADER, discriminator with member id 0 and the selected member, each with an EMHEADER; the EMHEADER of the discriminator carries `M_FLAG` by default (`0xA0000000` for a `long`), not in the dialects `Rti`, `FastDds` and `OpenDds`, as measured.

### §7.4.3.5.3 Rule (28) — MUNION_TYPE under version 1: parameters, then the sentinel

**Spec:** §7.4.3.5.3, p. 165 — "(28) XCDR[1] << {O : MUNION_TYPE} = XCDR << {
O.disc : MMEMBER } << { O.selected_member : MMEMBER }? << { PID_SENTINEL :
UInt16 } << { length = 0 : UInt16 }".

**Repo:** `crates/wire-codec/src/encode.rs::write_union`,
`crates/wire/src/writer.rs::write_sentinel`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_discriminator_of_a_mutable_union_is_must_understand`,
`crates/wire-probe/tests/decode_round_trip.rs::a_mutable_union_frames_its_discriminator_and_its_member`

**Status:** `done` — discriminator and selected member as parameters, then `PID_LIST_END` (`0x7f02`, Table 34; the rule writes `PID_SENTINEL`, see rule (23)). The parameter of the discriminator carries `FLAG_MUST_UNDERSTAND` (`0x4000`) by default. The comment before the rule refers to "(25)-(26)" instead of (24) and (25) (findings file).

### §7.4.3.5.3 Rule (29) — APPENDABLE_TYPE under version 1: AsFinal

**Spec:** §7.4.3.5.3, p. 165 — "(29) XCDR[1] << {O : APPENDABLE_TYPE} = XCDR <<
{ O : AsFinal(O.type) }".

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_aggregate` (l. 1720)

**Tests:** `crates/ast-layout/tests/vendor_gate.rs::appendable_adds_a_delimiter_under_xcdr2_and_nothing_under_xcdr1`,
`crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`

**Status:** done

### §7.4.3.5.3 Rule (30) — APPENDABLE_TYPE under version 2: DHEADER and AsFinal

**Spec:** §7.4.3.5.3, p. 165 — "(30) XCDR[2] << {O : APPENDABLE_TYPE} = XCDR <<
{ DHEADER(O) : UInt32 } << { O : AsFinal(O.type) }".

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_aggregate` (l. 1720);
`crates/wire/src/writer.rs::write_dheader` (l. 2233)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_appendable_struct_writes_a_dheader_with_the_length_that_followed`,
`crates/ast-layout/tests/wire_vectors.rs::v9_appendable_plans_a_dheader`

**Status:** done

---

## §7.4.4 XML Data Representation

### §7.4.4 One XML document per sample, structure by the XSD of the type

**Spec:** §7.4.4, p. 165 — "Each data sample shall constitute a separate XML
document. The structure of that document shall conform to the XML Schema Type
Representation for the sample's corresponding type definition."

**Repo:** `crates/dcps/src/xml_data.rs::to_xml`,
`crates/dcps/src/xml_data.rs::from_xml`,
`crates/dcps/src/xml_data.rs::to_xml_key`,
`crates/dcps/src/runtime.rs::xml_wire_body`,
`crates/dcps/src/runtime.rs::xml_sample_as_cdr`

**Tests:** `crates/dcps/src/xml_data.rs::tests::a_value_is_written_as_its_document_and_read_back`,
`crates/dcps/src/xml_data.rs::tests::an_absent_optional_member_is_not_in_the_document`,
`crates/dcps/src/xml_data.rs::tests::the_document_of_a_cdr_body_is_that_body_again`,
`crates/dcps/src/xml_data.rs::tests::the_key_document_holds_the_key_members_only`,
`crates/dcps/src/xml_data.rs::tests::a_document_that_is_none_of_the_types_is_refused`,
`crates/dcps/src/runtime.rs::tests::an_xml_sample_reaches_the_reader_as_its_cdr_body`,
`crates/dcps/src/runtime.rs::tests::an_xml_sample_that_is_no_document_of_the_type_is_refused`,
`crates/dcps/src/runtime.rs::tests::an_xml_dispose_carries_the_instance_key`,
`crates/dcps/src/runtime.rs::tests::an_xml_writer_sends_the_document_of_the_sample`,
`crates/dcps/tests/dynamic_data_writer_reader.rs::linux_e2e::dynamic_data_goes_over_xml`

**Status:** done — A sample is a document of its own: the root element carries the name of the type, the members of an aggregate are elements of their name, the elements of a sequence and an array `<item>`, an entry of a map an `<item>` with `<key>` and `<value>`, a union its `<discriminator>` and the element of the selected member, a bitmask the names of the flags set, separated by blanks; an absent optional member is left out. `xml_data::to_xml` writes the document, `from_xml` reads it against the type; a DataWriter whose first representation is XML sends it behind `{0x00, 0x04}` (`xml_wire_body`), a DataReader reads it (`xml_sample_as_cdr`). A document with another root element, an unknown or a repeated element or a value that does not fit the type is refused and reported as `reader.sample.encapsulation_refused`. `crates/xml` (`serialize_sample` with `<sample type_ref>`) is the path of DDS-XML 1.0 and not that of the wire.

### §7.4.4 Mutable without a form of its own

**Spec:** §7.4.4, p. 165 — "(Note that, unlike in the CDR Representation,
samples of mutable types are serialized no differently than samples of final or
appendable types.)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — note.

### §7.4.4 Two variants, example types

**Spec:** §7.4.4, p. 165–166 — "The XML Data Representation has two variants:
the Valid XML Data Representation and the Well-formed XML Data Representation.
[...] non-normative example type definitions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure; the variants are the two following
entries.

### §7.4.4.1 Valid: namespace declared, ddstype://www.omg.org/<module path>

**Spec:** §7.4.4.1, p. 166 — "The XML document shall declare the namespace(s)
against which it may be validated. In the event that the XSD Type Representation
of the sample's type does not specify an explicit target namespace, the modules
that scope that type shall imply the namespace for the document. This implied
namespace shall take the form ddstype://www.omg.org/<module path>".

**Repo:** `crates/xml/src/sample.rs::serialize_data_representation`,
`crates/xml/src/sample.rs::implied_namespace`,
`crates/xml/src/sample.rs::parse_data_representation`

**Tests:** `crates/xml/src/sample.rs::tests::the_valid_form_declares_the_implied_namespace`,
`crates/xml/src/sample.rs::tests::both_forms_round_trip_and_are_told_apart`,
`crates/xml/src/sample.rs::tests::another_root_or_another_namespace_is_refused`

**Status:** done — `serialize_data_representation` with `DataRepresentationForm::Valid` declares `ddstype://www.omg.org/<module path>` as the default namespace of the root element, the modules separated by `/` from the outermost to the innermost (`implied_namespace`); `parse_data_representation` refuses another namespace and reports the variant. An explicit target namespace of the XSD is not read, because `zerodds-xml` does not carry the XSD in that form.

### §7.4.4.2 Well-formed: no namespace

**Spec:** §7.4.4.2, p. 166 — "The XML document shall not declare the
namespace(s) against which it may be validated, regardless of whether a target
namespace was specified in the XSD Type Representation".

**Repo:** `crates/xml/src/sample.rs::serialize_data_representation`

**Tests:** `crates/xml/tests/types_and_samples.rs::t20_sample_roundtrip`,
`crates/xml/src/sample.rs::tests::serialize_sample_uses_item_tag_for_sequence`,
`crates/xml/src/sample.rs::tests::the_well_formed_form_has_the_type_as_root_and_no_namespace`

**Status:** done — `DataRepresentationForm::WellFormed` writes no `xmlns`, the root element carries the name of the type.

### §7.4.4.2 Non-normative note: default namespace

**Spec:** §7.4.4.2, p. 167 — "Non-normative note: Valid XML data representation
can be nearly as compact [...] by using a default namespace."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — "Non-normative note".

---

## §7.5 Language Binding

### §7.5 Two bindings: Plain Language Binding and Dynamic Language Binding

**Spec:** §7.5, p. 167–168 — "The specification defines two language bindings:
Plain Language Objects and Dynamic Data."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure; the requirements are in §7.5.1 and
§7.5.2.

### §7.5 Table 41 — Plain Language Binding

**Spec:** §7.5, Table 41, p. 168 — "Plain Language Binding | Each data type is
mapped into the most natural 'native' construct in the programming language of
choice."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — advantages and disadvantages.

### §7.5 Table 41 — Dynamic Language Binding

**Spec:** §7.5, Table 41, p. 168 — "Dynamic Language Binding | All data types
are mapped into a single Language 'Dynamic Data' construct".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — advantages and disadvantages.

---

## §7.5.1 Plain Language Binding

### §7.5.1 Steps: IDL by §7.3.1, OMG IDL language mapping, language-specific transformation

**Spec:** §7.5.1, p. 169 — "This mapping reuses the OMG-standard IDL language
mappings [C-MAP, C++-MAP, JAVA-MAP]. [...] 1. First, express the type in IDL as
specified in Clause 7.3.1. 2. Then, apply the OMG Standard IDL to Language
Mapping to the IDL in step 2. 3. Finally, apply any programming
language-specific transformations".

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_items`;
`crates/lang-rules/src/adapter/cpp.rs::cpp_items`, `crates/lang-rules/src/adapter/cpp_psm.rs`, `crates/lang-rules/src/adapter/cpp_classic.rs`;
`crates/lang-rules/src/adapter/java.rs::java_items`

**Tests:** `tools/idlc/tests/cpp_classic.rs::the_classic_types_are_cpp03_and_the_codec_writes_the_native_bytes`,
`crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`,
`crates/lang-rules/tests/java_runtime_selftest.rs`,
`crates/lang-rules/tests/java_map_class.rs::a_map_is_a_java_util_map_with_the_operations_of_table_47`

**Status:** done — `zerodds-lang-rules` generates the bindings per language by the OMG mapping: C++ by default as DDS-PSM-Cxx (`std::map`, `dds::core::optional`; `--cpp-mapping psm`), as the classic mapping of [C++-MAP] (OMG IDL to C++ 1.3, C++03; `--cpp-mapping classic`) and as the native structure of the codecs (`--cpp-mapping native`); Java with `octet` as `byte`, `fixed` as `BigDecimal`, the enumeration as a class by CORBA 3.3 part 3 §1.5.9, a union with accessor functions and a map as `java.util.Map`; C with map and sequence by [C-MAP] and §7.5.1.3.2. Log: `.agent/REPORT.md`.

### §7.5.1 C: sequence by [C-MAP]

**Spec:** §7.5.1, p. 169 — "This mapping reuses the OMG-standard IDL language
mappings [C-MAP, C++-MAP, JAVA-MAP]."

**Repo:** `crates/lang-rules/src/adapter/c.rs::sequence_decl`, `crates/lang-rules/src/emit/c.rs::make_room`, `crates/lang-rules/src/default_c.rs`, `tools/idlc/src/c_support.rs::buffer_walks`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_sequence_becomes_a_length_and_a_buffer`,
`tools/idlc/tests/c_typesupport.rs` (the harness checks the capacity and `_release` of a decoded and of a released sequence),
`crates/wire-probe/tests/three_column_gate.rs::the_three_columns_agree`

**Status:** done — `sequence<T>` is `struct { uint32_t _maximum; uint32_t _length; T *_buffer; bool _release; }`; the reader sets the capacity and `_release = false` (the arena is not the sequence's), the default and the release set and clear both, the probes and the harness fill them. Log: `.agent/REPORT.md`.

### §7.5.1 A language PSM may override the binding

**Spec:** §7.5.1, p. 169 — "Note that any of the following language bindings may
be overridden in a language-specific PSM, such as [DDS-PSM-CXX]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — permission.

## §7.5.1.1 Primitive Types

### §7.5.1.1.1 Table 42 — Int32: DDS_Int32

**Spec:** §7.5.1.1.1, Table 42, p. 169 — "Int32 | DDS_Int32 | int32_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_Int32`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — UInt32: DDS_UInt32

**Spec:** §7.5.1.1.1, Table 42, p. 169 — "UInt32 | DDS_UInt32 | uint32_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_UInt32`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — Int8: DDS_Int8

**Spec:** §7.5.1.1.1, Table 42, p. 169 — "Int8 | DDS_Int8 | int8_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_Int8`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — UInt8: DDS_UInt8

**Spec:** §7.5.1.1.1, Table 42, p. 169 — "UInt8 | DDS_UInt8 | uint8_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_UInt8`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — Int16: DDS_Int16

**Spec:** §7.5.1.1.1, Table 42, p. 169 — "Int16 | DDS_Int16 | int16_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_Int16`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — UInt16: DDS_UInt16

**Spec:** §7.5.1.1.1, Table 42, p. 170 — "UInt16 | DDS_UInt16 | uint16_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_UInt16`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — Int64: DDS_Int64

**Spec:** §7.5.1.1.1, Table 42, p. 170 — "Int64 | DDS_Int64 | int64_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_Int64`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — UInt64: DDS_UInt64

**Spec:** §7.5.1.1.1, Table 42, p. 170 — "UInt64 | DDS_UInt64 | uint64_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_UInt64`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — Float32: DDS_Float32

**Spec:** §7.5.1.1.1, Table 42, p. 170 — "Float32 | DDS_Float32 |
(unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_Float32`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — Float64: DDS_Float64

**Spec:** §7.5.1.1.1, Table 42, p. 170 — "Float64 | DDS_Float64 |
(unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_Float64`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — Float128: DDS_Float128

**Spec:** §7.5.1.1.1, Table 42, p. 170 — "Float128 | DDS_Float128 |
(unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_Float128`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — Char8: DDS_Char8

**Spec:** §7.5.1.1.1, Table 42, p. 170 — "Char8 | DDS_Char8 | (unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_Char8`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — Char16: DDS_Char16

**Spec:** §7.5.1.1.1, Table 42, p. 170 — "Char16 | DDS_Char16 | (unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_Char16`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — Boolean: DDS_Boolean

**Spec:** §7.5.1.1.1, Table 42, p. 170 — "Boolean | DDS_Boolean | _Bool".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_Boolean`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Table 42 — Byte: DDS_Byte

**Spec:** §7.5.1.1.1, Table 42, p. 170 — "Byte | DDS_Byte | (unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` declares `DDS_Byte`; the generated code may write the native or the C99 types under §7.5.1.1.1 and writes `int32_t` etc.

### §7.5.1.1.1 Degree of freedom: native or C99 types in the generated code

**Spec:** §7.5.1.1.1, p. 169 — "On platforms compliant with the C99
specification, the implementation may generate equivalent C99-compatible types."

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_primitive` (l. 872)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::every_idl_primitive_reaches_a_c_type`

**Status:** done

### §7.5.1.1.1 Boolean: only 0 and 1 defined

**Spec:** §7.5.1.1.1, p. 170 — "With respect to DDS::Boolean, only the values 0
and 1 are defined. Other values result in unspecified behavior."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement on the implementation; reading
another octet is listed under Table 31.

### §7.5.1.1.1 Char16: wchar_t or an integer type

**Spec:** §7.5.1.1.1, p. 170 — "compliant implementations may consider wchar_t
to be an equivalent C type if the platform supports it and it is of sufficient
size. Otherwise, they may map Char16 to an equivalent integer type."

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_primitive` (l. 872, arm `PrimitiveKind::WChar`)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::every_idl_primitive_reaches_a_c_type`

**Status:** done — `uint16_t`.

### §7.5.1.1.2 Table 43 — Int32: DDS::Int32

**Spec:** §7.5.1.1.2, Table 43, p. 170 — "Int32 | DDS::Int32 | [std::]int32_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::Int32`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — UInt32: DDS::UInt32

**Spec:** §7.5.1.1.2, Table 43, p. 170 — "UInt32 | DDS::UInt32 |
[std::]uint32_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::UInt32`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — Int8: DDS::Int8

**Spec:** §7.5.1.1.2, Table 43, p. 170 — "Int8 | DDS::Int8 | [std::]int8_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::Int8`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — UInt8: DDS::UInt8

**Spec:** §7.5.1.1.2, Table 43, p. 170 — "UInt8 | DDS::UInt8 | [std::]uint8_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::UInt8`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — Int16: DDS::Int16

**Spec:** §7.5.1.1.2, Table 43, p. 170 — "Int16 | DDS::Int16 | [std::]int16_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::Int16`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — UInt16: DDS::UInt16

**Spec:** §7.5.1.1.2, Table 43, p. 170 — "UInt16 | DDS::UInt16 |
[std::]uint16_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::UInt16`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — Int64: DDS::Int64

**Spec:** §7.5.1.1.2, Table 43, p. 171 — "Int64 | DDS::Int64 | [std::]int64_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::Int64`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — UInt64: DDS::UInt64

**Spec:** §7.5.1.1.2, Table 43, p. 171 — "UInt64 | DDS::UInt64 |
[std::]uint64_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::UInt64`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — Float32: DDS::Float32

**Spec:** §7.5.1.1.2, Table 43, p. 171 — "Float32 | DDS::Float32 |
(unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::Float32`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — Float64: DDS::Float64

**Spec:** §7.5.1.1.2, Table 43, p. 171 — "Float64 | DDS::Float64 |
(unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::Float64`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — Float128: DDS::Float128

**Spec:** §7.5.1.1.2, Table 43, p. 171 — "Float128 | DDS::Float128 |
(unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::Float128`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — Char8: DDS::Char8

**Spec:** §7.5.1.1.2, Table 43, p. 171 — "Char8 | DDS::Char8 | (unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::Char8`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — Char16: DDS::Char16

**Spec:** §7.5.1.1.2, Table 43, p. 171 — "Char16 | DDS::Char16 | (unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::Char16`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — Boolean: DDS::Boolean

**Spec:** §7.5.1.1.2, Table 43, p. 171 — "Boolean | DDS::Boolean | bool or
_Bool".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::Boolean`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Table 43 — Byte: DDS::Byte

**Spec:** §7.5.1.1.2, Table 43, p. 171 — "Byte | DDS::Byte | (unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` declares `DDS::Byte`; the generated code may write the native or the C99 types under §7.5.1.1.2 and writes `std::int32_t` etc.

### §7.5.1.1.2 Degree of freedom: native or C99 types in the generated C++

**Spec:** §7.5.1.1.2, p. 170 — "On platforms compliant with the C99
specification, the implementation may generate equivalent C99-compatible types."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::cpp_primitive`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::the_integers_are_spelled_with_the_c99_types_of_table_43`; `crates/lang-rules/src/adapter/cpp.rs::tests::a_long_double_is_the_runtime_type_and_never_the_compiler_s`

**Status:** done — the integer types are `std::int8_t` to `std::uint64_t`; log: `.agent/REPORT.md`.

### §7.5.1.1.2 Boolean: 0, 1, true, false

**Spec:** §7.5.1.1.2, p. 171 — "With respect to DDS::Boolean, only the values 0
and 1 are defined. Alternatively, the C++ keywords true and false may be used."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement on the implementation.

### §7.5.1.1.2 Char16 may be indistinguishable from integer types

**Spec:** §7.5.1.1.2, p. 171 — "they may map Char16 to an equivalent integer
type. This means that DDS::Char16 may not be distinguishable from integer types
for purposes of overloading."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::cpp_primitive` (l. 858, arm `PrimitiveKind::WChar`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::the_tag_keeps_apart_what_the_cpp_spelling_collapses`

**Status:** done — `std::uint16_t`, as `UInt16`.

### §7.5.1.1.2 Overloading: only Boolean, Char8 and Byte may coincide

**Spec:** §7.5.1.1.2, p. 171 — "Types DDS::Boolean, DDS::Char8, and DDS::Byte
may all map to the same underlying C++ type. [...] All other mappings for basic
types shall be distinguishable for the purposes of overloading."

**Repo:** `runtime/cpp/include/zerodds/wire.hpp::Octet`; `crates/lang-rules/src/adapter/cpp.rs::octet`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_types_of_table_43_are_distinguishable_for_overloading`; `tools/idlc/tests/cpp_inheritance.rs::a_derived_struct_inherits_and_the_octet_is_its_own_type`

**Status:** done — `DDS::Byte` and `octet` are `zerodds::wire::Octet`, a type of its own; `UInt8` is `std::uint8_t`; overloads on the two are told apart; log: `.agent/REPORT.md`.

---

## §7.5.1.2 Annotations and Built-in Annotations

### §7.5.1.2.1.1 C: enumeration as #define with the actual values

**Spec:** §7.5.1.2.1.1, p. 171 — "[C-MAP] transforms an IDL enumeration into a
series of #define directives [...] The values to which these definitions
correspond shall be the actual values of the enumerated literals [...], whether
implicitly or explicitly defined."

**Repo:** `crates/lang-rules/src/adapter/c.rs::declare` (l. 937)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::an_enumeration_becomes_an_integer_and_constants_not_a_c_enum`

**Status:** done — `#define ACTIVE ((Mode)4)` after `@value(4)`, `FAULT` after
it `5`.

### §7.5.1.2.1.2 C++: enumerator with = and the value

**Spec:** §7.5.1.2.1.2, p. 171–172 — "for any enumerated literal in IDL that
bears the Value annotation, the corresponding C++ enumerated literal definition
shall be followed by an equals sign ('=') and the value of the data member of
the annotation."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::declare` (l. 1203)

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::cpp_follows_an_enumerator_with_a_value_by_an_equals_sign_and_the_value`,
`crates/lang-rules/src/adapter/cpp.rs::tests::an_enumeration_takes_an_underlying_type_from_its_bit_bound`

**Status:** done — `enum Mode { IDLE, @value(4) ACTIVE, FAULT, @value(70000) HIGH }` gives `ACTIVE = 4`, `FAULT = 5` and `HIGH = 70000`; every enumerator carries `=` and its value.

### §7.5.1.2.1.3 Java: public static final int _<label> = <value>

**Spec:** §7.5.1.2.1.3, p. 172 — "The value of each IDL enumerated literal is
given in a Java integer constant of the following form: public static final int
_<label> = <value>; [...] that numeric value shall be set according to the
explicit or implicit value".

**Repo:** `crates/lang-rules/src/adapter/java.rs::enumeration_class_body`

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::java_has_an_int_constant_with_an_underscore_and_an_object_per_literal`,
`crates/lang-rules/src/adapter/java.rs::tests::an_explicit_enumerator_value_is_written_out`,
`crates/lang-rules/src/adapter/java.rs::tests::an_enumeration_is_a_class_of_instances_and_a_member_of_it_is_one`

**Status:** done — per literal `public static final int _<label> = <value>` (the explicit or implicit value) and an object `<label>` of the class of CORBA 3.3 part 3 §1.5.9 with `value()` and `from_int`; a member of the enumeration is an instance of that class.

### §7.5.1.2.2 Bitmask: as an enumeration <TypeName>Bits with @value(1 << position)

**Spec:** §7.5.1.2.2, p. 172 — "For each bitmask type defining flags FLAG_0
through FLAG_n, the language binding shall be as if there was an enumeration
definition like the following: @bit_bound(<bit_bound_value>) enum <TypeName>Bits
{ @value(1 << <flag_value_0>) FLAG_0, ... };"

**Repo:** `crates/lang-rules/src/adapter/c.rs::declare`,
`crates/lang-rules/src/adapter/cpp.rs::declare`,
`crates/lang-rules/src/adapter/java.rs::declare`

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::the_example_of_the_clause_myflags_binds_as_the_text_shows`,
`crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::a_type_named_like_the_flag_enumeration_is_a_collision_in_every_column`,
`crates/lang-rules/tests/c_end_to_end.rs::a_bitmask_becomes_an_integer_and_one_constant_per_bit`,
`crates/lang-rules/tests/keyword_collision.rs::the_c_column_refuses_a_type_named_like_the_flag_type_of_a_bitmask`,
`crates/lang-rules/tests/keyword_collision.rs::the_c_column_accepts_a_bitmask_and_a_type_with_another_name`

**Status:** done — C: integer type `<TypeName>Bits` with a `#define` per flag; C++: `enum class <TypeName>Bits : <holder>` with `= 1 << position` per flag; Java: type-safe class `<TypeName>Bits` with `_<label>` and an object per flag. A type named like the flag enumeration in the same scope is a collision in C, C++ and Java (`SpellingCollision`).

### §7.5.1.2.2 Bitmask: typedef to the unsigned integer type

**Spec:** §7.5.1.2.2, p. 172 — "the language binding shall be as if there was a
typedef like the following [...] typedef <unsigned_integer_equivalent>
<TypeName>;"

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_holder` (l. 897);
`crates/lang-rules/src/adapter/cpp.rs::cpp_holder` (l. 884)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_bitmask_becomes_an_integer_and_one_constant_per_bit`

**Status:** done — C `typedef uint16_t Flags;`, C++ an alias; Java has no
typedef and uses the integer type at the member.

### §7.5.1.2.2 Table 44 — Bound 1–8: uint8

**Spec:** §7.5.1.2.2, Table 44, p. 172 — "1–8 | uint8".

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_holder` (l. 897);
`crates/lang-rules/src/adapter/cpp.rs::cpp_holder` (l. 884)

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::table_44_one_to_eight_bits_is_an_8_bit_unsigned_integer`,
`crates/lang-rules/src/adapter/java.rs::tests::a_bitmask_takes_the_holder_its_bound_calls_for`

**Status:** done — `@bit_bound(8)` gives `typedef uint8_t` in C, `using … = std::uint8_t` and `enum class …Bits : std::uint8_t` in C++.

### §7.5.1.2.2 Table 44 — Bound 9–16: unsigned short

**Spec:** §7.5.1.2.2, Table 44, p. 172 — "9–16 | unsigned short".

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_holder` (l. 897);
`crates/lang-rules/src/adapter/cpp.rs::cpp_holder` (l. 884)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_bitmask_becomes_an_integer_and_one_constant_per_bit`

**Status:** done — `uint16_t`.

### §7.5.1.2.2 Table 44 — Bound 17–32: unsigned long

**Spec:** §7.5.1.2.2, Table 44, p. 172 — "17–32 | unsigned long".

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_holder`;
`crates/lang-rules/src/adapter/cpp.rs::cpp_holder`

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::table_44_seventeen_to_thirty_two_bits_is_an_unsigned_long`

**Status:** done — `@bit_bound(32)` gives `uint32_t` in C and C++ (flag 31 as `1 << 31`), `long` in Java.

### §7.5.1.2.2 Table 44 — Bound 33–64: unsigned long long

**Spec:** §7.5.1.2.2, Table 44, p. 172 — "33–64 | unsigned long long".

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_holder` (l. 897);
`crates/lang-rules/src/adapter/cpp.rs::cpp_holder` (l. 884)

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::table_44_thirty_three_to_sixty_four_bits_is_an_unsigned_long_long`,
`crates/lang-rules/src/adapter/java.rs::tests::a_bitmask_takes_the_holder_its_bound_calls_for`

**Status:** done — `@bit_bound(64)` gives `uint64_t` in C and C++ (flag 63), `long` in Java.

### §7.5.1.2.2 Example MyFlags

**Spec:** §7.5.1.2.2, p. 173 — "@bit_bound(19) bitmask MyFlags { FIRST_FLAG,
@position(14) SECOND_FLAG, THIRD_FLAG, };" with `typedef unsigned long
MyFlags;`.

**Repo:** `crates/lang-rules/src/adapter/c.rs::declare`,
`crates/lang-rules/src/adapter/cpp.rs::declare`,
`crates/lang-rules/src/adapter/java.rs::declare`

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::the_example_of_the_clause_myflags_binds_as_the_text_shows`

**Status:** done — `@bit_bound(19) bitmask MyFlags` gives `uint32_t` (`unsigned long`) in C and C++, `MyFlagsBits` with FIRST_FLAG = 1 << 0, SECOND_FLAG = 1 << 14 and THIRD_FLAG = 1 << 15, the class `MyFlagsBits` in Java.

### §7.5.1.2.3 External Members: purpose

**Spec:** §7.5.1.2.3, p. 173 — "The storage for a member of an aggregated type
may be declared to be external to the storage of the enclosing object of that
type."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — use cases.

### §7.5.1.2.3.1 C: external members as pointers

**Spec:** §7.5.1.2.3.1, p. 173 — "External members shall be represented using
pointers. [...] Other external members are mapped like non-external members
except that a member of type X shall instead be mapped as type pointer-to-X."

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_held_member_type` (l. 583)

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_non_recursive_external_member_is_a_pointer_in_c`,
`crates/lang-rules/tests/external_is_an_indirection.rs::the_c_column_declares_a_recursive_member_as_a_pointer`,
`crates/lang-rules/tests/external_is_an_indirection.rs::an_external_string_stays_the_pointer_it_is`

**Status:** done — An external member is a pointer in C (`string`, `wstring` and `fixed` stay `const char *`); `@external long a` becomes `int32_t *a`.

### §7.5.1.2.3.1 C: a non-recursive external member is a pointer

**Spec:** §7.5.1.2.3.1, p. 173 — "a member of type X shall instead be mapped as
type pointer-to-X. For example, short shall be replaced by short*."

**Repo:** —

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_non_recursive_external_member_is_a_pointer_in_c`

**Status:** done — A non-recursive external member is a pointer too (`short` becomes `short*`).

### §7.5.1.2.3.1 C: the initializer sets external pointers to NULL

**Spec:** §7.5.1.2.3.1, p. 173 — "The constructor/initializer of the enclosing
object shall set the external member pointers to NULL."

**Repo:** `crates/lang-rules/src/adapter/c_lifecycle.rs`

**Tests:** `crates/lang-rules/tests/c_external_lifecycle.rs::the_initialiser_sets_the_external_pointers_to_null`,
`crates/lang-rules/tests/c_external_lifecycle.rs::an_aggregate_with_an_external_member_gets_the_four_functions`

**Status:** done — `zd_init_<T>` sets the external pointers to NULL; for every aggregate type that holds an external member the C generator makes four functions (`zd_init_<T>`, `zd_fini_<T>`, `zd_copy_<T>`, `zd_copy_with_<T>`).

### §7.5.1.2.3.1 C: the destructor deletes referenced objects

**Spec:** §7.5.1.2.3.1, p. 174 — "The destructor of the enclosing object shall
delete the objects referenced by non-NULL external member pointers."

**Repo:** —

**Tests:** `crates/lang-rules/tests/c_external_lifecycle.rs::the_destructor_deletes_what_a_non_null_pointer_references`,
`crates/lang-rules/tests/c_external_lifecycle.rs::an_external_branch_of_a_union_is_released_by_the_live_discriminator`

**Status:** done — `zd_fini_<T>` deletes the objects that a non-NULL pointer references; for a union the live discriminator selects the branch.

### §7.5.1.2.3.1 C: the copy function copies external members deeply

**Spec:** §7.5.1.2.3.1, p. 174 — "The copy function of the enclosing object
shall do a deep copy of the external members. If the destination external member
is NULL it shall be allocated. [...] If the (recursive call to the) copy
operation of the external member fails, then the copy function of the containing
object shall fail as well."

**Repo:** —

**Tests:** `crates/lang-rules/tests/c_external_lifecycle.rs::the_copy_allocates_a_null_destination_and_fills_it`,
`crates/lang-rules/tests/c_external_lifecycle.rs::the_copy_has_the_modes_of_table_45`

**Status:** done — `zd_copy_<T>` copies external members deeply and allocates a NULL destination pointer; `zd_copy_with_<T>` has the depth and the action of Table 45. If the copy of an external member fails, the copy of the aggregate fails.

### §7.5.1.2.3.1 C: additional copy function with selectable behaviour

**Spec:** §7.5.1.2.3.1, p. 174 — "There may be an additional copy function [...]
This additional copy function shall allow the user to choose whether a shallow
or deep copy is made as well as whether any existing memory pointed by the
member is reused, released, or replaced".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — optional ("There may be"); the requirements
apply only where the function exists. ZeroDDS does not have it.

### §7.5.1.2.3.1 Table 45 — Shallow Copy, Replace

**Spec:** §7.5.1.2.3.1, Table 45, p. 174 — "Shallow Copy | Replace".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — behaviour of the optional additional copy
function, which ZeroDDS does not have.

### §7.5.1.2.3.1 Table 45 — Shallow Copy, Release

**Spec:** §7.5.1.2.3.1, Table 45, p. 174 — "Shallow Copy | Release".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — behaviour of the optional additional copy
function, which ZeroDDS does not have.

### §7.5.1.2.3.1 Table 45 — Deep Copy, Reuse

**Spec:** §7.5.1.2.3.1, Table 45, p. 175 — "Deep Copy | Reuse".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — behaviour of the optional additional copy
function, which ZeroDDS does not have.

### §7.5.1.2.3.1 Table 45 — Deep Copy, Replace

**Spec:** §7.5.1.2.3.1, Table 45, p. 175 — "Deep Copy | Replace".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — behaviour of the optional additional copy
function, which ZeroDDS does not have.

### §7.5.1.2.3.1 Table 45 — Deep Copy, Release

**Spec:** §7.5.1.2.3.1, Table 45, p. 175 — "Deep Copy | Release".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — behaviour of the optional additional copy
function, which ZeroDDS does not have.

### §7.5.1.2.3.1.1 C: an external optional member may be NULL when writing

**Spec:** §7.5.1.2.3.1.1, p. 175 — "A member that is both external and optional
shall be mapped as if it was just external. [...] If the member is only external
but not optional, then it is not allowed for the member to be NULL at the time
of a write."

**Repo:** `crates/lang-rules/src/emit/c.rs::refuse_unset_external`;
`runtime/c/src/writer.c::zdw_writer_refuse`

**Tests:** `crates/lang-rules/tests/c_optional_and_external_run.rs::an_external_member_that_is_not_optional_may_not_be_null_at_a_write`,
`crates/lang-rules/tests/external_is_an_indirection.rs::the_c_encoder_hands_the_pointer_and_not_its_address`,
`crates/lang-rules/tests/external_is_an_indirection.rs::the_c_encoder_writes_through_an_external_pointer`

**Status:** done — an optional external member is a pointer and may be NULL at a write (marker 0); an external member that is not optional and NULL sets `ZDW_UNSET_EXTERNAL` in the writer (`zdw_writer_refuse`) and nothing more is written.

### §7.5.1.2.3.2 Traditional C++: a pointer-like type with an "unset" state

**Spec:** §7.5.1.2.3.2, p. 175 — "External members shall be represented by any
type that behaves similarly to a pointer (e.g., a plain pointer or a _var type).
The chosen type must support the concept of being 'unset.'"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::declare` (l. 1203, `held_externally`);
`crates/lang-rules/src/adapter/cpp.rs::external_doc` (l. 898)

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value`,
`crates/lang-rules/tests/external_is_an_indirection.rs::an_external_union_branch_takes_the_same_indirection`

**Status:** done — every `@external` member is a `std::shared_ptr`; empty is
"unset".

### §7.5.1.2.3.2 Traditional C++: constructor, destructor, copy as in C

**Spec:** §7.5.1.2.3.2, p. 175 — "The behavior of the constructor, destructor,
and copy functions shall be the same as specified for C."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::owning_members`

**Tests:** `crates/lang-rules/tests/cpp_optional_and_external_run.rs::a_copy_of_a_class_with_a_pointer_member_is_deep_and_the_destructor_deletes`,
`crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value`

**Status:** done — a class with a pointer member has a default constructor (NULL), a copy constructor and assignment that copy deep, move operations that leave NULL behind, and a destructor that deletes.

### §7.5.1.2.3.2 Traditional C++: a copy shares the external member

**Spec:** §7.5.1.2.3.2, p. 175 — "The behavior of the [...] copy functions shall
be the same as specified for C" (§7.5.1.2.3.1: "deep copy of the external
members").

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::owning_members`;
`runtime/cpp/include/zerodds/wire.hpp::clone`

**Tests:** `crates/lang-rules/tests/cpp_optional_and_external_run.rs::a_copy_of_a_class_with_a_pointer_member_is_deep_and_the_destructor_deletes`

**Status:** done — the copy makes a new object for a `std::shared_ptr` and for a pointer (`zerodds::wire::clone`); the test checks that source and copy hold different objects.

### §7.5.1.2.3.2.1 Traditional C++: external optional member

**Spec:** §7.5.1.2.3.2.1, p. 175 — "A member that is both external and optional
shall be mapped as if it was just external. [...] it is valid for the member to
be unset when writing".

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::external_doc`;
`crates/lang-rules/src/emit/cpp.rs::refuse_unset_external`

**Tests:** `crates/lang-rules/tests/cpp_optional_and_external_run.rs::an_external_member_that_is_not_optional_may_not_be_unset_at_a_write`,
`crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value`

**Status:** done — an optional external member is the same `std::shared_ptr`, empty means absent; an empty pointer at a member that is only external throws `zerodds::wire::UnsetExternal` at a write.

### §7.5.1.2.3.3 Modern C++: template external<T> in dds::core

**Spec:** §7.5.1.2.3.3, p. 176 — "External members shall be represented as an
instantiation of a template class external<T> [...] The external<T> class shall
be generated inside of an appropriate namespace. In the case of [DDS-PSM-CXX],
this namespace is dds::core."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`;
`crates/lang-rules/src/adapter/cpp_psm.rs::attribute`

**Tests:** `crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`,
`crates/lang-rules/tests/cpp_psm_external_run.rs::a_psm_external_member_is_dds_core_external_and_an_unset_one_is_refused_at_a_write`

**Status:** done — `dds::core::external<T>` in `runtime/cpp/include/dds/core/external.hpp` with `ptr_` (a shared pointer) and `locked_`; under `CppMapping::Psm` an `@external` member is a `dds::core::external<T>`.

### §7.5.1.2.3.3 Modern C++: locked_ controls deep or shallow copy

**Spec:** §7.5.1.2.3.3, p. 176 — "The value of the locked_ attribute dictates
whether copying an external member performs a deep copy or shallow copy [...]
The locked_ attribute can only be set to true when the shared pointer is set to
a non-NULL value."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::copy_constructor`,
`runtime/cpp/tests/external_selftest.cpp::constructor_from_a_pointer`,
`runtime/cpp/tests/external_selftest.cpp::lock_and_is_locked`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — unlocked, copy and assignment share the reference, locked they copy deep; `locked_` can only be true for a non-NULL pointer (the constructor and `lock()` throw otherwise).

### §7.5.1.2.3.3.1 external<T>: Default Constructor

**Spec:** §7.5.1.2.3.3.1, p. 177 — "Create an empty external<T> object with an
empty ptr_ and locked_ initialized to false."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::default_constructor`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — empty `ptr_`, `locked_` is false.

### §7.5.1.2.3.3.2 external<T>: Constructor from a T*

**Spec:** §7.5.1.2.3.3.2, p. 177 — "Create a new external<T> object referencing
the provided managed object. The attribute locked_ is set to false and ptr_ is
initialized with p."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::constructor_from_a_pointer`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — `ptr_` manages `p`, `locked_` is false (or the parameter `locked`).

### §7.5.1.2.3.3.3 external<T>: Constructor from a shared pointer to T object

**Spec:** §7.5.1.2.3.3.3, p. 177 — "Create a new external<T> object that
references the same object managed by the specified shared pointer p."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::constructor_from_a_shared_pointer`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — `ptr_` shares the object of the given `shared_ptr`, `locked_` is false.

### §7.5.1.2.3.3.4 external<T>: Copy Constructor

**Spec:** §7.5.1.2.3.3.4, p. 177 — "If other.is_locked() is false, then the new
external<T> object shares the reference with other. [...] If other.is_locked()
is true, then a new T object is created [...] deep copy."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::copy_constructor`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — unlocked source: the same reference; locked source: a new `T`, copied deep; `locked_` of the copy is false in both cases.

### §7.5.1.2.3.3.5 external<T>: Assignment Operator

**Spec:** §7.5.1.2.3.3.5, p. 178 — "The behavior specified in Table 46 below
shall be applied when assigning an external<T> object source to another
external<T> object destination"

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_destination_locked_is_an_error`,
`runtime/cpp/tests/external_selftest.cpp::assignment_source_empty_resets_the_destination`,
`runtime/cpp/tests/external_selftest.cpp::assignment_empty_destination_locked_source_creates_and_deep_copies`,
`runtime/cpp/tests/external_selftest.cpp::assignment_existing_destination_locked_source_reuses_and_deep_copies`,
`runtime/cpp/tests/external_selftest.cpp::assignment_unlocked_source_is_a_shallow_copy`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — the five rows of Table 46 (see there); a locked destination throws `external_locked_error`.

### §7.5.1.2.3.3.6 external<T>: Destructor

**Spec:** §7.5.1.2.3.3.6, p. 178 — "If ptr_ is the last reference to the managed
object, then the managed object will be released, otherwise the reference count
will simply be decreased."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::destructor`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — the destructor of the `shared_ptr` releases the object with the last reference.

### §7.5.1.2.3.3.7 external<T>: operator* (const and non-const versions)

**Spec:** §7.5.1.2.3.3.7, p. 178 — "Get a reference to the underlying managed
object that ptr_ points at."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::accessors`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — both versions give the managed object; an empty `external` throws `external_empty_error`.

### §7.5.1.2.3.3.8 external<T>: get (const and non-const versions)

**Spec:** §7.5.1.2.3.3.8, p. 178 — "Obtains a pointer to the managed object."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::accessors`,
`runtime/cpp/tests/external_selftest.cpp::default_constructor`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — both versions give the pointer to the managed object, NULL when empty.

### §7.5.1.2.3.3.9 external<T>: get_shared_ptr

**Spec:** §7.5.1.2.3.3.9, p. 178 — "Obtains a shared pointer to the managed
object."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::constructor_from_a_shared_pointer`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — gives the `shared_ptr` of the managed object.

### §7.5.1.2.3.3.10 external<T>: operator-> (const and non-const versions)

**Spec:** §7.5.1.2.3.3.10, p. 179 — "Allows accessing members of the managed
object."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::accessors`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — both versions access the members of the object.

### §7.5.1.2.3.3.11 external<T>: operator==

**Spec:** §7.5.1.2.3.3.11, p. 179 — "Returns whether two external objects manage
the same object or are both empty."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::comparison_and_bool`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — equal when both manage the same object or both are empty.

### §7.5.1.2.3.3.12 external<T>: operator!=

**Spec:** §7.5.1.2.3.3.12, p. 179 — "Returns whether two external objects do not
manage the same object."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::comparison_and_bool`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — unequal when they do not manage the same object.

### §7.5.1.2.3.3.13 external<T>: operator bool

**Spec:** §7.5.1.2.3.3.13, p. 179 — "Checks if there is a managed object (is not
NULL) or not (is NULL)."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::comparison_and_bool`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — true when an object is managed.

### §7.5.1.2.3.3.14 external<T>: is_locked

**Spec:** §7.5.1.2.3.3.14, p. 179 — "Indicates whether this object is locked or
not."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::lock_and_is_locked`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — returns `locked_`.

### §7.5.1.2.3.3.15 external<T>: lock

**Spec:** §7.5.1.2.3.3.15, p. 179 — "Sets the locked_ attribute to true."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::lock_and_is_locked`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — sets `locked_`; with an empty `ptr_` it throws.

### §7.5.1.2.3.3.5 Table 46 — destination locked_ TRUE, ptr_ <any>; source locked_ <any>, ptr_ <any>

**Spec:** §7.5.1.2.3.3.5, Table 46, p. 178 — "TRUE | <any> | <any> | <any> |
Error. Operation cannot be called when destination.is_locked() == TRUE".

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_destination_locked_is_an_error`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — error: `external_locked_error`, the destination is unchanged.

### §7.5.1.2.3.3.5 Table 46 — destination locked_ FALSE, ptr_ <any>; source locked_ <any>, ptr_ EMPTY

**Spec:** §7.5.1.2.3.3.5, Table 46, p. 178 — "FALSE | <any> | <any> | EMPTY |
The destination is reset.".

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_source_empty_resets_the_destination`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — the destination is reset, `ptr_` is empty.

### §7.5.1.2.3.3.5 Table 46 — destination locked_ FALSE, ptr_ EMPTY; source locked_ TRUE, ptr_ Not EMPTY

**Spec:** §7.5.1.2.3.3.5, Table 46, p. 178 — "FALSE | EMPTY | TRUE | Not EMPTY |
Create new object for destination.ptr_ Perform deep copy".

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_empty_destination_locked_source_creates_and_deep_copies`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — a new object, copied deep.

### §7.5.1.2.3.3.5 Table 46 — destination locked_ FALSE, ptr_ Not EMPTY; source locked_ TRUE, ptr_ Not EMPTY

**Spec:** §7.5.1.2.3.3.5, Table 46, p. 178 — "FALSE | Not EMPTY | TRUE | Not
EMPTY | Reuse existing destination.ptr_ Perform deep copy".

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_existing_destination_locked_source_reuses_and_deep_copies`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — the existing object of the destination is reused, copied deep.

### §7.5.1.2.3.3.5 Table 46 — destination locked_ FALSE, ptr_ <any>; source locked_ FALSE, ptr_ Not EMPTY

**Spec:** §7.5.1.2.3.3.5, Table 46, p. 178 — "FALSE | <any> | FALSE | Not EMPTY
| Perform shallow copy.".

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_unlocked_source_is_a_shallow_copy`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — shallow copy: the `ptr_` of the destination is the `ptr_` of the source.

### §7.5.1.2.3.3.16 Modern C++: external optional member

**Spec:** §7.5.1.2.3.3.16, p. 179 — "A member that is both external and optional
shall be mapped as if it was just external. The difference is that it is valid
for ptr_ to be empty when writing".

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs::attribute`;
`crates/cpp/include/zerodds/psm/convert.hpp::Conv<dds::core::external<E>>`

**Tests:** `crates/lang-rules/tests/cpp_psm_external_run.rs::a_psm_external_member_is_dds_core_external_and_an_unset_one_is_refused_at_a_write`

**Status:** done — an optional external member is a `dds::core::external<T>` that may be empty at a write (marker 0); a member that is only external with an empty `ptr_` throws `UnsetExternal` at a write.

### §7.5.1.2.3.4 Java: external members as object references, primitive types boxed

**Spec:** §7.5.1.2.3.4, p. 179 — "External members shall be represented using
object references. [...] For IDL types that map to Java primitive types, those
Java primitive types shall be replaced by the corresponding object box types
from the java.lang package."

**Repo:** `crates/lang-rules/src/adapter/java.rs::declare`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_external_member_is_an_object_reference_and_a_primitive_one_is_boxed`

**Status:** done — a primitive `@external` member (and union branch) is `java.lang.Short`, `Integer`, `Boolean` and so on; a string and a sequence stay the reference they are.

### §7.5.1.2.4 Optional Members: effect on the bindings

**Spec:** §7.5.1.2.4, p. 179 — "A member of an aggregated type may be declared
to be optional [...] This concept impacts the language bindings for C, C++, and
Java in the following ways."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction.

### §7.5.1.2.4.1 C: optional member as a pointer, NULL for absent

**Spec:** §7.5.1.2.4.1, p. 180 — "Optional members shall be represented using
pointers. [...] a member of type X shall instead be mapped as type pointer-to-X.
[...] A NULL pointer shall indicate an omitted value."

**Repo:** `crates/lang-rules/src/adapter/c.rs::pointer_member`;
`crates/lang-rules/src/emit/c.rs::present`

**Tests:** `crates/lang-rules/tests/c_optional_and_external_run.rs::an_optional_member_is_written_with_its_marker_and_read_back_through_a_pointer`,
`crates/lang-rules/tests/c_optional_and_external_run.rs::an_optional_string_is_the_pointer_it_already_is`,
`crates/lang-rules/tests/c_end_to_end.rs::an_optional_member_is_a_pointer_and_null_is_the_absent_one`

**Status:** done — an optional member is `T *m`, a `string` stays `const char *`; the encoder asks `m != NULL`, the decoder allocates the value in the reader's arena and sets an absent member to NULL; the `_has_` flag is gone.

### §7.5.1.2.4.2 C++: optional member as a plain pointer

**Spec:** §7.5.1.2.4.2, p. 180 — "Optional members shall be represented using
plain pointers rather than automatic values or smart pointers. [...] A NULL
pointer shall indicate an omitted value."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::held`;
`crates/lang-rules/src/emit/cpp.rs::present`

**Tests:** `crates/lang-rules/tests/cpp_optional_and_external_run.rs::an_optional_member_is_written_with_its_marker_and_read_back_through_a_pointer`,
`crates/lang-rules/tests/cpp_optional_and_external_run.rs::an_optional_string_is_a_pointer_to_a_string`,
`crates/lang-rules/src/adapter/cpp.rs::tests::an_optional_member_is_a_plain_pointer_and_not_a_second_field`

**Status:** done — an optional member of the native C++ mapping is `T *m = nullptr`, not a `std::optional`; encoder and decoder ask `!= nullptr`, the class owns the object (see §7.5.1.2.3.2). The PSM mapping (`CppMapping::Psm`) keeps `dds::core::optional<T>` of DDS-PSM-Cxx 1.0 §8.1.4 and converts to the pointer.

### §7.5.1.2.4.3 Java: optional member as an object reference, null for absent

**Spec:** §7.5.1.2.4.3, p. 180 — "Optional members shall be represented using
object references. [...] For IDL types that map to Java primitive types, those
Java primitive types shall be replaced by the corresponding object box types.
[...] A null pointer shall indicate an omitted value."

**Repo:** `crates/lang-rules/src/adapter/java.rs::boxed` (l. 1282)

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_optional_member_is_boxed_and_a_primitive_one_too`

**Status:** done

### §7.5.1.2.4.4 Optional arrays in C and C++: pointer to array

**Spec:** §7.5.1.2.4.4, p. 180–181 — "Optional arrays having element type 'T'
shall be mapped to type pointer-to-array-of-type-T rather than to type
array-of-pointers-to-type-T. [...] int32_t (*array_member)[10];"

**Repo:** `crates/lang-rules/src/adapter/c.rs::pointer_member`;
`crates/lang-rules/src/adapter/cpp.rs::held`

**Tests:** `crates/lang-rules/tests/c_optional_and_external_run.rs::an_optional_array_is_a_pointer_to_the_array_and_not_an_array_of_pointers`,
`crates/lang-rules/tests/cpp_optional_and_external_run.rs::an_optional_array_is_a_pointer_to_the_array_and_not_an_array_of_pointers`

**Status:** done — C: `@optional long array_member[3]` is a pointer to the array type and can be assigned to `int32_t (*p)[3]`; C++: a pointer to `std::array<std::int32_t, 3>`, which has the layout of the C array.

### §7.5.1.2.5 Nested Types: no TypeSupport, DataReader, DataWriter needed

**Spec:** §7.5.1.2.5, p. 181 — "An IDL compiler need not (although it may)
generate TypeSupport, DataReader, or DataWriter classes for any nested type."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — permission.

### §7.5.1.2.6 User-Defined Annotation Types: languages without annotations may ignore them

**Spec:** §7.5.1.2.6, p. 181 — "In programming languages that lack the concept
of annotations, an implementation of this specification may choose to ignore
user-defined annotations with respect to this language binding."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — permission.

### §7.5.1.2.6.1 Java: @interface per IDL annotation type and <TypeName>Group

**Spec:** §7.5.1.2.6.1, p. 181 — "Each user-defined IDL annotation type shall be
represented by a corresponding Java annotation type. [...] public @interface
<TypeName> { ... } public @interface <TypeName>Group { <TypeName>[] value(); }"

**Repo:** `crates/lang-rules/src/adapter/java.rs::annotation_types`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_user_annotation_is_an_interface_with_its_defaults_and_a_group`

**Status:** done — per annotation declaration `public @interface <TypeName>` with its members and `public @interface <TypeName>Group { <TypeName>[] value(); }`.

### §7.5.1.2.6.1 Java: default values of the annotation members

**Spec:** §7.5.1.2.6.1, p. 181 — "If a default value is specified for a given
member, it shall be reflected in the Java definition. Otherwise, the Java
definition shall have no default value."

**Repo:** `crates/lang-rules/src/adapter/java.rs::annotation_types`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_user_annotation_is_an_interface_with_its_defaults_and_a_group`

**Status:** done — a member with a default carries `default <value>`, one without does not.

### §7.5.1.2.6.1 Java: applied annotation on the Java element, multiple through Group

**Spec:** §7.5.1.2.6.1, p. 181–182 — "For each IDL element to which a single
instance user-defined annotation is applied, the corresponding Java element
shall be annotated with the Java annotation of the same name. For each IDL
element to which multiple instances of the annotation are applied, the
corresponding Java element shall be annotated with the generated annotation
bearing the 'Group' suffix".

**Repo:** `crates/lang-rules/src/adapter/java.rs::user_annotations`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_applied_annotation_sits_on_the_java_element_and_several_go_through_the_group`,
`crates/lang-rules/src/adapter/java.rs::tests::an_annotation_on_a_module_and_on_a_constant_reaches_the_java_element`

**Status:** done — an applied annotation sits on the class, field, enumerator constant, bit constant, module class or constant; several instances of one annotation on one element go through `<TypeName>Group({…})`.

---

## §7.5.1.3 Map Types

### §7.5.1.3 Key types UInt32, UInt64, String<Char8> mandatory

**Spec:** §7.5.1.3, p. 182 — "Implementations are only required to support keys
of types UInt32, UInt64, and String<Char8>. Implementations may choose to
support other key types".

**Repo:** `crates/wire-codec/src/encode.rs::write_map` (l. 2221);
`crates/lang-rules/src/adapter/c.rs::c_map_entry_name` (l. 818)

**Tests:** `crates/wire-conformance/tests/map_bytes.rs::the_bytes_are_the_recorded_fast_dds_row`,
`crates/wire-probe/tests/spec_constructs.rs::a_map_of_primitives_is_a_length_and_the_pairs`

**Status:** done — keys of every type the model admits, `string` and the integer
types among them. The tests use `string` and `long`; a test with `unsigned long`
or `unsigned long long` as key is not cited (corpus case `map-key-ulonglong`
exists).

### §7.5.1.3 Anonymous map with an unsupported key: error

**Spec:** §7.5.1.3, p. 182 — "If a Type Representation compiler encounters an
anonymous map with key type that it does not support, it shall fail with an
error."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — conditional: the generator supports every key
type of the model, an unsupported key does not occur. Under
`MapEntryOrder::AscendingKey` the encoder refuses a key without an order
(§7.2.2.4.3).

### §7.5.1.3.2.2 Table 47 — map operation new

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "new | map<KeyType, ElementType>";
§7.5.1.3.2.2, p. 184 — "Allocate a new map. If this operation fails in an
implementation-specific way, this operation shall return NULL."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_new` allocates an empty map, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.3 Table 47 — map operation delete

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "delete | void"; §7.5.1.3.2.3, p.
184 — "Delete the map and all of its contents."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_delete` frees the map, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.4 Table 47 — map operation initialize

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "initialize | void"; §7.5.1.3.2.4,
p. 184 — "Initialize the map. The initial size and capacity of the map shall be
0."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_initialize` sets size and capacity to 0, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.5 Table 47 — map operation finalize

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "finalize | void"; §7.5.1.3.2.5, p.
184 — "Finalize the map. The entries in the map will be deleted, and both the
size and maximum size set to 0."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_finalize` frees the entries, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.6 Table 47 — map operation copy

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "copy | ReturnCode_t";
§7.5.1.3.2.6, p. 184 — "Overwrite the contents of this (destination) map with
the contents of another (source) map. [...] If autogrow is FALSE, the operation
shall fail and return DDS_RETCODE_PRECONDITION_NOT_MET."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_copy` copies, with and without autogrow, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.7 Table 47 — map operation get_size

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "get_size | unsigned int";
§7.5.1.3.2.7, p. 184 — "Get the current size of the map."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_get_size`, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.8 Table 47 — map operation get_max_size

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "get_max_size | unsigned int";
§7.5.1.3.2.8, p. 185 — "Get the current maximum size of the map."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_get_max_size`, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.9 Table 47 — map operation set_max_size

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "set_max_size | ReturnCode_t";
§7.5.1.3.2.9, p. 185 — "Set the maximum size of the map. [...] If the new
max_size is less than the current size of the map, the operation shall fail and
return DDS_RETCODE_BAD_PARAMETER."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_set_max_size` (not below the size, not above the bound), generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.10 Table 47 — map operation clear

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "clear | void"; §7.5.1.3.2.10, p.
185 — "Clear all of the entries from the map. The size of the map is set to 0
and the maximum size does not change."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_clear`, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.11 Table 47 — map operation insert

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "insert | ReturnCode_t";
§7.5.1.3.2.11, p. 185 — "Insert a new entry into the map [...] If the key
already exists in the map, the operation shall fail and return
DDS_RETCODE_BAD_PARAMETER."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_insert` (existing key: DDS_RETCODE_BAD_PARAMETER, full: DDS_RETCODE_PRECONDITION_NOT_MET), generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.12 Table 47 — map operation insert_or_assign

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "insert_or_assign | ReturnCode_t";
§7.5.1.3.2.12, p. 185–186 — "If the key already exists in the map, then the
corresponding element shall be replaced."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_insert_or_assign`, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.13 Table 47 — map operation erase

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "erase | ReturnCode_t";
§7.5.1.3.2.13, p. 186 — "Remove the entry with the given key from the map. If
successful, the size of the map shall be decreased by 1."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_erase`, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.14 Table 47 — map operation get_first

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "get_first | ReturnCode_t";
§7.5.1.3.2.14, p. 186 — "Retrieves a MapEntry referencing the first entry in the
map. The returned MapEntry may be a sentinel if the map is empty."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_get_first`, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.15 Table 47 — map operation get_next

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "get_next | ReturnCode_t";
§7.5.1.3.2.15, p. 186 — "Advance the MapEntry to the next entry in the Map."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_get_next`, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.16 Table 47 — map operation find_element

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "find_element | ElementType";
§7.5.1.3.2.16, p. 186 — "Retrieve the element whose key matches the specified
one from the map. If the key exists, then return the element corresponding to
the key, otherwise return NULL."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_find_element`, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.17 Table 47 — map operation find_entry

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "find_entry | MapEntry";
§7.5.1.3.2.17, p. 186 — "Retrieve the MapEntry whose key matches the specified
one from the map."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_find_entry`, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2.18 Table 47 — map operation get_pair

**Spec:** §7.5.1.3.1, Table 47, p. 182–183 — "get_pair | Boolean";
§7.5.1.3.2.18, p. 186–187 — "Retrieve the key and element associated with the
MapEntry. If the MapEntry was a sentinel the operation will return FALSE"

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_get_pair`, generated for every map beside its type; log: `.agent/REPORT.md`.

### §7.5.1.3.2 C: map as a collection of structures of key and element

**Spec:** §7.5.1.3.2, p. 183 — "Map types shall be represented as a collection
of structures that contain a member of the key type followed by a member of the
element type. A set of methods which create, delete and manipulate objects of
the map type shall also be generated."

**Repo:** `crates/lang-rules/src/adapter/c_map.rs::map_decl`; `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — `struct { _maximum; _length; Entry *_buffer; _release }` of `struct { K key; V value; }`, with the methods of Table 47; log: `.agent/REPORT.md`.

### §7.5.1.3.2.1 C: name [key_type][fully_qualified_element_type]Map

**Spec:** §7.5.1.3.2.1, p. 183 — "For maps whose key type is a Primitive Type
the name of the map type shall be constructed by combining the key type name
with the element type name. [...] [key_type][fully_qualified_element_type]Map
[...] StringFooMap UInt32FooMap UInt64FooMap".

**Repo:** `crates/lang-rules/src/adapter/c_map.rs::map_type_name`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_name_of_a_map_is_the_key_and_the_element`

**Status:** done — `UInt32m_FooMap`, `StringInt32Map`, `UInt64m_ColorMap`; with a bound `Int32StringMap_b3`; log: `.agent/REPORT.md`.

### §7.5.1.3.2.1 C: maps with UInt32, UInt64 and String keys beside every type T

**Spec:** §7.5.1.3.2.1, p. 183 — "For any type T, the declaration and
implementation of the map types having element type T and key types UInt32,
UInt64, and String shall be generated alongside the implementation code for
element type T."

**Repo:** `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_name_of_a_map_is_the_key_and_the_element`

**Status:** done — for every declared type T the header holds `UInt32TMap`, `UInt64TMap` and `StringTMap`; log: `.agent/REPORT.md`.

### §7.5.1.3.2.19 Example UInt32MyModule_MyElementTypeMap

**Spec:** §7.5.1.3.2.19, p. 187–188 — "Example (Non-Normative)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example.

### §7.5.1.3.3 Traditional C++: map class in the namespace of the element type

**Spec:** §7.5.1.3.3.1, p. 189 — "The map class shall be named the same as the C
structure [...] except that it is placed in the same namespace as the element
type declaration."

**Repo:** `runtime/cpp/include/zerodds/map.hpp`; `crates/lang-rules/src/adapter/cpp.rs::maps_alongside`

**Tests:** `tools/idlc/tests/cpp_map_class.rs::a_map_is_a_class_with_the_operations_of_table_47`

**Status:** done — `zerodds::Map<K, V, Bound>` with the operations of Table 47; `UInt32FooMap`, `UInt64FooMap`, `StringFooMap` stand in the namespace of `Foo`; log: `.agent/REPORT.md`.

### §7.5.1.3.3 Traditional C++: insert with replace

**Spec:** §7.5.1.3.3.1, p. 189 — "The C++ operation insert behaves as the C
insert() if the replace() parameter is false and it behaves as the C
insert_or_assign() if replace parameter is true."

**Repo:** `runtime/cpp/include/zerodds/map.hpp::Map::insert`

**Tests:** `tools/idlc/tests/cpp_map_class.rs::a_map_is_a_class_with_the_operations_of_table_47`

**Status:** done — `insert(key, element, replace)`: with `replace` an existing entry is overwritten, without it the result is DDS::RETCODE_BAD_PARAMETER; log: `.agent/REPORT.md`.

### §7.5.1.3.4 Modern C++: std::map<K, T>

**Spec:** §7.5.1.3.4, p. 190 — "The Map type shall be bound to an instantiation
of the std::map template."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Map unter `CppMapping::Psm`), `cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader` (`crates/cpp/tests/generated_psm_roundtrip.cpp`)

**Status:** done — in the default mapping `map<K,V>` is `std::map<K,V>`; `--cpp-mapping native` (`zerodds::Map`, the operations of Table 47) and `classic` are named options; log: `.agent/REPORT.md`.

### §7.5.1.3.4 Modern C++: Compare on the contents for string keys

**Spec:** §7.5.1.3.4, p. 190 — "When a map has keys of a string type, the
Compare function shall operate on the character contents of the strings; it
shall not operate on the strings' pointer values".

**Repo:** `crates/cpp/include/zerodds/` (the `std::map` comparison), `crates/cpp/tests/generated_psm_roundtrip.cpp`

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** done — a `std::string` key compares by contents (`std::less<std::string>`), which the program in `generated_psm_roundtrip.cpp` checks; log: `.agent/REPORT.md`.

### §7.5.1.3.5 Java: implementation of java.util.Map, boxed keys and values

**Spec:** §7.5.1.3.5, p. 190 — "An IDL map type shall be represented in Java by
an implementation of the standard java.util.Map interface. [...] If either of
these Java types is a primitive type, then the corresponding object box type
[...] shall be used in its place."

**Repo:** `crates/lang-rules/src/adapter/java.rs::map_class`, `map_key_supported`

**Tests:** `crates/lang-rules/tests/java_map_class.rs::a_map_is_a_java_util_map_with_the_operations_of_table_47`,
`crates/lang-rules/tests/java_map_class.rs::a_structure_or_union_is_a_key_of_a_java_util_map`

**Status:** done — `ZdMap_…` extends `java.util.LinkedHashMap` over the boxed types and carries the operations of Table 47; keys are primitive, string, enumeration, bitmask, bit set, structure and union (generated `equals`/`hashCode`); log: `.agent/REPORT.md`.

### §7.5.1.3.5 Java: map with keys that are none of primitive, string, enumeration or bitmask

**Spec:** §7.5.1.3.5, p. 190 — "An IDL map type shall be represented in Java by an implementation of the standard java.util.Map interface."

**Repo:** `crates/lang-rules/src/adapter/java.rs::map_key_supported`, `equals_and_hash_code`

**Tests:** `crates/lang-rules/tests/java_map_class.rs::a_structure_or_union_is_a_key_of_a_java_util_map`

**Status:** done — a structure, union or bit set as a key gives the generated class `equals` and `hashCode` over the contents; log: `.agent/REPORT.md`.

### §7.5.1.3.6 Other languages: binding by the IDL equivalent in §7.4.1.1.4

**Spec:** §7.5.1.3.6, p. 190 — "In all languages for which no language-specific
mapping is specified, the language binding for map types shall be based on the
equivalent IDL definition given in 7.4.1.1.4."

**Repo:** `crates/lang-rules/src/adapter/cst.rs` (`RustMapForm`), `crates/wire/src/map_entries.rs`; the other languages in `crates/lang-rules/src/adapter/`

**Tests:** `crates/lang-rules/tests/rust_corpus_compiles.rs::a_map_is_a_sequence_of_entries_that_keeps_its_order`,
`crates/wire-conformance/tests/map_bytes.rs::the_bytes_are_the_recorded_fast_dds_row`,
`crates/lang-rules/tests/go_end_to_end.rs::a_map_becomes_a_slice_of_entries_and_never_a_go_map` (ebenso D, Nim, Zig, Swift, Kotlin, F#, Julia, Lua, OCaml, Ada, Ada83, Elixir)

**Status:** done — Rust holds a map as a sequence of `MapEntry` by default (`--rust-map btree-map` is the named option); the other languages without a mapping of their own bind a sequence of entries (Go, D, Nim, Zig, Swift, Kotlin, F#, Julia, Lua, OCaml, Ada, Ada83) or a list of pairs (Python, Elixir); C# follows the form of Java; TypeScript takes the mapping of DDS-TS §7.9.1. Log: `.agent/REPORT.md`.

---

## §7.5.1.4 Structure and Union Types

### §7.5.1.4 Structures and unions by the IDL language mappings

**Spec:** §7.5.1.4, p. 190 — "The Plain Language Binding for structure and union
types shall correspond to the IDL language mappings for IDL structures and
unions as amended below."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::union_members`, `crates/lang-rules/src/adapter/java.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `tools/idlc/tests/cpp_union_accessors.rs::a_union_has_the_accessor_functions_and_goes_through_the_codec`,
`crates/lang-rules/tests/java_map_class.rs` (Union-Zugriffsfunktionen),
`crates/lang-rules/tests/c_end_to_end.rs::a_union_becomes_a_discriminator_beside_a_union`

**Status:** done — a structure becomes a structure or class, a union a class with accessor functions (`_d()` and an accessor and a modifier per case; C++ and Java) or in C `_d` with `union _u`; log: `.agent/REPORT.md`.

### §7.5.1.4.1.1 C++: a derived structure inherits publicly

**Spec:** §7.5.1.4.1.1, p. 191 — "The C++ struct corresponding to the subtype
shall publicly inherit from the C++ struct corresponding to the supertype."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::class`

**Tests:** `tools/idlc/tests/cpp_inheritance.rs::a_derived_struct_inherits_and_the_octet_is_its_own_type`

**Status:** done — `struct Derived : public Base`, only the own members stand in the class; the bytes equal those of the flat structure (§7.4.1.1.5.1); log: `.agent/REPORT.md`.

### §7.5.1.4.1.2 Java: the derived class extends the base class

**Spec:** §7.5.1.4.1.2, p. 191 — "The Java class corresponding to the subtype
shall extend the Java class corresponding to the supertype."

**Repo:** `crates/lang-rules/src/adapter/java.rs::data_class`

**Tests:** `crates/lang-rules/tests/java_map_class.rs::a_derived_class_extends_its_base_class`

**Status:** done — `class Derived extends Base`, only the own members stand in the class; log: `.agent/REPORT.md`.

### §7.5.1.4.1.3 Other languages: the base as the first member parent

**Spec:** §7.5.1.4.1.3, p. 191 — "The language binding shall be generated as if
an instance of the base type were the first member of the subtype with the name
'parent,' as in the following equivalent IDL definition: struct <struct_name> {
<base_type_name> parent; ... };"

**Repo:** `crates/lang-rules/src/emit/mod.rs::reach`; the adapters `crates/lang-rules/src/adapter/c.rs`, `crates/lang-rules/src/adapter/cst.rs`, `crates/lang-rules/src/adapter/go.rs`, `crates/lang-rules/src/adapter/d.rs`, `crates/lang-rules/src/adapter/nim.rs`, `crates/lang-rules/src/adapter/zig.rs`, `crates/lang-rules/src/adapter/swift.rs`, `crates/lang-rules/src/adapter/kotlin.rs`, `crates/lang-rules/src/adapter/fsharp.rs`, `crates/lang-rules/src/adapter/julia.rs`, `crates/lang-rules/src/adapter/lua.rs`, `crates/lang-rules/src/adapter/python.rs`, `crates/lang-rules/src/adapter/elixir.rs`, `crates/lang-rules/src/adapter/node.rs`, `crates/lang-rules/src/adapter/ada.rs`, `crates/lang-rules/src/adapter/ada83.rs`, `crates/lang-rules/src/adapter/ocaml.rs`, `crates/lang-rules/src/adapter/csharp.rs`, `crates/lang-rules/src/adapter/typescript.rs` (`cst.rs` is Rust; C# as a base class by IDL4 to C# §7.14.1)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_derived_struct_holds_the_base_as_its_first_member_parent`,
`crates/lang-rules/tests/rust_corpus_compiles.rs::a_derived_structure_holds_its_base_as_its_first_member_parent`,
`crates/lang-rules/tests/typescript_descriptors.rs::a_derived_struct_holds_its_base_as_the_member_parent`,
`crates/wire-probe/tests/three_column_gate.rs::the_three_columns_agree` (case `inherit-two-levels`: Rust, C and Python),
the `*_end_to_end` tests of Go, D, Nim, Zig, Swift, Julia, Lua, OCaml, Ada, Ada83 and Node, `python_corpus_compiles`, `elixir_end_to_end`, `csharp_corpus_compiles`, `typescript_corpus_compiles`

**Status:** done — the base is the first member `parent` in C, Rust, Go, D, Nim, Zig, Swift, Kotlin, F#, Julia, Lua, Python, Elixir, Node, Ada, Ada83, OCaml and TypeScript (in the descriptor as a member with `base: true`, which `getKey` and `withDefaults` follow); C# inherits from the base class (IDL4 to C# §7.14.1); the bytes are those of the flat structure (§7.4.1.1.5.1). `kotlin_end_to_end` and `fsharp_end_to_end` run in the integration round. Log: `.agent/REPORT.md`.

### §7.5.1.4.1.3 Other languages except C: base as first member parent

**Spec:** §7.5.1.4.1.3, p. 191 — "The language binding shall be generated as if an instance of the base type were the first member of the subtype with the name 'parent'"

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`, `crates/lang-rules/src/emit/typescript.rs`, `runtime/typescript/src/registry.ts`

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::a_derived_struct_holds_its_base_as_the_member_parent`,
`crates/lang-rules/tests/typescript_corpus_compiles.rs`

**Status:** done — TypeScript: the interface and the factory have `parent` as the first member, the descriptor lists it with `base: true`, the guard calls that of the base, the codec reaches a member of the base as `parent.m`. Log: `.agent/REPORT.md`.

---

## §7.5.2 Dynamic Language Binding

### §7.5.2 Dynamic Type and Dynamic Data Language Binding

**Spec:** §7.5.2, p. 191 — "The Dynamic Type Language Binding provides an API to
manipulate types. [...] The Dynamic Data Language Binding provides an API to
manipulate objects of any Type."

**Repo:** `crates/types/src/dynamic/type_.rs::DynamicType`; `crates/types/src/dynamic/data.rs::DynamicData`; `crates/types/src/dynamic/builder.rs::DynamicTypeBuilder`

**Tests:** `crates/dcps/tests/annex_c_idl_conformance.rs::every_operation_of_annex_c_has_its_counterpart`; `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`; `crates/types/tests/dynamic_corpus.rs::a_dynamic_data_writes_the_octets_a_json_value_writes`

**Status:** done — every operation of Tables 48 to 58 and of Annex C has its counterpart (entries below, all done or n/a); `annex_c_idl_conformance` reads the IDL of the annex and checks it; log in `.agent/REPORT.md`.

### §7.5.2 Figure 26 — classes DynamicType, DynamicTypeBuilderFactory, DynamicData, DynamicDataFactory

**Spec:** §7.5.2, Figure 26, p. 192 — "DynamicTypeBuilderFactory: This type is
logically a singleton. [...] DynamicDataFactory: This type is logically a
singleton."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the classes are listed one by one
below.

### §7.5.2.1 UML mapping: enumerations, classifiers, properties, operations

**Spec:** §7.5.2.1, p. 192–194 — "UML enumerations are mapped to IDL
enumerations. [...] UML classifiers with value semantics are represented as IDL
valuetypes. Classifiers with reference semantics are represented as local
interfaces. [...] Properties with multiplicity [0..1] are defined as @optional.
[...] UML operations are represented as IDL operations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — describes how the IDL in Annex C arises from
the UML model. ZeroDDS has no IDL PSM of the Dynamic Language Binding (Annex C
is listed there).

### §7.5.2.1 Multi-valued property: get_<p>_count, get_<p>, set_<p> with RETCODE_BAD_PARAMETER

**Spec:** §7.5.2.1, p. 193 — "The 'get' operation shall fail with
RETCODE_BAD_PARAMETER if the given index is outside of the current range. The
'set' operation shall do the same with one exception: it shall allow an index
one past the end [...] Either operation shall fail with RETCODE_BAD_PARAMETER if
either argument is nil."

**Repo:** `crates/types/src/dynamic/type_.rs::annotation`,
`crates/types/src/dynamic/type_.rs::get_member_by_index`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_index_past_the_end_is_a_bad_parameter`,
`crates/types/tests/dynamic_annotation_descriptors.rs::the_member_lookups_report_a_bad_parameter`

**Status:** done — `annotation_count`/`annotation(index)`, `verbatim_text_count`/`verbatim_text(index)` and `member_count`/`get_member_by_index` follow the rule: an index past the end is `RETCODE_BAD_PARAMETER` (`DynamicError::BadParameter`, `return_code`). Appending is `apply_annotation` and `add_member`.

### §7.5.2.1 Qualified association: get_<p>, get_all_<p>, set_<p>, RETCODE_NO_DATA

**Spec:** §7.5.2.1, p. 193–194 — "The 'get' operation shall return with
RETCODE_NO_DATA if no value exists for the given key. Either operation shall
return with RETCODE_BAD_PARAMETER if either argument is nil."

**Repo:** `crates/types/src/dynamic/annotation.rs::get_value`,
`crates/types/src/dynamic/annotation.rs::set_value`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`

**Status:** done — `AnnotationDescriptor::get_value(key)`, `get_all_value()` and `set_value(key, value)`: a key with no value is `RETCODE_NO_DATA`, an empty key or value `RETCODE_BAD_PARAMETER`.

---

## §7.5.2.2 DynamicTypeBuilderFactory

### §7.5.2.2 DynamicTypeBuilderFactory: logically a singleton

**Spec:** §7.5.2.2, p. 194 — "This class is logically a singleton (although it
need not technically be a singleton in practice). Its 'only' instance is the
starting point for creating and deleting DynamicTypeBuilder objects."

**Repo:** `crates/types/src/dynamic/factory.rs::DynamicTypeBuilderFactory`,
`crates/types/src/dynamic/import.rs::import`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`,
`crates/types/tests/dynamic_type_builder_factory.rs::a_copy_carries_the_state_and_leaves_the_original_alone`

**Status:** done — `DynamicTypeBuilderFactory` has no state; every instance behaves as every other. A `TypeBuilder` per type owns a model of its own; a type that an argument names is copied into that model with everything it reaches (`DynamicTypeBuilder::import`, recursive types included). `DynamicTypeBuilder` remains the factory over one shared model.

### §7.5.2.2.11 Table 48 — get_instance

**Spec:** §7.5.2.2.11, Table 48, p. 194, 198 — "Return a
DynamicTypeBuilderFactory instance that behaves like a singleton [...] Calling
this operation is legal even after delete_instance has been called."

**Repo:** `crates/types/src/dynamic/factory.rs::get_instance`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_primitive_type_is_answered_and_a_composite_kind_is_not`

**Status:** done — `DynamicTypeBuilderFactory::get_instance()`; a call after `delete_instance` is legal.

### §7.5.2.2.9 Table 48 — delete_instance

**Spec:** §7.5.2.2.9, Table 48, p. 194, 198 — "Reclaim any resources associated
with any object(s) previously returned from get_instance. [...] This operation
shall fail with RETCODE_ERROR if it fails for any implementation-specific
reason."

**Repo:** `crates/types/src/dynamic/factory.rs::delete_instance`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_primitive_type_is_answered_and_a_composite_kind_is_not`

**Status:** done — `DynamicTypeBuilderFactory::delete_instance()` succeeds; there is nothing to reclaim.

### §7.5.2.2.12 Table 48 — get_primitive_type

**Spec:** §7.5.2.2.12, Table 48, p. 194, 198 — "Retrieve a DynamicType object
corresponding to the indicated primitive type kind. [...] If the given kind does
not correspond to a primitive type, the operation shall fail and return a nil
value."

**Repo:** `crates/types/src/dynamic/factory.rs::get_primitive_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_primitive_type_is_answered_and_a_composite_kind_is_not`

**Status:** done — `get_primitive_type(kind)` returns the type; a kind that is no primitive type is `RETCODE_BAD_PARAMETER`.

### §7.5.2.2.6 Table 48 — create_type(TypeDescriptor)

**Spec:** §7.5.2.2.6, Table 48, p. 194, 197 — "Create and return a new
DynamicTypeBuilder object as described by the given type descriptor. [...] If
this argument is nil or inconsistent (as indicated by its is_consistent
operation), this operation shall fail and return a nil value."

**Repo:** `crates/types/src/dynamic/factory.rs::create_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`,
`crates/types/tests/dynamic_type_builder_factory.rs::an_inconsistent_descriptor_creates_no_type`

**Status:** done — `create_type(&TypeDescriptor)` creates structure, union, enumeration, bitmask, bitset, alias, string, sequence, array and map; an inconsistent descriptor and a type with no name are `RETCODE_BAD_PARAMETER`; an annotation kind is no type of this model and is refused as `Unrepresentable`.

### §7.5.2.2.7 Table 48 — create_type_copy

**Spec:** §7.5.2.2.7, Table 48, p. 194, 197 — "Create and return a new
DynamicTypeBuilder object with a copy of the state of the given type."

**Repo:** `crates/types/src/dynamic/factory.rs::create_type_copy`,
`crates/types/src/dynamic/import.rs::import`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_copy_carries_the_state_and_leaves_the_original_alone`

**Status:** done — `create_type_copy(&DynamicType)` returns a builder with a copy of the state; changes to the builder do not reach the type.

### §7.5.2.2.8 Table 48 — create_type_w_type_object

**Spec:** §7.5.2.2.8, Table 48, p. 194, 197 — "Create and return a new
DynamicTypeBuilder object that describes a type identical to that described by
the given TypeObject object. Subsequent changes to the new DynamicTypeBuilder
object shall not be reflected in the input TypeObject object."

**Repo:** `crates/types/src/dynamic/factory.rs::create_type_w_type_object`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_type_is_created_from_a_type_object`,
`crates/types/src/dynamic/to_ast.rs::tests::a_minimal_type_object_is_refused_by_name`

**Status:** done — `create_type_w_type_object(&TypeObject, &TypeRegistry)` returns a builder for the type a COMPLETE TypeObject describes; changes to the builder do not reach the object. A MINIMAL TypeObject has no names and is refused as `MinimalHasNoNames`.

### §7.5.2.2.5 Table 48 — create_string_type

**Spec:** §7.5.2.2.5, Table 48, p. 194, 197 — "The element type of the result
returned by create_string_type shall be Char8. [...] If this argument is equal
to LENGTH_UNLIMITED, the string type shall be considered to be unbounded."

**Repo:** `crates/types/src/dynamic/factory.rs::create_string_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::the_collection_and_string_creators_build_their_kinds`

**Status:** done — `create_string_type(bound)`; `LENGTH_UNLIMITED` is unbounded, the element is Char8.

### §7.5.2.2.5 Table 48 — create_wstring_type

**Spec:** §7.5.2.2.5, Table 48, p. 194, 197 — "The element type of the result
returned by create_wstring_type shall be Char16."

**Repo:** `crates/types/src/dynamic/factory.rs::create_wstring_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::the_collection_and_string_creators_build_their_kinds`

**Status:** done — `create_wstring_type(bound)`; the element is Char16.

### §7.5.2.2.4 Table 48 — create_sequence_type

**Spec:** §7.5.2.2.4, Table 48, p. 195–197 — "All sequence types having equal
element types and equal bounds shall be considered equal. [...] Parameter
element_type [...] If this argument is nil, the operation shall fail with
RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::create_sequence_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::the_collection_and_string_creators_build_their_kinds`

**Status:** done — `create_sequence_type(&DynamicType, bound)`; the element type is a copy of the argument.

### §7.5.2.2.1 Table 48 — create_array_type

**Spec:** §7.5.2.2.1, Table 48, p. 195–196 — "All array types having equal
element types, an equal number of dimensions, and equal bounds in each dimension
shall be considered equal. [...] Parameter bound - A collection of unsigned
integers, the length of which is equal to the number of dimensions".

**Repo:** `crates/types/src/dynamic/factory.rs::create_array_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::the_collection_and_string_creators_build_their_kinds`

**Status:** done — `create_array_type(&DynamicType, &[bound])`; no dimension is `RETCODE_BAD_PARAMETER`.

### §7.5.2.2.3 Table 48 — create_map_type

**Spec:** §7.5.2.2.3, Table 48, p. 195–196 — "All map types having equal key and
value element types and equal bounds shall be considered equal. [...] If this
argument is equal to LENGTH_UNLIMITED, the map type shall be considered to be
unbounded."

**Repo:** `crates/types/src/dynamic/factory.rs::create_map_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::the_collection_and_string_creators_build_their_kinds`

**Status:** done — `create_map_type(&DynamicType, &DynamicType, bound)`.

### §7.5.2.2.2 Table 48 — create_bitmask_type

**Spec:** §7.5.2.2.2, Table 48, p. 195–196 — "Parameter bound - The number of
reserved bits in the bitmask. If this value is out of range, the operation shall
fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::create_bitmask_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::an_enumeration_and_a_bitmask_take_literals_and_flags`

**Status:** done — `create_bitmask_type(bound)`; a bound outside 1 to 64 is `RETCODE_BAD_PARAMETER`.

### §7.5.2.2.13 Table 48 — create_type_w_uri

**Spec:** §7.5.2.2.13, Table 48, p. 195, 199 — "Create and return a new
DynamicType object by parsing the type description at the given URL. [...]
Implementations shall minimally support the file:// URL scheme [...] the XML
Type Description format [...] include_paths".

**Repo:** `crates/xml-compose/src/lib.rs::create_type_w_uri`

**Tests:** `crates/xml-compose/tests/create_type_operations.rs::a_type_is_created_from_a_file_url`

**Status:** done — `create_type_w_uri(url, type_name, include_paths)` in `zerodds-xml-compose`: a `file://` URL and a path are read, the directory of the document is on the search path implicitly, another scheme is an error.

### §7.5.2.2.14 Table 48 — create_type_w_document

**Spec:** §7.5.2.2.14, Table 48, p. 195, 199–200 — "Create and return a new
DynamicType object by parsing the type description contained in the given
string. [...] If no type exists of this name in the document [...] the operation
shall fail and return a nil result."

**Repo:** `crates/xml-compose/src/lib.rs::create_type_w_document`

**Tests:** `crates/xml-compose/tests/create_type_operations.rs::a_type_is_created_from_a_document`,
`crates/xml-compose/tests/create_type_operations.rs::a_missing_type_and_a_bad_document_fail`,
`crates/xml-compose/tests/create_type_operations.rs::the_include_paths_are_searched`

**Status:** done — `create_type_w_document(document, type_name, include_paths)` in `zerodds-xml-compose`: the type of that name from the XML document; a missing type and a document with an error give an error.

### §7.5.2.2.10 Table 48 — delete_type

**Spec:** §7.5.2.2.10, Table 48, p. 195, 198 — "Delete the given DynamicType
object [...] Deletions of nil [...] Deletions of objects returned by
get_primitive_type [...] shall always succeed but shall have no observable
effect [...] RETCODE_ALREADY_DELETED".

**Repo:** `crates/types/src/dynamic/factory.rs::delete_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_primitive_type_is_answered_and_a_composite_kind_is_not`

**Status:** done — `delete_type(DynamicType)` succeeds; the type is dropped with its last holder.

---

## §7.5.2.3 AnnotationDescriptor

### §7.5.2.3.4 Table 49 — Property type

**Spec:** §7.5.2.3.4, Table 49, p. 200–202 — "The type property contains a
reference to the annotation type [...] When an annotation descriptor is newly
created, this reference shall be nil."

**Repo:** `crates/types/src/dynamic/annotation.rs::AnnotationDescriptor`,
`crates/types/src/dynamic/annotation.rs::AnnotationType`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`,
`crates/types/tests/dynamic_annotation_descriptors.rs::a_type_reports_its_builtin_and_declared_annotations`

**Status:** done — `AnnotationDescriptor::annotation_type()` is an `AnnotationType` of kind `ANNOTATION_TYPE`; a new descriptor has none.

### §7.5.2.3.5 Table 49 — Property value

**Spec:** §7.5.2.3.5, Table 49, p. 200, 202 — "This property contains a mapping
from the names of attributes defined by type to valid values of that type. Any
attribute defined by type but for which no name appears in this property shall
be considered to have its default value."

**Repo:** `crates/types/src/dynamic/annotation.rs::get_all_value`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_reports_its_builtin_and_declared_annotations`

**Status:** done — `value` is a map from attribute names to strings; an attribute with no entry has its default value.

### §7.5.2.3.1 Table 49 — Operation copy_from

**Spec:** §7.5.2.3.1, Table 49, p. 200–201 — "Overwrite the contents of this
descriptor with those of another descriptor such that subsequent calls to
equals, passing the same argument as to this method, return true."

**Repo:** `crates/types/src/dynamic/annotation.rs::copy_from`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`

**Status:** done — `AnnotationDescriptor::copy_from(&other)`.

### §7.5.2.3.2 Table 49 — Operation equals

**Spec:** §7.5.2.3.2, Table 49, p. 200–201 — "Two annotation descriptors ad1 and
ad2 are considered equal if and only if all of the following apply: Their type
properties refer to equal types. [...]"

**Repo:** `crates/types/src/dynamic/annotation.rs::equals`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`

**Status:** done — `AnnotationDescriptor::equals(&other)`: equal types and the same attributes with equal values.

### §7.5.2.3.3 Table 49 — Operation is_consistent

**Spec:** §7.5.2.3.3, Table 49, p. 200–201 — "The type property refers to a
non-nil type of kind ANNOTATION_TYPE. [...] String s2 is a well-formed string
representation of an object of the type of the attribute named by s1."

**Repo:** `crates/types/src/dynamic/annotation.rs::is_consistent`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`

**Status:** done — `AnnotationDescriptor::is_consistent()`: the type is not nil and an annotation type, every attribute belongs to the type, every value is a well-formed string of its type.

### §7.5.2.3.5 AnnotationDescriptor: well-formed values, no quotation marks, evaluated expressions

**Spec:** §7.5.2.3.5, Table 49, p. 202 — "String and character literals shall
not be surrounded by quotation characters [...] All expressions shall be fully
evaluated"

**Repo:** `crates/types/src/dynamic/annotation.rs::is_well_formed`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`

**Status:** done — A value without quotation marks and without operators: `"5"` is well-formed for an integer attribute, `"2 + 3"` is not (`ValueKind::is_well_formed`).

---

## §7.5.2.4 TypeDescriptor

### §7.5.2.4 TypeDescriptor: value semantics

**Spec:** §7.5.2.4, p. 202 — "A TypeDescriptor packages together the state of a
type. TypeDescriptor objects have value semantics, allowing them to be deeply
copied and compared."

**Repo:** `crates/types/src/dynamic/descriptor.rs::TypeDescriptor` (l. 381);
`crates/types/src/dynamic/type_.rs::descriptor` (l. 189)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`

**Status:** done — a value (`Clone`, `PartialEq`), read from the model.

### §7.5.2.4.9 Table 50 — kind

**Spec:** §7.5.2.4.9, Table 50, p. 203, 205 — "An enumerated value that
indicates what 'kind' of type this descriptor describes".

**Repo:** `crates/types/src/dynamic/type_.rs::kind` (l. 153);
`crates/types/src/dynamic/descriptor.rs::TypeKind` (l. 48)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`,
`crates/types/src/dynamic/descriptor.rs::tests::octet_and_uint8_are_two_kinds`

**Status:** done — constructs without an XTypes kind (`fixed`, `valuetype`,
`any` and others) give `TypeKind::NoType`.

### §7.5.2.4.10 Table 50 — name: fully qualified

**Spec:** §7.5.2.4.10, Table 50, p. 203, 205 — "The fully qualified name of the
type described by this descriptor. To be consistent, this name must be a valid
identifier for the given type kind".

**Repo:** `crates/types/src/dynamic/type_.rs::qualified_name`,
`crates/types/src/dynamic/descriptor.rs::is_consistent`

**Tests:** `crates/types/tests/dynamic_corpus.rs::a_dynamic_type_names_the_node_the_encoder_encodes`,
`crates/types/tests/dynamic_annotation_descriptors.rs::the_descriptor_of_a_union_and_a_map_and_the_names`,
`crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `name` is the fully qualified name without a leading `::` (`qualified_name`); `is_consistent` requires identifiers separated by `::` for named kinds.

### §7.5.2.4.1 Table 50 — base_type: supertype of a structure, target of an alias

**Spec:** §7.5.2.4.1, Table 50, p. 203 — "If this descriptor represents a
structure type, base_type indicates the supertype of that type. [...] If this
descriptor represents an alias type, base_type indicates the type being aliased.
[...] In all other cases, a consistent descriptor shall have a nil value for
this property."

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor` (l. 189)

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_alias_names_its_target_as_the_base_type`,
`crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`

**Status:** done — For a structure the supertype, for an alias the target (`base_type`, Table 50); a bitset carries its base there too.

### §7.5.2.4.1 base_type: the target of an alias is in base_type

**Spec:** §7.5.2.4.1, p. 203 — "If this descriptor represents an alias type,
base_type indicates the type being aliased. A nil value for this property is not
considered consistent."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_alias_names_its_target_as_the_base_type`

**Status:** done — `descriptor` sets the target of an alias in `base_type`; `is_consistent` checks it there.

### §7.5.2.4.4 Table 50 — discriminator_type

**Spec:** §7.5.2.4.4, Table 50, p. 203–204 — "If this descriptor represents a
union type, discriminator_type indicates the type of the discriminator of the
union. It must not be nil for the descriptor to be consistent."

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor`,
`crates/types/src/dynamic/descriptor.rs::is_consistent`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::the_descriptor_of_a_union_and_a_map_and_the_names`

**Status:** done — `discriminator_type` of a union is the type of the discriminator; without it the descriptor is inconsistent.

### §7.5.2.4.2 Table 50 — bound

**Spec:** §7.5.2.4.2, Table 50, p. 203 — "If this descriptor represents an array
type, the length of the property value indicates the number of dimensions [...]
If this descriptor represents a sequence, map, bitmask, or string type, the
length of the property value is one and the integral value in that property
indicates the bound of the collection."

**Repo:** `crates/types/src/dynamic/type_.rs::one_bound`

**Tests:** `crates/types/tests/dynamic_annex_c.rs::the_bound_of_an_unbounded_collection_is_one_value`; `crates/types/tests/dynamic_type_descriptors.rs::bounds_are_checked_and_unlimited_is_unbounded`

**Status:** done — array: one number per dimension; sequence, map, string, bitmask: one number; unbounded is `LENGTH_UNLIMITED`; log: `.agent/REPORT.md`.

### §7.5.2.4.2 the bound of a bitmask is in bound

**Spec:** §7.5.2.4.2, p. 203 — "If this descriptor represents a sequence, map,
bitmask, or string type, the length of the property value is one".

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — For a bitmask the bound is in `bound`; `bit_bound` stays empty.

### §7.5.2.4.5 Table 50 — element_type

**Spec:** §7.5.2.4.5, Table 50, p. 203–204 — "If this descriptor represents an
array, sequence, or string type, this property indicates the element type of the
collection. It must not be nil [...] If this descriptor represents a map type,
this property indicates the value element type [...] If this descriptor
represents a bitmask type, this property must indicate a Boolean type".

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor` (l. 189)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`,
`crates/types/tests/dynamic_type_descriptors.rs::a_string_has_a_character_element_type`,
`crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — Array, sequence: the element; map: the value; string: `Char8` or `Char16`; bitmask: Boolean.

### §7.5.2.4.5 element_type: set for string and bitmask

**Spec:** §7.5.2.4.5, p. 204 — "[array, sequence, or string type] It must not be
nil for the descriptor to be consistent. [...] If this descriptor represents a
bitmask type, this property must indicate a Boolean type for the descriptor to
be consistent."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_string_has_a_character_element_type`,
`crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — `descriptor` sets `Char8` or `Char16` for a string and Boolean for a bitmask as `element_type`.

### §7.5.2.4.8 Table 50 — key_element_type

**Spec:** §7.5.2.4.8, Table 50, p. 203–205 — "If this descriptor represents a
map type, this property indicates the value element type of the map. It must not
be nil for the descriptor to be consistent."

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor`,
`crates/types/src/dynamic/descriptor.rs::is_consistent`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::the_descriptor_of_a_union_and_a_map_and_the_names`

**Status:** done — `key_element_type` of a map is the key type; without it the descriptor is inconsistent. (The text of the spec says "value element type" for the key; a typo, listed in the findings file.)

### §7.5.2.4.3 Table 50 — copy_from

**Spec:** §7.5.2.4.3, Table 50, p. 203–204 — "Overwrite the contents of this
descriptor with those of another descriptor such that subsequent calls to
equals, passing the same argument as to this method, return true."

**Repo:** `crates/types/src/dynamic/descriptor.rs::copy_from`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_descriptor_has_value_semantics`

**Status:** done — `TypeDescriptor::copy_from(&other)`.

### §7.5.2.4.6 Table 50 — equals

**Spec:** §7.5.2.4.6, Table 50, p. 203–204 — "Two type descriptors are
considered equal if and only if the values of all of the properties identified
in Table 50 above are equal in each of them."

**Repo:** `crates/types/src/dynamic/descriptor.rs::equals`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_descriptor_has_value_semantics`

**Status:** done — `TypeDescriptor::equals(&other)`: all properties of Table 50 equal, the types as types.

### §7.5.2.4.7 Table 50 — is_consistent

**Spec:** §7.5.2.4.7, Table 50, p. 203–204 — "Indicates whether the states of
all of this descriptor's properties are consistent."

**Repo:** `crates/types/src/dynamic/descriptor.rs::is_consistent`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::the_descriptor_of_a_union_and_a_map_and_the_names`,
`crates/types/tests/dynamic_type_descriptors.rs::an_alias_names_its_target_as_the_base_type`

**Status:** done — `TypeDescriptor::is_consistent()` checks each property against the kinds that carry it, and the name.

---

## §7.5.2.5 MemberId

### §7.5.2.5 MemberId: alias to UInt32, MEMBER_ID_INVALID

**Spec:** §7.5.2.5, p. 205 — "The type MemberId is an alias to UInt32 [...] It
is also used to type the constant MEMBER_ID_INVALID, which is a sentinel
indicating a member ID that is missing, irrelevant, or otherwise invalid in a
given context."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_enumerator_has_no_member_id_and_the_enumeration_as_its_type`,
`crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — `MemberId` is `u32`, `MEMBER_ID_INVALID` is `0x0FFF_FFFF`; the members of an enumeration and a bitmask carry it.

---

## §7.5.2.6 DynamicTypeMember

### §7.5.2.6 Table 51 — ANNOTATION_TYPE, STRUCTURE_TYPE, UNION_TYPE: members as for aggregates

**Spec:** §7.5.2.6, Table 51, p. 205 — "ANNOTATION_TYPE STRUCTURE_TYPE
UNION_TYPE | For these aggregated types, a 'member' in this sense has the same
meaning as it does in the definition of aggregated types generally."

**Repo:** `crates/types/src/dynamic/annotation.rs::AnnotationType::members`

**Tests:** `crates/types/tests/dynamic_annex_c.rs::an_annotation_type_has_members`

**Status:** done — `AnnotationType::members`, `member_count`, `get_member_by_index`, `get_member_by_name`, `get_member`, `get_all_members*` and `AnnotationMember`; log: `.agent/REPORT.md`.

### §7.5.2.6 Table 51 — BITMASK_TYPE: flags as members of type Boolean

**Spec:** §7.5.2.6, Table 51, p. 205 — "BITMASK_TYPE | Each named flag in a
bitmask shall be considered to be a 'member' of that bitmask with Boolean type."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — The flags of a bitmask are members of type Boolean; `literal_value` carries the position.

### §7.5.2.6 Table 51 — ENUMERATION_TYPE: literals as members of the enumeration's type

**Spec:** §7.5.2.6, Table 51, p. 205 — "ENUMERATION_TYPE | Each literal in the
enumeration shall be considered a 'member' of the type. These members shall have
the type of the enclosing enumeration itself."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_enumerator_has_no_member_id_and_the_enumeration_as_its_type`

**Status:** done — The literals of an enumeration are members of the type of the enumeration without a member ID; `literal_value` carries the value.

### §7.5.2.6 Table 51 — ALIAS_TYPE: as the base type

**Spec:** §7.5.2.6, Table 51, p. 205 — "ALIAS_TYPE | The behavior is as it would
be for the alias's base type."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_alias_has_the_members_of_its_base_and_a_bitset_has_none`

**Status:** done — An alias has the members of its base type.

### §7.5.2.6 Table 51 — no other kinds have members

**Spec:** §7.5.2.6, Table 51, p. 205 — "No other type kinds are considered to
have members."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_alias_has_the_members_of_its_base_and_a_bitset_has_none`

**Status:** done — A bitset has no members; kinds other than those named have none either.

### §7.5.2.6.1 Table 52 — Property annotation

**Spec:** §7.5.2.6.1, Table 52, p. 206–207 — "This property provides all
annotations previously applied to this member."

**Repo:** `crates/types/src/dynamic/type_.rs::annotations`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_reports_its_annotations`

**Status:** done — `DynamicTypeMember::annotations()`, `annotation_count()` and `annotation(index)`: the built-in and the declared annotations of the member.

### §7.5.2.6.2 Table 52 — get_descriptor

**Spec:** §7.5.2.6.2, Table 52, p. 206–207 — "This operation provides a summary
of the state of this type. It overwrites the state of an application-provided
object."

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor` (l. 853)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done — returns a new `MemberDescriptor` instead of overwriting an
argument.

### §7.5.2.6.3 Table 52 — equals

**Spec:** §7.5.2.6.3, Table 52, p. 206–207 — "Two members shall be considered
equal if and only if they belong to the same type and all of their respective
properties, as identified in Table 52 above, are equal."

**Repo:** `crates/types/src/dynamic/type_.rs::equals_at`

**Tests:** `crates/types/tests/dynamic_annex_c.rs::members_of_different_types_are_not_equal`; `crates/types/tests/dynamic_type_builder_factory.rs::equals_includes_the_applied_annotations`

**Status:** done — compared are the owner type, name, ID, index, labels, flags, type, annotations and `@verbatim` texts; log: `.agent/REPORT.md`.

### §7.5.2.6.4 Table 52 — get_id

**Spec:** §7.5.2.6.4, Table 52, p. 206–207 — "This convenience operation
provides the member ID of this member. Its result shall be identical to the ID
value that is a member of the descriptor property."

**Repo:** `crates/types/src/dynamic/type_.rs::id` (l. 701)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

### §7.5.2.6.5 Table 52 — get_name

**Spec:** §7.5.2.6.5, Table 52, p. 206–207 — "This convenience operation
provides the name of this member. Its result shall be identical to the name
string that is a member of the descriptor property."

**Repo:** `crates/types/src/dynamic/type_.rs::name` (l. 658)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

---

## §7.5.2.7 MemberDescriptor

### §7.5.2.7 MemberDescriptor: value semantics

**Spec:** §7.5.2.7, p. 207 — "A MemberDescriptor packages together the state of
a DynamicTypeMember. MemberDescriptor objects have value semantics".

**Repo:** `crates/types/src/dynamic/descriptor.rs::MemberDescriptor` (l. 462)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

### §7.5.2.7.9 Table 53 — name

**Spec:** §7.5.2.7.9, Table 53, p. 207, 210 — "This property indicates the name
of this member. The value must be a well-formed member name."

**Repo:** `crates/types/src/dynamic/descriptor.rs::is_consistent_in`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::the_descriptor_of_a_union_and_a_map_and_the_names`

**Status:** done — `name` of the member; `is_consistent_in` requires an identifier.

### §7.5.2.7.5 Table 53 — id

**Spec:** §7.5.2.7.5, Table 53, p. 207, 209 — "When a descriptor is retrieved
from an existing member, this property shall reflect the actual ID of the
member. [...] If this member does not belong to an aggregated type, this
property must be MEMBER_ID_INVALID, or the descriptor is not consistent."

**Repo:** `crates/types/src/dynamic/type_.rs::id` (l. 701)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`,
`crates/types/tests/dynamic_type_descriptors.rs::an_enumerator_has_no_member_id_and_the_enumeration_as_its_type`

**Status:** done — For structure and union members the effective ID; an enumerator, a bit and a bitfield report `MEMBER_ID_INVALID`.

### §7.5.2.7.5 id: members outside aggregates carry MEMBER_ID_INVALID

**Spec:** §7.5.2.7.5, p. 209 — "If this member does not belong to an aggregated
type, this property must be MEMBER_ID_INVALID".

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_enumerator_has_no_member_id_and_the_enumeration_as_its_type`,
`crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — `id` returns `MEMBER_ID_INVALID` for members outside aggregates.

### §7.5.2.7.5 id on adding: MEMBER_ID_INVALID gives maximum + 1

**Spec:** §7.5.2.7.5, p. 209 — "When a descriptor is used to add a new member to
a type, this property may be set to MEMBER_ID_INVALID; in that case, the
implementation shall select an ID for the new member that is one more than the
current maximum member ID in the type."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_member_with_no_id_gets_the_maximum_plus_one`

**Status:** done — A member without an ID gets the maximum of the existing IDs plus 1.

### §7.5.2.7.10 Table 53 — type

**Spec:** §7.5.2.7.10, Table 53, p. 207, 210 — "This property indicates the type
of the member's value. It must not be nil and must indicate a type that can
legally type a member".

**Repo:** `crates/types/src/dynamic/type_.rs::dynamic_type` (l. 767)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done — a handle, never nil; an unresolved type is
`DynamicError::Unresolved`.

### §7.5.2.7.3 Table 53 — default_value as a string

**Spec:** §7.5.2.7.3, Table 53, p. 207–208 — "This property provides the
member's default value in string form. [...] A nil or empty string indicates
that the member takes the 'default default' value for its type."

**Repo:** `crates/types/src/dynamic/descriptor.rs::MemberDescriptor`,
`crates/types/src/dynamic/type_.rs::descriptor`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`,
`crates/types/tests/dynamic_type_builder_factory.rs::an_enumeration_and_a_bitmask_take_literals_and_flags`

**Status:** done — `MemberDescriptor::default_value` is the `@default` as a string (evaluated, without quotation marks); empty means "default default". For a literal of an enumeration it is the value, for a flag the position.

### §7.5.2.7.6 Table 53 — index

**Spec:** §7.5.2.7.6, Table 53, p. 207, 209 — "The first member shall have index
zero, the next one, and so on. [...] When a descriptor is retrieved from an
existing member, this property shall always reflect the actual index at which
the member exists."

**Repo:** `crates/types/src/dynamic/type_.rs::index` (l. 748)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

### §7.5.2.7.6 index on adding: append or insert

**Spec:** §7.5.2.7.6, p. 209 — "any value greater than the current largest index
value in the type shall be taken to indicate that the new member will become the
last member [...] if this property is set to an index at which a member already
exists, that member and all those after it shall be shifted up".

**Repo:** `crates/types/src/dynamic/builder.rs::add_member_at`

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_member_added_at_an_index_shifts_the_others`

**Status:** done — `add_member_at` and `add_case_at` shift the members from the index on; an index past the end appends.

### §7.5.2.7.8 Table 53 — label: Int64[*]

**Spec:** §7.5.2.7.8, Table 53, p. 207, 209–210 — "label | Int64 [*]" — "If the
type to which the member belongs is a union, this property indicates the case
labels that apply to this member. If default_label is false, it must not be
empty. In addition, no two members of the same union can specify the same label
value."

**Repo:** `crates/types/src/dynamic/descriptor.rs::MemberDescriptor` (l. 462, field `label`)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`,
`crates/types/tests/dynamic_type_descriptors.rs::a_union_label_past_32_bits_is_kept`

**Status:** done — `label` is `Vec<i64>` (Table 53, `Int64 [*]`); Figure 29 names `Int32 [*]` (contradiction in the specification, kept in the findings file).

### §7.5.2.7.8 label: a value outside i32 is kept

**Spec:** §7.5.2.7.8, p. 209 — "this property indicates the case labels that
apply to this member".

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_union_label_past_32_bits_is_kept`

**Status:** done — A label beyond 32 bits is kept in the descriptor.

### §7.5.2.7.2 Table 53 — default_label

**Spec:** §7.5.2.7.2, Table 53, p. 207–208 — "For this descriptor to be
consistent, this property must be true if this descriptor identifies the default
member of a union type or false if not."

**Repo:** `crates/types/src/dynamic/descriptor.rs::is_consistent_in`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`,
`crates/types/tests/dynamic_type_builder_factory.rs::add_member_follows_table_55`

**Status:** done — `is_default_label` of a union member; `is_consistent_in` requires labels when it is not `default:`, and neither labels nor `default:` for other kinds.

### §7.5.2.7.1 Table 53 — copy_from

**Spec:** §7.5.2.7.1, Table 53, p. 208 — "Overwrite the contents of this
descriptor with those of another descriptor such that subsequent calls to equals
[...] return true."

**Repo:** `crates/types/src/dynamic/descriptor.rs::copy_from`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`

**Status:** done — `MemberDescriptor::copy_from(&other)`.

### §7.5.2.7.4 Table 53 — equals

**Spec:** §7.5.2.7.4, Table 53, p. 208–209 — "Two descriptors are considered
equal if and only if the values of all of the properties identified in Table 53
above are equal in each of them."

**Repo:** `crates/types/src/dynamic/descriptor.rs::equals`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`

**Status:** done — `MemberDescriptor::equals(&other)`.

### §7.5.2.7.7 Table 53 — is_consistent

**Spec:** §7.5.2.7.7, Table 53, p. 208–209 — "A descriptor shall be considered
consistent if and only if all of the values of its properties are considered
consistent."

**Repo:** `crates/types/src/dynamic/descriptor.rs::is_consistent_in`,
`crates/types/src/dynamic/descriptor.rs::is_consistent_for_add`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`,
`crates/types/tests/dynamic_type_builder_factory.rs::add_member_follows_table_55`

**Status:** done — `MemberDescriptor::is_consistent_in(owner)`: id, name, labels and default by the kind of the type the member belongs to; `is_consistent_for_add` allows `MEMBER_ID_INVALID`.

---

## §7.5.2.8 DynamicType

### §7.5.2.8 DynamicType: reference semantics, comparison by value

**Spec:** §7.5.2.8, p. 210 — "DynamicType objects have reference semantics [...]
However, the type nevertheless provides operations to allow copying and
comparison by value."

**Repo:** `crates/types/src/dynamic/type_.rs::DynamicType` (l. 58);
`crates/types/src/dynamic/type_.rs::equals` (l. 492)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::equality_is_structural_and_not_by_pointer`

**Status:** done — an `Arc` handle; `equals` compares structurally.

### §7.5.2.8.1 Table 54 — Property annotation

**Spec:** §7.5.2.8.1, Table 54, p. 211–212 — "This property provides all
annotations that have previously been applied to this type."

**Repo:** `crates/types/src/dynamic/type_.rs::annotations`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_reports_its_builtin_and_declared_annotations`

**Status:** done — `DynamicType::annotations()`, `annotation_count()` and `annotation(index)`.

### §7.5.2.8.2 Table 54 — Property member: by member ID, aggregates only

**Spec:** §7.5.2.8.2, Table 54, p. 211–212 — "This property contains a mapping
from the member ID of a member of this (aggregated) type to the member itself.
[...] If this type is not an aggregated type, the collection of members
available through this property shall be empty."

**Repo:** `crates/types/src/dynamic/type_.rs::member_by_id` (l. 480)

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`,
`crates/types/tests/dynamic_type_descriptors.rs::an_enumerator_has_no_member_id_and_the_enumeration_as_its_type`

**Status:** done — `member_by_id` applies to aggregates only; for enumeration, bitmask and bitset the mapping is empty.

### §7.5.2.8.3 Table 54 — Property member_by_name

**Spec:** §7.5.2.8.3, Table 54, p. 211, 213 — "This property contains a mapping
from the name of a member of this type to the member itself. [...] the
constituents of enumerations, bitmasks, and other kinds of types are also
considered to be 'members'".

**Repo:** `crates/types/src/dynamic/type_.rs::member_by_name` (l. 472)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

### §7.5.2.8.3 Table 55 — ANNOTATION_TYPE

**Spec:** §7.5.2.8.3, Table 55, p. 213 — "ANNOTATION_TYPE | The member
descriptor must describe a consistent annotation type member. If the descriptor
does not satisfy these constraints, the operation shall fail with
RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::create_annotation_type`,
`crates/types/src/dynamic/factory.rs::add_member_descriptor`,
`crates/types/src/dynamic/factory.rs::AnnotationTypeBuilder`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::an_annotation_type_is_built_and_applied`

**Status:** done — `DynamicTypeBuilderFactory::create_annotation_type(name)` returns an `AnnotationTypeBuilder`; `add_member_descriptor(&MemberDescriptor)` takes the descriptor of an annotation member (name, type, `default_value`) and refuses one that sets key, optional, shared, must understand or labels, a type an attribute cannot have, a duplicate member and a default that is no value of the type, with `RETCODE_BAD_PARAMETER`; `build` returns the `AnnotationType` that `apply_annotation` applies. An annotation type is no `TypeNode` of the model, so `create_type` with `TypeKind::Annotation` names the operation and stays `Unrepresentable`.

### §7.5.2.8.3 Table 55 — ALIAS_TYPE: members go to the base type

**Spec:** §7.5.2.8.3, Table 55, p. 213 — "ALIAS_TYPE | The behavior is as it
would be for the alias's base type. If adding a member is not defined for the
alias's base type, this operation shall fail with RETCODE_PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_member_added_to_an_alias_goes_to_the_base_type`

**Status:** done — `add_member` on an alias adds the member to the base type.

### §7.5.2.8.3 Table 55 — BITMASK_TYPE

**Spec:** §7.5.2.8.3, Table 55, p. 213 — "BITMASK_TYPE | The member descriptor
must describe a Boolean flag with a value within the bound of this bitmask type.
If the descriptor does not satisfy these constraints, the operation shall fail
with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::add_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::an_enumeration_and_a_bitmask_take_literals_and_flags`

**Status:** done — A flag has to be of type Boolean and have a position inside the bound; otherwise `RETCODE_BAD_PARAMETER`.

### §7.5.2.8.3 Table 55 — ENUMERATION_TYPE

**Spec:** §7.5.2.8.3, Table 55, p. 213 — "ENUMERATION_TYPE | The member
descriptor must describe a literal with the type of this enumeration. If the
descriptor does not satisfy these constraints, the operation shall fail with
RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::add_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::an_enumeration_and_a_bitmask_take_literals_and_flags`

**Status:** done — A literal has to have the type of the enumeration; its value is `default_value`, positional otherwise; else `RETCODE_BAD_PARAMETER`.

### §7.5.2.8.3 Table 55 — STRUCTURE_TYPE

**Spec:** §7.5.2.8.3, Table 55, p. 213 — "STRUCTURE_TYPE | The member descriptor
must describe a consistent structure member. If the descriptor does not satisfy
this constraint, the operation shall fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::add_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`,
`crates/types/tests/dynamic_type_builder_factory.rs::add_member_follows_table_55`

**Status:** done — A structure member is checked by `is_consistent_for_add` (name, id, no label, `@optional` without `@default`); the default has to read in the member's type.

### §7.5.2.8.3 Table 55 — UNION_TYPE

**Spec:** §7.5.2.8.3, Table 55, p. 213 — "UNION_TYPE | The member descriptor
must describe a consistent union member. If the descriptor does not satisfy this
constraint, the operation shall fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::add_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::add_member_follows_table_55`

**Status:** done — A union member needs labels or `default:`; otherwise `RETCODE_BAD_PARAMETER`.

### §7.5.2.8.3 Table 55 is titled member_by_name but describes add_member

**Spec:** §7.5.2.8.3, Table 55, p. 213 — "Table 55 – DynamicType::member_by_name
behavior"; §7.5.2.9.4, p. 217 — "the behavior shall be as described in Table 55
in Clause 7.5.2.8.3".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — defect of the specification (listed in the
findings file); the rows are listed above as rules for adding.

### §7.5.2.8.3 Lifecycle of DynamicTypeMember

**Spec:** §7.5.2.8.3, p. 213 — "Users of the DynamicTypeMember class are not
required to explicitly delete objects of that class. [...] Changes to one
DynamicTypeMember object representing a given member shall be reflected in all
observable DynamicTypeMember objects representing the same member. [...] All
DynamicTypeMember objects representing the same member shall compare as equal".

**Repo:** `crates/types/src/dynamic/type_.rs::DynamicTypeMember` (l. 635)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::equality_is_structural_and_not_by_pointer`

**Status:** done — a member is a view on the immutable model; two views on the
same member are equal.

### §7.5.2.8.4 Table 54 — equals

**Spec:** §7.5.2.8.4, Table 54, p. 212, 214 — "Two types shall be considered
equal if and only if all of their respective properties, as identified in Table
54 above, are equal."

**Repo:** `crates/types/src/dynamic/type_.rs::equals`,
`crates/types/src/dynamic/type_.rs::equals_ignoring_annotations`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::equals_includes_the_applied_annotations`,
`crates/types/src/dynamic/type_.rs::tests::equality_is_structural_and_not_by_pointer`

**Status:** done — `DynamicType::equals` compares descriptor, members, base type, discriminator, element and key type, cycles included, and the applied annotations and `@verbatim` texts of the type and of every member (in any order, by `AnnotationDescriptor::equals`). Built-in annotations that restate a compared property (`@key`, `@id`, `@final`, `@optional`, `@bit_bound`, …) count once. `equals_ignoring_annotations` is the comparison without annotations, for the compatibility check of `create_sample`.

### §7.5.2.8.5 Table 54 — get_annotation

**Spec:** §7.5.2.8.5, Table 54, p. 212, 214 — "This operation returns the
annotation that corresponds to the specified index, if any. The operation shall
fail if the specified index is greater than the current annotation count."

**Repo:** `crates/types/src/dynamic/type_.rs::annotation`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_reports_its_builtin_and_declared_annotations`

**Status:** done — `DynamicType::annotation(index)`.

### §7.5.2.8.6 Table 54 — get_annotation_count

**Spec:** §7.5.2.8.6, Table 54, p. 212, 214 — "This operation returns the
current number of annotations applied to the type."

**Repo:** `crates/types/src/dynamic/type_.rs::annotation_count`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_reports_its_builtin_and_declared_annotations`

**Status:** done — `DynamicType::annotation_count()`.

### §7.5.2.8.7 Table 54 — get_descriptor

**Spec:** §7.5.2.8.7, Table 54, p. 212, 214 — "This operation provides a summary
of the state of this type. It overwrites the state of an application-provided
object."

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor` (l. 189)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`

**Status:** done — returns a new `TypeDescriptor`.

### §7.5.2.8.8 Table 54 — get_kind

**Spec:** §7.5.2.8.8, Table 54, p. 212, 214 — "Its result shall be the same as
the kind indicated by the type's descriptor property."

**Repo:** `crates/types/src/dynamic/type_.rs::kind` (l. 153)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`,
`crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`

**Status:** done

### §7.5.2.8.9 Table 54 — get_member

**Spec:** §7.5.2.8.9, Table 54, p. 212, 214 — "This operation returns the member
that corresponds to the specified member ID, if any. If there is no member with
the specified member ID, the operation shall fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/type_.rs::member_by_id` (l. 480)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done — `None` instead of `RETCODE_BAD_PARAMETER`.

### §7.5.2.8.10 Table 54 — get_member_by_index

**Spec:** §7.5.2.8.10, Table 54, p. 212, 214 — "This operation returns the
member that corresponds to the specified index, if any. The operation shall fail
if the specified index is greater than the current member count."

**Repo:** `crates/types/src/dynamic/type_.rs::get_member_by_index`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::the_member_lookups_report_a_bad_parameter`

**Status:** done — `DynamicType::get_member_by_index(index)`: an index from the count on is `RETCODE_BAD_PARAMETER`. (The spec says "greater than"; listed in the findings file.)

### §7.5.2.8.11 Table 54 — get_member_by_name

**Spec:** §7.5.2.8.11, Table 54, p. 212, 214 — "If there is no member with the
specified name, the operation shall fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/type_.rs::member_by_name` (l. 472)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done — `None` instead of `RETCODE_BAD_PARAMETER`.

### §7.5.2.8.12 Table 54 — get_member_count

**Spec:** §7.5.2.8.12, Table 54, p. 212, 214 — "This operation returns the
current number of members."

**Repo:** `crates/types/src/dynamic/type_.rs::member_count` (l. 405)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

### §7.5.2.8.13 Table 54 — get_name: fully qualified

**Spec:** §7.5.2.8.13, Table 54, p. 212, 215 — "This convenience operation
provides the fully qualified name of this type. It shall be identical to the
name string that is a member of the descriptor property."

**Repo:** `crates/types/src/dynamic/type_.rs::qualified_name`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `get_name` is the fully qualified name, equal to the name of the descriptor (`qualified_name`); `name` remains the simple name.

---

## §7.5.2.9 DynamicTypeBuilder

### §7.5.2.9 DynamicTypeBuilder: transitional state of a type

**Spec:** §7.5.2.9, p. 215 — "A DynamicTypeBuilder object represents a
transitional state of a particular type defined according to the Type System. It
is used to instantiate concrete DynamicType objects."

**Repo:** `crates/types/src/dynamic/factory.rs::TypeBuilder`,
`crates/types/src/dynamic/factory.rs::build`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder` is the transitional state of one type with a model of its own; `build()` returns the type over a snapshot, later changes do not reach it.

### §7.5.2.9.1 Table 56 — Property annotation

**Spec:** §7.5.2.9.1, Table 56, p. 215–216 — "This property provides all
annotations that have previously been applied to this type with
apply_annotation."

**Repo:** `crates/types/src/dynamic/factory.rs::get_annotation_count`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::get_annotation_count()` and `get_annotation(index)`.

### §7.5.2.9.2 Table 56 — Property member

**Spec:** §7.5.2.9.2, Table 56, p. 215–216 — "This property contains a mapping
from the member ID of a member of this (aggregated) type to the member itself."

**Repo:** `crates/types/src/dynamic/factory.rs::all_members_by_id`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::all_members_by_id()` and `get_member(id)`.

### §7.5.2.9.3 Table 56 — Property member_by_name

**Spec:** §7.5.2.9.3, Table 56, p. 215–216 — "This property contains a mapping
from the name of a member of this type to the member itself."

**Repo:** `crates/types/src/dynamic/factory.rs::all_members_by_name`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::all_members_by_name()` and `get_member_by_name(name)`.

### §7.5.2.9.10 Table 56 — get_annotation

**Spec:** §7.5.2.9.10, Table 56, p. 215, 218 — "This operation returns the
annotation that corresponds to the specified index, if any."

**Repo:** `crates/types/src/dynamic/factory.rs::get_annotation`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::get_annotation(index)`.

### §7.5.2.9.11 Table 56 — get_annotation_count

**Spec:** §7.5.2.9.11, Table 56, p. 215, 218 — "This operation returns the
current number of annotations applied to the type."

**Repo:** `crates/types/src/dynamic/factory.rs::get_annotation_count`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::get_annotation_count()`.

### §7.5.2.9.8 Table 56 — get_descriptor

**Spec:** §7.5.2.9.8, Table 56, p. 215, 217 — "This operation provides a summary
of the state of this type."

**Repo:** `crates/types/src/dynamic/factory.rs::get_descriptor`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::get_descriptor()`.

### §7.5.2.9.12 Table 56 — get_kind

**Spec:** §7.5.2.9.12, Table 56, p. 215, 218 — "This convenience operation
indicates the kind of this type"

**Repo:** `crates/types/src/dynamic/factory.rs::get_kind`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_kind()`.

### §7.5.2.9.13 Table 56 — get_member

**Spec:** §7.5.2.9.13, Table 56, p. 215–216, 218 — "This operation returns the
member that corresponds to the specified member ID, if any."

**Repo:** `crates/types/src/dynamic/factory.rs::get_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_member(id)`.

### §7.5.2.9.14 Table 56 — get_member_by_index

**Spec:** §7.5.2.9.14, Table 56, p. 216, 218 — "This operation returns the
member that corresponds to the specified index, if any."

**Repo:** `crates/types/src/dynamic/factory.rs::get_member_by_index`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_member_by_index(index)`.

### §7.5.2.9.15 Table 56 — get_member_by_name

**Spec:** §7.5.2.9.15, Table 56, p. 216, 218 — "This operation returns the
member that corresponds to the specified name, if any."

**Repo:** `crates/types/src/dynamic/factory.rs::get_member_by_name`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_member_by_name(name)`.

### §7.5.2.9.16 Table 56 — get_member_count

**Spec:** §7.5.2.9.16, Table 56, p. 216, 218 — "This operation returns the
current number of members."

**Repo:** `crates/types/src/dynamic/factory.rs::get_member_count`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_member_count()`.

### §7.5.2.9.17 Table 56 — get_name

**Spec:** §7.5.2.9.17, Table 56, p. 216, 218 — "This convenience operation
provides the fully qualified name of this type."

**Repo:** `crates/types/src/dynamic/factory.rs::get_name`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_name()`.

### §7.5.2.9.9 Table 56 — equals

**Spec:** §7.5.2.9.9, Table 56, p. 215, 217 — "Two types shall be considered
equal if and only if all of their respective properties, as identified in Table
56 above, are equal."

**Repo:** `crates/types/src/dynamic/factory.rs::equals`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`,
`crates/types/tests/dynamic_type_builder_factory.rs::a_copy_carries_the_state_and_leaves_the_original_alone`

**Status:** done — `TypeBuilder::equals(&DynamicType)`: the type in progress equals the type.

### §7.5.2.9.4 Table 56 — add_member

**Spec:** §7.5.2.9.4, Table 56, p. 215, 217 — "Add a 'member' to this type [...]
For type kinds not given in that table, this operation shall fail with
RETCODE_PRECONDITION_NOT_MET. [...] If this argument is nil, the operation shall
fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::add_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`,
`crates/types/tests/dynamic_type_builder_factory.rs::add_member_follows_table_55`

**Status:** done — `TypeBuilder::add_member(&MemberDescriptor)` after Table 55; numbering without an id is maximum plus 1, the index shifts the others.

### §7.5.2.9.5 Table 56 — apply_annotation

**Spec:** §7.5.2.9.5, Table 56, p. 215, 217 — "Apply the given annotation to
this type. It shall subsequently appear in the annotation property. [...] If
this argument is not consistent, the operation shall fail with
RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::apply_annotation`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::apply_annotation(&AnnotationDescriptor)`: built-in annotations set the typed fields, a declared one is copied in with its declaration; an inconsistent descriptor is `RETCODE_BAD_PARAMETER`.

### §7.5.2.9.6 Table 56 — apply_annotation_to_member

**Spec:** §7.5.2.9.6, Table 56, p. 215, 217 — "Apply the given annotation to
this member. It shall subsequently appear in the annotation property of the
identified member."

**Repo:** `crates/types/src/dynamic/factory.rs::apply_annotation_to_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::apply_annotation_to_member(id, &AnnotationDescriptor)`; an id with no member is `RETCODE_BAD_PARAMETER`.

### §7.5.2.9.7 Table 56 — build: immutable snapshot

**Spec:** §7.5.2.9.7, Table 56, p. 215, 217 — "Create an immutable DynamicType
object containing a snapshot of this builder's current state. Subsequent changes
to this builder, if any, shall have no observable effect on the states of any
previously created DynamicTypes."

**Repo:** `crates/types/src/dynamic/builder.rs::finish` (l. 236);
`crates/types/src/dynamic/builder.rs::finish_as` (l. 244)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::equality_is_structural_and_not_by_pointer`,
`crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`

**Status:** done — `finish` consumes the builder; the model is immutable
afterwards.

---

## §7.5.2.10 DynamicDataFactory

### §7.5.2.10 DynamicDataFactory: logically a singleton

**Spec:** §7.5.2.10, p. 218 — "This class is logically a singleton [...] Its
'only' instance is the starting point for creating and deleting DynamicData and
objects".

**Repo:** `crates/types/src/dynamic/sample.rs::DynamicDataFactory`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — `DynamicDataFactory` has no state.

### §7.5.2.10.4 Table 57 — get_instance

**Spec:** §7.5.2.10.4, Table 57, p. 219 — "Return a DynamicDataFactory instance
that behaves like a singleton [...] It is legal to call this operation even
after delete_instance has been called."

**Repo:** `crates/types/src/dynamic/sample.rs::get_instance`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — `DynamicDataFactory::get_instance()`.

### §7.5.2.10.3 Table 57 — delete_instance

**Spec:** §7.5.2.10.3, Table 57, p. 219 — "Reclaim any resources associated with
the object(s) previously returned from get_instance."

**Repo:** `crates/types/src/dynamic/sample.rs::delete_instance`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sample_with_a_loan_out_is_not_deleted`

**Status:** done — `DynamicDataFactory::delete_instance()` succeeds.

### §7.5.2.10.1 Table 57 — create_data

**Spec:** §7.5.2.10.1, Table 57, p. 219 — "Create and return a new data sample.
All objects returned by this operation should eventually be deleted by calling
delete_data. Parameter type - The type of the sample to create."

**Repo:** `crates/types/src/dynamic/sample.rs::create_data`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — `create_data(&DynamicType)`: every non-optional member at its default (Table 9), optional members absent.

### §7.5.2.10.2 Table 57 — delete_data

**Spec:** §7.5.2.10.2, Table 57, p. 219 — "Dispose of a data sample, reclaiming
any associated resources."

**Repo:** `crates/types/src/dynamic/sample.rs::delete_data`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sample_with_a_loan_out_is_not_deleted`

**Status:** done — `delete_data(DynamicSample)`; a sample with a loan out is `RETCODE_PRECONDITION_NOT_MET`.

---

## §7.5.2.11 DynamicData

### §7.5.2.11 DynamicData: object of the type, reference semantics, equals

**Spec:** §7.5.2.11, p. 220 — "Each object of the DynamicData class represents a
corresponding object of the type represented by the DynamicData object's
DynamicType. DynamicData objects have reference semantics; however, there is an
equals operation".

**Repo:** `crates/types/src/dynamic/sample.rs::DynamicSample`,
`crates/types/src/dynamic/sample.rs::get_complex_value`,
`crates/types/src/dynamic/sample.rs::share`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_value_from_get_complex_value_is_the_value_in_the_sample`,
`crates/types/tests/dynamic_sample.rs::a_handle_refers_to_stored_values_and_says_when_it_does_not`,
`crates/types/tests/dynamic_sample.rs::elements_of_an_array_and_values_of_a_map_are_references`,
`crates/types/tests/dynamic_sample.rs::a_character_and_a_flag_are_copies`,
`crates/types/tests/dynamic_sample.rs::a_loan_is_a_reference`,
`crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — `DynamicSample` carries its `DynamicType` and has reference semantics: the value `get_complex_value` or `loan_value` returns is the value inside the sample, and a setter acts both ways; `clone` is the copy (§7.5.2.11.4), `share` another handle on the same sample, `equals` compares types and values after §7.5.2.11.5. A character of a string, a flag of a bitmask and an unselected member of a union have no storage and come as a copy and as `RETCODE_NO_DATA`; a handle whose value was removed reads and writes `RETCODE_NO_DATA`.

### §7.5.2.11.8 Table 58 — Property type: never nil

**Spec:** §7.5.2.11.8, Table 58, p. 220, 224 — "This property provides the type
that defines the values within this sample. Its value shall not be nil."

**Repo:** `crates/types/src/dynamic/sample.rs::type_`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — `DynamicSample::type_()` returns the type; there is no sample without one.

### §7.5.2.11.1 Table 58 — Property value: values by name, member ID or index

**Spec:** §7.5.2.11.1, Table 58, p. 220–221 — "Many of the properties and
operations defined by this class refer to values within the sample, which are
identified by name, member ID, or index."

**Repo:** `crates/types/src/dynamic/sample.rs::get_member_id_by_name`,
`crates/types/src/dynamic/sample.rs::get_member_id_at_index`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`,
`crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`

**Status:** done — The values of a sample by name, member id or index; which id is which value is fixed per kind in the head of `sample.rs`.

### §7.5.2.11.1 Values: aggregate: name, member ID or index

**Spec:** §7.5.2.11.1, p. 221 — "If this object is of an aggregated type, values
correspond to the type's members and can be accessed by name, member ID, or
index."

**Repo:** `crates/types/src/dynamic/sample.rs::get_member_id_at_index`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Aggregate: `get_member_id_by_name`, the id of the member, `get_member_id_at_index` over the members present.

### §7.5.2.11.1 Values: sequence, string: elements by index

**Spec:** §7.5.2.11.1, p. 221 — "If this object is of a sequence or string type,
values correspond to the elements of the collection. These elements must be
accessed by index; the mapping from index to member ID is unspecified."

**Repo:** `crates/types/src/dynamic/sample.rs::write_element`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::strings_keep_their_bound_and_their_width`

**Status:** done — Sequence, string: elements by index; the index one past the end appends, up to the bound.

### §7.5.2.11.1 Values: map: keys as strings, values by name

**Spec:** §7.5.2.11.1, p. 221 — "If this object is of a map type, values
correspond to the values of the map. Map keys are implicitly converted to
strings and can thus be used to look up map values by name."

**Repo:** `crates/types/src/dynamic/sample.rs::write_entry`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_map_is_reached_by_key`

**Status:** done — Map: `get_member_id_by_name(key)` returns the entry, or the index past the end that remembers the key for the next setter.

### §7.5.2.11.1 Values: array: flattened, by index

**Spec:** §7.5.2.11.1, p. 221 — "If the object is of an array type, values
correspond to the elements of the array. [...] If the array is
multi-dimensional, elements are accessed as if they were 'flattened' into a
single-dimensional array"

**Repo:** `crates/types/src/dynamic/sample.rs::flat_get`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_array_is_flattened`

**Status:** done — Array: flattened by index, a multi-dimensional one in the order of the IDL.

### §7.5.2.11.1 Values: bitmask: flags as Boolean by name or index

**Spec:** §7.5.2.11.1, p. 221 — "If the object is of a bitmask type, values
correspond to the flags within the bitmask and are all of Boolean type. Named
flags can be accessed using that name; any bit within the bound of the bitmask
may be accessed by its index."

**Repo:** `crates/types/src/dynamic/sample.rs::resolve`,
`crates/rmw-zerodds-shim/src/dynamic.rs::flag_name`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_bitmask_has_boolean_flags_and_reads_as_an_unsigned_integer`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_bitmask_flag_is_a_boolean_by_bit`

**Status:** done — Bitmask: flags as Boolean by name or bit; a bit that no flag names is `RETCODE_BAD_PARAMETER`, because a bitmask value holds the names of the flags that are set.

### §7.5.2.11.1 Values: enumeration, primitive type: the value itself

**Spec:** §7.5.2.11.1, p. 221 — "If the object is of an enumeration or primitive
type, it has no contained values. However, the value of the sample itself may be
indicated by 'name' using a nil or empty string, by 'ID' by passing
MEMBER_ID_INVALID, or by 'index' by passing index 0."

**Repo:** `crates/types/src/dynamic/sample.rs::resolve`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — Enumeration, primitive type: the value itself, `MEMBER_ID_INVALID`, nil as name, index 0.

### §7.5.2.11.1 Indices relative to the sample

**Spec:** §7.5.2.11.1, p. 222 — "Note that indices used here are always relative
to other values in a particular DynamicData object."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — explanation.

### §7.5.2.11.1 Table 58 — get_member_id_by_name

**Spec:** §7.5.2.11.1, Table 58, p. 220–221 — "get_member_id_by_name | MemberId
| name string<Char8,256>".

**Repo:** `crates/types/src/dynamic/sample.rs::get_member_id_by_name`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`,
`crates/types/tests/dynamic_sample.rs::a_map_is_reached_by_key`

**Status:** done — `get_member_id_by_name`.

### §7.5.2.11.1 Table 58 — get_member_id_at_index

**Spec:** §7.5.2.11.1, Table 58, p. 220–221 — "get_member_id_at_index | MemberId
| index UInt32".

**Repo:** `crates/types/src/dynamic/sample.rs::get_member_id_at_index`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`,
`crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`

**Status:** done — `get_member_id_at_index`; an index past the end (except by one for variable-length collections) is `RETCODE_BAD_PARAMETER`.

### §7.5.2.11.2 Table 58 — Property descriptor per value

**Spec:** §7.5.2.11.2, Table 58, p. 220, 222 — "This property shall contain a
descriptor for each value in this object, identified by the member ID."

**Repo:** `crates/types/src/dynamic/sample.rs::get_descriptor`,
`crates/types/src/dynamic/sample.rs::set_descriptor`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_descriptor_of_a_value_is_its_types`

**Status:** done — `get_descriptor(id)` returns the descriptor of the member or, for a collection, one with the element type; `set_descriptor` succeeds for the same descriptor, because the type is frozen.

### §7.5.2.11.3 Table 58 — clear_value

**Spec:** §7.5.2.11.3, Table 58, p. 221–222 — "clear_value | ReturnCode_t | id
MemberId".

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_optional_member_is_set_and_removed`,
`crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::an_array_is_flattened`

**Status:** done — `clear_value(id)` by the kind of the sample.

### §7.5.2.11.3 Table 58 — clear_all_values

**Spec:** §7.5.2.11.3, Table 58, p. 221–222 — "The clear_all_members takes the
above action for each value in turn."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_all_values`

**Tests:** `crates/types/tests/dynamic_sample.rs::clearing_all_values_and_the_nonkey_values`

**Status:** done — `clear_all_values()`: every value at its default (Table 9).

### §7.5.2.11.3 Table 58 — clear_nonkey_values: key members keep their value

**Spec:** §7.5.2.11.3, Table 58, p. 221–222 — "The clear_nonkey_value operation
has exactly the same effect as clear_all_values with one exception: the values
of key fields of aggregated types retain their values."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_nonkey_values`,
`crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_clear_nonkey_values`

**Tests:** `crates/types/tests/dynamic_sample.rs::clearing_all_values_and_the_nonkey_values`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::clear_nonkey_values_keeps_the_key_members`

**Status:** done — `clear_nonkey_values()` keeps the key members; the ROS 2 interface does the same (`zerodds_dyn_data_clear_nonkey_values`).

### §7.5.2.11.3 Clearing: aggregate: remove an optional member, otherwise default

**Spec:** §7.5.2.11.3, p. 222 — "If this sample is of an aggregated type, and
the indicated member is optional, remove it. If the indicated member is not
optional, set it to its default value."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`,
`crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_optional_member_is_set_and_removed`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::clear_value_removes_an_optional_member_and_a_sequence_element`

**Status:** done — Aggregate: an optional member is removed, any other takes its default.

### §7.5.2.11.3 Clearing: variable-length collection: remove the element, the rest moves down

**Spec:** §7.5.2.11.3, p. 222 — "If this sample is of a variable-length
collection type, remove the indicated element, shifting any subsequent elements
to the next-lowest index."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`,
`crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::a_map_is_reached_by_key`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::clear_value_removes_an_optional_member_and_a_sequence_element`

**Status:** done — String, sequence, map: the element is removed and the rest move down.

### §7.5.2.11.3 Clearing: array: element to its default

**Spec:** §7.5.2.11.3, p. 222 — "If the sample is of an array type, set the
indicated element to its default value."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_array_is_flattened`

**Status:** done — Array: the element takes its default, the size stays.

### §7.5.2.11.3 Clearing: bitmask: clear the bit

**Spec:** §7.5.2.11.3, p. 222 — "If the sample is of a bitmask type, clear the
indicated bit."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`,
`crates/rmw-zerodds-shim/src/dynamic.rs::write_slot`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_bitmask_has_boolean_flags_and_reads_as_an_unsigned_integer`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_bitmask_flag_is_a_boolean_by_bit`

**Status:** done — Bitmask: the bit is cleared.

### §7.5.2.11.3 Clearing: enumeration: first literal

**Spec:** §7.5.2.11.3, p. 222 — "If the sample is of an enumerated type, set it
to the first value of the enumerated type."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_enumeration_is_a_signed_integer`

**Status:** done — Enumeration: the first value.

### §7.5.2.11.3 Clearing: primitive type: default

**Spec:** §7.5.2.11.3, p. 222 — "If the sample is of a primitive type, set it to
its default value."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — Primitive type: its default.

### §7.5.2.11.4 Table 58 — clone

**Spec:** §7.5.2.11.4, Table 58, p. 221, 223 — "Create and return a new data
sample with the same contents as this one. A comparison of this object and the
clone using equals immediately following this call will return true."

**Repo:** `crates/types/src/dynamic/data.rs::DynamicData` (l. 81, `Clone`);
`crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_clone` (l. 1269)

**Tests:** `crates/rmw-zerodds-shim/src/dynamic.rs::tests::clone_and_equals_agree`

**Status:** done

### §7.5.2.11.5 Table 58 — equals

**Spec:** §7.5.2.11.5, Table 58, p. 220, 223 — "Their respective type
definitions are equal. All contained values are equal and occur in the same
order. [...] Members shall be compared without regard to their order. One of the
samples may omit a non-optional member that is present in the other if that
member takes its default value in the latter sample."

**Repo:** `crates/types/src/dynamic/sample.rs::equals`,
`crates/types/src/dynamic/data_equals.rs::values_equal`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_optional_member_is_set_and_removed`,
`crates/types/tests/dynamic_corpus.rs::a_dynamic_data_writes_the_octets_a_json_value_writes`

**Status:** done — `equals`: equal types, members without regard to their order, a left-out non-optional member equal to its default (`DynamicType::values_equal`).

### §7.5.2.11.6 Table 58 — get_item_count

**Spec:** §7.5.2.11.6, Table 58, p. 220, 223 — "If the object is of a collection
type, return the number of elements [...] bitmask type, return the number of
named flags that are currently set [...] union type [...] two if the
discriminator value selects a member and one otherwise. [...] primitive or
enumerated type, it is atomic: return one."

**Repo:** `crates/types/src/dynamic/sample.rs::get_item_count`,
`crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_item_count`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`,
`crates/types/tests/dynamic_sample.rs::an_array_is_flattened`,
`crates/types/tests/dynamic_sample.rs::a_union_member_is_selected_by_being_set`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::an_atomic_value_has_one_item`

**Status:** done — `get_item_count`: collection (array: product of the bounds), flags set, members of an aggregate, two for a union with a selected member, one for an atomic value.

### §7.5.2.11.7 Table 58 — loan_value: at most one outstanding loan

**Spec:** §7.5.2.11.7, Table 58, p. 221, 223–224 — "A given DynamicData object
may support only a single outstanding loan at a time. [...] If an application
violates this constraint, the loan operation shall return a nil value. A loan
operation shall also return a nil value if the indicated value does not exist."

**Repo:** `crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_loan` (l. 2396)

**Tests:** `crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_loan_writes_back_at_its_own_origin`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_second_loan_and_a_foreign_return_are_refused`

**Status:** done — `zerodds_dyn_data_loan` returns nil while a loan is out.

### §7.5.2.11.7 loan_value: a second open loan is refused

**Spec:** §7.5.2.11.7, p. 223 — "If an application violates this constraint, the
loan operation shall return a nil value."

**Repo:** —

**Tests:** `crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_second_loan_and_a_foreign_return_are_refused`

**Status:** done — A second open loan is refused.

### §7.5.2.11.7 Table 58 — return_loaned_value

**Spec:** §7.5.2.11.7, Table 58, p. 221, 224 — "The return_loaned_value
operation shall return RETCODE_PRECONDITION_NOT_MET if the provided sample
object does not represent an outstanding loan from the sample on which the
operation is invoked."

**Repo:** `crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_return_loan` (l. 2420)

**Tests:** `crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_loan_writes_back_at_its_own_origin`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_second_loan_and_a_foreign_return_are_refused`

**Status:** done — `zerodds_dyn_data_return_loan` refuses a value that is not the outstanding loan of that sample (a token that no other sample shares).

### §7.5.2.11.9 PSM: methods per type by the IDL of Annex C

**Spec:** §7.5.2.11.9, p. 224 — "The programming language-specific APIs for the
Dynamic Type and Dynamic Data classes and their companion classes shall be based
on the following IDL definitions, transformed according to the IDL language
mapping described above".

**Repo:** `crates/types/src/dynamic/sample.rs::DynamicSample`

**Tests:** `crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`,
`crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`

**Status:** done — The getters and setters of Annex C as methods of `DynamicSample`, one per type (`get_int32_value`, `set_int32_value`, … `get_complex_value`).

### §7.5.2.11.9 Expansions: one method per primitive type

**Spec:** §7.5.2.11.9, p. 224 — "Each primitive type has its own expansion."

**Repo:** `crates/types/src/dynamic/sample.rs::integer_accessors`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`

**Status:** done — One getter and one setter per primitive type.

### §7.5.2.11.9 Expansions: string and wstring

**Spec:** §7.5.2.11.9, p. 224 — "Strings of Char8 and Char16 elements have their
own expansions qualified by 'string' and 'wstring' respectively."

**Repo:** `crates/types/src/dynamic/sample.rs::get_string_value`,
`crates/types/src/dynamic/sample.rs::set_string_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::strings_keep_their_bound_and_their_width`

**Status:** done — `get_string_value`, `set_string_value`, `get_wstring_value`, `set_wstring_value`; the bound of the string applies.

### §7.5.2.11.9 Expansions: enumeration through signed integers

**Spec:** §7.5.2.11.9, p. 224 — "Enumerated types shall be implicitly converted
to any signed integer type having at least as many bits as the enumerated type's
@bit_bound."

**Repo:** `crates/types/src/dynamic/sample.rs::effective_kind`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_enumeration_is_a_signed_integer`

**Status:** done — An enumeration reads and sets as a signed integer at least as wide as its `@bit_bound` (default 32); a value with no enumerator is refused.

### §7.5.2.11.9 Expansions: bitmask through unsigned integers

**Spec:** §7.5.2.11.9, p. 224 — "Bitmasks shall be implicitly converted to any
unsigned integer type having at least as many bits as the bitmask's @bit_bound."

**Repo:** `crates/types/src/dynamic/sample.rs::effective_kind`,
`crates/types/src/dynamic/sample.rs::store_integer`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_bitmask_has_boolean_flags_and_reads_as_an_unsigned_integer`

**Status:** done — A bitmask reads and sets as an unsigned integer at least as wide as its bound; a bit that no flag names is refused.

### §7.5.2.11.9 Expansions: alias through the base type

**Spec:** §7.5.2.11.9, p. 224 — "Alias types shall be implicitly converted to
their ultimate base type and are thus accessible through the methods appropriate
for that type."

**Repo:** `crates/types/src/dynamic/sample.rs::resolve`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`

**Status:** done — An alias is read and set through its base type (`resolve_alias`).

### §7.5.2.11.9 Expansions: sequences of primitive types in the plural, also for arrays

**Spec:** §7.5.2.11.9, p. 224 — "Sequences of primitive types and strings have
their own expansions in which the name of the property has been made plural.
Arrays shall also be accessible through these methods."

**Repo:** `crates/types/src/dynamic/sample.rs::get_integers`,
`crates/types/src/dynamic/sample.rs::set_values`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::an_array_is_flattened`

**Status:** done — The plural forms (`get_int32_values`, `set_int32_values`, …, `get_string_values`) for sequences and arrays, with the promotions of the elements.

### §7.5.2.11.9 Expansions: complex for the remaining cases

**Spec:** §7.5.2.11.9, p. 224 — "Expansions that operate on DynamicData objects,
qualified by 'complex,' catch the remaining cases"

**Repo:** `crates/types/src/dynamic/sample.rs::get_complex_value`,
`crates/types/src/dynamic/sample.rs::set_complex_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_loan_is_taken_and_returned`,
`crates/types/tests/dynamic_sample.rs::a_bitmask_has_boolean_flags_and_reads_as_an_unsigned_integer`

**Status:** done — `get_complex_value` and `set_complex_value` for all other cases; the type has to be equal.

### §7.5.2.11.9 Expansions: setters append to resizable collections

**Spec:** §7.5.2.11.9, p. 224 — "If a DynamicData object represents an object of
a resizable collection type (string, sequence, or map), these setters may also
be used to append new elements to the collection."

**Repo:** `crates/types/src/dynamic/sample.rs::write_element`,
`crates/types/src/dynamic/sample.rs::write_entry`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::a_map_is_reached_by_key`,
`crates/types/tests/dynamic_sample.rs::strings_keep_their_bound_and_their_width`

**Status:** done — A setter appends to a string, a sequence or a map when the id is the index one past the end; the bound applies.

### §7.5.2.11.9 New elements: ID through get_member_id_at_index or get_member_id_by_name

**Spec:** §7.5.2.11.9, p. 225 — "For a string or sequence type, use
get_member_id_at_index to obtain an ID for the index one greater than the
current length. For a map type, use get_member_id_by_name to obtain an ID for
the new map key."

**Repo:** `crates/types/src/dynamic/sample.rs::get_member_id_at_index`,
`crates/types/src/dynamic/sample.rs::get_member_id_by_name`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::a_map_is_reached_by_key`

**Status:** done — The id of a new element comes from `get_member_id_at_index` (sequence, string) or `get_member_id_by_name` (map).

### §7.5.2.11.9 Promotion Int8 → Int16, Int32, Int64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Int8 → Int16, Int32, Int64, Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of Int8 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion Int16 → Int32, Int64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Int16 → Int32, Int64, Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of Int16 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion Int32 → Int64, Float64, Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Int32 → Int64, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of Int32 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion Int64 → Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Int64 → Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of Int64 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion UInt8 → Int16,Int32, Int64, UInt16, UInt32, UInt64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] UInt8 → Int16,Int32, Int64, UInt16, UInt32, UInt64, Float32, Float64,
Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of UInt8 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion UInt16 → Int32, Int64, UInt32, UInt64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] UInt16 → Int32, Int64, UInt32, UInt64, Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of UInt16 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion UInt32 → Int64, UInt64, Float64, Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] UInt32 → Int64, UInt64, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of UInt32 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion UInt64 → Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] UInt64 → Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of UInt64 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion Float32 → Float64, Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Float32 → Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of Float32 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion Float64 → Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Float64 → Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of Float64 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion Float128 → (none)

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Float128 → (none)"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Float128 has no promotion; a getter or setter of Float128 works only on a Float128 member (and the setter on no other).

### §7.5.2.11.9 Promotion Char8 → Char16, Int16, Int32, Int64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Char8 → Char16, Int16, Int32, Int64, Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of Char8 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion Char16 → Int32, Int64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Char16 → Int32, Int64, Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of Char16 after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion Byte → (any)

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Byte → (any)"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of Byte after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

### §7.5.2.11.9 Promotion Boolean → Int8, Int16, Int32, Int64, UInt8, UInt16, UInt32, UInt64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, p. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Boolean → Int8, Int16, Int32, Int64, UInt8, UInt16, UInt32, UInt64,
Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — The promotion of Boolean after the list of the spec: a getter of the larger type reads, a setter of the smaller type sets; the other way round is `RETCODE_BAD_PARAMETER`. All 15 by 15 pairs are checked against the list.

---

## §7.6 Use of the Type System by DDS

### §7.6.1 Topic Model: topic on the network and in the application

**Spec:** §7.6.1, p. 225–226 — "A DDS topic exists in two senses of the word: 1.
On the network, with respect to interoperability [...] 2. In application code,
with respect to portability".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition of terms.

### §7.6.1 One topic on the network with several types, check of consistent types at matching

**Spec:** §7.6.1, p. 226 — "On the network, a given topic is associated with one
or more types. [...] the Service evaluates the two endpoints to make sure that
they specify consistent types (see Clause 7.6.3.4.2 [...]) and compatible QoS".

**Repo:** `crates/dcps/src/runtime.rs::decide_type_consistency` (l. 18903)

**Tests:** `crates/dcps/src/runtime.rs::tests::xtypes_step2_falls_back_to_the_type_name_for_a_peer_without_type_information`,
`crates/dcps/tests/xtypes_aware_match.rs::appendable_compatible_matches_at_match_time`

**Status:** done

### §7.6.1 Generic services without type knowledge

**Spec:** §7.6.1, p. 226 — "Generic services (e.g., logger, monitor) may
discover a topic associated with one or more types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — use case.

### §7.6.2 Only structures and unions as topic types

**Spec:** §7.6.2, p. 226 — "The only types that may be associated with a DDS
Topic are the Aggregated Types defined in 7.2.2.4.4, that is, Structure types
[...] and Union types".

**Repo:** —

**Tests:** `crates/zerodds-build/tests/dds_type.rs::an_enumeration_and_a_bitset_are_no_topic_types`

**Status:** done — `zerodds-build` generates no `DdsType` implementation for an `enum` and a `bitset`; structures and unions remain topic types.

### §7.6.2 Any type may appear nested in the topic type

**Spec:** §7.6.2, p. 226 — "Any type in the type system may appear nested within
the (Aggregated Type) associated with a Topic."

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (l. 931)

**Tests:** `crates/wire-probe/tests/interop_corpus.rs::every_case_produces_the_bytes_its_readme_records_in_all_four_modes`

**Status:** done

### §7.6.3 Discovery and matching: three extensions

**Spec:** §7.6.3, p. 226–227 — "Data Representation [...] Discovery-Time Data
Typing [...] Type Consistency Enforcement".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure.

---

## §7.6.3.1 Data Representation QoS Policy

### §7.6.3.1 DataRepresentationQosPolicy for the negotiation

**Spec:** §7.6.3.1, p. 227 — "This negotiation shall occur based on a new QoS
policy: DataRepresentationQosPolicy."

**Repo:** `crates/dcps/src/qos.rs::DataWriterQos` (l. 26, field `data_representation` l. 73);
`crates/dcps/src/qos.rs::DataReaderQos` (l. 114, field l. 152)

**Tests:** `crates/dcps/src/qos.rs::data_representation_tests::data_representation_defaults_none_and_settable`

**Status:** done

### §7.6.3.1.1 Identifier: signed two-octet number, negative values for vendors

**Spec:** §7.6.3.1.1, p. 227 — "Each data representation shall be identified by
a two-byte signed integer value [...] the negative values shall be reserved for
definition by DDS implementations."

**Repo:** `crates/types/src/qos.rs::DataRepresentationId` (l. 174);
`crates/rtps/src/publication_data.rs::negotiate` (l. 152)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::publication_data_roundtrip_preserves_data_representation`

**Status:** done — `i16`; an unknown identifier is read and not accepted in the
negotiation.

### §7.6.3.1.1 XCDR = 0, XML = 1, XCDR2 = 2

**Spec:** §7.6.3.1.1, p. 227 — "XCDR [...] takes the value 0. XML [...] takes
the value 1. XCDR2 [...] takes the value 2."

**Repo:** `crates/types/src/qos.rs::DataRepresentationId` (l. 174);
`crates/rtps/src/publication_data.rs::DEFAULT_OFFER` (l. 108)

**Tests:** `crates/dcps/src/runtime.rs::tests::data_repr_offer_str_uses_spec_ids`

**Status:** done

### §7.6.3.1.1 Policy on Topic, DataReader and DataWriter

**Spec:** §7.6.3.1.1, p. 227 — "Each Topic, DataReader and DataWriter shall have
a QoS policy DataRepresentationQosPolicy. This policy shall contain a list of
representation identifiers."

**Repo:** `crates/dcps/src/qos.rs::DataWriterQos`;
`crates/dcps/src/qos.rs::DataReaderQos`;
`crates/dcps/src/qos.rs::TopicQos`;
`crates/zerodds-c-api/src/qos_ffi.rs::ZeroDdsDataRepresentationQosPolicy`;
`crates/cpp/include/dds/core/policy/CorePolicy.hpp::DataRepresentation`;
`crates/rtps/src/topic_data.rs::TopicBuiltinTopicData`

**Tests:** `crates/dcps/src/qos.rs::data_representation_tests::data_representation_defaults_none_and_settable`,
`crates/dcps/tests/topic_data_representation.rs::the_topic_qos_names_a_representation_for_its_writers_and_readers`,
`crates/zerodds-c-api/tests/qos_data_representation_c.rs::a_set_list_is_the_list_and_an_empty_one_is_kept_empty`,
`crates/zerodds-c-api/tests/qos_data_representation_c.rs::the_entities_hand_the_representation_back`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_data_representation_and_type_consistency_in_the_qos_sets`

**Status:** done — Topic, DataWriter and DataReader have the policy in Rust, in the C structs (`ZeroDdsTopicQos`, `ZeroDdsDataWriterQos`, `ZeroDdsDataReaderQos` end in `ZeroDdsDataRepresentationQosPolicy`: `is_set`, `count`, `ids[8]`; a struct of zeros is the default), in the Java PSM and in the QoS sets of the C++ PSM (`dds::topic::qos::TopicQos`, `dds::pub::qos::DataWriterQos`, `dds::sub::qos::DataReaderQos`; `DataRepresentation::is_set()` tells whether the application gave a list; the C++ PSM refuses a list of more than eight identifiers with `InvalidArgumentError`). The first element of the topic applies to writers, the whole list to readers; it cannot be changed after enable. The built-in topic data carry the field.

### §7.6.3.1.1 DataRepresentation and TypeConsistencyEnforcement in the QoS sets of the C++ PSM

**Spec:** §7.6.3.1.1, p. 227 — "Each Topic, DataReader and DataWriter shall have a QoS policy DataRepresentationQosPolicy"; Annex D, p. 329–331.

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::DataRepresentation`;
`crates/cpp/include/dds/core/policy/CorePolicy.hpp::TypeConsistencyEnforcement`;
`crates/cpp/include/dds/core/detail/policy_bridge.hpp`;
`crates/cpp/include/dds/core/qos_bridge.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_data_representation_and_type_consistency_in_the_qos_sets`

**Status:** done — The type lists of `dds::topic::qos::TopicQos`, `dds::pub::qos::DataWriterQos` and `dds::sub::qos::DataReaderQos` take `DataRepresentation`, that of `DataReaderQos` also `TypeConsistencyEnforcement`; `qos_bridge.hpp` passes both to the C ABI and reads them back. `TypeConsistencyEnforcement` has the five flags of Annex D (`ignore_sequence_bounds`, `ignore_string_bounds`, `ignore_member_names`, `prevent_type_widening`, `force_type_validation`) with the defaults of §7.6.3.4.1. The PSM class has four kinds, Annex D two (`DISALLOW_TYPE_COERCION`, `ALLOW_TYPE_COERCION`): `ASSIGNABLE_TYPE_CONSISTENCY` is `ALLOW_TYPE_COERCION`, the other three are `DISALLOW_TYPE_COERCION`, and `DISALLOW_TYPE_COERCION` is read back as `EXACT_TYPE_TYPE_CONSISTENCY`. Annex D has the policy on neither Topic nor DataWriter (`TypeConsistencyEnforcement` only in `DataReaderQos`).

### §7.6.3.1.1 TopicQos carries the DataRepresentationQosPolicy

**Spec:** §7.6.3.1.1, p. 228 — "When representations are specified in the
TopicQos, the first element of the sequence applies to writers of the Topic, and
the whole sequence applies to readers of the Topic."

**Repo:** —

**Tests:** `crates/dcps/tests/topic_data_representation.rs::the_topic_qos_names_a_representation_for_its_writers_and_readers`,
`crates/dcps/tests/topic_data_representation.rs::the_default_topic_leaves_the_default_representation`

**Status:** done — `TopicQos::data_representation`; `copy_from_topic_qos` takes it over for writers and readers.

### §7.6.3.1.1 Request-offer, not changeable after enable

**Spec:** §7.6.3.1.1, p. 227, 229 — "This policy has request-offer semantics,
and its value cannot be changed after the entity in question has been enabled
[...] The DataRepresentationQosPolicy shall not be changeable after its
corresponding Entity has been enabled."

**Repo:** `crates/dcps/src/qos.rs::immutable_writer_change` (l. 206);
`crates/dcps/src/qos.rs::immutable_reader_change` (l. 241)

**Tests:** `crates/dcps/src/entity.rs::tests::immutable_if_enabled_returns_correct_error`,
`crates/dcps/tests/topic_data_representation.rs::an_enabled_topic_refuses_a_new_representation`

**Status:** done — Request-offer; after enable the policy cannot be changed on writer, reader and topic.

### §7.6.3.1.1 A writer offers one representation, further elements without meaning

**Spec:** §7.6.3.1.1, p. 227–228 — "Writers offer a single representation. A
writer will use its offered policy to communicate with its matched readers.
[...] this specification does not specify any meaning for the representation
identifiers after the first, and implementations may ignore them."

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (l. 12220)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/sedp_data_representation_pid.rs::default_offer_is_xcdr1_alone_and_both_is_a_setting`

**Status:** done — the writer writes in the first offered representation and
matches only readers that accept it. The default offers `[XCDR1]` (§7.6.3.1.1); `[XCDR1, XCDR2]` is the setting `wire.data_representation_offer`.

### §7.6.3.1.1 Writers from XTYPES 1.2: version 2, version 1 optional and configurable at run time

**Spec:** §7.6.3.1.1, p. 228 — "Shall generate or include run-code that can
serialize using version 2 encodings. Optionally may generate or include run-code
that can serialize using version 1 encodings. In this case, they shall offer the
means to configure at run-time the encoding version used by the DataWriter".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728);
`crates/zerodds-build/src/lib.rs::encoding_for` (l. 983)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_offer_user_override_xcdr2_only`

**Status:** done — through `DataWriterQos::data_representation` and the runtime
configuration.

### §7.6.3.1.1 Writers and readers up to XTYPES 1.1 do not announce XCDR2

**Spec:** §7.6.3.1.1, p. 228 — "Writers belonging to implementations of XTYPES
version 1.1 or earlier shall not announce the XCDR2 representation identifier."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — concerns older implementations.

### §7.6.3.1.1 Readers from XTYPES 1.2: read version 2, request XCDR2, XCDR optional


**Spec:** §7.6.3.1.1, p. 228 — "Shall generate or include run-time code that can
deserialize version 2 encodings. Shall request XCDR2 encoding. Optionally may
[...] deserialize version 1 encodings. In this case they shall also request XCDR
encoding in addition to XCDR2 encoding."

**Repo:** `crates/dcps/src/runtime.rs::reader_accept_repr`;
`crates/dcps/src/runtime.rs::wire_reader_to_remote_writer`

**Tests:** `crates/dcps/src/runtime.rs::tests::reader_accept_repr_requests_xcdr2_and_xcdr`,
`crates/dcps/src/subscriber.rs::tests::decode_for_encap_dispatches_on_representation_and_endianness`

**Status:** done — The reader announces `[XCDR2, XCDR1]` without a setting: it reads both versions (the read paths are tested) and requests XCDR2 and, in addition, XCDR. An element of the configured offer that is neither (XML) follows. The offer `wire.data_representation_offer` (default `[XCDR1]`) is the writers'. The default of the policy is the empty list, which is `[XCDR]`; the clause for readers asks for more, and a reader at the default follows the clause (see "Contradictions within the specification" in the findings file).

### §7.6.3.1.1 XML readers: valid and well-formed documents

**Spec:** §7.6.3.1.1, p. 228 — "Readers requesting the XML Data Representation
shall be prepared to receive either valid or merely well-formed XML documents."

**Repo:** `crates/dcps/src/xml_data.rs::from_xml`,
`crates/dcps/src/runtime.rs::xml_sample_as_cdr`

**Tests:** `crates/dcps/src/xml_data.rs::tests::a_value_is_written_as_its_document_and_read_back`,
`crates/dcps/src/xml_data.rs::tests::entities_and_prefixes_are_read`,
`crates/dcps/tests/dynamic_data_writer_reader.rs::linux_e2e::dynamic_data_goes_over_xml`

**Status:** done — A reader that requests XML (`DataRepresentationQosPolicy` with `XML`, `wire.data_representation_offer=XML`) reads a document in the Valid variant (with the namespace `ddstype://…`, with a prefix) and in the Well-formed variant (without a namespace); prefixes and namespace declarations are skipped, comments and the XML declaration too. The writer sends the Well-formed variant, because the receiver infers the namespace (§7.4.4.2).

### §7.6.3.1.1 Compatible when the writer's representation is in the reader's list

**Spec:** §7.6.3.1.1, p. 228 — "If a writer's offered representation is
contained within a reader's sequence, the offer satisfies the request and the
policies are compatible. Otherwise, they are incompatible."

**Repo:** `crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (l. 12220);
`crates/dcps/src/runtime.rs::wire_reader_to_remote_writer` (l. 12732);
`crates/rtps/src/publication_data.rs::negotiate` (l. 152)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_match_mode_default_is_strict`,
`crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_match_mode_tolerant_is_selectable`,
`crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_match_mode_user_strict`

**Status:** done — The writer requires its representation to be in the reader's list; the local reader matches by default (`DataRepMatchMode::Strict`) only a writer whose representation is in its own list. The setting `wire.data_representation_match: tolerant` gives the old behaviour (an overlap suffices) back for peers that send so.

### §7.6.3.1.1 The reader matches only on a contained representation, the overlap is a setting

**Spec:** §7.6.3.1.1, p. 228 — "Otherwise, they are incompatible."

**Repo:** —

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_match_mode_default_is_strict`,
`crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_match_mode_tolerant_is_selectable`

**Status:** done — By default the reader matches only if the representation of the writer is in its list; the overlap is the named setting `wire.data_representation_match: tolerant`.

### §7.6.3.1.1 Default: empty list, equal to [XCDR]

**Spec:** §7.6.3.1.1, p. 228 — "The default value of the
DataRepresentationQosPolicy shall be an empty list of preferences. An empty list
of preferences shall be taken to be equivalent to a list containing the single
element XCDR."

**Repo:** `crates/types/src/qos.rs::negotiate_representation`;
`crates/rtps/src/publication_data.rs::negotiate`;
`crates/dcps/src/qos.rs::effective_data_representation`

**Tests:** `crates/types/src/qos.rs::tests::an_empty_list_of_preferences_is_the_single_element_xcdr`,
`crates/dcps/src/publisher.rs::tests::create_datawriter_rejects_invalid_data_representation_offers`,
`crates/dcps/src/publisher.rs::tests::qos_emit_xcdr1_only_for_explicit_xcdr1`,
`crates/zerodds-c-api/tests/qos_data_representation_c.rs::a_zeroed_qos_leaves_the_representation_at_its_default`

**Status:** done — An empty list is `[XCDR]` in matching and in the offer; `create_datawriter` accepts an explicitly empty list and the writer then encodes XCDR1. The policy stays `None` by default and the runtime resolves it to its own offer, whose first element is XCDR (`DEFAULT_OFFER = [XCDR1]`, setting `wire.data_representation_offer`); elements after the first have no meaning according to §7.6.3.1.1.

### §7.6.3.1.1 Table 59 — Writer XCDR, Reader XCDR: Compatible

**Spec:** §7.6.3.1.1, Table 59, p. 230 — "XCDR | XCDR | Compatible."

**Repo:** `crates/rtps/src/publication_data.rs::negotiate` (l. 152);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (l. 12220)

**Tests:** `crates/types/src/qos.rs::tests::an_empty_list_of_preferences_is_the_single_element_xcdr`,
`crates/discovery/tests/representation_and_type_information.rs::negotiation_matches_xcdr2_when_both_support`

**Status:** done

### §7.6.3.1.1 Table 59 — Writer XCDR, Reader XCDR and XCDR2: Compatible

**Spec:** §7.6.3.1.1, Table 59, p. 230 — "XCDR | XCDR and XCDR2 | Compatible."

**Repo:** `crates/rtps/src/publication_data.rs::negotiate` (l. 152);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (l. 12220)

**Tests:** `crates/types/src/qos.rs::tests::an_empty_list_of_preferences_is_the_single_element_xcdr`,
`crates/discovery/tests/representation_and_type_information.rs::negotiation_matches_xcdr2_when_both_support`

**Status:** done

### §7.6.3.1.1 Table 59 — Writer XCDR2, Reader XCDR: Not Compatible

**Spec:** §7.6.3.1.1, Table 59, p. 230 — "XCDR2 | XCDR | Not Compatible."

**Repo:** `crates/rtps/src/publication_data.rs::negotiate` (l. 152);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (l. 12220)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::strict_writer_xcdr2_first_reader_xcdr1_only_no_match`,
`crates/dcps/tests/sedp_data_representation_pid.rs::tolerant_writer_xcdr2_first_reader_xcdr1_only_falls_back`,
`crates/types/src/qos.rs::tests::an_empty_list_of_preferences_is_the_single_element_xcdr`

**Status:** done — Writer `[XCDR2]` against reader `[XCDR]` does not match, on both sides; a reader `[XCDR1]` and a writer `[XCDR2, XCDR1]` do not match by default, under `tolerant` they do.

### §7.6.3.1.1 Table 59 — Writer XCDR2, Reader XCDR and XCDR2: Compatible

**Spec:** §7.6.3.1.1, Table 59, p. 230 — "XCDR2 | XCDR and XCDR2 | Compatible."

**Repo:** `crates/rtps/src/publication_data.rs::negotiate` (l. 152);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (l. 12220)

**Tests:** `crates/types/src/qos.rs::tests::an_empty_list_of_preferences_is_the_single_element_xcdr`,
`crates/discovery/tests/representation_and_type_information.rs::negotiation_matches_xcdr2_when_both_support`

**Status:** done

### §7.6.3.1.2 Encapsulation identifiers 0xC0 to 0xFF for vendors

**Spec:** §7.6.3.1.2, p. 231 — "encapsulation identifiers where the first byte
is in the range 0xC0 to 0xFF (both included) shall be reserved for definition by
DDS implementations and shall be interpreted based on the RTPS vendor ID."

**Repo:** `crates/dcps/src/runtime.rs::user_encap_body` (l. 20061)

**Tests:** `crates/dcps/src/runtime.rs::tests::strip_user_encap_rejects_unknown_scheme`

**Status:** done — ZeroDDS defines no identifier of its own; an identifier whose
first octet is not 0 is refused.

### §7.6.3.1.2 XML: {0x00, 0x04}

**Spec:** §7.6.3.1.2, p. 231 — "an additional encapsulation identifier
corresponding to the XML Data Representation: XML, with the value {0x00, 0x04}."

**Repo:** `crates/dcps/src/runtime.rs::check_emittable_offer`,
`crates/dcps/src/runtime.rs::parse_data_repr_offer_str`,
`crates/dcps/src/xml_data.rs::is_xml_payload`,
`crates/dcps/src/publisher.rs::validate_writer_data_representation`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_writer_registered_with_xml_first_is_accepted`,
`crates/dcps/src/runtime.rs::tests::data_repr_offer_str_uses_spec_ids`,
`crates/dcps/src/runtime.rs::tests::an_xml_sample_reaches_the_reader_as_its_cdr_body`,
`crates/dcps/src/publisher.rs::tests::create_datawriter_rejects_invalid_data_representation_offers`

**Status:** done — `user_encap_body` does not take `{0x00, 0x04}` for CDR; `xml_data::is_xml_payload` recognises it, `delivered_to_user_sample` reads the document through `xml_sample_as_cdr`. A writer whose offer starts with XML is accepted (`check_emittable_offer`, `validate_writer_data_representation`, `parse_data_repr_offer_str` with `XML`) and writes `{0x00, 0x04}` with the two option octets (the number of padding bytes in the two low bits, §7.6.3.1.2).

### §7.6.3.1.2 Six identifiers for PLAIN_CDR2, DELIMITED_CDR, PL_CDR2

**Spec:** §7.6.3.1.2, p. 231 — "Version 1.2 of this specification adds six
additional encapsulation identifiers corresponding to PLAIN_CDR2, DELIMITED_CDR,
and PL_CDR2 each with big endian or little endian encoding".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728);
`crates/wire-codec/src/value.rs::encapsulation_header` (l. 527)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Identifier by version and representation of the outermost object

**Spec:** §7.6.3.1.2, p. 231 — "The encapsulation identifier field in an RTPS
data sub-message shall be set such that it corresponds to the encoding version
and the data representation of the outermost object".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — FINAL, version 1, Big Endian: CDR_BE {0x00, 0x00}

**Spec:** §7.6.3.1.2, Table 60, p. 231 — "XCDR | FINAL | 1 | Big Endian | CDR_BE
| {0x00, 0x00}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — FINAL, version 1, Little Endian: CDR_LE {0x00, 0x01}

**Spec:** §7.6.3.1.2, Table 60, p. 231 — "XCDR | FINAL | 1 | Little Endian |
CDR_LE | {0x00, 0x01}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — APPENDABLE, version 1, Big Endian: CDR_BE {0x00, 0x00}

**Spec:** §7.6.3.1.2, Table 60, p. 231 — "XCDR | APPENDABLE | 1 | Big Endian |
CDR_BE | {0x00, 0x00}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — APPENDABLE, version 1, Little Endian: CDR_LE {0x00, 0x01}

**Spec:** §7.6.3.1.2, Table 60, p. 232 — "XCDR | APPENDABLE | 1 | Little Endian
| CDR_LE | {0x00, 0x01}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — MUTABLE, version 1, Big Endian: PL_CDR_BE {0x00, 0x02}

**Spec:** §7.6.3.1.2, Table 60, p. 232 — "XCDR | MUTABLE | 1 | Big Endian |
PL_CDR_BE | {0x00, 0x02}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — MUTABLE, version 1, Little Endian: PL_CDR_LE {0x00, 0x03}

**Spec:** §7.6.3.1.2, Table 60, p. 232 — "XCDR | MUTABLE | 1 | Little Endian |
PL_CDR_LE | {0x00, 0x03}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — FINAL, version 2, Big Endian: CDR2_BE {0x00, 0x06}

**Spec:** §7.6.3.1.2, Table 60, p. 232 — "XCDR | FINAL | 2 | Big Endian |
CDR2_BE | {0x00, 0x06}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — FINAL, version 2, Little Endian: CDR2_LE {0x00, 0x07}

**Spec:** §7.6.3.1.2, Table 60, p. 232 — "XCDR | FINAL | 2 | Little Endian |
CDR2_LE | {0x00, 0x07}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — APPENDABLE, version 2, Big Endian: D_CDR2_BE {0x00, 0x08}

**Spec:** §7.6.3.1.2, Table 60, p. 232 — "XCDR | APPENDABLE | 2 | Big Endian |
D_CDR2_BE | {0x00, 0x08}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — APPENDABLE, version 2, Little Endian: D_CDR2_LE {0x00, 0x09}

**Spec:** §7.6.3.1.2, Table 60, p. 232 — "XCDR | APPENDABLE | 2 | Little Endian
| D_CDR2_LE | {0x00, 0x09}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — MUTABLE, version 2, Big Endian: PL_CDR2_BE {0x00, 0x0a}

**Spec:** §7.6.3.1.2, Table 60, p. 232 — "XCDR | MUTABLE | 2 | Big Endian |
PL_CDR2_BE | {0x00, 0x0a}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — MUTABLE, version 2, Little Endian: PL_CDR_LE {0x00, 0x0b}

**Spec:** §7.6.3.1.2, Table 60, p. 232 — "XCDR | MUTABLE | 2 | Little Endian |
PL_CDR_LE | {0x00, 0x0b}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (l. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done — the row names `PL_CDR_LE` instead of `PL_CDR2_LE`
(DDSXTY14-42; listed in the findings file).

### §7.6.3.1.2 Table 60 — XML, any: XML {0x00, 0x04}

**Spec:** §7.6.3.1.2, Table 60, p. 232 — "XML | any | any | any | XML | {0x00,
0x04}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap`

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/src/runtime.rs::tests::an_xml_sample_reaches_the_reader_as_its_cdr_body`

**Status:** done — The XML row of Table 60: `user_payload_encap(XML, …)` returns `{0x00, 0x04, 0x00, 0x00}` for every extensibility and every byte order; a reader reads it.

### §7.6.3.1.2 Options field of 2 octets after the identifier, also for XML

**Spec:** §7.6.3.1.2, p. 232 — "the Encapsulation Identifier is followed by a 2
byte options field. [...] The XML encapsulation identifier is also followed by a
2 byte options field".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap`,
`crates/dcps/src/runtime.rs::pad_user_payload`

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/src/runtime.rs::tests::an_xml_sample_reaches_the_reader_as_its_cdr_body`,
`crates/dcps/src/runtime.rs::tests::an_xml_writer_sends_the_document_of_the_sample`

**Status:** done — Two octets of options follow the identifier, for XCDR and for XML; the two low bits of the options count the padding bytes after the document (`pad_user_payload`), and the reader takes them off (`padding_present`).

### §7.6.3.1.2 Options: the two lowest bits count the padding bytes to the 4-octet boundary

**Spec:** §7.6.3.1.2, p. 232 — "Implementations of this specification shall set
the least significant two bits in the second byte of the options field to a
value that encodes the number of padding bytes needed after the end of the
serialized payload in order to reach the next 4-byte aligned offset. [...] These
bits in the options field shall be interpreted by the receiver to determine
where the serialized data exactly ended."

**Repo:** `crates/dcps/src/runtime.rs::user_encap_body` (l. 20061);
`crates/wire-codec/src/decode.rs::padding_present` (l. 281);
`crates/wire-codec/src/value.rs::encapsulation_header` (l. 527)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_user_payload_names_its_padding_in_the_options`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_drops_the_padding_the_options_name`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_keeps_a_body_whose_counted_padding_is_absent`

**Status:** done — The DataWriter pads the payload with zeros to a multiple of four and writes the count into `options`; the receiver evaluates the bits.

### §7.6.3.1.2 The DataWriter sets the padding bits

**Spec:** §7.6.3.1.2, p. 232 — "shall set the least significant two bits [...]
binary 01, 10, or 11 if, respectively, there would be one, two, or three bytes
of padding needed."

**Repo:** —

**Tests:** `crates/dcps/src/runtime.rs::tests::a_user_payload_names_its_padding_in_the_options`

**Status:** done — `runtime.rs::pad_user_payload` appends the padding, and `user_payload_encap` sets the two lowest bits of `options`.

### §7.6.3.1.2 Example TypeA, TypeB, TypeC

**Spec:** §7.6.3.1.2, p. 232–233 — example with `options` 0x0002, 0x0001,
0x0000.

**Repo:** `crates/wire-codec/src/encode.rs::encode`

**Tests:** `crates/wire-codec/tests/encapsulation_options_example.rs::the_options_count_the_padding_of_the_three_objects`

**Status:** done — The test encodes `TypeA`, `TypeB` and `TypeC` with the values of the example in CDR big endian with the encapsulation and reads the octets back: `options` 0x0002, 0x0001, 0x0000 and the padding of zeros. The spec's line for O3 names `O3.member2` twice; `member3` is meant (typing slip of the spec, in the findings file).

### §7.6.3.1.3 Built-in topics: @id(0x0073) representation in topic, publication and subscription data

**Spec:** §7.6.3.1.3, p. 234 — "The topic, publication, and subscription
built-in topic data types shall each indicate the data representation of the
associated entity with a new member: @id(0x0073)
DDS::DataRepresentationQosPolicy representation;"

**Repo:** `crates/rtps/src/parameter_list.rs` (`DATA_REPRESENTATION` l. 193);
`crates/rtps/src/topic_data.rs::TopicBuiltinTopicData` (l. 57);
`crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData` (l. 207, field `data_representation` l. 274);
`crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData` (l. 37)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::publication_data_emits_data_representation_pid_when_set`,
`crates/dcps/tests/sedp_data_representation_pid.rs::publication_data_roundtrip_preserves_data_representation`

**Status:** done

---

## §7.6.3.2 Discovery Built-in Topics

### §7.6.3.2.1 TypeInformation instead of TypeObject in the built-in topics

**Spec:** §7.6.3.2.1, p. 234 — "Send TypeInformation that include
TypeIdentifiers (instead of TypeObjects) in the discovery built-in topics. [...]
Uses a new pair of built-in endpoints to request the TypeObjects".

**Repo:** `crates/dcps/src/runtime.rs::announced_type_information` (l. 8241)

**Tests:** `crates/dcps/src/runtime.rs::tests::issue42_publication_announces_standard_type_information`,
`crates/dcps/src/runtime.rs::tests::issue42_subscription_announces_standard_type_information`

**Status:** done

### §7.6.3.2.1 IDL: TypeIdentfierWithSize, TypeIdentifierWithDependencies, TypeInformation

**Spec:** §7.6.3.2.1, p. 234–235 — "@extensibility(APPENDABLE) struct
TypeIdentfierWithSize [...] @extensibility(MUTABLE) struct TypeInformation {
@id(0x1001) TypeIdentifierWithDependencies minimal; @id(0x1002)
TypeIdentifierWithDependencies complete; };"

**Repo:** `crates/types/src/type_information.rs::TypeIdentifierWithSize` (l. 70);
`crates/types/src/type_information.rs::TypeIdentifierWithDependencies` (l. 138);
`crates/types/src/type_information.rs::TypeInformation` (l. 209)

**Tests:** `crates/discovery/tests/representation_and_type_information.rs::type_information_is_built_from_a_keyed_appendable_struct`,
`crates/types/src/type_information.rs::tests::negative_dependent_typeid_count_roundtrips`

**Status:** done — the specification writes `TypeIdentfierWithSize` (slip in the
specification, listed in the findings file).

### §7.6.3.2.1 minimal.typeid_with_size and complete.typeid_with_size

**Spec:** §7.6.3.2.1, p. 235 — "The field minimal.typeid_with_size shall contain
the MINIMAL Hash TypeIdentifier of the TopicType and the serialized size of the
associated TypeObject. [...] The field complete.typeid_with_size shall contain
the COMPLETE Hash TypeIdentifier".

**Repo:** `crates/types/src/type_information.rs::from_minimal_and_complete` (l. 223)

**Tests:** `crates/discovery/tests/representation_and_type_information.rs::type_information_is_built_from_a_keyed_appendable_struct`

**Status:** done

### §7.6.3.2.1 dependent_typeid_count: number of dependencies or -1

**Spec:** §7.6.3.2.1, p. 235 — "The field minimal.dependent_typeid_count shall
contain the total number of other MINIMAL Hash TypeIdentifiers that correspond
to data-types the TopicType depends on. This field may be set to -1 to indicate
it is not being provided."

**Repo:** `crates/types/src/type_information.rs::from_registry`

**Tests:** `crates/types/tests/type_information_dependencies.rs::the_count_is_the_number_of_types_the_topic_type_depends_on`,
`crates/types/tests/type_information_dependencies.rs::a_dependency_the_registry_lacks_gives_minus_one`,
`crates/types/tests/type_information_dependencies.rs::a_type_without_dependencies_has_count_zero`

**Status:** done — `TypeInformation::from_registry` fills `dependent_typeid_count` and the lists of both sides with the dependent types and their sizes; `-1` if the registry does not know a type.

### §7.6.3.2.1 dependent_typeids: selection free, length at most dependent_typeid_count

**Spec:** §7.6.3.2.1, p. 235–236 — "The decision of which types to include in
the dependent_typeids is left to the implementation [...] If
dependent_typeid_count is not -1, then length of the dependent_typeids sequence
shall be less or equal to dependent_typeid_count."

**Repo:** `crates/types/src/type_information.rs::add_dependency` (l. 248)

**Tests:** `crates/types/src/type_information.rs::tests::negative_dependent_typeid_count_roundtrips`

**Status:** done — the empty list is announced.

### §7.6.3.2.1 Only direct hash identifiers, an SCC as a whole

**Spec:** §7.6.3.2.1, p. 236 — "The TypeIdentifiers included in the
TypeInformation shall include only direct HASH TypeIdentifiers [...] it shall
not contain individual type identifiers for types belonging to Strongly
Connected Component [...] instead it shall include the identifier of the whole
Strongly-Connected Component".

**Repo:** `crates/dcps/src/runtime.rs::announced_type_information`,
`crates/types/src/type_information.rs::from_component`

**Tests:** `crates/dcps/src/runtime.rs::tests::issue42_an_unregistered_hash_announces_no_type_information`,
`crates/dcps/src/runtime.rs::tests::a_recursive_topic_type_announces_the_type_information_of_its_component`,
`crates/types/tests/scc_type_information.rs::a_component_member_announces_both_forms`,
`crates/types/tests/scc_type_information.rs::what_the_component_reaches_is_listed_once_each`

**Status:** done — Direct hash identifiers are announced. A type in an SCC names itself with the identifier of the component and its position, the `SCCIdentifier` (`scc_index = 0`) of its own and of every reached component are among the dependencies, individual identifiers of types of a component are not (`TypeInformation::from_component`, `DcpsRuntime::register_type_component`).

### §7.6.3.2.1 Fetch unknown identifiers through TypeLookup

**Spec:** §7.6.3.2.1, p. 236 — "In case some of the TypeIdentifiers announced by
a remote endpoint are not known to a DomainParticipant, it can use the built-in
TypeLookup Service to retrieve the TypeObject".

**Repo:** `crates/dcps/src/runtime.rs::send_type_lookup_request` (l. 8277);
`crates/dcps/src/runtime.rs::send_type_dependencies_request` (l. 8412)

**Tests:** `crates/dcps/tests/typelookup_resolution_e2e.rs::get_types_answers_the_complete_form_of_a_hash`

**Status:** done

### §7.6.3.2.2 @id(0x0007) type_name

**Spec:** §7.6.3.2.2, p. 236 — "@id(0x0007) ObjectName type_name;"

**Repo:** `crates/rtps/src/parameter_list.rs` (`TYPE_NAME` l. 92)

**Tests:** `crates/dcps/tests/shapes_type_wire.rs::shape_type_type_name_matches_interop_convention`

**Status:** done

### §7.6.3.2.2 @id(0x0072) @optional TypeObjectV1 type (XTYPES 1.1)

**Spec:** §7.6.3.2.2, p. 236 — "@id(0x0072) @optional TypeObjectV1 type; //
XTYPES 1.1".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — optional member for XTYPES 1.1; ZeroDDS
neither writes nor reads it, leaving it out is permitted.

### §7.6.3.2.2 @id(0x0075) @optional TypeInformation type_information

**Spec:** §7.6.3.2.2, p. 236 — "@id(0x0075) @optional XTypes::TypeInformation
type_information; // XTYPES 1.2".

**Repo:** `crates/rtps/src/parameter_list.rs` (`TYPE_INFORMATION` l. 195);
`crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData` (l. 207, field `type_information` l. 268)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::publication_standard_type_information_yields_type_identifier`,
`crates/dcps/tests/xtypes_aware_match.rs::subscription_standard_type_information_yields_type_identifier`

**Status:** done

### §7.6.3.2.2 Non-normative note: without type information only type_name

**Spec:** §7.6.3.2.2, p. 236 — "Non-normative note: When the TypeObjectV1 and
TypeInformation members [...] are omitted [...] type_name is the only way to
resolve entity matching".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — "Non-normative note".

---

## §7.6.3.3 Built-in TypeLookup service

### §7.6.3.3.1 Two built-in topics for request and reply

**Spec:** §7.6.3.3.1, p. 236 — "One built-in topic is used for TypeLookup
requests. It has two built-in endpoints [...] The second built-in topic is used
for TypeLookup replies."

**Repo:** `crates/discovery/src/type_lookup/endpoints.rs::format_service_instance_name` (l. 31);
`crates/dcps/src/type_lookup_transport.rs::new` (l. 25)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::typelookup_endpoints_4_distinct_guids_per_participant`

**Status:** done

### §7.6.3.3.1 DDS-RPC not required

**Spec:** §7.6.3.3.1, p. 236 — "It is not, however, a requirement to implement
the DDS-RPC specification in order to claim compliance with this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — clarification.

### §7.6.3.3.2 Types from DDS-RPC: SampleIdentity, RequestHeader, ReplyHeader

**Spec:** §7.6.3.3.2, p. 237–238 — "struct SampleIdentity { GUID_t writer_guid;
SequenceNumber_t sequence_number; }; [...] struct RequestHeader {
SampleIndentity_t requestId; InstanceName instanceName; }; struct ReplyHeader {
dds::SampleIdentity relatedRequestId; dds::rpc::RemoteExceptionCode_t remoteEx;
};"

**Repo:** `crates/types/src/type_lookup.rs::RequestHeader` (l. 742);
`crates/types/src/type_lookup.rs::ReplyHeader` (l. 797)

**Tests:** `crates/types/tests/type_lookup_vendor_envelope.rs::all_four_service_envelopes_match_cyclone_serializer`

**Status:** done — the block begins with "/* END of definitions copied from
DDS-RPC */" and writes `SampleIndentity_t` (slips in the specification, listed
in the findings file).

### §7.6.3.3.3 Table 61 — TypeLookupServiceRequestDataWriter: ENTITYID_TL_SVC_REQ_WRITER = {{00, 03, 00}, c3}

**Spec:** §7.6.3.3.3, Table 61, p. 238 — "TypeLookupServiceRequestDataWriter |
ENTITYID_TL_SVC_REQ_WRITER = {{00, 03, 00}, c3} | TypeLookup_Request".

**Repo:** `crates/rtps/src/wire_types.rs::TL_SVC_REQ_WRITER` (l. 506)

**Tests:** `crates/discovery/src/type_lookup/endpoints.rs::tests::entity_ids_match_spec_constants`

**Status:** done

### §7.6.3.3.3 Table 61 — TypeLookupServiceRequestDataReader: ENTITYID_TL_SVC_REQ_READER = {{00, 03, 00}, c4}

**Spec:** §7.6.3.3.3, Table 61, p. 238 — "TypeLookupServiceRequestDataReader |
ENTITYID_TL_SVC_REQ_READER = {{00, 03, 00}, c4} | TypeLookup_Request".

**Repo:** `crates/rtps/src/wire_types.rs::TL_SVC_REQ_READER` (l. 511)

**Tests:** `crates/discovery/src/type_lookup/endpoints.rs::tests::entity_ids_match_spec_constants`

**Status:** done

### §7.6.3.3.3 Table 61 — TypeLookupServiceReplyDataWriter: ENTITYID_TL_SVC_REPLY_WRITER = {{00, 03, 01}, c3}

**Spec:** §7.6.3.3.3, Table 61, p. 238 — "TypeLookupServiceReplyDataWriter |
ENTITYID_TL_SVC_REPLY_WRITER = {{00, 03, 01}, c3} | TypeLookup_Reply".

**Repo:** `crates/rtps/src/wire_types.rs::TL_SVC_REPLY_WRITER` (l. 516)

**Tests:** `crates/discovery/src/type_lookup/endpoints.rs::tests::entity_ids_match_spec_constants`

**Status:** done

### §7.6.3.3.3 Table 61 — TypeLookupServiceReplyDataReader: ENTITYID_TL_SVC_REPLY_READER = {{00, 03, 01}, c4}

**Spec:** §7.6.3.3.3, Table 61, p. 238 — "TypeLookupServiceReplyDataReader |
ENTITYID_TL_SVC_REPLY_READER = {{00, 03, 01}, c4} | TypeLookup_Reply".

**Repo:** `crates/rtps/src/wire_types.rs::TL_SVC_REPLY_READER` (l. 521)

**Tests:** `crates/discovery/src/type_lookup/endpoints.rs::tests::entity_ids_match_spec_constants`

**Status:** done

### §7.6.3.3.3 Pairs for invoking and for serving

**Spec:** §7.6.3.3.3, p. 239 — "The pair TypeLookupServiceRequestDataWriter and
TypeLookupServiceReplyDataReader is used to invoke the built-in TypeLookup
Service [...] The pair TypeLookupServiceRequestDataReader and
TypeLookupServiceReplyDataWriter is used to implement the TypeLookup Service".

**Repo:** `crates/dcps/src/runtime.rs::send_type_lookup_request` (l. 8277);
`crates/dcps/src/runtime.rs::dispatch_type_lookup_datagram` (l. 22895);
`crates/dcps/src/runtime.rs::send_type_lookup_reply` (l. 23189)

**Tests:** `crates/dcps/tests/type_lookup_e2e.rs::server_handles_inbound_get_types_request_via_dispatch`

**Status:** done

### §7.6.3.3.3 QoS of the four endpoints: RELIABLE, KEEP_ALL, VOLATILE

**Spec:** §7.6.3.3.3, p. 239 — "the RELIABILITY policy shall be
DDS_RELIABLE_RELIABILITY_QOS, the HISTORY policy to DDS_KEEP_ALL_HISTORY_QOS and
the DURABILITY policy to DDS_VOLATILE_DURABILITY_QOS."

**Repo:** `crates/dcps/src/type_lookup_transport.rs::Channel::new`

**Tests:** `crates/dcps/src/type_lookup_transport.rs::tests::the_endpoints_are_reliable_keep_all_and_volatile`,
`crates/dcps/src/type_lookup_transport.rs::tests::the_history_of_a_type_lookup_endpoint_keeps_all`

**Status:** done — The four endpoints are RELIABLE (reliable proxies, HEARTBEATs), KEEP_ALL (40 unacknowledged samples stay in the cache) and VOLATILE (a peer that matches after a write is not sent the sample).

### §7.6.3.3.3 Hash IDs getTypes 0x018252d3, getDependencies 0x05aafb31

**Spec:** §7.6.3.3.3, p. 239 — "const unsigned long TypeLookup_getTypes_HashId =
0x018252d3; [...] const unsigned long TypeLookup_getDependencies_HashId =
0x05aafb31;"

**Repo:** `crates/types/src/type_lookup.rs::OPERATION_GET_TYPES` (l. 140);
`crates/types/src/type_lookup.rs::OPERATION_GET_DEPENDENCIES` (l. 148)

**Tests:** `crates/types/src/type_lookup.rs::tests::the_operation_hashes_are_the_ones_the_clause_prints`

**Status:** done — the unions name `TypeLookup_getTypes_Hash` instead of
`…_HashId` (slip in the specification, listed in the findings file).

### §7.6.3.3.3 TypeLookup_getTypes_In and _Out: MUTABLE with @hashid

**Spec:** §7.6.3.3.3, p. 239 — "@extensibility(MUTABLE) struct
TypeLookup_getTypes_In { @hashid sequence<TypeIdentifier> type_ids; };
@extensibility(MUTABLE) struct TypeLookup_getTypes_Out { @hashid
sequence<TypeIdentifierTypeObjectPair> types; @hashid
sequence<TypeIdentifierPair> complete_to_minimal; };"

**Repo:** `crates/types/src/type_lookup.rs::GetTypesRequest` (l. 240);
`crates/types/src/type_lookup.rs::GetTypesReply` (l. 282);
`crates/types/src/type_lookup.rs::member_ids` (l. 122)

**Tests:** `crates/types/src/type_lookup.rs::tests::member_ids_are_the_hashid_of_their_name`,
`crates/types/src/type_lookup.rs::tests::get_types_reply_carries_types_and_complete_to_minimal`

**Status:** done

### §7.6.3.3.3 TypeLookup_getTypeDependencies_In and _Out with continuation_point

**Spec:** §7.6.3.3.3, p. 239–240 — "struct TypeLookup_getTypeDependencies_In {
@hashid sequence<TypeIdentifier> type_ids; @hashid sequence<octet, 32>
continuation_point; }; [...] struct TypeLookup_getTypeDependencies_Out { @hashid
sequence<TypeIdentifierWithSize> dependent_typeids; @hashid sequence<octet, 32>
continuation_point; };"

**Repo:** `crates/types/src/type_lookup.rs::GetTypeDependenciesRequest` (l. 442);
`crates/types/src/type_lookup.rs::GetTypeDependenciesReply` (l. 491);
`crates/types/src/type_lookup.rs::ContinuationPoint` (l. 407)

**Tests:** `crates/types/src/type_lookup.rs::tests::get_type_dependencies_request_reply_roundtrip`,
`crates/types/src/type_lookup.rs::tests::continuation_point_len_max_plus_one_on_decode_rejected`

**Status:** done

### §7.6.3.3.3 Result, Call and Return unions

**Spec:** §7.6.3.3.3, p. 239–240 — "union TypeLookup_getTypes_Result
switch(long) { case DDS_RETCODE_OK: [...] union TypeLookup_Call switch(long) {
[...] union TypeLookup_Return switch(long) {".

**Repo:** `crates/types/src/type_lookup.rs::TypeLookupCall` (l. 605);
`crates/types/src/type_lookup.rs::TypeLookupReturn` (l. 670)

**Tests:** `crates/types/src/type_lookup.rs::tests::type_lookup_call_roundtrips_both_operations`,
`crates/types/src/type_lookup.rs::tests::type_lookup_return_roundtrips_both_operations`,
`crates/types/src/type_lookup.rs::tests::get_types_result_roundtrips_both_arms`

**Status:** done — the unions are appendable (default of the specification).

### §7.6.3.3.3 TypeLookup_Request and TypeLookup_Reply

**Spec:** §7.6.3.3.3, p. 240–241 — "@RPCRequestType struct TypeLookup_Request {
dds::rpc::RequestHeader header; TypeLookup_Call data; }; [...] @RPCReplyType
struct TypeLookup_Reply { dds::rpc::RequestHeader header; TypeLookup_Return
return; };"

**Repo:** `crates/types/src/type_lookup.rs::TypeLookupRequest` (l. 851);
`crates/types/src/type_lookup.rs::TypeLookupReplyMessage` (l. 886)

**Tests:** `crates/types/src/type_lookup.rs::tests::type_lookup_request_and_reply_roundtrip`,
`crates/types/tests/type_lookup_vendor_envelope.rs::all_four_service_envelopes_match_cyclone_serializer`

**Status:** done — the reply carries a `ReplyHeader`; the specification writes
`RequestHeader` (defect of the specification, listed in the findings file). Both
envelopes are `@final` by DDSXTY14-30, not appendable as the specification's
default; DDS-XTypes 1.4 Beta 1 §7.6.3.3.3 writes them so. On
the wire both go out as CDR2_LE with the encoding version 2 by default (1.4
Beta 1 §7.6.3.3.3, OMG issue DDSXTY14-29), and as CDR_LE with version 1 under
the setting `wire.type_lookup_encoding = xcdr1` (Table 60 for an appendable
type under the default XCDR, §7.6.3.1.1; under XCDR1 final and appendable are
the same octets; DDSI-RTPS 2.5, item "§10.4 TypeLookup endpoints with
CDR2_LE"; tests `crates/types/tests/type_lookup_xcdr1.rs`).

### §7.6.3.3.3 Extensions of the _In/_Out types with @hashid under an own domain

**Spec:** §7.6.3.3.3, p. 241 — "Implementers may add their own members to these
MUTABLE types. If they do they shall use member IDs obtained using the @hashid
annotation with a string value that has an Internet domain name owned by the
implementor prefix."

**Repo:** `crates/types/src/type_lookup.rs::GetTypesRequest` (l. 240)

**Tests:** `crates/types/src/type_lookup.rs::tests::an_unknown_member_id_is_skipped_rather_than_refused`

**Status:** done — ZeroDDS adds no members; an unknown member of another vendor
is skipped.

### §7.6.3.3.4 availableBuiltinEndpoints: bits for the TypeLookup service

**Spec:** §7.6.3.3.4, p. 241 — "This DDS XTypes specification reserves
additional bits to indicate the presence of the corresponding built-in end
points for the TypeObjectLookup Service. These bits shall be set on the
availableBuiltinEndpoints."

**Repo:** `crates/rtps/src/participant_data.rs` (`TYPE_LOOKUP_REQUEST_DATA_WRITER` l. 71)

**Tests:** `crates/dcps/tests/type_lookup_e2e.rs::announces_type_lookup_endpoints_in_spdp`

**Status:** done

### §7.6.3.3.4 Table 62 — TypeLookupServiceRequestDataWriter: 0x00000001 << 12

**Spec:** §7.6.3.3.4, Table 62, p. 242 — "TypeLookupServiceRequestDataWriter |
(0x00000001 << 12)".

**Repo:** `crates/rtps/src/participant_data.rs` (l. 71)

**Tests:** `crates/discovery/src/capabilities.rs::tests::type_lookup_capabilities_require_the_four_distinct_vendor_bits`

**Status:** done

### §7.6.3.3.4 Table 62 — TypeLookupServiceRequestDataReader: 0x00000001 << 13

**Spec:** §7.6.3.3.4, Table 62, p. 242 — "TypeLookupServiceRequestDataReader |
(0x00000001 << 13)".

**Repo:** `crates/rtps/src/participant_data.rs` (l. 73)

**Tests:** `crates/discovery/src/capabilities.rs::tests::type_lookup_capabilities_require_the_four_distinct_vendor_bits`

**Status:** done

### §7.6.3.3.4 Table 62 — TypeLookupServiceReplyDataWriter: 0x00000001 << 14

**Spec:** §7.6.3.3.4, Table 62, p. 242 — "TypeLookupServiceReplyDataWriter |
(0x00000001 << 14)".

**Repo:** `crates/rtps/src/participant_data.rs` (l. 75)

**Tests:** `crates/discovery/src/capabilities.rs::tests::type_lookup_capabilities_require_the_four_distinct_vendor_bits`

**Status:** done

### §7.6.3.3.4 Table 62 — TypeLookupServiceReplyDataReader: 0x00000001 << 15

**Spec:** §7.6.3.3.4, Table 62, p. 242 — "TypeLookupServiceReplyDataReader |
(0x00000001 << 15)".

**Repo:** `crates/rtps/src/participant_data.rs` (l. 77)

**Tests:** `crates/discovery/src/capabilities.rs::tests::type_lookup_capabilities_require_the_four_distinct_vendor_bits`

**Status:** done

### §7.6.3.3.4 Server: request reader and reply writer, client: request writer and reply reader

**Spec:** §7.6.3.3.4, p. 242 — "Participants implementing (as a server) the
TypeLookup service shall implement the TypeObjectServiceRequestDataReader and
TypeObjectServiceReplyDataWriter. [...] Participants using (as a client) the
TypeLookup shall implement the TypeObjectServiceRequestDataWriter and
TypeObjectServiceReplyDataReader."

**Repo:** `crates/dcps/src/type_lookup_transport.rs::new` (l. 25)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::typelookup_endpoints_4_distinct_guids_per_participant`

**Status:** done — ZeroDDS is server and client. The specification names the
endpoints `TypeObjectService…` here instead of `TypeLookupService…` (listed in
the findings file).

### §7.6.3.3.4 instanceName: dds.builtin.TOS. and GUID in lower-case hex digits

**Spec:** §7.6.3.3.4, p. 242 — "The Service instanceName [...] shall be set to
the string obtained by concatenating the prefix 'dds.builtin.TOS .' With the
16-character string version of the DomainParticipant GUID encoded using
hexadecimal digits with lower case letters. There shall be no '0x' ahead of the
hexadecimal digits."

**Repo:** `crates/types/src/type_lookup.rs::service_instance_name` (l. 838);
`crates/discovery/src/type_lookup/endpoints.rs::format_service_instance_name` (l. 31)

**Tests:** `crates/types/src/type_lookup.rs::tests::the_service_instance_name_is_the_prefix_and_lowercase_hex`,
`crates/discovery/tests/type_lookup_service.rs::service_instance_name_format_matches_spec`

**Status:** done — 32 hex digits: the GUID has 16 octets. The specification says
16 characters, its example has 15 and the prefix a space before the dot (listed
in the findings file).

### §7.6.3.3.4 Reply to every announced TypeIdentifier

**Spec:** §7.6.3.3.4, p. 242 — "If a participant implements the TypeLookup it
shall respond to requests for any TypeIdentifier that it announced within the
TypeInformation".

**Repo:** `crates/discovery/src/type_lookup/server.rs::handle_get_types` (l. 94);
`crates/dcps/src/runtime.rs::announced_type_information` (l. 8241)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::server_handle_get_types_happy_path_one_type`,
`crates/dcps/tests/typelookup_resolution_e2e.rs::get_types_answers_the_complete_form_of_a_hash`

**Status:** done — only what the server's registry holds is announced.

### §7.6.3.3.4 RequestHeader by DDS-RPC

**Spec:** §7.6.3.3.4, p. 242 — "The dds::rpc::RequestHeader in the
TypeLookup_Request and the TypeLookup_Reply shall be set as specified in the
[DDS-RPC] specification."

**Repo:** `crates/types/src/type_lookup.rs::RequestHeader` (l. 742);
`crates/types/src/type_lookup.rs::ReplyHeader` (l. 797)

**Tests:** `crates/discovery/src/type_lookup/client.rs::reply_correlation_requires_original_peer_and_operation`

**Status:** done

### §7.6.3.3.4.1 getTypeDependencies: request with direct hashes of one kind, an SCC as a whole

**Spec:** §7.6.3.3.4.1, p. 242 — "The TypeIdentifiers shall be only direct HASH
Identifiers. The TypeIdentifiers shall be either all MINIMAL hash
TypeIdentifiers or all COMPLETE hash TypeIdentifiers. [...] The TypeIdentifiers
shall not include identifiers for individual types in Strongly Connected
Components".

**Repo:** `crates/discovery/src/type_lookup/server.rs::component_dependents`;
`crates/dcps/src/runtime.rs::send_type_dependencies_request`

**Tests:** `crates/dcps/tests/typelookup_dependencies_wire.rs::the_dependencies_of_a_hash_come_over_the_wire`,
`crates/dcps/tests/typelookup_dependencies_wire.rs::the_dependencies_of_a_component_come_over_the_wire`,
`crates/discovery/src/type_lookup/client.rs::tests::dependencies_payload_carries_continuation`,
`crates/discovery/tests/type_lookup_scc_minimal.rs::the_dependencies_of_a_component_are_the_types_its_members_name`

**Status:** done — The server answers `getTypeDependencies` for a hash and for the identifier of a component as a whole: the hash identifiers of the types the members name and the `SCCIdentifier`s of the other components reached; its own is not among them. Two runtimes: the request for the dependencies of `Outer` brings `Inner` into the requester's registry and not `Outer`; the one for a component brings the type outside that a member names.

### §7.6.3.3.4.1 continuation_point in the request

**Spec:** §7.6.3.3.4.1, p. 243 — "The field continuation_point shall not be
present if the requester wants the response to include all the types [...]
Otherwise it shall be set to the continuation_point of the
TypeLookup_getTypeDependencies_Out received in response to a previous call".

**Repo:** `crates/types/src/type_lookup.rs::GetTypeDependenciesRequest` (l. 442)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::server_continuation_point_paginates_200_dependencies`,
`crates/types/src/type_lookup.rs::tests::continuation_point_len_zero_encodes_to_four_zero_bytes`

**Status:** done — "not present" is an empty sequence; the member is not
`@optional`.

### §7.6.3.3.4.1 Reply: only direct hashes, continuation_point for an incomplete list

**Spec:** §7.6.3.3.4.1, p. 243 — "The field dependent_typeids shall exclusively
contain of direct HASH TypeIdentifiers that are recursive dependencies [...] The
field continuation_point shall not be present if the response contains the
complete list of types, otherwise it shall contain an opaque value".

**Repo:** `crates/discovery/src/type_lookup/server.rs::handle_get_type_dependencies` (l. 250)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::server_handle_get_type_dependencies_struct_with_5_deps`,
`crates/discovery/tests/type_lookup_service.rs::server_continuation_point_paginates_200_dependencies`

**Status:** done

### §7.6.3.3.4.2 getTypes: request with direct hashes, an SCC as a whole

**Spec:** §7.6.3.3.4.2, p. 243 — "The field type_ids shall contain the direct
HASH TypeIdentifiers for which the participant is requesting the TypeObjects.
The field type_ids shall not include individual TypeIdentifiers belonging to a
Strongly Connected Component".

**Repo:** `crates/dcps/src/runtime.rs::send_type_lookup_request` (l. 8277)

**Tests:** `crates/discovery/tests/type_lookup_requests.rs::a_get_types_request_and_its_reply_round_trip_between_two_stacks`

**Status:** done

### §7.6.3.3.4.2 Reply: COMPLETE for COMPLETE, either for MINIMAL

**Spec:** §7.6.3.3.4.2, p. 243 — "If the request had a COMPLETE TypeIdentifiers,
the types shall contain COMPLETE TypeObjects. If the request had MINIMAL
TypeIdentifiers the types may contain either MINIMAL or COMPLETE TypeObjects."

**Repo:** `crates/discovery/src/type_lookup/server.rs::handle_get_types` (l. 94)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::server_handle_get_types_happy_path_one_type`,
`crates/discovery/src/type_lookup/server.rs::tests::handle_get_types_pairs_a_complete_identifier_with_its_minimal_hash`

**Status:** done

### §7.6.3.3.4.2 complete_to_minimal

**Spec:** §7.6.3.3.4.2, p. 243–244 — "The field complete_to_minimal shall
contain the mapping from COMPLETE TypeIdentifiers to MINIMAL TypeIdentifiers for
any COMPLETE TypeIdentifiers that appear within COMPLETE TypeObjects that were
sent in response to a query for a MINIMAL TypeIdentfier."

**Repo:** `crates/discovery/src/type_lookup/server.rs::handle_get_types` (l. 94)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::typeidentifier_with_minimal_hash_yields_complete_to_minimal_pair`

**Status:** done — the server fills the mapping also for a request with a
COMPLETE identifier; the comment there cites §7.6.3.3.4.1 instead of
§7.6.3.3.4.2.

### §7.6.3.3.4.2 SCC identifier: all TypeObjects of the component or none

**Spec:** §7.6.3.3.4.2, p. 244 — "If a TypeIdentifier was a SCCIdentifier [...],
then the response shall threat the TypeObjects within the Strongly Connected
Components atomically. Either include all in the reply or none."

**Repo:** `crates/discovery/src/type_lookup/server.rs::handle_get_types`

**Tests:** `crates/discovery/tests/type_lookup_scc_atomic.rs::an_scc_identifier_gets_every_member_of_the_component`,
`crates/discovery/tests/type_lookup_scc_atomic.rs::an_unknown_component_or_a_wrong_length_gets_none`

**Status:** done — `getTypes` answers an SCCIdentifier with every TypeObject of the component or with none; an unknown component or a wrong length gets none.

---

## §7.6.3.4 Type Consistency Enforcement QoS Policy

### §7.6.3.4 TypeConsistencyEnforcementQosPolicy for DataReaders

**Spec:** §7.6.3.4, p. 244 — "The Type Consistency Enforcement QoS Policy
defines the rules for determining whether the type used to publish a given data
stream is consistent with that used to subscribe to it. It applies to
DataReaders."

**Repo:** `crates/dcps/src/qos.rs::DataReaderQos` (l. 114, field `type_consistency` l. 162);
`crates/types/src/qos.rs::TypeConsistencyEnforcement` (l. 52)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::datareader_qos_exposes_type_consistency_enforcement`

**Status:** done

### §7.6.3.4.1 Kinds DISALLOW_TYPE_COERCION and ALLOW_TYPE_COERCION

**Spec:** §7.6.3.4.1, p. 244 — "DISALLOW_TYPE_COERCION: The DataWriter and the
DataReader must support the same data type in order for them to communicate.
[...] ALLOW_TYPE_COERCION: [...] as long as the reader's type is assignable from
the writer's type."

**Repo:** `crates/types/src/qos.rs::TypeConsistencyKind` (l. 13)

**Tests:** `crates/types/src/qos.rs::tests::type_consistency_kind_has_the_two_values_of_annex_d`,
`crates/types/src/type_matcher.rs::tests::disallow_type_coercion_needs_equivalent_types`,
`crates/types/src/type_matcher_vectors.rs::disallow_coercion_rejects_int16_int32`

**Status:** done — `TypeConsistencyKind` has the two values of Annex D, `DISALLOW_TYPE_COERCION` and `ALLOW_TYPE_COERCION`; `force_type_validation` is a field of the policy. Under DISALLOW the matching requires equivalent types.

### §7.6.3.4.1 DISALLOW_TYPE_COERCION requires the same types

**Spec:** §7.6.3.4.1, p. 244 — "DISALLOW_TYPE_COERCION: The DataWriter and the
DataReader must support the same data type in order for them to communicate."

**Repo:** `crates/types/src/type_matcher.rs::build_config`

**Tests:** `crates/types/src/type_matcher.rs::tests::disallow_type_coercion_needs_equivalent_types`

**Status:** done — `type_matcher.rs::build_config` checks MINIMAL equivalence under §7.3.4.7 under DISALLOW; a reader with DISALLOW does not accept a type of the writer that is assignable but not equivalent.

### §7.6.3.4.1 Only for DataReaders, no RxO, not changeable after enable

**Spec:** §7.6.3.4.1, p. 244 — "This policy applies only to DataReaders; it does
not have request-offer (RxO) semantics [DDS]. The value of this policy cannot be
changed after the entity in question has been enabled."

**Repo:** `crates/dcps/src/qos.rs::immutable_reader_change` (l. 241)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::a_readers_type_consistency_policy_survives_its_announcement`

**Status:** done — the writer decides with the policy the reader announces.

### §7.6.3.4.1 Default ALLOW_TYPE_COERCION

**Spec:** §7.6.3.4.1, p. 244 — "The default enforcement kind shall be
ALLOW_TYPE_COERCION."

**Repo:** `crates/types/src/qos.rs::TypeConsistencyEnforcement` (l. 67, `Default`)

**Tests:** `crates/types/src/qos.rs::tests::consistency_roundtrip_default`

**Status:** done

### §7.6.3.4.1 A missing policy of the remote endpoint is taken as DISALLOW_TYPE_COERCION

**Spec:** §7.6.3.4.1, p. 244 — "if it observes that no
TypeConsistencyEnforcementQosPolicy value is provided [...] it shall assume a
kind of DISALLOW_TYPE_COERCION".

**Repo:** `crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData`,
`crates/types/src/qos.rs::TypeConsistencyEnforcement`

**Tests:** `crates/rtps/src/subscription_data.rs::tests::a_reader_announcing_no_type_consistency_is_read_as_disallow`,
`crates/dcps/tests/runtime_config_layering.rs::the_xtypes_defaults_are_the_clauses_and_each_has_a_named_way_back`

**Status:** done — An endpoint without PID `0x0074` is taken as `DISALLOW_TYPE_COERCION` (`TypeConsistencyEnforcement::not_announced`); `SubscriptionBuiltinTopicData::type_consistency_announced` tells that from an announced DISALLOW. The old behaviour is reachable with `xtypes.type_consistency_absent: allow` (environment `ZERODDS_TYPE_CONSISTENCY_ABSENT`).

### §7.6.3.4.1 Widening of a type

**Spec:** §7.6.3.4.1, p. 245 — "A type T2 is said to widen type T1 when type T2
contains non-optional fields that are not present in T1. [...] Note that
optional members in T2 that are not present on T1 do not make T2 'wider' than
T1".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition of terms.

### §7.6.3.4.1 prevent_type_widening: default FALSE, TRUE forbids assignment to the narrower type

**Spec:** §7.6.3.4.1, p. 245 — "If the option is set to FALSE (the default),
type widening is permitted. If the option is set to TRUE, it shall cause a wider
type to not be assignable to a narrower type."

**Repo:** `crates/types/src/type_matcher.rs::build_config` (l. 119)

**Tests:** `crates/types/src/assignability.rs::tests::prevent_type_widening_refuses_a_reader_that_grew_a_member`,
`crates/types/src/type_matcher_vectors.rs::prevent_widening_rejects_int16_int32`

**Status:** done

### §7.6.3.4.1 ignore_sequence_bounds: Default TRUE

**Spec:** §7.6.3.4.1, p. 245 — "If the option is set to TRUE (the default),
sequence bounds (maximum lengths) are not considered as part of the type
assignability. [...] If the option is set to false [...] it is required that
L1>= L2."

**Repo:** `crates/types/src/qos.rs::TypeConsistencyEnforcement` (l. 67);
`crates/types/src/type_matcher.rs::build_config` (l. 119)

**Tests:** `crates/types/src/type_matcher_vectors.rs::sequence_bounds_ignored_by_default_tce`,
`crates/types/src/assignability.rs::tests::sequence_writer_bound_exceeds_reader_bound_is_no`

**Status:** done

### §7.6.3.4.1 ignore_string_bounds: Default TRUE

**Spec:** §7.6.3.4.1, p. 245 — "If the option is set to TRUE (the default),
string bounds (maximum lengths) are not considered as part of the type
assignability."

**Repo:** `crates/types/src/qos.rs::TypeConsistencyEnforcement` (l. 67)

**Tests:** `crates/types/src/type_matcher_vectors.rs::string_bounds_ignored_by_default_tce`,
`crates/types/src/type_matcher_vectors.rs::string_bound_overflow_rejected_when_strict`

**Status:** done

### §7.6.3.4.1 ignore_member_names: Default FALSE

**Spec:** §7.6.3.4.1, p. 245 — "If the option is set to TRUE, member names are
considered as part of assignability in addition to member IDs [...] If the
option is set to FALSE (the default), then member names are not ignored."

**Repo:** `crates/types/src/qos.rs::TypeConsistencyEnforcement` (l. 67);
`crates/types/src/type_matcher.rs::build_config` (l. 119)

**Tests:** `crates/types/src/assignability.rs::tests::a_member_name_at_two_different_ids_is_refused`

**Status:** done — TRUE disregards the names, as step 1 in §7.6.3.4.2 says. The
sentence in §7.6.3.4.1 says the opposite for TRUE (contradiction in the
specification, listed in the findings file).

### §7.6.3.4.1 Flags effective only under ALLOW_TYPE_COERCION

**Spec:** §7.6.3.4.1, p. 245 — "The values of prevent_type_widening,
ignore_sequence_bounds, ignore_string_bounds, and ignore_member_names only apply
when the type consistency kind is ALLOW_TYPE_COERCION, otherwise the fields are
treated as though prevent_type_widening is set to true and the others are set to
false."

**Repo:** `crates/types/src/type_matcher.rs::build_config`

**Tests:** `crates/types/src/type_matcher.rs::tests::the_flags_apply_only_under_allow_type_coercion`,
`crates/types/src/type_matcher.rs::tests::disallow_type_coercion_needs_equivalent_types`

**Status:** done — Under DISALLOW_TYPE_COERCION `prevent_type_widening`, `ignore_sequence_bounds`, `ignore_string_bounds` and `ignore_member_names` read as true, false, false, false, however they are set; under ALLOW_TYPE_COERCION they apply as set.

### §7.6.3.4.1 force_type_validation: default FALSE, TRUE requires type information

**Spec:** §7.6.3.4.1, p. 246 — "The force_type_validation option requires type
information to be available in order to complete matching between a DataWriter
and DataReader when set to TRUE [...] The default value is false."

**Repo:** `crates/dcps/src/runtime.rs::decide_type_consistency` (l. 18903)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::type_matcher_rejects_incompatible_primitives_under_force_validation`

**Status:** done

### §7.6.3.4.2 Step 1: assignability under the flags, equivalence without coercion

**Spec:** §7.6.3.4.2, p. 246 — "If the Subscription allows type coercion, then
the_type indicated there must be assignable from the_type of the Publication,
taking into account the values of prevent_type_widening, ignore_sequence_bounds,
ignore_string_bounds, and ignore_member_names. If the Subscription does not
allow type coercion, then its type must be equivalent to the type of the
Publication."

**Repo:** `crates/dcps/src/runtime.rs::decide_type_consistency` (l. 18903);
`crates/types/src/type_matcher.rs::build_config` (l. 119)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::final_vs_appendable_mismatch_rejected_at_match_time`,
`crates/dcps/tests/xtypes_aware_match.rs::appendable_compatible_matches_at_match_time`,
`crates/types/src/type_matcher.rs::tests::disallow_type_coercion_needs_equivalent_types`

**Status:** done — Under ALLOW the assignability with the flags holds, under DISALLOW the equivalence.

### §7.6.3.4.2 Step 1: ignore_member_names disregards the names

**Spec:** §7.6.3.4.2, p. 246 — "If the subscription allows type coercion and the
ignore_member_names flag is true [...] assignability checking shall ignore the
member names in both Subscription and Publication types. That is, only member
IDs will impact assignability."

**Repo:** `crates/types/src/type_matcher.rs::build_config`

**Tests:** `crates/types/src/type_matcher.rs::tests::ignore_member_names_lets_only_the_member_ids_decide`,
`crates/types/src/assignability.rs::tests::a_member_name_at_two_different_ids_is_refused`

**Status:** done — With `ignore_member_names = true` under ALLOW_TYPE_COERCION only the member ids decide: the same name `x1` at id 1 and id 2 is then assignable, without the flag it is not; under DISALLOW_TYPE_COERCION the flag has no effect.

### §7.6.3.4.2 Step 2: without a TypeObject equal type_name, fails under force_type_validation

**Spec:** §7.6.3.4.2, p. 246 — "If either the Publication or the Subscription
does not provide a TypeObject definition, then the type names are consulted. The
Subscription and Publication type_name fields must match exactly [...] This step
will fail if force_type_validation is true, regardless of the type names."

**Repo:** `crates/dcps/src/runtime.rs::decide_type_consistency` (l. 18903)

**Tests:** `crates/dcps/src/runtime.rs::tests::xtypes_step2_falls_back_to_the_type_name_for_a_peer_without_type_information`,
`crates/dcps/tests/typelookup_resolution_e2e.rs::an_unanswered_lookup_falls_back_to_the_type_names_after_the_grace`

**Status:** done

### §7.6.3.4.2 Failure: no communication, INCONSISTENT_TOPIC on both topics

**Spec:** §7.6.3.4.2, p. 246 — "If either Step 1 or Step 2 fails [...] the
DataWriter and DataReader shall not communicate with each other, and the Service
shall trigger an INCONSISTENT_TOPIC status change for both the DataReader's
Topic and the DataWriter's Topic."

**Repo:** `crates/dcps/src/runtime.rs::record_inconsistent_topic` (l. 18314);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (l. 12220)

**Tests:** `crates/dcps/tests/listener_trigger_c22c.rs::inconsistent_topic_listener_fires_on_qos_mismatch_of_a_name`,
`crates/dcps/tests/topics_of_one_name.rs::an_inconsistent_qos_is_refused_and_counted`

**Status:** done — Each side reports `INCONSISTENT_TOPIC` for its own topic. A further type under a name in use is a further topic; a different QoS under the same name is refused and counted.

### §7.6.3.4.2 Success: continue with the QoS check; both sides reach the same result

**Spec:** §7.6.3.4.2, p. 246 — "If both Step 1 and Step 2 succeed, then the
Topics are considered to be consistent, and the matching shall proceed to check
other aspects of endpoint matching [...] the algorithm is such that both sides
are guaranteed to arrive at the same conclusion."

**Repo:** `crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (l. 12220);
`crates/dcps/src/runtime.rs::wire_reader_to_remote_writer` (l. 12732)

**Tests:** `crates/dcps/src/runtime.rs::tests::xtypes_step2_falls_back_to_the_type_name_for_a_peer_without_type_information`,
`crates/dcps/tests/xtypes_aware_match.rs::a_readers_type_consistency_policy_survives_its_announcement`,
`crates/rtps/src/subscription_data.rs::tests::a_reader_announcing_no_type_consistency_is_read_as_disallow`

**Status:** done — Both sides decide with the policy of the reader; the reader announces it (PID `0x0074`), and a missing PID is taken as DISALLOW as in §7.6.3.4.1.

### §7.6.3.4.3 Built-in subscription: @id(0x0074) type_compatibility

**Spec:** §7.6.3.4.3, p. 247 — "The subscription built-in topic data type shall
indicate the type consistency requirements of the corresponding reader:
@id(0x0074) DDS::TypeConsistencyEnforcementQosPolicy type_compatibility;"

**Repo:** `crates/rtps/src/parameter_list.rs` (`TYPE_CONSISTENCY_ENFORCEMENT` l. 198);
`crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData` (l. 37, field `type_consistency` l. 111)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::a_readers_type_consistency_policy_survives_its_announcement`

**Status:** done — always written, with `kind` as `unsigned short`.

---

## §7.6.4 Local API Extensions

### §7.6.4.1 create_topic: several topic objects of the same name

**Spec:** §7.6.4.1, p. 247 — "In implementations conforming to this
specification, that restriction shall be removed. The Service may instantiate
multiple objects of the same name, provided that all of them represent
type-based subsets of 'the same' network topic".

**Repo:** `crates/dcps/src/participant.rs::create_topic`;
`crates/dcps/src/participant.rs::delete_registered_topic`;
`crates/zerodds-c-api/src/participant_ffi.rs::zerodds_dp_create_topic`;
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java::created`

**Tests:** `crates/dcps/tests/topics_of_one_name.rs::a_second_type_under_one_name_is_a_second_topic`,
`crates/dcps/tests/topics_of_one_name.rs::an_inconsistent_qos_is_refused_and_counted`,
`crates/dcps/tests/topics_of_one_name.rs::deleting_one_topic_leaves_the_other_of_its_name`,
`crates/dcps/tests/topics_of_one_name.rs::a_topic_waits_for_its_own_endpoints_and_not_for_those_of_the_other_of_its_name`,
`crates/zerodds-c-api/tests/builtin_types_c.rs::lookup_iterates_over_the_topics_of_a_name`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::aTopicOfAnExistingNameWithOtherPoliciesIsRefused`

**Status:** done — `create_topic` creates a further topic for a further type under a name in use if the QoS is equal; with a differing QoS it is refused and `INCONSISTENT_TOPIC` is counted. `delete_topic` waits for the endpoints of its own topic and not for those of another topic of the same name (the users are counted by name and type name; a MultiTopic names its topics by name alone and counts for all). `zerodds_dp_create_topic` of the C ABI, the C++ PSM (through the C ABI) and the Java PSM (`DomainParticipantImpl`) do the same.

### §7.6.4.2 lookup_topicdescription with index

**Spec:** §7.6.4.2, p. 247 — "The lookup_topicdescription operation shall accept
an optional in unsigned long argument called index. [...] Values of the index
from 0 to one less than the number of different TopicDescriptions [...] shall
return a TopicDescription and values of the index outside the range shall return
nil."

**Repo:** `crates/dcps/src/participant.rs::lookup_topicdescription_at`;
`crates/zerodds-c-api/src/extra_ffi.rs::zerodds_dp_lookup_topicdescription_at`;
`crates/cpp/include/dds/domain/detail/DomainParticipantDelegate.hpp::lookup_topicdescription`;
`crates/java-omgdds/java/src/main/java/org/zerodds/domain/DomainParticipantImpl.java::lookupTopicDescription`

**Tests:** `crates/dcps/tests/topics_of_one_name.rs::lookup_iterates_over_the_topics_of_a_name`,
`crates/zerodds-c-api/tests/builtin_types_c.rs::lookup_iterates_over_the_topics_of_a_name`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_operations_without_other_coverage`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/DomainParticipantOperationsTest.java::lookupTopicDescriptionWithAnIndexIteratesTheTopicsOfOneName`

**Status:** done — `lookup_topicdescription_at(name, index)` iterates over the topics of a name in the order they were created; an index outside the range is nil. The same holds for `zerodds_dp_lookup_topicdescription_at`, for `dp->lookup_topicdescription(name, index)` of the C++ PSM and for `DomainParticipantImpl.lookupTopicDescription(name, index)` of the Java PSM. The interfaces of the normative namespaces (C++ `dds::domain`, Java `org.omg.dds`) are unchanged; the extension is on the delegate and on the implementation.

---

## §7.6.5 Built-in Types

### §7.6.5 Four built-in types, registered at enable

**Spec:** §7.6.5, p. 247–248 — "DDS shall provide a few types preregistered 'out
of the box' [...] The built-in types shall be defined as in the following
sections and shall be automatically registered by the Service under their fully
qualified physical names (as above) with each DomainParticipant at the time it
is enabled."

**Repo:** `crates/dcps/src/participant.rs::register_builtin_types` (l. 820);
`crates/dcps/src/builtin_type_set.rs::build` (l. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::participant_after_create_has_four_builtin_types_registered`,
`crates/dcps/tests/builtin_types_auto_register_c44b.rs::builtin_types_are_not_nested`

**Status:** done — The four types are registered in a local type register when the participant is created; they are `@appendable` and not nested (Annex E).

### §7.6.5 The built-in types are not declared nested

**Spec:** §7.6.5, p. 248 — "Like all non-nested types used with DDS, the
built-in types shall have corresponding type-specific DataWriter and DataReader
classes."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`,
`crates/types/src/dynamic/builder.rs::tests::the_builtin_types_are_those_of_annex_e`

**Status:** done — `builtin_type_set.rs::build` declares the four types `@extensibility(APPENDABLE)` without `@nested`, with the member names of Annex E; `builtin_topic_types` in the `DynamicTypeBuilder` defines the same types with the same names.

### §7.6.5 Type-specific DataWriters and DataReaders with additional overloads

**Spec:** §7.6.5, p. 248 — "These shall instantiate the type-specific operations
defined by the DDS specification as defined in the following sections; they
shall also provide additional overloads."

**Repo:** `crates/zerodds-c-api/src/builtin_type_ffi.rs::zerodds_dp_create_builtin_topic`;
`crates/zerodds-c-api/src/builtin_type_ffi.rs::zerodds_pub_create_builtin_datawriter`;
`crates/dcps/src/builtin_types.rs`

**Tests:** `crates/dcps/tests/builtin_types_writers.rs::keyed_string_operations_work_on_the_key`,
`crates/dcps/tests/builtin_types_writers.rs::bytes_are_written_from_an_offset_and_a_length`,
`crates/dcps/tests/builtin_types_writers.rs::keyed_bytes_operations_work_on_the_key`,
`crates/zerodds-c-api/tests/builtin_types_c.rs::a_string_is_written_and_taken`,
`crates/zerodds-c-api/tests/builtin_types_c.rs::a_keyed_string_has_its_instance_operations`,
`crates/zerodds-c-api/tests/builtin_types_c.rs::bytes_are_written_from_an_offset_and_a_length`,
`crates/zerodds-c-api/src/builtin_type_ffi.rs::tests::a_byte_range_is_checked`,
`crates/cpp/tests/builtin_data_endpoints_via_cargo.rs::the_data_writers_and_readers_of_the_built_in_types_write_and_take`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java`

**Status:** done — `dcps::builtin_types` has the aliases `StringDataWriter` etc. and the operations of Annex E; the C ABI has them too (`zerodds_string_writer_write`, `zerodds_keyed_string_writer_*`, `zerodds_bytes_writer_write_w_bytes`, `zerodds_keyed_bytes_writer_*` and the readers `zerodds_*_reader_take_next`). In C++ the four writers and readers are classes over the C ABI (`crates/cpp/include/zerodds/builtin_data_endpoints.hpp`), in Java the interfaces `DataWriter<T>` and `DataReader<T>` of the Java PSM over `String`, `byte[]`, `KeyedString` and `KeyedBytes`. Log: `/root/zerodds-rc7/agent-logs/xt-bind-1018-051712.log` (Linux test host, `builtin_data_endpoints_via_cargo` green) and `.agent/REPORT.md`.

### §7.6.5.1 DDS::String

**Spec:** §7.6.5.1, p. 248 — "The DDS::String type is a simple structure wrapper
around a single unbounded string."

**Repo:** `crates/dcps/src/builtin_type_set.rs::build` (l. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::dds_string_has_unbounded_string_value_member`,
`crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`,
`crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`

**Status:** done — The structure is `@appendable` and not nested, with the members of Annex E; in `dcps::builtin_types` it is a `DdsType` (XCDR1 and XCDR2, both byte orders) with the type-specific operations (see §7.6.5).

### §7.6.5.2 DDS::KeyedString

**Spec:** §7.6.5.2, p. 248 — "The DDS::KeyedString type is similar to
DDS::String, but it is a keyed type; the key is an additional unbounded string."

**Repo:** `crates/dcps/src/builtin_type_set.rs::build` (l. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::find_builtin_type_returns_correct_kind`,
`crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`,
`crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`

**Status:** done — The structure is `@appendable` and not nested, with the members of Annex E; in `dcps::builtin_types` it is a `DdsType` (XCDR1 and XCDR2, both byte orders) with the type-specific operations (see §7.6.5).

### §7.6.5.3 DDS::Bytes

**Spec:** §7.6.5.3, p. 248 — "The DDS::Bytes type is a simple structure wrapper
around a single unbounded sequence of bytes."

**Repo:** `crates/dcps/src/builtin_type_set.rs::build` (l. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::dds_bytes_has_octet_sequence_value_member`,
`crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`,
`crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`

**Status:** done — The structure is `@appendable` and not nested, with the members of Annex E; in `dcps::builtin_types` it is a `DdsType` (XCDR1 and XCDR2, both byte orders) with the type-specific operations (see §7.6.5).

### §7.6.5.4 DDS::KeyedBytes

**Spec:** §7.6.5.4, p. 248 — "The DDS::KeyedBytes type is similar to DDS::Bytes,
but it is a keyed type; the key is an unbounded string."

**Repo:** `crates/dcps/src/builtin_type_set.rs::build` (l. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::dds_keyed_bytes_has_key_member`,
`crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`,
`crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`

**Status:** done — The structure is `@appendable` and not nested, with the members of Annex E; in `dcps::builtin_types` it is a `DdsType` (XCDR1 and XCDR2, both byte orders) with the type-specific operations (see §7.6.5).

---

## §7.6.6 Use of Dynamic Data and Dynamic Type

### §7.6.6 Register and use dynamic types with the DomainParticipant

**Spec:** §7.6.6, p. 249 — "The DynamicType interface can be used to construct
the Type and register it with the DomainParticipant. The DynamicData interface
can be used to create objects of a specified Type [...] and publish and
subscribe data objects of that type."

**Repo:** `crates/dcps/src/serialized_type.rs::SerializedTypeSupport::register_type`;
`crates/dcps/src/serialized_type.rs::create_type_support`;
`crates/dcps/src/participant.rs::create_registered_topic`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::register_type_registers_a_type_under_a_name`,
`crates/dcps/src/serialized_type.rs::tests::create_type_support_refuses_a_nested_type_and_a_non_aggregate`,
`crates/dcps/src/serialized_type.rs::tests::a_sample_converts_to_dynamic_data_and_back`,
`crates/dcps/tests/dynamic_data_writer_reader.rs::a_dynamic_topic_takes_writers_and_readers_of_dynamic_data`

**Status:** done — A type is made with `DynamicTypeBuilderFactory`, `SerializedTypeSupport::create_type_support` makes a type support of it with a copy of the type, `register_type(participant, type_name)` registers it with the participant (the TypeObjects go to the TypeLookup registry), `create_registered_topic` and `create_dynamic_topic` create topics of the type, and `DynamicDataWriter` and `DynamicDataReader` write and read `DynamicData`.

### §7.6.6.1 Conversion between static and dynamic binding through TypeSupport

**Spec:** §7.6.6.1, p. 249 — "it is desirable to allow conversions among static
and dynamic bindings for the same types and samples. These conversions shall be
provided by operations on the generic TypeSupport interface and its extended
interfaces."

**Repo:** `crates/dcps/src/serialized_type.rs::get_type`,
`crates/dcps/src/serialized_type.rs::create_sample`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::a_sample_converts_to_dynamic_data_and_back`

**Status:** done — `SerializedTypeSupport` converts between the static and the dynamic binding: `get_type`, `create_sample`, `create_dynamic_sample`.

### §7.6.6.1.1 Table 63 — TypeSupport::get_type

**Spec:** §7.6.6.1.1, Table 63, p. 249 — "get_type | DynamicType" — "Get a
DynamicType object corresponding to the TypeSupport's data type."

**Repo:** `crates/dcps/src/serialized_type.rs::get_type`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::create_type_support_refuses_a_nested_type_and_a_non_aggregate`

**Status:** done — `SerializedTypeSupport::get_type()` returns the `DynamicType` of the data type of the type support.

### §7.6.6.1.2 Table 64 — FooTypeSupport::create_sample

**Spec:** §7.6.6.1.2, Table 64, p. 249–250 — "Create a sample of the
TypeSupport's data type with the contents of an input DynamicData object. [...]
This method shall fail with a nil return result if this object is nil or if the
DynamicType of this object is not compatible".

**Repo:** `crates/dcps/src/serialized_type.rs::create_sample`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::a_sample_converts_to_dynamic_data_and_back`

**Status:** done — `create_sample(&DynamicSample)`: a sample with the contents; a `DynamicData` of another type is refused.

### §7.6.6.1.2 Table 64 — FooTypeSupport::create_dynamic_sample

**Spec:** §7.6.6.1.2, Table 64, p. 250 — "Create a DynamicData object with the
contents of an input sample of the TypeSupport's data type."

**Repo:** `crates/dcps/src/serialized_type.rs::create_dynamic_sample`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::a_sample_converts_to_dynamic_data_and_back`

**Status:** done — `create_dynamic_sample(&SerializedSample)`: a `DynamicSample` with the contents; a sample of another type is refused.

### §7.6.6.1.3 DynamicTypeSupport extends FooTypeSupport with Foo = DynamicData

**Spec:** §7.6.6.1.3, p. 250 — "The DynamicTypeSupport interface extends the
FooTypeSupport interface defined by the DDS specification where 'Foo' is the
type DynamicData."

**Repo:** `crates/dcps/src/serialized_type.rs::SerializedTypeSupport`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::register_type_registers_a_type_under_a_name`,
`crates/dcps/src/serialized_type.rs::tests::create_type_support_refuses_a_nested_type_and_a_non_aggregate`,
`crates/dcps/src/serialized_type.rs::tests::runtime_support_preserves_identity_before_instance_mutation`,
`crates/dcps/tests/dynamic_data_writer_reader.rs::a_dynamic_topic_takes_writers_and_readers_of_dynamic_data`

**Status:** done — `SerializedTypeSupport` is the `DynamicTypeSupport`: `get_type_name`, `register_type`, `get_type`, `create_sample`, `create_dynamic_sample`, the static `create_type_support` and `delete_type_support`; as a `TopicCodec<DynamicSample>` it is the FooTypeSupport for Foo = `DynamicData`, and `DynamicDataWriter` and `DynamicDataReader` are `DataWriter<DynamicSample>` and `DataReader<DynamicSample>`.

### §7.6.6.1.4 Table 65 — register_type, get_type_name

**Spec:** §7.6.6.1.4, Table 65, p. 250–251 — "register_type | ReturnCode_t |
participant DomainParticipant | type_name string<Char8,256> [...] get_type_name
| string<Char8,256>" — "These operations are defined by, and described in, the
DDS specification."

**Repo:** `crates/dcps/src/serialized_type.rs::register_type`,
`crates/dcps/src/serialized_type.rs::with_type_name`,
`crates/dcps/src/participant.rs::create_registered_topic`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::register_type_registers_a_type_under_a_name`,
`crates/dcps/src/serialized_type.rs::tests::create_type_support_refuses_a_nested_type_and_a_non_aggregate`

**Status:** done — `SerializedTypeSupport::register_type(&participant, type_name)` registers the type with the participant under its name or under `type_name` (`with_type_name`; `string<Char8,256>`) and puts its `TypeObject`s into the TypeLookup registry; the same again is no error, another type under a name in use is `PreconditionNotMet`. `DomainParticipant::create_registered_topic(name, type_name, qos)` creates a topic of the registered type and refuses a name that is not registered. `get_type_name()` returns the name the type is registered under.

### §7.6.6.1.5 Table 65 — create_type_support: copy of the type, nil or nested refused

**Spec:** §7.6.6.1.5, Table 65, p. 251 — "The implementation shall ensure that
the new type support has a 'copy' of the given type object [...] If this
argument is nil or is a nested type, the operation shall fail and return a nil
value."

**Repo:** `crates/dcps/src/serialized_type.rs::create_type_support`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::create_type_support_refuses_a_nested_type_and_a_non_aggregate`

**Status:** done — `create_type_support(&DynamicType)`: a type support with a copy of the type (built from its TypeObjects); a `@nested` type and a type that is no structure or union are refused.

### §7.6.6.1.6 Table 65 — delete_type_support

**Spec:** §7.6.6.1.6, Table 65, p. 251 — "Delete the given type support object
[...] If this argument is nil, the operation shall return successfully without
having any observable effect."

**Repo:** `crates/dcps/src/serialized_type.rs::delete_type_support`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::create_type_support_refuses_a_nested_type_and_a_non_aggregate`

**Status:** done — `delete_type_support(Option<Arc<..>>)`: nil has no effect, a type support is dropped with its last holder.

### §7.6.6.2 DynamicDataWriter and DynamicDataReader

**Spec:** §7.6.6.2, p. 251 — "The DynamicDataWriter interface instantiates the
FooDataWriter interface defined by the DDS specification where 'Foo' is the type
DynamicData."

**Repo:** `crates/dcps/src/serialized_type.rs::DynamicDataWriter`,
`crates/dcps/src/serialized_type.rs::DynamicDataReader`,
`crates/dcps/src/participant.rs::create_dynamic_topic`

**Tests:** `crates/dcps/tests/dynamic_data_writer_reader.rs::a_dynamic_topic_takes_writers_and_readers_of_dynamic_data`,
`crates/dcps/tests/dynamic_data_writer_reader.rs::linux_e2e::dynamic_data_goes_over_xcdr`,
`crates/dcps/tests/dynamic_data_writer_reader.rs::linux_e2e::dynamic_data_goes_over_xml`

**Status:** done — `DynamicDataWriter` is `DataWriter<DynamicSample>`, `DynamicDataReader` `DataReader<DynamicSample>` (`FooDataWriter` and `FooDataReader` with "Foo" = `DynamicData`), over a topic from `DomainParticipant::create_dynamic_topic`; they have no additional properties or operations. The key, the instance and the filters read the value of the `DynamicSample` (`DdsType for DynamicSample`, `TopicCodec<DynamicSample>`). The path with `SerializedSample` (`create_serialized_topic`) stays as it is.

---

## §7.6.7 DCPS Queries and Filters

### §7.6.7.1 Member Names: arrays and sequences: index in square brackets

**Spec:** §7.6.7.1, p. 252 — "Arrays and sequences: Elements in these ordered
collections shall be indicated by a zero-based subscript enclosed in square
brackets, e.g. my_collection[0]."

**Repo:** —

**Tests:** `crates/sql-filter/src/evaluator.rs::tests::subscripts_reach_the_row_as_written`,
`crates/sql-filter/src/path.rs::tests::subscripts_are_indices_and_keys`,
`crates/dcps/src/serialized_type.rs::tests::a_filter_subscripts_a_sequence_and_a_bitmask`

**Status:** done — The filter reads `a[0]`: an index in square brackets on an array or a sequence (`sql-filter::path::split_path`, `PathStep`); on a `SerializedTypeSupport` sample it reaches the elements.

### §7.6.7.1 Member Names: maps: key as STRING in square brackets

**Spec:** §7.6.7.1, p. 252 — "Maps: Value elements in these unordered
collections shall be indicated by a string representation of a corresponding key
element [...] e.g. my_map['key']."

**Repo:** `crates/dcps/src/serialized_type.rs::field_value`

**Tests:** `crates/sql-filter/src/evaluator.rs::tests::subscripts_reach_the_row_as_written`,
`crates/sql-filter/src/path.rs::tests::subscripts_are_indices_and_keys`,
`crates/dcps/src/serialized_type.rs::tests::a_filter_reaches_a_map_value_and_tells_an_absent_optional_member_from_no_field`

**Status:** done — The filter reads `a['key']`; on a `SerializedTypeSupport` sample the type decides that a collection is a map, and `m['key']` (for an integer key also `m[3]`) finds the value by its key.

### §7.6.7.1 Member Names: bitmasks: flag as ENUMERATEDVALUE in square brackets, Boolean

**Spec:** §7.6.7.1, p. 252 — "Bitmasks: A flag in a bitmask shall be indicated
by its name [...] enclosed in square brackets, e.g. my_bitmask['MY_FLAG']. Such
an expression shall be considered to have a Boolean type [...] Comparisons with
the integer literals 1 and 0 shall also be allowed."

**Repo:** —

**Tests:** `crates/sql-filter/src/evaluator.rs::tests::a_bitmask_flag_compares_with_true_and_with_one_and_zero`,
`crates/dcps/src/serialized_type.rs::tests::a_filter_subscripts_a_sequence_and_a_bitmask`

**Status:** done — A bitmask flag is read as `m['READ']`, is Boolean and is comparable with `TRUE`, 1 and 0.

### §7.6.7.2 Optional members against null

**Spec:** §7.6.7.2, p. 252 — "If the member is optional, and it takes no value
in the given object, it shall be considered equal to null. [...] No non-optional
member shall ever be considered equal to null. Inequalities expressed relative
to null shall never evaluate to true".

**Repo:** `crates/dcps/src/serialized_type.rs::field_value`,
`crates/zerodds-build/src/dds_type.rs::field_value_item`

**Tests:** `crates/sql-filter/src/evaluator.rs::tests::null_follows_the_rules_for_optional_members`,
`crates/dcps/src/serialized_type.rs::tests::a_filter_reaches_a_map_value_and_tells_an_absent_optional_member_from_no_field`,
`crates/zerodds-build/tests/dds_type.rs::an_absent_optional_member_is_null_to_a_filter`,
`crates/zerodds-build/tests/dds_type.rs::the_generated_filter_value_of_an_optional_member_is_null_or_the_value`

**Status:** done — The literal `null` applies to optional members as in §7.6.7.2: a member without a value equals `null`, one with a value does not, no inequality against `null` is true. The sample knows its type: an absent optional member is `null`, a name the type does not have is no field. The generated `field_value` of an optional scalar or string member returns the value or `null`.

### §7.6.7.3 Grammar: BOOLEANVALUE true or false, case-insensitive

**Spec:** §7.6.7.3, p. 253 — "The BOOLEANVALUE token shall be either true or
false (case-insensitive)."

**Repo:** `crates/sql-filter/src/lexer.rs::tokenize`

**Tests:** `crates/sql-filter/src/lexer.rs::tests::a_boolean_literal_is_true_or_false_in_any_case`,
`crates/sql-filter/src/evaluator.rs::tests::a_boolean_literal_compares_with_a_boolean_member_in_any_case`

**Status:** done — `true` and `false` are read as BOOLEANVALUE in any case (`true`, `TRUE`, `True`, `tRuE`) and evaluated in a comparison with a Boolean member; `null` is NULLVALUE.

### §7.6.7.3 Grammar: NULLVALUE null is read

**Spec:** §7.6.7.3, p. 253 — "The NULLVALUE token shall always be null."

**Repo:** —

**Tests:** `crates/sql-filter/src/lexer.rs::tests::null_is_a_token_of_its_own`

**Status:** done — The lexer knows `null` as a token of its own, the evaluator `Value::Null`.

### §7.6.7.3 Parameter production without INTEGERVALUE

**Spec:** §7.6.7.3, p. 252–253 — "Parameter ::= | CHARVALUE | FLOATVALUE |
STRING | ENUMERATEDVALUE | BOOLEANVALUE | NULLVALUE | PARAMETER".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the production begins with an empty
alternative, INTEGERVALUE of DDS Annex A is missing (defect of the
specification, listed in the findings file). Integers are read.

---

## §7.6.8 Interoperability of Keyed Topics

### §7.6.8 KeyHash by RTPS §9.6.3.8, amended

**Spec:** §7.6.8, p. 253 — "As described in [RTPS] Clause 9.6.3.8 [...] the key
hash for a given object of a keyed type is obtained by first serializing the
values of the key members in their declaration order. The algorithm described in
that clause shall be amended as described below."

**Repo:** `crates/wire-codec/src/encode.rs::key_holder` (l. 617);
`crates/ast-layout/src/keyhash.rs::key_holder_plan` (l. 41)

**Tests:** `crates/wire-conformance/tests/bytes.rs::the_key_hash_covers_the_key_member_and_not_the_others`

**Status:** done

### §7.6.8 Step 1: FooKeyHolder FINAL, only key members, sorted by member ID

**Spec:** §7.6.8, p. 253 — "Change FooKeyHolder extensibility kind to FINAL
[...] If there are any key members, then remove the non-key members from
FooKeyHolder. Otherwise do not remove any members. Reorder the members in
ascending order of their memberId values."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_members` (l. 254);
`crates/ast-layout/src/keyhash.rs::holder_keeps_only_keys` (l. 133)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::key_members_come_back_in_member_id_order_not_declaration_order`,
`crates/ast-layout/tests/wire_vectors.rs::a_type_with_no_key_member_has_no_key_members_and_a_whole_type_holder`,
`crates/wire-probe/tests/vendor_bytes.rs::a_key_holders_member_order_is_the_one_each_vendor_hashes`

**Status:** done

### §7.6.8 Steps 2 and 3: take over the values, recursively for nested aggregates

**Spec:** §7.6.8, p. 253 — "Step 2. Define a new object 'FooKeyHolderObject'
from the FooObject [...] Step 3. Apply steps 1 and 2 recursively to the members
of FooKeyHolder if they are themselves AggregatedTypes."

**Repo:** `crates/ast-layout/src/keyhash.rs::member_key` (l. 161);
`crates/wire-codec/src/encode.rs::key_holder` (l. 617)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::nested_keyed_members_contribute_their_own_keys`,
`crates/wire-probe/tests/silent_drop.rs::a_key_member_behind_a_typedef_keeps_its_nested_keys`

**Status:** done

### §7.6.8 Step 4: PLAIN_CDR2 big endian, alignment 4, zero padding

**Spec:** §7.6.8, p. 253 — "Compute the PLAIN_CDR2 Big Endian Serialization (see
7.4.2) of FooKeyHolderObject. The serialization shall be performed on a buffer
that is initially aligned to the maximum alignment in PLAIN_CDR2 (i.e., 4).
Furthermore, any padding bytes added due to alignment rules shall be set to
zero."

**Repo:** `crates/wire-codec/src/encode.rs::key_holder` (l. 617)

**Tests:** `crates/wire-conformance/tests/bytes.rs::the_key_hash_is_big_endian_whatever_the_sample_is`,
`crates/ast-layout/tests/wire_vectors.rs::v8_keyed_sensor_plans_a_four_byte_zero_padded_key_holder`

**Status:** done — the default `KeyHolderAlignment::Aligned` caps at 4; other
values are dialects.

### §7.6.8 Step 5.1: at most 16 octets, padded to 16 with zeros

**Spec:** §7.6.8, p. 253 — "If the FooKeyHolder has a maximum serialized size
that is less than or equal to 16 bytes, then then the KeyHash of FooObject shall
be set to the result of Step 4, extended to 16 bytes."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_hash_form` (l. 85)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::the_key_hash_of_a_small_key_is_the_zero_padded_holder`,
`crates/dcps/src/dds_type.rs::tests::small_keyed_produces_zero_padded_keyhash`

**Status:** done

### §7.6.8 Step 5.2: more than 16 octets, MD5

**Spec:** §7.6.8, p. 253 — "If the FooKeyHolder has a maximum serialized size
that is greater than 16 bytes, then the KeyHash of FooObject shall be set to the
MD5 Hash of the serialized bytes obtained from Step 4."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_hash_form` (l. 85)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::a_key_holder_past_sixteen_octets_is_md5_hashed`,
`crates/ast-layout/tests/wire_vectors.rs::a_key_holder_over_sixteen_bytes_is_hashed`,
`crates/ast-layout/tests/key_maximum.rs::bounded_string_threshold_is_maximum_not_fixed_extent`

**Status:** done — what counts is the largest possible size of the holder, not
that of the sample.

### §7.6.8 Without encapsulation, type and member headers

**Spec:** §7.6.8, p. 254 — "the serialized bytes obtained in step 4 do not
include any encapsulation header, type header, or member headers and use a
maximum alignment of 4."

**Repo:** `crates/wire-codec/src/encode.rs::key_holder` (l. 617)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v8_keyed_sensor_plans_a_four_byte_zero_padded_key_holder`

**Status:** done

### §7.6.8 Example 1: @key long id

**Spec:** §7.6.8, p. 254 — KeyHash `{ 0x12, 0x34, 0x56, 0x78, 0x00, ... }`.

**Repo:** `crates/wire-codec/src/encode.rs::key_hash`

**Tests:** `crates/wire-codec/tests/keyhash_xtypes_7_6_8_examples.rs::example_1_a_four_octet_key_is_padded_with_zeros`

**Status:** done — The test computes example 1 with the values of the specification (`id = 0x12345678`, `x = 10`, `y = 20`): key stream `12 34 56 78`, KeyHash `12 34 56 78` and twelve zeros.

### §7.6.8 Example 2: string<12> label and long long id, MD5

**Spec:** §7.6.8, p. 254–255 — KeyHash `{ 0xf9, 0x1a, 0x59, 0xe3, ... }`.

**Repo:** `crates/wire-codec/src/encode.rs::key_hash`

**Tests:** `crates/wire-codec/tests/keyhash_xtypes_7_6_8_examples.rs::example_2_a_key_of_at_most_28_octets_is_hashed_with_md5`

**Status:** done — The test computes example 2 (`label = "BLUE"`, `id = 0x123456789abcdef0`): the stream has the 20 octets of the specification, the KeyHash is `f9 1a 59 e3 2e 45 35 d9 a6 9c d5 d9 f5 b6 e3 6e` (MD5, the maximum size 28 is above 16).

### §7.6.8 Example 3: @mutable with a nested key, MD5

**Spec:** §7.6.8, p. 255–257 — KeyHash `{ 0x37, 0x4b, 0x96, 0xe2, ... }`.

**Repo:** `crates/wire-codec/src/encode.rs::key_hash`

**Tests:** `crates/wire-codec/tests/keyhash_xtypes_7_6_8_examples.rs::example_3_the_key_members_are_reordered_by_member_id_and_nested_keys_reduced`

**Status:** done — The test computes example 3 (`@mutable`, `label` with `@id(40)`, `m_nested` with `@id(30)`): the key members are ordered by member id, the nested key is reduced to `m_long`, the stream has the 13 octets of the specification, the KeyHash is `37 4b 96 e2 e7 27 23 7f 01 6c c4 ce bb 6e b7 1e`. `Foo` declares `string<16>`, the specification's KeyHolder `string<12>` (contradiction in the example, in the findings file); the hash does not depend on the bound as long as the stream is above 16 octets.

### §7.6.8 Dispose with serializedKey: instance from the key, not from PID_KEY_HASH

**Spec:** §7.6.8, p. 253 — "the key hash for a given object of a keyed type is
obtained by first serializing the values of the key members".

**Repo:** `crates/dcps/src/subscriber.rs::lifecycle_instance` (l. 4638)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::a_dispose_with_a_foreign_key_hash_disposes_the_instance_of_its_key`

**Status:** done — the reader decodes the `serializedKey` of a dispose or
unregister and forms the KeyHash itself; only a marker without a `serializedKey`
keeps the transmitted hash.

---

## Annex A: XML Type Representation Schema

### Annex A Schema: root element types, namespace

**Spec:** Annex A, p. 259 — XSD of the XML Type Representation with the element
`types` of type `typeLibrary`.

**Repo:** `crates/xml/schemas/dds-xtypes_type_definition.xsd`;
`crates/xml/schemas/dds-xtypes_type_definition_nonamespace.xsd`;
`crates/xml/src/schemas.rs`

**Tests:** `crates/xml/tests/annex_a_types.rs::annex_a_schema_compiles_and_rejects_what_it_should`,
`crates/xml/tests/annex_a_types.rs::documents_annex_a_accepts_are_read`,
`crates/xml/tests/annex_a_types.rs::documents_annex_a_rejects_are_rejected`,
`crates/xml/tests/annex_a_types.rs::a_document_in_the_xtypes_namespace_is_read_by_the_annex_a_schema`,
`crates/xml/src/schemas.rs::tests::annex_a_schema_has_root_element_types_in_the_xtypes_namespace`

**Status:** done — The two XSD files of Annex A (namespace `http://www.omg.org/dds`, root element `types` of type `typeLibrary`, and the file without a namespace) are in the tree (`XTYPES_TYPES_XSD`, `XTYPES_TYPES_NONAMESPACE_XSD`; four typing slips of the PDF in `xs:attribute` are closed). Documents the schema accepts are read by `zerodds-xml`; documents it rejects are rejected by `zerodds-xml` (`xmllint` on the Linux test host). An element in the namespace of Annex A is read by its schema (`appendable`, not `append`), otherwise by the schema of DDS-XML 1.0.

### Annex A Schema: identifierName, fileName

**Spec:** Annex A, p. 259–260 — `xs:simpleType name="identifierName"`,
`xs:simpleType name="fileName"`.

**Repo:** `crates/xml/src/xtypes_parser.rs::is_identifier_name`;
`crates/xml-compose/src/lower.rs::check_identifier_names`

**Tests:** `crates/xml/tests/annex_a_types.rs::identifier_name_is_the_pattern_of_annex_a`,
`crates/xml/tests/annex_a_types.rs::a_name_that_is_no_identifier_name_is_refused`,
`crates/xml/tests/annex_a_types.rs::a_file_name_is_any_string`,
`crates/xml-compose/tests/annex_a_declarations.rs::a_name_that_is_no_identifier_name_is_an_error`

**Status:** done — `name`, `baseType`, `nonBasicTypeName` and `mapKeyNonBasicTypeName` are checked against `([a-zA-Z]|::)([a-zA-Z_0-9]|::)*` in both readers; a name outside it names the attribute and the element. `fileName` is any string.

### Annex A Schema: includeDecl

**Spec:** Annex A, p. 260 — `xs:complexType name="includeDecl"` with attribute
`file`.

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_include`,
`crates/xml-compose/src/lower.rs::load_units`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_include_element`,
`crates/xml/src/xtypes_parser.rs::tests::parse_include_missing_file_attribute_rejected`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_include_brings_the_named_document_into_the_model_once`

**Status:** done — `zerodds-xml` reads the entry, `zerodds-xml-compose` follows `file` (§7.3.2.1.1).

### Annex A Schema: forwardDecl, forwardDeclTypeKind enum/struct/union

**Spec:** Annex A, p. 260 — `xs:simpleType name="forwardDeclTypeKind"` with
`enum`, `struct`, `union`; `xs:complexType name="forwardDecl"`.

**Repo:** `crates/xml-compose/src/lower.rs::forward_declaration` (l. 630);
`crates/xml/src/xtypes_parser.rs::parse_forward_dcl` (l. 167)

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_forward_dcl_element`,
`crates/xml/src/xtypes_parser.rs::tests::parse_forward_dcl_default_kind_struct`

**Status:** done

### Annex A Schema: allTypeKind

**Spec:** Annex A, p. 261 — `xs:simpleType name="allTypeKind"`: `boolean`,
`byte`, `char8`, `char16`, `int8`, `uint8`, `int16`, `uint16`, `int32`,
`uint32`, `int64`, `uint64`, `float32`, `float64`, `float128`, `string`,
`wstring`, `nonBasic`.

**Repo:** `crates/xml-compose/src/lower.rs::primitive_of` (l. 1449)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::every_primitive_reaches_the_same_type_from_both_front_ends`,
`crates/xml-compose/src/lower.rs::tests::the_two_primitive_vocabularies_agree`

**Status:** done — the IDL names (`long`, `octet` and others) in addition.

### Annex A Schema: arrayDimensionsKind

**Spec:** Annex A, p. 261 — `xs:simpleType name="arrayDimensionsKind"`.

**Repo:** `crates/xml-compose/src/lower.rs::dimensions`

**Tests:** `crates/xml-compose/tests/idl_parity.rs::arrays_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/constants_and_annotations.rs::bounds_are_const_expressions_over_constants`

**Status:** done — The dimensions are a list of `<const_exp>`, separated by commas.

### Annex A Schema: constDecl

**Spec:** Annex A, p. 262 — `xs:complexType name="constDecl"` with `name`,
`type`, `nonBasicTypeName`, `value`.

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_const`,
`crates/xml-compose/src/lower.rs::constant`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_const_element`,
`crates/xml/src/xtypes_parser.rs::tests::parse_const_missing_value_attribute_rejected`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_constant_is_found_whichever_side_of_its_use_it_is_declared`

**Status:** done — `zerodds-xml` reads constants, `zerodds-xml-compose` puts them into the model with type and expression (§7.3.2.1.3).

### Annex A Schema: memberId, simpleMemberDecl, memberDecl with bound and map attributes

**Spec:** Annex A, p. 262–263 — `simpleMemberDecl` (`name`, `type`,
`nonBasicTypeName`), `memberDecl` (`annotate`, `external`, `tryConstruct`,
`mapKeyType`, `mapKeyNonBasicTypeName`, `stringMaxLength`,
`mapKeyStringMaxLength`, `sequenceMaxLength`, `mapMaxLength`,
`arrayDimensions`).

**Repo:** `crates/xml-compose/src/lower.rs::member_spec`;
`crates/xml/src/xtypes_parser.rs::parse_member`

**Tests:** `crates/xml/tests/annex_a_types.rs::member_id_is_between_zero_and_0x0fffffff`,
`crates/xml/tests/annex_a_types.rs::a_struct_declaration_keeps_everything_annex_a_lets_it_say`,
`crates/xml/tests/annex_a_types.rs::an_annotation_type_and_its_application_are_read`,
`crates/xml-compose/tests/idl_parity.rs::string_bounds_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/idl_parity.rs::sequences_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/idl_parity.rs::try_construct_reaches_the_same_type_from_both_front_ends`

**Status:** done — `memberId` is between 0 and 0x0FFFFFFF (an error outside); `simpleMemberDecl` (`name`, `type`, `nonBasicTypeName`) and `memberDecl` (`annotate`, `external`, `tryConstruct`, `mapKeyType`, `mapKeyNonBasicTypeName`, `stringMaxLength`, `mapKeyStringMaxLength`, `sequenceMaxLength`, `mapMaxLength`, `arrayDimensions`) are read by both readers.

### Annex A Schema: tryConstructKind discard, use_default, trim

**Spec:** Annex A, p. 263 — `xs:simpleType name="tryConstructKind"` with
`discard`, `use_default`, `trim`.

**Repo:** `crates/xml-compose/src/lower.rs::try_construct` (l. 972)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::try_construct_reaches_the_same_type_from_both_front_ends`

**Status:** done

### Annex A Schema: verbatimDecl

**Spec:** Annex A, p. 264 — `xs:complexType name="verbatimDecl"` with `text`,
`language`, `placement`.

**Repo:** `crates/xml-compose/src/lower.rs::verbatim_directives`;
`crates/xml/src/xtypes_parser.rs::parse_verbatim`

**Tests:** `crates/xml/tests/normative_forms.rs::annotations_verbatims_and_flags_are_read`,
`crates/xml-compose/tests/constants_and_annotations.rs::verbatim_text_reaches_the_type`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_verbatim_placement_that_is_none_of_the_six_is_reported`

**Status:** done — `verbatimDecl` (`text`, `language`, `placement`) is read by both readers; `language` is `*` and `placement` `before-declaration` when absent.

### Annex A Schema: extensibilityKind final, appendable, mutable

**Spec:** Annex A, p. 264 — `xs:simpleType name="extensibilityKind"` with
`final`, `appendable`, `mutable`.

**Repo:** `crates/xml-compose/src/lower.rs::extensibility_of` (l. 1480)

**Tests:** `crates/xml/tests/types_and_samples.rs::t03_extensibility_final_appendable_mutable`,
`crates/xml-compose/tests/loader.rs::the_default_extensibility_only_reaches_types_the_document_left_open`

**Status:** done — `extensible` for appendable in addition.

### Annex A Schema: autoIdKind hash, sequencial is read

**Spec:** Annex A, p. 265 — `xs:simpleType name="autoIdKind"` with `hash` and
`sequencial`.

**Repo:** `crates/xml-compose/src/lower.rs::record`

**Tests:** `crates/xml-compose/tests/annex_a_attributes.rs::autoid_reads_annex_as_spelling`

**Status:** done — `record` accepts `hash`, `sequential` and the spelling `sequencial` of Annex A.

### Annex A Schema: structOrUnionTypeDecl with nested, extensibility, autoid

**Spec:** Annex A, p. 265 — `structOrUnionTypeDecl` (`annotate`, `verbatim`,
`name`, `nested`, `extensibility`, `autoid`).

**Repo:** `crates/xml-compose/src/lower.rs::record`;
`crates/xml/src/xtypes_parser.rs::parse_struct`

**Tests:** `crates/xml/tests/annex_a_types.rs::a_struct_declaration_keeps_everything_annex_a_lets_it_say`,
`crates/xml/tests/normative_forms.rs::annotations_verbatims_and_flags_are_read`,
`crates/xml-compose/tests/idl_parity.rs::member_ids_reach_the_same_numbers_from_both_front_ends`

**Status:** done — `nested`, `extensibility`, `autoid`, `annotate` and `verbatim` on a structure and a union are read by both readers; `autoid="sequencial"` and `sequential` are both accepted.

### Annex A Schema: annotationTypeDecl, annotationMemberValueDecl, annotationDecl

**Spec:** Annex A, p. 266 — `annotationTypeDecl` (`member`, `name`, `baseType`),
`annotationMemberValueDecl`, `annotationDecl`.

**Repo:** `crates/xml-compose/src/lower.rs::annotation_declaration`;
`crates/xml/src/xtypes_parser.rs::parse_annotation_type`

**Tests:** `crates/xml/tests/annex_a_types.rs::an_annotation_type_and_its_application_are_read`,
`crates/xml-compose/tests/annex_a_declarations.rs::an_annotation_with_a_base_type_has_the_members_of_its_base_first`,
`crates/xml-compose/tests/annex_a_declarations.rs::an_annotation_base_that_is_not_declared_is_an_error`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_annotation_is_declared_and_applied`

**Status:** done — `annotationTypeDecl` (`member`, `name`, `baseType`), `annotationMemberValueDecl` and `annotationDecl` are read by both readers. The model has no base for annotation types; `zerodds-xml-compose` puts the members of the base before its own (IDL 4.2 §7.4.15.3), and a base that is not declared before is an error.

### Annex A Schema: structMemberDecl with id, optional, mustUnderstand, nonSerialized, key

**Spec:** Annex A, p. 267 — `structMemberDecl` (`id`, `optional`,
`mustUnderstand`, `nonSerialized`, `key`).

**Repo:** `crates/xml-compose/src/lower.rs::member_spec`;
`crates/xml/src/xtypes_parser.rs::parse_member`

**Tests:** `crates/xml/tests/annex_a_types.rs::a_struct_declaration_keeps_everything_annex_a_lets_it_say`,
`crates/xml/tests/types_and_samples.rs::t05_struct_member_key_true`,
`crates/xml/tests/types_and_samples.rs::t06_struct_member_optional_true`,
`crates/xml/tests/types_and_samples.rs::t07_struct_member_id_override`,
`crates/xml-compose/tests/idl_parity.rs::keys_reach_the_same_type_from_both_front_ends`

**Status:** done — `id`, `optional`, `mustUnderstand`, `nonSerialized` and `key` are read by both readers (`zerodds-xml` with `non_serialized`).

### Annex A Schema: structDecl with member, const, baseType

**Spec:** Annex A, p. 268 — `structDecl` (`member`, `const`, `baseType`).

**Repo:** `crates/xml-compose/src/lower.rs::structure`;
`crates/xml/src/xtypes_parser.rs::parse_struct`

**Tests:** `crates/xml/tests/annex_a_types.rs::a_struct_declaration_keeps_everything_annex_a_lets_it_say`,
`crates/xml/tests/types_and_samples.rs::t04_struct_inheritance_basetype`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_constant_inside_a_structure_is_in_its_scope`

**Status:** done — `structDecl` (`member`, `const`, `baseType`) is read by both readers; `zerodds-xml` puts the constants of the structure into `StructType::constants`.

### Annex A Schema: unionDecl, discriminatorDecl, caseDiscriminatorDecl, caseDecl

**Spec:** Annex A, p. 268–269 — `unionMemberDecl`, `discriminatorDecl`
(`annotate`, `type`, `nonBasicTypeName`, `key`), `caseDiscriminatorDecl`
(`value`), `caseDecl`, `unionDecl` (`discriminator`, `case`).

**Repo:** `crates/xml-compose/src/lower.rs::union` (l. 308);
`crates/xml/src/xtypes_parser.rs::parse_union` (l. 315)

**Tests:** `crates/xml/tests/types_and_samples.rs::t12_union_with_cases`,
`crates/xml/tests/types_and_samples.rs::t13_union_with_default_case`,
`crates/xml-compose/tests/idl_parity.rs::unions_reach_the_same_type_from_both_front_ends`

**Status:** done — a `key` on the discriminator is refused by default
(§7.2.2.4.4.4.8).

### Annex A Schema: typedefDecl

**Spec:** Annex A, p. 270 — `typedefDecl` (`name`, `type`, `nonBasicTypeName`,
bound attributes, `arrayDimensions`, `external`).

**Repo:** `crates/xml-compose/src/lower.rs::alias` (l. 588);
`crates/xml/src/xtypes_parser.rs::parse_typedef` (l. 356)

**Tests:** `crates/xml/tests/types_and_samples.rs::t15_typedef_simple_and_array`

**Status:** done

### Annex A Schema: enumBitBound, enumeratorDecl with value and defaultLiteral, enumDecl

**Spec:** Annex A, p. 271–272 — `enumBitBound`, `enumeratorDecl` (`annotate`,
`name`, `value`, `defaultLiteral`), `enumDecl` (`annotate`, `verbatim`,
`enumerator`, `name`, `bitBound`).

**Repo:** `crates/xml-compose/src/lower.rs::enumeration`;
`crates/xml/src/xtypes_parser.rs::parse_enum`

**Tests:** `crates/xml/tests/annex_a_types.rs::bit_bounds_and_flag_positions_are_within_their_ranges`,
`crates/xml/tests/types_and_samples.rs::t10_enum_with_explicit_values`,
`crates/xml-compose/tests/idl_parity.rs::a_narrow_bit_bound_reaches_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_enumerator_value_is_a_const_expression`

**Status:** done — `enumBitBound` (1 to 32), `bitmaskBitBound` (1 to 64) and `flagIndex` (0 to 63) are checked, the `position` of a flag is required; `enumeratorDecl` (`annotate`, `name`, `value`, `defaultLiteral`) and `enumDecl` (`annotate`, `verbatim`, `enumerator`, `name`, `bitBound`) are read by both readers.

### Annex A Schema: bitmaskBitBound, flagIndex, flagDecl with position, bitmaskDecl

**Spec:** Annex A, p. 273–274 — `bitmaskBitBound`, `flagIndex`, `flagDecl`
(`annotate`, `name`, `position`), `bitmaskDecl` (`annotate`, `flag`, `name`,
`bitBound`).

**Repo:** `crates/xml-compose/src/lower.rs::bitmask` (l. 534);
`crates/xml/src/xtypes_parser.rs::parse_bitmask` (l. 376)

**Tests:** `crates/xml/tests/types_and_samples.rs::t14_bitmask_and_bitset`,
`crates/xml-compose/tests/loader.rs::both_bitmask_spellings_reach_the_same_bits`

**Status:** done

### Annex A Schema: moduleElements, moduleDecl, typeLibrary

**Spec:** Annex A, p. 274–276 — `moduleElements` (`include`, `forward_dcl`,
`const`, `module`, `struct`, `union`, `annotation`, `typedef`, `enum`,
`bitmask`), `moduleDecl` (`name`, `autoid`), `typeLibrary`.

**Repo:** `crates/xml-compose/src/lower.rs::module`;
`crates/xml/src/xtypes_parser.rs::parse_module`

**Tests:** `crates/xml/tests/annex_a_types.rs::a_module_holds_every_module_element_in_any_order`,
`crates/xml-compose/tests/annex_a_declarations.rs::a_module_autoid_governs_the_types_declared_in_it`,
`crates/xml/tests/types_and_samples.rs::t16_module_three_levels_deep`,
`crates/xml-compose/tests/loader.rs::two_documents_reopen_one_module`

**Status:** done — `moduleElements` (`include`, `forward_dcl`, `const`, `module`, `struct`, `union`, `annotation`, `typedef`, `enum`, `bitmask`), `moduleDecl` with `autoid` and `typeLibrary` are read by both readers; `autoid` on a module governs the types in it that have none (§7.3.1.2.1.1). `bitset` is not in `moduleElements`; both readers read it as a vendor form.

---

## Annex B: Representing Types with TypeObject

### Annex B Serialization with XCDR version 2

**Spec:** Annex B, p. 277 — "// The types in this file shall be serialized with
XCDR encoding version 2".

**Repo:** `crates/types/src/type_object/mod.rs::TypeObject` (l. 36);
`crates/types/src/hash.rs`

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::a_type_object_opens_with_a_dheader_over_the_rest`

**Status:** done

### Annex B EquivalenceKind EK_MINIMAL = 0xF1

**Spec:** Annex B, p. 277 — "const octet EK_MINIMAL = 0xF1;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::EK_MINIMAL` (l. 90)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::equivalence_kind_roundtrip`,
`crates/types/src/type_identifier/mod.rs::tests::equivalence_hash_wire_is_discriminator_plus_14_bytes`

**Status:** done

### Annex B EquivalenceKind EK_COMPLETE = 0xF2

**Spec:** Annex B, p. 277 — "const octet EK_COMPLETE = 0xF2;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::EK_COMPLETE` (l. 92)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::equivalence_kind_roundtrip`,
`crates/types/src/type_identifier/mod.rs::tests::equivalence_hash_wire_is_discriminator_plus_14_bytes`

**Status:** done

### Annex B EquivalenceKind EK_BOTH = 0xF3

**Spec:** Annex B, p. 277 — "const octet EK_BOTH = 0xF3;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::EK_BOTH` (l. 98)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::equivalence_kind_roundtrip`,
`crates/types/src/type_identifier/mod.rs::tests::equivalence_hash_wire_is_discriminator_plus_14_bytes`

**Status:** done

### Annex B TypeKind TK_NONE = 0x00

**Spec:** Annex B, p. 277–278 — "const octet TK_NONE = 0x00;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_NONE` (l. 19)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_BOOLEAN = 0x01

**Spec:** Annex B, p. 277–278 — "const octet TK_BOOLEAN = 0x01;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_BOOLEAN` (l. 21)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_BYTE = 0x02

**Spec:** Annex B, p. 277–278 — "const octet TK_BYTE = 0x02;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_BYTE` (l. 23)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_INT16 = 0x03

**Spec:** Annex B, p. 277–278 — "const octet TK_INT16 = 0x03;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_INT16` (l. 25)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_INT32 = 0x04

**Spec:** Annex B, p. 277–278 — "const octet TK_INT32 = 0x04;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_INT32` (l. 27)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::primitive_int32_discriminator_is_spec_value`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done

### Annex B TypeKind TK_INT64 = 0x05

**Spec:** Annex B, p. 277–278 — "const octet TK_INT64 = 0x05;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_INT64` (l. 29)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_UINT16 = 0x06

**Spec:** Annex B, p. 277–278 — "const octet TK_UINT16 = 0x06;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_UINT16` (l. 31)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_UINT32 = 0x07

**Spec:** Annex B, p. 277–278 — "const octet TK_UINT32 = 0x07;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_UINT32` (l. 33)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_UINT64 = 0x08

**Spec:** Annex B, p. 277–278 — "const octet TK_UINT64 = 0x08;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_UINT64` (l. 35)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_FLOAT32 = 0x09

**Spec:** Annex B, p. 277–278 — "const octet TK_FLOAT32 = 0x09;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_FLOAT32` (l. 37)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_FLOAT64 = 0x0A

**Spec:** Annex B, p. 277–278 — "const octet TK_FLOAT64 = 0x0A;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_FLOAT64` (l. 39)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_FLOAT128 = 0x0B

**Spec:** Annex B, p. 277–278 — "const octet TK_FLOAT128 = 0x0B;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_FLOAT128` (l. 41)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_INT8 = 0x0C

**Spec:** Annex B, p. 277–278 — "const octet TK_INT8 = 0x0C;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_INT8` (l. 43)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_UINT8 = 0x0D

**Spec:** Annex B, p. 277–278 — "const octet TK_UINT8 = 0x0D;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_UINT8` (l. 45)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_CHAR8 = 0x10

**Spec:** Annex B, p. 277–278 — "const octet TK_CHAR8 = 0x10;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_CHAR8` (l. 47)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_CHAR16 = 0x11

**Spec:** Annex B, p. 277–278 — "const octet TK_CHAR16 = 0x11;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_CHAR16` (l. 49)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_STRING8 = 0x20

**Spec:** Annex B, p. 277–278 — "const octet TK_STRING8 = 0x20;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_STRING8` (l. 54)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_STRING16 = 0x21

**Spec:** Annex B, p. 277–278 — "const octet TK_STRING16 = 0x21;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_STRING16` (l. 56)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — the value is read from the code; the round trip checks the
mapping, not the number. The octet tests against Cyclone contain the identifiers
of their types' members.

### Annex B TypeKind TK_ALIAS = 0x30

**Spec:** Annex B, p. 278 — "const octet TK_ALIAS = 0x30;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_ALIAS` (l. 15)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — discriminator of `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_ENUM = 0x40

**Spec:** Annex B, p. 278 — "const octet TK_ENUM = 0x40;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_ENUM` (l. 18)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — discriminator of `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_BITMASK = 0x41

**Spec:** Annex B, p. 278 — "const octet TK_BITMASK = 0x41;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_BITMASK` (l. 20)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — discriminator of `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_ANNOTATION = 0x50

**Spec:** Annex B, p. 278 — "const octet TK_ANNOTATION = 0x50;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_ANNOTATION` (l. 23)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — discriminator of `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_STRUCTURE = 0x51

**Spec:** Annex B, p. 278 — "const octet TK_STRUCTURE = 0x51;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_STRUCTURE` (l. 26)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — discriminator of `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_UNION = 0x52

**Spec:** Annex B, p. 278 — "const octet TK_UNION = 0x52;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_UNION` (l. 28)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — discriminator of `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_BITSET = 0x53

**Spec:** Annex B, p. 278 — "const octet TK_BITSET = 0x53;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_BITSET` (l. 30)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — discriminator of `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_SEQUENCE = 0x60

**Spec:** Annex B, p. 278 — "const octet TK_SEQUENCE = 0x60;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_SEQUENCE` (l. 33)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — discriminator of `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_ARRAY = 0x61

**Spec:** Annex B, p. 278 — "const octet TK_ARRAY = 0x61;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_ARRAY` (l. 35)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — discriminator of `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_MAP = 0x62

**Spec:** Annex B, p. 278 — "const octet TK_MAP = 0x62;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_MAP` (l. 37)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — discriminator of `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeIdentifier TI_STRING8_SMALL = 0x70

**Spec:** Annex B, p. 278 — "const octet TI_STRING8_SMALL = 0x70;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING8_SMALL` (l. 63)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string8_small_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_STRING8_LARGE = 0x71

**Spec:** Annex B, p. 278 — "const octet TI_STRING8_LARGE = 0x71;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING8_LARGE` (l. 65)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string8_large_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_STRING16_SMALL = 0x72

**Spec:** Annex B, p. 278 — "const octet TI_STRING16_SMALL = 0x72;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING16_SMALL` (l. 67)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string16_small_and_large_roundtrip`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_STRING16_LARGE = 0x73

**Spec:** Annex B, p. 278 — "const octet TI_STRING16_LARGE = 0x73;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING16_LARGE` (l. 69)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string16_small_and_large_roundtrip`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_SEQUENCE_SMALL = 0x80

**Spec:** Annex B, p. 278 — "const octet TI_PLAIN_SEQUENCE_SMALL = 0x80;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_SEQUENCE_SMALL` (l. 72)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_sequence_of_int32_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_SEQUENCE_LARGE = 0x81

**Spec:** Annex B, p. 278 — "const octet TI_PLAIN_SEQUENCE_LARGE = 0x81;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_SEQUENCE_LARGE` (l. 74)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_sequence_large_of_string_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_ARRAY_SMALL = 0x90

**Spec:** Annex B, p. 278 — "const octet TI_PLAIN_ARRAY_SMALL = 0x90;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_ARRAY_SMALL` (l. 77)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_array_small_3d_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_ARRAY_LARGE = 0x91

**Spec:** Annex B, p. 278 — "const octet TI_PLAIN_ARRAY_LARGE = 0x91;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_ARRAY_LARGE` (l. 79)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_array_large_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_MAP_SMALL = 0xA0

**Spec:** Annex B, p. 278 — "const octet TI_PLAIN_MAP_SMALL = 0xA0;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_MAP_SMALL` (l. 82)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_map_small_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_MAP_LARGE = 0xA1

**Spec:** Annex B, p. 278 — "const octet TI_PLAIN_MAP_LARGE = 0xA1;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_MAP_LARGE` (l. 84)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_map_large_with_nested_map_value_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_STRONGLY_CONNECTED_COMPONENT = 0xB0

**Spec:** Annex B, p. 279 — "const octet TI_STRONGLY_CONNECTED_COMPONENT =
0xB0;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRONGLY_CONNECTED_COMPONENT` (l. 87)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::a_component_identifier_is_the_thirty_two_octets_of_annex_b`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B MEMBER_NAME_MAX_LENGTH and TYPE_NAME_MAX_LENGTH = 256 are checked

**Spec:** Annex B, p. 279 — "const long MEMBER_NAME_MAX_LENGTH = 256; typedef
string<MEMBER_NAME_MAX_LENGTH> MemberName; [...] const long TYPE_NAME_MAX_LENGTH
= 256; typedef string<TYPE_NAME_MAX_LENGTH> QualifiedTypeName;"

**Repo:** —

**Tests:** `crates/types/tests/annex_b_limits_and_masks.rs::the_limits_are_the_constants_of_annex_b`,
`crates/types/tests/annex_b_limits_and_masks.rs::a_member_name_holds_at_most_256_octets`,
`crates/types/tests/annex_b_limits_and_masks.rs::a_member_name_over_256_octets_is_refused_on_reading`,
`crates/types/tests/annex_b_limits_and_masks.rs::a_type_name_holds_at_most_256_octets`,
`crates/types/tests/annex_b_limits_and_masks.rs::an_object_with_a_long_type_name_does_not_encode`

**Status:** done — A member or type name above 256 octets is refused on writing and on reading; the constants are in `type_object::common`.

### Annex B EquivalenceHash[14], NameHash[4]

**Spec:** Annex B, p. 279 — "typedef octet EquivalenceHash[14]; [...] typedef
octet NameHash[4];"

**Repo:** `crates/types/src/type_identifier/mod.rs::EquivalenceHash` (l. 54);
`crates/types/src/type_object/common.rs::NameHash` (l. 26)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::equivalence_hash_wire_is_discriminator_plus_14_bytes`,
`crates/types/tests/compliance_typeobject.rs::md5_empty_string_first_14_bytes_golden_vector`

**Status:** done

### Annex B LBound, SBound, INVALID_LBOUND, INVALID_SBOUND

**Spec:** Annex B, p. 279 — "typedef unsigned long LBound; [...] const LBound
INVALID_LBOUND = 0; [...] typedef octet SBound; [...] const SBound
INVALID_SBOUND = 0;"

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (l. 197)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_sequence_large_with_u32_max_bound_roundtrips`,
`crates/types/src/type_identifier/mod.rs::tests::string8_small_roundtrips`

**Status:** done — 0 stands for unbounded.

### Annex B TypeObjectHashId

**Spec:** Annex B, p. 280 — "union TypeObjectHashId switch (octet) { case
EK_COMPLETE: case EK_MINIMAL: EquivalenceHash hash; };"

**Repo:** `crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId` (l. 176)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::a_component_identifier_takes_only_the_two_equivalence_kinds`

**Status:** done

### Annex B MemberFlag TRY_CONSTRUCT1 @position(0)

**Spec:** Annex B, p. 280 — "@position(0) TRY_CONSTRUCT1, // T1".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (l. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag TRY_CONSTRUCT2 @position(1)

**Spec:** Annex B, p. 280 — "@position(1) TRY_CONSTRUCT2, // T2".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (l. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag IS_EXTERNAL @position(2)

**Spec:** Annex B, p. 280 — "@position(2) IS_EXTERNAL, // X".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (l. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag IS_OPTIONAL @position(3)

**Spec:** Annex B, p. 280 — "@position(3) IS_OPTIONAL, // O".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (l. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag IS_MUST_UNDERSTAND @position(4)

**Spec:** Annex B, p. 280 — "@position(4) IS_MUST_UNDERSTAND, // M".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (l. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag IS_KEY @position(5)

**Spec:** Annex B, p. 280 — "@position(5) IS_KEY, // K".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (l. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag IS_DEFAULT @position(6)

**Spec:** Annex B, p. 280 — "@position(6) IS_DEFAULT, // D".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (l. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag: applicable flags per member kind

**Spec:** Annex B, p. 280 — "typedef MemberFlag CollectionElementFlag; // T1,
T2, X [...] StructMemberFlag; // T1, T2, O, M, K, X [...] UnionMemberFlag; //
T1, T2, D, X [...] UnionDiscriminatorFlag; // T1, T2, K [...]
EnumeratedLiteralFlag; // D".

**Repo:** `crates/types/src/type_object/flags.rs`;
`crates/types/src/from_ast.rs`

**Tests:** `crates/types/tests/annex_b_flags.rs::a_struct_member_carries_t1_t2_o_m_k_x_and_nothing_else`,
`crates/types/tests/annex_b_flags.rs::union_members_and_the_discriminator_carry_what_annex_b_lists`,
`crates/types/tests/annex_b_flags.rs::an_enumerated_literal_carries_d_and_the_unused_flags_are_zero`,
`crates/types/tests/annex_b_flags.rs::a_collection_element_carries_t1_t2_x`,
`crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`

**Status:** done — Structure members carry only T1, T2, O, M, K, X; union members T1, T2, D, X; an enumerated literal only D; collection elements T1, T2, X; alias members, bitflags and bitfields 0. The union discriminator carries M as well (Annex B lists T1, T2, K): §7.2.2.4.4.4.6 requires "always true", Cyclone DDS and Fast DDS write `0x11` (`docs/omg-xtypes-issues.md` §B2); the contradiction is in the findings file. A key member carries M according to §7.2.2.4.4.4.8. Expectation for Cyclone DDS, Fast DDS and OpenDDS for the M on a key member: not measured.

### Annex B MemberFlagMinimalMask = 0x003f, TypeFlagMinimalMask = 0x0007

**Spec:** Annex B, p. 280–281 — "// Selects T1, T2, O, M, K, D const unsigned
short MemberFlagMinimalMask = 0x003f; [...] const unsigned short
TypeFlagMinimalMask = 0x0007; // Selects M, A, F".

**Repo:** `crates/types/src/type_object/flags.rs::minimal_member_flags`;
`crates/types/src/type_object/flags.rs::minimal_union_member_flags`

**Tests:** `crates/types/tests/annex_b_flags.rs::the_minimal_object_keeps_the_flags_that_decide_assignability`,
`crates/types/tests/annex_b_limits_and_masks.rs::the_masks_are_the_constants_of_annex_b`,
`crates/types/tests/annex_b_limits_and_masks.rs::the_projection_masks_member_and_type_flags`,
`crates/types/tests/annex_b_limits_and_masks.rs::a_minimal_struct_carries_only_the_extensibility_flags`

**Status:** done — `MemberFlagMinimalMask = 0x003f` and `TypeFlagMinimalMask = 0x0007` take effect in the MINIMAL projection. The comment in Annex B names D among the selected flags, the value `0x003f` selects T1, T2, X, O, M, K (contradiction in the findings file): a minimal union member and an enumerated literal keep `IS_DEFAULT`, because the minimal union has no other place for the default branch and the assignability reads it.

### Annex B TypeFlag IS_FINAL @position(0)

**Spec:** Annex B, p. 281 — "@position(0) IS_FINAL, // F".

**Repo:** `crates/types/src/type_object/flags.rs::StructTypeFlag` (l. 10)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_mutable_final_flags`

**Status:** done

### Annex B TypeFlag IS_APPENDABLE @position(1)

**Spec:** Annex B, p. 281 — "@position(1) IS_APPENDABLE, // A".

**Repo:** `crates/types/src/type_object/flags.rs::StructTypeFlag` (l. 10)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_mutable_final_flags`

**Status:** done

### Annex B TypeFlag IS_MUTABLE @position(2)

**Spec:** Annex B, p. 281 — "@position(2) IS_MUTABLE, // M".

**Repo:** `crates/types/src/type_object/flags.rs::StructTypeFlag` (l. 10)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_mutable_final_flags`

**Status:** done

### Annex B TypeFlag IS_NESTED @position(3)

**Spec:** Annex B, p. 281 — "@position(3) IS_NESTED, // N".

**Repo:** `crates/types/src/type_object/flags.rs::StructTypeFlag` (l. 10)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_mutable_final_flags`

**Status:** done

### Annex B TypeFlag IS_AUTOID_HASH @position(4)

**Spec:** Annex B, p. 281 — "@position(4) IS_AUTOID_HASH, // H".

**Repo:** `crates/types/src/type_object/flags.rs::StructTypeFlag` (l. 10)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_mutable_final_flags`

**Status:** done

### Annex B TypeFlag: only struct and union, otherwise unused

**Spec:** Annex B, p. 281 — "typedef TypeFlag StructTypeFlag; // All flags apply
typedef TypeFlag UnionTypeFlag; // All flags apply typedef TypeFlag
CollectionTypeFlag; // Unused. No flags apply [...] EnumTypeFlag; // Unused".

**Repo:** `crates/types/src/type_object/flags.rs::EnumTypeFlag`

**Tests:** `crates/types/tests/annex_b_flags.rs::an_enumeration_writes_its_extensibility_and_the_other_unused_type_flags_are_zero`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Structure and union carry all flags; collection, annotation, alias, bitmask and bitset 0 ("Unused"). An enumeration writes its extensibility into `enum_flags` although Annex B calls the word unused: Table 18 compares the extensibility of two enumerations, RTI Connext, OpenDDS and Cyclone DDS write the bit (`docs/omg-xtypes-issues.md` §B8); Fast DDS writes 0. The contradiction is in the findings file.

### Annex B bodies of the TypeIdentifier and union TypeIdentifier

**Spec:** Annex B, p. 281–284 — `StringSTypeDefn`, `StringLTypeDefn`,
`PlainCollectionHeader`, `PlainSequenceSElemDefn`, `PlainSequenceLElemDefn`,
`PlainArraySElemDefn`, `PlainArrayLElemDefn`, `PlainMapSTypeDefn`,
`PlainMapLTypeDefn`, `StronglyConnectedComponentId`, `ExtendedTypeDefn`, `union
TypeIdentifier switch (octet)`.

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (l. 197);
`crates/types/src/type_identifier/mod.rs::PlainCollectionHeader` (l. 122);
`crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId` (l. 176)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_map_small_minimal_keys_complete_values_wire_vector`,
`crates/types/src/type_identifier/mod.rs::tests::the_extended_arm_carries_its_body`

**Status:** done — the individual forms are listed under §7.3.4.2 Table 29.

### Annex B ANNOTATION_STR_VALUE_MAX_LEN, ANNOTATION_OCTETSEC_VALUE_MAX_LEN = 128 are checked

**Spec:** Annex B, p. 286 — "const unsigned long ANNOTATION_STR_VALUE_MAX_LEN =
128; const unsigned long ANNOTATION_OCTETSEC_VALUE_MAX_LEN = 128;"

**Repo:** —

**Tests:** `crates/types/tests/annex_b_limits_and_masks.rs::an_annotation_string_value_holds_at_most_128_octets`,
`crates/types/tests/annex_b_limits_and_masks.rs::an_annotation_wstring_value_holds_at_most_128_code_units`

**Status:** done — A string value above 128 octets and a wstring value above 128 code units are refused on writing and on reading.

### Annex B AnnotationParameterValue and ExtendedAnnotationParameterValue

**Spec:** Annex B, p. 286 — "union AnnotationParameterValue switch (octet) {
case TK_BOOLEAN: [...] case TK_FLOAT128: [...] case TK_CHAR16: [...] case
TK_STRING16: [...] default: ExtendedAnnotationParameterValue extended_value; };"

**Repo:** `crates/types/src/type_object/common.rs::AnnotationParameterValue` (l. 488)

**Tests:** `crates/types/tests/typeobject_annotation_annexb.rs::the_default_value_is_the_annex_b_union_and_never_empty`,
`crates/types/tests/annotation_parameter_value_union.rs::an_unknown_discriminator_is_the_extended_value`,
`crates/types/tests/annotation_parameter_value_union.rs::an_extended_value_with_the_discriminator_of_a_case_is_refused`

**Status:** done — `AnnotationParameterValue` has every case of the union (`Float128Bits`, `Char16`, `String16`) and the extended case with a DHEADER; an unknown discriminator is the extended value. Under XCDR1 the arm is a parameter list that `PID_LIST_END` closes (the type is MUTABLE and empty); an unknown member in it is stepped over, a must-understand member refuses the value, and members read under XCDR2 for a type of a later revision have no XCDR1 form (`320c16503d2`, tests in `crates/types/tests/type_lookup_xcdr1.rs`).

### Annex B AppliedAnnotationParameter, AppliedAnnotation, AppliedAnnotationSeq

**Spec:** Annex B, p. 287–288 — `struct AppliedAnnotationParameter { NameHash
paramname_hash; AnnotationParameterValue value; }`, `struct AppliedAnnotation {
TypeIdentifier annotation_typeid; @optional AppliedAnnotationParameterSeq
param_seq; }`.

**Repo:** `crates/types/src/type_object/common.rs::AppliedAnnotationParameter` (l. 313);
`crates/types/src/type_object/common.rs::AppliedAnnotation` (l. 348)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_applied_annotation_octets_are_annex_b`,
`crates/types/tests/type_object_custom_annotations.rs::a_parameterless_annotation_is_written_without_a_param_seq`

**Status:** done

### Annex B AppliedVerbatimAnnotation

**Spec:** Annex B, p. 288 — "struct AppliedVerbatimAnnotation { string<32>
placement; string<32> language; string text; };"

**Repo:** `crates/types/src/type_object/common.rs::AppliedVerbatimAnnotation` (l. 207)

**Tests:** `crates/types/src/type_object/mod.rs::tests::complete_verbatim_placement_variants_roundtrip`,
`crates/types/src/type_object/common.rs::safe_capacity_tests::verbatim_placement_roundtrip_other_forward_compat`

**Status:** done — `placement` as `VerbatimPlacement` with an unknown value as a
variant of its own.

### Annex B AppliedBuiltinMemberAnnotations

**Spec:** Annex B, p. 288 — "struct AppliedBuiltinMemberAnnotations { @optional
string unit; @optional AnnotationParameterValue min; @optional
AnnotationParameterValue max; @optional string hash_id; };"

**Repo:** `crates/types/src/type_object/common.rs::AppliedBuiltinMemberAnnotations` (l. 640)

**Tests:** `crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`,
`crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`

**Status:** done

### Annex B CommonStructMember, CompleteMemberDetail, MinimalMemberDetail, Complete/MinimalStructMember

**Spec:** Annex B, p. 288–289 — `CommonStructMember { MemberId member_id;
StructMemberFlag member_flags; TypeIdentifier member_type_id; }`,
`CompleteMemberDetail { MemberName name; @optional
AppliedBuiltinMemberAnnotations ann_builtin; @optional AppliedAnnotationSeq
ann_custom; }`, `MinimalMemberDetail { NameHash name_hash; }`.

**Repo:** `crates/types/src/type_object/common.rs::CommonStructMember` (l. 89);
`crates/types/src/type_object/common.rs::CompleteMemberDetail` (l. 753);
`crates/types/src/type_object/complete/struct_type.rs::CompleteStructMember` (l. 28);
`crates/types/src/type_object/minimal/struct_type.rs::MinimalStructMember` (l. 46)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/typeobject_member_annotations.rs::the_minimal_object_is_cyclones_octet_for_octet`

**Status:** done

### Annex B AppliedBuiltinTypeAnnotations, Minimal/CompleteTypeDetail, struct headers and struct types

**Spec:** Annex B, p. 289–290 — `AppliedBuiltinTypeAnnotations { @optional
AppliedVerbatimAnnotation verbatim; }`, `MinimalTypeDetail { }`,
`CompleteTypeDetail { @optional AppliedBuiltinTypeAnnotations ann_builtin;
@optional AppliedAnnotationSeq ann_custom; QualifiedTypeName type_name; }`,
`CompleteStructHeader`, `MinimalStructHeader`, `CompleteStructType`,
`MinimalStructType`.

**Repo:** `crates/types/src/type_object/common.rs::AppliedBuiltinTypeAnnotations` (l. 262);
`crates/types/src/type_object/common.rs::CompleteTypeDetail` (l. 434);
`crates/types/src/type_object/complete/struct_type.rs::CompleteStructType` (l. 37);
`crates/types/src/type_object/minimal/struct_type.rs::MinimalStructType` (l. 92)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_with_base_type`,
`crates/types/src/type_object/mod.rs::tests::complete_struct_sensor_with_names_and_annotations`

**Status:** done

### Annex B Union: UnionCaseLabelSeq, members, discriminator, header, type

**Spec:** Annex B, p. 291–292 — `typedef sequence<long> UnionCaseLabelSeq`,
`CommonUnionMember`, `Complete/MinimalUnionMember`, `CommonDiscriminatorMember`,
`Complete/MinimalDiscriminatorMember`, `Complete/MinimalUnionHeader`,
`Complete/MinimalUnionType`.

**Repo:** `crates/types/src/type_object/common.rs::CommonUnionMember` (l. 127);
`crates/types/src/type_object/complete/union_type.rs::CompleteUnionType` (l. 47);
`crates/types/src/type_object/minimal/union_type.rs::MinimalUnionType` (l. 119)

**Tests:** `crates/types/src/type_object/mod.rs::tests::minimal_union_with_int_discriminator_two_cases`,
`crates/types/src/type_object/mod.rs::tests::complete_union_roundtrips`,
`crates/types/tests/typeobject_serialized_size.rs::the_announced_size_opens_with_the_dheader_of_the_appendable_union`

**Status:** done — labels are `long`; a `long long` discriminator with a larger
label cannot be represented this way (limit of the specification).

### Annex B Annotation: parameters, header, type

**Spec:** Annex B, p. 293–294 — `CommonAnnotationParameter`,
`Complete/MinimalAnnotationParameter` ("Ordered by
CompleteAnnotationParameter.name" or "MinimalAnnotationParameter.name_hash"),
`Complete/MinimalAnnotationHeader`, `Complete/MinimalAnnotationType`.

**Repo:** `crates/types/src/type_object/complete/annotation_type.rs::CompleteAnnotationType` (l. 129);
`crates/types/src/type_object/minimal/annotation_type.rs::MinimalAnnotationType` (l. 141)

**Tests:** `crates/types/tests/typeobject_annotation_annexb.rs::the_complete_object_is_annex_bs_octet_for_octet`,
`crates/types/tests/typeobject_annotation_annexb.rs::the_minimal_object_is_annex_bs_octet_for_octet`,
`crates/types/tests/typeobject_annotation_annexb.rs::each_sequence_is_ordered_by_its_own_key`

**Status:** done

### Annex B Alias: CommonAliasBody, body, header, type

**Spec:** Annex B, p. 294–295 — `CommonAliasBody { AliasMemberFlag
related_flags; TypeIdentifier related_type; }`, `Complete/MinimalAliasBody`,
`Complete/MinimalAliasHeader`, `Complete/MinimalAliasType`.

**Repo:** `crates/types/src/type_object/minimal/alias_type.rs::CommonAliasBody` (l. 12);
`crates/types/src/type_object/complete/alias_type.rs::CompleteAliasType` (l. 36)

**Tests:** `crates/types/src/type_object/mod.rs::tests::minimal_alias_int64_typedef`,
`crates/types/src/type_object/mod.rs::tests::complete_alias_typedef`

**Status:** done

### Annex B Collections: Element, Header, Sequence, Array, Map

**Spec:** Annex B, p. 295–298 — `CompleteElementDetail`,
`CommonCollectionElement`, `Complete/MinimalCollectionElement`,
`CommonCollectionHeader`, `Complete/MinimalCollectionHeader`,
`Complete/MinimalSequenceType`, `CommonArrayHeader`,
`Complete/MinimalArrayHeader`, `Complete/MinimalArrayType`,
`Complete/MinimalMapType`.

**Repo:** `crates/types/src/type_object/complete/collection_types.rs::CompleteSequenceType` (l. 58);
`crates/types/src/type_object/complete/collection_types.rs::CompleteArrayType` (l. 93);
`crates/types/src/type_object/complete/collection_types.rs::CompleteMapType` (l. 131);
`crates/types/src/type_object/minimal/collection_types.rs::MinimalMapType` (l. 155)

**Tests:** `crates/types/tests/typeobject_collections_annex_b.rs` (six tests,
against the serializer of Cyclone DDS 11.0.1),
`crates/types/src/type_object/mod.rs::tests::minimal_sequence_of_floats`,
`crates/types/src/type_object/mod.rs::tests::minimal_array_3d_doubles`,
`crates/types/src/type_object/mod.rs::tests::minimal_map_string_to_int64`,
`crates/types/src/type_object/mod.rs::tests::complete_map_roundtrips`

**Status:** done — header and element are embedded in the types, not named as
Rust types of their own; the framing is Annex B's (`cb58f485ebd`):
`CompleteCollectionElement`, `CompleteCollectionHeader`, `CompleteArrayHeader`,
`CompleteArrayType`, `MinimalCollectionElement`, `MinimalCollectionHeader` and
`MinimalArrayHeader` are appendable (a `DHEADER` each under XCDR2), and the
`detail` of `CompleteCollectionHeader` is `@optional` and absent for an
anonymous type ("not present for anonymous"), as every sequence and map of IDL
is (`CompleteSequenceType::detail`, `CompleteMapType::detail` as `Option`).
Before, the codecs wrote header and element without a `DHEADER` and the
`detail` always, with a name made of owner and member; the tests were round
trips of the codec itself and did not see it. Measured against
`dds_stream_write_sampleLE` of Cyclone DDS 11.0.1 with its installed
`DDS_XTypes_TypeObject` descriptor (`internal/release/evidence/rc7-typeobject-collections`):
the eight objects (sequence named and anonymous, element with `@unit`, array,
map, and the three minimal ones) octet for octet under XCDR2, the three minimal
ones under XCDR1 as well; Cyclone writes no COMPLETE collection under XCDR1
(the call returns `false`), where the own round trip holds. The name in the
`detail` of an array (always present) is the one of owner and member,
`Holder.arr`; Cyclone puts the scoped name of the declarator there (source
read, `descriptor_type_meta.c`, not measured).

### Annex B Enumeration: BitBound, literal, header, type

**Spec:** Annex B, p. 298–299 — `typedef unsigned short BitBound`,
`CommonEnumeratedLiteral { long value; EnumeratedLiteralFlag flags; }`,
`Complete/MinimalEnumeratedLiteral` ("Ordered by
EnumeratedLiteral.common.value"), `CommonEnumeratedHeader { BitBound bit_bound;
}`, `Complete/MinimalEnumeratedHeader`, `Complete/MinimalEnumeratedType`.

**Repo:** `crates/types/src/type_object/minimal/enum_type.rs::CommonEnumeratedLiteral` (l. 28);
`crates/types/src/type_object/complete/enum_type.rs::CompleteEnumeratedType` (l. 36);
`crates/types/src/type_object/minimal/enum_type.rs::MinimalEnumeratedType` (l. 76)

**Tests:** `crates/types/src/type_object/mod.rs::tests::minimal_enum_color_3_literals`,
`crates/types/src/type_object/mod.rs::tests::complete_enum_with_literal_names`,
`crates/types/tests/literal_and_flag_order.rs::enumerated_literals_are_written_in_increasing_value_order`

**Status:** done — The literals are written in increasing order of value.

### Annex B Bitmask: bitflag, header, type

**Spec:** Annex B, p. 300–301 — `CommonBitflag { unsigned short position;
BitflagFlag flags; }`, `Complete/MinimalBitflag` ("Ordered by
Bitflag.position"), `CommonBitmaskHeader`, `Complete/MinimalBitmaskHeader`,
`Complete/MinimalBitmaskType`.

**Repo:** `crates/types/src/type_object/minimal/bitmask_type.rs::CommonBitflag` (l. 14);
`crates/types/src/type_object/complete/bitmask_type.rs::CompleteBitmaskType` (l. 27)

**Tests:** `crates/types/src/type_object/mod.rs::tests::minimal_bitmask_3_flags`,
`crates/types/src/type_object/mod.rs::tests::complete_bitmask_roundtrips`,
`crates/types/tests/literal_and_flag_order.rs::bitmask_flags_are_written_in_increasing_position_order`

**Status:** done — The flags are written in increasing order of position.

### Annex B Bitset: bitfield, header, type

**Spec:** Annex B, p. 301–302 — `CommonBitfield { unsigned short position;
BitsetMemberFlag flags; octet bitcount; TypeKind holder_type; }`,
`Complete/MinimalBitfield`, `Complete/MinimalBitsetHeader`,
`Complete/MinimalBitsetType`.

**Repo:** `crates/types/src/type_object/minimal/bitset_type.rs::CommonBitfield` (l. 17);
`crates/types/src/type_object/complete/bitset_type.rs::CompleteBitsetType` (l. 27)

**Tests:** `crates/types/src/type_object/mod.rs::tests::minimal_bitset_roundtrips`,
`crates/types/src/type_object/mod.rs::tests::complete_bitset_roundtrips`

**Status:** done

### Annex B CompleteTypeObject, MinimalTypeObject, ExtendedType

**Spec:** Annex B, p. 302–303 — `union CompleteTypeObject switch (octet) { case
TK_ALIAS: [...] default: CompleteExtendedType extended_type; }`, `union
MinimalTypeObject switch (octet)`.

**Repo:** `crates/types/src/type_object/complete/mod.rs::CompleteTypeObject` (l. 44);
`crates/types/src/type_object/minimal/mod.rs::MinimalTypeObject` (l. 47)

**Tests:** `crates/types/src/type_object/mod.rs::tests::an_extended_type_kind_keeps_the_member_list_it_arrived_with`,
`crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`

**Status:** done — Under XCDR1 the arm is a parameter list that `PID_LIST_END` closes (the type is MUTABLE and empty); an unknown member in it is stepped over, a must-understand member refuses the value, and members read under XCDR2 for a type of a later revision have no XCDR1 form (`320c16503d2`, tests in `crates/types/tests/type_lookup_xcdr1.rs`).

### Annex B union TypeObject, TypeObjectSeq, StronglyConnectedComponent

**Spec:** Annex B, p. 304 — "union TypeObject switch (octet) { //
EquivalenceKind case EK_COMPLETE: CompleteTypeObject complete; case EK_MINIMAL:
MinimalTypeObject minimal; }; typedef sequence<TypeObject> TypeObjectSeq; [...]
typedef TypeObjectSeq StronglyConnectedComponent;"

**Repo:** `crates/types/src/type_object/mod.rs::TypeObject` (l. 36)

**Tests:** `crates/types/src/type_object/mod.rs::tests::unknown_equivalence_kind_fails_gracefully`,
`crates/types/tests/strongly_connected_components.rs::a_component_members_type_object_builds`

**Status:** done

### Annex B TypeIdentifierTypeObjectPair, TypeIdentifierPair, TypeIdentfierWithSize, TypeIdentifierWithDependencies, TypeInformation

**Spec:** Annex B, p. 305–306 — `TypeIdentifierTypeObjectPair`,
`TypeIdentifierPair`, `TypeIdentfierWithSize`, `TypeIdentifierWithDependencies`,
`@extensibility(MUTABLE) struct TypeInformation`.

**Repo:** `crates/types/src/type_lookup.rs::TypeIdentifierTypeObjectPair` (l. 166);
`crates/types/src/type_lookup.rs::TypeIdentifierPair` (l. 205);
`crates/types/src/type_information.rs::TypeInformation` (l. 209)

**Tests:** `crates/types/src/type_lookup.rs::tests::a_reply_pair_names_the_identifier_it_answers`,
`crates/discovery/tests/representation_and_type_information.rs::type_information_is_built_from_a_keyed_appendable_struct`

**Status:** done — see §7.6.3.2.1 and §7.6.3.3.3.

---

## Annex C: Dynamic Language Binding

### Annex C IDL PSM of the Dynamic Language Binding

**Spec:** Annex C, p. 307–322 — IDL of `DynamicTypeBuilderFactory`,
`TypeSupport`, `DynamicTypeSupport`, `AnnotationDescriptor`,
`VerbatimTextDescriptor`, `TypeDescriptor`, `MemberDescriptor`,
`DynamicTypeMember`, `DynamicTypeBuilder`, `DynamicType`, `DynamicDataFactory`,
`DynamicData`.

**Repo:** `crates/types/src/dynamic/`

**Tests:** `crates/dcps/tests/annex_c_idl_conformance.rs::every_operation_of_annex_c_has_its_counterpart`; `crates/dcps/tests/annex_c_idl_conformance.rs::the_state_of_the_valuetypes_is_in_the_records`; `crates/dcps/tests/annex_c_idl_conformance.rs::the_type_kind_constants_of_the_idl_are_the_rust_constants`

**Status:** done — the test reads the IDL of the annex (`crates/types/tests/fixtures/annex-c-dynamic-language-binding.idl`) and checks that every constant, operation and valuetype state member has a counterpart in the Rust API; log: `.agent/REPORT.md`.

### Annex C TypeKind constants

**Spec:** Annex C, p. 307–308 — "const TypeKind TK_NONE = 0x00; [...] const
TypeKind TK_MAP = 0x62;"

**Repo:** `crates/types/src/dynamic/descriptor.rs::TK_NONE`; `TypeKindValue`

**Tests:** `crates/types/tests/dynamic_annex_c.rs::the_type_kind_constants_have_the_numbers_of_annex_c`

**Status:** done — `TK_NONE` to `TK_MAP` with the numbers of the annex; log: `.agent/REPORT.md`.

### Annex C VerbatimTextDescriptor and get_verbatim_text

**Spec:** Annex C, p. 310–312 — "valuetype VerbatimTextDescriptor { public
string placement; public string text; [...] }";
`DynamicTypeMember::get_verbatim_text_count`, `get_verbatim_text`.

**Repo:** `crates/types/src/dynamic/type_.rs::verbatim_text_count`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::verbatim_text_is_reported_with_its_placement`

**Status:** done — `get_verbatim_text_count` and `get_verbatim_text` on `DynamicType` and `DynamicTypeMember`; log: `.agent/REPORT.md`.

### Annex C ExtensibilityKind and TryConstructKind

**Spec:** Annex C, p. 311 — "enum ExtensibilityKind { FINAL, APPENDABLE, MUTABLE
}; enum TryConstructKind { USE_DEFAULT, DISCARD, TRIM };"

**Repo:** `crates/types/src/dynamic/descriptor.rs::ExtensibilityKind` (l. 275);
`crates/types/src/dynamic/descriptor.rs::TryConstructKind` (l. 331)

**Tests:** `crates/types/src/dynamic/descriptor.rs::tests::the_two_renames_round_trip`

**Status:** done

### Annex C TypeDescriptor with extensibility_kind and is_nested

**Spec:** Annex C, p. 311 — "valuetype TypeDescriptor { public TypeKind kind;
public ObjectName name; public DynamicType base_type; public DynamicType
discriminator_type; public BoundSeq bound; @optional public DynamicType
element_type; @optional public DynamicType key_element_type; public
ExtensibilityKind extensibility_kind; public boolean is_nested; [...] };"

**Repo:** `crates/types/src/dynamic/descriptor.rs::TypeDescriptor` (l. 381)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done — Table 50 does not name `extensibility_kind` and `is_nested`
(listed in the findings file).

### Annex C MemberDescriptor with try_construct_kind and the flags

**Spec:** Annex C, p. 311–312 — "valuetype MemberDescriptor { public ObjectName
name; public MemberId id; public DynamicType type; public string default_value;
public unsigned long index; public UnionCaseLabelSeq label; public
TryConstructKind try_construct_kind; public boolean is_key; public boolean
is_optional; public boolean is_must_understand; public boolean is_shared; public
boolean is_default_label; [...] };"

**Repo:** `crates/types/src/dynamic/descriptor.rs::MemberDescriptor`

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`; `crates/types/tests/dynamic_type_descriptors.rs::a_union_label_past_32_bits_is_kept`; `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`; `crates/dcps/tests/annex_c_idl_conformance.rs::the_state_of_the_valuetypes_is_in_the_records`

**Status:** done — `MemberDescriptor` carries `try_construct_kind`, the flags and `default_value` as a string (§7.5.2.7.3, item of its own: done); `label` is `Vec<i64>` as in Table 53; log in `.agent/REPORT.md`.

### Annex C sequence types Int32Seq to WstringSeq for DynamicData

**Spec:** Annex C, p. 315 — "typedef sequence<long> Int32Seq; [...] typedef
sequence<wstring> WstringSeq;"

**Repo:** `crates/types/src/dynamic/sample.rs::Int32Seq`

**Tests:** `crates/types/tests/dynamic_annex_c.rs::the_typed_sequences_are_the_types_of_the_values_operations`

**Status:** done — `Int32Seq` to `WstringSeq`; log: `.agent/REPORT.md`.

---

## Annex D: DDS Built-in Topic Data Types

### Annex D Serialization with XCDR version 1

**Spec:** Annex D, p. 323 — "// The types in this file shall be serialized with
XCDR encoding version 1".

**Repo:** `crates/rtps/src/parameter_list.rs`

**Tests:** `crates/rtps/src/parameter_list.rs::tests::pid_ignore_skipped_in_pl_cdr_decode`

**Status:** done — the built-in topic data go as PL_CDR (the RTPS parameter
list).

### Annex D QoS structures and enumerations of the built-in topics

**Spec:** Annex D, p. 323–327 — `BuiltinTopicKey_t`, `Duration_t`,
`DeadlineQosPolicy`, `DestinationOrderQosPolicy`, `DurabilityQosPolicy`,
`HistoryQosPolicy`, `DurabilityServiceQosPolicy`, `GroupDataQosPolicy`,
`LatencyBudgetQosPolicy`, `LifespanQosPolicy`, `LivelinessQosPolicy`,
`OwnershipQosPolicy`, `OwnershipStrengthQosPolicy`, `PartitionQosPolicy`,
`PresentationQosPolicy`, `ReliabilityQosPolicy` (`@value(1)`, `@value(2)`),
`ResourceLimitsQosPolicy`, `TimeBasedFilterQosPolicy`, `TopicDataQosPolicy`,
`TransportPriorityQosPolicy`, `UserDataQosPolicy`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — restatement of the DDS and RTPS definitions;
checked in the audit of DDS 1.4 and DDSI-RTPS 2.5.

### Annex D ParticipantBuiltinTopicData: key 0x0050, user_data 0x002C

**Spec:** Annex D, p. 327 — "@extensibility(MUTABLE) struct
ParticipantBuiltinTopicData { @id(0x0050) @key BuiltinTopicKey_t key;
@id(0x002C) UserDataQosPolicy user_data; };"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — RTPS parameters; checked in the audit of
DDSI-RTPS 2.5.

### Annex D DataRepresentationId_t and constants 0, 1, 2

**Spec:** Annex D, p. 327 — "typedef short DataRepresentationId_t; const
DataRepresentationId_t XCDR_DATA_REPRESENTATION = 0; const
DataRepresentationId_t XML_DATA_REPRESENTATION = 1; const DataRepresentationId_t
XCDR2_DATA_REPRESENTATION = 2;"

**Repo:** `crates/rtps/src/publication_data.rs::DEFAULT_OFFER` (l. 108);
`crates/types/src/qos.rs::DataRepresentationId` (l. 174)

**Tests:** `crates/dcps/src/runtime.rs::tests::data_repr_offer_str_uses_spec_ids`

**Status:** done

### Annex D DATA_REPRESENTATION_QOS_POLICY_ID = 23, TYPE_CONSISTENCY_ENFORCEMENT_QOS_POLICY_ID = 24

**Spec:** Annex D, p. 327–328 — "const QosPolicyId_t
DATA_REPRESENTATION_QOS_POLICY_ID = 23; [...] const QosPolicyId_t
TYPE_CONSISTENCY_ENFORCEMENT_QOS_POLICY_ID = 24;"

**Repo:** `crates/dcps/src/psm_constants.rs`
(`qos_policy_id::DATA_REPRESENTATION`,
`qos_policy_id::TYPE_CONSISTENCY_ENFORCEMENT`)

**Tests:** `crates/dcps/src/psm_constants.rs::tests::the_xtypes_policy_ids_are_those_of_its_idl`

**Status:** `done` — 23 and 24 as in Annex D.

### Annex D names DataRepresentation and TypeConsistencyEnforcement

**Spec:** Annex D, p. 327–328 — "const string
DATA_REPRESENTATION_QOS_POLICY_NAME = 'DataRepresentation'; [...] const string
TYPE_CONSISTENCY_ENFORCEMENT_QOS_POLICY_NAME = 'TypeConsistencyEnforcement';"

**Repo:** `crates/dcps/src/psm_constants.rs`

**Tests:** `crates/dcps/src/psm_constants.rs::tests::the_xtypes_policy_names_and_representation_ids_are_those_of_annex_d`

**Status:** done — `psm_constants::qos_policy_name` returns `DataRepresentation` and `TypeConsistencyEnforcement`; `psm_constants::data_representation_id` gives `XCDR` 0, `XML` 1 and `XCDR2` 2.

### Annex D DataRepresentationQosPolicy { DataRepresentationIdSeq value; }

**Spec:** Annex D, p. 328 — "@extensibility(APPENDABLE) @nested struct
DataRepresentationQosPolicy { DataRepresentationIdSeq value; };"

**Repo:** `crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData` (l. 207, field `data_representation` l. 274)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::publication_data_roundtrip_preserves_data_representation`

**Status:** done

### Annex D TypeConsistencyKind @bit_bound(16) and TypeConsistencyEnforcementQosPolicy

**Spec:** Annex D, p. 328 — "@bit_bound(16) enum TypeConsistencyKind {
DISALLOW_TYPE_COERCION, ALLOW_TYPE_COERCION }; [...] struct
TypeConsistencyEnforcementQosPolicy { TypeConsistencyKind kind; boolean
ignore_sequence_bounds; boolean ignore_string_bounds; boolean
ignore_member_names; boolean prevent_type_widening; boolean
force_type_validation; };"

**Repo:** `crates/types/src/qos.rs::TypeConsistencyEnforcement` (l. 52);
`crates/types/src/qos.rs::from_bytes` (l. 148)

**Tests:** `crates/types/src/qos.rs::tests::consistency_roundtrip_default`,
`crates/types/src/qos.rs::tests::type_consistency_kind_has_the_two_values_of_annex_d`,
`crates/dcps/tests/xtypes_aware_match.rs::a_readers_type_consistency_policy_survives_its_announcement`

**Status:** done — `kind` has two octets, then the five Booleans follow in this order; `TypeConsistencyKind` has the two values of Annex D, an unknown value is refused.

### Annex D TopicBuiltinTopicData with type_information 0x0075 and representation 0x0073

**Spec:** Annex D, p. 328–329 — "struct TopicBuiltinTopicData { @id(0x005A) @key
BuiltinTopicKey_t key; @id(0x0005) ObjectName name; @id(0x0007) ObjectName
type_name; @id(0x0069) @optional TypeIdV1 type_id; @id(0x0072) @optional
TypeObjectV1 type; @id(0x0075) @optional XTypes::TypeInformation
type_information; [...] @id(0x0073) DataRepresentationQosPolicy representation;
};"

**Repo:** `crates/rtps/src/topic_data.rs::TopicBuiltinTopicData`;
`crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData`;
`crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData`

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::topic_data_roundtrip_preserves_data_representation_and_type_information`,
`crates/dcps/tests/sedp_data_representation_pid.rs::publication_data_roundtrip_preserves_data_representation`,
`crates/rtps/tests/type_object_v1_discovery.rs::a_topic_carries_its_type_id_and_type`,
`crates/rtps/tests/type_object_v1_discovery.rs::a_publication_carries_its_type_id_and_type`,
`crates/rtps/tests/type_object_v1_discovery.rs::a_subscription_carries_its_type_id_and_type`,
`crates/rtps/tests/type_object_v1_discovery.rs::the_parameters_have_the_ids_of_annex_d`

**Status:** done — The topic data write `type_information` under `0x0075`, `representation` under `0x0073`, `type_id` (`TypeIdV1`) under `0x0069` and `type` (`TypeObjectV1`) under `0x0072` and read them back; the publication and subscription data carry the same fields (`type_id_v1`, `type_object_v1`). A parameter that does not decode stays out of the data (both members are `@optional`).

### Annex D TypeIdV1 (0x0069) and TypeObjectV1 (0x0072) of XTYPES 1.1

**Spec:** Annex D, p. 328 — "@id(0x0069) @optional TypeIdV1 type_id; // XTYPES 1.1" and "@id(0x0072) @optional TypeObjectV1 type; // XTYPES 1.1".

**Repo:** `crates/types/src/type_object_v1/mod.rs`;
`crates/types/src/type_object_v1/codec.rs`;
`crates/types/src/type_object_v1/convert.rs`;
`crates/rtps/src/type_v1.rs`;
`crates/dcps/src/runtime.rs::announced_type_object_v1`

**Tests:** `crates/types/tests/type_object_v1.rs::a_minimal_object_has_the_octets_the_rules_give`,
`crates/types/tests/type_object_v1.rs::a_type_id_has_the_octets_of_its_union`,
`crates/types/tests/type_object_v1.rs::a_library_made_of_every_kind_survives_both_byte_orders`,
`crates/types/tests/type_object_v1.rs::the_id_of_a_type_is_the_low_64_bits_of_the_md5_of_its_serialization`,
`crates/types/tests/type_object_v1.rs::a_topic_type_comes_back_as_the_complete_objects_it_was`,
`crates/types/tests/type_object_v1.rs::an_annotation_and_its_application_cross_both_ways`,
`crates/types/tests/type_object_v1.rs::what_annex_b_has_no_form_for_is_refused_by_name`,
`crates/types/tests/type_object_v1.rs::a_type_that_refers_to_itself_has_no_form`,
`crates/rtps/tests/type_object_v1_discovery.rs::a_parameter_that_does_not_decode_is_left_out_not_a_refusal`,
`crates/dcps/src/runtime.rs::type_object_v1_tests::the_type_object_v1_is_announced_only_when_asked`,
`crates/dcps/src/runtime.rs::type_object_v1_tests::the_type_object_v1_of_a_remote_makes_its_types_known`

**Status:** done — Annex D writes `TypeIdV1` and `TypeObjectV1` without a definition in 1.3; `TypeObjectV1` is the `TypeObject` of Annex B of XTypes 1.1 and `TypeIdV1` its `_TypeId` (the only identifier of 1.1). The model, the XCDR1 serialization (parameter lists for the mutable structures, plain CDR for the extensible ones; a mutable union as its discriminator and selected member by XTypes 1.3 §7.4.3.5.3 rule (28)), the type ids of 1.1 §7.3.4.1.2 (the low 64 bits of the MD5 over the big-endian serialization) and the conversion from and to Complete TypeObjects are in `zerodds_types::type_object_v1`. Where the text leaves a choice, the module documentation names it: member ids of the union `TypeLibraryElement` by declaration order, `MODULE_ELEMENT` = 24 (the text gives the enumerator no value, the next would be 22 and collides), the `type_id` of the type itself is `NO_TYPE` while it is hashed. The setting `discovery.type_object_v1` (default off) has endpoints and topics add both members; the `TypeObjectV1` of a remote endpoint without `PID_TYPE_INFORMATION` is put into the registry as Complete TypeObjects and gives the endpoint its identifier. What 1.1 cannot carry is not taken along (`try_construct`, `@must_understand` except that of a key, `@autoid(HASH)`, `@verbatim`, `@unit`, `@min`, `@max`, `@hashid`, default values, `@ignore_literal_names`) or gives a named error (`int8`, a bit set with bit fields, a type that reaches itself: its id would be the hash of a serialization that contains it).

### Annex D PublicationBuiltinTopicData with type_information and representation

**Spec:** Annex D, p. 329–330 — "struct PublicationBuiltinTopicData { [...]
@id(0x0075) @optional XTypes::TypeInformation type_information; [...]
@id(0x0073) DataRepresentationQosPolicy representation; };"

**Repo:** `crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData` (l. 207)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::publication_standard_type_information_yields_type_identifier`,
`crates/dcps/tests/sedp_data_representation_pid.rs::publication_data_emits_data_representation_pid_when_set`

**Status:** done

### Annex D SubscriptionBuiltinTopicData with type_information, representation and type_consistency 0x0074

**Spec:** Annex D, p. 330–331 — "struct SubscriptionBuiltinTopicData { [...]
@id(0x0073) DataRepresentationQosPolicy representation; @id(0x0074)
TypeConsistencyEnforcementQosPolicy type_consistency; };"

**Repo:** `crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData` (l. 37)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::subscription_standard_type_information_yields_type_identifier`,
`crates/dcps/tests/xtypes_aware_match.rs::a_readers_type_consistency_policy_survives_its_announcement`

**Status:** done

### Annex D TopicQos, DataWriterQos, DataReaderQos with representation and type_consistency

**Spec:** Annex D, p. 329–331 — "struct TopicQos { // ...
DataRepresentationQosPolicy representation; }; struct DataWriterQos { // ...
DataRepresentationQosPolicy representation; }; struct DataReaderQos { // ...
DataRepresentationQosPolicy representation; TypeConsistencyEnforcementQosPolicy
type_consistency; };"

**Repo:** `crates/dcps/src/qos.rs::DataWriterQos`;
`crates/dcps/src/qos.rs::DataReaderQos`;
`crates/zerodds-c-api/src/qos_ffi.rs`;
`crates/cpp/include/dds/core/qos_bridge.hpp`

**Tests:** `crates/dcps/src/qos.rs::data_representation_tests::data_representation_defaults_none_and_settable`,
`crates/dcps/tests/topic_data_representation.rs::the_topic_qos_names_a_representation_for_its_writers_and_readers`,
`crates/zerodds-c-api/tests/qos_data_representation_c.rs::the_type_consistency_of_a_reader_is_read_and_a_wrong_kind_is_refused`,
`crates/zerodds-c-api/tests/qos_data_representation_c.rs::the_entities_hand_the_representation_back`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_data_representation_and_type_consistency_in_the_qos_sets`

**Status:** done — `TopicQos`, `DataWriterQos` and `DataReaderQos` carry `representation` and `DataReaderQos` `type_consistency`, in Rust, in the C structs, in the Java PSM and in the QoS sets of the C++ PSM; a `TypeConsistencyKind` that Annex D does not know is refused by `zerodds_sub_create_datareader`.

---

## Annex E: Built-in Types

### Annex E DDS::_String, KeyedString, Bytes, KeyedBytes as APPENDABLE

**Spec:** Annex E, p. 333–336 — "@extensibility(APPENDABLE) struct _String {
string value; }; [...] @extensibility(APPENDABLE) struct KeyedString { @key
string key; string value; }; [...] struct Bytes { ByteSeq value; }; [...] struct
KeyedBytes { @key string key; ByteSeq value; };"

**Repo:** `crates/dcps/src/builtin_type_set.rs::build` (l. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::dds_string_has_unbounded_string_value_member`,
`crates/dcps/tests/builtin_types_auto_register_c44b.rs::dds_keyed_bytes_has_key_member`,
`crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`

**Status:** done — Members as in Annex E, `@extensibility(APPENDABLE)`, not nested.

### Annex E footnote 9: leading underscore in _String

**Spec:** Annex E, footnote 9, p. 333 — "The leading underscore in the
declaration of the String structure is necessary to prevent collision with the
IDL keyword 'string.'"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — explanation; the type name is `DDS::String`.

### Annex E StringDataWriter, StringDataReader, StringTypeSupport

**Spec:** Annex E, p. 333 — "interface StringDataWriter : DataWriter { /* This
interface shall instantiate the type FooDataWriter [...] where 'Foo' is an
unbounded string. */ };"

**Repo:** `crates/dcps/src/builtin_types.rs::StringTypeSupport`; `crates/zerodds-c-api/src/builtin_type_ffi.rs`; `crates/cpp/include/zerodds/builtin_type_support.hpp`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::StringDataWriter`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::StringDataReader`

**Tests:** `crates/dcps/tests/builtin_type_support.rs::each_type_support_names_its_type`; `crates/dcps/tests/builtin_type_support.rs::register_type_registers_each_type_under_a_name`; `crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`; `crates/zerodds-c-api/tests/builtin_types_c.rs::the_type_supports_name_and_register_their_types`; `crates/cpp/tests/builtin_data_endpoints_via_cargo.rs::the_data_writers_and_readers_of_the_built_in_types_write_and_take`; `crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java`

**Status:** done — `StringTypeSupport` with `get_type_name` and `register_type` in Rust, the C ABI, C++ and Java; `StringDataWriter` and `StringDataReader` in Rust (aliases of `DataWriter<…>`, `DataReader<…>`), the C ABI and C++ (classes over the C ABI). In Java the interfaces are `DataWriter<T>` and `DataReader<T>` over `String`, `byte[]`, `KeyedString` and `KeyedBytes` by DDS-Java-PSM 1.0 §7.8.2; log in `.agent/REPORT.md`.

### Annex E KeyedStringDataWriter: *_w_key operations

**Spec:** Annex E, p. 334–335 — `register_instance_w_key`,
`register_instance_w_key_w_timestamp`, `unregister_instance_w_key`,
`unregister_instance_w_key_w_timestamp`, `write_string_w_key`,
`write_string_w_key_w_timestamp`, `dispose_w_key`, `dispose_w_key_w_timestamp`,
`get_key_value_w_key`, `lookup_instance_w_key`.

**Repo:** `crates/dcps/src/builtin_types.rs::KeyedStringDataWriterOperations`

**Tests:** `crates/dcps/tests/builtin_types_writers.rs::keyed_string_operations_work_on_the_key`

**Status:** done — The operations of Annex E are in the trait `KeyedStringDataWriterOperations` for `DataWriter<…>` (`*_w_key`, each with `_w_timestamp`).

### Annex E KeyedStringDataReader, KeyedStringTypeSupport

**Spec:** Annex E, p. 335 — "interface KeyedStringDataReader : DataReader {
[...] }; interface KeyedStringTypeSupport : TypeSupport { [...] };"

**Repo:** `crates/dcps/src/builtin_types.rs::KeyedStringTypeSupport`; `crates/zerodds-c-api/src/builtin_type_ffi.rs`; `crates/cpp/include/zerodds/builtin_type_support.hpp`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::KeyedStringDataWriter`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::KeyedStringDataReader`

**Tests:** `crates/dcps/tests/builtin_type_support.rs::each_type_support_names_its_type`; `crates/dcps/tests/builtin_type_support.rs::register_type_registers_each_type_under_a_name`; `crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`; `crates/zerodds-c-api/tests/builtin_types_c.rs::the_type_supports_name_and_register_their_types`; `crates/cpp/tests/builtin_data_endpoints_via_cargo.rs::the_data_writers_and_readers_of_the_built_in_types_write_and_take`; `crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java`

**Status:** done — `KeyedStringTypeSupport` with `get_type_name` and `register_type` in Rust, the C ABI, C++ and Java; `KeyedStringDataWriter` and `KeyedStringDataReader` in Rust (aliases of `DataWriter<…>`, `DataReader<…>`), the C ABI and C++ (classes over the C ABI). In Java the interfaces are `DataWriter<T>` and `DataReader<T>` over `String`, `byte[]`, `KeyedString` and `KeyedBytes` by DDS-Java-PSM 1.0 §7.8.2; log in `.agent/REPORT.md`.

### Annex E BytesDataWriter: write_w_bytes, write_w_bytes_w_timestamp

**Spec:** Annex E, p. 335–336 — "ReturnCode_t write_w_bytes(in ByteArray bytes,
in long offset, in long length, in InstanceHandle_t handle); ReturnCode_t
write_w_bytes_w_timestamp(...)".

**Repo:** `crates/dcps/src/builtin_types.rs::BytesDataWriterOperations`

**Tests:** `crates/dcps/tests/builtin_types_writers.rs::bytes_are_written_from_an_offset_and_a_length`

**Status:** done — The operations of Annex E are in the trait `BytesDataWriterOperations` for `DataWriter<…>` (`*_w_key`, each with `_w_timestamp`).

### Annex E BytesDataReader, BytesTypeSupport

**Spec:** Annex E, p. 336 — "interface BytesDataReader : DataReader { [...] };
interface BytesTypeSupport : TypeSupport { [...] };"

**Repo:** `crates/dcps/src/builtin_types.rs::BytesTypeSupport`; `crates/zerodds-c-api/src/builtin_type_ffi.rs`; `crates/cpp/include/zerodds/builtin_type_support.hpp`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::BytesDataWriter`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::BytesDataReader`

**Tests:** `crates/dcps/tests/builtin_type_support.rs::each_type_support_names_its_type`; `crates/dcps/tests/builtin_type_support.rs::register_type_registers_each_type_under_a_name`; `crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`; `crates/zerodds-c-api/tests/builtin_types_c.rs::the_type_supports_name_and_register_their_types`; `crates/cpp/tests/builtin_data_endpoints_via_cargo.rs::the_data_writers_and_readers_of_the_built_in_types_write_and_take`; `crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java`

**Status:** done — `BytesTypeSupport` with `get_type_name` and `register_type` in Rust, the C ABI, C++ and Java; `BytesDataWriter` and `BytesDataReader` in Rust (aliases of `DataWriter<…>`, `DataReader<…>`), the C ABI and C++ (classes over the C ABI). In Java the interfaces are `DataWriter<T>` and `DataReader<T>` over `String`, `byte[]`, `KeyedString` and `KeyedBytes` by DDS-Java-PSM 1.0 §7.8.2; log in `.agent/REPORT.md`.

### Annex E KeyedBytesDataWriter: *_w_key operations

**Spec:** Annex E, p. 336–337 — `register_instance_w_key`,
`unregister_instance_w_key`, `write_bytes_w_key`,
`write_bytes_w_key_w_timestamp`, `dispose_w_key`, `get_key_value_w_key`,
`lookup_instance_w_key` and the `_w_timestamp` forms.

**Repo:** `crates/dcps/src/builtin_types.rs::KeyedBytesDataWriterOperations`

**Tests:** `crates/dcps/tests/builtin_types_writers.rs::keyed_bytes_operations_work_on_the_key`

**Status:** done — The operations of Annex E are in the trait `KeyedBytesDataWriterOperations` for `DataWriter<…>` (`*_w_key`, each with `_w_timestamp`).

### Annex E KeyedBytesDataReader, KeyedBytesTypeSupport

**Spec:** Annex E, p. 337–338 — "interface KeyedBytesDataReader : DataReader {
[...] }; interface KeyedBytesTypeSupport : TypeSupport { [...] };"

**Repo:** `crates/dcps/src/builtin_types.rs::KeyedBytesTypeSupport`; `crates/zerodds-c-api/src/builtin_type_ffi.rs`; `crates/cpp/include/zerodds/builtin_type_support.hpp`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::KeyedBytesDataWriter`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::KeyedBytesDataReader`

**Tests:** `crates/dcps/tests/builtin_type_support.rs::each_type_support_names_its_type`; `crates/dcps/tests/builtin_type_support.rs::register_type_registers_each_type_under_a_name`; `crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`; `crates/zerodds-c-api/tests/builtin_types_c.rs::the_type_supports_name_and_register_their_types`; `crates/cpp/tests/builtin_data_endpoints_via_cargo.rs::the_data_writers_and_readers_of_the_built_in_types_write_and_take`; `crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java`

**Status:** done — `KeyedBytesTypeSupport` with `get_type_name` and `register_type` in Rust, the C ABI, C++ and Java; `KeyedBytesDataWriter` and `KeyedBytesDataReader` in Rust (aliases of `DataWriter<…>`, `DataReader<…>`), the C ABI and C++ (classes over the C ABI). In Java the interfaces are `DataWriter<T>` and `DataReader<T>` over `String`, `byte[]`, `KeyedString` and `KeyedBytes` by DDS-Java-PSM 1.0 §7.8.2; log in `.agent/REPORT.md`.

---

## Annex F: Characterizing Legacy DDS Implementations

### Annex F Introduction: non-normative

**Spec:** Annex F, p. 339 — "This annex provides a non-normative description of
what is believed to be the consensus Type System [...] of DDS implementations
that do not conform to this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — "non-normative".

### Annex F.1 Type System

**Spec:** Annex F.1, p. 339 — "The following portions of the Type System are
believed to be supported by the majority of DDS implementations".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — non-normative.

### Annex F.2 Type Representation

**Spec:** Annex F.2, p. 339 — "The IDL Type Representations of those portions of
the Type System enumerated above are generally supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — non-normative.

### Annex F.3 Data Representation

**Spec:** Annex F.3, p. 340 — "The exception is the extended parameter ID and
length facility based on PID_EXTENDED, which is not generally supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — non-normative.

### Annex F.4 Language Binding

**Spec:** Annex F.4, p. 340 — "The Plain Language Bindings of those portions of
the Type System enumerated above are generally supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — non-normative.

---

## Audit-Status

1105 done / 0 partial / 0 open / 93 n/a (informative) / 0 n/a (rejected).

Test run: to be added on the Linux test host.

