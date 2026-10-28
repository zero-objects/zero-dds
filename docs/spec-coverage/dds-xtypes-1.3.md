# DDS-XTypes 1.3 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (340 Seiten, OMG formal/2020-02-04) —
[OMG DDS-XTypes 1.3](https://www.omg.org/spec/DDS-XTypes/1.3/)

Audit Item für Item gegen die PDF nach `internal/spec-coverage/PROCESS.md`.
Seitenangaben sind PDF-Seiten (PDF-Seite = gedruckte Seite + 16). Jede Regel
der Typ-Zuweisbarkeit, jede Tabellenzeile, jede Annotation, jede
Serialisierungsregel, jede Operation der Dynamic Language Binding und jedes
Feld der Built-in-Topic-Erweiterungen ist ein eigenes Item. `done` steht nur,
wo Code und Test die Anforderung abdecken; ein zitierter Test nennt die
Testfunktion als `datei::modul::name`. Eine Fundstelle im Repo nennt Datei, Symbol
und Zeile (Z.); die Zeilen beziehen sich auf den Stand, mit dem diese Datei
eingecheckt wurde. Jede Aussage über Verhalten ist aus dem Code gelesen; für
dieses Audit wurde kein Test ausgeführt und nichts gebaut.

**Kontext:** XTypes ist in ZeroDDS über diese Crates verteilt:

- `crates/ast/` — das gemeinsame Typmodell (`TypeKind`, `PrimitiveKind`,
  `XTypesFacet` mit den XTypes-Annotationen).
- `crates/idl-parse/` und `crates/idl-lower/` — die IDL-Type-Representation:
  Parser, Annotationen, Vergabe der Member-IDs, Prüfregeln.
- `crates/types/` — TypeIdentifier, TypeObject (MINIMAL und COMPLETE),
  EquivalenceHash, Zuweisbarkeit, TypeLookup-Typen, TypeInformation, die
  dynamische Schicht (`DynamicType`, `DynamicData`) und die Aushandlung der
  Data Representation.
- `crates/ast-layout/` — der Plan der Serialisierung: Alignment, DHEADER, EMHEADER,
  Längencodes, KeyHolder, Parameterliste von XCDR1.
- `crates/wire/` und `crates/wire-codec/` — Leser und Schreiber der
  Oktette, der planbasierte Encoder und Decoder, MD5 und TryConstruct.
- `crates/lang-rules/` — die Codegeneratoren der Sprachbindungen.
- `crates/xml/` — XML- und XSD-Type-Representation und XML-Daten.
- `crates/rtps/`, `crates/discovery/` und `crates/dcps/` — die Nutzung durch
  DDS: Parameter der Built-in-Topics, TypeLookup-Endpunkte, Matching mit
  Typkonsistenz (`runtime.rs`), DataRepresentation-QoS, Built-in-Typen.

Die Rust-API von `zerodds-dcps` ist keine Abbildung der IDL von Annex C:
Operationen geben `Result<T, E>` statt `DDS::ReturnCode_t` zurück und
Sammlungen sind `Vec`. Wo eine Operation der Spec in der Rust-API fehlt, ist
das als eigenes Item geführt.

Die Plain Language Binding ist für die drei Sprachen geprüft, für die §7.5.1
Tabellen enthält (C, C++, Java), sowie für Rust. Die Codegeneratoren der
übrigen Sprachen in `crates/lang-rules/` sind nicht Gegenstand dieses Audits.

**Negativliste:** `dds-xtypes-1.3.gaps.md` ist das Ergebnis des Werkzeugs
`tools/xtypes-1.3-gap-scan/` und nicht Teil dieses Audits.

**Offene Punkte:** keine; jedes Item ist `done` oder `n/a (informative)`.

---

## §1 Scope

### §1 Scope

**Spec:** §1, S. 17–18 — "The specification addresses four related concerns:
the type system, the representation of types, the representation of data, and
the language bindings used to access types and data."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung der Spec in vier Pakete; die
Anforderungen stehen in §7.2 bis §7.6 und sind dort einzeln geführt.

---

## §2 Conformance Criteria

### §2 Konformitätsstufen Minimal, Basic, Complete

**Spec:** §2, S. 19 — "There are three conformance levels: Minimal conformance
with XTYPES version 1.2 requires conformance to the Programming Interface and
the Minimal Network Interoperability Profile. Basic conformance [...] the Basic
Network Interoperability Profile. Complete conformance [...] requires Basic
conformance as well as conformance to the two optional profiles."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung von Klauseln zu
Konformitätsstufen; ob eine Stufe erfüllt ist, folgt aus dem Status der
zugeordneten Items. Die Stufen nennen „XTYPES version 1.2“ in einer
Spec der Version 1.3 (Fehler der Spec, in der Befunddatei geführt).

### §2.1 Programming Interface Conformance

**Spec:** §2.1, S. 19 — Das Profil „Extensible and Dynamic Types“ besteht aus
Klausel 7 bis einschließlich 7.3, 7.5, 7.6 ohne 7.6.8, Annex B, Annex C und
"Annex D: Built-in Types".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung von Klauseln zum Profil; die
Klauseln sind einzeln geführt. Die Aufzählung nennt „Annex D: Built-in
Types“; in der PDF heißt Annex D „DDS Built-in Topic Data Types“ und Annex E
„Built-in Types“ (Fehler der Spec, in der Befunddatei geführt).

### §2.2 Network Interoperability Conformance

**Spec:** §2.2, S. 20 — "Regardless of profile, conformance with respect to
network interoperability requires conformance to the Real-Time
Publish-Subscribe Wire Protocol specification [RTPS]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf DDSI-RTPS; dessen Coverage
steht in `ddsi-rtps-2.5.md`.

### §2.2.1 Minimal Network Interoperability Profile

**Spec:** §2.2.1, S. 20 — umfasst 7.3.4, 7.6.1, 7.6.3 ohne 7.6.3.3 (mit
7.6.3.1.1 "with support limited to version 2 encoding"), 7.6.8, 7.4.2, 7.4.3,
Annex B und Annex D.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung von Klauseln zum Profil; die
Klauseln sind einzeln geführt.

### §2.2.2 Basic Network Interoperability Profile

**Spec:** §2.2.2, S. 20 — "Conformance with the Basic Network Interoperability
Profile requires conformance with the Minimal Network Interoperability profile
and the following clauses: 'Built-in TypeLookup service' (Clause 7.6.3.3)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; §7.6.3.3 ist einzeln geführt.

### §2.3 Optional XTYPES 1.1 Interoperability Profile

**Spec:** §2.3, S. 20 — "requires conformance with the Basic Network
Interoperability profile and support of version 1 encoding in Clause 7.6.3.1.1".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; die Encoding-Version 1 ist unter
§7.4.1 einzeln geführt.

### §2.4 Optional XML Data Representation Profile

**Spec:** §2.4, S. 21 — umfasst 7.3.2, 7.3.3, 7.4.4 und die Schemata von
Annex A.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; die Klauseln sind einzeln
geführt.

---

## §3 Normative References

### §3 Normative References

**Spec:** §3, S. 23–24 — Liste der normativen Dokumente ([DDS] 1.4, [RTPS] 2.3,
[IDL] 4.2, [CDR] CORBA 3.1 Part 2 §9.3, Sprachstandards, Sprachabbildungen,
[UNICODE] 9.0.0, [IEEE-754] u. a.).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweisliste.

---

## §4 Terms and Definitions

### §4 Terms and Definitions

**Spec:** §4, S. 25 — Definitionen von DCPS und DDS.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar.

---

## §5 Symbols

### §5 Symbols

**Spec:** §5, S. 27 — "No additional symbols are used in this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Symbole.

---

## §6 Additional Information

### §6.1 Data Distribution Service for Real-Time Systems (DDS)

**Spec:** §6.1, S. 29–31 — Motivation; Table 1 (bisherige Mechanismen) und
Table 2 (Merkmale dieser Spec).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hintergrund; die Merkmale aus Table 2 sind
in §7.2 bis §7.6 einzeln geführt.

### §6.2 Acknowledgments

**Spec:** §6.2, S. 31 — Liste der einreichenden Firmen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Danksagung.

---

## §7.1 Overview

### §7.1 Overview

**Spec:** §7.1, S. 33–35 — Figure 2 und Figure 3: Beziehungen zwischen Type
System, Type Representation, Language Binding und Data Representation am
Beispiel „AlarmType“.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsmodell; die Figure 3 ist
ausdrücklich „non-normative“.

---

## §7.2 Type System

### §7.2 Type System — Zweck

**Spec:** §7.2, S. 35 — "The Type System defines the data types that can be
used for DDS Topics and therefore the type of the data that can be published
and subscribed via DDS."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; das Modell folgt in §7.2.2.

### §7.2.1 Background (Non-Normative)

**Spec:** §7.2.1, S. 35–36 — Aufzählung der unterstützten primitiven und
konstruierten Typen; Verweis auf Type Evolution, Type Inheritance und Sparse
Types.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Überschrift sagt „Non-Normative“.

### §7.2.1.1 Type Evolution Example

**Spec:** §7.2.1.1, S. 36–37 — Beispiel `VehicleLocationType` mit vier
angehängten Membern.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel unter §7.2.1 (non-normative); die
Regeln stehen in §7.2.4.

### §7.2.1.2 Type Inheritance Example

**Spec:** §7.2.1.2, S. 37–38 — Beispiel `VehicleLocation3DType :
VehicleLocationType`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel unter §7.2.1 (non-normative).

### §7.2.1.3 Sparse Types Example

**Spec:** §7.2.1.3, S. 38 — optionale Member und ausgelassene Werte mit
Default.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel unter §7.2.1 (non-normative); die
Regeln stehen in §7.2.2.4.4.4.7 und §7.4.

---

## §7.2.2 Type System Model

### §7.2.2 Typname: nicht leer, eindeutig im Namensraum, gültiger IDL-Identifier

**Spec:** §7.2.2, S. 39 — "A type has a non-empty name that is unique within
its namespace (see Clause 7.2.2.1). The set of valid names is the set of valid
identifiers defined by the OMG IDL specification [IDL]."

**Repo:** `crates/idl-lower/src/lower.rs::duplicate` (Z. 3734);
`crates/ast/src/invariant.rs::NameCollidesOnlyInCase` (Z. 99);
`crates/idl-lower/src/scope.rs::is_bound_here` (Z. 160)

**Tests:** `crates/idl-lower/tests/lowering.rs::two_names_differing_only_by_case_collide`,
`crates/idl-lower/tests/lowering.rs::a_name_declared_in_two_openings_of_one_module_collides`,
`crates/idl-lower/tests/validation.rs::the_same_name_in_two_scopes_does_not_collide`

**Status:** done — die IDL-Front-End-Regeln für Identifier gelten; zwei
Deklarationen gleichen Namens in einem Scope (auch nur in der Schreibweise
verschieden) werden mit `DuplicateName` abgewiesen.

### §7.2.2 Typart (kind)

**Spec:** §7.2.2, S. 39 — "A type has a kind that identifies which primitive
type it is or, if it is a constructed type, whether it is a structure, union,
sequence, etc."

**Repo:** `crates/ast/src/node/type_.rs::TypeKind` (Z. 201);
`crates/types/src/dynamic/descriptor.rs::TypeKind` (Z. 48);
`crates/types/src/type_object/kinds.rs::TK_STRUCTURE` (Z. 26)

**Tests:** `crates/types/src/dynamic/descriptor.rs::tests::every_model_primitive_has_a_kind_and_comes_back`,
`crates/types/src/dynamic/descriptor.rs::tests::the_classifications_are_disjoint`

**Status:** done

### §7.2.2 Primitive und konstruierte Typen

**Spec:** §7.2.2, S. 39 — "The type system supports Primitive Types [...] whose
names are predefined. [...] The type system supports Constructed Types whose
names are explicitly provided as part of the type-definition process."

**Repo:** `crates/ast/src/node/type_.rs::PrimitiveKind` (Z. 17);
`crates/ast/src/node/type_.rs::TypeNode` (Z. 444, Feld `name`)

**Tests:** `crates/ast/tests/node_space.rs::every_primitive_of_the_spec_table_is_representable`,
`crates/idl-lower/tests/lowering.rs::a_struct_carries_its_members_in_order`

**Status:** done

### §7.2.2.1 Namensraum: gleiche Namen im selben Namensraum sind ein Fehler

**Spec:** §7.2.2.1, S. 39 — "it is an error for different elements within the
same namespace to have the same name. However, it is legal for different
elements within different namespaces to have the same name."

**Repo:** `crates/idl-lower/src/lower.rs::duplicate` (Z. 3734);
`crates/ast/src/invariant.rs::DuplicateMemberName` (Z. 56);
`crates/ast/src/invariant.rs::DuplicateEnumeratorName` (Z. 63)

**Tests:** `crates/idl-lower/tests/validation.rs::two_members_of_one_aggregate_differing_only_by_case_collide`,
`crates/idl-lower/tests/validation.rs::the_same_name_in_two_scopes_does_not_collide`,
`crates/idl-lower/tests/lowering.rs::one_literal_name_in_two_enumerations_collides`

**Status:** done

### §7.2.2.1 Module als Namensraum, vollqualifizierter Name

**Spec:** §7.2.2.1, S. 40 — "Modules are namespaces whose contained named
elements are types. The concatenation of module names with the name of a type
inside of those modules is referred to as the type's 'fully qualified name.'"

**Repo:** `crates/idl-lower/src/scope.rs::qualify` (Z. 199);
`crates/ast/src/node/type_.rs::TypeNode` (Z. 444, Felder `module`, `scope`)

**Tests:** `crates/idl-lower/tests/lowering.rs::nested_modules_nest_in_the_model_too`,
`crates/idl-lower/tests/lowering.rs::a_module_reopened_across_two_files_converges_on_one_node`,
`crates/idl-lower/tests/reported_idl.rs::a_fully_qualified_scoped_name_resolves`

**Status:** done

### §7.2.2.1 Typen als Namensraum ihrer Elemente

**Spec:** §7.2.2.1, S. 40 — "Certain kinds of types are themselves namespaces
with respect to the elements inside of them."

**Repo:** `crates/ast/src/invariant.rs::DuplicateMemberName` (Z. 56);
`crates/ast/src/invariant.rs::DuplicateEnumeratorName` (Z. 63)

**Tests:** `crates/idl-lower/tests/validation.rs::two_members_of_one_aggregate_differing_only_by_case_collide`,
`crates/idl-lower/tests/validation.rs::a_case_member_named_like_its_union_collides`

**Status:** done — Member eines Aggregats und Literale einer Aufzählung sind
je Typ eindeutig. Die Literale einer IDL-Aufzählung liegen nach IDL 4.2 im
umgebenden Scope; zwei Aufzählungen eines Scopes mit gleichem Literal werden
abgewiesen (`one_literal_name_in_two_enumerations_collides`).

---

## §7.2.2.2 Primitive Types

### §7.2.2.2 Primitive Typen ohne Modul, Namen auf oberster Ebene

**Spec:** §7.2.2.2, S. 43 — "The primitive types do not exist within any module;
their names are top-level names." Figure 6–8: `{nested = true}`,
`{extensibility_kind = <Not Applicable>}`.

**Repo:** `crates/ast/src/node/type_.rs::PrimitiveKind` (Z. 17);
`crates/types/src/type_identifier/mod.rs::PrimitiveKind` (Z. 315)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done — ein primitiver Typ ist im TypeIdentifier ein
Diskriminator ohne Rumpf, ohne Namen und ohne Extensibility.

### §7.2.2.2 Table 3 — BOOLEAN_TYPE

**Spec:** §7.2.2.2, Table 3, S. 42 — "BOOLEAN_TYPE Boolean Boolean type. Data
of this type can only take two values: true and false."

**Repo:** `crates/ast/src/node/type_.rs::Boolean` (Z. 19);
`crates/types/src/type_identifier/kinds.rs::TK_BOOLEAN` (Z. 21);
`crates/types/src/from_ast.rs::primitive_kind` (Z. 1479)

**Tests:** `crates/ast/tests/node_space.rs::every_primitive_of_the_spec_table_is_representable`,
`crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`,
`crates/types/src/dynamic/descriptor.rs::tests::every_model_primitive_has_a_kind_and_comes_back`

**Status:** done

### §7.2.2.2 Table 3 — BYTE_TYPE

**Spec:** §7.2.2.2, Table 3, S. 42 — "BYTE_TYPE Byte Single opaque byte. A Byte
value has no numeric value."

**Repo:** `crates/ast/src/node/type_.rs::Octet` (Z. 21);
`crates/types/src/type_identifier/kinds.rs::TK_BYTE` (Z. 23);
`crates/types/src/from_ast.rs::primitive_kind` (Z. 1479, Arm Z. 1493)

**Tests:** `crates/types/src/from_ast.rs::primitive_kind_tests::octet_and_uint8_are_two_kinds`,
`crates/types/src/dynamic/descriptor.rs::tests::octet_and_uint8_are_two_kinds`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done — `octet` wird `TK_BYTE` (0x02) und bleibt von `uint8`
(`TK_UINT8`) getrennt.

### §7.2.2.2 Table 3 — INT_8_TYPE

**Spec:** §7.2.2.2, Table 3, S. 42 — "INT_8_TYPE Int8 Signed integer minimally
capable of representing values in the range -127 to +128."

**Repo:** `crates/ast/src/node/type_.rs::Int8` (Z. 27);
`crates/types/src/type_identifier/kinds.rs::TK_INT8` (Z. 43)

**Tests:** `crates/ast/tests/node_space.rs::every_primitive_of_the_spec_table_is_representable`,
`crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done — der Wertebereich ist `i8` (−128 bis +127). Die Tabelle
nennt „-127 to +128“, was für einen vorzeichenbehafteten 8-Bit-Typ nicht
darstellbar ist (Fehler der Spec, in der Befunddatei geführt).

### §7.2.2.2 Table 3 — UINT_8_TYPE

**Spec:** §7.2.2.2, Table 3, S. 42 — "UINT_8_TYPE UInt8 Unsigned integer
minimally capable of representing values in the range 0 to +255."

**Repo:** `crates/ast/src/node/type_.rs::UInt8` (Z. 29);
`crates/types/src/type_identifier/kinds.rs::TK_UINT8` (Z. 45);
`crates/types/src/from_ast.rs::primitive_kind` (Z. 1479, Arm Z. 1494)

**Tests:** `crates/types/src/from_ast.rs::primitive_kind_tests::octet_and_uint8_are_two_kinds`,
`crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — INT_16_TYPE

**Spec:** §7.2.2.2, Table 3, S. 42 — "INT_16_TYPE Int16 Signed integer minimally
capable of representing values in the range -32768 to +32767."

**Repo:** `crates/ast/src/node/type_.rs::Int16` (Z. 31);
`crates/types/src/type_identifier/kinds.rs::TK_INT16` (Z. 25)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — UINT_16_TYPE

**Spec:** §7.2.2.2, Table 3, S. 42 — "UINT_16_TYPE UInt16 Unsigned integer
minimally capable of representing values in the range 0 to +65535."

**Repo:** `crates/ast/src/node/type_.rs::UInt16` (Z. 33);
`crates/types/src/type_identifier/kinds.rs::TK_UINT16` (Z. 31)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — INT_32_TYPE

**Spec:** §7.2.2.2, Table 3, S. 42 — "INT_32_TYPE Int32 Signed integer minimally
capable of representing values in the range -2147483648 to +2147483647."

**Repo:** `crates/ast/src/node/type_.rs::Int32` (Z. 35);
`crates/types/src/type_identifier/kinds.rs::TK_INT32` (Z. 27)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — UINT_32_TYPE

**Spec:** §7.2.2.2, Table 3, S. 42 — "UINT_32_TYPE UInt32 Unsigned integer
minimally capable of representing values in the range 0 to +4294967295."

**Repo:** `crates/ast/src/node/type_.rs::UInt32` (Z. 37);
`crates/types/src/type_identifier/kinds.rs::TK_UINT32` (Z. 33)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — INT_64_TYPE

**Spec:** §7.2.2.2, Table 3, S. 43 — "INT_64_TYPE Int64 Signed integer minimally
capable of supporting values in the range -9223372036854775808 to
+9223372036854775807."

**Repo:** `crates/ast/src/node/type_.rs::Int64` (Z. 39);
`crates/types/src/type_identifier/kinds.rs::TK_INT64` (Z. 29)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — UINT_64_TYPE

**Spec:** §7.2.2.2, Table 3, S. 43 — "UINT_64_TYPE UInt64 Unsigned integer
minimally capable of supporting values in the range 0 to
+18446744073709551617."

**Repo:** `crates/ast/src/node/type_.rs::UInt64` (Z. 41);
`crates/types/src/type_identifier/kinds.rs::TK_UINT64` (Z. 35)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done — der Wertebereich ist `u64` (0 bis 18446744073709551615).
Die Tabelle nennt als Obergrenze 18446744073709551617, also 2^64 + 1 (Fehler
der Spec, in der Befunddatei geführt).

### §7.2.2.2 Table 3 — FLOAT_32_TYPE

**Spec:** §7.2.2.2, Table 3, S. 43 — "FLOAT_32_TYPE Float32 Floating point
number minimally capable of supporting the range and precision of an IEEE 754
single-precision floating point value."

**Repo:** `crates/ast/src/node/type_.rs::Float32` (Z. 43);
`crates/types/src/type_identifier/kinds.rs::TK_FLOAT32` (Z. 37)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — FLOAT_64_TYPE

**Spec:** §7.2.2.2, Table 3, S. 43 — "FLOAT_64_TYPE Float64 Floating point
number minimally capable of supporting the range and precision of an IEEE 754
double-precision floating point value."

**Repo:** `crates/ast/src/node/type_.rs::Float64` (Z. 45);
`crates/types/src/type_identifier/kinds.rs::TK_FLOAT64` (Z. 39)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — FLOAT_128_TYPE

**Spec:** §7.2.2.2, Table 3, S. 43 — "FLOAT_128_TYPE Float128 Floating point
number minimally capable of supporting the range and precision of an IEEE 754
quadruple-precision floating point value."

**Repo:** `crates/ast/src/node/type_.rs::Float128` (Z. 51);
`crates/types/src/type_identifier/kinds.rs::TK_FLOAT128` (Z. 41);
`crates/types/src/from_ast.rs::primitive_kind` (Z. 1479, Arm Z. 1506)

**Tests:** `crates/ast/tests/node_space.rs::every_primitive_of_the_spec_table_is_representable`,
`crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done — `long double` ist im Typmodell und im TypeIdentifier
vorhanden; der Wert wird über `zerodds_float128::Float128` geführt (siehe
§7.4.1.1.1, Table 31).

### §7.2.2.2 Table 3 — CHAR_8_TYPE

**Spec:** §7.2.2.2, Table 3, S. 43 — "CHAR_8_TYPE Char8 8-bit character type.
There is no encoding specified, it may be ASCII, ISO-8859-1, or used to hold a
byte of a multi-byte-encoded character set."

**Repo:** `crates/ast/src/node/type_.rs::Char` (Z. 23);
`crates/types/src/type_identifier/kinds.rs::TK_CHAR8` (Z. 47)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2 Table 3 — CHAR_16_TYPE

**Spec:** §7.2.2.2, Table 3, S. 43 — "CHAR_16_TYPE Char16 16-bit character type
capable of supporting the Basic Multilingual Plane (BMP) encoded in UTF-16."

**Repo:** `crates/ast/src/node/type_.rs::WChar` (Z. 25);
`crates/types/src/type_identifier/kinds.rs::TK_CHAR16` (Z. 49)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.2.2.2.1.1 Design Rationale (Non-Normative)

**Spec:** §7.2.2.2.1.1, S. 43 — Begründung für zwei Zeichentypen Char8 und
Char16.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Überschrift sagt „Non-Normative“.

### §7.2.2.2.1.2.1 Use of Unicode

**Spec:** §7.2.2.2.1.2.1, S. 43–44 — "This specification uses the Unicode
Standard (version 9.0, June 2016) as the means to represent characters and
strings." Es folgen Erläuterungen zu Codepoints, Planes, UTF-8, UTF-16.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung; die Anforderungen
folgen in §7.2.2.2.1.2.2 bis .7.

### §7.2.2.2.1.2.2 CHAR_8_TYPE: keine Codierung, 8 Bit

**Spec:** §7.2.2.2.1.2.2, S. 44 — "This specification does not define an
encoding for the CHAR_8_TYPE. The only constraint is that it shall be
representable using 8 bits."

**Repo:** `crates/wire/src/writer.rs::write_char` (Z. 1524);
`crates/wire/src/reader.rs::read_char` (Z. 410)

**Tests:** `crates/wire/tests/char_width.rs::every_code_point_a_char_can_hold_round_trips`,
`crates/wire/tests/char_width.rs::a_char_above_u00ff_is_refused_and_not_narrowed`

**Status:** done — ein `char` ist ein Oktett. Ein `char`-Wert außerhalb von
Latin-1 wird vom Schreiber abgewiesen (§7.4.1.1.1).

### §7.2.2.2.1.2.3 Array oder Sequence von CHAR_8_TYPE: keine Codierung

**Spec:** §7.2.2.2.1.2.3, S. 44 — "This specification does not define an
encoding for the CHAR_8_TYPE that appears as an element of an array or
sequence of CHAR_8_TYPE."

**Repo:** `crates/wire/src/writer.rs::write_char` (Z. 1524)

**Tests:** `crates/wire/tests/char_width.rs::every_code_point_a_char_can_hold_round_trips`,
`crates/wire/tests/char_width.rs::a_char_above_u00ff_is_refused_and_not_narrowed`

**Status:** done — die Elemente werden einzeln als Oktett geschrieben; eine
Codierung über die Elemente hinweg wird nicht angenommen.

### §7.2.2.2.1.2.4 String<Char8>: UTF-8 in der Data Representation

**Spec:** §7.2.2.2.1.2.4, S. 44–45 — "The default encoding for String<Char8>
shall be UTF-8. This encoded shall be used for the externalized Data
Representation (see clause 7.4). Language bindings [...] If this is different
from UTF-8 the language binding shall manage the transformation to/from UTF-8
external Data Representation."

**Repo:** `crates/wire/src/writer.rs::write_string` (Z. 1881);
`crates/wire/src/reader.rs::read_string` (Z. 927)

**Tests:** `crates/wire/tests/wire.rs::a_string_of_bytes_that_are_not_utf8_is_refused`,
`crates/wire/tests/wire.rs::a_string_round_trips_with_its_terminator_consumed`

**Status:** done — der Schreiber schreibt die UTF-8-Oktette eines Rust-`str`,
der Leser weist Oktette ab, die kein UTF-8 sind (`WireError::NotUtf8`).

### §7.2.2.2.1.2.5 CHAR_16_TYPE: BMP, UTF-16

**Spec:** §7.2.2.2.1.2.5, S. 45 — "The CHAR_16_TYPE shall be restricted to
representing Unicode codepoints in the Basic Multilingual Plane. [...] The
CHAR_16_TYPE encoding shall be UTF-16."

**Repo:** `crates/wire/src/writer.rs::write_wchar` (Z. 1559)

**Tests:** `crates/wire/tests/char_width.rs::a_wchar_above_the_bmp_is_refused_and_not_narrowed`,
`crates/wire/tests/char_width.rs::the_reader_refuses_a_surrogate_rather_than_returning_something_else`

**Status:** done — ein `wchar` außerhalb der BMP wird vom Schreiber
abgewiesen („a BMP character, not U+…“).

### §7.2.2.2.1.2.6 Array oder Sequence von CHAR_16_TYPE: UTF-16, BMP

**Spec:** §7.2.2.2.1.2.6, S. 45 — "The representation of each CHAR_16_TYPE
element of an array or sequence of CHAR_16_TYPE shall be UTF-16 and shall be
restricted to being in the Basic Multilingual Plane".

**Repo:** `crates/wire/src/writer.rs::write_wchar` (Z. 1559)

**Tests:** `crates/wire/tests/char_width.rs::a_wchar_above_the_bmp_is_refused_and_not_narrowed`,
`crates/wire/tests/char_width.rs::the_reader_refuses_a_surrogate_rather_than_returning_something_else`

**Status:** done — jedes Element läuft durch denselben Schreiber wie ein
einzelnes `wchar`.

### §7.2.2.2.1.2.7 String<Char16>: UTF-16

**Spec:** §7.2.2.2.1.2.7, S. 45 — "The encoding for String<Char16> shall be
UTF-16. This encoded shall be used for the externalized Data Representation".

**Repo:** `crates/wire/src/writer.rs::write_wstring` (Z. 1980);
`crates/wire/src/reader.rs::read_wstring` (Z. 1027)

**Tests:** `crates/wire/tests/wstring.rs::every_cell_round_trips_under_every_unit_order`,
`crates/wire/tests/wstring.rs::an_unpaired_surrogate_is_refused_and_not_replaced`

**Status:** done — der Default `WstringLengthPolicy::SPECIFICATION` schreibt
zwei Oktette je UTF-16-Code-Unit. Die Herstellervarianten (vier Oktette je
Unit, UTF-8) sind über `WstringLengthPolicy` wählbar und nicht der Default.
Der Satz endet mit „If this is different from UTF-8“ statt UTF-16 (Fehler der
Spec, in der Befunddatei geführt).

### §7.2.2.3 String Types: Element Char8 oder Char16

**Spec:** §7.2.2.3, S. 45–46 — "the element type of a string must be either
Char8 or Char16 (or an alias to one of these); other element types are
undefined." Figure 9: String8 `{element_type = Char8}`, String16
`{element_type = Char16}`.

**Repo:** `crates/ast/src/node/type_.rs::CharWidth` (Z. 120);
`crates/types/src/type_identifier/kinds.rs::TI_STRING8_SMALL` (Z. 63)

**Tests:** `crates/types/src/assignability.rs::tests::string8_vs_string16_not_assignable`,
`crates/types/src/assignability.rs::tests::string_small_and_large_interchangeable`

**Status:** done — eine Zeichenkette hat im Modell nur die Breite `Char` oder
`WChar`; andere Elementtypen sind nicht darstellbar.

### §7.2.2.3 String Types: variable Länge bis zum Bound

**Spec:** §7.2.2.3, S. 45 — "StringTypes are variable-sized; objects of a given
string type can have different numbers of elements [...] the length of a given
string object may change between zero and the string type's 'bound'".

**Repo:** `crates/ast/src/node/type_.rs::Bound` (Z. 132);
`crates/wire/src/writer.rs::write_string_bounded` (Z. 1915)

**Tests:** `crates/idl-lower/tests/completion.rs::a_bounded_string_and_sequence_carry_their_bound_expressions`,
`crates/idl-lower/tests/completion.rs::an_unbounded_collection_says_so_rather_than_carrying_a_zero`

**Status:** done

---

## §7.2.2.4 Constructed Types

### §7.2.2.4 Konstruierte Typen, rekursive Definitionen

**Spec:** §7.2.2.4, S. 46–47 — "These other types may be either primitive types
or other constructed types: type definitions may be recursive to an arbitrary
depth. Constructed types are explicitly defined by a user [...] and are
assigned a name when they are defined."

**Repo:** `crates/types/src/from_ast.rs::structure` (Z. 2082);
`crates/types/src/resolve.rs::transitive_dependencies` (Z. 246)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_self_referential_type_gets_a_component_identifier`,
`crates/types/tests/strongly_connected_components.rs::a_mutually_recursive_pair_is_one_component_of_two`,
`crates/dcps/tests/recursive_topic_type_wire.rs::a_recursive_topic_type_is_announced_matched_and_fetched_as_a_component`

**Status:** done — rekursive Typen über Sequenzen und `@external` werden
gebaut; ihr TypeIdentifier ist eine strongly connected component (§7.3.4.9). Zwischen zwei Runtimes gemessen (Ankündigung, Match, `getTypes` der Komponente), Log `/root/zerodds-rc7/agent-logs/sec-access-1018-131826.log`.

### §7.2.2.4 Figure 10 — Arten konstruierter Typen

**Spec:** §7.2.2.4, Figure 10, S. 47 — "There are several kinds of Constructed
Types: Collections, Aggregated types, Aliases, and Enumerated types.
Collections are homogeneous [...] Aggregated types are heterogeneous [...]
Aliases introduce a new name for another type. Enumerated types define a
finite set of possible integer values".

**Repo:** `crates/ast/src/node/type_.rs::TypeKind` (Z. 201);
`crates/types/src/type_object/kinds.rs::TK_ALIAS` (Z. 15)

**Tests:** `crates/types/src/dynamic/descriptor.rs::tests::the_classifications_are_disjoint`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::every_constructed_form_can_be_defined_inside_a_typedef`

**Status:** done

---

## §7.2.2.4.1 Enumerated Types

### §7.2.2.4.1 EnumeratedType: bit_bound

**Spec:** §7.2.2.4.1, Figure 11, S. 48 — `EnumeratedType` hat
"+ bit_bound: Int32"; Enumeration und Bitmask sind Unterklassen.

**Repo:** `crates/ast/src/query.rs::effective_bit_bound` (Z. 319);
`crates/ast/src/lib.rs::DEFAULT_BIT_BOUND` (Z. 66)

**Tests:** `crates/idl-lower/tests/completion.rs::an_enum_without_a_bit_bound_is_thirty_two_bits`,
`crates/idl-lower/tests/completion.rs::a_written_bit_bound_is_what_the_rule_reports`

**Status:** done — die Grenzen von `@bit_bound` sind unter §7.3.1.2.1.9
geführt.

### §7.2.2.4.1 EnumeratedType: root = false

**Spec:** §7.2.2.4.1, Figure 11, S. 48 — Constraint `{root = false}`.

**Repo:** `crates/ast/src/node/type_.rs::Enumeration` (Z. 258);
`crates/ast/src/node/type_.rs::Bitmask` (Z. 263)

**Tests:** `crates/idl-lower/tests/lowering.rs::an_enum_carries_its_enumerators_in_order`,
`crates/idl-lower/tests/lowering.rs::a_bitmask_carries_its_bits`

**Status:** done — Enumeration und Bitmask haben im Modell kein Feld für
einen Basistyp; eine Vererbung ist nicht darstellbar.

### §7.2.2.4.1 EnumeratedType: Extensibility FINAL oder APPENDABLE

**Spec:** §7.2.2.4.1, Figure 11, S. 48 — Constraint `{extensibility_kind =
FINAL or APPENDABLE}`; ebenso Table 12, S. 74: "These types may be final or
appendable on a type-by-type basis."

**Repo:** `crates/idl-lower/src/annotation.rs::set_extensibility` (Z. 1036)

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/idl-lower/tests/validation.rs::an_enum_or_bitmask_is_never_mutable`

**Status:** done — `@final` und `@appendable` erreichen eine Aufzählung und eine Bitmask; `@mutable` an beiden weist das Front-End ab (`validate.rs::extensibility_is_final_or_appendable`).

### §7.2.2.4.1 EnumeratedType: @mutable wird abgewiesen

**Spec:** §7.2.2.4.1, Figure 11, S. 48 — `{extensibility_kind = FINAL or
APPENDABLE}`.

**Repo:** `crates/idl-lower/src/validate.rs::extensibility_is_final_or_appendable`

**Tests:** `crates/idl-lower/tests/validation.rs::an_enum_or_bitmask_is_never_mutable`

**Status:** done — `crates/idl-lower/src/validate.rs::extensibility_is_final_or_appendable` weist `@mutable` an `enum` und `bitmask` ab; das Modell trägt kein `IS_MUTABLE` an einer Aufzählung.

### §7.2.2.4.1.1 Table 4 — ENUMERATION_TYPE: eine oder mehrere Literale

**Spec:** §7.2.2.4.1.1, Table 4, S. 49 — "An enumerated type defines a closed
set of one or more literal objects of that type."

**Repo:** `crates/idl-lower/src/validate.rs::body_is_not_empty` (Z. 175)

**Tests:** `crates/idl-lower/tests/validation.rs::an_enumeration_with_no_enumerator_is_refused`

**Status:** done

### §7.2.2.4.1.1 Table 4 — Name je Literal eindeutig

**Spec:** §7.2.2.4.1.1, Table 4, S. 49 — "Each object of a given enumerated type
has a name and an Int32 value that are each unique within that type."

**Repo:** `crates/ast/src/invariant.rs::DuplicateEnumeratorName` (Z. 63)

**Tests:** `crates/idl-lower/tests/lowering.rs::one_literal_name_in_two_enumerations_collides`

**Status:** done

### §7.2.2.4.1.1 Table 4 — Wert je Literal eindeutig

**Spec:** §7.2.2.4.1.1, Table 4, S. 49 — "[...] a name and an Int32 value that
are each unique within that type."

**Repo:** `crates/idl-lower/src/validate.rs::enumerator_values_are_unique`

**Tests:** `crates/idl-lower/tests/validation.rs::two_enumerators_of_one_value_are_rejected`

**Status:** done — `validate.rs::enumerator_values_are_unique` weist zwei Literale gleichen Werts ab (`enum E { @value(1) A, @value(1) B };`).

### §7.2.2.4.1.1 Table 4 — Wert ist Int32

**Spec:** §7.2.2.4.1.1, Table 4, S. 49 — "[...] has a name and an Int32 value".

**Repo:** `crates/idl-lower/src/validate.rs::enumerator_values_fit_int32`,
`crates/types/src/from_ast.rs::enumeration`

**Tests:** `crates/idl-lower/tests/validation.rs::an_enumerator_value_outside_int32_is_rejected`,
`crates/types/tests/type_system_rules.rs::an_enumerator_value_outside_int32_is_no_type_object`,
`crates/idl-lower/tests/completion.rs::an_explicit_enumerator_value_shifts_the_ones_after_it`

**Status:** `done` — das Front-End weist `@value` außerhalb von `i32` ab (`enumerator_values_fit_int32`, auch den Wert, den die implizite Nummerierung hinter `2147483647` erreicht); `from_ast::enumeration` meldet `FromAstError::OutOfRange`, wenn die Diagnose übergangen wird.

### §7.2.2.4.1.1 Table 4 — Wert außerhalb Int32: kein Test

**Spec:** §7.2.2.4.1.1, Table 4, S. 49 — Int32-Wert je Literal.

**Repo:** `crates/idl-lower/src/validate.rs::enumerator_values_fit_int32`,
`crates/types/src/from_ast.rs::enumeration`

**Tests:** `crates/idl-lower/tests/validation.rs::an_enumerator_value_outside_int32_is_rejected`,
`crates/types/tests/type_system_rules.rs::an_enumerator_value_outside_int32_is_no_type_object`

**Status:** `done` — Abweisung im Front-End (`validate.rs`) und im TypeObject-Bau (`from_ast.rs`), je mit Test; beide Formen (`Minimal`, `Complete`) melden `OutOfRange`.

### §7.2.2.4.1.1 Table 4 — Reihenfolge der Literale ist signifikant

**Spec:** §7.2.2.4.1.1, Table 4, S. 49 — "The order in which the literals of an
enumerated type are defined is significant to the definition of that type. For
example, some type representations may base the numeric values of the literals
on their order of definition."

**Repo:** `crates/ast/src/node/type_.rs::Enumeration` (Z. 258, Feld
`enumerators` in Deklarationsreihenfolge);
`crates/ast/src/query.rs::effective_enumerator_value` (Z. 559)

**Tests:** `crates/idl-lower/tests/lowering.rs::an_enum_carries_its_enumerators_in_order`,
`crates/idl-lower/tests/completion.rs::enumerators_without_values_run_from_zero_upwards`

**Status:** done

### §7.2.2.4.1.2 Table 5 — BITMASK_TYPE: Bound größer 0 und höchstens 64

**Spec:** §7.2.2.4.1.2, Table 5, S. 50 — "A bitmask defines a bound—the maximum
number of bits in the set—and identifies by name certain bits within the set.
The bound must be greater than zero and no greater than 64."

**Repo:** `crates/idl-lower/src/complete.rs::check_bitmask_bounds` (Z. 176)

**Tests:** `crates/idl-lower/tests/validation.rs::a_bit_bound_over_sixty_four_is_refused`,
`crates/idl-lower/tests/validation.rs::the_bit_forms_the_specification_admits_still_get_through`

**Status:** done

### §7.2.2.4.1.2 Bitmask: Bits mit Name und Index 0 bis bound − 1

**Spec:** §7.2.2.4.1.2, S. 50–51 — "Each bit in this subset is identified by
name and by an index, numbered from 0 to (bound – 1). The bitmask need not
identify every bit it reserves. Furthermore, the bits it does identify need
not be contiguous."

**Repo:** `crates/idl-lower/src/complete.rs::check_bitmask_bounds` (Z. 176)

**Tests:** `crates/idl-lower/tests/validation.rs::a_bit_position_outside_the_bitmasks_size_is_refused`,
`crates/idl-lower/tests/validation.rs::a_bitmask_with_more_values_than_its_size_is_refused`,
`crates/idl-lower/tests/completion.rs::bit_positions_run_from_zero_and_an_explicit_one_shifts_the_rest`

**Status:** done

### §7.2.2.4.1.2 Bitmask: keine Vorgabe für die Speicherform

**Spec:** §7.2.2.4.1.2, S. 51 — "It does not actually constrain such
representations to represent each 'bit' in the set as a single memory bit or
to align the bitmask in any particular way."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis ohne Anforderung; die
Data Representation steht in §7.4.1.1.3.

### §7.2.2.4.1.2.1 Design Rationale (Non-Normative)

**Spec:** §7.2.2.4.1.2.1, S. 51 — Begründung für Bitmasks.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Überschrift sagt „Non-Normative“.

---

## §7.2.2.4.2 Alias Types

### §7.2.2.4.2 Table 6 — ALIAS_TYPE: zweiter Name für einen Typ

**Spec:** §7.2.2.4.2, Table 6, S. 52 — "An alias type—also referred to as a
typedef [...]—applies an additional name to an already-existing type. [...] an
alias/typedef does not introduce a distinct type."

**Repo:** `crates/ast/src/node/type_.rs::Alias` (Z. 275);
`crates/types/src/from_ast.rs::alias` (Z. 2658);
`crates/types/src/resolve.rs::resolve_alias_chain`

**Tests:** `crates/idl-lower/tests/lowering.rs::a_typedef_becomes_an_alias_to_its_target`,
`crates/types/src/resolve.rs::tests::resolve_follows_single_alias_to_target`,
`crates/types/src/resolve.rs::tests::resolve_chained_aliases_two_deep`

**Status:** done — für Zuweisbarkeit und Schlüssel wird ein Alias auf seinen
Zieltyp aufgelöst; im TypeObject ist er ein eigenes `TK_ALIAS`-Objekt, wie
Annex B es vorsieht.

### §7.2.2.4.2 Figure 14 — Alias: nested = base_type.nested

**Spec:** §7.2.2.4.2, Figure 14, S. 52 — Constraint `{nested =
base_type.nested}`; Annotationen am Alias und am Basistyp
(`+base_annotation`).

**Repo:** `crates/types/src/from_ast.rs::alias` (Z. 2658)

**Tests:** `crates/idl-lower/tests/annotations.rs::an_alias_supplies_a_default_that_the_use_site_overrides`

**Status:** done — ein Alias ist kein Topic-Typ eigener Art; die
Annotationen am Basistyp (`@try_construct`) wirken über den Alias.

---

## §7.2.2.4.3 Collection Types

### §7.2.2.4.3 Collection: Elementtyp bei der Definition

**Spec:** §7.2.2.4.3, S. 53 — "The type of the element might be any other type,
primitive or constructed [...] and must be specified when the collection type
is defined." Figure 15: `{extensibility_kind = <Not Applicable>}`,
`{nested = true}`, `external: Boolean = false`.

**Repo:** `crates/ast/src/node/type_.rs::Sequence` (Z. 219);
`crates/ast/src/node/type_.rs::Array` (Z. 226);
`crates/ast/src/node/type_.rs::Map` (Z. 233)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_sequence_member_carries_its_element_type`

**Status:** done

### §7.2.2.4.3 Table 7 — ARRAY_TYPE: feste Größe, mehrdimensional

**Spec:** §7.2.2.4.3, Table 7, S. 54 — "Fixed-size multi-dimensional
collection. Arrays are of a fixed size in that all objects of a given array
type will have the same number of elements. [...] array types of higher
dimensionality are distinct from arrays of arrays of lower dimensionality."

**Repo:** `crates/ast/src/node/type_.rs::Array` (Z. 226, `dims`);
`crates/types/src/type_identifier/kinds.rs::TI_PLAIN_ARRAY_SMALL` (Z. 77)

**Tests:** `crates/idl-lower/tests/completion.rs::a_multi_dimensional_array_keeps_its_dimensions_in_order`,
`crates/idl-lower/tests/lowering.rs::dimensions_belong_to_the_declarator_and_not_to_the_typedef`,
`crates/types/src/assignability.rs::tests::plain_array_diff_bounds_is_no`

**Status:** done — ein mehrdimensionales Array trägt alle Dimensionen in
einem `array_bounds`; ein Array von Arrays über einen `typedef` ist ein
anderer TypeIdentifier.

### §7.2.2.4.3 Table 7 — SEQUENCE_TYPE: variable Länge bis zum Bound

**Spec:** §7.2.2.4.3, Table 7, S. 54 — "Variable-size single-dimensional
collection. [...] the length of a given sequence object may change between
zero and the sequence type's 'bound' [...]. Elements are addressed by a single
index."

**Repo:** `crates/ast/src/node/type_.rs::Sequence` (Z. 219);
`crates/types/src/type_identifier/kinds.rs::TI_PLAIN_SEQUENCE_SMALL` (Z. 72)

**Tests:** `crates/idl-lower/tests/completion.rs::a_bounded_string_and_sequence_carry_their_bound_expressions`,
`crates/lang-rules/tests/codec.rs::a_bounded_sequence_checks_its_bound_and_an_unbounded_one_has_none`

**Status:** done

### §7.2.2.4.3 Table 7 — MAP_TYPE: Schlüssel eindeutig je Map

**Spec:** §7.2.2.4.3, Table 7, S. 54 — "'Map value' elements are addressed by a
'map key' object, the value of which must be unique within a given map object.
The types of both of these are homogeneous within a given map type".

**Repo:** `crates/ast/src/node/type_.rs::Map` (Z. 233);
`crates/wire/src/reader.rs::read_map` (Z. 1764)

**Tests:** `crates/wire/tests/wire.rs::the_ordered_map_reader_refuses_a_stream_that_repeats_a_key`,
`crates/wire-conformance/tests/map_bytes.rs::a_stream_that_repeats_a_key_keeps_every_entry`

**Status:** done

### §7.2.2.4.3 Map: Schlüsseltypen Ganzzahl und Zeichenkette

**Spec:** §7.2.2.4.3, S. 55 — "Implementers of this specification need only
support key elements of signed and unsigned integer types and of narrow and
wide string types; the behavior of maps with other key element types is
undefined and may not be portable."

**Repo:** `crates/ast-layout/src/xcdr.rs::map_key_ordering` (Z. 1586);
`crates/ast-layout/src/xcdr.rs::key_ordering` (Z. 1605)

**Tests:** `crates/wire-conformance/tests/map_bytes.rs::the_recorded_row_round_trips_back_into_the_sample`

**Status:** done — Ganzzahl- und Zeichenkettenschlüssel werden geschrieben
und gelesen. Schlüssel anderer Typen werden angenommen; nur unter
`MapEntryOrder::AscendingKey` liefert `map_key_ordering` für Gleitkomma-,
`fixed`- und zusammengesetzte Schlüssel keine Ordnung, und der Aufrufer weist
ab.

### §7.2.2.4.3 Collection: Bound größer 0, unbeschränkt bei Sequence, String, Map

**Spec:** §7.2.2.4.3, S. 55 — "Bound: The maximum number of elements the
collection may contain (inclusively); it must be greater than zero. In the
cases of sequences, strings, and maps, the bound parameter may be omitted.
[...] (All arrays must be bounded.)"

**Repo:** `crates/idl-lower/src/validate.rs::bound_is_positive` (Z. 842);
`crates/ast/src/node/type_.rs::Bound` (Z. 132)

**Tests:** `crates/idl-lower/tests/validation.rs::a_zero_string_bound_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_negative_sequence_bound_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_zero_array_dimension_is_rejected`,
`crates/idl-lower/tests/completion.rs::an_unbounded_collection_says_so_rather_than_carrying_a_zero`

**Status:** done

### §7.2.2.4.3 Collection: Gesamtbound eines Arrays

**Spec:** §7.2.2.4.3, S. 55 — "Arrays have independent bounds on each of their
dimensions; they can also be said to have an overall bound, which is the
product of all of their dimensions' bounds."

**Repo:** `crates/ast/src/node/type_.rs::Array` (Z. 226, `dims`)

**Tests:** `crates/idl-lower/tests/completion.rs::a_multi_dimensional_array_keeps_its_dimensions_in_order`

**Status:** done

### §7.2.2.4.3 Collection: verschiedene Typen je Art, Elementtyp und Bound

**Spec:** §7.2.2.4.3, S. 55 — "a one-dimensional array of 10 integers, a
one-dimensional array of 10 short integers, a sequence of at most 10 integers,
and a sequence of an unspecified number of integers are all of different
types. However, all one-dimensional arrays of 10 integers are of the same
type."

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier`

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_array_small_3d_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done — ein anonymer Collection-Typ ist durch seinen plain
TypeIdentifier aus Art, Bound und Elementtyp bestimmt; gleiche Werte ergeben
denselben Identifier.

---

## §7.2.2.4.4 Aggregated Types

### §7.2.2.4.4.1 Member: Name, Typ, ID, Index, boolesche Eigenschaften

**Spec:** §7.2.2.4.4.1, S. 55–56 — "Each member has a string name, a type, an
integer ID, an integer index, and a set of boolean properties". Figure 16:
`id: UInt32`, `key: Boolean = false`, `member_index: UInt32`,
`must_understand: Boolean = false`, `optional: Boolean = false`,
`shared: Boolean = false`.

**Repo:** `crates/ast/src/node/member.rs::MemberNode` (Z. 18);
`crates/ast/src/facet/xtypes.rs::XTypesFacet` (Z. 242);
`crates/types/src/type_object/flags.rs::StructMemberFlag` (Z. 39)

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`,
`crates/ast-layout/tests/layout.rs::must_understand_is_set_exactly_when_the_facet_says_so`

**Status:** done — `key`, `must_understand`, `optional` und `external` sind
Felder der Facette und Bits in `StructMemberFlag`; das UML-Attribut `shared`
entspricht `@external` (`IS_EXTERNAL`).

### §7.2.2.4.4.1 Table 8 — Arten von Aggregaten

**Spec:** §7.2.2.4.4.1, S. 56 — "There are three kinds of Aggregated Types:
structures, unions, and annotations." Figure 16 nennt `STRUCTURE_TYPE`,
`UNION_TYPE`, `BITSET_TYPE`; Table 8 nennt `UNION_TYPE` und
`STRUCTURE_TYPE`.

**Repo:** `crates/types/src/type_object/kinds.rs::TK_STRUCTURE` (Z. 26);
`crates/types/src/type_object/kinds.rs::TK_UNION` (Z. 28);
`crates/types/src/type_object/kinds.rs::TK_ANNOTATION` (Z. 23);
`crates/types/src/type_object/kinds.rs::TK_BITSET` (Z. 30)

**Tests:** `crates/types/src/builder.rs::tests::struct_builder_basic_with_two_members`,
`crates/types/src/builder.rs::tests::union_builder_minimal_with_two_cases_and_default`,
`crates/types/tests/type_object_annotations.rs::an_annotation_declaration_has_a_type_object`,
`crates/types/src/builder.rs::tests::bitset_builder_two_fields`

**Status:** done — alle vier Arten haben ein TypeObject. Text, Figure 16 und
Table 8 nennen verschiedene Mengen von Arten (Widerspruch der Spec, in der
Befunddatei geführt; DDSXTY14-63 und DDSXTY15-5 zu Bitset, siehe
`docs/omg-xtypes-issues.md` §A2).

### §7.2.2.4.4.1 Table 8 — UNION_TYPE

**Spec:** §7.2.2.4.4.1, Table 8, S. 56 — "Discriminated exclusive aggregation
of members. Unions define a well-known discriminator member and a set of
type-specific members."

**Repo:** `crates/ast/src/node/type_.rs::Union` (Z. 249);
`crates/types/src/from_ast.rs::union` (Z. 2178)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_union_over_an_enumeration_lowers_with_its_labels_evaluated`,
`crates/types/src/builder.rs::tests::union_builder_minimal_with_two_cases_and_default`

**Status:** done

### §7.2.2.4.4.1 Table 8 — STRUCTURE_TYPE

**Spec:** §7.2.2.4.4.1, Table 8, S. 56 — "Non-exclusive aggregation of members.
A type designer may declare any number of members within a structure. Unlike
in a union, there are no implicit members in a structure, and values for
multiple members may coexist."

**Repo:** `crates/ast/src/node/type_.rs::Structure` (Z. 242);
`crates/types/src/from_ast.rs::structure` (Z. 2082)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_struct_carries_its_members_in_order`,
`crates/types/src/builder.rs::tests::struct_builder_basic_with_two_members`

**Status:** done

### §7.2.2.4.4.2 Structure: optionaler Basistyp, Kollision von Name oder ID

**Spec:** §7.2.2.4.4.2, S. 57 — "A structure can optionally extend one other
structure, its 'base_type.' In the event that there is a name or ID collision
between a structure and its base type, the definition of the derived structure
is ill-formed."

**Repo:** `crates/ast/src/node/type_.rs::Structure` (Z. 242, Feld `base`);
`crates/idl-lower/src/validate.rs::inheritance_rules` (Z. 415)

**Tests:** `crates/idl-lower/tests/keys_and_inheritance.rs::a_member_name_of_the_base_chain_cannot_be_declared_again`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_member_id_of_the_base_chain_cannot_be_given_again`

**Status:** done

### §7.2.2.4.4.3 Union: Name „discriminator“ für Member reserviert

**Spec:** §7.2.2.4.4.3, S. 57 — "The name of the discriminator member is always
'discriminator'; that name is reserved for union types and is not permitted for
type-specific union members."

**Repo:** `crates/idl-lower/src/validate.rs::union_member_rules`

**Tests:** `crates/idl-lower/tests/validation.rs::a_union_member_is_not_named_discriminator`

**Status:** done — `validate.rs::union_member_rules` weist einen Union-Member namens `discriminator` ab. Figure 18 nennt den Diskriminator `disc` (Widerspruch der Spec, in der Befunddatei geführt).

### §7.2.2.4.4.3 Union: Diskriminator ist der erste Member

**Spec:** §7.2.2.4.4.3, S. 57 — "The discriminator member is always considered
to be the first member of a union."

**Repo:** `crates/types/src/from_ast.rs::union` (Z. 2178, Nummerierung der
Zweige ab 1)

**Tests:** `crates/types/src/builder.rs::tests::union_builder_minimal_with_two_cases_and_default`

**Status:** done — die Zweige erhalten die Member-IDs ab 1, der
Diskriminator gilt als Member 0.

### §7.2.2.4.4.3 Union: kein Label für zwei Member

**Spec:** §7.2.2.4.4.3, S. 58 — "it is not allowed for multiple members to
explicitly identify the same discriminator value."

**Repo:** `crates/idl-lower/src/validate.rs::union_rules` (Z. 185)

**Tests:** `crates/idl-lower/tests/validation.rs::a_repeated_case_label_is_rejected`,
`crates/idl-lower/tests/validation.rs::distinct_case_labels_are_accepted`

**Status:** done

### §7.2.2.4.4.3 Union: höchstens ein Default-Member

**Spec:** §7.2.2.4.4.3, S. 58 — "At most one member of the union may be
identified as the 'default' member; any discriminator value that does not
explicitly identify another member is considered to identify the default
member."

**Repo:** `crates/ast/src/invariant.rs::MultipleDefaultCases` (Z. 84);
`crates/ast/src/node/type_.rs::Union` (Z. 249, `default_case`)

**Tests:** `crates/idl-lower/tests/validation.rs::two_default_cases_are_rejected`,
`crates/lang-rules/src/default_python.rs::tests::a_unions_default_case_is_what_an_unlisted_discriminator_selects`

**Status:** done

### §7.2.2.4.4.3 Union: nur der gewählte Member existiert

**Spec:** §7.2.2.4.4.3, S. 58 — "The value of the member associated with the
current value of the discriminator is the only member value considered to
exist in a given object of a union type at a given moment in time. [...] the
initial value of the newly observed member is undefined by the type system".

**Repo:** `crates/lang-rules/src/rust_union.rs`

**Tests:** `crates/lang-rules/tests/a_decoded_union_has_only_its_selected_case.rs::every_case_is_cleared_before_the_selected_one_is_read`

**Status:** done — eine Union wird in Rust als `enum` je Zweig erzeugt; ein
dekodierter Wert hält nur den gewählten Zweig.

### §7.2.2.4.4.3 Union: zulässige Diskriminatortypen

**Spec:** §7.2.2.4.4.3, S. 58–59 — "The discriminator of a union must be of one
of the following types: Boolean, Byte, Char8, Char16, Int8, UInt8, Int16,
UInt16, Int32, UInt32, Int64, UInt64, Any enumerated type, Any alias type that
resolves, directly or indirectly, to one of the aforementioned types."

**Repo:** `crates/idl-lower/src/validate.rs::is_legal_discriminator` (Z. 244)

**Tests:** `crates/idl-lower/tests/validation.rs::each_legal_discriminator_type_is_accepted`,
`crates/idl-lower/tests/validation.rs::an_enum_discriminator_is_accepted`,
`crates/idl-lower/tests/validation.rs::a_typedef_to_a_legal_discriminator_is_accepted`,
`crates/idl-lower/tests/validation.rs::a_float_discriminator_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_string_discriminator_is_rejected`

**Status:** done — „Any enumerated type“ umfasst nach §7.2.2.4.1 auch
Bitmask; `is_legal_discriminator` nimmt nur `enum` an. Bitmask als
Diskriminator wird abgewiesen (IDL 4.2 lässt ihn nicht zu).

### §7.2.2.4.4.4.1 Member Name: eindeutig im Aggregat

**Spec:** §7.2.2.4.4.4.1, S. 59 — "Each member of an Aggregated type is
identified by a string name. The name shall be unique within the scope of the
Aggregated type".

**Repo:** `crates/ast/src/invariant.rs::DuplicateMemberName` (Z. 56)

**Tests:** `crates/idl-lower/tests/validation.rs::two_members_of_one_aggregate_differing_only_by_case_collide`,
`crates/idl-lower/tests/lowering.rs::two_names_differing_only_by_case_collide`

**Status:** done

### §7.2.2.4.4.4.2 Member Type

**Spec:** §7.2.2.4.4.4.2, S. 59 — "Each member of an Aggregated type has an
associated member type."

**Repo:** `crates/ast/src/node/member.rs::MemberNode` (Z. 18, Feld `type_`)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_sequence_member_carries_its_element_type`

**Status:** done

### §7.2.2.4.4.4.3 Member Index: ab 0, lückenlos

**Spec:** §7.2.2.4.4.4.3, S. 59 — "The member index indicates the declaration
order of the member within the Aggegated type. [...] Member indices shall start
at zero and increment by one with the highest index being equal to the number
of members minus one."

**Repo:** `crates/ast/src/node/type_.rs::Structure` (Z. 242, `members` in
Deklarationsreihenfolge); `crates/ast/src/query.rs::all_members_of` (Z. 151)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_struct_carries_its_members_in_order`,
`crates/types/src/assignability.rs::tests::flatten_inheritance_two_levels_concatenates_base_first`

**Status:** done — der Index ist die Position im Member-Vektor; für eine
abgeleitete Struktur zählen die Member der Basis zuerst.

### §7.2.2.4.4.4.4 Member IDs: 28-Bit-Bereich

**Spec:** §7.2.2.4.4.4.4, S. 59 — "Member IDs are unsigned and have a range that
can be represented in 28 bits: from zero to 268,435,455 (0x0FFFFFFF)."

**Repo:** `crates/idl-lower/src/complete.rs::hash_id`,
`crates/idl-lower/src/annotation.rs::id_annotation`,
`crates/wire/src/writer.rs::write_emheader`

**Tests:** `crates/idl-lower/tests/validation.rs::a_member_id_is_at_most_28_bits`,
`crates/idl-lower/tests/completion.rs::a_hashed_id_stays_within_the_28_bit_member_id_space`,
`crates/idl-lower/src/complete.rs::tests::a_hashed_id_stays_inside_the_28_bit_space`,
`crates/wire/tests/wire.rs::an_emheader_member_id_past_28_bits_is_refused`

**Status:** `done` — `id_annotation` weist eine ID über 0x0FFFFFFF ab (`MAX_MEMBER_ID`), gehashte IDs werden auf 28 Bit maskiert, `write_emheader` weist eine ID über 0x0FFFFFFF beim Schreiben ab.

### §7.2.2.4.4.4.4 Member IDs: @id über 0x0FFFFFFF wird abgewiesen

**Spec:** §7.2.2.4.4.4.4, S. 59 — 28-Bit-Bereich der Member-IDs.

**Repo:** `crates/idl-lower/src/annotation.rs::id_annotation`

**Tests:** `crates/idl-lower/tests/validation.rs::a_member_id_is_at_most_28_bits`

**Status:** done — Das Front-End weist `@id` über `0x0FFFFFFF` ab (28 Bit, Annex B `MemberId`).

### §7.2.2.4.4.4.4 Member IDs: Bereich für Anwendungen

**Spec:** §7.2.2.4.4.4.4, S. 59 — "The remaining part of the member ID
range—from 0 to 268,402,687 (0x0FFFBFFF)—is available for use by
application-defined types compliant with this specification."

**Repo:** `crates/idl-lower/src/annotation.rs::id_annotation`

**Tests:** `crates/idl-lower/tests/validation.rs::an_application_member_id_in_the_available_range_is_accepted`

**Status:** `done` — jede ID von 0 bis 0x0FFFBFFF wird angenommen. Der Satz nennt, was Anwendungen zur Verfügung steht, und verbietet den Rest nicht; 0x0FFFC000 bis 0x0FFFFFFF werden ebenfalls angenommen, weil sie in den 28 Bit liegen und das Vendor-Korpus `@id(268435455)` schreibt. Der Satz bezieht sich auf einen Teil, den kein Text abgrenzt, und nennt dezimal 268,402,687 für hexadezimal 0x0FFFBFFF (DDSXTY14-8, `docs/omg-xtypes-issues.md` §A4).

### §7.2.2.4.4.4.5 Member Name Hashes: erste 4 Oktette von MD5(UTF-8)

**Spec:** §7.2.2.4.4.4.5, S. 59 — "the 4-byte hashes computed from strings
shall consist of the first 4 bytes of the MD5 hash of the UTF-8 encoding of
the string, without any 'nul' terminator. For example, computing the 4-byte
Hash of the member name 'color' shall result on the 4 octets {0x70, 0xDD,
0xA5, 0xDF}."

**Repo:** `crates/types/src/type_object/common.rs::from_name` (Z. 34);
`crates/idl-lower/src/complete.rs::hash_id` (Z. 1165);
`crates/wire/src/digest.rs::md5`

**Tests:** `crates/types/src/builder.rs::tests::member_id_from_name_matches_known_md5_vectors`,
`crates/idl-lower/src/complete.rs::tests::the_digest_matches_the_published_vectors`,
`crates/types/tests/compliance_typeobject.rs::md5_empty_string_first_14_bytes_golden_vector`

**Status:** done — der Test prüft „color“ als maskierte ID `0x0FA5DD70`,
also die Oktette `70 DD A5` und das maskierte vierte Oktett.

### §7.2.2.4.4.4.6 Must Understand: Verwerfen der Probe bei unbekanntem Member

**Spec:** §7.2.2.4.4.4.6, S. 60 — "If the attribute is true, a data consumer,
upon identifying a member it does not recognize, must discard the entire data
sample to which the member belongs. If the attribute is false, the consumer is
permitted to process the sample, omitting the value of the unrecognized
member."

**Repo:** `crates/wire-codec/src/decode.rs` (XCDR2 Z. 1485–1498, XCDR1
Z. 1573–1589); `crates/wire/src/reader.rs::read_emheader` (Z. 1265)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_unknown_mutable_member_is_skipped_and_a_must_understand_one_is_not`,
`crates/wire-probe/tests/decode_round_trip.rs::an_unknown_must_understand_parameter_discards_the_sample_under_xcdr1`

**Status:** done — die Erkennung je Data Representation steht unter §7.4.

### §7.2.2.4.4.4.6 Must Understand: Strukturmember unabhängig

**Spec:** §7.2.2.4.4.4.6, S. 60 — "In a structure type, each member may have the
'must understand' attribute set to true or false independently."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm
`must_understand` Z. 337); `crates/types/src/from_ast.rs::member_bits` (Z. 1633)

**Tests:** `crates/ast-layout/tests/layout.rs::must_understand_is_set_exactly_when_the_facet_says_so`

**Status:** done

### §7.2.2.4.4.4.6 Must Understand: Union-Diskriminator immer true

**Spec:** §7.2.2.4.4.4.6, S. 60 — "In a union type, the discriminator member
shall always have the 'must understand' attribute set to true."

**Repo:** `crates/types/src/from_ast.rs::union`,
`crates/ast-layout/src/xcdr.rs::plan_at (Diskriminator)`,
`crates/ast-layout/src/key.rs::MustUnderstandPolicy::DeclaredKeyAndDiscriminator`

**Tests:** `crates/types/tests/type_system_rules.rs::the_discriminator_of_a_union_is_must_understand_in_both_forms`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_discriminator_of_a_mutable_union_is_must_understand`,
`crates/ast-layout/tests/vendor_gate.rs::each_dialect_holds_what_its_implementation_was_measured_writing`

**Status:** `done` — das TypeObject setzt `IS_MUST_UNDERSTAND` am Diskriminator (Minimal und Complete). Auf der Leitung setzt `Dialect::Specification` (`MustUnderstandPolicy::DeclaredKeyAndDiscriminator`) `M_FLAG` (XCDR2) und `FLAG_MUST_UNDERSTAND` (XCDR1) am Member-Header des Diskriminators einer `@mutable`-Union. Die Dialekte `Rti` und `OpenDds` lassen das Bit gelöscht, wie gemessen (`union-mutable`, `vendor-corpus.tsv`); der Leser nimmt beide Formen.

### §7.2.2.4.4.4.6 Must Understand: kein Test des Bits am Diskriminator

**Spec:** §7.2.2.4.4.4.6, S. 60 — Diskriminator immer must understand.

**Repo:** `crates/types/src/from_ast.rs::union`,
`crates/ast-layout/src/xcdr.rs::plan_at (Diskriminator)`

**Tests:** `crates/types/tests/type_system_rules.rs::the_discriminator_of_a_union_is_must_understand_in_both_forms`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_discriminator_of_a_mutable_union_is_must_understand`

**Status:** `done` — beide Tests fragen das Bit ab: `IS_MUST_UNDERSTAND` im Minimal- und Complete-TypeObject, `0xA0000000` (XCDR2) und `0x4000` (XCDR1) im Member-Header; `Rti` schreibt `0x20000000`.

### §7.2.2.4.4.4.6 Must Understand: Erkennung je Data Representation

**Spec:** §7.2.2.4.4.4.6, S. 60 — "The ability of a consumer to detect the
presence of an unrecognized member depends on the Data Representation. Each
representation shall therefore define the means by which such detection
occurs."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Anforderung an die Data Representations
der Spec; die Mittel (`M_FLAG` im EMHEADER, Parameter-Header) sind unter §7.4
geführt.

### §7.2.2.4.4.4.7 Optional: jedes Objekt hat einen Wert je nicht-optionalem Member

**Spec:** §7.2.2.4.4.4.7, S. 60 — "Every object of a given type shall be
considered to contain a value for every non-optional member defined by that
type. In the event that no explicit value for such a member is ever provided
in a Data Representation of that object, that member is considered to
nevertheless have the default 'zero' value defined in Table 9 below."

**Repo:** `crates/wire-codec/src/default.rs::member` (Z. 100);
`crates/lang-rules/src/default_rust.rs::member_default` (Z. 48)

**Tests:** `crates/lang-rules/tests/codec.rs::an_appendable_tail_the_writer_did_not_have_takes_its_default`,
`crates/lang-rules/tests/c_end_to_end.rs::a_mutable_member_that_did_not_arrive_takes_its_default`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — BYTE: 0x00

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 60 — "BYTE 0x00".

**Repo:** `crates/wire-codec/src/default.rs::primitive` (Z. 237);
`crates/lang-rules/src/default_rust.rs::primitive_default` (Z. 349)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/tests/c_end_to_end.rs::every_table_nine_row_has_a_c_default`,
`crates/lang-rules/tests/cpp_reader_generates.rs::every_kind_table_9_answers_for_gets_a_defaulter`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — BOOLEAN: False

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 60 — "BOOLEAN False".

**Repo:** `crates/wire-codec/src/default.rs::primitive` (Z. 237);
`crates/lang-rules/src/default_rust.rs::primitive_default` (Z. 349)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/tests/c_end_to_end.rs::every_table_nine_row_has_a_c_default`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — Ganzzahl- und Gleitkommatypen: 0

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 60 — "INT_8_TYPE, UINT_8_TYPE, INT_16_TYPE,
UINT_16_TYPE, INT_32_TYPE, UINT_32_TYPE, INT_64_TYPE, UINT_64_TYPE,
FLOAT_32_TYPE, FLOAT_64_TYPE, FLOAT_128_TYPE 0".

**Repo:** `crates/wire-codec/src/default.rs::primitive` (Z. 237);
`crates/lang-rules/src/default_rust.rs::primitive_default` (Z. 349)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/tests/c_end_to_end.rs::every_table_nine_row_has_a_c_default`,
`crates/lang-rules/src/default_python.rs::tests::a_long_double_is_a_float128_and_never_a_float`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — CHAR_8_TYPE, CHAR_16_TYPE: '\0'

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 61 — "CHAR_8_TYPE, CHAR_16_TYPE '\0'".

**Repo:** `crates/wire-codec/src/default.rs::primitive` (Z. 237);
`crates/lang-rules/src/default_rust.rs::primitive_default` (Z. 349)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_char_is_a_one_character_string_and_not_an_integer`,
`crates/lang-rules/tests/cpp_reader_generates.rs::every_kind_table_9_answers_for_gets_a_defaulter`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — STRING_TYPE: ""

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 61 — "STRING_TYPE \"\"".

**Repo:** `crates/wire-codec/src/default.rs::at_depth` (Z. 152, Arm
`TypeKind::String`)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/tests/codec.rs::an_appendable_tail_the_writer_did_not_have_takes_its_default`,
`crates/lang-rules/tests/c_end_to_end.rs::every_table_nine_row_has_a_c_default`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — ARRAY_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 61 — "An array of the same dimensions and
same element type whose elements take the default value for their
corresponding type."

**Repo:** `crates/wire-codec/src/default.rs::array` (Z. 304)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_fixed_size_array_is_that_many_elements`,
`crates/lang-rules/src/default_python.rs::tests::a_two_dimensional_array_is_one_tuple_per_dimension`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — ALIAS_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 61 — "The default type of the alias's base
type."

**Repo:** `crates/wire-codec/src/default.rs::at_depth` (Z. 152, Arm
`TypeKind::Alias`); `crates/lang-rules/src/default_rust.rs::resolve_alias`
(Z. 516)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_typedef_and_its_target_are_one_function_and_it_does_not_call_itself`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — SEQUENCE_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 61 — "A zero-length sequence of the same
element type."

**Repo:** `crates/wire-codec/src/default.rs::at_depth` (Z. 152, Arm
`TypeKind::Sequence`)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_sequence_is_an_empty_list_at_every_bound`,
`crates/lang-rules/tests/c_end_to_end.rs::every_table_nine_row_has_a_c_default`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — MAP_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 61 — "An empty map of the same element
type."

**Repo:** `crates/wire-codec/src/default.rs::at_depth` (Z. 152, Arm
`TypeKind::Map` Z. 202)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — ENUM_TYPE: erstes Literal

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 61 — "The first literal in the
enumeration."

**Repo:** `crates/wire-codec/src/default.rs::first_enumerator` (Z. 270);
`crates/lang-rules/src/default_rust.rs::default_enumerator` (Z. 370)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/tests/cpp_reader_generates.rs::an_enumeration_whose_first_literal_is_not_zero`,
`crates/lang-rules/src/default_python.rs::tests::an_enumeration_takes_its_first_literal_by_name`

**Status:** done — mit `@default_literal` gilt das so markierte Literal
(`an_enumeration_with_a_default_literal_takes_that_one`); Table 9 und
§7.3.1.2.1.12.5 widersprechen sich hier (`docs/omg-xtypes-issues.md` §B5).

### §7.2.2.4.4.4.7 Table 9 — UNION_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 61 — "A union with the discriminator set
to the default value for the discriminator type. If this selects a branch,
then the selected member is also set to the default value for the member type.
Otherwise the value of the union is fully specified by the discriminator
value."

**Repo:** `crates/wire-codec/src/default.rs::union` (Z. 366);
`crates/lang-rules/src/default_rust.rs::discriminator_default_value` (Z. 393)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_union_whose_discriminators_default_selects_a_case_sets_both`,
`crates/lang-rules/src/default_python.rs::tests::a_union_whose_discriminators_default_selects_nothing_is_the_discriminator_alone`,
`crates/lang-rules/tests/c_end_to_end.rs::a_union_member_the_stream_does_not_carry_takes_its_default`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — STRUCTURE_TYPE

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 61 — "A structure without any of the
optional members and with other members set to their default values based on
their corresponding types."

**Repo:** `crates/wire-codec/src/default.rs::structure` (Z. 330)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/lang-rules/src/default_python.rs::tests::a_structure_of_primitives_names_every_required_field`,
`crates/lang-rules/src/default_python.rs::tests::an_optional_member_is_absent_from_the_constructor_call`

**Status:** done

### §7.2.2.4.4.4.7 Table 9 — Bitmask und Bitset ohne Zeile

**Spec:** §7.2.2.4.4.4.7, Table 9, S. 60–61 — Table 9 hat keine Zeile für
`BITMASK_TYPE` und `BITSET_TYPE`.

**Repo:** `crates/wire-codec/src/default.rs::at_depth` (Z. 152, Arme
`TypeKind::Bitmask`, `TypeKind::Bitset`)

**Tests:** `crates/wire-probe/tests/evolution.rs::table_nine_gives_every_kind_the_value_the_clause_names`,
`crates/wire-conformance/tests/bitmask_bytes.rs::table_9_gives_a_bitmask_no_bit_and_a_bitset_no_field`,
`crates/lang-rules/src/default_python.rs::tests::a_bitset_is_the_dataclass_with_every_field_at_zero`

**Status:** done — der Baum nimmt „kein Bit gesetzt“ und „jedes Feld 0“.
Die fehlende Zeile ist ein Mangel der Spec (DDSXTY14-60, in der Befunddatei
geführt).

### §7.2.2.4.4.4.7 Optional: ausgelassener Wert ohne Default

**Spec:** §7.2.2.4.4.4.7, S. 61 — "An object may omit a value for any optional
member(s) defined by its type. [...] Implementations shall not provide a
default value in such a case."

**Repo:** `crates/lang-rules/src/default_rust.rs::member_default` (Z. 48)

**Tests:** `crates/lang-rules/src/default_python.rs::tests::an_optional_member_is_absent_from_the_constructor_call`

**Status:** done — ein `@optional`-Member wird `None`, nicht Table 9.

### §7.2.2.4.4.4.7 Optional: Union-Member nie optional

**Spec:** §7.2.2.4.4.4.7, S. 61 — "Union members, including the discriminator,
shall never be optional."

**Repo:** `crates/idl-lower/src/validate.rs::union_member_rules` (Z. 768)

**Tests:** `crates/idl-lower/tests/validation.rs::no_union_member_is_optional`

**Status:** done — ein `@optional` am Diskriminator steht an keiner
zulässigen Position einer Annotation und wird von `apply_use_site` gemeldet.

### §7.2.2.4.4.4.7 Optional: Strukturmember je Member wählbar

**Spec:** §7.2.2.4.4.4.7, S. 61 — "Structure members may be optional. The
designer of a structure can choose which members are optional on a
member-by-member basis."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm
`optional` Z. 335); `crates/types/src/from_ast.rs::member_bits` (Z. 1633)

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`

**Status:** done

### §7.2.2.4.4.4.7 Optional und Must Understand unabhängig

**Spec:** §7.2.2.4.4.4.7, S. 61 — "The value of a member's 'optional' attribute
is unrelated to the value of its 'must understand' attribute."

**Repo:** `crates/types/src/from_ast.rs::member_bits` (Z. 1633)

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`

**Status:** done

### §7.2.2.4.4.4.8 Key Members: Schlüssel des Typs ist Schlüssel des Topics

**Spec:** §7.2.2.4.4.4.8, S. 61 — "The type's key will become the key of any DDS
Topic that is constructed using the aforementioned aggregated type as the
Topic's type. If a given type has no members designated as key members, then
the type [...] has no key."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_members` (Z. 254);
`crates/zerodds-build/src/dds_type.rs`

**Tests:** `crates/ast-layout/tests/vendor_gate.rs::a_keyless_topic_type_still_reports_no_key_members`,
`crates/ast-layout/tests/wire_vectors.rs::a_type_with_no_key_member_has_no_key_members_and_a_whole_type_holder`

**Status:** done

### §7.2.2.4.4.4.8 Key Members: nie optional

**Spec:** §7.2.2.4.4.4.8, S. 61 — "Key members shall never be optional".

**Repo:** `crates/idl-lower/src/validate.rs::member_rules` (Z. 327, Prüfung
Z. 348)

**Tests:** `crates/idl-lower/tests/validation.rs::a_key_member_must_not_be_optional`

**Status:** done

### §7.2.2.4.4.4.8 Key Members: must understand immer true

**Spec:** §7.2.2.4.4.4.8, S. 61 — "[...] and they shall always have their 'must
understand' attribute set to true."

**Repo:** `crates/types/src/from_ast.rs::member_bits`,
`crates/types/src/assignability.rs::must_not_be_missing`,
`crates/ast-layout/src/xcdr.rs::member_must_understand`

**Tests:** `crates/types/tests/type_system_rules.rs::a_key_member_is_must_understand_in_both_forms`,
`crates/types/tests/table19_key_members.rs::a_writer_enum_key_literal_the_reader_lacks_is_not_assignable`,
`crates/wire-probe/tests/opendds_mutable_framing.rs::the_key_rows_are_reproduced`,
`crates/wire-probe/tests/opendds_mutable_framing.rs::the_clause_reaches_only_the_specification_dialect`

**Status:** `done` — das TypeObject setzt `IS_MUST_UNDERSTAND` an jedem Key-Member (Minimal und Complete), die Zuweisbarkeit behandelt einen Key-Member wie einen Member mit must understand. Auf der Leitung setzt der Default `Dialect::Specification` `M_FLAG` auch an Key-Membern; `Cyclone`, `Rti` und `FastDds` setzen es nicht, wie gemessen (`docs/omg-xtypes-issues.md` §B3).

### §7.2.2.4.4.4.8 Key Members: kein Test des Bits im TypeObject

**Spec:** §7.2.2.4.4.4.8, S. 61 — must understand an Key-Membern.

**Repo:** `crates/types/src/from_ast.rs::member_bits`

**Tests:** `crates/types/tests/type_system_rules.rs::a_key_member_is_must_understand_in_both_forms`

**Status:** `done` — der Test fragt `IS_KEY` und `IS_MUST_UNDERSTAND` an Key-Member und Nicht-Key-Member im Minimal- und Complete-TypeObject ab. Ein Vergleich mit den TypeObjects anderer Hersteller (gleicher `EquivalenceHash`) ist nicht gemessen.

### §7.2.2.4.4.4.8 Key Members: zulässige Typen, keine Map

**Spec:** §7.2.2.4.4.4.8, S. 62 — "A type's key can only include members of the
following types: primitive, aggregation, enumeration, bitmask, array, and
sequence. Aliases to one of the previous types can also be used as key
members. Members of type map cannot be included as part of the key."

**Repo:** `crates/idl-lower/src/validate.rs::key_member_type_rules` (Z. 683);
`crates/idl-lower/src/validate.rs::map_in_key` (Z. 714)

**Tests:** `crates/idl-lower/tests/validation.rs::a_map_cannot_be_part_of_a_key`,
`crates/idl-lower/tests/validation.rs::a_map_that_is_not_in_the_key_is_accepted`,
`crates/idl-lower/tests/validation.rs::a_string_key_is_accepted_although_the_clause_does_not_list_it`

**Status:** done — `string` fehlt in der Liste der Spec, ist aber Schlüssel
ihres eigenen Beispiels (§7.6.8) und des ShapeType; der Baum nimmt ihn an
(Widerspruch der Spec, in der Befunddatei geführt).

### §7.2.2.4.4.4.8 Key Members: Struktur — beliebige Member

**Spec:** §7.2.2.4.4.4.8, S. 62 — "In a structure type, the key designation can
be applied to any member and to any number of members."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm `key` Z. 334)

**Tests:** `crates/idl-lower/tests/validation.rs::key_on_its_own_is_accepted`,
`crates/ast-layout/tests/wire_vectors.rs::key_members_come_back_in_member_id_order_not_declaration_order`

**Status:** done

### §7.2.2.4.4.4.8 Key Members: Union — nur der Diskriminator

**Spec:** §7.2.2.4.4.4.8, S. 62 — "In a union type, only the discriminator is
permitted to be a key member. The union discriminator is marked as a key by
annotating the discriminator itself with the @key annotation".

**Repo:** `crates/idl-lower/src/validate.rs::union_member_rules`,
`crates/idl-lower/src/lower.rs::KeyOnUnionDiscriminator`,
`crates/ast-layout/src/keyhash.rs::union_key_discriminator`

**Tests:** `crates/idl-lower/tests/validation.rs::only_a_unions_discriminator_may_be_a_key`,
`crates/idl-lower/tests/annotations.rs::a_key_on_a_union_discriminator_is_carried_by_default`,
`crates/idl-lower/tests/annotations.rs::a_key_on_a_union_discriminator_reaches_the_facet_when_carried`,
`crates/types/tests/table19_key_members.rs::one_keyed_union_discriminator_and_one_unkeyed_are_not_assignable`

**Status:** `done` — `@key` an einem Zweig wird abgewiesen. `@key` am Diskriminator wird im Default getragen (`KeyOnUnionDiscriminator::Carried` ist der Default); `Refused` ist die benannte Einstellung für Werkzeuge, die den gemessenen Herstellern folgen müssen. `zerodds-build` setzt `Refused` für den erzeugten Rust-Code, weil der Union-Codec dort keinen Key-Holder erzeugt (Sprachbindung, nicht dieses Paket).

### §7.2.2.4.4.4.8 Key Members: @key am Diskriminator wird getragen

**Spec:** §7.2.2.4.4.4.8, S. 62 — `union MyCommand switch (@key CommandKind)`.

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::a_key_on_a_union_discriminator_is_carried_by_default`,
`crates/xml-compose/tests/key_on_union_discriminator.rs::the_default_carries_the_declaration`,
`crates/xml-compose/tests/loader.rs::a_key_on_a_union_discriminator_is_refused_under_the_refusing_setting_and_says_why`

**Status:** done — `KeyOnUnionDiscriminator::default()` ist `Carried`: `@key` am Diskriminator geht in das Modell (`zerodds-idlc` und `zerodds-xml-compose`). `Refused` bleibt als benannte Einstellung für Hersteller, die die Deklaration abweisen.

### §7.2.2.4.4.4.8 Key Members: Array und Sequence, Länge vor den Elementen

**Spec:** §7.2.2.4.4.4.8, S. 62 — "If a member of type array or sequence is
marked as a key member of an aggregated type T, all the elements in the array
or sequence shall be considered part of the key of T. In the case of a
sequence, the length of the sequence is also considered as part of the key
ahead of the sequence elements."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_holder_plan` (Z. 41)

**Tests:** `crates/ast-layout/tests/key_maximum.rs::sequence_and_map_bounds_include_prefix_and_each_entry`,
`crates/ast-layout/tests/key_maximum.rs::nested_keys_and_arrays_do_not_double_count_or_add_trailing_padding`

**Status:** done — der KeyHolder serialisiert den ganzen Member
einschließlich der Längenangabe der Sequenz.

### §7.2.2.4.4.4.8 Key Members: Schlüssel eines geschachtelten Typs

**Spec:** §7.2.2.4.4.4.8, S. 62 — "In the event that the type K of a key member
of a given type T itself defines key members, only the key of K, and not any
other of its members, shall be considered part of the key of T. This
relationship is recursive".

**Repo:** `crates/ast-layout/src/keyhash.rs::member_key` (Z. 161);
`crates/ast-layout/src/keyhash.rs::holder_keeps_only_keys` (Z. 133)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::nested_keyed_members_contribute_their_own_keys`,
`crates/ast-layout/tests/vendor_gate.rs::a_key_member_of_a_keyless_type_contributes_all_of_its_members`,
`crates/ast-layout/tests/vendor_gate.rs::step_3_recursion_is_unlimited_except_in_rtis_xcdr1_holder`

**Status:** done

### §7.2.2.4.4.4.9 Default Member Value: explizit oder implizit

**Spec:** §7.2.2.4.4.4.9, S. 63 — "The default value for each member may be
defined explicitly or implicitly: An explicit definition associates a default
value with the member at the time where the aggregated type is defined. [...]
The implicit default value for a member is defined to match the default value
of the member type. See Table 9."

**Repo:** `crates/wire-codec/src/default.rs::member` (Z. 100);
`crates/lang-rules/src/default_rust.rs::member_default` (Z. 48)

**Tests:** `crates/lang-rules/tests/codec.rs::an_appendable_tail_takes_an_explicit_default_over_table_nine`,
`crates/lang-rules/tests/codec.rs::an_explicit_default_beats_table_nine_at_the_member_position`,
`crates/wire-probe/tests/evolution.rs::an_explicit_default_beats_table_nine`

**Status:** done

### §7.2.2.4.4.4.9 Default Member Value: kein expliziter Default bei @optional

**Spec:** §7.2.2.4.4.4.9, S. 63 — "Optional members are not allowed to associate
an explicit default value for the member."

**Repo:** `crates/wire-codec/src/default.rs::member_at_depth` (Z. 110, Prüfung
Z. 123); `crates/types/src/dynamic/builder.rs`

**Tests:** `crates/types/src/dynamic/builder.rs::tests::optional_with_a_default_is_refused`,
`crates/types/src/dynamic/builder.rs::tests::optional_with_a_default_is_refused_on_a_union_case`,
`crates/idl-lower/tests/validation.rs::an_optional_member_has_no_default`

**Status:** done — Der `DynamicTypeBuilder` und der reflektive Decoder weisen `@optional` mit `@default` ab; das IDL-Front-End tut es ebenfalls (`validate.rs::member_rules`).

### §7.2.2.4.4.4.9 Default Member Value: IDL-Front-End weist @optional @default ab

**Spec:** §7.2.2.4.4.4.9, S. 63 — kein expliziter Default bei optionalen
Membern.

**Repo:** `crates/idl-lower/src/validate.rs::member_rules`

**Tests:** `crates/idl-lower/tests/validation.rs::an_optional_member_has_no_default`

**Status:** done — `struct S { @optional @default(3) long a; };` wird vom Front-End abgewiesen.

---

## §7.2.2.4.5 Inheritance of Aggregated Types

### §7.2.2.4.5 Einfachvererbung

**Spec:** §7.2.2.4.5, S. 63 — "The Type System supports single inheritance of
Aggegated Types."

**Repo:** `crates/ast/src/node/type_.rs::Structure` (Z. 242, `base:
Option<Ref<TypeNode>>`); `crates/types/src/assignability.rs::flatten_inheritance`
(Z. 58)

**Tests:** `crates/types/src/assignability.rs::tests::flatten_inheritance_two_levels_concatenates_base_first`,
`crates/types/src/assignability.rs::tests::two_level_inheritance_assignability_chain`

**Status:** done — für Strukturen; Unionen siehe unten.

### §7.2.2.4.5 Struktur: Extensibility nicht angegeben oder gleich der Basis

**Spec:** §7.2.2.4.5, S. 63 — "The derived structure either has the
extensibility kind unspecified, or else, it has the same extensibility kind as
the base type."

**Repo:** `crates/idl-lower/src/validate.rs::inheritance_rules` (Z. 415, Prüfung
Z. 431–445); `crates/ast/src/query.rs::effective_extensibility` (Z. 290)

**Tests:** `crates/idl-lower/tests/keys_and_inheritance.rs::a_derived_structure_with_another_extensibility_than_its_base_is_refused`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_derived_structure_that_repeats_its_bases_extensibility_is_accepted`,
`crates/idl-lower/tests/completion.rs::a_derived_structure_that_said_nothing_has_its_bases_extensibility`

**Status:** done

### §7.2.2.4.5 Struktur: Namen und Member-IDs über die Kette eindeutig

**Spec:** §7.2.2.4.5, S. 63 — "The derived type does not have a member with the
same name or with the same memberId as the base type or any of the base
structure ancestor types. [...] The members of the hypothetical structure must
have unique names and member IDs."

**Repo:** `crates/idl-lower/src/validate.rs::inheritance_rules` (Z. 415,
Prüfungen Z. 473 und Z. 483); `crates/types/src/assignability.rs::flatten_inheritance`
(Z. 58)

**Tests:** `crates/idl-lower/tests/keys_and_inheritance.rs::a_member_name_of_the_base_chain_cannot_be_declared_again`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_member_id_of_the_base_chain_cannot_be_given_again`,
`crates/types/src/assignability.rs::tests::inheritance_conflict_same_id`,
`crates/types/src/assignability.rs::tests::inheritance_conflict_same_name`

**Status:** done

### §7.2.2.4.5 Struktur: keine Key-Felder im abgeleiteten Typ

**Spec:** §7.2.2.4.5, S. 63 — "The derived type does not define any key fields.
This ensures the key fields of the derived type are the same as those of the
base root type."

**Repo:** `crates/idl-lower/src/validate.rs::inheritance_rules` (Z. 415, Prüfung
Z. 499)

**Tests:** `crates/idl-lower/tests/keys_and_inheritance.rs::a_key_written_in_a_derived_structure_is_refused`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_keylist_that_gives_a_derived_structure_its_own_key_is_refused`

**Status:** done

### §7.2.2.4.5 Union-Vererbung: Bedingungen

**Spec:** §7.2.2.4.5, S. 63–64 — "A Union Type may inherit from another Union
Type as long as the following conditions are met: The derived Union either has
the extensibility kind unspecified, or else it has the same extensibility kind
as the base type. The derived Union has the same discriminator type as the
base Union. The derived Union discriminator does is not defined as key. The
base Union, or any of its ancestors have not explicitly defined a 'default'
member of the Union. Any given discriminator value identifies at most one
member of the derived Union or any of the base Union ancestor types."

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

**Status:** `done` — `check_union_derivation` prüft die fünf Bedingungen an den Objekten der Vorfahren und der abgeleiteten Union (je ein Test je Bedingung und für die Vorfahrenkette), `expand_union` bildet die hypothetische Union von Table 19. Keine Type Representation trägt eine Basis-Union: IDL 4.2 hat keine Syntax, die XSD gibt `baseType` nur `structDecl` und `annotationTypeDecl`, `MinimalUnionHeader` und `CompleteUnionHeader` sind leer (Befunddatei, Widersprüche). Eine Darstellung, die eine Basis ergänzt, ruft die Funktionen auf.

---

## §7.2.2.4.6 Related Key-Erased Type
### §7.2.2.4.6 KeyErased(T)

**Spec:** §7.2.2.4.6, S. 64–65 — "For each Aggergated Type 'T' the type system
defines a related Aggregated Type KeyErased(T) obtainied from 'T' by removing
the key designation from any member that had it [...]. In the case the type
'T' had no members designated key members then KeyErased(T) is the same type
'T'."

**Repo:** `crates/types/src/assignability.rs::key_erased`

**Tests:** `crates/types/tests/type_system_rules.rs::key_erased_removes_the_key_designation_and_nothing_else`,
`crates/types/tests/type_system_rules.rs::the_types_of_members_are_compared_with_their_keys_erased`

**Status:** `done` — `assignability::key_erased` ist öffentlich und bildet KeyErased(T) für Struktur und Union nach dem Beispiel der Klausel (`Aircraft`, `Coordinates`, `Command`); ein Typ ohne Schlüssel und eine Aufzählung bleiben unverändert. Table 19 vergleicht die Typen der Member als KeyErased (`member_types_assignable`).

---

## §7.2.2.4.7 Related Key-Holder Type
### §7.2.2.4.7 KeyHolder(T): Typ mit Schlüssel — nur die Key-Member

**Spec:** §7.2.2.4.7, S. 66 — "If 'T' has any members designated as key members
[...], then KeyHolder(T) removes any members of 'T' that are not designated as
key members."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_holder_plan` (Z. 41);
`crates/ast-layout/src/keyhash.rs::holder_keeps_only_keys` (Z. 133)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v8_keyed_sensor_plans_a_four_byte_zero_padded_key_holder`,
`crates/ast-layout/tests/wire_vectors.rs::key_members_come_back_in_member_id_order_not_declaration_order`

**Status:** done

### §7.2.2.4.7 KeyHolder(T): Struktur ohne Schlüssel — alle Member als Key

**Spec:** §7.2.2.4.7, S. 66 — "If 'T' is a structure with no key members, then
KeyHolder(T) adds a key designator to each member."

**Repo:** `crates/ast-layout/src/keyhash.rs::holder_keeps_only_keys` (Z. 133);
`crates/ast-layout/src/keyhash.rs::member_key` (Z. 161)

**Tests:** `crates/ast-layout/tests/vendor_gate.rs::a_key_member_of_a_keyless_type_contributes_all_of_its_members`,
`crates/ast-layout/tests/wire_vectors.rs::a_type_with_no_key_member_has_no_key_members_and_a_whole_type_holder`

**Status:** done — für einen geschachtelten Typ ohne Schlüssel. Ein
Topic-Typ ohne Schlüssel hat keinen KeyHash; `key_members` meldet für ihn
eine leere Liste (`a_keyless_topic_type_still_reports_no_key_members`).

### §7.2.2.4.7 KeyHolder(T): Union ohne Diskriminator mit @key ist T

**Spec:** §7.2.2.4.7, S. 66 — "If T' is a union and the discriminator is not
marked as key, then KeyHolder(T) is the same type T."

**Repo:** `crates/ast-layout/src/keyhash.rs::union_key_discriminator`,
`crates/ast-layout/src/key_maximum.rs`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::a_union_as_the_type_of_a_key_member_is_in_the_holder_whole`,
`crates/ast-layout/tests/key_maximum.rs::union_takes_largest_branch_not_sum`

**Status:** `done` — eine Union, deren Diskriminator nicht Schlüssel ist, trägt ganz in den Holder (Diskriminator und gewähltes Member, PLAIN_CDR2 big endian); `key_maximum` rechnet mit Diskriminator und größtem Zweig. Eine Union mit `@key` am Diskriminator ist im Holder der Diskriminator allein.

### §7.2.2.4.7 KeyHolder(T): Union als Typ eines Key-Members, kein Test

**Spec:** §7.2.2.4.7, S. 66 — KeyHolder einer Union ohne Schlüssel.

**Repo:** `crates/ast-layout/src/keyhash.rs::collect_key_members`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::a_union_as_the_type_of_a_key_member_is_in_the_holder_whole`

**Status:** `done` — der Test prüft die Oktette des KeyHash für zwei Zweige der Union und dass ein Nicht-Key-Member den Wert nicht ändert.

---

## §7.2.2.4.8 Verbatim Text
### §7.2.2.4.8 VerbatimText: language, placement, text

**Spec:** §7.2.2.4.8, S. 67–68 — "A VerbatimText object defines three
properties; each is a string: language [...] placement [...] text".

**Repo:** `crates/ast/src/facet/xtypes.rs::VerbatimDirective` (Z. 218);
`crates/idl-lower/src/annotation.rs::verbatim_annotation` (Z. 934)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_verbatim_directive_keeps_its_language_placement_and_text`,
`crates/idl-lower/tests/annotations.rs::repeated_verbatim_directives_accumulate_in_declaration_order`

**Status:** done

### §7.2.2.4.8.1 Property: Language — Sprachen und „*“, ohne Groß-/Kleinschreibung

**Spec:** §7.2.2.4.8.1, S. 68 — "When a Type Representation compiler generates
code for the programming language named (case-insensitively) by this property,
it shall copy the string contained in the text property into its output. The
string 'c' [...] 'c++' [...] 'java' [...] The string '*' (an asterisk) shall
indicate that text applies to all programming languages."

**Repo:** `crates/lang-rules/src/verbatim.rs::language_matches` (Z. 105);
`crates/lang-rules/src/verbatim.rs::aliases_of` (Z. 324)

**Tests:** `crates/lang-rules/src/verbatim.rs::tests::the_wildcard_concerns_every_backend_and_names_compare_caselessly`,
`crates/lang-rules/src/verbatim.rs::tests::a_directive_for_another_language_is_left_to_that_backend`

**Status:** done

### §7.2.2.4.8.2 Property: Placement — sechs Werte, ohne Groß-/Kleinschreibung

**Spec:** §7.2.2.4.8.2, S. 68 — "It shall be interpreted in a case-insensitive
manner. All Type Representation compilers shall recognize the following
placement strings [...] begin-declaration-file, before-declaration,
begin-declaration, end-declaration, after-declaration, end-declaration-file".

**Repo:** `crates/idl-lower/src/annotation.rs::verbatim_placement` (Z. 1103)

**Tests:** `crates/idl-lower/tests/annotations.rs::an_unknown_verbatim_placement_is_reported`,
`crates/idl-lower/tests/annotations.rs::a_verbatim_placement_is_read_without_regard_to_case_and_in_the_xtypes_spelling`,
`crates/lang-rules/src/verbatim.rs::tests::every_placement_lands_in_its_own_slot`

**Status:** done — Angenommen werden die Namen von IDL 4.2 §8.3.5.1 und die sechs der Spec (`before-declaration`, `begin-declaration`, `end-declaration`, `after-declaration`, `begin-file`, `end-file`) sowie `begin-declaration-file` und `end-declaration-file`, ohne Unterscheidung von Groß- und Kleinschreibung.

### §7.2.2.4.8.2 Placement: begin-declaration-file, end-declaration-file, Schreibweise

**Spec:** §7.2.2.4.8.2, S. 68 — Placement-Werte ohne Groß-/Kleinschreibung,
darunter `begin-declaration-file` und `end-declaration-file`.

**Repo:** `crates/idl-lower/src/annotation.rs::verbatim_placement`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_verbatim_placement_is_read_without_regard_to_case_and_in_the_xtypes_spelling`

**Status:** done — `verbatim_placement` kennt `begin-declaration-file` und `end-declaration-file` und vergleicht ohne Unterscheidung von Groß- und Kleinschreibung.

### §7.2.2.4.8.2 Placement: Verhalten je Wert

**Spec:** §7.2.2.4.8.2, S. 68 — Anfang der Datei vor allen Typdeklarationen,
unmittelbar vor der Deklaration, im Rumpf vor allen Membern, im Rumpf nach
allen Membern, unmittelbar nach der Deklaration, Ende der Datei.

**Repo:** `crates/lang-rules/src/verbatim.rs::declaration_slots` (Z. 129);
`crates/lang-rules/src/verbatim.rs::file_slots` (Z. 167);
`crates/lang-rules/src/verbatim.rs::weave` (Z. 245)

**Tests:** `crates/lang-rules/src/verbatim.rs::tests::every_placement_lands_in_its_own_slot`,
`crates/lang-rules/tests/verbatim_end_to_end.rs::cpp_places_all_six_slots_in_spec_order`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`

**Status:** done — ein Generator, dessen Baum an `begin`/`end` keinen Text
aufnehmen kann (C-`struct`, `enum`), schreibt ihn neben die Deklaration;
ein Verbatim an einem Member wird in den `begin`-Platz des Aggregats gehoben.

### §7.2.2.4.8.3 Property: Text — unverändert kopiert

**Spec:** §7.2.2.4.8.3, S. 69 — "The Type Representation compiler shall copy the
string contained in this property into its output as described above."

**Repo:** `crates/lang-rules/src/verbatim.rs::weave` (Z. 245)

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::multi_line_text_arrives_line_by_line`,
`crates/lang-rules/tests/verbatim_every_column.rs::the_wildcard_reaches_every_column`

**Status:** done

---

## §7.2.2.4.9 External Data

### §7.2.2.4.9 External: Member auf unvollständigen Typ

**Spec:** §7.2.2.4.9, S. 69 — "An external member of an aggregated type shall be
permitted to refer to a type whose definition is incomplete (i.e., is
identified only by a forward declaration) at the time of the member's
declaration."

**Repo:** `crates/types/src/resolve.rs::transitive_dependencies` (Z. 246)

**Tests:** `crates/types/src/resolve.rs::tests::external_forward_decl_roundtrip`

**Status:** done

### §7.2.2.4.9 External: Member auf den enthaltenden Typ

**Spec:** §7.2.2.4.9, S. 69 — "An external member of an aggregated type shall be
permitted to refer to the member's containing type."

**Repo:** `crates/lang-rules/tests/external_is_an_indirection.rs`;
`crates/ast-layout/src/plan.rs`

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value`,
`crates/types/src/resolve.rs::tests::external_diamond_resolves_through_both_paths`

**Status:** done

### §7.2.2.4.9 External: jeder Member außer dem Diskriminator

**Spec:** §7.2.2.4.9, S. 69 — "Each member of an aggregated type—with the
exception of the discriminator of a union type—may be optionally marked as
external."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm
`external` Z. 336); `crates/idl-lower/src/validate.rs::external_is_a_member_annotation`
(Z. 134)

**Tests:** `crates/idl-lower/tests/validation.rs::external_on_a_member_and_on_a_branch_is_accepted`,
`crates/idl-lower/tests/validation.rs::external_on_a_type_declaration_is_refused_by_name`

**Status:** done

### §7.2.2.4.9 External: Elemente einer Collection

**Spec:** §7.2.2.4.9, S. 69 — "Likewise, the elements of a collection type may
be optionally marked as external."

**Repo:** `crates/ast/src/query.rs::element_is_external`,
`crates/ast/src/query.rs::key_is_external`,
`crates/idl-parse/src/decl.rs::parse_declarator_list`,
`crates/idl-lower/src/lower.rs::declarator_type`

**Tests:** `crates/idl-lower/tests/annotations.rs::external_on_an_element_a_map_key_or_a_map_value_is_carried`,
`crates/idl-lower/tests/annotations.rs::external_after_an_array_declarators_name_is_the_elements`,
`crates/types/tests/collection_element_flags.rs::the_element_of_a_collection_carries_its_external_and_its_try_construct`

**Status:** done — `@external` an einem Element, einem Map-Schlüssel oder einem Map-Wert (`sequence<@external Foo, 42>`) und nach dem Namen eines Array-Deklarators (`Foo array_of_foo @external [42];`) gilt für die Elemente: das Modell hat `element_is_external` und `key_is_external`, der Element-Identifier trägt `IS_EXTERNAL` und `TRY_CONSTRUCT`.

### §7.2.2.4.9 External: keine Objektgraphen

**Spec:** §7.2.2.4.9, S. 69 — "Note that this attribute does not provide a means
for modeling object graphs."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis.

---

## §7.2.2.5 Nested Types

### §7.2.2.5 Nested Types

**Spec:** §7.2.2.5, S. 69 — "Types that are not intended to describe topic data
are referred to as nested types." Ein IDL-Compiler "need not generate typed
DataWriter, DataReader, and TypeSupport classes for types that are not
intended to type topics."

**Repo:** `crates/ast/src/query.rs::effective_nested` (Z. 484);
`crates/types/src/from_ast.rs::aggregate_flags` (Z. 1614, `IS_NESTED`)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_nested_annotation_reaches_the_type`,
`crates/idl-lower/tests/annotations.rs::default_nested_on_a_module_reaches_the_types_in_it`

**Status:** done — die Annotationen `@nested` und `@default_nested` sind unter
§7.3.1.2.1.7 geführt.

---

## §7.2.2.6 Annotations

### §7.2.2.6 Annotation an Typ, Element, Member, Literal, related_type

**Spec:** §7.2.2.6, S. 69 — "An annotation describes a piece of metadata
attached to a type or an element/member/literal of an
aggregated/collection/enumerated type. Annotations can also be attached to the
related_type of an alias type."

**Repo:** `crates/ast/src/facet/applied.rs`;
`crates/idl-lower/src/annotation.rs::apply_user_annotation` (Z. 417)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::an_annotation_applied_to_the_type_itself_reaches_the_object`,
`crates/types/tests/type_object_custom_annotations.rs::every_position_with_a_complete_member_detail_is_read`,
`crates/types/tests/type_object_custom_annotations.rs::an_annotation_on_a_typedef_reaches_the_alias_object`,
`crates/types/tests/annotations_on_uses.rs::an_annotation_on_an_element_is_in_the_complete_collection_object`

**Status:** done — die Annotation erreicht Typ, Member, Literal, ein Collection-Element und die `typedef`-Deklaration (`CompleteAliasType`, Kopf `ann_custom`).

### §7.2.2.6 Annotation an einem Collection-Element wird geführt

**Spec:** §7.2.2.6, S. 69 — Annotationen an einem Element eines
Collection-Typs.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_use_site`,
`crates/types/src/from_ast.rs::collection_object`,
`crates/types/src/type_object/to_minimal.rs::plain_collection`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_declared_annotation_on_an_element_a_key_or_an_array_is_kept`,
`crates/types/tests/annotations_on_uses.rs::an_annotation_on_an_element_is_in_the_complete_collection_object`,
`crates/types/tests/annotations_on_uses.rs::the_minimal_form_is_the_one_the_unannotated_declaration_has`,
`crates/types/tests/annotations_on_uses.rs::the_complete_object_projects_onto_the_same_minimal_object`,
`crates/types/tests/annotations_on_uses.rs::the_applications_come_back_into_the_model`,
`crates/types/tests/annotations_on_uses.rs::an_undeclared_annotation_leaves_the_collection_plain`

**Status:** done — Eine benutzerdeklarierte Annotation an einem Collection-Element, einem Map-Schlüssel oder nach einem Array-Namen wird im Modell an der Collection geführt (`XTypesFacet::element_annotations`, `key_annotations`) und in das `ann_custom` von `CompleteCollectionElement` geschrieben. Die Collection ist dann ein Typ mit `TypeObject` (§7.3.4.1): `CompleteSequenceType`, `CompleteArrayType` oder `CompleteMapType`, im Hash-Identifier der COMPLETE-Form; die MINIMAL-Form bleibt die einfache Collection.

### §7.2.2.6 AnnotationType: Parameter mit Default

**Spec:** §7.2.2.6, S. 69–70 — "An AnnotationType defines the structure of the
metadata as a set of AnnotationParameters that can be assigned values when the
annotation is applied. [...] The definition of an AnnotationType can specify
the default value of each AnnotationParameter."

**Repo:** `crates/types/src/from_ast.rs::annotation_type_object` (Z. 537);
`crates/idl-lower/src/annotation.rs::check_annotation_application` (Z. 468)

**Tests:** `crates/types/tests/type_object_annotations.rs::an_annotation_declaration_has_a_type_object`,
`crates/types/tests/type_object_custom_annotations.rs::the_parameter_value_is_the_value_that_was_applied`,
`crates/types/tests/type_object_custom_annotations.rs::an_unapplied_member_writes_ann_custom_absent`

**Status:** done

### §7.2.2.6 AnnotationParameter: zulässige Typen

**Spec:** §7.2.2.6, S. 70 — "The types permitted for an AnnotationParameter
are: Primitive types, String types of Char8 or Char16 elements, Enumerated
types".

**Repo:** `crates/types/src/from_ast.rs::annotation_type_object` (Z. 537);
`crates/types/src/type_object/common.rs::AnnotationParameterValue` (Z. 488)

**Tests:** `crates/types/tests/annotation_parameter_value_union.rs::an_annotation_with_long_double_wchar_and_wstring_parameters_builds`,
`crates/types/tests/type_object_custom_annotations.rs::an_enumerated_annotation_parameter_is_in_the_closure`,
`crates/idl-lower/tests/validation.rs::an_annotation_member_is_a_primitive_a_string_or_an_enum`

**Status:** done — Parameter vom Typ Boolean, Byte, Int8 bis UInt64, Float32, Float64, Float128, Char8, Char16, Enumeration, String8 und String16 werden dargestellt; `AnnotationParameterValue` hat alle Fälle der Union aus Annex B und den erweiterten Fall.

### §7.2.2.6 AnnotationParameter: long double, wchar, wstring werden dargestellt

**Spec:** §7.2.2.6, S. 70 — primitive Typen und Zeichenketten aus Char16.

**Repo:** `crates/types/src/type_object/common.rs::AnnotationParameterValue`

**Tests:** `crates/types/tests/annotation_parameter_value_union.rs::a_wchar_value_is_tk_char16_and_two_octets`,
`crates/types/tests/annotation_parameter_value_union.rs::a_long_double_value_is_tk_float128_and_sixteen_octets`,
`crates/types/tests/annotation_parameter_value_union.rs::a_wstring_value_is_tk_string16_with_an_octet_length`,
`crates/types/tests/annotation_parameter_value_union.rs::an_unknown_discriminator_is_the_extended_value`

**Status:** done — `AnnotationParameterValue` hat `Float128Bits`, `Char16`, `String16` und den erweiterten Fall (mit DHEADER).

### §7.2.2.6 AnnotationParameter: unzulässiger Typ wird abgewiesen

**Spec:** §7.2.2.6, S. 70 — nur primitive Typen, Zeichenketten und
Enumerated Types.

**Repo:** `crates/idl-lower/src/validate.rs::annotation_member_types`

**Tests:** `crates/idl-lower/tests/validation.rs::an_annotation_member_is_a_primitive_a_string_or_an_enum`

**Status:** done — `validate.rs::annotation_member_types` weist einen Annotationsmember ab, dessen Typ weder Primitive, String noch Enumeration ist.

---

## §7.2.2.7 Try Construct behavior

### §7.2.2.7 Hintergrund und Table 10

**Spec:** §7.2.2.7, S. 70–72 — Motivation mit `STRUCT1024`/`STRUCT128` und
`SEQ1024`/`SEQ128` (Table 10).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel; die Regeln folgen in Table 11
und §7.2.4.

### §7.2.2.7 Konstruktion rekursiv

**Spec:** §7.2.2.7, S. 72 — "Object construction for collection and aggregated
types is done recursively. [...] failure to construct a nested element/member
can prevent the construction of the collection/aggregated type."

**Repo:** `crates/wire-codec/src/decode.rs`

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/wire-probe/tests/try_construct.rs::an_element_annotation_trims_the_element_and_not_the_collection`

**Status:** done

### §7.2.2.7 TryConstruct an Array- und Sequence-Elementen

**Spec:** §7.2.2.7, S. 72 — "Array and Sequence collection types may explicitly
declare that their element has one of three kinds of TryConstruct behavior".

**Repo:** `crates/ast/src/facet/xtypes.rs::XTypesFacet` (Z. 242, Feld
`element_try_construct`); `crates/ast/src/query.rs::element_try_construct`
(Z. 413)

**Tests:** `crates/idl-lower/tests/annotations.rs::try_construct_reaches_every_position_the_clause_names`,
`crates/wire-probe/tests/try_construct.rs::an_element_annotation_trims_the_element_and_not_the_collection`

**Status:** done

### §7.2.2.7 TryConstruct an Map-Schlüssel und Map-Wert

**Spec:** §7.2.2.7, S. 72 — "Map collection types may explicitly declare that
their 'key' and or 'value' element has one of three kinds of TryConstruct
behavior".

**Repo:** `crates/ast/src/query.rs::key_try_construct` (Z. 449);
`crates/idl-lower/src/annotation.rs::apply_use_site` (Z. 775)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_map_key_may_only_be_discard`,
`crates/wire-probe/tests/try_construct.rs::a_map_value_trims_and_the_entry_survives`

**Status:** done — der Map-Wert nimmt alle drei Arten; der Map-Schlüssel nur
`DISCARD`, weil Table 17 für den Schlüssel keine andere vorsieht. §7.2.2.7
nennt drei Arten auch für den Schlüssel (Widerspruch der Spec, in der
Befunddatei geführt).

### §7.2.2.7 TryConstruct an Struktur- und Union-Membern, Diskriminator

**Spec:** §7.2.2.7, S. 72 — "Structure and Union types may explicitly declare
member has one of three kinds of TryConstruct behavior [...]. In the case of
Unions this extends to the discriminator member."

**Repo:** `crates/idl-lower/src/annotation.rs::try_construct_annotation` (Z. 725);
`crates/ast/src/query.rs::discriminator_try_construct` (Z. 508)

**Tests:** `crates/idl-lower/tests/annotations.rs::try_construct_reaches_every_position_the_clause_names`,
`crates/wire-probe/tests/try_construct.rs::a_defaulted_discriminator_defaults_the_member_it_now_selects`

**Status:** done

### §7.2.2.7 Default ist DISCARD

**Spec:** §7.2.2.7, S. 72 — "The default behavior unless otherwise specified
using the TryConstruct annotation is DISCARD."

**Repo:** `crates/ast/src/facet/xtypes.rs::ABSENT` (Z. 88)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_bare_try_construct_is_use_default_and_an_absent_one_is_discard`,
`crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/types/src/builder.rs::tests::member_default_try_construct_is_discard_bits`

**Status:** done

### §7.2.2.7 Table 11 — DISCARD

**Spec:** §7.2.2.7, Table 11, S. 72 — "Failure to construct an element or member
propagates to the collection or aggregated type that contains it."

**Repo:** `crates/wire/src/try_construct.rs::fit_string` (Z. 89);
`crates/wire/src/try_construct.rs::fit_sequence` (Z. 131)

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/wire-probe/tests/try_construct.rs::a_discard_and_a_malformed_stream_are_not_the_same_error`,
`crates/wire-conformance/tests/map_bytes.rs::an_over_long_map_is_discarded_where_the_position_says_discard`

**Status:** done

### §7.2.2.7 Table 11 — USE_DEFAULT

**Spec:** §7.2.2.7, Table 11, S. 72 — "If an element or member cannot be
constructed, the element/member shall be set to its default value (according
to its type as described in Table 9) and does not cause the
collection/aggregated object to fail its construction."

**Repo:** `crates/wire/src/try_construct.rs::Narrowed` (Z. 61)

**Tests:** `crates/wire-probe/tests/try_construct.rs::use_default_substitutes_table_nines_value_and_keeps_the_sample`,
`crates/wire-conformance/tests/map_bytes.rs::an_over_long_map_takes_the_default_where_the_position_says_use_default`

**Status:** done

### §7.2.2.7 Table 11 — TRIM: nur string, wstring, sequence, map

**Spec:** §7.2.2.7, Table 11, S. 73 — "This option only applies to elements or
members of type string, wide string, sequence, or map. The behavior when
applied to other element/member types is unspecified and may be treated as an
error."

**Repo:** `crates/idl-lower/src/validate.rs::refuse_trim` (Z. 594);
`crates/ast/src/facet/xtypes.rs::trim_is_defined_for` (Z. 131)

**Tests:** `crates/idl-lower/tests/annotations.rs::trim_on_a_type_that_cannot_be_trimmed_is_refused`

**Status:** done — als Fehler behandelt.

### §7.2.2.7 Table 11 — TRIM: Kürzung auf die Länge des Empfängers, Reihenfolge erhalten

**Spec:** §7.2.2.7, Table 11, S. 73 — "In this situation the element or member
is constructed trimming the received object to the length that can be
accommodated by the receiving member type. The order of the characters in the
string or elements in the sequence or map is preserved."

**Repo:** `crates/wire/src/try_construct.rs::fit_string` (Z. 89);
`crates/wire/src/try_construct.rs::fit_sequence` (Z. 131);
`crates/wire/src/try_construct.rs::fit_map` (Z. 169);
`crates/wire/src/try_construct.rs::fit_wstring` (Z. 209)

**Tests:** `crates/wire-probe/tests/try_construct.rs::trim_cuts_the_string_to_the_bound_and_keeps_the_sample`,
`crates/wire-probe/tests/try_construct.rs::trim_cuts_a_sequence_to_its_bound_keeping_the_first_elements`,
`crates/wire-probe/tests/try_construct.rs::a_trim_stops_on_a_character_boundary`,
`crates/wire-conformance/tests/map_bytes.rs::an_over_long_map_is_trimmed_where_the_position_says_trim`

**Status:** done — eine Zeichenkette wird nicht innerhalb eines Zeichens
geschnitten; bei einer Map bleiben die Einträge mit den kleinsten Schlüsseln.

---

## §7.2.3 Type Extensibility and Mutability

### §7.2.3 Drei Extensibility-Arten: FINAL, APPENDABLE, MUTABLE

**Spec:** §7.2.3, S. 73 — "the type system introduces the concept of type
extensibility and defines three extensibility kinds: final, appendable and
mutable." FINAL: "the range of its possible data values is strictly defined";
APPENDABLE: "two types, where one contains all of the elements/members of the
other plus additional elements/members appended to the end, may remain
assignable"; MUTABLE: "two types may differ from one another in the
additional, removal, and/or transposition of elements/members while remaining
assignable."

**Repo:** `crates/ast/src/facet/xtypes.rs::Extensibility` (Z. 16);
`crates/types/src/type_object/flags.rs::IS_FINAL` (Z. 14)

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/types/src/assignability.rs::tests::final_struct_with_extra_writer_field_is_not_assignable`,
`crates/types/src/assignability.rs::tests::appendable_struct_with_extra_writer_field_is_assignable`,
`crates/types/src/assignability.rs::tests::mutable_struct_member_id_matching`

**Status:** done

### §7.2.3 Extensibility beeinflusst die Codierung

**Spec:** §7.2.3, S. 74 — "This attribute may be used by the Data
Representations to modify the encoding of the type in order to support its
extensibility."

**Repo:** `crates/ast-layout/src/xcdr.rs::extensibility_of` (Z. 1689)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_gives_a_dheader_to_anything_that_is_not_final`,
`crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`

**Status:** done — die Regeln stehen unter §7.4.

### §7.2.3 Table 12 — Aggregated Types

**Spec:** §7.2.3, Table 12, S. 74 — "Aggregated types may be final, appendable,
or mutable on a type-by-type basis. However, the extensibility kind of a
structure type with a base type must match that of the base type. It shall not
be permitted for a subtype to change the extensibility kind of its base type.
Any members marked as keys must be present in all variants of the type."

**Repo:** `crates/idl-lower/src/validate.rs::inheritance_rules` (Z. 415);
`crates/types/src/assignability.rs::must_not_be_missing` (Z. 1054)

**Tests:** `crates/idl-lower/tests/keys_and_inheritance.rs::a_derived_structure_with_another_extensibility_than_its_base_is_refused`,
`crates/types/tests/table19_key_members.rs::a_writer_enum_key_literal_the_reader_lacks_is_not_assignable`,
`crates/types/src/assignability.rs::tests::a_mutable_writer_member_the_reader_lacks_and_must_understand_is_no`

**Status:** done — ein Key-Member, der auf einer Seite fehlt, macht zwei
Varianten nicht zuweisbar.

### §7.2.3 Table 12 — Collection Types: keine Wirkung

**Spec:** §7.2.3, Table 12, S. 74 — "ARRAY_TYPE, SEQUENCE_TYPE, MAP_TYPE: For
these types the extensibility kind has no effect."

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (Z. 89)

**Tests:** `crates/ast-layout/tests/layout.rs::an_array_of_aggregates_reuses_its_dheader_as_the_nextint`,
`crates/ast-layout/tests/layout.rs::a_map_frames_as_lc5_only_when_it_carries_a_dheader`

**Status:** done — ob eine Collection einen DHEADER trägt, hängt am
Elementtyp und an der Encoding-Version, nicht an einer Extensibility der
Collection (DDSXTY14-25, `docs/omg-xtypes-issues.md` §A5).

### §7.2.3 Table 12 — Enumerated Types: final oder appendable

**Spec:** §7.2.3, Table 12, S. 74 — "ENUMERATION_TYPE, BITMASK_TYPE: These types
may be final or appendable on a type-by-type basis."

**Repo:** `crates/idl-lower/src/validate.rs::extensibility_is_final_or_appendable`,
`crates/types/src/assignability.rs::check_enumerations`

**Tests:** `crates/idl-lower/tests/validation.rs::an_enum_or_bitmask_is_never_mutable`,
`crates/types/src/assignability.rs::tests::a_final_enum_has_the_same_literals_on_both_sides`,
`crates/types/src/assignability.rs::tests::an_enum_that_grew_a_literal_is_still_assignable`

**Status:** `done` — `@mutable` an einer Aufzählung oder Bitmaske wird abgewiesen (`@final` und `@appendable` werden angenommen).

### §7.2.3 Table 12 — String Types: keine Wirkung

**Spec:** §7.2.3, Table 12, S. 74 — "STRING8_TYPE, STRING16_TYPE: For these types
the extensibility kind has no effect."

**Repo:** `crates/ast-layout/src/xcdr.rs::extensibility_of` (Z. 1689)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v6_final_sequence_of_string_is_unbounded_and_frames_compactly_as_lc5`

**Status:** done — eine Zeichenkette hat keinen DHEADER.

### §7.2.3 Table 12 — ALIAS_TYPE: Extensibility des Basistyps

**Spec:** §7.2.3, Table 12, S. 74 — "Since aliases are semantically equivalent
to their base types, the extensibility kind of an alias is always equal to that
of its base type."

**Repo:** `crates/types/src/resolve.rs::resolve_alias_chain`;
`crates/ast-layout/src/plan.rs`

**Tests:** `crates/types/src/resolve.rs::tests::resolve_chained_aliases_two_deep`

**Status:** done — Layout und Zuweisbarkeit lösen den Alias auf.

### §7.2.3 Table 12 — Primitive Typen: keine Wirkung

**Spec:** §7.2.3, Table 12, S. 74 — "Primitive types: For these types the
extensibility kind has no effect."

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (Z. 89)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v5_final_sequence_of_int32_is_unbounded`

**Status:** done

---

## §7.2.4 Type Compatibility

### §7.2.4 Relation is-assignable-from zwischen jedem Typpaar

**Spec:** §7.2.4, S. 74–75 — "the type system defines the 'is-assignable-from'
directed binary relationship between every pair of types in the Type System.
[...] T1 is-assignable-from T2".

**Repo:** `crates/types/src/assignability.rs::is_assignable` (Z. 274);
`crates/types/src/assignability.rs::check_direct` (Z. 321)

**Tests:** `crates/types/tests/evolution_assignability.rs::a_type_is_assignable_to_itself`,
`crates/types/tests/evolution_assignability.rs::every_evolution_case_gets_a_verdict_and_the_verdicts_are_recorded`

**Status:** done — `is_assignable(w, r, …)` nimmt den Typ des Writers (T2) zuerst
und den Typ des Readers (T1) als zweites Argument.

### §7.2.4.1 Konstruktion: Teilmenge der T2-Objekte, Rest erkennbar

**Spec:** §7.2.4.1, S. 75 — "a meaningful subset of T2 objects can construct T1
objects without misinterpretation and [...] the remaining objects of T2—which
cannot construct T1 objects—can be detected as such [...]. these per-object
'can-construct' rules are designed such that their enforcement does not
require any inspection of a data producer's type definition."

**Repo:** `crates/wire-codec/src/decode.rs`; `crates/wire/src/try_construct.rs`

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/wire-probe/tests/try_construct.rs::an_enumerator_the_reader_does_not_declare_is_a_construction_failure`

**Status:** done — der Decoder liest mit dem Typ des Readers; der Typ des
Writers wird beim Lesen einer Probe nicht herangezogen.

### §7.2.4.1 Konstruktion: Default ist Verwerfen, TryConstruct am Member

**Spec:** §7.2.4.1, S. 75 — "In the case T1 is-assignable-from T2 but an object
O2 of type T2 is encountered that cannot construct any object of type T1, the
default behavior is to discard the O2 object to avoid misinterpretation. This
behavior can be altered when the object O2 is a member of an Aggregated type
[...] the behavior is determined by the TryConstruct behavior specified for the
member."

**Repo:** `crates/wire/src/try_construct.rs::TryConstruct` (Z. 42)

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/wire-probe/tests/try_construct.rs::use_default_substitutes_table_nines_value_and_keeps_the_sample`

**Status:** done

### §7.2.4.1 Table 13 — Beispiel Sequence of 10 / Sequence of 5

**Spec:** §7.2.4.1, Table 13, S. 75 — Sequenz von 10 Integern und Sequenz von 5
Integern sind gegenseitig zuweisbar; nur Objekte mit höchstens 5 Elementen
konstruieren T2.

**Repo:** `crates/types/src/assignability.rs::check_direct` (Z. 321, Arm
Sequence)

**Tests:** `crates/types/src/assignability.rs::tests::sequence_bounds_ignored_when_policy_allows`,
`crates/lang-rules/tests/codec.rs::a_bounded_sequence_checks_its_bound_and_an_unbounded_one_has_none`

**Status:** done — mit dem Default `ignore_sequence_bounds = true` sind beide
Richtungen zuweisbar; die Länge prüft der Decoder je Probe.

### §7.2.4.2 Delimited: primitive und Enumerated Types

**Spec:** §7.2.4.2, S. 76 — "Primitive and Enumerated types (Enumeration and
Bitmask) are delimited types as their serialized size is fixed."

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (Z. 89)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v5_final_sequence_of_int32_is_unbounded`

**Status:** done — als Eigenschaft der Codierung. Der Begriff „delimited“
wird im Baum nur für die Wahl des DHEADER verwendet; für die starke
Zuweisbarkeit siehe §7.2.4.3.

### §7.2.4.2 Delimited: Zeichenketten

**Spec:** §7.2.4.2, S. 76 — "Strings and wide strings are delimited types
because the serialization starts with a size from which it is possible to
derive the overall serialized length of the string."

**Repo:** `crates/wire/src/writer.rs::write_string` (Z. 1881);
`crates/wire/src/writer.rs::write_wstring` (Z. 1980)

**Tests:** `crates/wire/tests/wire.rs::a_string_length_counts_its_terminator`

**Status:** done

### §7.2.4.2 Delimited: Collections mit delimited Element- und Schlüsseltyp

**Spec:** §7.2.4.2, S. 76 — "Collection types (arrays, sequences, maps) are
delimited if the collection element type is delimited. In the case of a map
collection the key type must also be delimited. Otherwise the collection is
not delimited."

**Repo:** `crates/types/src/assignability.rs::is_delimited`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::a_collection_of_delimited_elements_is_delimited`,
`crates/types/tests/strong_assignability_and_table19.rs::appendable_elements_are_delimited_under_xcdr2_only`

**Status:** done — `assignability.rs::is_delimited` beantwortet §7.2.4.2: ein Typ ist delimited, wenn er MUTABLE ist, APPENDABLE unter XCDR2 (`AssignabilityConfig::appendable_is_delimited`) oder eine Collection mit delimited Element- und Schlüsseltyp.

### §7.2.4.2 Delimited: FINAL-Aggregate nicht delimited

**Spec:** §7.2.4.2, S. 76 — "Other than the types mentioned above all other
types with extensibility kind FINAL are not delimited."

**Repo:** `crates/ast-layout/src/xcdr.rs::extensibility_of` (Z. 1689)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v1_empty_final_struct_plans_zero_bytes_and_no_dheader`

**Status:** done — als Eigenschaft der Codierung: ein FINAL-Aggregat trägt
keinen DHEADER.

### §7.2.4.2 Delimited: APPENDABLE nur unter Encoding-Version 2

**Spec:** §7.2.4.2, S. 76 — "Types with extensibility kind APPENDABLE are
delimited if serialized with encoding version 2 (DELIMITED_CDR). [...] They are
not delimited if serialized with encoding version 1."

**Repo:** `crates/ast-layout/src/xcdr.rs::extensibility_of` (Z. 1689)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v9_appendable_plans_a_dheader`,
`crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`

**Status:** done

### §7.2.4.2 Delimited: MUTABLE unter beiden Encoding-Versionen

**Spec:** §7.2.4.2, S. 76 — "Mutable types are also delimited with both encoding
version 1 and encoding version 2. [...] PL_CDR is a list of length-encoded
elements ended by a sentinel [...] PL_CDR2 starts with a delimiter header".

**Repo:** `crates/ast-layout/src/xcdr.rs` (Parameterliste und DHEADER)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v12_empty_mutable_plans_a_dheader_and_no_members`,
`crates/ast-layout/tests/xcdr1_gate.rs::plain_cdr1_plans_no_sentinel`

**Status:** done

### §7.2.4.3 Strong Assignability

**Spec:** §7.2.4.3, S. 76 — "If types T1 and T2 are equivalent using the MINIMAL
relation (see Clause 7.3.4.7), or alternatively if T1 is assignable-from T2 and
T2 is a delimited type, then T1 is said to be 'strongly' assignable from T2."

**Repo:** `crates/types/src/assignability.rs::strongly_assignable`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::a_sequence_of_final_structs_needs_equivalent_elements`,
`crates/types/tests/strong_assignability_and_table19.rs::array_elements_and_map_keys_and_values_are_strongly_assignable`,
`crates/types/tests/strong_assignability_and_table19.rs::members_of_appendable_and_final_types_are_strongly_assignable`

**Status:** done — `assignability.rs::strongly_assignable` prüft die starke Zuweisbarkeit (Tabelle 17: Elemente von Array, Sequence und Map; Tabelle 19: Member am gleichen Index von APPENDABLE- und FINAL-Struct): ein Typ ist delimited oder MINIMAL-äquivalent.

### §7.2.4.4.1 Äquivalente Typen gegenseitig zuweisbar

**Spec:** §7.2.4.4.1, S. 76–77 — "If two types T1 and T2 are equivalent
according to the MINIMAL relation (see Clause 7.3.4.7), then they are mutually
assignable [...]. The reverse is not always true."

**Repo:** `crates/types/src/assignability.rs::is_assignable` (Z. 274, Prüfung
`w == r` Z. 301)

**Tests:** `crates/types/src/assignability.rs::tests::identical_type_identifiers_short_circuit_yes`,
`crates/types/src/assignability.rs::tests::equal_minimal_hash_is_assignable_with_empty_registry`,
`crates/types/src/assignability.rs::tests::equivalence_hash_identical_short_circuits_to_yes`

**Status:** done — `TK_NONE` ist ausgenommen; zwei Endpunkte mit dem
ungültigen Identifier sind nicht zuweisbar
(`tk_none_is_not_assignable_from_itself`).

### §7.2.4.4.2 Non-serialized Members werden ignoriert

**Spec:** §7.2.4.4.2, S. 77 — "Members that are marked as non-serialized, see
Sub Clause 7.3.1.2.1.14, shall be ignored during type compatibility checking."

**Repo:** `crates/types/src/from_ast.rs::structure` (Z. 2082, Auslassen Z. 2105–2123)

**Tests:** `crates/types/tests/table19_key_members.rs::a_non_serialized_member_does_not_enter_the_comparison`

**Status:** done — ein `@non_serialized`-Member kommt nicht in das
TypeObject.

### §7.2.4.4.3 Table 14 — ALIAS T1, Nicht-Alias T2

**Spec:** §7.2.4.4.3, Table 14, S. 77 — "ALIAS_TYPE | Any non ALIAS_TYPE type
kind T2 if and only if T1.base_type is-assignable-from T2".

**Repo:** `crates/types/src/assignability.rs::is_assignable` (Z. 274,
`resolve_alias_chain` Z. 306–311)

**Tests:** `crates/types/src/resolve.rs::tests::resolve_follows_single_alias_to_target`,
`crates/types/tests/evolution_assignability.rs::every_evolution_case_gets_a_verdict_and_the_verdicts_are_recorded`

**Status:** done

### §7.2.4.4.3 Table 14 — Nicht-Alias T1, ALIAS T2

**Spec:** §7.2.4.4.3, Table 14, S. 77 — "Any non ALIAS_TYPE type kind | ALIAS_TYPE
T2 if and only if T1 is-assignable-from T2.base_type".

**Repo:** `crates/types/src/assignability.rs::is_assignable` (Z. 274)

**Tests:** `crates/types/src/resolve.rs::tests::resolve_chained_aliases_two_deep`

**Status:** done

### §7.2.4.4.3 Table 14 — ALIAS T1, ALIAS T2

**Spec:** §7.2.4.4.3, Table 14, S. 77 — "ALIAS_TYPE | ALIAS_TYPE if and only if
T1.base_type is-assignable-from T2.base_type".

**Repo:** `crates/types/src/assignability.rs::check_minimal_types` (Z. 725, Arm
Alias)

**Tests:** `crates/types/src/resolve.rs::tests::resolve_chained_aliases_two_deep`

**Status:** done

### §7.2.4.4.3 Aliase vollständig aufgelöst

**Spec:** §7.2.4.4.3, S. 77 — "For the purpose of evaluating the
is-assignable-from relationship, aliases are considered to be fully resolved to
their ultimate base types."

**Repo:** `crates/types/src/resolve.rs::resolve_alias_chain`;
`crates/types/src/assignability.rs::key_members_fit` (Z. 1260, Auflösung
Z. 1291)

**Tests:** `crates/types/src/resolve.rs::tests::resolve_chained_aliases_two_deep`

**Status:** done

### §7.2.4.4.4 Table 15 — Primitiver Typ nur vom gleichen primitiven Typ

**Spec:** §7.2.4.4.4, Table 15, S. 77 — "Any Primitive Type | The same Primitive
Type | Copy the primitive object."

**Repo:** `crates/types/src/assignability.rs::primitive_compatible` (Z. 698)

**Tests:** `crates/types/src/assignability.rs::tests::only_the_identical_primitive_kind_is_assignable`,
`crates/types/src/assignability.rs::tests::primitive_widening_is_rejected_per_table_15`,
`crates/types/src/assignability.rs::tests::primitive_narrowing_and_cross_family_are_rejected`,
`crates/types/src/type_matcher_vectors.rs::uint8_to_uint32_widening_does_not_match`

**Status:** done — `octet` und `uint8` sind verschiedene primitive Typen und
nicht gegenseitig zuweisbar; die OMG-ShapeType-IDL in
`interop/shapes/omg-dds-rtps.idl` deklariert deshalb `sequence<uint8>`.

### §7.2.4.4.4 Table 15 — UINT8_TYPE von BITMASK mit bound 1 bis 8

**Spec:** §7.2.4.4.4, Table 15, S. 78 — "UINT8_TYPE | BITMASK_TYPE if and only if
T2.bound is between 1 and 8, inclusive."

**Repo:** `crates/types/src/assignability.rs::bitmask_uint_compatible` (Z. 560)

**Tests:** `crates/types/src/assignability.rs::tests::bitmask_is_assignable_to_the_uint_of_its_bound`,
`crates/types/src/assignability.rs::tests::bitmask_is_not_assignable_to_a_wider_uint`

**Status:** done

### §7.2.4.4.4 Table 15 — UINT16_TYPE von BITMASK mit bound 9 bis 16

**Spec:** §7.2.4.4.4, Table 15, S. 78 — "UINT16_TYPE | BITMASK_TYPE if and only if
T2.bound is between 9 and 16, inclusive."

**Repo:** `crates/types/src/assignability.rs::bitmask_uint_compatible` (Z. 560)

**Tests:** `crates/types/src/assignability.rs::tests::bitmask_is_assignable_to_the_uint_of_its_bound`

**Status:** done

### §7.2.4.4.4 Table 15 — UINT32_TYPE von BITMASK mit bound 17 bis 32

**Spec:** §7.2.4.4.4, Table 15, S. 78 — "UINT32_TYPE | BITMASK_TYPE if and only if
T2.bound is between 17 and 32, inclusive."

**Repo:** `crates/types/src/assignability.rs::bitmask_uint_compatible` (Z. 560)

**Tests:** `crates/types/src/assignability.rs::tests::bitmask_is_assignable_to_the_uint_of_its_bound`

**Status:** done

### §7.2.4.4.4 Table 15 — UINT64_TYPE von BITMASK mit bound 33 bis 64

**Spec:** §7.2.4.4.4, Table 15, S. 78 — "UINT64_TYPE | BITMASK_TYPE if and only if
T2.bound is between 33 and 64, inclusive."

**Repo:** `crates/types/src/assignability.rs::bitmask_uint_compatible` (Z. 560)

**Tests:** `crates/types/src/assignability.rs::tests::bitmask_is_assignable_to_the_uint_of_its_bound`

**Status:** done

### §7.2.4.4.4 Table 15 — Konstruktion UINT aus BITMASK

**Spec:** §7.2.4.4.4, Table 15, S. 78 — "For each bitflag that is set in the
bitmask construct the integer value (1 << position) using the position of that
bitflag. Add all those integer values to obtain the resulting object O1 of type
T1".

**Repo:** `crates/types/src/assignability.rs::bitmask_uint_compatible`,
`crates/wire-codec/src/decode.rs::read_bitmask`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::an_unsigned_integer_is_built_from_the_flags_of_a_bitmask`,
`crates/types/tests/type_system_rules.rs::an_unsigned_integer_is_assignable_from_a_bitmask_of_its_width_only`

**Status:** `done` — Table 33 schreibt eine Bitmaske als vorzeichenlose Ganzzahl ihres Bounds; ein Leser vom Typ `uint8` bis `uint64` liest die Oktette und erhält die Summe der `1 << position` der gesetzten Flags (vier Bounds, beide Byte-Reihenfolgen, beide Versionen). Die Zuweisbarkeit gilt nur für die Breite des Bounds. Die Umkehrung (Bitmaske aus Ganzzahl) nimmt der Code ebenfalls an, wie OpenDDS; Table 15 führt sie nicht.

### §7.2.4.4.5 Table 16 — STRING_TYPE: Elementtyp zuweisbar

**Spec:** §7.2.4.4.5, Table 16, S. 78 — "STRING_TYPE | STRING_TYPE if and only if
T1.element_type is-assignable-from T2.element_type".

**Repo:** `crates/types/src/assignability.rs::check_direct` (Z. 321, Arme
String8 und String16)

**Tests:** `crates/types/src/assignability.rs::tests::string8_vs_string16_not_assignable`,
`crates/types/src/assignability.rs::tests::string_small_and_large_interchangeable`,
`crates/types/src/assignability.rs::tests::string16_small_and_large_interchangeable`

**Status:** done — mit dem Default `ignore_string_bounds = true` hängt die
Zuweisbarkeit nicht am Bound.

### §7.2.4.4.5 Table 16 — Konstruktion: O2.length <= T1.length

**Spec:** §7.2.4.4.5, Table 16, S. 78 — "An object O2 of type T2 can-construct an
object of type T1 if and only if O2.length <= T1.length. Copy each character.
O1.length is set to O2.length."

**Repo:** `crates/wire/src/try_construct.rs::fit_string` (Z. 89);
`crates/wire/src/try_construct.rs::fit_wstring` (Z. 209)

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_unannotated_member_discards_the_sample`,
`crates/wire/tests/wire.rs::a_value_longer_than_its_string_bound_is_refused_and_not_truncated`

**Status:** done — der Bound einer `string<N>` zählt Oktette, der einer
`wstring<N>` UTF-16-Code-Units.

### §7.2.4.4.5.1 Example: Strings

**Spec:** §7.2.4.4.5.1, S. 78–79 — schmale und breite Zeichenketten sind
nicht gegenseitig zuweisbar; eine zu lange Zeichenkette konstruiert kein
T1-Objekt.

**Repo:** `crates/types/src/assignability.rs::check_direct`,
`crates/wire-codec/src/decode.rs (TryConstruct)`

**Tests:** `crates/types/tests/type_system_rules.rs::narrow_and_wide_strings_are_not_assignable_to_each_other`,
`crates/wire-probe/tests/xtypes_enc_items.rs::a_string_longer_than_the_bound_of_the_reader_constructs_no_object`

**Status:** `done` — schmale und breite Zeichenketten sind nicht gegenseitig zuweisbar, gleichartige sind es unabhängig vom Bound; eine Zeichenkette über dem Bound des Lesers konstruiert kein Objekt (`ProbeError::Discarded`), eine im Bound schon.

### §7.2.4.4.6 Table 17 — ARRAY_TYPE: gleiche Bounds, Element stark zuweisbar

**Spec:** §7.2.4.4.6, Table 17, S. 79 — "ARRAY_TYPE | ARRAY_TYPE if and only if:
T1.bounds[] == T2.bounds[]; T1.element_type is strongly assignable from
T2.element_type".

**Repo:** `crates/types/src/assignability.rs::strongly_assignable`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::array_elements_and_map_keys_and_values_are_strongly_assignable`,
`crates/types/tests/strong_assignability_and_table19.rs::a_sequence_of_final_structs_needs_equivalent_elements`

**Status:** done — Der Elementtyp (bei Map Schlüssel und Wert) wird mit `strongly_assignable` geprüft.

### §7.2.4.4.6 Table 17 — ARRAY_TYPE: Konstruktion je Element mit TryConstruct

**Spec:** §7.2.4.4.6, Table 17, S. 79 — "Each element of the T1 array shall be
constructed from the corresponding element of the O2 array. If an element of
T1 cannot be constructed from the O2 element, the result depends on the
TryConstruct behavior associated with T1 element type."

**Repo:** `crates/ast/src/query.rs::element_try_construct` (Z. 413);
`crates/wire-codec/src/decode.rs`

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_element_annotation_trims_the_element_and_not_the_collection`

**Status:** done

### §7.2.4.4.6 Table 17 — SEQUENCE_TYPE: Element stark zuweisbar

**Spec:** §7.2.4.4.6, Table 17, S. 80 — "SEQUENCE_TYPE | SEQUENCE_TYPE if and only
if T1.element_type is strongly assignable from T2.element_type".

**Repo:** `crates/types/src/assignability.rs::strongly_assignable`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::array_elements_and_map_keys_and_values_are_strongly_assignable`,
`crates/types/tests/strong_assignability_and_table19.rs::a_sequence_of_final_structs_needs_equivalent_elements`

**Status:** done — Der Elementtyp (bei Map Schlüssel und Wert) wird mit `strongly_assignable` geprüft.

### §7.2.4.4.6 Table 17 — SEQUENCE_TYPE: Konstruktion O2.length <= T1.length

**Spec:** §7.2.4.4.6, Table 17, S. 80 — "An object O2 of type T2 can construct T1
if and only if O2.length <= T1.length. [...] If an element of O2 cannot
construct T1.element_type, the result depends on the TryConstruct behavior".

**Repo:** `crates/wire/src/try_construct.rs::fit_sequence` (Z. 131)

**Tests:** `crates/wire-probe/tests/try_construct.rs::trim_cuts_a_sequence_to_its_bound_keeping_the_first_elements`,
`crates/lang-rules/tests/codec.rs::a_bounded_sequence_checks_its_bound_and_an_unbounded_one_has_none`

**Status:** done

### §7.2.4.4.6 Table 17 — MAP_TYPE: Schlüssel und Wert stark zuweisbar

**Spec:** §7.2.4.4.6, Table 17, S. 80 — "MAP_TYPE | MAP_TYPE if and only if:
T1.key_element_type is strongly assignable from T2.key_element_type;
T1.element_type is strongly assignable from T2.element_type."

**Repo:** `crates/types/src/assignability.rs::strongly_assignable`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::array_elements_and_map_keys_and_values_are_strongly_assignable`,
`crates/types/tests/strong_assignability_and_table19.rs::a_sequence_of_final_structs_needs_equivalent_elements`

**Status:** done — Der Elementtyp (bei Map Schlüssel und Wert) wird mit `strongly_assignable` geprüft.

### §7.2.4.4.6 Table 17 — MAP_TYPE: Konstruktion

**Spec:** §7.2.4.4.6, Table 17, S. 80 — "An object O2 of type T2 can construct T1
if and only if O2.length <= T1.length [...] If a key element of O2 cannot
construct the corresponding key type of T1 the entire map O2 cannot construct
any object of type T1."

**Repo:** `crates/wire/src/try_construct.rs::fit_map` (Z. 169)

**Tests:** `crates/wire-conformance/tests/map_bytes.rs::an_over_long_map_is_discarded_where_the_position_says_discard`,
`crates/wire-conformance/tests/map_bytes.rs::an_over_long_map_is_trimmed_where_the_position_says_trim`,
`crates/idl-lower/tests/annotations.rs::a_map_key_may_only_be_discard`

**Status:** done

### §7.2.4.4.7 Table 18 — BITMASK von BITMASK: gleicher Bound

**Spec:** §7.2.4.4.7, Table 18, S. 81 — "BITMASK_TYPE | BITMASK_TYPE if and only
if T1.bound == T2.bound".

**Repo:** `crates/types/src/assignability.rs::check_minimal_types` (Z. 725, Arm
Bitmask Z. 813–844)

**Tests:** `crates/types/tests/table19_key_members.rs::two_bitmasks_of_the_same_bound_are_assignable_whatever_their_flags`

**Status:** done

### §7.2.4.4.7 Table 18 — BITMASK von UINT_8/16/32/64

**Spec:** §7.2.4.4.7, Table 18, S. 81 — "UINT_32_TYPE if and only if T1.bound is
between 17 and 32, inclusive. UINT_16_TYPE [...] 9 and 16 [...] UINT_64_TYPE
[...] 33 and 64 [...] UINT_8_TYPE [...] 1 and 8, inclusive."

**Repo:** `crates/types/src/assignability.rs::check_direct` (Z. 321, Arm
Primitive gegen Hash Z. 516–519); `crates/types/src/assignability.rs::bitmask_uint_compatible`
(Z. 560)

**Tests:** `crates/types/src/assignability.rs::tests::bitmask_is_assignable_to_the_uint_of_its_bound`,
`crates/types/src/assignability.rs::tests::a_struct_against_a_uint_is_still_no_row`

**Status:** done

### §7.2.4.4.7 Table 18 — BITMASK: Konstruktion je gemeinsamem Bit

**Spec:** §7.2.4.4.7, Table 18, S. 81 — "Preserve bit values by index for all
bits identified in both T1 and T2."

**Repo:** `crates/wire-codec/src/decode.rs::read_bitmask` (Z. 2499)

**Tests:** `crates/wire-codec/tests/bitmask_unnamed_bits.rs::a_bit_inside_the_bound_that_the_type_does_not_name_is_not_read`,
`crates/wire/tests/wire.rs::a_bitmask_read_keeps_only_the_declared_bits`,
`crates/wire-conformance/tests/bitmask_bytes.rs::a_bit_the_type_does_not_declare_is_not_part_of_the_value`

**Status:** done — Gemeinsame Bits bleiben erhalten; ein gesetztes Bit, das der Reader nicht benennt, gehört nicht zum Wert und wird beim Lesen verworfen (Rust-Leser und die von `lang-rules` erzeugten Decoder).

### §7.2.4.4.7 Table 18 — BITMASK: ein Bit nur im Writer wird verworfen

**Spec:** §7.2.4.4.7, Table 18, S. 81 — Konstruktion nur über die Bits, die
beide Typen benennen.

**Repo:** `crates/wire-codec/src/decode.rs::read_bitmask`

**Tests:** `crates/wire-codec/tests/bitmask_unnamed_bits.rs::a_bit_inside_the_bound_that_the_type_does_not_name_is_not_read`,
`crates/wire-codec/tests/bitmask_unnamed_bits.rs::the_declared_bits_beside_an_unnamed_one_are_kept`

**Status:** done — `read_bitmask` verwirft ein Bit, das der Typ nicht benennt, ohne Fehler; die benannten Bits neben einem unbenannten bleiben.

### §7.2.4.4.7 Table 18 — ENUMERATION: gleiche Extensibility

**Spec:** §7.2.4.4.7, Table 18, S. 81 — "ENUMERATION_TYPE if an only if:
T1.extensibility == T2.extensibility".

**Repo:** `crates/types/src/assignability.rs::check_enumerations`

**Tests:** `crates/types/src/assignability.rs::tests::a_final_enum_has_the_same_literals_on_both_sides`,
`crates/types/tests/strong_assignability_and_table19.rs::an_enum_without_an_extensibility_flag_is_appendable`

**Status:** done — Eine Aufzählung ohne Extensibility-Flag gilt als APPENDABLE (§7.3.1.2.1.8), und die Extensibility wird immer verglichen: FINAL gegen APPENDABLE ist nicht zuweisbar.

### §7.2.4.4.7 Table 18 — ENUMERATION: Extensibility-Vergleich ist getestet

**Spec:** §7.2.4.4.7, Table 18, S. 81 — `T1.extensibility == T2.extensibility`.

**Repo:** —

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::an_enum_without_an_extensibility_flag_is_appendable`

**Status:** done — Der Test prüft FINAL gegen APPENDABLE und gegen eine Aufzählung ohne Flag in beiden Richtungen.

### §7.2.4.4.7 Table 18 — ENUMERATION: gleicher Name, gleicher Wert und umgekehrt

**Spec:** §7.2.4.4.7, Table 18, S. 81 — "Any literals that have the same name in
T1 and T2 also have the same value, and any literals that have the same value
in T1 and T2 also have the same name. This behavior may be modified with the
@ignore_literal_names annotation".

**Repo:** `crates/types/src/assignability.rs::check_enumerations` (Z. 964,
Schleife Z. 1025–1038)

**Tests:** `crates/types/src/assignability.rs::tests::enum_literals_that_disagree_are_not_assignable`,
`crates/types/src/assignability.rs::tests::an_enum_that_grew_a_literal_is_still_assignable`,
`crates/types/src/assignability.rs::tests::ignore_literal_names_on_both_sides_drops_the_name_rule`,
`crates/types/tests/table19_key_members.rs::ignore_literal_names_needs_the_annotation_on_both_types`

**Status:** done — zusätzlich verlangt der Code gleiche `bit_bound` beider
Aufzählungen (DDSXTY14-34), was Table 18 nicht nennt.

### §7.2.4.4.7 Table 18 — ENUMERATION: FINAL verlangt dieselben Literale

**Spec:** §7.2.4.4.7, Table 18, S. 81 — "If extensibility is final T1 and T2 have
the same literals."

**Repo:** `crates/types/src/assignability.rs::check_enumerations` (Z. 964,
Zweig Z. 1009–1023)

**Tests:** `crates/types/src/assignability.rs::tests::a_final_enum_has_the_same_literals_on_both_sides`

**Status:** done

### §7.2.4.4.7 Table 18 — ENUMERATION: Konstruktion

**Spec:** §7.2.4.4.7, Table 18, S. 81 — "Choose the corresponding T1 literal if
it exists. If the name or value of the T2 object does not exist in T1, the
object cannot construct any object of type T1."

**Repo:** `crates/wire-codec/src/decode.rs` (Aufzählung, Z. 2457–2473)

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_enumerator_the_reader_does_not_declare_is_a_construction_failure`

**Status:** done — die Entscheidung trifft dann `@try_construct` des Orts.

### §7.2.4.4.8 Aggregated: Zuordnung über Member-ID und Name

**Spec:** §7.2.4.4.8, S. 81 — "For aggregated types, is-assignable-from is based
on the same relationship between the types' members. The correspondence
between members in the two types is established based on their respective
member IDs and on their respective member names."

**Repo:** `crates/types/src/assignability.rs::check_minimal_types` (Z. 725)

**Tests:** `crates/types/src/assignability.rs::tests::mutable_struct_member_id_matching`,
`crates/types/src/assignability.rs::tests::a_member_name_at_two_different_ids_is_refused`

**Status:** done — der Name wird im MINIMAL-TypeObject als 4-Oktett-Hash
verglichen.

### §7.2.4.4.8 Table 19 — UNION: gleiche Extensibility

**Spec:** §7.2.4.4.8, Table 19, S. 82 — "T1.extensibility == T2.extensibility."

**Repo:** `crates/types/src/assignability.rs::check_unions`

**Tests:** `crates/types/tests/type_system_rules.rs::two_unions_of_different_extensibility_are_not_assignable`,
`crates/types/src/assignability.rs::tests::a_final_union_has_a_default_member_on_both_sides_or_neither`

**Status:** `done` — zwei Unionen gleicher Extensibility (`@final`, `@appendable`, `@mutable`) sind zuweisbar, je zwei verschiedener nicht (alle sechs Paarungen).

### §7.2.4.4.8 Table 19 — UNION: Extensibility-Vergleich ohne Test

**Spec:** §7.2.4.4.8, Table 19, S. 82 — gleiche Extensibility.

**Repo:** `crates/types/src/assignability.rs::check_unions`

**Tests:** `crates/types/tests/type_system_rules.rs::two_unions_of_different_extensibility_are_not_assignable`

**Status:** `done` — der Test deckt alle Paarungen verschiedener Extensibility ab.

### §7.2.4.4.8 Table 19 — UNION: Diskriminator stark zuweisbar

**Spec:** §7.2.4.4.8, Table 19, S. 82 — "T1.discriminator.type
is-strongly-assignable-from T2.discriminator.type."

**Repo:** `crates/types/src/assignability.rs::check_unions` (Z. 1536, Aufruf
Z. 1589–1597)

**Tests:** `crates/types/src/assignability.rs::tests::two_unions_need_a_case_label_in_common`

**Status:** done — ein Diskriminator ist primitiv oder eine Aufzählung und
damit delimited; `is_assignable` fällt hier mit der starken Zuweisbarkeit
zusammen.

### §7.2.4.4.8 Table 19 — UNION: beide Diskriminatoren Key oder keiner

**Spec:** §7.2.4.4.8, Table 19, S. 82 — "Either the discriminators of both T1 and
T2 are keys or neither are keys."

**Repo:** `crates/types/src/assignability.rs::check_unions` (Z. 1536, Prüfung
Z. 1580–1587)

**Tests:** `crates/types/tests/table19_key_members.rs::one_keyed_union_discriminator_and_one_unkeyed_are_not_assignable`

**Status:** done

### §7.2.4.4.8 Table 19 — UNION: gleicher Name, gleiche ID und umgekehrt

**Spec:** §7.2.4.4.8, Table 19, S. 83 — "Any members in T1 and T2 that have the
same name also have the same ID and any members with the same ID also have
the same name."

**Repo:** `crates/types/src/assignability.rs::union_names_and_ids_agree` (Z. 1121)

**Tests:** `crates/types/src/assignability.rs::tests::a_shared_union_label_may_carry_different_member_ids`

**Status:** done — abschaltbar über `ignore_member_names`.

### §7.2.4.4.8 Table 19 — UNION: Label von T2, das in T1 einen Member wählt

**Spec:** §7.2.4.4.8, Table 19, S. 82 — "For all non-default labels in T2 that
select some member in T1 (including selecting the member in T1's default
label), the type of the selected member in T1 is assignable from the type of
the T2 member."

**Repo:** `crates/types/src/assignability.rs::check_unions` (Z. 1536, Schleife
Z. 1666–1712)

**Tests:** `crates/types/src/assignability.rs::tests::a_label_the_other_side_lacks_is_checked_against_its_default_member`

**Status:** done

### §7.2.4.4.8 Table 19 — UNION: Label von T1, das den Default von T2 wählt

**Spec:** §7.2.4.4.8, Table 19, S. 82 — "If any non-default labels in T1 that
select the default member in T2, the type of the member in T1 is assignable
from the type of the T2 default member."

**Repo:** `crates/types/src/assignability.rs::check_unions` (Z. 1536, Z. 1734–1749)

**Tests:** `crates/types/src/assignability.rs::tests::a_label_the_other_side_lacks_is_checked_against_its_default_member`

**Status:** done

### §7.2.4.4.8 Table 19 — UNION: beide mit Default-Member

**Spec:** §7.2.4.4.8, Table 19, S. 82 — "If T1 and T2 both have default labels,
the type associated with T1 default member is assignable from the type
associated with T2 default member."

**Repo:** `crates/types/src/assignability.rs::check_unions` (Z. 1536, Z. 1751–1754)

**Tests:** `crates/types/src/assignability.rs::tests::a_final_union_has_a_default_member_on_both_sides_or_neither`

**Status:** done

### §7.2.4.4.8 Table 19 — UNION: FINAL gleiche Labels, sonst ein gemeinsames

**Spec:** §7.2.4.4.8, Table 19, S. 83 — "If T1 (and therefore T2) extensibility
is final then the set of labels is identical. Otherwise, they have at least
one common label other than the default label."

**Repo:** `crates/types/src/assignability.rs::check_unions` (Z. 1536,
Z. 1653–1664 und Z. 1714–1728)

**Tests:** `crates/types/src/assignability.rs::tests::two_unions_need_a_case_label_in_common`,
`crates/types/src/assignability.rs::tests::a_default_case_is_not_a_case_label_in_common`

**Status:** done — für FINAL verlangt der Code zusätzlich, dass entweder
beide oder keine Seite einen Default-Member hat.

### §7.2.4.4.8 Table 19 — UNION: Konstruktion

**Spec:** §7.2.4.4.8, Table 19, S. 82–83 — Diskriminator aus O2 oder nach
TryConstruct (DEFAULT); gewählter Member m1 aus m2 oder nach TryConstruct;
wählt der Wert in O1 keinen Member, wird m2 abgeschnitten; wählt er in O2
keinen, aber in O1 einen Member, nimmt m1 seinen Default.

**Repo:** `crates/wire-codec/src/decode.rs`;
`crates/ast/src/query.rs::discriminator_try_construct` (Z. 508)

**Tests:** `crates/wire-probe/tests/try_construct.rs::a_defaulted_discriminator_defaults_the_member_it_now_selects`,
`crates/lang-rules/tests/c_end_to_end.rs::a_union_member_the_stream_does_not_carry_takes_its_default`

**Status:** done — die Spalte nennt TryConstruct „DEFAULT“; Table 11 nennt
die Art USE_DEFAULT (Benennung der Spec, in der Befunddatei geführt).

### §7.2.4.4.8 Table 19 — STRUCTURE: gleiche Extensibility

**Spec:** §7.2.4.4.8, Table 19, S. 83 — "T1 and T2 have the same extensibility
kind."

**Repo:** `crates/types/src/assignability.rs::check_minimal_types` (Z. 725,
Prüfung Z. 738–742)

**Tests:** `crates/types/src/assignability.rs::tests::extensibility_mismatch_fails`,
`crates/types/src/assignability.rs::tests::minimal_and_complete_hash_still_reject_an_extensibility_mismatch`

**Status:** done

### §7.2.4.4.8 Table 19 — STRUCTURE: gleicher Name, gleiche ID und umgekehrt

**Spec:** §7.2.4.4.8, Table 19, S. 83 — "Any members in T1 and T2 that have the
same name also have the same ID and any members with the same ID also have the
same name."

**Repo:** `crates/types/src/assignability.rs::struct_names_and_ids_agree` (Z. 1100)

**Tests:** `crates/types/src/assignability.rs::tests::a_member_name_at_two_different_ids_is_refused`

**Status:** done — abschaltbar über `ignore_member_names`.

### §7.2.4.4.8 Table 19 — STRUCTURE: mindestens eine gemeinsame Member-ID

**Spec:** §7.2.4.4.8, Table 19, S. 83 — "There is at least one member 'm1' of T1
and one corresponding member 'm2' of T2 such that m1.id == m2.id."

**Repo:** `crates/types/src/assignability.rs::at_least_one_common_member_id` (Z. 1155)

**Tests:** `crates/types/src/assignability.rs::tests::two_mutable_structs_need_a_member_id_in_common`

**Status:** done — zwei Strukturen ohne Member gelten als zuweisbar; die
Regel setzt Member voraus.

### §7.2.4.4.8 Table 19 — STRUCTURE: KeyErased der Membertypen zuweisbar

**Spec:** §7.2.4.4.8, Table 19, S. 84 — "For any member 'm2' in T2, if there is a
member 'm1' in T1 with the same member ID, then the type KeyErased(m1.type)
is-assignable-from the type KeyErased(m2.type)."

**Repo:** `crates/types/src/assignability.rs::key_erased`,
`crates/types/src/assignability.rs::check_member_pair`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::a_nested_types_key_bound_does_not_count_for_a_non_key_member`,
`crates/types/src/assignability.rs::tests::mutable_struct_member_id_matching`

**Status:** done — Die Membertypen werden als `KeyErased` verglichen (`assignability.rs::key_erased`): die Länge eines Key-Members eines geschachtelten Typs zählt für einen Nicht-Key-Member nicht.

### §7.2.4.4.8 Table 19 — STRUCTURE: nicht-optionale Member mit must understand auf beiden Seiten

**Spec:** §7.2.4.4.8, Table 19, S. 84 — "Members for which both optional is false
and must_understand is true in either T1 or T2 appear (i.e., have a
corresponding member of the same member ID) in both T1 and T2."

**Repo:** `crates/types/src/assignability.rs::must_not_be_missing` (Z. 1054);
`crates/types/src/assignability.rs::check_appendable_structs` (Z. 1414);
`crates/types/src/assignability.rs::check_mutable_structs` (Z. 1474)

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::an_optional_must_understand_member_may_be_missing`,
`crates/types/src/assignability.rs::tests::an_appendable_tail_member_that_must_be_understood_is_no`,
`crates/types/src/assignability.rs::tests::a_mutable_reader_member_the_writer_lacks_is_yes_unless_it_must_be_understood`,
`crates/types/src/assignability.rs::tests::a_mutable_writer_member_the_reader_lacks_and_must_understand_is_no`

**Status:** done — `must_not_be_missing` liest `IS_OPTIONAL`: ein Member `@optional @must_understand`, der auf einer Seite fehlt, macht das Paar zuweisbar.

### §7.2.4.4.8 Table 19 — STRUCTURE: ein optionaler Member mit must understand darf fehlen

**Spec:** §7.2.4.4.8, Table 19, S. 84 — "both optional is false and
must_understand is true".

**Repo:** `crates/types/src/assignability.rs::must_not_be_missing`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::an_optional_must_understand_member_may_be_missing`

**Status:** done — `must_not_be_missing` liest `IS_OPTIONAL`.

### §7.2.4.4.8 Table 19 — STRUCTURE: Key-Member auf beiden Seiten

**Spec:** §7.2.4.4.8, Table 19, S. 84 — "Members marked as key in either T1 or T2
appear (i.e., have a corresponding member of the same member ID) in both T1 and
T2."

**Repo:** `crates/types/src/assignability.rs::must_not_be_missing`

**Tests:** `crates/types/tests/type_system_rules.rs::a_key_member_that_one_side_lacks_makes_the_structures_not_assignable`,
`crates/types/tests/table19_key_members.rs::a_writer_enum_key_literal_the_reader_lacks_is_not_assignable`,
`crates/types/src/assignability.rs::tests::a_mutable_writer_member_the_reader_lacks_and_must_understand_is_no`

**Status:** `done` — ein Key-Member, das auf einer Seite fehlt (Leser oder Schreiber), macht die Strukturen nicht zuweisbar; ein Member, das auf beiden Seiten steht und nur auf einer Key ist, nicht.

### §7.2.4.4.8 Table 19 — STRUCTURE: fehlender Key-Member, kein Test

**Spec:** §7.2.4.4.8, Table 19, S. 84 — Key-Member auf beiden Seiten.

**Repo:** `crates/types/src/assignability.rs::must_not_be_missing`

**Tests:** `crates/types/tests/type_system_rules.rs::a_key_member_that_one_side_lacks_makes_the_structures_not_assignable`

**Status:** `done` — der Test lässt den Key-Member je auf der Leser- und auf der Schreiberseite fehlen.

### §7.2.4.4.8 Table 19 — STRUCTURE: Key-Member vom Typ String, m1.length >= m2.length

**Spec:** §7.2.4.4.8, Table 19, S. 84 — "For any string key member m2 in T2, the
m1 member of T1 with the same member ID verifies m1.type.length >=
m2.type.length."

**Repo:** `crates/types/src/assignability.rs::key_members_fit` (Z. 1260,
Prüfung Z. 1301–1308)

**Tests:** `crates/types/tests/table19_key_members.rs::a_shorter_string_key_in_the_reader_is_not_assignable`,
`crates/types/tests/table19_key_members.rs::a_shorter_string_member_that_is_not_a_key_is_still_assignable`

**Status:** done — über `KeyMemberBounds` abschaltbar; der Default ist die
Regel der Spec. Die OMG-Testsuite erwartet für vier `struct_key_*`-Fälle das
Gegenteil (`docs/omg-xtypes-issues.md` §C1).

### §7.2.4.4.8 Table 19 — STRUCTURE: Enumerated-Key: alle Literale von m2 in m1

**Spec:** §7.2.4.4.8, Table 19, S. 84 — "For any enumerated key member m2 in T2,
the m1 member of T1 with the same member ID verifies that all literals in
m2.type appear as literals in m1.type."

**Repo:** `crates/types/src/assignability.rs::key_members_fit` (Z. 1260,
Prüfung Z. 1313–1329)

**Tests:** `crates/types/tests/table19_key_members.rs::a_writer_enum_key_literal_the_reader_lacks_is_not_assignable`

**Status:** done — der Vergleich geht über den Wert des Literals.

### §7.2.4.4.8 Table 19 — STRUCTURE: Sequence-/Map-Key m1.length >= m2.length

**Spec:** §7.2.4.4.8, Table 19, S. 84 — "For any sequence or map key member m2 in
T2, the m1 member of T1 with the same member ID verifies m1.type.length >=
m2.type.length."

**Repo:** `crates/types/src/assignability.rs::key_members_fit` (Z. 1260);
`crates/types/src/assignability.rs::key_length_of` (Z. 1198)

**Tests:** `crates/types/tests/table19_key_members.rs::a_shorter_sequence_key_in_the_reader_is_not_assignable`

**Status:** done — eine Map ist als Key-Member nach §7.2.2.4.4.4.8 nicht
zulässig; die Prüfung deckt sie dennoch ab.

### §7.2.4.4.8 Table 19 — STRUCTURE: Key-Member vom Typ Struktur oder Union über KeyHolder

**Spec:** §7.2.4.4.8, Table 19, S. 84 — "For any structure or union key member
m2 in T2, the m1 member of T1 with the same member ID verifies that
KeyHolder(m1.type) is-assignable-from KeyHolder(m2.type)."

**Repo:** `crates/types/src/assignability.rs::key_holder_assignable`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::a_nested_types_key_bound_does_not_count_for_a_non_key_member`,
`crates/types/tests/strong_assignability_and_table19.rs::every_value_of_a_union_key_member_must_select_a_member_in_the_reader`

**Status:** done — Ein Key-Member vom Typ Struktur oder Union wird über `KeyHolder` verglichen (`key_holder_struct`, `key_holder_assignable`).

### §7.2.4.4.8 Table 19 — STRUCTURE: Key-Member vom Typ Union, KeyHolder je gewähltem Member

**Spec:** §7.2.4.4.8, Table 19, S. 84–85 — "For any union key member m2 in T2,
the m1 member of T1 with the same member ID verifies that: For every
discriminator value of m2.type that selects a member m22 in m2.type, the
discriminator value selects a member m11 in m1.type that verifies
KeyHolder(m11.type) is-assignable-from KeyHolder(m22.type)."

**Repo:** `crates/types/src/assignability.rs::union_key_values_fit`

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::every_value_of_a_union_key_member_must_select_a_member_in_the_reader`

**Status:** done — Für einen Key-Member vom Typ Union muss jeder Wert des Diskriminators im Writer einen Member im Reader wählen (`union_key_values_fit`).

### §7.2.4.4.8 Table 19 — STRUCTURE: APPENDABLE — gleicher Index, gleiche ID, optional, stark zuweisbar

**Spec:** §7.2.4.4.8, Table 19, S. 85 — "AND if T1 is appendable, then members
with the same member_index have the same member ID, the same setting for the
'optional' attribute and the T1 member type is strongly assignable from the T2
member type."

**Repo:** `crates/types/src/assignability.rs::check_appendable_structs` (Z. 1414)

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::members_of_appendable_and_final_types_are_strongly_assignable`,
`crates/types/tests/strong_assignability_and_table19.rs::members_of_the_same_index_agree_on_optional`,
`crates/types/src/assignability.rs::tests::appendable_struct_with_extra_writer_field_is_assignable`,
`crates/types/src/assignability.rs::tests::appendable_struct_reader_longer_than_writer_is_yes`

**Status:** done — Member-ID, Typ (stark zuweisbar) und `optional` am gleichen Index werden verglichen.

### §7.2.4.4.8 Table 19 — STRUCTURE: APPENDABLE und FINAL — optional wird verglichen

**Spec:** §7.2.4.4.8, Table 19, S. 85 — "the same setting for the 'optional'
attribute".

**Repo:** —

**Tests:** `crates/types/tests/strong_assignability_and_table19.rs::members_of_the_same_index_agree_on_optional`

**Status:** done — `check_appendable_structs` und `check_final_structs` vergleichen `optional` der Member am gleichen Index.

### §7.2.4.4.8 Table 19 — STRUCTURE: FINAL — zusätzlich gleiche Menge der IDs

**Spec:** §7.2.4.4.8, Table 19, S. 85 — "AND if T1 is final, then they meet the
same condition as for T1 being appendable and in addition T1 and T2 have the
same set of member IDs."

**Repo:** `crates/types/src/assignability.rs::check_final_structs` (Z. 1370)

**Tests:** `crates/types/src/assignability.rs::tests::final_struct_with_extra_writer_field_is_not_assignable`,
`crates/types/src/assignability.rs::tests::final_struct_member_count_mismatch_is_no`,
`crates/types/tests/strong_assignability_and_table19.rs::members_of_appendable_and_final_types_are_strongly_assignable`

**Status:** done — Gleiche Anzahl, gleiche ID je Position, gleiches `optional` und starke Zuweisbarkeit werden geprüft.

### §7.2.4.4.8 Table 19 — STRUCTURE: Basistyp-Member aufgefaltet

**Spec:** §7.2.4.4.8, Table 19, S. 85 — "For the purposes of the above
conditions, members belonging to base types of T1 or T2 shall be considered
'expanded' inside T1 or T2 respectively, as if they had been directly defined
as part of the sub-type."

**Repo:** `crates/types/src/assignability.rs::check_minimal_types` (Z. 725,
Auffalten Z. 757–780); `crates/types/src/assignability.rs::flatten_inheritance`
(Z. 58)

**Tests:** `crates/types/src/assignability.rs::tests::two_level_inheritance_assignability_chain`,
`crates/types/src/assignability.rs::tests::flat_type_construction_two_levels`

**Status:** done

### §7.2.4.4.8 Table 19 — STRUCTURE: Konstruktion

**Spec:** §7.2.4.4.8, Table 19, S. 83–84 — jeder Member von T1 nimmt den Wert
des T2-Members gleicher ID oder gleichen Namens; nicht-optionale fehlende
Member nehmen ihren Default, optionale keinen Wert; ein vorhandener
Member von T2 mit must understand ohne Gegenstück in T1 verhindert die
Konstruktion, unabhängig von TryConstruct; ein Member, der nicht konstruiert
werden kann, folgt seinem TryConstruct.

**Repo:** `crates/wire-codec/src/decode.rs` (Z. 1485–1498, Z. 1573–1589);
`crates/wire-codec/src/default.rs::member` (Z. 100)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_unknown_mutable_member_is_skipped_and_a_must_understand_one_is_not`,
`crates/lang-rules/tests/c_end_to_end.rs::a_mutable_member_that_did_not_arrive_takes_its_default`,
`crates/wire-probe/tests/try_construct.rs::use_default_substitutes_table_nines_value_and_keeps_the_sample`

**Status:** done

### §7.2.4.4.8 Note zu den Regeln für Key-Member

**Spec:** §7.2.4.4.8, Table 19, S. 85 — "Note: The rules regarding key members
ensure that the key of T2 can be transformed faithfully into the key of T1
without aliasing or loss of information."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung der Regeln für Key-Member.

### §7.2.4.4.8.1 Example: Type Truncation

**Spec:** §7.2.4.4.8.1, S. 85–86 — `Coordinate2D` ist von `Coordinate3D`
zuweisbar.

**Repo:** `crates/types/src/assignability.rs::check_appendable_structs`

**Tests:** `crates/types/tests/type_system_rules.rs::coordinate2d_is_assignable_from_coordinate3d`

**Status:** `done` — `Coordinate2D` ist von `Coordinate3D` zuweisbar (IDL aus dem Beispiel, unannotiert, also `@appendable`).

### §7.2.4.4.8.2 Example: Type Inheritance

**Spec:** §7.2.4.4.8.2, S. 86 — "LandVehicle is assignable from Vehicle. Any
consumer of the latter that receives an instance of the former will observe
the value of the member km_per_hour and ignore the member num_wheels."

**Repo:** `crates/types/src/assignability.rs::flatten_inheritance`

**Tests:** `crates/types/tests/type_system_rules.rs::a_structure_and_its_derived_structure_are_assignable`

**Status:** `done` — `Vehicle` und `LandVehicle` sind in beiden Richtungen zuweisbar (der erste Satz des Beispiels vertauscht die Richtung, der zweite nennt sie richtig; Befunddatei).

### §7.2.4.4.8.3 Example: Type Refactoring

**Spec:** §7.2.4.4.8.3, S. 86–87 — `Giraffe` vor und nach dem Einziehen von
`Mammal` sind gegenseitig zuweisbar, weil die Member aufgefaltet verglichen
werden.

**Repo:** `crates/types/src/assignability.rs::flatten_inheritance`

**Tests:** `crates/types/tests/type_system_rules.rs::both_versions_of_giraffe_are_assignable_to_one_another`

**Status:** `done` — beide Fassungen von `Giraffe` sind gegenseitig zuweisbar, `Giraffe` und `Snake` nicht.

---

## §7.3 Type Representation
### §7.3 Zweck und Table 20

**Spec:** §7.3, S. 87–89 — "This specification introduces multiple equivalent
Type Representations. [...] These representations are all equivalent because
they describe the same Type System." Table 20 vergleicht IDL, TypeObject, XML
und XSD.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung; die vier Representations sind
unter §7.3.1 bis §7.3.4 einzeln geführt.

---

## §7.3.1 IDL Type Representation

### §7.3.1.1 IDL Compatibility — zwei Aspekte

**Spec:** §7.3.1.1, S. 89 — Rückwärtskompatibilität bestehender
Typdefinitionen und Vorwärtskompatibilität gegenüber IDL-Compilern ohne
XTypes.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung; die Anforderungen folgen in
§7.3.1.1.1 und §7.3.1.1.2.

### §7.3.1.1.1 Extensible DDS Profile von IDL 4.2

**Spec:** §7.3.1.1.1, S. 90 — "it uses the Extensible DDS Profile (Sub Clause
9.3.2 [IDL]), which is composed of [...] Core Data Types [...] Extended Data
Types [...] Anonymous Types [...] Annotations [...] General Purpose [...] Data
Modeling [...] Data Implementation [...] Code Generation".

**Repo:** `crates/idl-parse/src/parser.rs`; `crates/idl-lower/src/lower.rs::lower`
(Z. 358)

**Tests:** `crates/idl-lower/tests/conformance.rs::the_xtypes_specifications_own_idl_lowers_completely`,
`crates/idl-lower/tests/conformance.rs::every_fixture_lowers_to_exactly_the_diagnostics_it_should`

**Status:** done — die Bausteine selbst sind in `idl-4.2.md` geführt.

### §7.3.1.1.1 Konstrukte außerhalb des Profils: ignorieren oder warnen

**Spec:** §7.3.1.1.1, S. 90 — "These constructs remain legal for use in IDL files
provided to IDL compilers compliant with this specification. However, their
meanings are undefined with respect to this specification. Compilers that do
not support them shall ignore them or issue a warning rather than halting with
an error."

**Repo:** `crates/idl-lower/src/lower.rs::lower`,
`crates/ast/src/profile/outside.rs::drop_outside_profile`,
`tools/idlc/src/main.rs::run_generate`,
`crates/zerodds-build/src/lib.rs::leave_out_what_xtypes_has_no_type_for`

**Tests:** `crates/idl-lower/tests/lowering.rs::an_any_member_reaches_the_model_as_a_type_rather_than_a_broken_reference`,
`crates/idl-lower/tests/lowering.rs::a_native_declaration_becomes_a_native_type`,
`crates/idl-lower/tests/outside_profile.rs::what_holds_an_any_is_dropped_and_the_rest_of_the_file_stays`,
`tools/idlc/tests/outside_profile.rs::the_rest_of_the_file_is_generated_and_the_two_declarations_are_named`,
`tools/idlc/tests/outside_profile.rs::deny_warnings_turns_it_back_into_a_failure`

**Status:** done — Das IDL-Front-End nimmt `any`, `native`, `valuetype` und Interfaces in das Modell, ohne den Lauf anzuhalten. `zerodds-idlc generate` und `zerodds-build` lassen eine Deklaration aus, die `any`, `ValueBase`, `native` oder einen Objektverweis erreicht (`Ast::drop_outside_profile`), melden sie mit Namen, Konstrukt und Paragraph als Warnung und erzeugen den Rest der Datei. `--deny-warnings` macht daraus wieder einen Fehler.

### §7.3.1.1.1 Konstrukte außerhalb des Profils: Auslassen und Melden statt Abweisen

**Spec:** §7.3.1.1.1, S. 90 — "shall ignore them or issue a warning rather than
halting with an error."

**Repo:** —

**Tests:** `crates/idl-lower/tests/outside_profile.rs::what_holds_an_any_is_dropped_and_the_rest_of_the_file_stays`,
`crates/idl-lower/tests/outside_profile.rs::the_report_names_the_construct_and_the_clause`,
`crates/idl-lower/tests/outside_profile.rs::native_and_an_object_reference_are_dropped_too`,
`crates/idl-lower/tests/outside_profile.rs::a_type_an_operation_names_is_kept`,
`crates/idl-lower/tests/outside_profile.rs::a_profile_that_admits_any_keeps_it`,
`crates/zerodds-build/src/lib.rs::tests::a_struct_holding_an_any_is_left_out_and_the_rest_of_the_unit_builds`,
`crates/ast-layout/tests/layout.rs::any_is_refused_at_planning_and_the_reason_names_the_construct`

**Status:** done — `Ast::drop_outside_profile(Profile::XTypes13)` entfernt jeden Datentyp, der `any`, `ValueBase`, `native` oder einen Objektverweis erreicht (Member, Fall, Alias, Collection-Element, Map-Schlüssel und -Wert, Basis, Diskriminator), und meldet ihn. Ein Typ, den eine Konstante, eine Operation, ein Parameter, ein Attribut, ein Annotationsmember oder ein Topic nennt, bleibt im Modell; für ihn weist der Layout-Planer das Konstrukt wie bisher mit Namen ab (`LayoutError::UnsupportedForEncoding`).

### §7.3.1.1.2 #pragma dds_xtopics begin und end

**Spec:** §7.3.1.1.2, S. 90 — "#pragma dds_xtopics begin [<version_number>] //
IDL definitions #pragma dds_xtopics end [<version_number>]".

**Repo:** `crates/idl-lower/src/lower.rs::dds_xtopics` (Z. 825)

**Tests:** `crates/idl-lower/tests/dialects.rs::the_clauses_own_example_is_read_without_a_word`,
`crates/idl-lower/tests/dialects.rs::a_dds_xtopics_region_is_kept_verbatim_and_reaches_no_declaration`,
`crates/idl-lower/tests/dialects.rs::a_dds_xtopics_that_is_neither_begin_nor_end_is_reported`

**Status:** done

### §7.3.1.1.2 Versionsnummer ohne Groß-/Kleinschreibung, Leerzeichen als Unterstrich

**Spec:** §7.3.1.1.2, S. 91 — "It shall be interpreted without respect to case,
and any spaces (for example, in '1.0 Beta 1') shall be replaced with
underscores."

**Repo:** `crates/idl-lower/src/lower.rs::xtopics_version` (Z. 4451)

**Tests:** `crates/idl-lower/tests/dialects.rs::the_clauses_own_example_is_read_without_a_word`

**Status:** done

### §7.3.1.1.2 Verschachtelung: innerste Version gilt, end-Version wie begin

**Spec:** §7.3.1.1.2, S. 91 — "In the event that such pragma declarations are
nested within one another, the innermost version number specified, if any,
shall be in effect. If version numbers are used with 'end' declarations, those
version numbers should be the same as those used with the matching 'begin'
declarations."

**Repo:** `crates/idl-lower/src/lower.rs::dds_xtopics` (Z. 825)

**Tests:** `crates/idl-lower/tests/dialects.rs::the_inner_end_closes_the_inner_begin`,
`crates/idl-lower/tests/dialects.rs::a_version_that_does_not_match_its_begin_is_reported`,
`crates/idl-lower/tests/dialects.rs::an_end_with_no_begin_is_reported`

**Status:** done

### §7.3.1.1.2 begin ohne end reicht bis zum Ende der Eingabe

**Spec:** §7.3.1.1.2, S. 91 — "In the event that such a pragma 'begin'
declaration is not matched with a subsequent closing 'end' declaration, the
'begin' declaration shall be considered to continue until the end of the IDL
input."

**Repo:** `crates/idl-lower/src/lower.rs::dds_xtopics` (Z. 825)

**Tests:** `crates/idl-lower/tests/dialects.rs::a_begin_with_no_end_runs_to_the_end_of_the_input`

**Status:** done

### §7.3.1.1.2 Verhalten des Compilers nicht festgelegt

**Spec:** §7.3.1.1.2, S. 91 — "The above declarations are informative only. The
behavior of an IDL compiler upon encountering them is unspecified".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Spec nennt die Deklarationen selbst
„informative only“.

### §7.3.1.2 Annotation Language

**Spec:** §7.3.1.2, S. 91 — "This specification makes use of different standard
annotation groups defined in [IDL]. It also proposes an alternative annotation
syntax for pre-existing IDL compilers."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §7.3.1.2.1 Anwendbarkeit auf konstruierte Typen, Bitmask-Konstanten, Literale, Member

**Spec:** §7.3.1.2.1, S. 92 — "This specification restricts the applicability of
annotations to constructed types, bitmask constants, enumerated type literals,
and members of aggregated types."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_annotations` (Z. 295)

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_outside_its_row_of_table_21_is_refused`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_on_its_row_of_table_21_is_accepted`

**Status:** done — die Annotationen erreichen Typen, Member, Literale und Bits; eine eingebaute Annotation an einer Stelle außerhalb ihrer Zeile von Table 21 wird abgewiesen (eigene Items unter §7.3.1.2.2, alle `done`).

### §7.3.1.2.1.1 Member IDs: @id oder @hashid, höchstens eine

**Spec:** §7.3.1.2.1.1, S. 92 — "To assign the member ID explicitly the member
shall have either the @id or the @hashid annotation. At most one of these two
annotations may appear on a member. Otherwise it shall be considered an
error."

**Repo:** `crates/idl-lower/src/annotation.rs::reject_two_member_ids` (Z. 312)

**Tests:** `crates/idl-lower/tests/annotations.rs::an_id_and_a_hashid_on_one_member_are_rejected`

**Status:** done

### §7.3.1.2.1.1 @id setzt die Member-ID auf den Wert

**Spec:** §7.3.1.2.1.1, S. 92 — "The @id annotation [...] shall assign the member
ID to the value specified in the annotation parameter."

**Repo:** `crates/idl-lower/src/annotation.rs::id_annotation` (Z. 582)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_declared_id_records_that_it_was_declared`,
`crates/idl-lower/tests/annotations.rs::an_id_that_is_not_an_integer_is_reported_rather_than_ignored`

**Status:** done — zum Wertebereich siehe §7.2.2.4.4.4.4.

### §7.3.1.2.1.1 @hashid: aus Membername oder Parameter, leerer Parameter heißt Membername

**Spec:** §7.3.1.2.1.1, S. 92 — "@annotation hashid { string value default \"\";
}; [...] If the annotation is used without any parameter or with the empty
string as a parameter, then the Member ID shall be computed from the member
name."

**Repo:** `crates/idl-lower/src/annotation.rs::hashid_annotation` (Z. 613)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_hashid_records_the_name_it_hashes`,
`crates/idl-lower/tests/annotations.rs::an_empty_hashid_argument_is_the_member_name`,
`crates/idl-lower/tests/completion.rs::a_hashid_uses_the_name_it_names_rather_than_the_members_own`

**Status:** done

### §7.3.1.2.1.1 Explizite ID vor automatischer

**Spec:** §7.3.1.2.1.1, S. 92 — "The explicit assignment of member IDs takes
precedence over the automatic mechanisms."

**Repo:** `crates/idl-lower/src/complete.rs::member_ids` (Z. 1051)

**Tests:** `crates/idl-lower/tests/completion.rs::a_declared_id_wins_over_the_aggregates_autoid`,
`crates/types/src/builder.rs::tests::explicit_id_wins_over_hashid`

**Status:** done

### §7.3.1.2.1.1 @autoid: SEQUENTIAL oder HASH, auch an enthaltenden Modulen

**Spec:** §7.3.1.2.1.1, S. 92 — "If there is no explicit member ID specified,
member IDs are automatically assigned either sequentially or as a hash of the
member name. The decision is controlled by the @autoid annotation [...]. If no
@autoid annotation has been specified for the type or any of the containing
modules, then the default shall be to use a sequential approach".

**Repo:** `crates/idl-lower/src/complete.rs::member_ids` (Z. 1051);
`crates/idl-lower/src/annotation.rs::autoid_annotation` (Z. 635)

**Tests:** `crates/idl-lower/tests/completion.rs::autoid_sequential_and_autoid_hash_differ`,
`crates/idl-lower/tests/annotations.rs::both_autoid_policies_are_distinguished`,
`crates/idl-lower/tests/annotations.rs::autoid_on_a_module_numbers_the_members_of_its_types`

**Status:** done — `@autoid` am Aggregat und an einem enthaltenden Modul wirkt (`ast::query::effective_autoid`); ohne Annotation wird sequentiell nummeriert.

### §7.3.1.2.1.1 @autoid an einem enthaltenden Modul wirkt

**Spec:** §7.3.1.2.1.1, S. 92 — "for the type or any of the containing modules".

**Repo:** `crates/ast/src/query.rs::effective_autoid`,
`crates/idl-lower/src/complete.rs::member_ids`

**Tests:** `crates/idl-lower/tests/annotations.rs::autoid_on_a_module_numbers_the_members_of_its_types`

**Status:** done — `member_ids` fragt `effective_autoid`, das die enthaltenden Module bis zur obersten Ebene befragt.

### §7.3.1.2.1.1 Sequentiell: vorige ID plus eins, erste ID 0

**Spec:** §7.3.1.2.1.1, S. 92–93 — "The value of the member ID assigned to a
member using the sequential approach shall equal to the member ID of the
previous member of the same Aggregated Type plus one [...] In the case of the
first member of Aggregated Type that does not have any base type the member ID
shall be set to zero."

**Repo:** `crates/idl-lower/src/complete.rs::next_id` (Z. 1144)

**Tests:** `crates/idl-lower/tests/completion.rs::members_without_annotations_number_from_zero_upwards`,
`crates/idl-lower/tests/completion.rs::a_declared_id_shifts_the_numbering_of_what_follows`

**Status:** done

### §7.3.1.2.1.1 Sequentiell: abgeleiteter Typ setzt nach dem letzten Basis-Member fort

**Spec:** §7.3.1.2.1.1, S. 92–93 — "In the case an aggregated type inherits from
another aggregated type the first member of the derived type gets the member ID
of the last member in the base type plus one." Example 1: ID(s1_m1)=0,
ID(s1_m2)=1, ID(s2_m1)=2, ID(s3_m1)=3.

**Repo:** `crates/idl-lower/src/complete.rs::last_base_member_id` (Z. 1036)

**Tests:** `crates/idl-lower/tests/completion.rs::example_1_of_the_member_id_clause_numbers_a_derived_chain_0_1_2_3`,
`crates/idl-lower/tests/validation.rs::a_declared_id_colliding_with_a_derived_one_is_rejected_and_names_both_origins`

**Status:** done — die erste ID des abgeleiteten Typs ist die letzte ID der Basiskette plus eins; Example 1 gibt 0, 1, 2, 3.

### §7.3.1.2.1.1 Example 1 ohne Test

**Spec:** §7.3.1.2.1.1, S. 93 — Example 1 mit drei Ebenen.

**Repo:** `crates/idl-lower/src/complete.rs::last_base_member_id`

**Tests:** `crates/idl-lower/tests/completion.rs::example_1_of_the_member_id_clause_numbers_a_derived_chain_0_1_2_3`

**Status:** done — der Test lässt `MyStruct1` bis `MyStruct3` mit `@autoid(SEQUENTIAL)` lowern und prüft ID(s1_m1)=0, ID(s1_m2)=1, ID(s2_m1)=2, ID(s3_m1)=3.

### §7.3.1.2.1.1 Hash-Berechnung: MD5, Little Endian, & 0x0FFFFFFF

**Spec:** §7.3.1.2.1.1, S. 94 — "1. Compute a 4-byte hash of the string as
specified in 7.2.2.4.4.4.5. 2. Interpret the resulting 4-byte has as a Little
Endian unsigned 32-bit integer. 3. Perform a bitwise AND operation with the
integer 0x0FFFFFFF to zero the most significant 4-bits of the integer."

**Repo:** `crates/idl-lower/src/complete.rs::hash_id` (Z. 1165);
`crates/types/src/type_object/common.rs::member_id_from_name` (Z. 53)

**Tests:** `crates/types/src/builder.rs::tests::member_id_from_name_matches_known_md5_vectors`,
`crates/idl-lower/src/complete.rs::tests::a_hashed_id_stays_inside_the_28_bit_space`

**Status:** done

### §7.3.1.2.1.1 Example 2: color und @hashid("shapesize")

**Spec:** §7.3.1.2.1.1, S. 94 — "ID(color) = 0x0FA5DD70, ID(size) = 0x047790DA."

**Repo:** `crates/idl-lower/src/complete.rs::hash_id` (Z. 1165)

**Tests:** `crates/idl-lower/tests/completion.rs::example_2_of_the_member_id_clause_gives_the_two_published_ids`,
`crates/types/src/builder.rs::tests::member_id_from_name_matches_known_md5_vectors`

**Status:** done — `@autoid(HASH)` gibt `color` die ID 0x0FA5DD70 und `@hashid("shapesize") long size` die ID 0x047790DA.

### §7.3.1.2.1.1 Example 2: shapesize ohne Test

**Spec:** §7.3.1.2.1.1, S. 94 — `ID(size) = 0x047790DA`.

**Repo:** `crates/idl-lower/src/complete.rs::hash_id`

**Tests:** `crates/idl-lower/tests/completion.rs::example_2_of_the_member_id_clause_gives_the_two_published_ids`

**Status:** done — der Test lowert `MyStruct1` des Beispiels und `hashed_member_id("shapesize")`; beide ergeben 0x047790DA.

### §7.3.1.2.1.2 Optional Members: @optional

**Spec:** §7.3.1.2.1.2, S. 94 — "To declare a member optional, users shall apply
the @optional annotation [...] It is an error to declare the same member as
both optional and as a key."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm
`optional` Z. 335); `crates/idl-lower/src/validate.rs::member_rules` (Z. 327)

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`,
`crates/idl-lower/tests/validation.rs::a_key_member_must_not_be_optional`,
`crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — `@optional` und `@optional(TRUE)` machen den Member optional, `@optional(FALSE)` nicht; `@key @optional` wird abgewiesen.

### §7.3.1.2.1.3 Key Members: @key

**Spec:** §7.3.1.2.1.3, S. 94 — "To declare a member as part of the key, users
shall apply the @key annotation [...] It is an error to declare the same member
as both optional and as a key."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm `key`
Z. 334); `crates/idl-lower/src/validate.rs::member_rules` (Z. 327)

**Tests:** `crates/idl-lower/tests/validation.rs::a_key_member_must_not_be_optional`,
`crates/idl-lower/tests/validation.rs::key_on_its_own_is_accepted`,
`crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — `@key` und `@key(TRUE)` setzen den Schlüssel, `@key(FALSE)` nicht.

### §7.3.1.2.1.4 External Data: Member immer vorhanden, Key zulässig

**Spec:** §7.3.1.2.1.4, S. 94 — "Unless also annotated as Optional, external
members shall always be present and therefore the pointer [...] to non-optional
external members cannot be NULL. Non-optional external members can be annotated
as Key."

**Repo:** `crates/lang-rules/tests/external_is_an_indirection.rs`

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_non_optional_external_member_gets_table_9_behind_the_indirection`,
`crates/lang-rules/tests/d_end_to_end.rs::an_optional_external_member_is_one_pointer_and_not_a_nullable_pointer`

**Status:** done — ein nicht-optionaler `@external`-Member ist im erzeugten
Code eine nicht-leere Indirektion. Ein Test mit `@key @external` fehlt;
die Prüfregeln weisen die Kombination nicht ab.

### §7.3.1.2.1.4 External: Graphstruktur nicht erhalten, Diamond, Zyklus

**Spec:** §7.3.1.2.1.4, S. 95 — "If a conforming implementation encounters a
graph [...], it is not required to maintain the graph structure through
serialization/deserialization." Non-normative note zu Baum, Diamond, Zyklus.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung an eine
Implementierung; der Hinweis ist „non-normative“.

### §7.3.1.2.1.4 External: Syntax am Member, auch als Suffix

**Spec:** §7.3.1.2.1.4, S. 95 — "@external long my_aggregation_member; or: long
my_aggregation_member; //@external".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm
`external` Z. 336); `crates/idl-parse/src/suffix.rs`

**Tests:** `crates/idl-lower/tests/validation.rs::external_on_a_member_and_on_a_branch_is_accepted`,
`crates/idl-lower/tests/annotations.rs::the_prefix_and_the_suffix_form_produce_the_same_model`

**Status:** done

### §7.3.1.2.1.4 External: Syntax für Elemente von Sequence, Array, Map

**Spec:** §7.3.1.2.1.4, S. 95 — "sequence<@external Foo, 42> sequence_of_foo;
[...] Foo array_of_foo @external [42]; [...] map<string, @external Foo, 42>
map_of_string_to_foo;"

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::external_on_an_element_a_map_key_or_a_map_value_is_carried`,
`crates/idl-lower/tests/annotations.rs::external_after_an_array_declarators_name_is_the_elements`,
`crates/idl-lower/tests/annotations.rs::an_annotation_after_the_name_of_a_declarator_with_no_dimensions_is_reported`

**Status:** done — `@external` an einem Element einer Sequence, an einem Map-Schlüssel und Map-Wert und die Array-Form `Foo a @external [42]` werden gelesen; die Annotation nach dem Namen gilt für die Elemente des Arrays, ein Deklarator ohne Dimensionen meldet sie.

### §7.3.1.2.1.5 Enumerated Literal Values: @bit_bound 1 bis 32 an Aufzählungen

**Spec:** §7.3.1.2.1.5, S. 95 — "the value member of the annotation may take any
value from 1 to 32, inclusive, when this annotation is applied to an
enumerated type."

**Repo:** `crates/idl-lower/src/annotation.rs::bit_bound_annotation` (Z. 710);
`crates/ast/src/query.rs::effective_bit_bound` (Z. 319)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_bit_bound_reaches_the_type`,
`crates/idl-lower/tests/completion.rs::an_enum_without_a_bit_bound_is_thirty_two_bits`,
`crates/idl-lower/tests/annotations.rs::an_enumerations_bit_bound_is_from_1_to_32`

**Status:** done — `@bit_bound` erreicht die Aufzählung, der Default ist 32; ein Wert außerhalb von 1 bis 32 an einer Aufzählung wird abgewiesen (`complete.rs::check_enumeration_bound`).

### §7.3.1.2.1.5 @bit_bound an Aufzählung außerhalb 1 bis 32 wird abgewiesen

**Spec:** §7.3.1.2.1.5, S. 95 — Wertebereich 1 bis 32.

**Repo:** `crates/idl-lower/src/complete.rs::check_enumeration_bound`

**Tests:** `crates/idl-lower/tests/annotations.rs::an_enumerations_bit_bound_is_from_1_to_32`

**Status:** done — `@bit_bound(0)` und `@bit_bound(48)` an einem `enum` werden abgewiesen.

### §7.3.1.2.1.5 @value und implizite Fortzählung

**Spec:** §7.3.1.2.1.5, S. 95 — "this specification allows enumerated literals
to be given explicit custom values [...] by means of the @value annotation [...]
implicit values are assigned in a progression starting from the most-recently
specified value (or an implicit value of zero for the first literal, if there
is no previous specified value) and adding one with each successive literal."

**Repo:** `crates/idl-lower/src/annotation.rs::value_annotation` (Z. 660);
`crates/ast/src/query.rs::effective_enumerator_value` (Z. 559)

**Tests:** `crates/idl-lower/tests/completion.rs::enumerators_without_values_run_from_zero_upwards`,
`crates/idl-lower/tests/completion.rs::an_explicit_enumerator_value_shifts_the_ones_after_it`,
`crates/idl-lower/tests/annotations.rs::an_explicit_enumerator_value_keeps_the_expression_it_was_written_as`

**Status:** done

### §7.3.1.2.1.6 Bitmask Positions: Größe 32, @bit_bound bis 64, @position

**Spec:** §7.3.1.2.1.6, S. 96 — "By default, the size of a bit mask is 32-bit.
This behavior may be amended with the use of the @bit_bound annotation, which
may set the size of the whole bit mask to a value lower or equal to 64 [...] a
bit value may be set explicitly by means of the @position annotation".

**Repo:** `crates/idl-lower/src/complete.rs::check_bitmask_bounds` (Z. 176);
`crates/idl-lower/src/annotation.rs::value_annotation` (Z. 660, Arm `BitValue`)

**Tests:** `crates/idl-lower/tests/annotations.rs::an_explicit_bit_position_reaches_the_bit`,
`crates/idl-lower/tests/validation.rs::a_bit_bound_over_sixty_four_is_refused`,
`crates/idl-lower/tests/completion.rs::bit_positions_run_from_zero_and_an_explicit_one_shifts_the_rest`

**Status:** done

### §7.3.1.2.1.7 Nested Types: Default nicht nested, @nested

**Spec:** §7.3.1.2.1.7, S. 96 — "By default, aggregated types and aliases to
aggregated types defined in IDL are not considered to be nested types. This
designation may be changed by applying the IDL @nested annotation to a type
definition".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm `nested`
Z. 339); `crates/ast/src/query.rs::effective_nested` (Z. 484)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_nested_annotation_reaches_the_type`,
`crates/idl-lower/tests/annotations.rs::nested_false_and_topic_override_the_default_nested_of_the_module`

**Status:** done — `@nested`, `@nested(TRUE)` und `@nested(FALSE)` werden unterschieden: das Modell trägt `nested_explicit` neben `is_nested`, ein eigenes `false` hat Vorrang vor dem Default des Moduls.

### §7.3.1.2.1.7 @nested(FALSE) in einem Modul mit @default_nested(TRUE) gilt

**Spec:** §7.3.1.2.1.7, S. 96 — `@nested` an der Typdefinition ändert die
Bestimmung.

**Repo:** `crates/ast/src/query.rs::effective_nested`

**Tests:** `crates/idl-lower/tests/annotations.rs::nested_false_and_topic_override_the_default_nested_of_the_module`

**Status:** done — `effective_nested` gibt einem eigenen `false` den Vorrang vor `@default_nested(TRUE)` des Moduls.

### §7.3.1.2.1.7 @default_nested an Modulen, vom enthaltenden Modul geerbt, Default FALSE

**Spec:** §7.3.1.2.1.7, S. 96 — "@annotation default_nested { boolean value
default TRUE; }; The @default_nested annotation may be applied to modules. If
not present on a module, the value defaults to that of the enclosing module. If
a top-level module is not annotated, the default is FALSE."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm
`default_nested` Z. 347); `crates/ast/src/query.rs::effective_nested` (Z. 484)

**Tests:** `crates/idl-lower/tests/annotations.rs::default_nested_on_a_module_reaches_the_types_in_it`

**Status:** done

### §7.3.1.2.1.7 Compiler-Option für den Default nicht annotierter Module auf oberster Ebene gilt

**Spec:** §7.3.1.2.1.7, S. 96 — "In addition to the above annotations, IDL
compilers shall provide the means to change the default value for
non-annotated top-level modules."

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::the_default_for_unannotated_top_level_modules_is_an_option`

**Status:** done — `Options::default_nested` (Compiler-Option) setzt den Default für nicht annotierte Module auf oberster Ebene.

### §7.3.1.2.1.8 @extensibility an Aggregaten

**Spec:** §7.3.1.2.1.8, S. 96 — "The extensibility kind of a type may be defined
by means of a @extensibility annotation [...] This annotation may be applied
to the definitions of aggregated types."

**Repo:** `crates/idl-lower/src/annotation.rs::extensibility_argument` (Z. 879)

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/idl-lower/tests/annotations.rs::an_extensibility_argument_that_is_not_one_of_the_three_is_reported`

**Status:** done — zur Anwendung auf andere Typarten siehe Table 21.

### §7.3.1.2.1.8 Mehrfaches @extensibility ist ein Fehler und wird gemeldet

**Spec:** §7.3.1.2.1.8, S. 96 — "It shall be considered an error for it to be
applied to the same type multiple times."

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::the_extensibility_is_given_once`

**Status:** done — Ein zweites `@extensibility`, `@final`, `@appendable` oder `@mutable` an einem Typ wird als Fehler gemeldet.

### §7.3.1.2.1.8 Ohne Angabe appendable, überschreibbar

**Spec:** §7.3.1.2.1.8, S. 96 — "In the event that the representation of a given
type does not indicate the type's extensibility kind, the type shall be
considered appendable. Implementations may provide a mechanism to override this
default behavior".

**Repo:** `crates/ast/src/lib.rs::DEFAULT_EXTENSIBILITY` (Z. 62);
`crates/idl-lower/src/lower.rs::Options` (Z. 50, Feld `default_extensibility`
Z. 73); `crates/ast-layout/src/key.rs::default_extensibility` (Z. 5640)

**Tests:** `crates/idl-lower/tests/completion.rs::a_type_that_said_nothing_is_appendable_by_rule_and_silent_in_the_facet`,
`crates/idl-lower/tests/completion.rs::the_callers_default_extensibility_does_not_override_a_bases`

**Status:** done — der Default ist APPENDABLE; das Dialekt `Cyclone` des
Layout-Planers nimmt FINAL. Auf der Leitung ist nicht erkennbar, welcher
Default galt (`docs/omg-xtypes-issues.md` §B6).

### §7.3.1.2.1.8 Kurzformen @final, @appendable, @mutable

**Spec:** §7.3.1.2.1.8, S. 96 — "IDL compilers shall also implement the shortcut
annotations for the different extensibility kinds. That is, @final,
@appendable, and @mutable".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arme Z. 353–355)

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/idl-lower/tests/annotations.rs::every_spelling_of_one_builtin_says_the_same_thing`

**Status:** done

### §7.3.1.2.1.9 @must_understand

**Spec:** §7.3.1.2.1.9, S. 97 — "If the @must_understand annotation is set to
true in particular member M2 of a type T2, then the assignment to an object of
type T1 shall fail if the type T1 does not define such a member."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm
`must_understand` Z. 337); `crates/types/src/assignability.rs::check_mutable_structs`
(Z. 1474)

**Tests:** `crates/types/src/assignability.rs::tests::a_mutable_writer_member_the_reader_lacks_and_must_understand_is_no`,
`crates/wire-conformance/tests/bytes.rs::an_unknown_mutable_member_is_skipped_and_a_must_understand_one_is_not`,
`crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — wirkt in Zuweisbarkeit und beim Lesen; `@must_understand(FALSE)` setzt das Attribut nicht.

### §7.3.1.2.1.10 @default_literal

**Spec:** §7.3.1.2.1.10, S. 97 — "The annotation @default_literal allows this
behavior to be changed. @annotation default_literal {};"

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm
`default_literal` Z. 351); `crates/types/src/from_ast.rs::enumeration` (Z. 2361)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_default_literal_marks_the_enumerator_and_not_the_enum`,
`crates/lang-rules/src/default_python.rs::tests::an_enumeration_with_a_default_literal_takes_that_one`

**Status:** done

### §7.3.1.2.1.11 @ignore_literal_names: fehlend FALSE, ohne Parameter TRUE

**Spec:** §7.3.1.2.1.11, S. 97 — "@annotation ignore_literal_names { boolean
value default TRUE; }; If the @ignore_literal_names annotation is not present it
is interpreted as having the value FALSE. If it is present without a parameter,
it is interpreted as having the value TRUE."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm
`ignore_literal_names` Z. 352)

**Tests:** `crates/types/tests/table19_key_members.rs::ignore_literal_names_needs_the_annotation_on_both_types`,
`crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — fehlend ist FALSE, ohne Parameter und mit TRUE ist es TRUE, `@ignore_literal_names(FALSE)` ist FALSE.

### §7.3.1.2.1.11 @ignore_literal_names: TRUE auf beiden Seiten

**Spec:** §7.3.1.2.1.11, S. 97 — "If the @ignore_literal_names annotation has the
value TRUE in both enumerated types, then the literal names are ignored for the
purpose of checking the assignability of the two Enumerated Types."

**Repo:** `crates/types/src/assignability.rs::check_enumerations` (Z. 964,
Z. 1005–1007); `crates/types/src/from_ast.rs::enumeration` (Z. 2361)

**Tests:** `crates/types/tests/table19_key_members.rs::ignore_literal_names_needs_the_annotation_on_both_types`,
`crates/types/src/assignability.rs::tests::one_sided_ignore_literal_names_does_not_drop_the_name_rule`

**Status:** done — die Einleitung des Abschnitts sagt, Table 18 verlange
gleiche Literale für den Default; Table 18 sagt das nicht (Widerspruch der Spec, in
der Befunddatei geführt).

### §7.3.1.2.1.12 @try_construct: Deklaration und Default USE_DEFAULT

**Spec:** §7.3.1.2.1.12, S. 98–99 — "enum TryConstructFailAction { DISCARD,
USE_DEFAULT, TRIM }; @annotation try_construct { TryConstructFailAction value
default USE_DEFAULT; }; [...] if the @try_construct annotation is not used it is
the same as if it had been explicitly set to DISCARD [...] If the annotation is
specified without a value [...] then the behavior is set to DEFAULT".

**Repo:** `crates/idl-lower/src/annotation.rs::try_construct_behaviour` (Z. 746);
`crates/ast/src/facet/xtypes.rs::BARE` (Z. 99)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_bare_try_construct_is_use_default_and_an_absent_one_is_discard`,
`crates/idl-lower/tests/annotations.rs::each_try_construct_behaviour_is_distinguished`,
`crates/wire-probe/tests/try_construct.rs::a_bare_annotation_is_use_default_and_not_discard`

**Status:** done

### §7.3.1.2.1.12 @try_construct: zulässige Positionen

**Spec:** §7.3.1.2.1.12, S. 99 — "the TryConstruct annotation may be used in
structure and union members, the union discriminator, the elements of arrays
and sequences, and the key and/or values of map types."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_use_site` (Z. 775);
`crates/ast/src/facet/xtypes.rs::XTypesFacet` (Z. 242, Felder `try_construct`,
`element_try_construct`, `key_try_construct`, `discriminator_try_construct`)

**Tests:** `crates/idl-lower/tests/annotations.rs::try_construct_reaches_every_position_the_clause_names`

**Status:** done — zusätzlich nimmt das Front-End `@try_construct` an einem
`typedef` an; eine Verwendung ohne eigene Angabe erbt es
(`an_alias_supplies_a_default_that_the_use_site_overrides`). Table 21 nennt
nur Struktur- und Union-Member (Widerspruch der Spec,
`docs/omg-xtypes-issues.md` §B4).

### §7.3.1.2.1.12.1 TryConstruct Example 1

**Spec:** §7.3.1.2.1.12.1, S. 99–100 — `@try_construct(USE_DEFAULT) string<5>`
gegen „Hello World!“: `O1.member` ist der leere String.

**Repo:** `crates/wire/src/try_construct.rs::fit_string` (Z. 89)

**Tests:** `crates/wire-probe/tests/try_construct.rs::use_default_substitutes_table_nines_value_and_keeps_the_sample`

**Status:** done — das Beispiel nennt den Typ `String4 (string<5>)`
(Benennungsfehler der Spec, in der Befunddatei geführt).

### §7.3.1.2.1.12.2 TryConstruct Example 2

**Spec:** §7.3.1.2.1.12.2, S. 100 — `@try_construct(TRIM) string<5>` gegen
„Hello World!“: „Hello“.

**Repo:** `crates/wire/src/try_construct.rs::fit_string` (Z. 89)

**Tests:** `crates/wire-probe/tests/try_construct.rs::trim_cuts_the_string_to_the_bound_and_keeps_the_sample`

**Status:** done

### §7.3.1.2.1.12.3 TryConstruct Example 3

**Spec:** §7.3.1.2.1.12.3, S. 100 — `@try_construct(TRIM) sequence<long,4>`
gegen acht Elemente: die ersten vier.

**Repo:** `crates/wire/src/try_construct.rs::fit_sequence` (Z. 131)

**Tests:** `crates/wire-probe/tests/try_construct.rs::trim_cuts_a_sequence_to_its_bound_keeping_the_first_elements`

**Status:** done

### §7.3.1.2.1.12.4 TryConstruct Example 4

**Spec:** §7.3.1.2.1.12.4, S. 101 — `sequence<@try_construct(TRIM) String5, 10>`:
das Element wird gekürzt, die Sequenz bleibt.

**Repo:** `crates/ast/src/query.rs::element_try_construct` (Z. 413)

**Tests:** `crates/wire-probe/tests/try_construct.rs::an_element_annotation_trims_the_element_and_not_the_collection`

**Status:** done

### §7.3.1.2.1.12.5 TryConstruct Example 5

**Spec:** §7.3.1.2.1.12.5, S. 101–102 — `union T1 switch (@try_construct
T1Enum)`: ein Diskriminator ENUM3 wird zum Default ENUM2.

**Repo:** `crates/ast/src/query.rs::discriminator_try_construct` (Z. 508)

**Tests:** `crates/wire-probe/tests/try_construct.rs::a_defaulted_discriminator_defaults_the_member_it_now_selects`

**Status:** done — das Beispiel endet mit „O1.e1_value would be constructed
from O2.e3_value“, obwohl ENUM2 `e2_value` wählt (Fehler der Spec, in der
Befunddatei geführt).

### §7.3.1.2.1.13 @verbatim

**Spec:** §7.3.1.2.1.13, S. 102 — "Verbatim Text objects associated with a
constructed type declaration shall be indicated using the following @verbatim
annotation defined in Sub Clause 8.3.5.1 of [IDL]."

**Repo:** `crates/idl-lower/src/annotation.rs::verbatim_annotation` (Z. 934)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_verbatim_directive_keeps_its_language_placement_and_text`,
`crates/idl-lower/tests/annotations.rs::a_verbatim_directive_defaults_language_and_placement`

**Status:** done — die Placement-Werte siehe §7.2.2.4.8.2.

### §7.3.1.2.1.14 @non_serialized

**Spec:** §7.3.1.2.1.14, S. 103 — "@annotation non_serialized { boolean value
default TRUE; }; It is an error to declare the same member as both
non_serialized and as a key."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330, Arm
`non_serialized` Z. 338); `crates/idl-lower/src/validate.rs::member_rules`
(Z. 327, Prüfung Z. 357)

**Tests:** `crates/idl-lower/tests/validation.rs::a_key_member_must_not_be_non_serialized`,
`crates/types/tests/table19_key_members.rs::a_non_serialized_member_does_not_enter_the_comparison`,
`crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — die Annotation und die Abweisung zusammen mit `@key`; `@non_serialized(FALSE)` setzt das Attribut nicht.

### §7.3.1.2.1.15 @data_representation: Bitmask XCDR1, XML, XCDR2

**Spec:** §7.3.1.2.1.15, S. 103 — "@bit_bound(32) bitmask DataRepresentationMask
{ @position(0) XCDR1, @position(1) XML, @posiiton(2) XCDR2 } @annotation
data_representation { DataRepresentationMask allowed_kinds; };"

**Repo:** `crates/idl-lower/src/annotation.rs::data_representation_annotation`
(Z. 998); `crates/ast/src/facet/xtypes.rs::DataRepresentationMask` (Z. 428)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_data_representation_annotation_is_a_mask_not_one_value`,
`crates/idl-lower/tests/annotations.rs::an_unknown_data_representation_is_reported`

**Status:** done — der Code nimmt `XCDR1` und `XCDR`; das Beispiel der Spec
schreibt `XCDR | XCDR2`, die Bitmask heißt `XCDR1`, und `@posiiton` ist ein
Tippfehler (Fehler der Spec, in der Befunddatei geführt).

### §7.3.1.2.1.15 @data_representation: fehlend heißt 0xffffffff

**Spec:** §7.3.1.2.1.15, S. 104 — "If the @data_representation annotation is not
present it is interpreted as if the DataRepresentationMask value was set to
0xffffffff."

**Repo:** `crates/ast/src/query.rs::data_representation_mask`;
`crates/ast/src/facet/xtypes.rs::DataRepresentationMask`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_type_without_data_representation_has_the_mask_0xffffffff`,
`crates/idl-lower/tests/annotations.rs::a_data_representation_annotation_is_a_mask_not_one_value`

**Status:** done — `Ast::data_representation_mask` gibt einem Typ ohne eigene und ohne Modulmaske `DataRepresentationMask::ALL` (`ALL_BITS` = 0xffffffff); eine angegebene Maske bleibt, was sie sagt (XCDR2 = Bit 2, XCDR1 = Bit 0, XML = Bit 1).

### §7.3.1.2.1.15 @data_representation an Typ- und Moduldeklarationen

**Spec:** §7.3.1.2.1.15, S. 104 — "This annotation may be applied to type
declarations and module declarations."

**Repo:** `crates/idl-lower/src/annotation.rs::data_representation_annotation`,
`crates/ast/src/query.rs::effective_data_representation`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_data_representation_annotation_is_a_mask_not_one_value`,
`crates/idl-lower/tests/annotations.rs::data_representation_on_a_module_is_the_default_of_its_types`

**Status:** done — Die Maske erreicht Typen und Module; ein Typ ohne eigene Maske erbt die des Moduls (`ast::query::effective_data_representation`).

### §7.3.1.2.1.15 @data_representation an einem Modul wirkt auf seine Typen

**Spec:** §7.3.1.2.1.15, S. 104 — Anwendung auf Moduldeklarationen.

**Repo:** `crates/ast/src/query.rs::effective_data_representation`

**Tests:** `crates/idl-lower/tests/annotations.rs::data_representation_on_a_module_is_the_default_of_its_types`

**Status:** done — `effective_data_representation` liest die Facette der enthaltenden Module; das Modul wirkt auf die Typen darin.

### §7.3.1.2.1.16 @topic: name, platform

**Spec:** §7.3.1.2.1.16, S. 104 — "@annotation topic { string name default \"\";
string platform default \"*\"; }; [...] The @topic annotation overrides the
@nested and @default_nested annotations."

**Repo:** `crates/idl-lower/src/annotation.rs::topic_annotation` (Z. 979);
`crates/ast/src/facet/xtypes.rs::TopicAnnotation` (Z. 416)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_topic_annotation_keeps_both_of_its_parameters`,
`crates/idl-lower/tests/annotations.rs::nested_false_and_topic_override_the_default_nested_of_the_module`

**Status:** done — die Parameter werden übernommen und `@topic` überstimmt `@nested` und `@default_nested`: ein Topic-Typ ist nie nested.

### §7.3.1.2.1.16 @topic hebt @nested und @default_nested auf

**Spec:** §7.3.1.2.1.16, S. 104 — "The @topic annotation overrides the @nested
and @default_nested annotations."

**Repo:** `crates/ast/src/query.rs::effective_nested`

**Tests:** `crates/idl-lower/tests/annotations.rs::nested_false_and_topic_override_the_default_nested_of_the_module`

**Status:** done — `@topic` überstimmt `@nested` und `@default_nested`: ein Topic-Typ ist nie nested (`effective_nested`).

### §7.3.1.2.1.16 @topic nur an zulässigen Topic-Typen wird geprüft

**Spec:** §7.3.1.2.1.16, S. 104 — "The @topic annotation can only be used on
types that are legal as DDS Topic types, see 7.6.2."

**Repo:** `crates/idl-lower/src/validate.rs::topic_is_on_a_topic_type`

**Tests:** `crates/idl-lower/tests/annotations.rs::topic_on_a_type_that_is_no_topic_type_is_refused`

**Status:** done — `validate.rs::topic_is_on_a_topic_type` weist `@topic` an einem Typ ab, der kein Topic-Typ ist (Aufzählung, Bitmask, Bitset, Alias, Sequenz, Map).

---

## §7.3.1.2.2 Using Built-in Annotations (Table 21)

### §7.3.1.2.2 Table 21 — @optional, @must_understand, @non_serialized: Strukturmember

**Spec:** §7.3.1.2.2, Table 21, S. 104 — "@optional, @must_understand,
@non_serialized | Structure Members".

**Repo:** `crates/idl-lower/src/annotation.rs::place_of`

**Tests:** `crates/idl-lower/tests/validation.rs::no_union_member_is_optional`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_outside_its_row_of_table_21_is_refused`,
`crates/idl-lower/tests/annotations.rs::must_understand_on_a_union_branch_is_refused`

**Status:** done — `@optional`, `@must_understand` und `@non_serialized` gelten für Strukturmember (Table 21); an einem Union-Member oder einer Typdeklaration weist das Front-End sie ab.

### §7.3.1.2.2 Table 21 — Ort der Annotationen wird geprüft

**Spec:** §7.3.1.2.2, S. 104 — "The application of the annotations listed above
is restricted to the type elements specified in Table 21."

**Repo:** `crates/idl-lower/src/annotation.rs::place_of`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_builtin_annotation_outside_its_row_of_table_21_is_refused`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_on_its_row_of_table_21_is_accepted`

**Status:** done — `annotation.rs::place_of` ordnet jede eingebaute Annotation den Orten von Table 21 zu; an einem anderen Ort ist sie ein Fehler (`@bit_bound` an einer Struktur, `@position` an einem Member, `@autoid` an einer Aufzählung).

### §7.3.1.2.2 Boolesche Parameter von @key, @optional, @external, @must_understand, @non_serialized, @ignore_literal_names werden gelesen

**Spec:** §7.3.1.2.1.11, S. 97, und §7.3.1.2.1.14, S. 103 — die Annotationen
haben `boolean value default TRUE`; IDL 4.2 §8.3 erklärt `@key`, `@optional`,
`@external` und `@must_understand` ebenso.

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::the_boolean_parameter_of_a_builtin_annotation_is_read`

**Status:** done — Die Parameter von `@key`, `@optional`, `@external`, `@must_understand`, `@non_serialized` und `@ignore_literal_names` werden gelesen: `@key(FALSE)` erzeugt keinen Key-Member. Ein Argument, das sich nicht auswerten lässt oder keinen Wert hat, ist ein Fehler.

### §7.3.1.2.2 Table 21 — @id, @hashid, @external, @try_construct: Struktur- und Union-Member

**Spec:** §7.3.1.2.2, Table 21, S. 104 — "@id, @hashid, @external,
@try_construct | Structure Members, Union members (except union
discriminator)".

**Repo:** `crates/idl-lower/src/validate.rs::external_is_a_member_annotation`
(Z. 134); `crates/idl-lower/src/annotation.rs::apply_use_site` (Z. 775)

**Tests:** `crates/idl-lower/tests/validation.rs::external_on_a_type_declaration_is_refused_by_name`,
`crates/idl-lower/tests/validation.rs::external_on_a_member_and_on_a_branch_is_accepted`,
`crates/idl-lower/tests/annotations.rs::try_construct_reaches_every_position_the_clause_names`

**Status:** done — `@try_construct` ist zusätzlich am Diskriminator und an
Collection-Elementen zugelassen, wie §7.3.1.2.1.12 es verlangt; Table 21 und
§7.3.1.2.1.12 widersprechen sich (`docs/omg-xtypes-issues.md` §B4).

### §7.3.1.2.2 Table 21 — @key: Strukturmember, Union-Diskriminator

**Spec:** §7.3.1.2.2, Table 21, S. 105 — "@key | Structure Members, Union
discriminator".

**Repo:** `crates/idl-lower/src/validate.rs::union_member_rules` (Z. 768);
`crates/idl-lower/src/annotation.rs::apply_use_site` (Z. 775)

**Tests:** `crates/idl-lower/tests/validation.rs::only_a_unions_discriminator_may_be_a_key`,
`crates/idl-lower/tests/annotations.rs::a_key_on_a_union_discriminator_is_carried_by_default`,
`crates/wire-probe/tests/interop_corpus.rs::every_case_produces_the_bytes_its_readme_records_in_all_four_modes` (Fall `key-on-union-discriminator`)

**Status:** done — `@key` gilt für Strukturmember und den Union-Diskriminator; am Diskriminator wird es im Default getragen (`KeyOnUnionDiscriminator::Carried`), `Refused` bleibt als Einstellung. Rust-Spalte des Corpus-Falls `key-on-union-discriminator`; Herstellerspalten nicht gemessen bis zur Aufzeichnung.

### §7.3.1.2.2 Table 21 — @bit_bound: Aufzählung, Bitmask

**Spec:** §7.3.1.2.2, Table 21, S. 105 — "@bit_bound | Enumerated Types, Bit Mask
Types".

**Repo:** `crates/idl-lower/src/annotation.rs::bit_bound_annotation` (Z. 710)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_bit_bound_reaches_the_type`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_outside_its_row_of_table_21_is_refused`

**Status:** done — `@bit_bound` an Aufzählung und Bitmask; an einem anderen Ort weist das Front-End es ab. Erweiterung des Projekts: zusätzlich an Alias und Bitset (`@bit_bound(40) typedef`).

### §7.3.1.2.2 Table 21 — @extensibility, @mutable, @appendable, @final, @nested: Typdeklarationen

**Spec:** §7.3.1.2.2, Table 21, S. 105 — "@extensibility, @mutable, @appendable,
@final, @nested | Type declarations".

**Repo:** `crates/idl-lower/src/annotation.rs::set_extensibility` (Z. 1036)

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/idl-lower/tests/annotations.rs::a_nested_annotation_reaches_the_type`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_outside_its_row_of_table_21_is_refused`

**Status:** done — `@extensibility`, `@mutable`, `@appendable`, `@final` und `@nested` wirken an Typdeklarationen; an Membern weist das Front-End sie ab.

### §7.3.1.2.2 Table 21 — @default_literal, @value: Literale

**Spec:** §7.3.1.2.2, Table 21, S. 105 — "@default_literal, @value | Enumerated
Literals".

**Repo:** `crates/idl-lower/src/annotation.rs::value_annotation` (Z. 660)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_default_literal_marks_the_enumerator_and_not_the_enum`,
`crates/idl-lower/tests/annotations.rs::an_explicit_enumerator_value_keeps_the_expression_it_was_written_as`

**Status:** done — `@value` an einem anderen Ziel als Literal oder Bit wird
gemeldet (`value_annotation`, letzter Arm); `@default_literal` an anderen
Orten nicht (offen oben).

### §7.3.1.2.2 Table 21 — @position: Bitmask-Werte

**Spec:** §7.3.1.2.2, Table 21, S. 105 — "@position | Bitmask Values".

**Repo:** `crates/idl-lower/src/annotation.rs::value_annotation` (Z. 660, Arm
`BitValue`)

**Tests:** `crates/idl-lower/tests/annotations.rs::an_explicit_bit_position_reaches_the_bit`

**Status:** done — an einem anderen Ziel gemeldet.

### §7.3.1.2.2 Table 21 — @autoid: Modul, Struktur, Union

**Spec:** §7.3.1.2.2, Table 21, S. 105 — "@autoid | Module declarations,
Structure declarations, Union declarations".

**Repo:** `crates/idl-lower/src/annotation.rs::autoid_annotation` (Z. 635)

**Tests:** `crates/idl-lower/tests/annotations.rs::both_autoid_policies_are_distinguished`,
`crates/idl-lower/tests/annotations.rs::autoid_on_a_module_numbers_the_members_of_its_types`

**Status:** done — `@autoid` wirkt an Struktur, Union und Modul.

### §7.3.1.2.2 Table 21 — @verbatim: alle Elemente

**Spec:** §7.3.1.2.2, Table 21, S. 105 — "@verbatim | All elements".

**Repo:** `crates/idl-lower/src/annotation.rs::verbatim_annotation` (Z. 934);
`crates/lang-rules/src/verbatim.rs::declaration_slots` (Z. 129)

**Tests:** `crates/idl-lower/tests/annotations.rs::a_verbatim_directive_on_a_member_reaches_the_member`,
`crates/lang-rules/src/verbatim.rs::tests::a_member_directive_is_hoisted_into_the_aggregate_and_taken_once`

**Status:** done

### §7.3.1.2.2 Table 21 — fehlende Zeilen

**Spec:** §7.3.1.2.2, Table 21, S. 104–105 — die Tabelle nennt
`@default_nested`, `@ignore_literal_names`, `@data_representation`, `@topic`,
`@unit`, `@range`, `@min`, `@max` und `@default` nicht.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Lücke der Spec (DDSXTY14-50 für `@range`,
`@min`, `@max`; in der Befunddatei geführt); keine Anforderung an eine
Implementierung.

### §7.3.1.2.3 Alternative Annotation Syntax: //@ als Suffix

**Spec:** §7.3.1.2.3, S. 105 — "it is legal to follow the declaration with a
single-line comment containing the annotation string. To distinguish such
comments from regular comments, there must be no space in between the double
slash ('//') and the at-sign ('@')."

**Repo:** `crates/idl-parse/src/suffix.rs`; `crates/idl-parse/src/preprocess/mod.rs`

**Tests:** `crates/idl-parse/tests/parse.rs::a_suffix_annotation_reaches_the_tree_as_an_annotation`,
`crates/idl-parse/tests/parse.rs::only_the_exact_slash_slash_at_marker_is_the_suffix_form`,
`crates/idl-lower/tests/annotations.rs::the_prefix_and_the_suffix_form_produce_the_same_model`

**Status:** done

### §7.3.1.2.3 Alternative Syntax: mehrere Annotationen

**Spec:** §7.3.1.2.3, S. 105 — "If multiple annotations are to be applied to the
same element, the at-sign of each shall be preceded by a double slash and no
white space."

**Repo:** `crates/idl-parse/src/suffix.rs`

**Tests:** `crates/idl-lower/tests/annotations.rs::several_suffix_annotations_apply_to_the_same_element`,
`crates/idl-lower/tests/annotations.rs::two_suffix_annotations_on_one_line_both_apply`

**Status:** done

### §7.3.1.2.4 Defining Annotations: @annotation

**Spec:** §7.3.1.2.4, S. 106 — "An annotation type is defined using the new token
@annotation [...] Annotation identifiers are orthogonal to any other kind of
type and therefore do not conflict with other types that may use the same
identifier name even when defined in the same module."

**Repo:** `crates/idl-lower/src/lower.rs` (Annotationsdeklaration, um Z. 2600);
`crates/ast/src/node/annotation.rs`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_user_declared_annotation_is_kept_whole`,
`crates/types/tests/type_object_annotations.rs::an_annotation_declaration_has_a_type_object`

**Status:** done — eine Annotationsdeklaration ist ein `AnnotationDefNode`
und kein `TypeNode`; ein Typ gleichen Namens kollidiert nicht.

### §7.3.1.2.4 Table 22 — Deklaration als struct mit //@annotation wird gelesen

**Spec:** §7.3.1.2.4, Table 22, S. 106 — "struct <ann_identifier> '{'
<ann_members> '};' //@annotation | The 'struct' <ann_identifier> is actually an
annotation type".

**Repo:** `crates/idl-lower/src/lower.rs::annotation_from_struct`

**Tests:** `crates/idl-lower/tests/annotations.rs::table_22s_declaration_form_declares_an_annotation_type`

**Status:** done — Die Form `struct … //@annotation` (Table 22) deklariert einen Annotationstyp (`lower.rs::annotation_from_struct`).

### §7.3.1.2.4 Table 23 — Annotationsmember mit default

**Spec:** §7.3.1.2.4, Table 23, S. 106–107 — "[ <pre_annotations>] <member_type>
<member_name> [ default <member_value> ]; [ <post_annotations> ]" und
"The specified default value may be any legal IDL literal compatible with the
declared member type."

**Repo:** `crates/idl-lower/src/lower.rs` (Annotationsmember, um Z. 2647);
`crates/idl-lower/src/annotation.rs::check_annotation_application` (Z. 468)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_parameter_value_is_the_value_that_was_applied`,
`crates/types/tests/type_object_custom_annotations.rs::an_unapplied_member_writes_ann_custom_absent`

**Status:** done — Annotationen an Annotationsmembern (`<pre_annotations>`)
werden im TypeObject abgewiesen, weil Annex B dafür keinen Platz hat
(`annotation_type_object`).

### §7.3.1.2.5 Applying Annotations: Präfix, mehrere, mehrfach

**Spec:** §7.3.1.2.5, S. 107 — "Annotations may be applied to any type definition
or type member definition. [...] More than one annotation may be applied to the
same element, and multiple instances of the same annotation may be applied to
the same element."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_annotations` (Z. 295)

**Tests:** `crates/idl-lower/tests/annotations.rs::several_annotations_on_one_member_all_apply`,
`crates/types/tests/type_object_custom_annotations.rs::one_declaration_applied_twice_is_one_node`

**Status:** done

### §7.3.1.2.5 Table 24 — Präfix- und Suffixform gleichwertig

**Spec:** §7.3.1.2.5, Table 24, S. 107 — "{ '@' <annotation_type_name> [ '('
<arguments> ')' ] }* [...] Alternately and equivalently [...] { '//@'
<annotation_type_name> [ '(' <arguments> ')' ] }*".

**Repo:** `crates/idl-parse/src/suffix.rs`

**Tests:** `crates/idl-lower/tests/annotations.rs::the_prefix_and_the_suffix_form_produce_the_same_model`,
`crates/idl-lower/tests/annotations.rs::a_suffix_annotation_carries_its_arguments`

**Status:** done

### §7.3.1.2.5 Annotation am Typ des Diskriminators im switch

**Spec:** §7.3.1.2.5, S. 107 — "Annotations can be applied to the implicit
discriminator member of a union type by applying them to the discriminator type
declaration in the header of the union type's definition: union MyUnion switch
(@MyAnnotation long)".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_use_site` (Z. 775)

**Tests:** `crates/idl-lower/tests/annotations.rs::try_construct_reaches_every_position_the_clause_names`,
`crates/idl-lower/tests/annotations.rs::a_declared_annotation_on_a_discriminator_is_kept`

**Status:** done — `@try_construct`, `@key` (unter `Carried`) und eine benutzerdeklarierte Annotation am Typ des Diskriminators im `switch` werden übernommen.

### §7.3.1.2.5 Benutzerannotation am Diskriminator wird geführt

**Spec:** §7.3.1.2.5, S. 107 — `union MyUnion switch (@MyAnnotation long)`.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_use_site`,
`crates/types/src/from_ast.rs::union`

**Tests:** `crates/idl-lower/tests/annotations.rs::a_declared_annotation_on_a_discriminator_is_kept`,
`crates/types/tests/annotations_on_uses.rs::an_annotation_on_the_discriminator_is_in_the_discriminator_member`,
`crates/types/tests/annotations_on_uses.rs::the_applications_come_back_into_the_model`,
`crates/idl-lower/tests/annotations.rs::a_builtin_annotation_with_no_meaning_on_a_use_is_still_reported`

**Status:** done — Eine benutzerdeklarierte Annotation am Diskriminator (`union MyUnion switch (@MyAnnotation long)`) steht in `XTypesFacet::discriminator_annotations` und im `ann_custom` von `CompleteDiscriminatorMember`; die Liste der Union bleibt leer. Ein eingebauter Name ohne Bedeutung dort wird weiter gemeldet.

### §7.3.1.2.5 Namen ohne Groß-/Kleinschreibung

**Spec:** §7.3.1.2.5, S. 108 — "As with any IDL identifier, the name of an
annotation and its members are not case-sensitive."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Z. 330)

**Tests:** `crates/idl-lower/tests/annotations.rs::annotation_names_are_matched_without_regard_to_case`,
`crates/idl-lower/tests/annotations.rs::every_spelling_of_one_builtin_says_the_same_thing`,
`crates/idl-lower/tests/validation.rs::the_names_of_annotation_members_are_not_case_sensitive`

**Status:** done — Die Namen der eingebauten Annotationen werden ohne Unterscheidung von Groß- und Kleinschreibung erkannt, auch qualifiziert (`@::key`); die Namen von Annotationsmembern ebenso.

### §7.3.1.2.5 @Key wird als @key erkannt

**Spec:** §7.3.1.2.5, S. 108 — Namen von Annotationen nicht case-sensitive.

**Repo:** —

**Tests:** `crates/idl-lower/tests/annotations.rs::annotation_names_are_matched_without_regard_to_case`

**Status:** done — `@Key` wird als `@key` erkannt (XTypes 1.3 §7.3.1.2.5). IDL 4.2 §8.2 verlangt für Annotationsnamen die Schreibweise der Deklaration; für die eingebauten Annotationen gilt hier die Regel von XTypes (Widerspruch beider Specs, in der Befunddatei geführt).

### §7.3.1.2.5 Werte als name = value, leere Liste, Defaults, Kurzform

**Spec:** §7.3.1.2.5, S. 108 — "each list item is of the form 'member_name =
member_value.' Each value must be a compile-time constant. [...] An annotation
with an empty list of member values is equivalent to a member list that is
omitted altogether. [...] If a value for a given member is omitted, and that
member has a defined default value, it will take that value. If an omitted
member does not have a specified default, it will take the default value
specified for its type [...] the member name may be omitted when applying the
annotation."

**Repo:** `crates/idl-lower/src/annotation.rs::check_annotation_application`
(Z. 468); `crates/types/src/from_ast.rs::annotation_type_object` (Z. 537)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_short_form_comes_back_as_the_named_one`,
`crates/types/tests/type_object_custom_annotations.rs::a_parameterless_annotation_is_written_without_a_param_seq`,
`crates/idl-lower/tests/validation.rs::an_annotation_member_with_no_default_takes_the_default_of_its_type`

**Status:** done — Benannte Werte, Kurzform, Defaults und ein ausgelassener Member sind umgesetzt; ein Member ohne Default, dem kein Wert gegeben wird, nimmt den Default seines Typs.

### §7.3.1.2.5 Fehlender Wert ohne Default: Default des Typs

**Spec:** §7.3.1.2.5, S. 108 — "If an omitted member does not have a specified
default, it will take the default value specified for its type".

**Repo:** —

**Tests:** `crates/idl-lower/tests/validation.rs::an_annotation_member_with_no_default_takes_the_default_of_its_type`

**Status:** done — `check_annotation_application` meldet für einen Member ohne Default und ohne Wert keinen Fehler; der Wert ist der Default seines Typs (§7.3.1.2.5). IDL 4.2 §7.4.15.4.2 verlangt dagegen einen Wert (Widerspruch beider Specs, in der Befunddatei geführt).

### §7.3.1.3 Constants and Expressions

**Spec:** §7.3.1.3, S. 109 — "Such declarations and expressions remain legal IDL.
However, they are not reflected directly in the Type System specified here,
which assumes that all compile-time-constant values have already been
evaluated."

**Repo:** `crates/idl-lower/src/expr.rs`; `crates/ast/src/node/constant.rs`

**Tests:** `crates/ast/tests/node_space.rs::evaluation_yields_the_folded_value_without_mutating_the_tree`,
`crates/idl-lower/tests/completion.rs::an_array_dimension_is_the_evaluated_constant_expression`

**Status:** done

### §7.3.1.4 Table 25 — IDL primitive type mapping

**Spec:** §7.3.1.4, Table 25, S. 109 — Int8 `int8`, UInt8 `uint8`, Int16 `short`,
UInt16 `unsigned short`, Int32 `long`, UInt32 `unsigned long`, Int64 `long
long`, UInt64 `unsigned long long`, Float32 `float`, Float64 `double`,
Float128 `long double`, Char8 `char`, Char16 `wchar`, Boolean `boolean`, Byte
`octet`.

**Repo:** `crates/idl-lower/src/lower.rs::primitive_ref`;
`crates/types/src/from_ast.rs::primitive_kind` (Z. 1479)

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/types/src/from_ast.rs::primitive_kind_tests::octet_and_uint8_are_two_kinds`

**Status:** done — `uint8` wird `TK_UINT8`, `octet` wird `TK_BYTE`. Die
OMG-ShapeType-IDL `interop/shapes/omg-dds-rtps.idl` ist auf `sequence<uint8>`
umgestellt, damit sie dem Typ der Interop-Suite entspricht.

### §7.3.1.5 Alias Types

**Spec:** §7.3.1.5, S. 109 — "Aliases as described in this specification are
fully compatible with the IDL typedef construct."

**Repo:** `crates/idl-lower/src/lower.rs`; `crates/ast/src/node/type_.rs::Alias`
(Z. 275)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_typedef_becomes_an_alias_to_its_target`,
`crates/idl-lower/tests/lowering.rs::one_typedef_may_declare_several_names`

**Status:** done

### §7.3.1.6 Array and Sequence Types

**Spec:** §7.3.1.6, S. 109 — "Arrays and sequences as described in this
specification are fully compatible with the IDL constructs of the same names."

**Repo:** `crates/ast/src/node/type_.rs::Sequence` (Z. 219);
`crates/ast/src/node/type_.rs::Array` (Z. 226)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_sequence_member_carries_its_element_type`,
`crates/idl-lower/tests/lowering.rs::a_typedef_declarator_keeps_its_array_dimensions`

**Status:** done

### §7.3.1.7 String Types: string und wstring, Bound erhalten

**Spec:** §7.3.1.7, S. 109 — "Strings of Char8 shall be represented by the IDL
type string. Strings of Char16 shall be represented by the IDL type wstring. In
either case, any bound shall be retained."

**Repo:** `crates/idl-lower/src/lower.rs` (`type_ref_of`, Arme `StringType`,
`WStringType`)

**Tests:** `crates/idl-lower/tests/completion.rs::a_bounded_string_and_sequence_carry_their_bound_expressions`

**Status:** done

### §7.3.1.8 Enumerated Types

**Spec:** §7.3.1.8, S. 109 — "Enumerations and bitmasks as described in this
specification are fully compatible with the IDL constructs of the same name."

**Repo:** `crates/ast/src/node/type_.rs::Enumeration` (Z. 258);
`crates/ast/src/node/type_.rs::Bitmask` (Z. 263)

**Tests:** `crates/idl-lower/tests/lowering.rs::an_enum_carries_its_enumerators_in_order`,
`crates/idl-lower/tests/lowering.rs::a_bitmask_carries_its_bits`

**Status:** done

### §7.3.1.9 Map Types

**Spec:** §7.3.1.9, S. 110 — "Map types as described in this specification are
fully compatible with the IDL constructs of the same name defined in the
Extended Data-Types Building Block of [IDL]."

**Repo:** `crates/ast/src/node/type_.rs::Map` (Z. 233)

**Tests:** `crates/types/src/builder.rs::tests::map_builder_string_to_int`,
`crates/wire-conformance/tests/map_bytes.rs::the_recorded_row_round_trips_back_into_the_sample`

**Status:** done

### §7.3.1.10 Structure Types

**Spec:** §7.3.1.10, S. 110 — "Structures as described in this specification are
fully compatible with the IDL constructs of the same name."

**Repo:** `crates/ast/src/node/type_.rs::Structure` (Z. 242)

**Tests:** `crates/idl-lower/tests/lowering.rs::a_struct_carries_its_members_in_order`,
`crates/idl-lower/tests/lowering.rs::a_struct_with_a_base_records_it`

**Status:** done

### §7.3.1.11 Union Types: octet und wchar als Diskriminator

**Spec:** §7.3.1.11, S. 110 — "Compliant IDL parsers shall implement the Building
Block Extended Data-Types of [IDL], which adds support for Byte (octet) and
Char16 (wchar) type discriminators."

**Repo:** `crates/idl-lower/src/validate.rs::is_legal_discriminator` (Z. 244)

**Tests:** `crates/idl-lower/tests/validation.rs::each_legal_discriminator_type_is_accepted`

**Status:** done

---

## §7.3.2 XML Type Representation

### §7.3.2 XML Type Representation: XSD und Namensraum http://www.omg.org/dds

**Spec:** §7.3.2, S. 110 — "This format is defined by an XML schema document (XSD)
and a set of semantic rules [...] The XML namespace of the XML Type
Representation shall be http://www.omg.org/dds."

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_type_libraries` (Z. 34);
`crates/xml-compose/src/lower.rs::lower_all` (Z. 79)

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_simple_struct`,
`crates/xml/src/xtypes_parser.rs::tests::dds_root_with_multiple_types_blocks`,
`crates/xml-compose/tests/loader.rs::a_malformed_document_is_an_error_and_not_an_empty_model`

**Status:** done — zwei Pfade: `zerodds-xml` liest die Typen in eine eigene
`TypeLibrary` und von dort in ein `MinimalTypeObject`; `zerodds-xml-compose`
liest sie in das gemeinsame Modell. Die Einträge unten zitieren den zweiten
Pfad, wo er die Konstruktion trägt. Ob der Namensraum geprüft wird, ist nicht
geprüft.

### §7.3.2 Design Rationale (non-normative)

**Spec:** §7.3.2, S. 110 — die eingebauten Annotationen sind in XML
eigene Konstrukte.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — „non-normative“.

### §7.3.2.1.1 File Inclusion

**Spec:** §7.3.2.1.1, S. 110–111 — "A file inclusion specified as in this Type
Representation shall be considered equivalent to an IDL #include of the same
file."

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_include`,
`crates/xml-compose/src/lower.rs::load_units`,
`crates/xml-compose/src/lower.rs::load_one`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_include_element`,
`crates/xml/src/xtypes_parser.rs::tests::parse_include_missing_file_attribute_rejected`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_include_brings_the_named_document_into_the_model_once`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_include_that_cannot_be_followed_is_reported`

**Status:** done — `zerodds-xml` liest `<include file=…/>` als Eintrag. `zerodds-xml-compose` folgt dem Eintrag: die Datei liegt neben dem Dokument, das sie nennt, wird einmal in das Modell geladen, egal wie oft sie eingebunden wird, und steht vor dem einbindenden Dokument. Eine Datei, die nicht als XML gelesen werden kann, und eine Datei einer anderen Representation sind ein Fehler (§7.3.2.1.1 verlangt das Einbinden anderer Representations nicht).

### §7.3.2.1.1 File Inclusion im Modellpfad

**Spec:** §7.3.2.1.1, S. 110 — Inclusion gleichwertig zu `#include`.

**Repo:** `crates/xml-compose/src/lower.rs::load_units`,
`crates/xml-compose/src/lower.rs::load_one`

**Tests:** `crates/xml-compose/tests/constants_and_annotations.rs::an_include_brings_the_named_document_into_the_model_once`

**Status:** done — `load_units` und `load_one` in `zerodds-xml-compose` laden die genannten Dokumente vor dem einbindenden; ein Typ des eingebundenen Dokuments ist im Modell gleichwertig zu einem, der per `#include` aus IDL kommt. Include-Tiefe begrenzt auf `MAX_INCLUDE_DEPTH`.

### §7.3.2.1.1 Einbinden anderer Representations nicht verlangt

**Spec:** §7.3.2.1.1, S. 111 — "Conformant Type Representation compilers need not
support the inclusion of files of other Type Representations from within an XML
Type Representation document."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Freistellung.

### §7.3.2.1.2 Forward Declarations

**Spec:** §7.3.2.1.2, S. 111 — "A forward declaration as described in this Type
Representation shall be considered semantically equivalent to an IDL forward
declaration."

**Repo:** `crates/xml-compose/src/lower.rs::forward_declaration` (Z. 630);
`crates/xml/src/xtypes_parser.rs::parse_forward_dcl` (Z. 167)

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_forward_dcl_element`,
`crates/xml-compose/tests/idl_parity.rs::forward_and_enclosing_scope_references_reach_the_same_type`

**Status:** done

### §7.3.2.1.3 Constants: value mit const_exp, oben, im Modul, in Struktur

**Spec:** §7.3.2.1.3, S. 111–112 — "the string specified in the value attribute
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

**Status:** done — `<const name type value/>` steht oben, im Modul und in einer Struktur. `value` ist ein `<const_exp>` nach IDL 4.2 §7.4.1.4 mit der Rangfolge der Operatoren aus §7.4.1.4.2; `zerodds-xml-compose` parst ihn (`expr::parse`) und legt den Ausdrucksbaum im Modell ab. Namen werden von innen nach außen aufgelöst, `S::C` erreicht eine Konstante in einer Struktur, ein Name ohne Fundstelle ist ein Fehler. `zerodds-xml` liest `<const>` als Eintrag.

### §7.3.2.1.3 Konstanten und Ausdrücke im Modellpfad

**Spec:** §7.3.2.1.3, S. 111 — `const_exp`-Syntax.

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

**Status:** done — Bounds, Array-Dimensionen und Aufzählungswerte sind `<const_exp>`: `"2 + MY_CONSTANT"`, `"6 * 3"` und `" 2, MY_CONSTANT + 3 "` aus den Beispielen der Spec werden gelesen und ausgewertet (`Ast::eval_integral`). Ein Name, der erst weiter hinten deklariert wird, bleibt im Baum stehen und wird ausgewertet, wenn die Zahl gebraucht wird. Ein Ausdruck, der nicht parst, nennt das Zeichen, bei dem er abbricht.

### §7.3.2.2 Basic Types: type, nonBasicTypeName

**Spec:** §7.3.2.2, S. 112 — "A 'type' attribute, typed by an enumeration
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

**Status:** done — Beide Leser lesen `type="nonBasic"` mit `nonBasicTypeName`. `nonBasicTypeName` neben einem anderen `type` ist ein Fehler, der das Attribut nennt: an Member, Typedef, Diskriminator, Annotationsmember und (als `mapKeyNonBasicTypeName`) am Map-Schlüssel.

### §7.3.2.2 Basic Types: nonBasicTypeName im Pfad zerodds-xml

**Spec:** §7.3.2.2, S. 112 — Attribut für den Namen eines nicht basischen Typs.

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_type_attribute`

**Tests:** `crates/xml/tests/annex_a_types.rs::a_non_basic_type_is_named_by_nonbasictypename_on_every_element_that_has_a_type`,
`crates/xml/tests/annex_a_types.rs::nonbasictypename_beside_a_basic_type_is_an_error`

**Status:** done — `zerodds-xml` liest `nonBasicTypeName` an Member, Typedef, Diskriminator und Annotationsmember und prüft den Namen gegen `identifierName` (Annex A).

### §7.3.2.2 Table 26 — Namen der primitiven Typen und Zeichenketten

**Spec:** §7.3.2.2, Table 26, S. 112–113 — boolean, byte, char8, char16, int32,
uint32, int8, uint8, int16, uint16, int64, uint64, float32, float64, float128,
string, wstring.

**Repo:** `crates/xml-compose/src/lower.rs::primitive_of` (Z. 1449)

**Tests:** `crates/xml-compose/src/lower.rs::tests::the_two_primitive_vocabularies_agree`,
`crates/xml-compose/src/lower.rs::tests::octet_and_uint8_are_two_kinds`,
`crates/xml-compose/tests/idl_parity.rs::every_primitive_reaches_the_same_type_from_both_front_ends`

**Status:** done — zusätzlich werden die IDL-Namen (`long`, `octet`, …)
angenommen.

### §7.3.2.3 String Types: stringMaxLength

**Spec:** §7.3.2.3, S. 113 — "The stringMaxLength attribute, if present,
indicates the string's bound. If the attribute is omitted, the string shall be
considered unbounded. The presence of this attribute is legal only when a
member's type is a string, a wide string, or an alias to string or wide
string."

**Repo:** `crates/xml-compose/src/lower.rs::type_reference` (Z. 753, Arm
`string`/`wstring`)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::string_bounds_reach_the_same_type_from_both_front_ends`

**Status:** done — ob `stringMaxLength` an einem Nicht-String-Member gemeldet
wird, ist nicht geprüft.

### §7.3.2.4 Collection Types: MaxLength-Attribute, „-1“ unbeschränkt

**Spec:** §7.3.2.4, S. 113 — "Collection bounds are indicated by attributes named
according to the convention <collection>MaxLength [...] the values of these
attributes may be any constant expression [...] The literal expression '-1'
shall indicate an unbounded collection; no other 'negative' value is
permitted."

**Repo:** `crates/xml-compose/src/lower.rs::bound`,
`crates/xml-compose/src/lib.rs::ComposeOptions`

**Tests:** `crates/xml-compose/tests/loader.rs::a_bound_of_zero_is_refused_by_default`,
`crates/xml-compose/tests/loader.rs::a_length_unlimited_bound_is_an_unbounded_collection`,
`crates/xml-compose/tests/annex_a_attributes.rs::a_negative_bound_other_than_minus_one_is_refused`,
`crates/xml-compose/tests/constants_and_annotations.rs::bounds_are_const_expressions_over_constants`

**Status:** done — "-1" ist unbeschränkt, jeder andere negative Bound ist ein Fehler, ein Bound ist ein `<const_exp>` und wird ausgewertet (`lower.rs::bound`). Der Wert `0` ist ein Fehler: die Klausel nennt nur "-1", und ein Bound ist in IDL eine positive ganze Zahl (`positive_int_const`). Die Suite schreibt in `sequences.xml` "0 or -1"; `ComposeOptions::zero_bound_is_unbounded` (Standard `false`) liest `0` als unbeschränkt, und der Treiber der Suite (`tools/omg-xtypes-test-main`) setzt es.

### §7.3.2.4 Collection Types: andere negative Bounds werden abgewiesen

**Spec:** §7.3.2.4, S. 113 — "no other 'negative' value is permitted."

**Repo:** `crates/xml-compose/src/lower.rs::bound`

**Tests:** `crates/xml-compose/tests/annex_a_attributes.rs::a_negative_bound_other_than_minus_one_is_refused`

**Status:** done — `bound` weist einen negativen Wert außer -1 mit einem Fehler ab.

### §7.3.2.4 Collection Types: external als Attribut gilt für die Elemente

**Spec:** §7.3.2.4, S. 113 — "The element_external property of the Type System
Model shall be represented by an attribute external."

**Repo:** `crates/xml-compose/src/lower.rs::member_spec`

**Tests:** `crates/xml-compose/tests/annex_a_attributes.rs::external_on_a_collection_member_is_the_elements`

**Status:** done — `external="true"` an einem Collection-Member gilt für die Elemente (`element_is_external`, bei einer Map auch `key_is_external`).

### §7.3.2.4.1 Array Types: arrayDimensions

**Spec:** §7.3.2.4.1, S. 113–114 — "The presence of the arrayDimensions attribute
shall indicate that given member is an array. Array dimensions are represented
as a comma-delimited list [...] Whitespace is allowed around each bound".

**Repo:** `crates/xml-compose/src/lower.rs::dimensions`,
`crates/xml-compose/src/lower.rs::collections_around`,
`crates/xml-compose/src/expr.rs::parse_list`

**Tests:** `crates/xml-compose/tests/idl_parity.rs::arrays_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/constants_and_annotations.rs::bounds_are_const_expressions_over_constants`,
`crates/xml-compose/src/expr.rs::tests::a_list_splits_at_the_commas_outside_parentheses_and_literals`

**Status:** done — Die Dimensionen sind eine Liste von `<const_exp>`, getrennt durch Kommas außerhalb von Klammern und Literalen; Leerraum um eine Grenze zählt nicht. Eine Dimension kleiner als 1 und eine Liste ohne Grenze sind ein Fehler.

### §7.3.2.4.2 Sequence Types: sequenceMaxLength

**Spec:** §7.3.2.4.2, S. 114 — "The sequenceMaxLength attribute, if present, shall
indicate that the member is of a sequence type. This attribute is required for
all sequence types. In case of an unbounded sequence, sequenceMaxLength shall be
set to '-1'."

**Repo:** `crates/xml-compose/src/lower.rs::collections_around` (Z. 816)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::sequences_reach_the_same_type_from_both_front_ends`

**Status:** done

### §7.3.2.4.3 Map Types: mapMaxLength, mapKeyType, mapKeyNonBasicTypeName, mapKeyStringMaxLength

**Spec:** §7.3.2.4.3, S. 114–115 — mapMaxLength ist Pflicht, „-1“
unbeschränkt; mapKeyType ist Pflicht; mapKeyNonBasicTypeName nur bei nicht
basischem Schlüssel; mapKeyStringMaxLength nur bei Zeichenkettenschlüssel.

**Repo:** `crates/xml-compose/src/lower.rs::collections_around`

**Tests:** `crates/xml-compose/tests/constants_and_annotations.rs::map_key_attributes_are_checked_against_the_key_type`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_map_without_map_max_length_is_reported`,
`crates/xml-compose/tests/constants_and_annotations.rs::the_collection_attributes_nest_sequence_then_array_then_map`

**Status:** done — `mapMaxLength` fehlt: Fehler. `mapKeyNonBasicTypeName` neben einem basischen Schlüssel und `mapKeyStringMaxLength` neben einem Schlüssel, der kein String ist: Fehler. Eine Map aus XML wird ins Modell gelowert und ihre Bounds ausgewertet. Ein nicht basischer Schlüssel, der ein Alias auf einen String sein kann, wird bei `mapKeyStringMaxLength` nicht beurteilt.

### §7.3.2.4.4 Kombinationen: Sequence vor Array vor Map

**Spec:** §7.3.2.4.4, S. 115 — "The following list is ordered from
most-tightly-binding to least-tightly-binding: Sequence designations [...]
Array designations [...] Map designations".

**Repo:** `crates/xml-compose/src/lower.rs::collections_around`

**Tests:** `crates/xml-compose/tests/constants_and_annotations.rs::the_collection_attributes_nest_sequence_then_array_then_map`

**Status:** done — `collections_around` bindet die Sequenz zuerst, dann das Array, dann die Map: `sequenceMaxLength`, `arrayDimensions` und `mapKeyType` zusammen ergeben eine Map, deren Werte Arrays von Sequenzen sind. Zuvor lag das Array um die Map und die Sequenz statt um die Sequenz. Eine andere Reihenfolge verlangt einen Alias (§7.3.2.4.4).

### §7.3.2.5 Aggregated Types: Eigenschaften als Attribute

**Spec:** §7.3.2.5, S. 116 — "the XML Type Definition is able to express these
properties directly. For example, structures and unions may indicate whether
they are appendable/mutable and/or nested types: <struct name=\"MyStructure\"
extensibility=\"mutable\" nested=\"true\">".

**Repo:** `crates/xml-compose/src/lower.rs::record` (Z. 1145)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::keys_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/idl_parity.rs::member_ids_reach_the_same_numbers_from_both_front_ends`

**Status:** done

### §7.3.2.5 Aggregated Types: Option für die Default-Extensibility

**Spec:** §7.3.2.5, S. 116 — "type representation compilers shall provide
configuration options to allow users to specify whether types of unspecified
extensibility will be considered final, appendable, or mutable."

**Repo:** `crates/xml-compose/src/lower.rs::lower_all` (Z. 79)

**Tests:** `crates/xml-compose/tests/loader.rs::the_default_extensibility_only_reaches_types_the_document_left_open`

**Status:** done

### §7.3.2.5.1 Structures: Annotationen, Verbatim, Member, Konstanten

**Spec:** §7.3.2.5.1, S. 116 — "Structures contain four kinds of declarations:
Applied annotations, Verbatim text, Members, Constants".

**Repo:** `crates/xml-compose/src/lower.rs::structure`

**Tests:** `crates/xml-compose/tests/idl_parity.rs::keys_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_constant_inside_a_structure_is_in_its_scope`,
`crates/xml-compose/tests/constants_and_annotations.rs::verbatim_text_reaches_the_type`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_annotation_is_declared_and_applied`

**Status:** done — `structure` liest die vier Arten von Kindern: `<annotate>`, `<verbatim>`, `<member>` und `<const>`. Konstanten zuerst, damit ein Bound sie vor der Deklaration nennen kann; die Namen im Inneren beginnen die Suche bei der Struktur.

### §7.3.2.5.1.1 Verbatim Text und §7.3.2.9 Annotations in XML

**Spec:** §7.3.2.5.1.1, S. 116–117, und §7.3.2.9, S. 119 — `<verbatim
language=… placement=…>`, `<annotation name=…>` mit Membern und
`<annotate name=…>` mit Werten.

**Repo:** `crates/xml-compose/src/lower.rs::verbatim_directives`,
`crates/xml-compose/src/lower.rs::annotation_declaration`,
`crates/xml-compose/src/lower.rs::applied_annotations`,
`crates/xml-compose/src/lower.rs::annotate_enumerated`

**Tests:** `crates/xml-compose/tests/constants_and_annotations.rs::verbatim_text_reaches_the_type`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_verbatim_placement_that_is_none_of_the_six_is_reported`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_annotation_is_declared_and_applied`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_annotate_of_an_unknown_or_a_built_in_annotation_is_reported`

**Status:** done — `<verbatim language placement>` steht an Struktur, Union, Aufzählung und Bitset, der Text als Inhalt des Elements oder als `<text>`-Kind nach Annex A; die Platzierung ist eine der sechs Schreibweisen aus IDL 4.2 §8.3.5.1 (Standard `before-declaration`), die Sprache hat den Standard `*`. `<annotation>` deklariert einen Annotationstyp mit Membern, `<annotate>` wendet ihn an Typ, Member, Diskriminator, Aufzählung und Literal an, die Werte sind `<const_exp>`. Eine eingebaute Annotation als `<annotate>` (`key`, `optional`) ist ein Fehler mit Hinweis auf das Attribut. `zerodds-xml` liest keines der drei Elemente.

### §7.3.2.5.1.2 Members: key, optional

**Spec:** §7.3.2.5.1.2, S. 117 — "Each structure type shall include one or more
members. Each member of a structure type can indicate individually whether or
not it is a key member and whether or not it is an optional member."

**Repo:** `crates/xml-compose/src/lower.rs::member_spec` (Z. 667)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::keys_reach_the_same_type_from_both_front_ends`

**Status:** done — „one or more members“ wird nicht erzwungen; IDL 4.2
erlaubt eine leere Struktur (Widerspruch der Spec, in der Befunddatei
geführt).

### §7.3.2.5.1.3 Inheritance: baseType

**Spec:** §7.3.2.5.1.3, S. 117 — "A structure declaration's baseType attribute
indicates the name of the structure's base type, if any".

**Repo:** `crates/xml-compose/src/lower.rs::structure` (Z. 257, `baseType`
Z. 274); `crates/xml/src/xtypes_parser.rs::parse_struct` (Z. 196)

**Tests:** `crates/xml/tests/types_and_samples.rs::t04_struct_inheritance_basetype`,
`crates/xml/tests/types_and_samples.rs::t18_struct_with_inheritance_resolves`

**Status:** done — die Tests decken den Pfad `zerodds-xml` ab; für
`zerodds-xml-compose` fehlt ein Test mit `baseType` (nicht geprüft, ob das
Ergebnis dem IDL-Pfad gleicht).

### §7.3.2.5.2 Unions: discriminator zuerst, case, caseDiscriminator, default

**Spec:** §7.3.2.5.2, S. 117–118 — "exactly one discriminator member [...] and one
or more cases [...] The discriminator member must be declared before the others.
Each case of a union contains one or more discriminator values
(caseDiscriminator elements) and one data member. [...] The literal 'default'
is also allowed".

**Repo:** `crates/xml-compose/src/lower.rs::union` (Z. 308);
`crates/xml-compose/src/lower.rs::case_label` (Z. 992)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::unions_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/loader.rs::a_bitmask_discriminated_union_keeps_its_cases`

**Status:** done

### §7.3.2.6 Aliases: typedef

**Spec:** §7.3.2.6, S. 118 — "Alias definitions are defined in typedef elements.
They have syntax very similar to that of structure members."

**Repo:** `crates/xml-compose/src/lower.rs::alias` (Z. 588);
`crates/xml/src/xtypes_parser.rs::parse_typedef` (Z. 356)

**Tests:** `crates/xml/tests/types_and_samples.rs::t15_typedef_simple_and_array`

**Status:** done — der Test deckt den Pfad `zerodds-xml` ab; für
`zerodds-xml-compose` fehlt ein Test mit `typedef`.

### §7.3.2.7.1 Enumerations: enumerator, value, bitBound

**Spec:** §7.3.2.7.1, S. 118 — "Enumerations consist of a list of enumeration
literals, each of which has a name and a value. The syntax of the value shall be
defined by the <const_exp> production [...] If the value is omitted, it shall
be assigned automatically."

**Repo:** `crates/xml-compose/src/lower.rs::enumeration` (Z. 459)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::a_narrow_bit_bound_reaches_the_same_type_from_both_front_ends`

**Status:** done — `value` als `const_exp` siehe §7.3.2.1.3.

### §7.3.2.7.2 Bitmasks: flag, position, bitBound

**Spec:** §7.3.2.7.2, S. 119 — "<bitmask name=\"MyBitmask\" bitBound=\"64\">
<flag name=\"FIRST_BIT\" position=\"0\"/>".

**Repo:** `crates/xml-compose/src/lower.rs::bitmask` (Z. 534)

**Tests:** `crates/xml-compose/tests/loader.rs::both_bitmask_spellings_reach_the_same_bits`,
`crates/xml-compose/tests/table_15_bitmask.rs::a_bitmask_discriminator_matches_the_uint_of_its_bound`

**Status:** done

### §7.3.2.8 Modules

**Spec:** §7.3.2.8, S. 119 — "A module groups type declarations and serves as a
namespace for those definitions."

**Repo:** `crates/xml-compose/src/lower.rs::module` (Z. 226)

**Tests:** `crates/xml-compose/tests/loader.rs::two_documents_reopen_one_module`,
`crates/xml/src/xtypes_parser.rs::tests::parse_module_nested`

**Status:** done

---

## §7.3.3 XSD Type Representation

### §7.3.3 XSD nach der IDL-zu-XSD-Abbildung

**Spec:** §7.3.3, S. 120 — "An XSD Representation of a given type shall be as if
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

**Status:** done — Das Dokument der Abbildung (CORBA to WSDL/SOAP Interworking 1.2.1, formal/2008-08-03, Abschnitte 4.1.6 bis 4.1.7.6) liegt gelesen vor (https://www.omg.org/spec/C2WSDL/1.2.1/PDF). Gelesen und geschrieben werden: Struktur (4.1.7.2; ein String-Member ist `nillable`), Alias als `simpleType`-Restriction oder, bei einem komplexen Typ, als `complexContent`-Restriction, die den Inhalt wiederholt (4.1.7.3), Union (4.1.7.4), Sequenz mit dem Element `item` (4.1.7.5), Array (4.1.7.6: Feld eines Arraytyps hat den Typ `<Typ>.<Feld>_ArrayOf<Element>`, mehrere Dimensionen sind geschachtelt wie bei `long matrix[5][3]`, die erste Dimension ist der innerste Typ, ein bereits vergebener Name `ArrayOf<Element>` mit anderen Dimensionen bekommt das Suffix `_n`), beschränkter String mit `maxLength fixed` (4.1.6), Konstanten als eingesetzter Wert. `element` (DDS-XML 1.0) wird weiter gelesen. Die SOAP-Encoding-Form der Sequenzen und Arrays (`_SE_`, `soapenc:Array`, 4.1.7.5) wird überlesen und nicht geschrieben; die WS-I-Form ist die geschriebene. Objektreferenzen, Interfaces, Valuetypes, `any`, `TypeCode` und `fixed` (4.1.5, 4.1.7.7 bis 4.1.7.10, 4.1.8) bilden Konstrukte ab, die das Typmodell von XTypes nicht hat. Die Regeln für Modulnamen mit `#pragma prefix`, `version` und `id` (4.1.4) setzen Pragmas voraus, die die Bibliothek nicht kennt; der Name ist der qualifizierte Name mit `.`. Der Nachweis gegen einen zweiten Prüfer läuft auf dem Linux-Testhost (`xmllint`, Log in `.agent/REPORT.md`).

### §7.3.3 Abbildung von Union, Bitmask und Collection nach dem Text der IDL-zu-XSD-Abbildung

**Spec:** §7.3.3, S. 120 — "as if the OMG-standard IDL mapping to XSD were applied to the IDL Representation of the type".

**Repo:** `crates/xml/src/xsd_schema.rs::parse_xsd_schema`;
`crates/xml/src/xsd_writer.rs::type_library_to_xsd`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::the_examples_of_the_mapping_are_read_as_the_idl_they_come_from`,
`crates/xml/tests/xsd_type_representation.rs::the_writer_writes_what_the_mapping_writes`,
`crates/xml/tests/xsd_type_representation.rs::intermediate_types_with_other_dimensions_get_a_suffix`,
`crates/xml/tests/xsd_type_representation.rs::the_element_of_a_collection_may_still_be_named_element`,
`crates/xml/tests/xsd_type_representation.rs::a_library_survives_its_xsd_representation`,
`crates/xml/tests/xsd_type_representation.rs::the_written_schema_compiles_and_validates_sample_documents`

**Status:** done — Union (`sequence` aus `discriminator` und `choice`, 4.1.7.4) und Collections (4.1.7.5, 4.1.7.6) folgen dem Text der Abbildung. Die Labels einer Union stehen in der Abbildung nur als XML-Kommentar (`<!-- case 2, 3 -->`), den ein Leser nicht sieht; sie stehen hier in der Annotation `case` des Zweigs (§7.3.3.1.2), ohne die ein Zweig einen Fehler gibt. Eine Bitmask hat in der Abbildung keine Form (das Dokument kennt keine `bitmask`) und §7.3.3 gibt keine; die `list` einer Restriction mit den Flagnamen ist die Wahl dieser Bibliothek, mit der Annotation `position` je Flag.

### §7.3.3.1.1 Annotationstypen als complexType mit appInfo Definition

**Spec:** §7.3.3.1.1, S. 120–121 — "Any complexType definition immediately
containing an XSD Annotation with an appInfo element having a source attribute
value of http://www.omg.org/Type/Annotation/Definition shall be considered to
be an OMG Annotation." Table 27.

**Repo:** `crates/xml/src/xsd_schema.rs::annotation_definition`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::an_annotation_type_is_a_complex_type_with_the_definition_appinfo`

**Status:** done — Ein `complexType`, dessen `xsd:annotation` ein `appinfo` mit der `source` `http://www.omg.org/Type/Annotation/Definition` enthält, ist ein Annotationstyp; jedes `xsd:attribute` ist ein Member mit Typ und `default` (Table 27). Die Schreibweise `appInfo` des Klauseltexts wird wie `appinfo` gelesen; der Writer schreibt Annotationstypen in dieser Form.

### §7.3.3.1.2 Anwendung über appInfo Usage und annotate

**Spec:** §7.3.3.1.2, S. 121–122 — "OMG Annotations shall be applied to a
definition by declaring, immediately within that definition's XML element, an
XSD Annotation containing an appInfo with its source attribute set to
http://www.omg.org/Type/Annotation/Usage [...] The appInfo element shall
contain an element annotate for each OMG Annotation to be applied."

**Repo:** `crates/xml/src/xsd_schema.rs::applied_to`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::annotations_are_applied_through_the_usage_appinfo`

**Status:** done — Ein `appinfo` der `source` `http://www.omg.org/Type/Annotation/Usage` in der Definition enthält je Annotation ein `annotate`; dessen `type` (auch als `omg:type`) nennt die Annotation, die übrigen Attribute die Werte ihrer Member. Gelesen an `complexType`, `element`, `simpleType` und `enumeration`.

### §7.3.3.1.3 Built-in Annotations in XSD

**Spec:** §7.3.3.1.3, S. 122 — "those Type System concepts represented with
built-in annotations in the IDL Type Representation shall be represented by
equivalent built-in annotations in this Type Representation."

**Repo:** `crates/xml/src/xsd_schema.rs::struct_type`;
`crates/xml/src/xsd_schema.rs::member_of`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::built_in_annotations_are_read_from_their_equivalents`

**Status:** done — Die Built-in-Annotationen des IDL-Typrepräsentation werden unter ihrem IDL-Namen gelesen und geschrieben: `key`, `id`, `optional`, `must_understand`, `non_serialized`, `external`, `try_construct`, `nested`, `autoid`, `final`, `appendable`, `mutable`, `extensibility`, `bit_bound`, `value`, `default_literal`, `position`, `verbatim`. `minOccurs="0"` ist `@optional` (§7.3.3.2.2).

### §7.3.3.2.1 Inheritance: complexContent extension

**Spec:** §7.3.3.2.1, S. 122 — "The subtype shall extend its base type using an
XSD complexContent element." Table 28.

**Repo:** `crates/xml/src/xsd_schema.rs::complex_type`, `crates/xml/src/xsd_schema.rs::struct_type`

**Tests:** `crates/xml/src/xsd_schema.rs::tests::complex_content_extension_yields_inheritance`

**Status:** done

### §7.3.3.2.2 Optional Members: minOccurs 0

**Spec:** §7.3.3.2.2, S. 123 — "Optional members of an aggregated type shall be
indicated with a minOccurs attribute value of 0 instead of 1."

**Repo:** `crates/xml/src/xsd_schema.rs::member_of`, `crates/xml/src/xsd_schema.rs::occurs`

**Tests:** `crates/xml/src/xsd_schema.rs::tests::min_occurs_zero_yields_optional_member`

**Status:** done

### §7.3.3.3 Nested Types: Wurzelelement je nicht nested Typ

**Spec:** §7.3.3.3, S. 123 — "For each type T that is not a nested type, the
schema shall define an XML element of that type suitable for use as a document
root. The name of this element shall be the fully qualified name of T."

**Repo:** `crates/xml/src/xsd_schema.rs::parse_xsd_schema`;
`crates/xml/src/xsd_writer.rs::type_library_to_xsd`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::a_root_element_is_declared_for_each_type_that_is_not_nested`,
`crates/xml/tests/xsd_type_representation.rs::a_nested_type_with_a_root_element_is_refused`

**Status:** done — Der Writer deklariert für jede Struktur und Union, die nicht `nested` ist, ein Element mit dem voll qualifizierten Namen (`MyModule.MyStructure`) als Wurzel; ein nested Typ hat keins. Der Leser erkennt die Wurzelelemente; ein als nested annotierter Typ mit Wurzelelement ist ein Fehler. Aus dem Fehlen eines Wurzelelements schließt der Leser nicht, dass ein Typ nested ist.

### §7.3.3.4 Maps: sequence mit maxOccurs, key und value

**Spec:** §7.3.3.4, S. 123 — "the XSD sequence declaration specifies a maxOccurs
multiplicity equal to the bound of the map [...] The map elements are
represented by elements named key and value".

**Repo:** `crates/xml/src/xsd_schema.rs::wrapper_shape`;
`crates/xml/src/xsd_writer.rs::collection_body`

**Tests:** `crates/xml/tests/xsd_type_representation.rs::a_map_is_a_sequence_with_max_occurs_and_the_elements_key_and_value`

**Status:** done — Eine `sequence` mit `maxOccurs` (die Schranke, sonst `unbounded`) aus den Elementen `key` und `value` ist eine Map, als benannter Typ (Alias) und als Typ eines Members; das Beispiel des Klauseltexts (`MyMap`, int auf double, Schranke 32) wird gelesen, der Writer schreibt dieselbe Form.

---

## §7.3.4 Representing Types with TypeIdentifier and TypeObject

### §7.3.4 Jeder Typ hat einen TypeIdentifier, komplexe Typen ein TypeObject

**Spec:** §7.3.4, S. 123 — "Any possible type within the XTYPES type system is
uniquely identified by a TypeIdentifier. In the case of simple types [...] the
TypeIdentifier completely describes the Type. For more complex types, the
TypeIdentifier only identifies the type and its full description uses a
TypeObject."

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (Z. 197);
`crates/types/src/type_object/mod.rs::TypeObject` (Z. 36);
`crates/types/src/from_ast.rs::type_identifier` (Z. 389)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/src/hash.rs::tests::to_hashed_type_identifier_picks_correct_kind`

**Status:** done

### §7.3.4.1 Plain Types: Collections ohne Annotationen außer @external und @try_construct

**Spec:** §7.3.4.1, S. 124 — "This specification uses the term Plain Collection
type to refer to anonymous collection types (array, sequence, and map) that have
no annotations beyond @external and @try_construct. [...] Plain types only have
a TypeIdentifier. Non-plain types have both a TypeIdentifier and a
TypeObject."

**Repo:** `crates/types/src/from_ast.rs::is_plain` (Z. 1261)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`,
`crates/types/tests/dependency_type_objects.rs::a_type_of_fully_descriptive_members_has_no_dependencies`

**Status:** done — eine anonyme Collection ist immer plain; andere
Annotationen an ihr weist das Front-End ab (§7.2.2.6).

### §7.3.4.2 Type Identifier: identifiziert eine Äquivalenzklasse

**Spec:** §7.3.4.2, S. 124 — "More precisely it identifies each equivalence class
of types, see Clause 7.3.4.6. [...] TypeIdentifier is a discriminated union
allowing the format of the identifier to vary depending on the type. Table 21
below lists the TypeIdentifier discriminator values".

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (Z. 197)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::primitive_none_roundtrips`

**Status:** done — der Text verweist auf „Table 21“ statt Table 29 und auf
§7.3.4.6 statt §7.3.4.7 (Fehler der Spec, in der Befunddatei geführt).

### §7.3.4.2 Table 29 — TK_NONE: ungültiger Identifier

**Spec:** §7.3.4.2, Table 29, S. 124 — "TK_NONE | N/A | Invalid identifier".

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_NONE` (Z. 19);
`crates/types/src/assignability.rs::is_assignable` (Z. 274, Prüfung Z. 295)

**Tests:** `crates/types/tests/table19_key_members.rs::tk_none_is_not_assignable_from_itself`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_none_roundtrips`

**Status:** done

### §7.3.4.2 Table 29 — primitive Typen: Diskriminator ohne Rumpf

**Spec:** §7.3.4.2, Table 29, S. 124 — "TK_BOOLEAN, TK_BYTE, TK_INT8, TK_UINT8,
TK_INT16, TK_INT32, TK_INT64, TK_UINT16, TK_UINT32, TK_UINT64, TK_FLOAT32,
TK_FLOAT64, TK_FLOAT128, TK_CHAR8, TK_CHAR16 | Primitive Types | Plain Type. No
TypeObject. Fully described by the discriminator."

**Repo:** `crates/types/src/type_identifier/mod.rs::PrimitiveKind` (Z. 315)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/src/type_identifier/mod.rs::tests::primitive_kind_roundtrip_via_u8`

**Status:** done

### §7.3.4.2 Table 29 — TI_STRING8_SMALL, TI_STRING8_LARGE

**Spec:** §7.3.4.2, Table 29, S. 124 — "Fully described by the discriminator and
the bound of the string. The SMALL discriminators have a bound represented as
an octet. It is used for unbounded strings or strings with bounds smaller than
256."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING8_SMALL` (Z. 63);
`crates/types/src/type_identifier/kinds.rs::TI_STRING8_LARGE` (Z. 65)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string8_small_roundtrips`,
`crates/types/src/type_identifier/mod.rs::tests::string8_large_roundtrips`

**Status:** done — die Schwelle ist im Code `bound <= 255` für SMALL; die
Spec sagt „smaller than 256“, das ist dieselbe Menge.

### §7.3.4.2 Table 29 — TI_STRING16_SMALL, TI_STRING16_LARGE

**Spec:** §7.3.4.2, Table 29, S. 125 — "Wide String types | Plain Type. No
TypeObject. Fully described by the discriminator and the bound of the string."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING16_SMALL` (Z. 67);
`crates/types/src/type_identifier/kinds.rs::TI_STRING16_LARGE` (Z. 69)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string16_small_and_large_roundtrip`

**Status:** done

### §7.3.4.2 Table 29 — TI_PLAIN_SEQUENCE_SMALL, TI_PLAIN_SEQUENCE_LARGE

**Spec:** §7.3.4.2, Table 29, S. 125 — "TypeIdentifier contains maximum length of
sequence and the TypeIdentifier of element. SMALL and LARGE indicate
representation of maximum length."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_SEQUENCE_SMALL`
(Z. 72); `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_SEQUENCE_LARGE`
(Z. 74)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_sequence_of_int32_roundtrips`,
`crates/types/src/type_identifier/mod.rs::tests::plain_sequence_large_of_string_roundtrips`

**Status:** done

### §7.3.4.2 Table 29 — TI_PLAIN_ARRAY_SMALL, TI_PLAIN_ARRAY_LARGE

**Spec:** §7.3.4.2, Table 29, S. 125 — "TypeIdentifier contains array dimensions
and the TypeIdentifier of element. SMALL and LARGE indicate representation of
dimensions."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_ARRAY_SMALL` (Z. 77);
`crates/types/src/type_identifier/kinds.rs::TI_PLAIN_ARRAY_LARGE` (Z. 79)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_array_small_3d_roundtrips`,
`crates/types/src/type_identifier/mod.rs::tests::plain_array_large_roundtrips`

**Status:** done

### §7.3.4.2 Table 29 — TI_PLAIN_MAP_SMALL, TI_PLAIN_MAP_LARGE

**Spec:** §7.3.4.2, Table 29, S. 125 — "TypeIdentifier contains length of map and
the TypeIdentifier of key and element."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_MAP_SMALL` (Z. 82);
`crates/types/src/type_identifier/kinds.rs::TI_PLAIN_MAP_LARGE` (Z. 84)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_map_small_roundtrips`,
`crates/types/src/type_identifier/mod.rs::tests::plain_map_large_with_nested_map_value_roundtrips`

**Status:** done

### §7.3.4.2 Table 29 — TI_STRONGLY_CONNECTED_COMPONENT

**Spec:** §7.3.4.2, Table 29, S. 125 — "Types with mutual dependencies on other
types | Not plain type. Has TypeObject. Uses a Hash computed on the TypeObjects
of the set of mutually-dependent types."

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRONGLY_CONNECTED_COMPONENT`
(Z. 87); `crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId`
(Z. 176)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::strongly_connected_component_large_scc_length_and_index`,
`crates/types/tests/strongly_connected_components.rs::a_self_referential_type_gets_a_component_identifier`

**Status:** done — zur Berechnung des Hashes siehe §7.3.4.9.2.

### §7.3.4.2 Table 29 — EK_COMPLETE

**Spec:** §7.3.4.2, Table 29, S. 125 — "Not mutually dependent on other types |
Not plain type. Has TypeObject. Uses a Hash of the Complete TypeObject that
describes the type."

**Repo:** `crates/types/src/type_identifier/kinds.rs::EK_COMPLETE` (Z. 92);
`crates/types/src/hash.rs::compute_complete_hash` (Z. 78)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::the_complete_hash_is_the_one_rti_and_cyclone_both_report`

**Status:** done

### §7.3.4.2 Table 29 — EK_MINIMAL

**Spec:** §7.3.4.2, Table 29, S. 125 — "Uses a Hash of the Minimal TypeObject that
describes the type."

**Repo:** `crates/types/src/type_identifier/kinds.rs::EK_MINIMAL` (Z. 90);
`crates/types/src/hash.rs::compute_minimal_hash` (Z. 53)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_minimal_object_is_cyclones_octet_for_octet`,
`crates/types/src/hash.rs::tests::minimal_and_complete_same_semantic_differ_in_hash`

**Status:** done

### §7.3.4.2 Table 29 — TK_ANNOTATION

**Spec:** §7.3.4.2, Table 29, S. 126 — "TK_ANNOTATION | Annotation Declaration |
Not plain type. Has TypeObject. Uses Hash of the TypeObject representation of
the Annotation declaration".

**Repo:** `crates/types/src/from_ast.rs::annotation_type_object` (Z. 537)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_typeid_carries_the_hash_of_the_declarations_own_object`

**Status:** done — die Annotationsdeklaration erhält ein TypeObject, und ihr
Identifier ist dessen Hash (`EK_MINIMAL`/`EK_COMPLETE`). `TK_ANNOTATION` als
eigener Diskriminator eines TypeIdentifier wird nicht angenommen, weil die
`union TypeIdentifier` von Annex B dafür keinen `case` hat
(`docs/omg-xtypes-issues.md` §B9; Widerspruch der Spec).

### §7.3.4.2 Table 29 — TI_EXTENDED

**Spec:** §7.3.4.2, Table 29, S. 126 — "TI_EXTENDED | | Reserved for future
extensions".

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (Z. 197,
Variante `Extended`)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::an_extended_arm_with_a_member_list_round_trips_verbatim`

**Status:** done — Annex B hat keine Konstante `TI_EXTENDED`, sondern den Zweig
`default: ExtendedTypeDefn`; der Baum liest einen unbekannten Diskriminator in
diesen Zweig (Widerspruch der Spec, in der Befunddatei geführt). Unter XCDR1 ist der Zweig eine Parameterliste, die `PID_LIST_END` schließt (der Typ ist MUTABLE und leer); ein unbekanntes Member darin wird übersprungen, ein must-understand-Member weist den Wert ab, und Member, die unter XCDR2 für einen Typ einer späteren Revision gelesen wurden, haben keine XCDR1-Form (`320c16503d2`, Tests in `crates/types/tests/type_lookup_xcdr1.rs`).

### §7.3.4.3 Complete TypeObject: verlustfrei zurück nach IDL

**Spec:** §7.3.4.3, S. 126 — "Any non-plain type represented in IDL can be
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

**Status:** done — Struktur, Union, Aufzählung, Bitmask, Bitset, Alias, Annotationen (auch Parameter vom Typ `long double`, `wchar`, `wstring`) und der `@verbatim` eines Typs kommen zurück; er steht in `AppliedBuiltinTypeAnnotations` der `CompleteTypeDetail` (Platzierung wie §7.2.2.4.8.2 sie schreibt, `string<32>` geprüft). Annex B hat Platz für einen `@verbatim`, §7.2.2.4.8 lässt mehrere zu: ein Typ mit mehr als einem wird mit Namen abgewiesen, nicht ohne die übrigen geschrieben (Widerspruch in der Befunddatei). Das MINIMAL-Objekt enthält keinen `@verbatim`.

### §7.3.4.4 Minimal TypeObject: nur für die Zuweisbarkeit relevante Information

**Spec:** §7.3.4.4, S. 126 — "This representation does not include information on
the type that would not impact type assignability. For example user-defined
annotations or the order of members for types with extensibility kind
MUTABLE."

**Repo:** `crates/types/src/from_ast.rs::minimal_type_object`;
`crates/types/src/type_object/to_minimal.rs`

**Tests:** `crates/types/tests/minimal_object_7_3_4_4.rs::user_defined_annotations_are_not_in_the_minimal_object`,
`crates/types/tests/minimal_object_7_3_4_4.rs::the_members_of_a_mutable_type_are_in_declaration_order`,
`crates/types/tests/minimal_object_7_3_4_4.rs::a_minimal_member_has_the_hash_of_its_name`,
`crates/types/tests/type_object_custom_annotations.rs::the_minimal_form_leaves_the_applied_annotation_out`,
`crates/types/tests/typeobject_annotation_annexb.rs::the_minimal_object_is_annex_bs_octet_for_octet`

**Status:** done — Das MINIMAL-Objekt enthält keine benutzerdefinierte Annotation, keinen `@verbatim` und keine Namen (nur `NameHash`); gleiche Typen mit und ohne Annotation haben dasselbe MINIMAL-Objekt und denselben MINIMAL-Identifier. Die Member eines MUTABLE-Typs stehen in Deklarationsreihenfolge (`member_index`, §7.3.4.5 mit den Ordnungsregeln der Serialisierung); der Beispielsatz in §7.3.4.4 sagt das Gegenteil (Widerspruch in der Befunddatei). Zwei Lager von Herstellern rechnen für ein MINIMAL-Struct mit geschachteltem Aggregat verschiedene Hashes; der Baum wählt über `minimal_member_trailing_octet` (`docs/omg-xtypes-issues.md` §C2, benannte Einstellung).

### §7.3.4.5 Serialisierung: XCDR2, Little Endian

**Spec:** §7.3.4.5, S. 126 — "The serialization of a TypeObject shall happen in
accordance to its IDL declaration and the general serialization rules defined
in this specification (see Clause 7.4) for XCDR encoding version 2. [...] The
serialization shall use Little Endian encoding."

**Repo:** `crates/types/src/type_object/mod.rs::to_bytes_le` (Z. 115)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/tests/typeobject_vendor_dheader.rs::the_first_four_octets_announce_everything_after_them`,
`crates/types/tests/typeobject_annotation_annexb.rs::every_appendable_and_sequence_carries_its_own_length`

**Status:** done

### §7.3.4.5 Reihenfolge: AnnotationParameterSeq nach paramname_hash

**Spec:** §7.3.4.5, S. 126 — "The elements in AnnotationParameterSeq shall be
ordered in increasing values of their paramname_hash _typeid."

**Repo:** `crates/types/src/from_ast.rs::applied_parameters` (Z. 2011, Sortierung Z. 2078)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_parameters_are_ordered_by_name_hash`

**Status:** done — „paramname_hash _typeid“ ist ein Tippfehler der Spec (in
der Befunddatei geführt).

### §7.3.4.5 Reihenfolge: AppliedAnnotationSeq nach annotation_typeid

**Spec:** §7.3.4.5, S. 126 — "The elements in AppliedAnnotationSeq shall be
ordered in increasing values of their annotation_typeid."

**Repo:** `crates/types/src/from_ast.rs::applied_annotations` (Z. 1958, Sortierung Z. 1985)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_applications_are_ordered_by_annotation_typeid`

**Status:** done

### §7.3.4.5 Reihenfolge: Struct- und Union-Member nach member_index

**Spec:** §7.3.4.5, S. 126–127 — "The elements in CompleteStructMemberSeq [...]
MinimalStructMemberSeq [...] CompleteUnionMember [...] MinimalUnionMember shall
be ordered in increasing values of the member_index."

**Repo:** `crates/types/src/from_ast.rs::structure` (Z. 2082);
`crates/types/src/from_ast.rs::union` (Z. 2178)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/src/assignability.rs::tests::flatten_inheritance_two_levels_concatenates_base_first`

**Status:** done — die Member werden in Deklarationsreihenfolge geschrieben.

### §7.3.4.5 Reihenfolge: Annotationsmember

**Spec:** §7.3.4.5, S. 127 — "The elements in CompleteAnnotationMemberSeq shall be
ordered in increasing values of the member_index. The elements in
MinimalAnnotationMemberSeq shall be ordered in increasing values of the
member_name hash."

**Repo:** `crates/types/src/type_object/complete/annotation_type.rs::AnnotationParameterOrder`;
`crates/types/src/type_object/minimal/annotation_type.rs::ordered_by_name_hash`

**Tests:** `crates/types/tests/annotation_parameter_order.rs::the_parameters_follow_member_index_and_the_annex_b_comment_is_a_setting`,
`crates/types/tests/annotation_parameter_order.rs::the_minimal_form_is_ordered_by_the_hash_of_the_name_in_both_readings`,
`crates/types/tests/typeobject_annotation_annexb.rs::each_sequence_is_ordered_by_its_own_key`

**Status:** done — MINIMAL nach dem Hash des Namens; COMPLETE nach `member_index`, der Deklarationsreihenfolge (§7.3.4.5, normativer Text; die Entscheidung des Maintainers). Das Lesen von Annex B ("Ordered by CompleteAnnotationParameter.name") ist die benannte Einstellung `xtypes.annotation_parameter_order: name`, Umgebungsvariable `ZERODDS_ANNOTATION_PARAMETER_ORDER`; die Reihenfolge steht im Objekt und damit in `annotation_typeid` jeder Anwendung. Der Widerspruch steht in der Befunddatei.

### §7.3.4.5 Reihenfolge: CompleteAnnotationMemberSeq nach member_index

**Spec:** §7.3.4.5, S. 127 — `member_index`.

**Repo:** `crates/types/src/from_ast.rs::annotation_type_object`;
`crates/types/src/type_object/complete/annotation_type.rs::annotation_parameter_order`

**Tests:** `crates/types/tests/annotation_parameter_order.rs::the_parameters_follow_member_index_and_the_annex_b_comment_is_a_setting`,
`crates/types/tests/annotation_parameter_order.rs::the_setting_has_a_spelling_that_reads_back`,
`crates/types/tests/type_object_annotations.rs::an_annotation_declaration_has_a_type_object`

**Status:** done — `CompleteAnnotationParameterSeq` ist nach `member_index` geordnet, der Position des Parameters in der Deklaration (`CommonAnnotationParameter` hat keine Member-Id); der Test deklariert `zebra`, `alpha`, `mid` und erwartet diese Reihenfolge, unter der Einstellung `name` die Namensreihenfolge. Der Hash des Annotationstyps ändert sich mit der Einstellung. Erwartung bei Cyclone DDS, Fast DDS und OpenDDS: keiner schreibt `ann_custom` mit Annotationstypen; nicht gemessen.

### §7.3.4.5 Reihenfolge: EnumeratedLiteralSeq nach numerischem Wert

**Spec:** §7.3.4.5, S. 127 — "The elements in CompleteEnumeratedLiteralSeq [...]
MinimalEnumeratedLiteralSeq shall be ordered in increasing values of their
numeric value."

**Repo:** `crates/types/src/from_ast.rs::enumeration`

**Tests:** `crates/types/tests/literal_and_flag_order.rs::enumerated_literals_are_written_in_increasing_value_order`,
`crates/types/src/type_object/mod.rs::tests::minimal_enum_color_3_literals`

**Status:** done — Die Literale werden nach aufsteigendem Wert geschrieben, auch wenn `@value` die Deklarationsreihenfolge umkehrt.

### §7.3.4.5 Reihenfolge: Literale werden nach Wert sortiert

**Spec:** §7.3.4.5, S. 127 — aufsteigend nach numerischem Wert.

**Repo:** —

**Tests:** `crates/types/tests/literal_and_flag_order.rs::enumerated_literals_are_written_in_increasing_value_order`

**Status:** done — `enum E { @value(7) A, @value(0) B };` hat `B` vor `A` im TypeObject und im gehashten Strom.

### §7.3.4.5 Reihenfolge: BitflagSeq nach Position

**Spec:** §7.3.4.5, S. 127 — "The elements in CompleteBitflagSeq [...]
MinimalBitflagSeq shall be ordered in increasing values of their position."

**Repo:** `crates/types/src/from_ast.rs::bitmask`

**Tests:** `crates/types/tests/literal_and_flag_order.rs::bitmask_flags_are_written_in_increasing_position_order`,
`crates/types/src/type_object/mod.rs::tests::minimal_bitmask_3_flags`

**Status:** done — Die Flags werden nach aufsteigender Position geschrieben.

### §7.3.4.5 Reihenfolge: Bitflags werden nach Position sortiert

**Spec:** §7.3.4.5, S. 127 — aufsteigend nach Position.

**Repo:** —

**Tests:** `crates/types/tests/literal_and_flag_order.rs::bitmask_flags_are_written_in_increasing_position_order`

**Status:** done — Bei `@position` in absteigender Reihenfolge werden die Flags trotzdem nach Position sortiert.

### §7.3.4.5 Reihenfolge: BitfieldSeq nach Position

**Spec:** §7.3.4.5, S. 127 — "The elements in CompleteBitfieldSeq [...]
MinimalBitfieldSeq shall be ordered in increasing values of their position."

**Repo:** `crates/types/src/from_ast.rs` (Bitset, Positionen fortlaufend
Z. 2549–2600)

**Tests:** `crates/types/src/builder.rs::tests::bitset_builder_two_fields`

**Status:** done — Positionen werden fortlaufend vergeben, die Sequenz ist
dadurch aufsteigend.

### §7.3.4.6.1 Fully-descriptive TypeIdentifiers, EK_BOTH

**Spec:** §7.3.4.6.1, S. 127 — "The TypeIdentifiers for Primitive and String
types. The TypeIdentifiers of plain collections where the element (and key)
TypeIdentifer a fully descriptive TypeIdentifier. They are recognized by the
contained PlainCollectionHeader having EquivalenceKind set to EK_BOTH."

**Repo:** `crates/types/src/from_ast.rs` (Header, Z. 1443–1456);
`crates/types/src/type_identifier/kinds.rs::EK_BOTH` (Z. 98)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::ek_both_is_not_a_type_identifier_discriminator`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_nested_sequence_of_sequence_minimal_wire_roundtrip`

**Status:** done

### §7.3.4.6.2 Hash TypeIdentifiers

**Spec:** §7.3.4.6.2, S. 127–128 — "Those with discriminator EK_MINIMAL,
EK_COMPLETE, or TI_STRONG_COMPONENT. Those with discriminator
TI_PLAIN_SEQUENCE_SMALL, [...] TI_PLAIN_MAP_LARGE where the contained
PlainCollectionHeader has EquivalenceKind EK_MINIMAL or EK_COMPLETE."

**Repo:** `crates/types/src/type_identifier/mod.rs::PlainCollectionHeader` (Z. 122)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_large_complete_wire_vector`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_map_small_minimal_keys_complete_values_wire_vector`

**Status:** done — die Spec schreibt hier `TI_STRONG_COMPONENT`, Annex B
`TI_STRONGLY_CONNECTED_COMPONENT` (in der Befunddatei geführt).

### §7.3.4.6.3 Direct Hash TypeIdentifiers

**Spec:** §7.3.4.6.3, S. 128 — "These are the HASH TypeIdentifiers with
discriminator EK_MINIMAL, EK_COMPLETE, or TI_STRONG_COMPONENT."

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (Z. 197,
Varianten `EquivalenceHashMinimal`, `EquivalenceHashComplete`,
`StronglyConnectedComponent`)

**Tests:** `crates/types/src/hash.rs::tests::to_hashed_type_identifier_picks_correct_kind`

**Status:** done

### §7.3.4.6.4 Indirect Hash TypeIdentifiers

**Spec:** §7.3.4.6.4, S. 128 — "These are the TypeIdentifiers for plain
collections with the element type identified using a HASH TypeIdentifier.
[...] Having the contained PlainCollectionHeader with EquivalenceKind
EK_MINIMAL or EK_COMPLETE."

**Repo:** `crates/types/src/type_identifier/mod.rs::PlainCollectionHeader` (Z. 122)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_array_small_minimal_wire_vector`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_array_large_complete_wire_vector`

**Status:** done

### §7.3.4.6.5 Minimal Hash TypeIdentifiers

**Spec:** §7.3.4.6.5, S. 128 — EK_MINIMAL, TI_STRONG_COMPONENT mit
TypeObjectHashId EK_MINIMAL, plain collections mit EquivalenceKind EK_MINIMAL.

**Repo:** `crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId`
(Z. 176, Feld `equivalence_kind`)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### §7.3.4.6.6 Complete Hash TypeIdentifiers

**Spec:** §7.3.4.6.6, S. 129 — EK_COMPLETE, TI_STRONG_COMPONENT mit
TypeObjectHashId EK_COMPLETE, plain collections mit EquivalenceKind
EK_COMPLETE.

**Repo:** `crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId`
(Z. 176)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_large_complete_wire_vector`

**Status:** done

### §7.3.4.6 PlainCollectionHeader: unbekannte EquivalenceKind

**Spec:** §7.3.4.6.1–§7.3.4.6.6, S. 127–129 — die EquivalenceKind im
PlainCollectionHeader ist EK_BOTH, EK_MINIMAL oder EK_COMPLETE.

**Repo:** `crates/types/src/type_identifier/mod.rs::EquivalenceKind` (Z. 85)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_invalid_equiv_kind_in_header_decodes_as_unknown_ek_byte`

**Status:** done — ein anderes Oktett wird gelesen und unverändert
behalten.

### §7.3.4.7 Type Equivalence: Complete und Minimal

**Spec:** §7.3.4.7, S. 129–130 — "Two types are equivalent according to the
Complete equivalence relation if and only if either they have equal
Fully-Descriptive TypeIdentifiers, or else they have equal Complete
TypeIdentifiers. Two types are equivalent according to the Minimal equivalence
relation if and only if either they have equal Fully-Descriptive
TypeIdentifiers, or else they have equal Minimal TypeIdentifiers."

**Repo:** `crates/types/src/assignability.rs::is_assignable` (Z. 274, Prüfung
`w == r` Z. 301); `crates/types/src/hash.rs::compute_hash` (Z. 37)

**Tests:** `crates/types/src/hash.rs::tests::different_type_objects_have_different_hashes`,
`crates/types/src/hash.rs::tests::hash_is_14_bytes_and_deterministic`,
`crates/types/src/assignability.rs::tests::equal_complete_hash_is_assignable_with_empty_registry`

**Status:** done

### §7.3.4.7 Equivalence-Relationen: Mengenlehre, finer/coarser

**Spec:** §7.3.4.7, S. 129 — reflexiv, symmetrisch, transitiv; Zerlegung in
Klassen; „finer“ und „coarser“.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung.

### §7.3.4.8 Typen mit gegenseitigen Abhängigkeiten

**Spec:** §7.3.4.8, S. 130 — rekursive Typen wie `TreeNode` mit
`sequence<@external TreeNode>`; der einfache Hash-Algorithmus scheitert an der
zirkulären Abhängigkeit.

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component` (Z. 236)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_self_referential_type_gets_a_component_identifier`,
`crates/types/tests/strongly_connected_components.rs::a_mutually_recursive_pair_is_one_component_of_two`

**Status:** done — das Beispiel verwendet `sequence<@external TreeNode>`,
eine Form, die das Front-End abweist (offen unter §7.3.1.2.1.4); die
Rekursion über eine Sequenz ohne `@external` wird gebaut.

### §7.3.4.8.1 Graphentheorie

**Spec:** §7.3.4.8.1, S. 130–131 — Abhängigkeitsdigraph, Pfad, Zyklus,
Erreichbarkeit, starke Zusammenhangskomponenten, Kernel-DAG, Figure 21.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung.

### §7.3.4.9.1 Introduction

**Spec:** §7.3.4.9.1, S. 131–132 — Figure 22; der Algorithmus wird je
Äquivalenzrelation einmal ausgeführt.

**Repo:** `crates/types/src/from_ast.rs::component_identifier` (Z. 440)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_component_members_type_object_builds`

**Status:** done — `form` wählt MINIMAL oder COMPLETE.

### §7.3.4.9.2 Schritt 1: Graph ohne Zyklus — gewöhnlicher Hash

**Spec:** §7.3.4.9.2, S. 132 — "If this graph has no cycles, then T is not
affected by mutual dependencies and the TypeIdentifier can use the regular
algorithm of hashing the serialized TypeObjects".

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component` (Z. 236)

**Tests:** `crates/types/tests/strongly_connected_components.rs::an_ordinary_type_is_not_given_a_component_identifier`

**Status:** done

### §7.3.4.9.2 Schritte 2 und 3: Knoten ohne Ausgang entfernen, Komponenten bestimmen

**Spec:** §7.3.4.9.2, S. 133 — "Let ReducedDependencyDG(T) be the subdigraph [...]
where all the vertices that have no outgoing edges are removed. [...] Identify
the Strongly Connected Components of the ReducedDependencyDG(T)."

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component` (Z. 236);
`crates/types/src/from_ast.rs::reachable` (Z. 189)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_mutually_recursive_pair_is_one_component_of_two`,
`crates/types/tests/strongly_connected_components.rs::a_type_that_holds_a_component_member_from_outside_builds`

**Status:** done — die Komponente ist die Menge der Typen, die `T` erreichen
und von `T` erreicht werden; plain Collections sind keine Mitglieder.

### §7.3.4.9.2 Schritt 4a: Komponente mit einem Typ

**Spec:** §7.3.4.9.2, S. 133 — "If the Strongly Connected Component (SCC) has a
single type, then use the regular algorithm to compute its TypeIdentifier".

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component`

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_type_that_holds_a_component_member_from_outside_builds`,
`crates/types/tests/strongly_connected_components.rs::an_ordinary_type_is_not_given_a_component_identifier`

**Status:** done — Ein Typ, der selbst auf keinem Zyklus liegt und nur eine Komponente erreicht, erhält den gewöhnlichen Hash; sein TypeObject trägt den SCC-Identifier des erreichten Members.

### §7.3.4.9.2 Schritt 4a: selbstbezüglicher Typ erhält einen SCC-Identifier

**Spec:** §7.3.4.9.2, S. 133 — eine Komponente mit einem Typ verwendet den
gewöhnlichen Algorithmus.

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component`

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_self_referential_type_gets_a_component_identifier`,
`crates/types/tests/strongly_connected_components.rs::a_component_members_type_object_builds`,
`crates/types/tests/strongly_connected_components.rs::the_component_hash_is_the_md5_of_the_xcdr2_sequence_of_its_objects`

**Status:** done — Schritt 4a: eine Komponente aus einem Typ verwendet den gewöhnlichen Algorithmus. Ein Typ, der sich selbst erreicht, hat dafür keinen Hash, weil das TypeObject den Identifier enthielte, den es erst ergibt; er wird als Komponente aus einem Typ nach den Schritten 4b bis 4f behandelt (`TI_STRONGLY_CONNECTED_COMPONENT` mit `scc_length = 1`, `scc_index = 1`). Die Spec nennt für diesen Fall keinen anderen Weg (Widerspruch in der Befunddatei). Ein Abgleich mit dem Identifier eines anderen Herstellers ist nicht möglich: keiner in der Matrix berechnet einen SCC-Identifier.

### §7.3.4.9.2 Schritt 4b: Sortierung nach vollqualifiziertem Namen, Index ab 1

**Spec:** §7.3.4.9.2, S. 133 — "sort them using the lexicographic order of their
fully qualified type name. Let SCCIndex(U) be the sort index of each type U
belonging to the SCC starting with index 1 for the first type. For anonymous
types concatenate the fully-qualified name of the containing type with the
member name using '.' as the separator".

**Repo:** `crates/types/src/from_ast.rs::strongly_connected_component` (Z. 236,
Sortierung Z. 265); `crates/types/src/from_ast.rs::Descent` (Z. 88, Index ab 1)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_mutually_recursive_pair_is_one_component_of_two`

**Status:** done — anonyme Typen kommen als Mitglieder nicht vor (plain
Collections sind ausgeschlossen), die Namensregel für sie wird daher nicht
gebraucht.

### §7.3.4.9.2 Schritt 4b.i: vorläufiger Identifier {EK, hash = 0}, scc_length, scc_index wird gesetzt

**Spec:** §7.3.4.9.2, S. 133 — "Temporarily set the TypeIdentifier of each U
belonging to the SC to: discriminator = TI_STRONGLY_CONNECTED_COMPONENT,
sc_component_id = {discriminator=EK, hash= 0}, scc_length = Number of types in
SCC, scc_index = SCCIndex(U)".

**Repo:** `crates/types/src/from_ast.rs::component_hash`

**Tests:** `crates/types/tests/strongly_connected_components.rs::the_component_hash_is_the_md5_of_the_xcdr2_sequence_of_its_objects`

**Status:** done — Die Hash-Eingabe der Komponente trägt den vorläufigen Identifier mit Hash 0, `scc_length` und `scc_index` (Schritt 4b.i).

### §7.3.4.9.2 Schritt 4b.ii: TypeObjects mit vorläufigen Identifiern

**Spec:** §7.3.4.9.2, S. 133 — "Construct the TypeObject of all the types in the
SC using the temporary TypeIdentifier for references to other types in the
SCC."

**Repo:** `crates/types/src/from_ast.rs::component_hash` (Z. 466)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_component_members_type_object_builds`,
`crates/types/tests/strongly_connected_components.rs::the_component_hash_is_the_md5_of_the_xcdr2_sequence_of_its_objects`

**Status:** done — Die TypeObjects der Komponente werden mit den vorläufigen Identifiern gebaut (Schritt 4b.ii).

### §7.3.4.9.2 Schritte 4c bis 4e: TypeObjectSeq, XCDR2 LE, MD5, 14 Oktette

**Spec:** §7.3.4.9.2, S. 133 — "Place computed TypeObjects [...] into a sequence
TypeObjectSeq in the order of their scc_index. Serialize the TypeObjectSeq
using the XCDR serialization for sequences with encoding version 2 and little
endian. Compute the MD5 hash of the serialized buffer. Let EquivalenceHash(SC)
be the first 14 bytes."

**Repo:** `crates/types/src/from_ast.rs::component_hash` (Z. 466)

**Tests:** `crates/types/tests/strongly_connected_components.rs::the_component_hash_is_the_md5_of_the_xcdr2_sequence_of_its_objects`,
`crates/types/tests/strongly_connected_components.rs::the_component_hash_is_deterministic`

**Status:** done — Der Strom ist die `sequence<TypeObject>` unter XCDR2 Little Endian mit DHEADER und Ausrichtung auf vier; der Hash ist MD5, die ersten 14 Oktette zählen (Schritte 4c bis 4e).

### §7.3.4.9.2 Hash-Eingabe der Komponente mit DHEADER und Ausrichtung

**Spec:** §7.3.4.9.2, S. 133 — XCDR2-Serialisierung der `TypeObjectSeq`.

**Repo:** —

**Tests:** `crates/types/tests/strongly_connected_components.rs::the_component_hash_is_the_md5_of_the_xcdr2_sequence_of_its_objects`

**Status:** done — `component_hash` schreibt DHEADER und Ausrichtung der Sequenz. Ein Vergleich mit dem SCC-Hash eines anderen Herstellers ist nicht möglich.

### §7.3.4.9.2 Schritt 4f: endgültiger Identifier je Typ

**Spec:** §7.3.4.9.2, S. 133–134 — "Set the TypeIdentifier of each of the types
in SC to: discriminator = TI_STRONGLY_CONNECTED_COMPONENT, strong_component_id
= StronglyConnectedComponentId(SC), scc_index = SCCIndex(U)".

**Repo:** `crates/types/src/from_ast.rs::component_identifier` (Z. 440)

**Tests:** `crates/types/tests/strongly_connected_components.rs::a_mutually_recursive_pair_is_one_component_of_two`

**Status:** done

### §7.3.4.9.2 Implementation notes

**Spec:** §7.3.4.9.2, S. 134 — Erreichbarkeit vorwärts und rückwärts,
Kosaraju-Sharir.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — „non-normative“.

### §7.3.4.9.3 SCCIdentifier: scc_index = 0 ist kodierbar

**Spec:** §7.3.4.9.3, S. 134 — "There are situations where an SCC needs to be
identified without referencing a concrete type inside the SCC. In this
situation a TypeIdentifier is constructed the same way as for any of the types
in the SCC except the scc_index field is set to 0."

**Repo:** —

**Tests:** `crates/types/tests/strongly_connected_components.rs::an_scc_identifier_with_index_zero_is_written_and_read`

**Status:** done — `TypeIdentifier::to_bytes_le` und der Leser nehmen `scc_index = 0` an.

### §7.3.4.9.3 StronglyConnectedComponentId: scc_length und Hash

**Spec:** §7.3.4.9.3, S. 134 — "The StronglyConnectedComponentId contains the
number of types in the strongly connected component (field scc_length) and a
hash of all the corresponding TypeObjects (field sc_component_id). [...] The
TypeIdentifiers of all the types belonging to the same SCC only differ on the
scc_index field, which always takes values from 1 to scc_length."

**Repo:** `crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId`
(Z. 176)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::scc_encode_rejects_index_out_of_bounds`,
`crates/types/src/type_identifier/mod.rs::tests::scc_encode_rejects_negative_values`

**Status:** done

---

## §7.4 Data Representation

### §7.4 Figure 23 — DataRepresentationId_t: XCDR 0, XML 1, XCDR2 2

**Spec:** §7.4, Figure 23, S. 135 — "XCDR_DATA_REPRESENTATION = 0,
XML_DATA_REPRESENTATION = 1, XCDR2_DATA_REPRESENTATION = 2".

**Repo:** `crates/types/src/qos.rs::DataRepresentationId` (Z. 174)

**Tests:** `crates/types/src/qos.rs::tests::negotiate_xcdr2_preferred`

**Status:** done

### §7.4 Table 30 — Alternative Data Representations

**Spec:** §7.4, Table 30, S. 136 — Extended CDR und XML mit Vor- und Nachteilen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gegenüberstellung.

---

## §7.4.1 Extended CDR Representation (encoding version 1)

### §7.4.1 Zwei Formate: PLAIN_CDR und PL_CDR

**Spec:** §7.4.1, S. 136 — "These extensions result in two encoding formats:
PLAIN_CDR and PL_CDR. [...] PLAIN_CDR introduces extensions to CDR in order to
handle optional members, bitmasks, and maps. PL_CDR leverages the RTPS
Parameter List encoding to handle mutable types."

**Repo:** `crates/ast-layout/src/key.rs::Encoding` (Varianten `PlainCdr1`,
`PlCdr1`)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::plain_cdr1_plans_no_sentinel`,
`crates/ast-layout/tests/xcdr1_gate.rs::a_mutable_struct_plans_the_pid_list_end_sentinel`

**Status:** done

### §7.4.1.1 PLAIN_CDR für final, appendable, Strings, Sequenzen, Maps; PL_CDR für mutable

**Spec:** §7.4.1.1, S. 136 — "The PLAIN_CDR encoding shall be used for final and
appendable types, including (trivially) primitive types. It shall also be used
for all string, sequence, and map types. Aggregated types declared as mutable
shall use the PL_CDR encoding".

**Repo:** `crates/zerodds-build/src/lib.rs::xcdr1_encoding_for` (Z. 978);
`crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/ast-layout/tests/vendor_gate.rs::appendable_adds_a_delimiter_under_xcdr2_and_nothing_under_xcdr1`

**Status:** done

### §7.4.1.1 Füllbytes am Ende der Serialized Data sind null

**Spec:** §7.4.1.1, S. 136 — "This XTYPES specification further requires that any
padding bytes added at the end of the serialized data shall be set to zero."

**Repo:** `crates/wire/src/writer.rs::align_to` (Z. 1415);
`crates/wire-codec/src/value.rs::encapsulation_header`

**Tests:** `crates/wire/tests/padding_record.rs::every_recorded_octet_is_a_zero_the_writer_wrote`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_drops_the_padding_the_options_name`

**Status:** done — jede Ausrichtung schreibt Null-Oktette; die Anzahl der
Füllbytes am Ende steht im Feld `options` des Encapsulation Header.

### §7.4.1.1.1 Primitive Typen: ausgerichtet auf ihre Größe, Byte-Tausch nach cendian

**Spec:** §7.4.1.1.1, S. 137 — "The serialized data shall be encoded at an offset
that aligned to the size of the primitive type. An endianness byte swap shall be
performed in case the native system endianness is different from the one
currently configured in the XCDR stream (XCDR.cendian)."

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (Z. 456);
`crates/wire/src/writer.rs::align_to` (Z. 1415)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`,
`crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`,
`crates/wire/tests/wire.rs::a_long_after_a_char_sits_at_four_not_at_one`

**Status:** done

### §7.4.1.1.1 Table 31 — Byte: 1 Oktett, Ausrichtung 1

**Spec:** §7.4.1.1.1, Table 31, S. 137 — "Byte | 1 | 1 | The byte value".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (Z. 456)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`

**Status:** done

### §7.4.1.1.1 Table 31 — Boolean: 1 Oktett, 0 oder 1

**Spec:** §7.4.1.1.1, Table 31, S. 137 — "Boolean | 1 | 1 | 0 for false, 1 for
true".

**Repo:** `crates/wire/src/writer.rs::write_bool` (Z. 1460)

**Tests:** `crates/wire/tests/wire.rs::a_boolean_that_is_neither_zero_nor_one_is_refused`

**Status:** done — ein anderes Oktett als 0 oder 1 wird beim Lesen
abgewiesen.

### §7.4.1.1.1 Table 31 — Char8: 1 Oktett

**Spec:** §7.4.1.1.1, Table 31, S. 137 — "Char8 | 1 | 1 | The character value
encoded as described in 7.2.2.2.1.2".

**Repo:** `crates/wire/src/writer.rs::write_char` (Z. 1524)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_char_and_a_wchar_encode_to_one_byte_and_two`

**Status:** done

### §7.4.1.1.1 Table 31 — Char16: 2 Oktette, Ausrichtung 2

**Spec:** §7.4.1.1.1, Table 31, S. 137 — "Char16 | 2 | 2 | The character value
encoded as described in 7.2.2.2.1.2".

**Repo:** `crates/wire/src/writer.rs::write_wchar` (Z. 1559);
`crates/ast-layout/src/xcdr.rs::primitive_layout` (Z. 456)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_char_and_a_wchar_encode_to_one_byte_and_two`

**Status:** done — die Breite ist über `WcharWidth` einstellbar (RTI unter
XCDR1: vier Oktette); der Default ist die Tabelle.

### §7.4.1.1.1 Table 31 — Int8, UInt8: 1 Oktett

**Spec:** §7.4.1.1.1, Table 31, S. 137 — "Int8, UInt8 | 1 | 1 | The integer value
using two's complement notation".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (Z. 456)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`

**Status:** done

### §7.4.1.1.1 Table 31 — Int16, UInt16: 2 Oktette

**Spec:** §7.4.1.1.1, Table 31, S. 137 — "Int16, UInt16 | 2 | 2".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (Z. 456)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`

**Status:** done

### §7.4.1.1.1 Table 31 — Int32, UInt32: 4 Oktette

**Spec:** §7.4.1.1.1, Table 31, S. 137 — "Int32, UInt32 | 4 | 4".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (Z. 456)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`

**Status:** done

### §7.4.1.1.1 Table 31 — Int64, UInt64: 8 Oktette, Ausrichtung 8

**Spec:** §7.4.1.1.1, Table 31, S. 137 — "Int64, UInt64 | 8 | 8".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (Z. 456)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::xcdr1_aligns_sixty_four_bit_primitives_to_eight`

**Status:** done

### §7.4.1.1.1 Table 31 — Float32: 4 Oktette, IEEE einfach

**Spec:** §7.4.1.1.1, Table 31, S. 137 — "Float32 | 4 | 4 | IEEE standard for
normalized single-precision floating-point numbers".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (Z. 456)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`

**Status:** done

### §7.4.1.1.1 Table 31 — Float64: 8 Oktette, Ausrichtung 8

**Spec:** §7.4.1.1.1, Table 31, S. 137 — "Float64 | 8 | 8".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (Z. 456)

**Tests:** `crates/wire/tests/wire.rs::a_double_aligns_to_eight`

**Status:** done

### §7.4.1.1.1 Table 31 — Float128: 16 Oktette, Ausrichtung 8

**Spec:** §7.4.1.1.1, Table 31, S. 137 — "Float128 | 16 | 8 | IEEE standard for
normalized quadruple-precision floating-point numbers".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (Z. 456);
`crates/wire/src/writer.rs::write_f128` (Z. 1864);
`crates/wire/src/reader.rs::read_f128` (Z. 766)

**Tests:** `crates/ast-layout/tests/layout.rs::long_double_is_sixteen_bytes`,
`crates/float128/tests/binary128.rs::one_and_a_half_is_the_pattern_the_vendors_write`,
`crates/wire-probe/tests/interop_corpus.rs::every_case_produces_the_bytes_its_readme_records_in_all_four_modes`

**Status:** done — der Korpusfall `primitive-longdouble` läuft durch
`write_f128` in beiden Versionen und Byte-Reihenfolgen; die Oktette sind
die im Repo aufgezeichneten, keine Messung eines anderen Herstellers.

### §7.4.1.1.2 Char8 wie Byte

**Spec:** §7.4.1.1.2, S. 137 — "Objects of Char8 type shall not be interpreted to
have a specific encoding and shall be serialized as-is in the same way as the
Byte primitive type."

**Repo:** `crates/wire/src/writer.rs::write_char` (Z. 1524)

**Tests:** `crates/wire/tests/char_width.rs::every_code_point_a_char_can_hold_round_trips`

**Status:** done — ein Rust-`char` über U+00FF wird abgewiesen, weil er kein
Oktett ist.

### §7.4.1.1.2 String<Char8>: UTF-8, Länge mit NUL

**Spec:** §7.4.1.1.2, S. 137–138 — "Objects of String<Char8> type shall be
represented using the UTF-8 character encoding. The serialized length [...]
shall be the number of bytes in the CDR buffer taken by the String<Char8>
characters, including the terminating NUL character."

**Repo:** `crates/wire/src/writer.rs::write_string` (Z. 1881)

**Tests:** `crates/wire/tests/wire.rs::a_string_length_counts_its_terminator`,
`crates/wire/tests/wire.rs::an_empty_string_is_a_length_of_one_and_a_nul`,
`crates/wire/tests/wire.rs::a_string_whose_terminator_is_not_a_nul_is_refused`

**Status:** done

### §7.4.1.1.2 String<Char16>: UTF-16, Länge in Oktetten, ohne BOM, ohne NUL

**Spec:** §7.4.1.1.2, S. 138 — "Objects of String<Char16> type shall be
represented using the UTF-16 character encoding. The serialized length [...]
shall be the number of bytes in the CDR buffer taken by the String<Char16>
characters. [...] The UTF-16 representation [...] shall not include a Byte Order
Mark (BOM). The representation shall also not include any terminating NUL
character(s)."

**Repo:** `crates/wire/src/writer.rs::write_wstring` (Z. 1980);
`crates/ast-layout/src/key.rs::WstringLengthPolicy`

**Tests:** `crates/wire-conformance/tests/wstring_bytes.rs::the_unbounded_octets_are_the_recorded_cyclone_and_opendds_rows`,
`crates/wire/tests/wstring.rs::every_cell_round_trips_under_every_unit_order`

**Status:** done — `WstringLengthPolicy::SPECIFICATION` ist der Default;
Zeichenzählung, NUL und vier Oktette je Unit sind Dialekte.

### §7.4.1.1.2 Rationale

**Spec:** §7.4.1.1.2, S. 138 — Begründung für Länge in Oktetten, fehlende BOM und
fehlende NUL.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung.

### §7.4.1.1.3.1 Table 32 — Aufzählung mit bit_bound 1 bis 8: Int8

**Spec:** §7.4.1.1.3.1, Table 32, S. 138 — "Int8 | 1-8".

**Repo:** `crates/ast-layout/src/xcdr.rs::enumeration_bit_bound` (Z. 1317);
`crates/ast-layout/src/xcdr.rs::bit_bound_size` (Z. 1257)

**Tests:** `crates/wire-conformance/tests/enumeration_bytes.rs::an_enumeration_member_is_its_holder_and_round_trips`,
`crates/xml-compose/tests/idl_parity.rs::a_narrow_bit_bound_reaches_the_same_type_from_both_front_ends`

**Status:** done — `EnumHolderWidth` erlaubt den Dialekt, der jede Aufzählung
in vier Oktetten schreibt.

### §7.4.1.1.3.1 Table 32 — bit_bound 9 bis 16: Int16

**Spec:** §7.4.1.1.3.1, Table 32, S. 138 — "Int16 | 9-16".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (Z. 1257)

**Tests:** `crates/wire-conformance/tests/enumeration_bytes.rs::an_enumeration_member_is_its_holder_and_round_trips`

**Status:** done

### §7.4.1.1.3.1 Table 32 — bit_bound 17 bis 32: Int32, Default

**Spec:** §7.4.1.1.3.1, Table 32, S. 138 — "Int32 | 17-32 (32 bits is the default
size, and corresponds to all enumerated types prior to this specification)".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (Z. 1257);
`crates/ast/src/lib.rs::DEFAULT_BIT_BOUND` (Z. 66)

**Tests:** `crates/idl-lower/tests/completion.rs::an_enum_without_a_bit_bound_is_thirty_two_bits`,
`crates/wire-conformance/tests/enumeration_bytes.rs::an_enumeration_member_is_its_holder_and_round_trips`

**Status:** done

### §7.4.1.1.3.2 Table 33 — Bitmask [1..8]: UInt8

**Spec:** §7.4.1.1.3.2, Table 33, S. 138 — "[1..8] | UInt8".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (Z. 1257)

**Tests:** `crates/wire-conformance/tests/bitmask_bytes.rs::a_bitmask_takes_the_holder_its_bit_bound_names`

**Status:** done

### §7.4.1.1.3.2 Table 33 — Bitmask [9..16]: UInt16

**Spec:** §7.4.1.1.3.2, Table 33, S. 138 — "[9..16] | UInt16".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (Z. 1257)

**Tests:** `crates/wire-conformance/tests/bitmask_bytes.rs::a_bitmask_takes_the_holder_its_bit_bound_names`,
`crates/wire-conformance/tests/bitmask_bytes.rs::a_bound_that_is_not_a_holder_width_still_takes_the_holder`

**Status:** done

### §7.4.1.1.3.2 Table 33 — Bitmask [17..32]: UInt32

**Spec:** §7.4.1.1.3.2, Table 33, S. 139 — "[17..32] | UInt32".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (Z. 1257)

**Tests:** `crates/wire-conformance/tests/bitmask_bytes.rs::a_bitmask_takes_the_holder_its_bit_bound_names`

**Status:** done

### §7.4.1.1.3.2 Table 33 — Bitmask [33..64]: UInt64

**Spec:** §7.4.1.1.3.2, Table 33, S. 139 — "[33..64] | UInt64".

**Repo:** `crates/ast-layout/src/xcdr.rs::bit_bound_size` (Z. 1257)

**Tests:** `crates/wire-conformance/tests/bitmask_bytes.rs::a_bitmask_takes_the_holder_its_bit_bound_names`

**Status:** done

### §7.4.1.1.3.2 Bitmask: Bitindex ab LSB, Bits über dem Bound nicht Teil der Maske

**Spec:** §7.4.1.1.3.2, S. 139 — "Bit indexes are counted from zero starting at
the least-significant bit of the full byte size of the bitmask. In the case
where the bound of the bitmask is less than the number of bits in the
corresponding primitive type, the states of the remaining serialized bits are
not specified, and those bits are not considered to be part of the bitmask."

**Repo:** `crates/wire-codec/src/decode.rs::read_bitmask` (Z. 2499)

**Tests:** `crates/wire-conformance/tests/bitmask_bytes.rs::the_bytes_are_the_c_and_go_columns`,
`crates/wire-conformance/tests/bitmask_bytes.rs::a_bit_the_type_does_not_declare_is_not_part_of_the_value`

**Status:** done — Der Bitindex läuft ab dem niedrigsten Bit; ein Bit über dem Bound oder an einer nicht deklarierten Position gehört nicht zum Wert und wird beim Lesen verworfen.

### §7.4.1.1.3.2 Bits über dem Bound werden verworfen

**Spec:** §7.4.1.1.3.2, S. 139 — "the states of the remaining serialized bits
are not specified, and those bits are not considered to be part of the
bitmask."

**Repo:** `crates/wire-codec/src/decode.rs::read_bitmask`

**Tests:** `crates/wire-codec/tests/bitmask_unnamed_bits.rs::a_bit_inside_the_bound_that_the_type_does_not_name_is_not_read`,
`crates/wire/tests/wire.rs::a_bitmask_read_keeps_only_the_declared_bits`

**Status:** done — `read_bitmask` gibt für ein solches Bit keinen Fehler zurück; der Leser in `lang-rules` maskiert ebenso.

### §7.4.1.1.4 Map als Sequenz von MapEntry

**Spec:** §7.4.1.1.4, S. 139 — "Objects of map types shall be represented
according to the following equivalent IDL: struct MapEntry_<key_type>_<value_type>
[...] { <key_type> key; <value_type> value; }; typedef
sequence<MapEntry_...> Map_...;"

**Repo:** `crates/wire/src/writer.rs::write_map`; `crates/wire/src/reader.rs::read_map`
(Z. 1764)

**Tests:** `crates/wire-conformance/tests/map_bytes.rs::the_length_word_is_the_entry_count_and_not_the_number_of_halves`,
`crates/wire-conformance/tests/map_bytes.rs::the_bytes_are_the_recorded_fast_dds_row`

**Status:** done

### §7.4.1.1.5 Strukturen nach CDR

**Spec:** §7.4.1.1.5, S. 139 — "Objects of structure type shall be represented as
defined by the CDR specification [CDR], augmented as described below."

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_aggregate` (Z. 1720)

**Tests:** `crates/ast-layout/tests/layout.rs::a_final_struct_pads_between_members`,
`crates/ast-layout/tests/vendor_gate.rs::point_is_sixteen_bytes_under_xcdr1_and_twelve_under_xcdr2`

**Status:** done

### §7.4.1.1.5.1 Inheritance: Basismember zuerst

**Spec:** §7.4.1.1.5.1, S. 139 — "The members defined by the base type, if any,
shall be serialized before the members of their derived types. The
representation shall be exactly as if all of the members had been defined, in
the same order, in the most-derived type."

**Repo:** `crates/ast/src/query.rs::all_members_of` (Z. 151);
`crates/ast-layout/src/xcdr.rs::plan_aggregate` (Z. 1720)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::an_inherited_member_is_written_ahead_of_the_derived_ones`,
`crates/wire-probe/tests/vendor_bytes.rs::member_ids_continue_across_the_inheritance_boundary`,
`crates/ast-layout/tests/layout.rs::a_bitset_that_inherits_is_sized_over_the_whole_chain`

**Status:** done

### §7.4.1.1.5.2 Optional Members: Parameter-Header davor

**Spec:** §7.4.1.1.5.2, S. 139 — "Structure members marked as optional shall be
preceded by a parameter header as described in Clause 7.4.1.2".

**Repo:** `crates/ast-layout/src/plan.rs::MemberHeader` (Z. 93, Variante
`OptionalCdr1`)

**Tests:** `crates/ast-layout/tests/layout.rs::an_optional_member_carries_a_header_in_every_extensibility`,
`crates/ast-layout/tests/wire_vectors.rs::v11_optional_member_plans_a_header_regardless_of_presence`

**Status:** done

### §7.4.1.2 PL_CDR: Parameter-Header aus ID und Länge, Sentinel

**Spec:** §7.4.1.2, S. 140 — "Preceding each one is a parameter header consisting
of a two-byte parameter ID followed by a two-byte parameter length. One
parameter follows another until a list-terminating sentinel is reached."

**Repo:** `crates/ast-layout/src/plan.rs::MemberHeader` (Z. 93, Variante
`PidCdr1`); `crates/wire/src/reader.rs::read_pid` (Z. 1344)

**Tests:** `crates/wire/tests/wire.rs::the_sentinel_ends_a_parameter_list`,
`crates/wire-conformance/tests/bytes.rs::a_mutable_xcdr1_struct_round_trips_through_its_sentinel`

**Status:** done

### §7.4.1.2 Parameterlänge ohne Füllbytes, Ausrichtung auf 4

**Spec:** §7.4.1.2, S. 140 — "the value of the parameter length is the exact
length of the serialized member. It does not account for any padding bytes that
may follow the serialized member. Padding bytes may be added in order to start
the next parameterID at a 4 byte offset relative to the previous parameterID."

**Repo:** `crates/ast-layout/src/key.rs::PlCdr1HeaderPolicy`,
`crates/wire/src/writer.rs::DEFAULT`,
`crates/wire/src/reader.rs::check_declared_length`

**Tests:** `crates/wire/tests/wire.rs::the_default_parameter_length_is_the_exact_length_of_the_member`,
`crates/wire/tests/wire.rs::a_parameter_length_excludes_the_padding_that_follows_it`,
`crates/wire/tests/wire.rs::a_parameter_length_covers_the_padding_where_the_framing_says_so`

**Status:** `done` — der Default (`PlCdr1HeaderPolicy::Specification`) schreibt die genaue Länge des Members; der nächste Parameter beginnt auf einer 4-Oktett-Grenze (Füllbytes hinter dem Wert). Die auf vier aufgerundete Länge von DDSI-RTPS 2.5 §9.4.2.11 ist die benannte Einstellung `PlCdr1HeaderPolicy::ShortRoundedLength` (`--pl-cdr1-headers short-rounded-length`, Laufzeit `wire.pl_cdr1_headers`). Die Spezifikationen widersprechen sich hier; die Lesart von XTypes ist die Entscheidung des Maintainers.

### §7.4.1.2 Default schreibt und erwartet die aufgerundete Parameterlänge

**Spec:** §7.4.1.2, S. 140 — "the value of the parameter length is the exact
length of the serialized member. It does not account for any padding bytes that
may follow the serialized member."

**Repo:** `crates/ast-layout/src/key.rs::PlCdr1HeaderPolicy::pads_declared_length`,
`crates/wire/src/writer.rs::PlCdr1Framing::DEFAULT`,
`crates/wire/src/reader.rs::check_declared_length`

**Tests:** `crates/wire/tests/wire.rs::the_default_parameter_length_is_the_exact_length_of_the_member`,
`crates/wire/tests/wire.rs::a_default_reader_refuses_the_rounded_length_and_names_the_setting`,
`crates/wire-codec/src/value.rs::tests::the_default_framing_is_the_default_dialects`

**Status:** `done` — Schreiber und Leser des Defaults benutzen die genaue Länge; ein Strom mit aufgerundeter Länge wird als `WireError::WrongFraming` gemeldet, der `short-rounded-length` nennt. Die Rust-Konstante `PlCdr1Framing::DEFAULT` und der Default-Dialekt stimmen überein (Test). Gemessene Hersteller: Cyclone DDS und RTI Connext schreiben die aufgerundete Länge, Fast DDS und OpenDDS die genaue (`vendor_bytes.md`); ihre Dialekte behalten das.

### §7.4.1.2 Mutable als Parameterliste, ein Parameter je Member

**Spec:** §7.4.1.2, S. 140 — "Any object of a mutable aggregated type shall be
serialized as a parameter list. Each of its members shall correspond to a single
parameter within that list."

**Repo:** `crates/ast-layout/src/xcdr.rs::member_header` (Z. 2018)

**Tests:** `crates/ast-layout/tests/layout.rs::a_mutable_struct_frames_every_member_with_a_header`,
`crates/wire-conformance/tests/bytes.rs::an_xcdr1_parameter_list_is_read_by_its_ids_too`

**Status:** done

### §7.4.1.2 Optional in final/appendable: Header mit Länge 0, kein Sentinel

**Spec:** §7.4.1.2, S. 140 — "Any optional member of a final or appendable
structure shall be preceded by a parameter header describing that member. If
the member takes no value within that particular object, the data length
indicated by the header shall be zero. [...] the optional member shall not be
followed by list-terminating sentinel."

**Repo:** `crates/ast-layout/src/plan.rs::MemberHeader` (Z. 93, Variante
`OptionalCdr1`)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_absent_optional_member_round_trips_as_the_absence_the_vendors_write`,
`crates/ast-layout/tests/xcdr1_gate.rs::plain_cdr1_plans_no_sentinel`

**Status:** done

### §7.4.1.2.1 Parameter-ID: FLAG_IMPL_EXTENSION und FLAG_MUST_UNDERSTAND

**Spec:** §7.4.1.2.1, S. 140 — "The first bit of the bitmask [...] indicates
whether the parameter has an implementation-specific interpretation [...]
FLAG_IMPL_EXTENSION. The second bit [...] FLAG_MUST_UNDERSTAND. This bit shall
be set if and only if the must_understand property of the member being
encapsulated is set to true."

**Repo:** `crates/ast-layout/src/key.rs::PidFlagPolicy` (Z. 2119);
`crates/ast-layout/src/xcdr.rs::member_must_understand` (Z. 1287)

**Tests:** `crates/ast-layout/tests/layout.rs::must_understand_is_set_exactly_when_the_facet_says_so`,
`crates/wire/tests/wire.rs::a_flagged_sentinel_and_a_flagged_extended_header_are_recognised`

**Status:** done — im Default `Dialect::Specification` setzt das Flag auch ein Key-Member
(§7.2.2.4.4.4.8).

### §7.4.1.2.1 Reservierter Bereich 0x3F01 bis 0x3FFF

**Spec:** §7.4.1.2.1, S. 140 — "this specification further reserves the largest
255 values—from 16,129 (0x3F01) to 16,383 (0x3FFF)—for use by the OMG [...] For a
parameter to be recognized as one of the well-known values in Table 34, the
FLAG_IMPL_EXTENSION bit must be set to zero."

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (Z. 1357, Feld `reserved` Z. 1486);
`crates/wire-codec/src/decode.rs::read_parameter_list` (Z. 1526)

**Tests:** `crates/wire-probe/tests/decode_round_trip.rs::a_vendor_private_or_reserved_parameter_is_not_a_member_of_this_type`

**Status:** done — ein Parameter mit `FLAG_IMPL_EXTENSION` oder einer ID im
reservierten Bereich wird keinem Member zugeordnet.

### §7.4.1.2.1 Table 34 — PID_EXTENDED 0x3F01, Must Understand

**Spec:** §7.4.1.2.1, Table 34, S. 141 — "PID_EXTENDED | 0x3F01 | Yes | Allows the
specification of large member ID and/or data length values".

**Repo:** `crates/ast-layout/src/xcdr.rs::PID_EXTENDED` (Z. 49)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::the_pl_cdr1_constants_match_the_oracle`,
`crates/wire/tests/wire.rs::the_extended_parameter_form_carries_a_thirty_two_bit_id`

**Status:** done — geschrieben als `0x7F01` (mit `FLAG_MUST_UNDERSTAND`) im Default
`PidFlagPolicy::SentinelMustUnderstand`; `PidFlagPolicy::Bare` schreibt
`0x3F01`, `PidFlagPolicy::ImplExtensionOnMembers` `0xFF01`.

### §7.4.1.2.1 Table 34 — PID_LIST_END 0x3F02, Must Understand

**Spec:** §7.4.1.2.1, Table 34, S. 141 — "PID_LIST_END | 0x3F02 | Yes | Indicates
the end of the parameter list data structure."

**Repo:** `crates/ast-layout/src/xcdr.rs::PID_LIST_END` (Z. 53)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::a_mutable_struct_plans_the_pid_list_end_sentinel`,
`crates/ast-layout/tests/xcdr1_gate.rs::an_empty_mutable_struct_still_plans_the_sentinel`

**Status:** done — geschrieben als `0x7F02` im Default.

### §7.4.1.2.1 Table 34 — PID_LIST_END: Typen der Simple Discovery mit PID 1, 0x3F02 ebenfalls annehmen

**Spec:** §7.4.1.2.1, Table 34, S. 141 — "Simple Discovery types shall be subject
to a special limitation: member ID 1 shall not be used and parameter ID 1 shall
terminate the parameter list [...]. Implementations shall be robust to receiving
parameter ID 0x3F02 to indicate the end of a list as well."

**Repo:** `crates/rtps/src/parameter_list.rs` (`SENTINEL`, Dekodierschleife)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::the_parameter_id_3f02_ends_a_list_as_well`,
`crates/rtps/src/parameter_list.rs::tests::pid_ignore_skipped_in_pl_cdr_decode`

**Status:** done — Eine Parameterliste endet bei PID 1 und bei `0x3F02` (`0x7F02` mit gesetzten Flags).

### §7.4.1.2.1 Parameterliste der Built-in Topics: 0x3F02 wird als Ende erkannt

**Spec:** §7.4.1.2.1, Table 34, S. 141 — "Implementations shall be robust to
receiving parameter ID 0x3F02 to indicate the end of a list as well."

**Repo:** —

**Tests:** `crates/rtps/src/parameter_list.rs::tests::the_parameter_id_3f02_ends_a_list_as_well`

**Status:** done — Die Dekodierschleife vergleicht mit `pid::SENTINEL` und mit `pid::LIST_END`.

### §7.4.1.2.1 Table 34 — PID_IGNORE 0x3F03

**Spec:** §7.4.1.2.1, Table 34, S. 141 — "PID_IGNORE | 0x3F03 | No | All consumers
of this Data Representation shall ignore parameters with this ID."

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (Z. 1357, Feld `ignore` Z. 1487);
`crates/wire-codec/src/decode.rs::read_parameter_list` (Z. 1526, Prüfung Z. 1553);
`crates/rtps/src/parameter_list.rs` (`IGNORE`)

**Tests:** `crates/wire-probe/tests/decode_round_trip.rs::a_vendor_private_or_reserved_parameter_is_not_a_member_of_this_type`,
`crates/rtps/src/parameter_list.rs::tests::pid_ignore_skipped_in_pl_cdr_decode`,
`crates/rtps/src/parameter_list.rs::tests::pid_ignore_zero_length_is_valid`

**Status:** done — `PID_IGNORE` wird auch mit gesetztem `FLAG_MUST_UNDERSTAND` übersprungen.

### §7.4.1.2.1 Table 34 — 0x3F04 bis 0x3FFF reserviert

**Spec:** §7.4.1.2.1, Table 34, S. 141 — "Reserved for OMG | 0x3F04-0x3FFF | N/A".

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (Z. 1357, Feld `reserved` Z. 1486)

**Tests:** `crates/wire-probe/tests/decode_round_trip.rs::a_vendor_private_or_reserved_parameter_is_not_a_member_of_this_type`

**Status:** done — eine reservierte ID wird keinem Member zugeordnet; ohne
`FLAG_MUST_UNDERSTAND` übersprungen, mit dem Bit wird das Sample verworfen.

### §7.4.1.2.1 Schreiben nach Table 34, Lesen robust gegen jedes FLAG_MUST_UNDERSTAND

**Spec:** §7.4.1.2.1, S. 141 — "When writing data, implementations of this
specification shall set the FLAG_MUST_UNDERSTAND bit as described in Table 34.
When reading data, implementations of this specification shall be robust to any
setting of the FLAG_MUST_UNDERSTAND bit and accept the parameter nevertheless."

**Repo:** `crates/wire/src/reader.rs::read_pid_as`,
`crates/wire/src/reader.rs::Reader::with_pl_cdr1_reading`

**Tests:** `crates/wire/tests/wire.rs::a_tolerant_reader_takes_any_flags_on_the_extended_marker`,
`crates/wire/tests/wire.rs::a_tolerant_reader_takes_any_flags_on_the_sentinel`,
`crates/wire/tests/wire.rs::a_flagged_sentinel_and_a_flagged_extended_header_are_recognised`

**Status:** `done` — Schreiben nach Table 34. Lesen: `PlCdr1Reading::Tolerant` (`Reader::with_pl_cdr1_reading`, `LayoutKey::with_pl_cdr1_reading`, `zerodds_build::Config::pl_cdr1_reading`) nimmt jede Belegung von `FLAG_MUST_UNDERSTAND` und `FLAG_IMPL_EXTENSION` an `PID_EXTENDED` und `PID_LIST_END` an. Der Default-Leser meldet die Flags eines anderen Dialekts weiter als `WrongFraming`: Entscheidung des Maintainers, nachsichtiges Lesen nur hinter einer benannten Einstellung. Für die Laufzeit: `wire.pl_cdr1_reading` (`ZERODDS_WIRE_PL_CDR1_READING`), prozessweit; kein `--`-Flag.

### §7.4.1.2.1 Lesen: abweichende Flags an PID_EXTENDED und PID_LIST_END abgewiesen

**Spec:** §7.4.1.2.1, S. 141 — "robust to any setting of the
FLAG_MUST_UNDERSTAND bit and accept the parameter nevertheless."

**Repo:** `crates/wire/src/reader.rs::read_pid_as`

**Tests:** `crates/wire/tests/wire.rs::a_tolerant_reader_takes_any_flags_on_the_extended_marker`,
`crates/wire/tests/wire.rs::a_tolerant_reader_takes_any_flags_on_the_sentinel`

**Status:** `done` — der Test liest `0x3f02`, `0x7f02`, `0xbf02`, `0xff02` und `0x3f01`, `0x7f01`, `0xbf01`, `0xff01`: der Default-Leser nimmt nur die Form seines Dialekts, der nachsichtige Leser alle.

### §7.4.1.2.1 Design rationale zu PID_PAD

**Spec:** §7.4.1.2.1, Fußnote 5, S. 141 — "Design rationale (non-normative)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — „non-normative“.

### §7.4.1.2.1 PID_EXTENDED: 0x7F01, eMemberHeader mit vier Flags, llength

**Spec:** §7.4.1.2.1, S. 142 — "The value of the PID_EXTENDED with the must
understand flag set is 0x7F01 [...] The four bytes following the PID_EXTENDED and
length shall be a serialized UINT32 value 'eMemberHeader' [...] FLAG_1 & FLAG_IMPL_EXTENSION
+ FLAG_2 & FLAG_MUST_UNDERSTAND + FLAG_3 & FLAG_UNSPECIFIED1 + FLAG_4 &
FLAG_UNSPECIFIED2 + memberId [...] The second four bytes [...] llength [...] the
exact length of the serialized member".

**Repo:** `crates/wire/src/writer.rs::promote_pid`;
`crates/wire/src/reader.rs::read_pid_as` (Z. 1357)

**Tests:** `crates/wire/tests/wire.rs::the_extended_parameter_form_carries_a_thirty_two_bit_id`,
`crates/wire/tests/wire.rs::flag_two_of_an_extended_header_is_the_members_must_understand`,
`crates/wire/tests/wire.rs::a_promoted_header_moves_the_must_understand_bit_to_flag_two`

**Status:** done

### §7.4.1.2.1 PID_EXTENDED: slength = 8, Member nach 12 Oktetten

**Spec:** §7.4.1.2.1, S. 142 — "The value of the 16-bit length associated with
the PID_EXTENDED (slength) shall be equal to eight. The serialized member shall
start immediately after the long member length (llength). That is exactly 12
bytes from the beginning of the PID_EXTENDED parameter." Figure 24.

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (Z. 1357, Prüfung
`length != 8` Z. 1379)

**Tests:** `crates/wire/tests/wire.rs::the_extended_parameter_form_carries_a_thirty_two_bit_id`

**Status:** done

### §7.4.1.2.1 PID_EXTENDED: Flags des 16-Bit-ID gelten nicht für den Member

**Spec:** §7.4.1.2.1, S. 143 — "The setting of the FLAG_IMPL_EXTENSION and
FLAG_MUST_UNDERSTAND bits in the 16-bit parameter ID shall not be interpreted to
apply to the extended parameter as well. Instead, the first most-significant
bit of the four-bitmask [...] shall represent the value of FLAG_IMPL_EXTENSION
[...] The second [...] FLAG_MUST_UNDERSTAND [...] The remaining two bits [...]
should be set to zero."

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (Z. 1357, Z. 1413–1422)

**Tests:** `crates/wire/tests/wire.rs::flag_two_of_an_extended_header_is_the_members_must_understand`

**Status:** done

### §7.4.1.2.1 PID_EXTENDED: zulässig in mutable Listen und vor optionalen Membern

**Spec:** §7.4.1.2.1, S. 143 — "These extended parameter headers [...] shall be
legal within the parameter list data structures used to serialize objects of
mutable aggregated types. They shall also be legal when preceding optional
members of final or appendable structures".

**Repo:** `crates/ast-layout/src/plan.rs::MemberHeader` (Z. 93, Feld `extended`
in `PidCdr1` und `OptionalCdr1`)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::the_planner_applies_the_extended_rule_to_a_real_member`,
`crates/ast-layout/tests/layout.rs::a_member_id_above_the_short_pid_range_needs_the_extended_form`

**Status:** done

### §7.4.1.2.1 PID_EXTENDED: Ausrichtung wie bei kurzen Parametern

**Spec:** §7.4.1.2.1, S. 143 — "The alignment rules for extended parameters shall
be the same as those for non-extended parameters, which are defined in [RTPS]
Clause 9.4.2.11."

**Repo:** `crates/wire/src/reader.rs::read_pid_as` (Z. 1357, `align_to(4)` Z. 1358)

**Tests:** `crates/wire/tests/wire.rs::a_parameter_value_counts_alignment_from_its_own_block`

**Status:** done

### §7.4.1.2.2 Member-ID 0 bis 0x3F00: kurze Form

**Spec:** §7.4.1.2.2, S. 143 — "Member IDs from 0 to 16,128 (0x3F00) inclusive
shall be represented exactly in the lower 14 bits of the parameter ID."

**Repo:** `crates/ast-layout/src/xcdr.rs::PID_SHORT_MAX_ID` (Z. 45);
`crates/ast-layout/src/xcdr.rs::needs_extended_pid` (Z. 2110)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::the_last_id_the_short_header_holds_is_the_threshold_itself`,
`crates/ast-layout/tests/xcdr1_gate.rs::a_short_member_with_a_low_id_uses_the_standard_header`

**Status:** done

### §7.4.1.2.2 Andere Member-IDs: erweiterte Form

**Spec:** §7.4.1.2.2, S. 144 — "All other member IDs must be expressed using the
extended parameter header format."

**Repo:** `crates/ast-layout/src/xcdr.rs::needs_extended_pid` (Z. 2110)

**Tests:** `crates/ast-layout/tests/layout.rs::a_member_id_above_the_short_pid_range_needs_the_extended_form`,
`crates/ast-layout/tests/xcdr1_gate.rs::a_member_id_beyond_u16_still_produces_a_usable_plan`

**Status:** done

### §7.4.1.2.2 Kurze und erweiterte Form gleichwertig

**Spec:** §7.4.1.2.2, S. 144 — "if a parameter could be described using a short
parameter header or an extended header, the short and extended expressions of
that header shall be considered totally equivalent."

**Repo:** `crates/wire/src/reader.rs::read_pid_as`

**Tests:** `crates/wire/tests/wire.rs::a_tolerant_reader_takes_the_short_form_where_the_extended_one_is_expected`,
`crates/ast-layout/tests/xcdr1_gate.rs::the_always_extended_policy_takes_the_long_header_for_a_member_that_fits_the_short_one`,
`crates/wire/tests/wire.rs::a_short_parameter_header_becomes_extended_when_the_value_overruns_it`

**Status:** `done` — beide Formen werden geschrieben und gelesen; unter `PlCdr1Reading::Tolerant` liest ein Leser mit `AlwaysExtended` auch die kurze Form (und ein Leser der kurzen Form die erweiterte). Im Default bleibt die kurze Form unter `AlwaysExtended` ein `WrongFraming` (Entscheidung des Maintainers).

### §7.4.1.2.2 Leser unter AlwaysExtended weist die kurze Form ab

**Spec:** §7.4.1.2.2, S. 144 — "totally equivalent".

**Repo:** `crates/wire/src/reader.rs::read_pid_as`

**Tests:** `crates/wire/tests/wire.rs::a_tolerant_reader_takes_the_short_form_where_the_extended_one_is_expected`

**Status:** `done` — der Test liest die kurze Form unter `require_extended` mit dem nachsichtigen Leser und prüft, dass der Default-Leser sie als `WrongFraming` meldet.

### §7.4.1.2.2 Member von Nutzertypen setzen FLAG_IMPL_EXTENSION nie

**Spec:** §7.4.1.2.2, S. 144 — "This mapping ensures that members of
user-defined data types will never set the FLAG_IMPL_EXTENSION bit."

**Repo:** `crates/ast-layout/src/key.rs::impl_extension_at` (Z. 2192)

**Tests:** `crates/wire-probe/tests/opendds_mutable_framing.rs::the_key_rows_are_reproduced`

**Status:** done — nur `PidFlagPolicy::ImplExtensionOnMembers` (OpenDDS, von
keinem Dialekt gewählt) setzt das Bit.

### §7.4.1.2.3 Mutable: Reihenfolge beliebig, Member dürfen fehlen

**Spec:** §7.4.1.2.3, S. 144 — "members of mutable structures may appear in any
order. Furthermore, from the receiver's point of view, any mutable structure
member's value may be omitted. [...] if the receiver's member is not optional,
it logically takes its default value. If the member is optional, it takes no
value at all."

**Repo:** `crates/wire-codec/src/decode.rs::read_parameter_list` (Z. 1526);
`crates/wire-codec/src/default.rs::member` (Z. 100)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_mutable_member_list_is_read_by_its_ids_and_not_by_its_order`,
`crates/wire-conformance/tests/bytes.rs::a_mutable_member_the_stream_omits_takes_its_default`,
`crates/wire-conformance/tests/bytes.rs::a_reordered_list_a_vendor_actually_wrote_reads_here_too`

**Status:** done

### §7.4.1.2.3 PL_CDR enthält alle nicht-optionalen Member

**Spec:** §7.4.1.2.3, S. 144 — "When encoding data using Parameterized CDR, the
encoded data shall contain all non-optional members, even if a member value
matches the default value for the member."

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (Z. 931)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_mutable_xcdr1_struct_round_trips_through_its_sentinel`

**Status:** done — der Encoder schreibt jeden Member des Plans; ein
Weglassen bei Gleichheit mit dem Default ist nicht vorgesehen.

### §7.4.1.2.3 Final und appendable: keine Auslassung, keine Umordnung

**Spec:** §7.4.1.2.3, S. 144 — "Objects of final or appendable structures are not
serialized as full parameter lists, even if some members are optional.
Therefore, the members of these types may not be omitted or reordered."

**Repo:** `crates/ast-layout/src/plan.rs::MemberHeader` (Z. 93, `writes_absence`)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_absent_optional_member_round_trips_as_the_absence_the_vendors_write`

**Status:** done

### §7.4.1.2.3 Union: Diskriminator zuerst, nie ausgelassen

**Spec:** §7.4.1.2.3, S. 144 — "the value of the discriminator member must be
serialized before the value of the current non-discriminator member. The
discriminator value shall not be omitted."

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_at_inner` (Z. 736, Arm
`TypeKind::Union`)

**Tests:** `crates/wire-probe/tests/decode_round_trip.rs::a_mutable_union_frames_its_discriminator_and_its_member`,
`crates/wire-probe/tests/vendor_bytes.rs::a_boolean_and_a_char_discriminator_are_one_octet_and_then_the_member`

**Status:** done

### §7.4.1.2.4 Verschachtelte Parameterlisten: IDs relativ zum innersten Typ

**Spec:** §7.4.1.2.4, S. 144 — "parameter IDs are interpreted relative to the
innermost type definition. [...] an occurrence of PID_LIST_END indicates the
conclusion of the innermost parameter list."

**Repo:** `crates/wire-codec/src/decode.rs::read_parameter_list` (Z. 1526)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::a_nested_parameter_list_numbers_and_terminates_per_level`,
`crates/wire-conformance/tests/bytes.rs::a_parameter_list_that_does_not_end_at_its_sentinel_is_refused`

**Status:** done — der Test prüft die Oktette von `Outer { @id(5) Inner }`
mit `Inner { @id(5) long }`: jede Ebene hat eigene IDs und einen eigenen
Sentinel.

---

## §7.4.2 Extended CDR Representation (encoding version 2)

### §7.4.2 Drei Formate: PLAIN_CDR2, DELIMITED_CDR, PL_CDR2

**Spec:** §7.4.2, S. 144–145 — "This specification defines three encoding
formats used with encoding version 2: PLAIN_CDR2, DELIMITED_CDR, and PL_CDR2."

**Repo:** `crates/ast-layout/src/key.rs::Encoding` (Varianten `PlainCdr2`,
`DelimitedCdr`, `PlCdr2`)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v9_appendable_plans_a_dheader`,
`crates/ast-layout/tests/wire_vectors.rs::v10_mutable_plans_the_oracle_member_id_and_length_code_sequence`

**Status:** done

### §7.4.2 PLAIN_CDR2: 8-Oktett-Typen auf 4 ausgerichtet

**Spec:** §7.4.2, S. 145 — "PLAIN_CDR2 shall be used for all primitive, strings,
and enumerated types. It is also used for any type with extensibility kind
FINAL. The encoding is similar to PLAIN_CDR except that INT64, UINT64, FLOAT64,
and FLOAT128 are serialized into the CDR buffer at offsets that are aligned to 4
rather than 8".

**Repo:** `crates/ast-layout/src/key.rs::max_alignment` (Z. 83)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_caps_alignment_at_four_and_leaves_sizes_alone`,
`crates/ast-layout/tests/vendor_gate.rs::point_is_sixteen_bytes_under_xcdr1_and_twelve_under_xcdr2`

**Status:** done

### §7.4.2 DELIMITED_CDR: DHEADER vor PLAIN_CDR2

**Spec:** §7.4.2, S. 145 — "DELIMITED_CDR shall be used for types with
extensibility kind APPENDABLE. It serializes a UINT32 delimiter header (DHEADER)
before serializing the object using PLAIN_CDR2. The delimiter encodes the
endianness and the length of the serialized object that follows."

**Repo:** `crates/ast-layout/src/xcdr.rs::opens_with_dheader` (Z. 2378);
`crates/wire/src/writer.rs::write_dheader` (Z. 2233)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_appendable_struct_writes_a_dheader_with_the_length_that_followed`,
`crates/ast-layout/tests/wire_vectors.rs::v9_appendable_plans_a_dheader`

**Status:** done — „encodes the endianness“: der DHEADER wird in der
Endianness des Stroms geschrieben; er trägt kein eigenes Feld dafür.

### §7.4.2 PL_CDR2: DHEADER und EMHEADER je Member

**Spec:** §7.4.2, S. 145 — "PL_CDR2 shall be used for aggregated types with
extensibility kind MUTABLE. Similar to DELIMITED_CDR it also serializes a
DHEADER before serializing the object. In addition, it serializes a member
header (EMHEADER) ahead each serialized member."

**Repo:** `crates/ast-layout/src/xcdr.rs::member_header` (Z. 2018);
`crates/wire/src/writer.rs::write_emheader` (Z. 2261)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v10_mutable_plans_the_oracle_member_id_and_length_code_sequence`,
`crates/wire/tests/wire.rs::an_emheader_packs_its_id_code_and_flag_into_one_word`

**Status:** done

---

## §7.4.3 Extended CDR encoding virtual machine

### §7.4.3 Modell aus Strom, Zustandsvariablen und Operationen

**Spec:** §7.4.3, S. 145 — "The encoding formats are specified using a virtual
machine that acts on an XCDR stream object. [...] The XCDR stream model consists
of: A linear byte buffer [...] A set of internal state variables [...] A set of
operations [...] A 'stream insertion' operation".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibungsmittel. Die Anforderungen
stehen in Table 35 bis Table 39 und in den Regeln (1) bis (30), einzeln
geführt. Der Code bildet die Maschine nicht nach; der Plan
(`zerodds-ast-layout`) und der `Writer` (`zerodds-wire`) erzeugen dieselben
Oktette.

### §7.4.3.1 Table 35 — FINAL, Version 1: PLAIN_CDR

**Spec:** §7.4.3.1, Table 35, S. 146 — "FINAL | 1 | PLAIN_CDR".

**Repo:** `crates/zerodds-build/src/lib.rs::xcdr1_encoding_for` (Z. 978);
`crates/zerodds-build/src/dds_type.rs::xcdr1_encoding` (Z. 685);
`crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.1 Table 35 — FINAL, Version 2: PLAIN_CDR2

**Spec:** §7.4.3.1, Table 35, S. 146 — "FINAL | 2 | PLAIN_CDR2".

**Repo:** `crates/zerodds-build/src/lib.rs::encoding_for` (Z. 983);
`crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.1 Table 35 — APPENDABLE, Version 1: PLAIN_CDR

**Spec:** §7.4.3.1, Table 35, S. 146 — "APPENDABLE | 1 | PLAIN_CDR".

**Repo:** `crates/zerodds-build/src/dds_type.rs::xcdr1_encoding` (Z. 685);
`crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/ast-layout/tests/vendor_gate.rs::appendable_adds_a_delimiter_under_xcdr2_and_nothing_under_xcdr1`

**Status:** done

### §7.4.3.1 Table 35 — APPENDABLE, Version 2: DELIMITED_CDR

**Spec:** §7.4.3.1, Table 35, S. 146 — "APPENDABLE | 2 | DELIMITED_CDR".

**Repo:** `crates/zerodds-build/src/lib.rs::encoding_for` (Z. 983);
`crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/ast-layout/tests/vendor_gate.rs::appendable_adds_a_delimiter_under_xcdr2_and_nothing_under_xcdr1`

**Status:** done

### §7.4.3.1 Table 35 — MUTABLE, Version 1: PL_CDR

**Spec:** §7.4.3.1, Table 35, S. 146 — "MUTABLE | 1 | PL_CDR".

**Repo:** `crates/zerodds-build/src/dds_type.rs::xcdr1_encoding` (Z. 685);
`crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.1 Table 35 — MUTABLE, Version 2: PL_CDR2

**Spec:** §7.4.3.1, Table 35, S. 146 — "MUTABLE | 2 | PL_CDR2".

**Repo:** `crates/zerodds-build/src/lib.rs::encoding_for` (Z. 983);
`crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.2.1 Table 36 — NENDIAN

**Spec:** §7.4.3.2.1, Table 36, S. 146 — "NENDIAN | Constant that represents the
native endianness used by the system."

**Repo:** `crates/wire/src/writer.rs::order4` (Z. 2997)

**Tests:** `crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`

**Status:** done — der `Writer` setzt Werte über `to_le_bytes`/`to_be_bytes`
der eingestellten Reihenfolge zusammen; die native Reihenfolge des Systems
geht nicht ein.

### §7.4.3.2.1 Table 36 — cendian

**Spec:** §7.4.3.2.1, Table 36, S. 146 — "cendian | [...] the endianness that
will be used to serialize subsequent objects into the stream. It affects integer
types, floating-point types, enumerated types, and the Char16 type."

**Repo:** `crates/wire/src/writer.rs::Writer` (Z. 1105, Feld `endianness`)

**Tests:** `crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`,
`crates/wire-probe/tests/decode_round_trip.rs::every_case_comes_back_the_value_that_went_in`

**Status:** done — die Reihenfolge gilt für den ganzen Strom; ein Wechsel
mitten im Strom ist nicht vorgesehen und von keiner Regel verlangt. Die
Einheiten eines `wstring` folgen im Default der Stromreihenfolge
(`WstringUnitOrder`).

### §7.4.3.2.1 Table 36 — offset

**Spec:** §7.4.3.2.1, Table 36, S. 146 — "offset | [...] computed relative to
the beginning of the stream so that XCDR.offset counts the number of bytes
currently serialized into the stream."

**Repo:** `crates/wire/src/writer.rs::Writer` (Z. 1105, Feld `out`);
`crates/wire/src/writer.rs::position`

**Tests:** `crates/wire/tests/wire.rs::a_long_after_a_char_sits_at_four_not_at_one`

**Status:** done

### §7.4.3.2.1 Table 36 — origin: Ausrichtung relativ zum logischen Beginn

**Spec:** §7.4.3.2.1, Table 36, S. 146–147 — "origin | [...] used as the
'logical beginning of the stream' for alignment operations. [...] An object O of
type T shall be serialized at an offset that verifies: ((XCDR.offset -
XCDR.origin) % T.dalignment) == 0 [...] the serialization shall insert the
minimum 'padding bytes' needed".

**Repo:** `crates/wire/src/writer.rs::Writer` (Z. 1105, Feld `origin`);
`crates/wire/src/writer.rs::align_to` (Z. 1415);
`crates/wire/src/writer.rs::begin_parameter_value` (Z. 2541)

**Tests:** `crates/wire/tests/wire.rs::a_parameter_value_counts_alignment_from_its_own_block`,
`crates/wire-conformance/tests/bytes.rs::a_parameter_value_is_its_own_alignment_origin_and_the_origin_is_put_back`

**Status:** done

### §7.4.3.2.1 Table 36 — eversion

**Spec:** §7.4.3.2.1, Table 36, S. 147 — "eversion | Octet state variable used to
identify the version of the encoding rules [...] {0x00} -- VERSION_NONE {0x01}
-- VERSION1 {0x02} -- VERSION2".

**Repo:** `crates/ast-layout/src/key.rs::Encoding` (Z. 13)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_caps_alignment_at_four_and_leaves_sizes_alone`

**Status:** done — die Version ist Teil von `Encoding`
(`PlainCdr1`/`PlCdr1` gegen `PlainCdr2`/`DelimitedCdr`/`PlCdr2`). Einen Wert
für VERSION_NONE gibt es nicht; ein Strom ohne Version kommt nicht vor.

### §7.4.3.2.1 Table 36 — maxalign: MALIGN = MIN(Typausrichtung, maxalign)

**Spec:** §7.4.3.2.1, Table 36, S. 147 — "maxalign | [...] MALIGN(O) =
MIN(O.type.alignment, XCDR.maxalign) [...] XCDR.maxalign == MAXALIGN(
XCDR.eversion )".

**Repo:** `crates/wire/src/writer.rs::Writer` (Z. 1105, Feld `max_align`);
`crates/wire/src/writer.rs::with_max_align` (Z. 1341);
`crates/ast-layout/src/key.rs::max_alignment` (Z. 83)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_caps_alignment_at_four_and_leaves_sizes_alone`,
`crates/wire/tests/wire.rs::a_double_aligns_to_eight`

**Status:** done

### §7.4.3.2.2 Table 37 — INIT

**Spec:** §7.4.3.2.2, Table 37, S. 147 — "INIT(V1=<nv1>, V2=<nv2>,…) |
Initializes (constructs) the XCDR stream and sets the state variables".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Operation der Beschreibungsmaschine;
wirksam über Regel (1), unten geführt.

### §7.4.3.2.2 Table 37 — PUSH(VARIABLE=<newvalue>)

**Spec:** §7.4.3.2.2, Table 37, S. 147 — "Pushes the specified XCDR stream
variable VARIABLE into the stack and sets the current value to <newvalue>.
[...] This action is reverted by the POP() operation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Operation der Beschreibungsmaschine;
wirksam über Regel (1), (24) und (25), unten geführt. Der `Writer` sichert
und restauriert `origin` je Parameter (`begin_parameter_value`).

### §7.4.3.2.2 Table 37 — PUSH(V1=<nv1>,V2=<nv2>,…)

**Spec:** §7.4.3.2.2, Table 37, S. 147 — "A shortcut for calling PUSH() multiple
times".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Kurzschreibweise.

### §7.4.3.2.2 Table 37 — POP(VARIABLE)

**Spec:** §7.4.3.2.2, Table 37, S. 148 — "Replaces the XCDR stream variable
VARIABLE with the value for that variable that was pushed on the last PUSH()
operation".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Operation der Beschreibungsmaschine. Die
Regeln (24) und (25) schreiben ein `PUSH(ORIGIN=0)` ohne zugehöriges `POP`;
der `Writer` setzt `origin` nach jedem Parameter zurück
(`crates/wire/src/writer.rs::finish_pid`).

### §7.4.3.2.2 Table 37 — POP(V1, V2,…)

**Spec:** §7.4.3.2.2, Table 37, S. 148 — "A shortcut for calling POP() multiple
times".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Kurzschreibweise.

### §7.4.3.2.2 Table 37 — MAXALIGN: VERSION2 = 4, VERSION1 = 8, VERSION_NONE = 8

**Spec:** §7.4.3.2.2, Table 37, S. 148 — "MAXALIGN( VERSION2) = 4 MAXALIGN(
VERSION1) = 8 MAXALIGN( VERSION_NONE) = 8".

**Repo:** `crates/ast-layout/src/key.rs::max_alignment` (Z. 83)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_caps_alignment_at_four_and_leaves_sizes_alone`,
`crates/ast-layout/tests/xcdr1_gate.rs::xcdr1_aligns_sixty_four_bit_primitives_to_eight`

**Status:** done

### §7.4.3.2.2 Table 37 — ALIGN(N): neededalign = MIN(N, maxalign)

**Spec:** §7.4.3.2.2, Table 37, S. 148 — "ALIGN(N) | [...] inserting 'padding
bytes' into the stream. The value of the padded bytes is left unspecified. [...]
neededalign = MIN(N, XCDR.maxalign) [...] (XCDR.offset - XCDR.origin) %
neededalign == 0".

**Repo:** `crates/wire/src/writer.rs::align_to` (Z. 1415)

**Tests:** `crates/wire/tests/wire.rs::a_reader_skips_exactly_the_padding_the_writer_wrote`,
`crates/wire/tests/padding_record.rs::every_recorded_octet_is_a_zero_the_writer_wrote`

**Status:** done — die Füllbytes sind null.

### §7.4.3.2.2 Table 37 — XCDR << { O : T }

**Spec:** §7.4.3.2.2, Table 37, S. 148 — "The 'append' stream operation.
Serializes [...] an object 'O' of type 'T' onto the XCDR stream starting at
offset XCDR.offset."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Notation der Regeln.

### §7.4.3.2.3 Initialisierung des Stroms: leerer Puffer, Endianness frei wählbar

**Spec:** §7.4.3.2.3, S. 148 — "The XCDR stream shall be initialized with an
empty buffer. The endianness shall be set as desired by the implementation,
although a common setting for best performance is the native system endianness
(NENDIAN)."

**Repo:** `crates/wire/src/writer.rs::with_max_align` (Z. 1341);
`crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728, Parameter
`big_endian`)

**Tests:** `crates/dcps/src/runtime.rs::tests::big_endian_encap_routes_to_decode_be`,
`crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`

**Status:** done

### §7.4.3.2.3 Version der Kodierung aus der Konfiguration des DataWriter, 1 oder 2

**Spec:** §7.4.3.2.3, S. 148 — "The encoding version (eversion) shall be set as
configured on the DataWriter. In this version of the DDS-XTypes specification it
may be set to 1 or 2."

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728, Parameter
`offer_first`)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done — das erste Element der angebotenen
`DataRepresentationQosPolicy` wählt die Version.

### §7.4.3.2.3 Die ersten zwei Oktette: ENC_HEADER des äußersten Typs

**Spec:** §7.4.3.2.3, S. 148 — "The first 2 octets in the XCDR stream shall be
the Encapsulation Header (ENC_HEADER) indicating the endianness, encoding
version, and encoding algorithm of the top-level type. See Table 39."

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header_in`,
`crates/dcps/src/runtime.rs::user_payload_encap`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_header_names_the_identifiers_of_table_39_when_asked`,
`crates/wire-probe/tests/vendor_bytes.rs::the_encapsulation_options_carry_the_padding_count`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** `done` — die ersten zwei Oktette sind der ENC_HEADER des äußersten Typs mit Endianness, Version und Format; die Kennung folgt Table 60 (Default) oder Table 39 (`EncapsulationIds::Table39`, `RuntimeConfig::encapsulation_ids`).

### §7.4.3.3 Table 38 — AsFinal(T)

**Spec:** §7.4.3.3, Table 38, S. 149 — "This transformation only affects
Aggregated types. [...] AsFinal(T) is a new type which is declared the same as
T except that its extensibility kind is FINAL."

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_aggregate` (Z. 1720)

**Tests:** `crates/ast-layout/tests/vendor_gate.rs::appendable_adds_a_delimiter_under_xcdr2_and_nothing_under_xcdr1`,
`crates/wire-conformance/tests/bytes.rs::an_appendable_struct_writes_a_dheader_with_the_length_that_followed`

**Status:** done — eine `@appendable`-Struktur hat unter XCDR2 den DHEADER
und dahinter die Member wie eine `@final`-Struktur.

### §7.4.3.3 Table 38 — AsNested(T)

**Spec:** §7.4.3.3, Table 38, S. 149 — "This transformation treats the type as a
Nested type for serialization purposes."

**Repo:** `crates/wire-codec/src/encode.rs::encode` (Z. 340)

**Tests:** `crates/wire-probe/tests/interop_corpus.rs::every_case_emits_a_keyhash_and_an_encapsulated_payload`

**Status:** done — der äußerste Typ wird hinter dem Header mit demselben
Pfad geschrieben wie ein verschachtelter Wert.

### §7.4.3.3 Table 38 — AsBytes(O)

**Spec:** §7.4.3.3, Table 38, S. 149 — "This transformation reinterprets the
primitive object as an array of bytes. The resulting bytes are ordered as they
appear in the processor memory according to the native Endianness".

**Repo:** `crates/wire/src/writer.rs::order4` (Z. 2997)

**Tests:** `crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`

**Status:** done — `to_le_bytes`/`to_be_bytes` ersetzen AsBytes und ESWAP
zusammen; das Ergebnis ist dasselbe.

### §7.4.3.3 Table 38 — ESWAP(B, <doit>) für 1, 2, 4, 8 Oktette

**Spec:** §7.4.3.3, Table 38, S. 149 — "Conditionally swaps the bytes [...] For
length 2: { B[1], B[0] } For length 4: { B[3], B[2], B[1], B[0] } For length 8:
{ B[7], ... B[0] }".

**Repo:** `crates/wire/src/writer.rs::order4` (Z. 2997);
`crates/wire/src/writer.rs::write_f128` (Z. 1864)

**Tests:** `crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`,
`crates/float128/tests/binary128.rs::every_pattern_round_trips_through_sixteen_bytes_in_both_orders`

**Status:** done — ESWAP ist für 16 Oktette nicht definiert, Regel (2)
wendet es auf jeden primitiven Typ an, also auch auf Float128 (Lücke der
Spec, in der Befunddatei aufgeführt). `write_f128` kehrt alle 16 Oktette um
(CORBA 3.3 Teil 2, Figure 9.2, laut Kommentar im Code).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x00}: PLAIN_CDR, BIG_ENDIAN

**Spec:** §7.4.3.4, Table 39, S. 150 — "{0x00, 0x00} -- PLAIN_CDR, BIG_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header` (Z. 527);
`crates/dcps/src/runtime.rs::user_encap_body` (Z. 20061)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/src/runtime.rs::tests::big_endian_encap_routes_to_decode_be`

**Status:** done

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x01}: PLAIN_CDR, LITTLE_ENDIAN

**Spec:** §7.4.3.4, Table 39, S. 150 — "{0x00, 0x01} -- PLAIN_CDR,
LITTLE_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header` (Z. 527);
`crates/dcps/src/runtime.rs::user_encap_body` (Z. 20061)

**Tests:** `crates/dcps/src/runtime.rs::tests::strip_user_encap_xcdr1_le`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x02}: PL_CDR, BIG_ENDIAN

**Spec:** §7.4.3.4, Table 39, S. 150 — "{0x00, 0x02} -- PL_CDR, BIG_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header` (Z. 527);
`crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done — `user_payload_encap` löscht das niedrigste Bit der
Kennung für Little Endian.

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x03}: PL_CDR, LITTLE_ENDIAN

**Spec:** §7.4.3.4, Table 39, S. 150 — "{0x00, 0x03} -- PL_CDR, LITTLE_ENDIAN".

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header` (Z. 527);
`crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x10}: PLAIN_CDR2, BIG_ENDIAN

**Spec:** §7.4.3.4, Table 39, S. 150 — "{0x00, 0x10} -- PLAIN_CDR2, BIG_ENDIAN".

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

**Status:** `done` — Lesen: `user_encap_body` und `encapsulation_form` nehmen die Kennung an. Schreiben: `EncapsulationIds::Table39` (Sonde, `Emit::PayloadWithEncapsulation`) und `RuntimeConfig::encapsulation_ids = Table39` (Laufzeit) schreiben `{0x00, 0x10}` für PLAIN_CDR2 big endian. Der Default bleibt Table 60 (`{0x00, 0x06}` bis `{0x00, 0x0b}`), den Cyclone DDS, RTI Connext und Fast DDS schreiben; die Tabellen widersprechen sich (Befunddatei, Widersprüche). Laufzeit: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x11}: PLAIN_CDR2, LITTLE_ENDIAN

**Spec:** §7.4.3.4, Table 39, S. 150 — "{0x00, 0x11} -- PLAIN_CDR2,
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

**Status:** `done` — Lesen: `user_encap_body` und `encapsulation_form` nehmen die Kennung an. Schreiben: `EncapsulationIds::Table39` (Sonde, `Emit::PayloadWithEncapsulation`) und `RuntimeConfig::encapsulation_ids = Table39` (Laufzeit) schreiben `{0x00, 0x11}` für PLAIN_CDR2 little endian. Der Default bleibt Table 60 (`{0x00, 0x06}` bis `{0x00, 0x0b}`), den Cyclone DDS, RTI Connext und Fast DDS schreiben; die Tabellen widersprechen sich (Befunddatei, Widersprüche). Laufzeit: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x12}: PL_CDR2, BIG_ENDIAN

**Spec:** §7.4.3.4, Table 39, S. 150 — "{0x00, 0x12} -- PL_CDR2, BIG_ENDIAN".

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

**Status:** `done` — Lesen: `user_encap_body` und `encapsulation_form` nehmen die Kennung an. Schreiben: `EncapsulationIds::Table39` (Sonde, `Emit::PayloadWithEncapsulation`) und `RuntimeConfig::encapsulation_ids = Table39` (Laufzeit) schreiben `{0x00, 0x12}` für PL_CDR2 big endian. Der Default bleibt Table 60 (`{0x00, 0x06}` bis `{0x00, 0x0b}`), den Cyclone DDS, RTI Connext und Fast DDS schreiben; die Tabellen widersprechen sich (Befunddatei, Widersprüche). Laufzeit: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x13}: PL_CDR2, LITTLE_ENDIAN

**Spec:** §7.4.3.4, Table 39, S. 150 — "{0x00, 0x13} -- PL_CDR2, LITTLE_ENDIAN".

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

**Status:** `done` — Lesen: `user_encap_body` und `encapsulation_form` nehmen die Kennung an. Schreiben: `EncapsulationIds::Table39` (Sonde, `Emit::PayloadWithEncapsulation`) und `RuntimeConfig::encapsulation_ids = Table39` (Laufzeit) schreiben `{0x00, 0x13}` für PL_CDR2 little endian. Der Default bleibt Table 60 (`{0x00, 0x06}` bis `{0x00, 0x0b}`), den Cyclone DDS, RTI Connext und Fast DDS schreiben; die Tabellen widersprechen sich (Befunddatei, Widersprüche). Laufzeit: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x14}: DELIMIT_CDR, BIG_ENDIAN

**Spec:** §7.4.3.4, Table 39, S. 150 — "{0x00, 0x14} -- DELIMIT_CDR,
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

**Status:** `done` — Lesen: `user_encap_body` und `encapsulation_form` nehmen die Kennung an. Schreiben: `EncapsulationIds::Table39` (Sonde, `Emit::PayloadWithEncapsulation`) und `RuntimeConfig::encapsulation_ids = Table39` (Laufzeit) schreiben `{0x00, 0x14}` für DELIMIT_CDR big endian. Der Default bleibt Table 60 (`{0x00, 0x06}` bis `{0x00, 0x0b}`), den Cyclone DDS, RTI Connext und Fast DDS schreiben; die Tabellen widersprechen sich (Befunddatei, Widersprüche). Laufzeit: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x00, 0x15}: DELIMIT_CDR, LITTLE_ENDIAN

**Spec:** §7.4.3.4, Table 39, S. 150 — "{0x00, 0x15} -- DELIMIT_CDR,
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

**Status:** `done` — Lesen: `user_encap_body` und `encapsulation_form` nehmen die Kennung an. Schreiben: `EncapsulationIds::Table39` (Sonde, `Emit::PayloadWithEncapsulation`) und `RuntimeConfig::encapsulation_ids = Table39` (Laufzeit) schreiben `{0x00, 0x15}` für DELIMIT_CDR little endian. Der Default bleibt Table 60 (`{0x00, 0x06}` bis `{0x00, 0x0b}`), den Cyclone DDS, RTI Connext und Fast DDS schreiben; die Tabellen widersprechen sich (Befunddatei, Widersprüche). Laufzeit: `wire.encapsulation_ids` (`ZERODDS_WIRE_ENCAPSULATION_IDS`).

### §7.4.3.4 Table 39 — ENC_HEADER {0x01, 0x00}: XML

**Spec:** §7.4.3.4, Table 39, S. 150 — "{0x01, 0x00} -- XML".

**Repo:** `crates/dcps/src/xml_data.rs::is_xml_payload`,
`crates/dcps/src/runtime.rs::is_xml_encap`,
`crates/dcps/src/runtime.rs::encap_in_table`

**Tests:** `crates/dcps/src/runtime.rs::tests::an_xml_sample_under_the_identifier_of_table_39_reaches_the_reader`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::an_xml_writer_sends_the_document_of_the_sample`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** `done` — ein Sample unter `{0x01, 0x00}` wird als XML-Dokument gelesen (`is_xml_payload`), unter `RuntimeConfig::encapsulation_ids = Table39` schreibt ein XML-Writer `{0x01, 0x00}` vor das Dokument des Samples und des `serializedKey`. Der Default ist `{0x00, 0x04}` (Table 60, DDSI-RTPS 2.5 Table 10.3). Ein Hersteller, der `{0x01, 0x00}` schreibt, ist nicht gemessen.

### §7.4.3.4 Table 39 — EVERSION: 0x00 als Version 1, 0x01, 0x02

**Spec:** §7.4.3.4, Table 39, S. 150 — "EVERSION is an octet used to identify
the version of the encoding rules [...] 0x00 -- unspecified version (understood
as version 1) 0x01 -- version 1 0x02 -- version 2".

**Repo:** `crates/ast-layout/src/key.rs::max_alignment` (Z. 83)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_caps_alignment_at_four_and_leaves_sizes_alone`

**Status:** done — jede Kodierung außer Version 2 wird wie Version 1
behandelt (`is_v1`). EVERSION erscheint nur im ENC_HEADER, nicht als eigenes
Oktett.

### §7.4.3.4 Table 39 — DHEADER(O) = O.ssize

**Spec:** §7.4.3.4, Table 39, S. 151 — "DHEADER(O) = O.ssize Where O.ssize is
the number of bytes following the header that are required to hold the
serialized representation of O."

**Repo:** `crates/wire/src/writer.rs::finish_dheader` (Z. 2241)

**Tests:** `crates/wire/tests/wire.rs::a_dheader_holds_the_length_of_what_followed_it`

**Status:** done

### §7.4.3.4 Table 39 — EMHEADER1 = (M_FLAG<<31) + (LC<<28) + M.id

**Spec:** §7.4.3.4, Table 39, S. 151 — "EMHEADER1 = (M_FLAG<<31) + (LC<<28) +
M.id".

**Repo:** `crates/wire/src/writer.rs::write_emheader` (Z. 2261)

**Tests:** `crates/wire/tests/wire.rs::an_emheader_packs_its_id_code_and_flag_into_one_word`

**Status:** done

### §7.4.3.4 Table 39 — LC(M): 3-Bit-Längencode

**Spec:** §7.4.3.4, Table 39, S. 151 — "LC is a 3-bit length code used to
construct the EMHEADER1. It determines whether EMHEADER header has an additional
4 bytes (the NEXTINT)".

**Repo:** `crates/ast-layout/src/xcdr.rs::length_code` (Z. 2149);
`crates/ast-layout/src/plan.rs::LengthCode` (Z. 15)

**Tests:** `crates/ast-layout/tests/layout.rs::all_eight_length_codes_are_representable`

**Status:** done

### §7.4.3.4 Table 39 — NEXTINT nur bei LC ≥ 4

**Spec:** §7.4.3.4, Table 39, S. 151 — "NEXTINT is only present if LC(M)>=4."

**Repo:** `crates/wire/src/writer.rs::write_emheader` (Z. 2261);
`crates/wire/src/reader.rs::read_emheader` (Z. 1265)

**Tests:** `crates/wire/tests/wire.rs::a_length_code_below_four_carries_no_next_int`,
`crates/wire/tests/wire.rs::a_length_code_of_five_leaves_its_next_int_for_the_member`

**Status:** done

### §7.4.3.4.1 DHEADER vor DELIMITED_CDR und PL_CDR2, ohne sich selbst

**Spec:** §7.4.3.4.1, S. 151 — "The DELIMITED_CDR and PL_CDR encoding formats
prepend a UInt32 delimiter header (DHEADER) ahead of the serialization of the
object content. The DHEADER encodes the serialized size of the object that
follows (not including the DHEADER itself)."

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_aggregate` (Z. 1720);
`crates/wire/src/writer.rs::finish_dheader` (Z. 2241)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr2_gives_a_dheader_to_anything_that_is_not_final`,
`crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`

**Status:** done — „PL_CDR“ steht für PL_CDR2; PL_CDR (Version 1) hat keinen
DHEADER (Schreibfehler der Spec, in der Befunddatei aufgeführt).

### §7.4.3.4.1 O.ssize kleiner als 2^32

**Spec:** §7.4.3.4.1, S. 151 — "In this expression, O.ssize is constrained to
being smaller than 4 Giga Bytes (2^32 Bytes)."

**Repo:** `crates/wire/src/reader.rs::read_dheader`,
`crates/wire/src/writer.rs::try_finish_dheader`,
`crates/wire/src/writer.rs::finish_dheader`,
`runtime/c/src/writer.c::zdw_finish_dheader`

**Tests:** `crates/wire/tests/wire.rs::a_delimited_sequence_cannot_run_past_its_delimiter`,
`crates/wire/tests/wire.rs::a_dheader_length_of_4_gib_or_more_is_refused`,
`crates/wire/src/writer.rs::tests::an_infallible_finish_dheader_over_4_gib_is_deferred_and_not_truncated`

**Status:** `done` — Lesen: ein UInt32 kann keine größere Länge nennen, `read_dheader` weist eine Länge über den Rest ab. Schreiben: `try_finish_dheader` gibt den Fehler zurück, `finish_dheader` (ohne Rückgabe) merkt ihn (`Writer::deferred_error`, `check_deferred`, `finish_checked`) und schreibt keine Länge modulo 2^32; die `DdsType`-Glue von `zerodds-build` prüft ihn vor der Rückgabe. Die C-Laufzeit setzt den Status des Writers auf `ZDW_RANGE`. Die Tests für 4 GiB laufen über die Länge als Argument, nicht über einen Puffer dieser Größe. Die übrigen Sprachlaufzeiten (C++, Python, Lua, Julia, OCaml, Elixir) prüfen den Bereich im Patch-Aufruf.

### §7.4.3.4.1 finish_dheader kürzt eine Länge ab 2^32 ohne Fehler

**Spec:** §7.4.3.4.1, S. 151 — "O.ssize is constrained to being smaller than 4
Giga Bytes".

**Repo:** `crates/wire/src/writer.rs::try_finish_dheader`,
`crates/wire/src/writer.rs::finish_dheader`

**Tests:** `crates/wire/tests/wire.rs::a_dheader_length_of_4_gib_or_more_is_refused`,
`crates/wire/src/writer.rs::tests::an_infallible_finish_dheader_over_4_gib_is_deferred_and_not_truncated`,
`crates/wire/src/writer.rs::tests::a_dheader_below_4_gib_is_patched_and_nothing_is_deferred`

**Status:** `done` — `finish_dheader` schreibt keine gekürzte Länge mehr: der Platzhalter bleibt unbeschrieben, der Fehler wird gemerkt und von `check_deferred` und `finish_checked` zurückgegeben; `lang-rules` erzeugt weiter `w.finish_dheader(..)`, die Glue von `zerodds-build` prüft danach.

### §7.4.3.4.1 DHEADER auf 4 ausgerichtet relativ zu origin

**Spec:** §7.4.3.4.1, S. 151 — "The serialization of the DHEADER being a Uint32
type forces a 4-byte alignment relative to XCDR.origin, this may insert into the
stream up to 3 padding bytes prior to the DHEADER."

**Repo:** `crates/wire/src/writer.rs::write_dheader` (Z. 2233);
`crates/wire/src/reader.rs::read_dheader` (Z. 1213)

**Tests:** `crates/wire/tests/wire.rs::an_appending_writer_writes_what_a_fresh_one_appended_would`

**Status:** done

### §7.4.3.4.1 DHEADER in der aktuellen Stromreihenfolge

**Spec:** §7.4.3.4.1, S. 151 — "The serialization of the DHEADER uses the
endianness active in the XCDR stream at the time it is serialized
(XCDR.cendian)."

**Repo:** `crates/wire/src/writer.rs::finish_dheader` (Z. 2241, `order4`)

**Tests:** `crates/wire/tests/wire.rs::an_appending_writer_writes_what_a_fresh_one_appended_would`,
`crates/wire-probe/tests/interop_corpus.rs::every_case_produces_the_bytes_its_readme_records_in_all_four_modes`

**Status:** done

### §7.4.3.4.2 Member-Header mit 4 oder 8 Oktetten

**Spec:** §7.4.3.4.2, S. 152 — "A member header precedes the serialization of
each member. The member header can be either 4 or 8 bytes."

**Repo:** `crates/wire/src/writer.rs::write_emheader` (Z. 2261)

**Tests:** `crates/wire/tests/wire.rs::a_length_code_below_four_carries_no_next_int`,
`crates/ast-layout/tests/wire_vectors.rs::v10_mutable_plans_the_oracle_member_id_and_length_code_sequence`

**Status:** done

### §7.4.3.4.2 EMHEADER1 und NEXTINT in der Stromreihenfolge

**Spec:** §7.4.3.4.2, S. 152 — "EMHEADER1 shall be serialized using the XCDR
stream endianness current at the place the serialization occurs [...] NEXTINT.
It shall be serialized with the same endianness as EMHEADER1."

**Repo:** `crates/wire/src/writer.rs::write_emheader` (Z. 2261);
`crates/wire/src/writer.rs::finish_emheader` (Z. 2321)

**Tests:** `crates/wire/tests/wire.rs::shortening_a_header_keeps_its_id_and_its_flag_in_both_orders`,
`crates/wire/tests/wire.rs::an_emheader_next_int_that_cannot_be_true_is_refused`

**Status:** done

### §7.4.3.4.2 EMHEADER1 = (M_FLAG << 31) + (LC << 28) + (MemberId & 0x0fffffff)

**Spec:** §7.4.3.4.2, S. 152 — "EMHEADER1 = (M_FLAG << 31) + (LC << 28) +
(MemberId & 0x0fffffff)".

**Repo:** `crates/wire/src/writer.rs::write_emheader` (Z. 2261);
`crates/wire/src/reader.rs::read_emheader` (Z. 1265)

**Tests:** `crates/wire/tests/wire.rs::an_emheader_packs_its_id_code_and_flag_into_one_word`

**Status:** done — eine Member-ID über `0x0FFFFFFF` wird beim Schreiben
abgewiesen statt maskiert.

### §7.4.3.4.2 M_FLAG = must understand des Members

**Spec:** §7.4.3.4.2, S. 152 — "The must understand flag (M_FLAG) shall be set to
1 if the corresponding member must be understood by the receiver, see Clause
7.2.2.4.4.4.6. Otherwise it shall be set to zero."

**Repo:** `crates/ast-layout/src/xcdr.rs::member_must_understand`,
`crates/ast-layout/src/key.rs::MustUnderstandPolicy`

**Tests:** `crates/ast-layout/tests/layout.rs::must_understand_is_set_exactly_when_the_facet_says_so`,
`crates/wire-probe/tests/vendor_bytes.rs::the_must_understand_flag_is_the_one_each_vendor_writes_in_both_versions`,
`crates/wire-probe/tests/xtypes_enc_items.rs::the_discriminator_of_a_mutable_union_is_must_understand`

**Status:** `done` — gesetzt für `@must_understand`, im Default (`Dialect::Specification`) für Key-Member und für den Diskriminator einer `@mutable`-Union (`MustUnderstandPolicy::DeclaredKeyAndDiscriminator`); die Vendor-Dialekte schreiben, was gemessen ist.

### §7.4.3.4.2 LC 0: 1 Oktett

**Spec:** §7.4.3.4.2, S. 152 — "LC = 0 = 0b000 indicates serialized member
length is 1 Byte".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (Z. 1265);
`crates/wire/src/writer.rs::finish_emheader` (Z. 2321)

**Tests:** `crates/ast-layout/tests/layout.rs::fixed_size_members_map_onto_lc0_through_lc3`

**Status:** done — `finish_emheader` weist einen Wert ab, dessen Länge nicht
zum Code 0 bis 3 passt.

### §7.4.3.4.2 LC 1: 2 Oktette

**Spec:** §7.4.3.4.2, S. 152 — "LC = 1 = 0b001 indicates serialized member
length is 2 Bytes".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (Z. 1265)

**Tests:** `crates/ast-layout/tests/layout.rs::fixed_size_members_map_onto_lc0_through_lc3`

**Status:** done

### §7.4.3.4.2 LC 2: 4 Oktette

**Spec:** §7.4.3.4.2, S. 152 — "LC = 2 = 0b010 indicates serialized member
length is 4 Bytes".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (Z. 1265)

**Tests:** `crates/ast-layout/tests/layout.rs::fixed_size_members_map_onto_lc0_through_lc3`

**Status:** done

### §7.4.3.4.2 LC 3: 8 Oktette

**Spec:** §7.4.3.4.2, S. 152 — "LC = 3 = 0b011 indicates serialized member
length is 8 Bytes".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (Z. 1265)

**Tests:** `crates/ast-layout/tests/layout.rs::fixed_size_members_map_onto_lc0_through_lc3`

**Status:** done

### §7.4.3.4.2 LC 4: Länge = NEXTINT

**Spec:** §7.4.3.4.2, S. 152 — "LC = 4 = 0b100 indicates serialized member
length is NEXTINT".

**Repo:** `crates/wire/src/writer.rs::write_emheader` (Z. 2261);
`crates/wire/src/reader.rs::read_emheader` (Z. 1265)

**Tests:** `crates/wire/tests/wire.rs::the_default_writer_leaves_an_lc4_header_alone`,
`crates/ast-layout/tests/layout.rs::a_union_member_frames_with_an_explicit_nextint`

**Status:** done

### §7.4.3.4.2 LC 5: Länge = NEXTINT, NEXTINT Teil des Members

**Spec:** §7.4.3.4.2, S. 152 — "LC = 5 = 0b101 indicates serialized member
length is also NEXTINT".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (Z. 1265);
`crates/ast-layout/src/xcdr.rs::length_code` (Z. 2149)

**Tests:** `crates/wire/tests/wire.rs::a_length_code_of_five_leaves_its_next_int_for_the_member`,
`crates/ast-layout/tests/layout.rs::a_string_member_frames_as_lc5`

**Status:** done — der Leser nimmt `4 + NEXTINT` als Ausdehnung des Members:
das NEXTINT ist die führende Länge des Members selbst. Der Wortlaut
"serialized member length is also NEXTINT" lässt offen, ob die vier Oktette
mitzählen (in der Befunddatei aufgeführt).

### §7.4.3.4.2 LC 6: Länge = 4 · NEXTINT

**Spec:** §7.4.3.4.2, S. 152 — "LC = 6 = 0b110 indicates serialized member
length is 4*NEXTINT".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (Z. 1265);
`crates/ast-layout/src/xcdr.rs::length_code` (Z. 2149)

**Tests:** `crates/ast-layout/tests/layout.rs::the_sequence_length_code_follows_the_element_width`,
`crates/wire-probe/tests/vendor_bytes.rs::an_aliased_sequence_element_takes_the_same_length_code_as_the_type_it_aliases`

**Status:** done — wie bei LC 5 nimmt der Leser `4 + 4 · NEXTINT`.

### §7.4.3.4.2 LC 7: Länge = 8 · NEXTINT

**Spec:** §7.4.3.4.2, S. 152 — "LC = 7 = 0b111 indicates serialized member
length is 8*NEXTINT".

**Repo:** `crates/wire/src/reader.rs::read_emheader` (Z. 1265);
`crates/ast-layout/src/xcdr.rs::length_code` (Z. 2149)

**Tests:** `crates/ast-layout/tests/layout.rs::the_sequence_length_code_follows_the_element_width`,
`crates/ast-layout/tests/layout.rs::all_eight_length_codes_are_representable`

**Status:** done — der Leser nimmt `4 + 8 · NEXTINT`.

### §7.4.3.4.2 LC 5 bis 7: NEXTINT wird Teil des Members

**Spec:** §7.4.3.4.2, S. 152 — "EMHEADER1 with LC values 5 to 7 also affect the
serialization/deserialization virtual machine in that they cause NEXTINT to be
reused also as part of the serialized member."

**Repo:** `crates/wire/src/writer.rs::write_emheader` (Z. 2261, kein Platzhalter
für LC 5 bis 7); `crates/wire/src/reader.rs::read_emheader` (Z. 1265,
`peek_u32`)

**Tests:** `crates/wire/tests/wire.rs::a_length_code_of_five_leaves_its_next_int_for_the_member`,
`crates/wire-probe/tests/vendor_bytes.rs::every_length_code_matches_the_dialect_s_own_vendor`

**Status:** done — welcher Code gewählt wird, entscheidet
`LengthCodePolicy`; der Default `Compact` folgt Cyclone DDS.

---

## §7.4.3.5 Encoding (serialization) rules

### §7.4.3.5 Form der Regeln, erste passende Regel, Einstieg TOP_LEVEL_TYPE

**Spec:** §7.4.3.5, S. 153 — "When serializing an object each rule is evaluated
in sequence and the first one that has a matching version and criteria is
applied. [...] the entrypoint is a so-called 'Top Level' type [...] Other entry
points are possible if the intent is to simply serialize an object and not
embed it within an RTPS SerializedData."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Notation der Regeln; die Regeln selbst
sind unten einzeln geführt.

### §7.4.3.5.1 Table 40 — Notation der Kriterien

**Spec:** §7.4.3.5.1, Table 40, S. 154–157 — `O:T`, `TOP_LEVEL_TYPE`,
`PRIMITIVE_TYPE`, `STRING_TYPE`, `WSTRING_TYPE`, `ENUM_TYPE`, `BITMASK_TYPE`,
`ALIAS_TYPE`, `ARRAY_TYPE`, `FARRAY_TYPE`, `PARRAY_TYPE`, `SEQUENCE_TYPE`,
`PSEQUENCE_TYPE`, `FSEQUENCE_TYPE`, `MAP_TYPE`, `FMAP_TYPE`, `PMAP_TYPE`,
`UNION_TYPE`, `FUNION_TYPE`, `STRUCT_TYPE`, `FSTRUCT_TYPE`, `MSTRUCT_TYPE`,
`MEMBER`, `FMEMBER`, `OPT_FMEMBER`, `NOPT_FMEMBER`, `MMEMBER`, `FINAL_TYPE`,
`APPENDABLE_TYPE`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Notation. Die `holder_type` von `ENUM_TYPE`
und `BITMASK_TYPE` sind unter Table 32 und Table 33 geführt, die Reihenfolge
mehrdimensionaler Arrays im nächsten Item. Die Regeln verwenden `MUNION_TYPE`,
das Table 40 nicht definiert (in der Befunddatei aufgeführt).

### §7.4.3.5.1 Table 40 — mehrdimensionale Arrays: erster Index am langsamsten

**Spec:** §7.4.3.5.1, Table 40, S. 155 — "Multi-dimensional arrays are treated
for serialization purposes as a single dimensional array containing all the
elements ordered such that the index of the first dimension varies most slowly,
and the index of the last dimension varies most quickly."

**Repo:** `crates/wire-codec/src/encode.rs::write_array_dimension` (Z. 884)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::a_typedef_of_an_array_matches_cyclone_and_rti`

**Status:** done

### §7.4.3.5.2 PLAIN_CDR: optionaler Member mit Short- oder LongMemberHeader

**Spec:** §7.4.3.5.2, S. 158 — "PLAIN_CDR serializes optional members by
prepending either a ShortMemberHeader or a 12 byte LongMemberHeader. [...] The
associated size is set to zero if the optional member is not present [...]
These headers are serialized at a 4-byte offset relative to the current stream
origin (XCDR.origin) and adjust the alignment origin to zero for the
serialization of the member itself."

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (Z. 931, Arm
`MemberHeader::OptionalCdr1`); `crates/wire/src/writer.rs::begin_parameter_value`
(Z. 2541)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_absent_optional_member_round_trips_as_the_absence_the_vendors_write`,
`crates/ast-layout/tests/wire_vectors.rs::v11_optional_member_plans_a_header_regardless_of_presence`

**Status:** done

### §7.4.3.5.2 PLAIN_CDR2 und DELIMITED_CDR: Boolean is_present

**Spec:** §7.4.3.5.2, S. 158 — "PLAIN_CDR2 and DELIMITED_CDR serialize optional
members by first serializing a boolean (<is_present>) [...] set to 0 if the
member is not present and to 1 if it is. [...] If it is not present, the member
shall be omitted from the serialization."

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (Z. 931, Arm
`MemberHeader::OptionalFlag`)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_optional_member_under_xcdr2_is_the_marker_byte_the_three_vendors_write`,
`crates/wire-probe/tests/decode_round_trip.rs::the_optional_marker_is_the_one_the_vendors_write_and_read`

**Status:** done

### §7.4.3.5.2 PL_CDR und PL_CDR2: abwesender optionaler Member ohne Header

**Spec:** §7.4.3.5.2, S. 158 — "PL_CDR and PL_CDR2 serialize optional members as
it would with regular members except that if the optional member is not
present, then the corresponding member header and serialized member are omitted
from the serialized stream."

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::an_absent_optional_member_has_no_parameter_under_pl_cdr`,
`crates/wire-probe/tests/vendor_bytes.rs::an_absent_optional_member_is_not_written`,
`crates/wire-probe/src/decode.rs::tests::an_absent_optional_stays_absent_and_a_present_one_comes_back`

**Status:** `done` — PL_CDR2 und PL_CDR1 lassen Header und Wert eines abwesenden `@optional`-Members weg und lesen ihn als abwesend zurück.

### §7.4.3.5.2 PL_CDR: kein Test für einen abwesenden optionalen Member

**Spec:** §7.4.3.5.2, S. 158 — "if the optional member is not present, then the
corresponding member header and serialized member are omitted".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::an_absent_optional_member_has_no_parameter_under_pl_cdr`

**Status:** `done` — der Test schreibt eine `@mutable`-Struktur mit abwesendem und mit vorhandenem `@optional`-Member unter XCDR1 (`0000040007000000027f0000` und mit dem Parameter für `b`) und liest beide zurück.

---

## §7.4.3.5.3 Complete Serialization Rules
### §7.4.3.5.3 Regel (1) — TOP_LEVEL_TYPE: ENC_HEADER, OPTIONS, Objekt mit ORIGIN=0

**Spec:** §7.4.3.5.3, S. 158 — "(1) XCDR << {O : TOP_LEVEL_TYPE} = XCDR << INIT(
OFFSET=0, ORIGIN=0, CENDIAN=<E>, EVERSION=<eversion> ) << { ENC_HEADER(<E>,
<eversion>, O.type) : Byte[2] } << PUSH( EVERSION = <eversion> ) << PUSH(
MAXALIGN = MAXALIGN(<eversion>) ) << PUSH( ORIGIN = 0 ) << { <OPTIONS> : Byte[2]
} << { O : AsNested(O.type) }".

**Repo:** `crates/wire-codec/src/value.rs::encapsulation_header_in`,
`crates/wire-codec/src/encode.rs::encode_with_padding`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_header_names_the_identifiers_of_table_39_when_asked`,
`crates/wire-probe/tests/vendor_bytes.rs::the_encapsulation_options_carry_the_padding_count`,
`crates/wire-probe/tests/vendor_bytes.rs::a_reader_that_is_told_the_padding_count_stops_at_the_data`

**Status:** `done` — vier Oktette Header (ENC_HEADER, OPTIONS), Ausrichtung ab dem Oktett danach (ORIGIN=0); ENC_HEADER nach Table 60 (Default) oder Table 39, OPTIONS tragen die Zahl der Füllbytes (§7.6.3.1.2).

### §7.4.3.5.3 Regel (2) — PRIMITIVE_TYPE: ALIGN(ssize), ESWAP(AsBytes)

**Spec:** §7.4.3.5.3, S. 158 — "(2) XCDR << {O : PRIMITIVE_TYPE} = XCDR << ALIGN(
O.ssize ) << ESWAP( AsBytes(O) )".

**Repo:** `crates/ast-layout/src/xcdr.rs::primitive_layout` (Z. 456);
`crates/wire/src/writer.rs::align_to` (Z. 1415)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_aligns_every_primitive_to_its_own_size`,
`crates/wire/tests/wire.rs::the_two_byte_orders_write_the_same_value_differently`

**Status:** done — Float128 richtet sich nach Table 31 auf 8 aus, nicht auf
`O.ssize` = 16; beide Regeln zusammen ergeben MIN(16, MAXALIGN) = 8 bzw. 4.

### §7.4.3.5.3 Regel (3) — STRING_TYPE: Länge mit NUL, Oktette mit NUL

**Spec:** §7.4.3.5.3, S. 159 — "(3) XCDR << {O : STRING_TYPE} = XCDR << {
O.ssize : UInt32 } // includes NUL << { O[i] : Byte }* // includes NUL".

**Repo:** `crates/wire/src/writer.rs::write_string` (Z. 1881)

**Tests:** `crates/wire/tests/wire.rs::a_string_length_counts_its_terminator`,
`crates/wire-probe/tests/vendor_bytes.rs::a_string_member_matches_all_three`

**Status:** done

### §7.4.3.5.3 Regel (4) — WSTRING_TYPE: Länge in Oktetten, ohne NUL

**Spec:** §7.4.3.5.3, S. 159 — "(4) XCDR << {O : WSTRING_TYPE} = XCDR << {
O.ssize : UInt32 } // No NUL << { O[i] : Char16 }* // No NUL".

**Repo:** `crates/wire/src/writer.rs::write_wstring` (Z. 1980)

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_wstring_counts_bytes_and_writes_no_terminator`,
`crates/wire-probe/tests/spec_constructs.rs::a_wstring_is_the_same_under_both_encoding_versions`

**Status:** done

### §7.4.3.5.3 Regel (5) — ENUM_TYPE: Wert im holder_type

**Spec:** §7.4.3.5.3, S. 159 — "(5) XCDR << {O : ENUM_TYPE} = XCDR << { O.value :
O.holder_type }".

**Repo:** `crates/wire-codec/src/encode.rs::write_enumeration` (Z. 1550)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::an_enumeration_matches_all_three`,
`crates/wire-probe/tests/vendor_bytes.rs::a_negative_enumerator_is_the_twos_complement_rti_wrote`

**Status:** done

### §7.4.3.5.3 Regel (6) — BITMASK_TYPE: Wert im holder_type

**Spec:** §7.4.3.5.3, S. 159 — "(6) XCDR << {O : BITMASK_TYPE} = XCDR << {
O.value : O.holder_type }".

**Repo:** `crates/wire-codec/src/encode.rs::write_bitmask` (Z. 2040)

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_bitmask_is_the_integer_its_bit_bound_calls_for`

**Status:** done

### §7.4.3.5.3 Regel (7) — ALIAS_TYPE: wie der Basistyp

**Spec:** §7.4.3.5.3, S. 159 — "(7) XCDR << {O : ALIAS_TYPE} = XCDR << { O :
O.base_type }".

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_at_inner` (Z. 736, Arm
`TypeKind::Alias`)

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_typedef_of_a_fixed_writes_what_the_target_writes`,
`crates/wire-probe/tests/vendor_bytes.rs::an_aliased_sequence_element_takes_the_same_length_code_as_the_type_it_aliases`

**Status:** done

### §7.4.3.5.3 Regel (8) — PARRAY_TYPE: nur die Elemente

**Spec:** §7.4.3.5.3, S. 159 — "(8) XCDR << {O : PARRAY_TYPE} = XCDR << { O[i] :
O.element_type }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::array_is_delimited` (Z. 199)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::an_array_of_primitives_matches_all_three`,
`crates/wire/tests/wire.rs::a_sequence_writes_a_count_and_an_array_does_not`

**Status:** done

### §7.4.3.5.3 Regel (9) — ARRAY_TYPE unter Version 2: DHEADER und Elemente

**Spec:** §7.4.3.5.3, S. 160 — "(9) XCDR[2] << {O : ARRAY_TYPE} = XCDR << {
DHEADER(O) : UINT32 } << { O[i] : O.element_type }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::array_is_delimited` (Z. 199);
`crates/ast-layout/src/xcdr.rs::collection_is_delimited` (Z. 89)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::an_array_of_structs_reproduces_each_vendor_under_its_policy`,
`crates/wire-probe/tests/spec_constructs.rs::a_bitset_in_a_collection_is_delimited_the_way_fast_dds_delimits_it`

**Status:** done — der Default `CollectionDelimiterPolicy::Specification`
setzt den DHEADER für jedes nicht-primitive Element; eine Aufzählung zählt
nicht als primitiv. Ein Array von Arrays wird bis zum Basistyp aufgelöst.

### §7.4.3.5.3 Regel (10) — ARRAY_TYPE unter Version 1: nur die Elemente

**Spec:** §7.4.3.5.3, S. 160 — "(10) XCDR[1] << {O : ARRAY_TYPE} = XCDR << {
O[i] : O.element_type }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (Z. 89)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`,
`crates/wire-probe/tests/spec_constructs.rs::a_bitset_in_a_collection_is_delimited_the_way_fast_dds_delimits_it`

**Status:** done

### §7.4.3.5.3 Arrays: APPENDABLE nach (29)/(30), MUTABLE nicht erlaubt

**Spec:** §7.4.3.5.3, S. 160 — "// Arrays with extensibility APPENDABLE use
common APPENDABLE rules: (29)-(30) // Arrays with extensibility MUTABLE are not
allowed. Treated as APPENDABLE."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Kommentar der Regeln. Eine Collection hat
im Modell keine Extensibility; ein Array ist keine Deklaration, an die IDL
`@appendable` oder `@mutable` anbringen kann.

### §7.4.3.5.3 Regel (11) — PSEQUENCE_TYPE: Länge und Elemente

**Spec:** §7.4.3.5.3, S. 160 — "(11) XCDR << { O : PSEQUENCE_TYPE } = XCDR << {
O.length : UInt32 } << { O[i] : O.element_type }*".

**Repo:** `crates/wire/src/writer.rs::write_sequence`

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::a_sequence_of_a_primitive_matches_all_three`,
`crates/wire/tests/wire.rs::a_sequence_writes_a_count_and_an_array_does_not`

**Status:** done

### §7.4.3.5.3 Regel (12) — SEQUENCE_TYPE unter Version 2: DHEADER, Länge, Elemente

**Spec:** §7.4.3.5.3, S. 160 — "(12) XCDR[2] << {O : SEQUENCE_TYPE} = XCDR << {
DHEADER(O) : UINT32 } << { O.length : UINT32 } << { O[i] : O.element_type }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (Z. 89)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_sequence_of_sequences_writes_the_bytes_cyclone_and_fast_dds_write`,
`crates/wire-probe/tests/vendor_bytes.rs::a_collection_of_structs_reproduces_each_vendor_under_its_policy`

**Status:** done

### §7.4.3.5.3 Regel (13) — SEQUENCE_TYPE unter Version 1: Länge und Elemente

**Spec:** §7.4.3.5.3, S. 160–161 — "(13) XCDR[1] << {O : SEQUENCE_TYPE} = XCDR <<
{ O.length : UInt32 } << { O[i] : O.element_type }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::collection_is_delimited` (Z. 89)

**Tests:** `crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`,
`crates/wire-probe/tests/spec_constructs.rs::a_bitset_in_a_collection_is_delimited_the_way_fast_dds_delimits_it`

**Status:** done

### §7.4.3.5.3 Regel (14) — PMAP_TYPE: Länge und Paare

**Spec:** §7.4.3.5.3, S. 161 — "(14) XCDR << {O : PMAP_TYPE} = XCDR << { O.length
: UInt32 } << { (O[i].key : O.key_type), (O[i].element : O.element_type) }*".

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_at_inner` (Z. 736, Arm
`TypeKind::Map`); `crates/ast-layout/src/key.rs::MapDelimiter` (Z. 464)

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_map_of_primitives_is_a_length_and_the_pairs`,
`crates/wire-conformance/tests/map_bytes.rs::a_map_is_delimited_only_where_rule_14_does_not_exempt_it`

**Status:** done — im Default `MapDelimiter::EitherHalf` macht ein
nicht-primitiver Schlüssel (`string`) die Map zur Regel (15).

### §7.4.3.5.3 Regel (15) — MAP_TYPE unter Version 2: DHEADER und Paare

**Spec:** §7.4.3.5.3, S. 161 — "(15) XCDR[2] << { O : MAP_TYPE } = XCDR << {
DHEADER(O) : UINT32 } << { (O[i].key : O.key_type), (O[i].element :
O.element_type) }*".

**Repo:** `crates/wire-codec/src/encode.rs::write_map`,
`crates/wire-codec/src/value.rs::MapLengthPolicy`

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_map_with_a_structured_value_is_delimited_under_xcdr2`,
`crates/wire-probe/tests/spec_constructs.rs::the_two_readings_of_a_delimited_maps_length_differ_by_four_bytes`,
`crates/wire-probe/tests/spec_constructs.rs::each_reading_of_a_delimited_maps_length_is_the_bytes_of_one_implementation`

**Status:** `done` — Default ist die Lesart der aufgezeichneten Herstellerzeilen: unter XCDR2 schreibt eine delimited Map nach dem DHEADER die Anzahl der Einträge (Fast DDS, `vendor-corpus.tsv`, 24 Zeilen `map-*`; Äquivalenz zu `sequence<MapEntry>` aus §7.4.1.1.4 mit Regel (12)). Für OpenDDS, das die Anzahl auslässt, gibt es keine Zeile im Korpus (gemessen in `unspecified_constructs.md`). Regel (15) wörtlich (DHEADER, Paare, keine Anzahl) ist die benannte Einstellung `MapLengthPolicy::Rule15Omits` (`--map-length rule-15-omits`, `Dialect::OpenDds`). Gewählte Lesart in der Befunddatei unter DDSXTY14-72.

### §7.4.3.5.3 Regel (16) — MAP_TYPE unter Version 1: Länge und Paare

**Spec:** §7.4.3.5.3, S. 161 — "(16) XCDR[1] << {O : MAP_TYPE} = XCDR << {
O.length : UInt32 } << { (O[i].key : O.key_type), (O[i].element :
O.element_type) }*".

**Repo:** `crates/wire-codec/src/encode.rs::write_map` (Z. 2221)

**Tests:** `crates/wire-probe/tests/spec_constructs.rs::a_map_with_a_structured_value_is_delimited_under_xcdr2`,
`crates/wire-conformance/tests/map_bytes.rs::the_bytes_are_the_recorded_fast_dds_row`

**Status:** done

### §7.4.3.5.3 Regel (17) — FSTRUCT_TYPE: Member der Reihe nach

**Spec:** §7.4.3.5.3, S. 162 — "(17) XCDR << {O : FSTRUCT_TYPE} = XCDR << {
O.member[i] : FMEMBER }*".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (Z. 931)

**Tests:** `crates/wire-conformance/tests/bytes.rs::a_final_struct_of_a_long_and_a_double_encodes_to_sixteen_bytes`,
`crates/wire-probe/tests/vendor_bytes.rs::point_matches_cyclone_rti_and_fastdds`

**Status:** done

### §7.4.3.5.3 Regel (18) — NOPT_FMEMBER: nur der Wert

**Spec:** §7.4.3.5.3, S. 162 — "(18) XCDR << {M : NOPT_FMEMBER} = XCDR << {
M.value : M.value.type }".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (Z. 931, Arm
`MemberHeader::NoHeader`)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::the_mixed_primitive_struct_matches_all_three`

**Status:** done

### §7.4.3.5.3 Regel (19) — OPT_FMEMBER unter Version 1: wie MMEMBER

**Spec:** §7.4.3.5.3, S. 162 — "(19) XCDR[1] << {M : OPT_FMEMBER} = XCDR << { M :
MMEMBER }".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (Z. 931, Arm
`MemberHeader::OptionalCdr1`)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_absent_optional_member_round_trips_as_the_absence_the_vendors_write`

**Status:** done — ein abwesender Member hat einen Header mit Länge 0
(§7.4.1.2).

### §7.4.3.5.3 Regel (20) — OPT_FMEMBER unter Version 2: is_present, dann Wert

**Spec:** §7.4.3.5.3, S. 162 — "(20) XCDR[2] << {M : OPT_FMEMBER} = XCDR << {
<is_present> : BOOLEAN } << IF (<is_present>) { M.value : M.value.type }".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (Z. 931, Arm
`MemberHeader::OptionalFlag`)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_optional_member_under_xcdr2_is_the_marker_byte_the_three_vendors_write`

**Status:** done

### §7.4.3.5.3 Regel (21) — MSTRUCT_TYPE unter Version 2: DHEADER und MMEMBER

**Spec:** §7.4.3.5.3, S. 163 — "(21) XCDR[2] << {O : MSTRUCT_TYPE} = XCDR << {
DHEADER(O) : UInt32 } << { O.member[i] : MMEMBER }*".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (Z. 931);
`crates/ast-layout/src/xcdr.rs::member_header` (Z. 2018)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v10_mutable_plans_the_oracle_member_id_and_length_code_sequence`,
`crates/ast-layout/tests/wire_vectors.rs::v12_empty_mutable_plans_a_dheader_and_no_members`

**Status:** done

### §7.4.3.5.3 Regel (22) — MMEMBER unter Version 2: EMHEADER1, NEXTINT, Wert

**Spec:** §7.4.3.5.3, S. 163 — "(22) XCDR[2] << {M : MMEMBER} = XCDR << {
EMHEADER1(M) : UInt32 } << IF (LC(M)>=4) { NEXTINT(M) : UInt32 } << IF
(LC(M)>=5) XCDR.offset = XCDR.offset-4 << { M.value : M.value.type }".

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (Z. 931, Arm
`MemberHeader::EmHeader`); `crates/wire/src/writer.rs::write_emheader`
(Z. 2261)

**Tests:** `crates/wire/tests/wire.rs::a_length_code_of_five_leaves_its_next_int_for_the_member`,
`crates/wire-probe/tests/vendor_bytes.rs::every_length_code_matches_the_dialect_s_own_vendor`

**Status:** done — zu `M_FLAG` siehe §7.4.3.4.2.

### §7.4.3.5.3 Regel (23) — MSTRUCT_TYPE unter Version 1: Parameter, dann Sentinel

**Spec:** §7.4.3.5.3, S. 163 — "(23) XCDR[1] << {O : MSTRUCT_TYPE} = XCDR << {
O.member[i] : MMEMBER }* << { PID_SENTINEL : UInt16 } << { length = 0 : UInt16
}".

**Repo:** `crates/wire/src/writer.rs::write_sentinel` (Z. 2664)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::a_mutable_struct_plans_the_pid_list_end_sentinel`,
`crates/wire/tests/wire.rs::the_sentinel_ends_a_parameter_list`

**Status:** done — geschrieben wird `PID_LIST_END` (`0x3F02`) nach Table 34.
Die Regel nennt `PID_SENTINEL`, das in DDSI-RTPS den Wert `0x0001` hat
(Widerspruch der Spec, in der Befunddatei aufgeführt).

### §7.4.3.5.3 Regel (24) — MMEMBER unter Version 1, kurze Form

**Spec:** §7.4.3.5.3, S. 163 — "// using short PL encoding when both M.id <= 2^14
and M.value.ssize <= 2^16 (24) XCDR[1] << {M : MMEMBER} = XCDR << ALIGN(4) << {
FLAG_I + FLAG_M + M.id : UInt16 } << { M.value.ssize : UInt16 } << PUSH(
ORIGIN=0 ) << { M.value : M.value.type }".

**Repo:** `crates/ast-layout/src/xcdr.rs::needs_extended_pid` (Z. 2110);
`crates/wire/src/writer.rs::write_pid` (Z. 2422)

**Tests:** `crates/ast-layout/tests/xcdr1_gate.rs::the_last_id_the_short_header_holds_is_the_threshold_itself`,
`crates/wire/tests/wire.rs::a_value_that_fits_sixteen_bits_keeps_the_short_header`

**Status:** done — die kurze Form gilt bis ID `0x3F00` und Länge `0xFFFF`
(§7.4.1.2.2, Breite des Felds). Der Kommentar der Regel nennt `2^14` und
`2^16`; `2^16` passt nicht in ein UInt16, und IDs ab `0x3F01` sind
reserviert (Widerspruch der Spec, in der Befunddatei aufgeführt). Die Länge
wird im Default auf vier aufgerundet (offen unter §7.4.1.2).

### §7.4.3.5.3 Regel (25) — MMEMBER unter Version 1, lange Form

**Spec:** §7.4.3.5.3, S. 164 — "(25) XCDR[1] << {M : MMEMBER} = XCDR << ALIGN(4)
<< { FLAG_I + FLAG_M + PID_EXTENDED : UInt16 } << { slength=8 : UInt16 } << {
M.id : UInt32 } << { M.value.ssize : UInt32 } << PUSH( ORIGIN=0 ) << { M.value
: M.value.type }".

**Repo:** `crates/wire/src/writer.rs::write_pid_extended`;
`crates/wire/src/writer.rs::promote_pid` (Z. 2628)

**Tests:** `crates/wire/tests/wire.rs::the_extended_parameter_form_carries_a_thirty_two_bit_id`,
`crates/wire/tests/wire.rs::a_short_parameter_header_becomes_extended_when_the_value_overruns_it`

**Status:** done — das 32-Bit-Wort trägt die Flags von §7.4.1.2.1 über der
ID; Regel (25) schreibt `M.id` ohne sie.

### §7.4.3.5.3 Regel (26) — FUNION_TYPE: Diskriminator, gewählter Member falls vorhanden

**Spec:** §7.4.3.5.3, S. 164 — "(26) XCDR << {O : FUNION_TYPE} = XCDR << { O.disc
: NOPT_FMEMBER } << { O.selected_member : FMEMBER }?".

**Repo:** `crates/wire-codec/src/encode.rs::write_union` (Z. 1167)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::a_union_in_a_struct_matches_all_three`,
`crates/wire-probe/src/decode.rs::tests::a_union_with_no_default_and_no_matching_label_carries_no_member`

**Status:** done

### §7.4.3.5.3 Regel (27) — MUNION_TYPE unter Version 2: DHEADER, Diskriminator, Member

**Spec:** §7.4.3.5.3, S. 164 — "(27) XCDR[2] << {O : MUNION_TYPE} = XCDR << {
DHEADER(O) : UInt32 } << { O.disc : MMEMBER } << { O.selected_member : MMEMBER
}?".

**Repo:** `crates/wire-codec/src/encode.rs::write_union`,
`crates/wire-codec/src/encode.rs::write_mmember`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_discriminator_of_a_mutable_union_is_must_understand`,
`crates/wire-probe/tests/decode_round_trip.rs::a_mutable_union_frames_its_discriminator_and_its_member`,
`crates/wire-probe/tests/opendds_mutable_framing.rs::the_union_rows_are_reproduced`

**Status:** `done` — DHEADER, Diskriminator mit Member-ID 0 und gewählter Member je mit EMHEADER; der EMHEADER des Diskriminators trägt im Default `M_FLAG` (`0xA0000000` für einen `long`), in den Dialekten `Rti`, `FastDds` und `OpenDds` nicht, wie gemessen.

### §7.4.3.5.3 Regel (28) — MUNION_TYPE unter Version 1: Parameter, dann Sentinel

**Spec:** §7.4.3.5.3, S. 165 — "(28) XCDR[1] << {O : MUNION_TYPE} = XCDR << {
O.disc : MMEMBER } << { O.selected_member : MMEMBER }? << { PID_SENTINEL :
UInt16 } << { length = 0 : UInt16 }".

**Repo:** `crates/wire-codec/src/encode.rs::write_union`,
`crates/wire/src/writer.rs::write_sentinel`

**Tests:** `crates/wire-probe/tests/xtypes_enc_items.rs::the_discriminator_of_a_mutable_union_is_must_understand`,
`crates/wire-probe/tests/decode_round_trip.rs::a_mutable_union_frames_its_discriminator_and_its_member`

**Status:** `done` — Diskriminator und gewählter Member als Parameter, dann `PID_LIST_END` (`0x7f02`, Table 34; die Regel schreibt `PID_SENTINEL`, siehe Regel (23)). Der Parameter des Diskriminators trägt im Default `FLAG_MUST_UNDERSTAND` (`0x4000`). Der Kommentar vor der Regel verweist auf "(25)-(26)" statt auf (24) und (25) (Befunddatei).

### §7.4.3.5.3 Regel (29) — APPENDABLE_TYPE unter Version 1: AsFinal

**Spec:** §7.4.3.5.3, S. 165 — "(29) XCDR[1] << {O : APPENDABLE_TYPE} = XCDR << {
O : AsFinal(O.type) }".

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_aggregate` (Z. 1720)

**Tests:** `crates/ast-layout/tests/vendor_gate.rs::appendable_adds_a_delimiter_under_xcdr2_and_nothing_under_xcdr1`,
`crates/ast-layout/tests/layout.rs::xcdr1_plans_no_dheader_at_all`

**Status:** done

### §7.4.3.5.3 Regel (30) — APPENDABLE_TYPE unter Version 2: DHEADER und AsFinal

**Spec:** §7.4.3.5.3, S. 165 — "(30) XCDR[2] << {O : APPENDABLE_TYPE} = XCDR << {
DHEADER(O) : UInt32 } << { O : AsFinal(O.type) }".

**Repo:** `crates/ast-layout/src/xcdr.rs::plan_aggregate` (Z. 1720);
`crates/wire/src/writer.rs::write_dheader` (Z. 2233)

**Tests:** `crates/wire-conformance/tests/bytes.rs::an_appendable_struct_writes_a_dheader_with_the_length_that_followed`,
`crates/ast-layout/tests/wire_vectors.rs::v9_appendable_plans_a_dheader`

**Status:** done

---

## §7.4.4 XML Data Representation

### §7.4.4 Ein XML-Dokument je Sample, Struktur nach dem XSD des Typs

**Spec:** §7.4.4, S. 165 — "Each data sample shall constitute a separate XML
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

**Status:** done — Ein Sample ist ein eigenes XML-Dokument: das Wurzelelement trägt den Namen des Typs, die Member eines Aggregats sind Elemente ihres Namens, die Elemente einer Sequenz und eines Arrays `<item>`, ein Eintrag einer Map `<item>` mit `<key>` und `<value>`, eine Union `<discriminator>` und das Element des gewählten Members, eine Bitmask die Namen der gesetzten Flags, durch Leerzeichen getrennt; ein abwesender optionaler Member fehlt. `xml_data::to_xml` schreibt das Dokument, `from_xml` liest es gegen den Typ; ein DataWriter mit XML als erster Darstellung sendet es hinter `{0x00, 0x04}` (`xml_wire_body`), ein DataReader liest es (`xml_sample_as_cdr`). Ein Dokument mit einem anderen Wurzelelement, einem unbekannten oder doppelten Element oder einem Wert, der nicht zum Typ passt, wird abgewiesen und als `reader.sample.encapsulation_refused` gemeldet. `crates/xml` (`serialize_sample` mit `<sample type_ref>`) ist der Pfad von DDS-XML 1.0 und nicht der der Leitung.

### §7.4.4 Mutable ohne eigene Form

**Spec:** §7.4.4, S. 165 — "(Note that, unlike in the CDR Representation, samples
of mutable types are serialized no differently than samples of final or
appendable types.)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Anmerkung.

### §7.4.4 Zwei Varianten, Beispieltypen

**Spec:** §7.4.4, S. 165–166 — "The XML Data Representation has two variants: the
Valid XML Data Representation and the Well-formed XML Data Representation. [...]
non-normative example type definitions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung; die Varianten sind die beiden
folgenden Items.

### §7.4.4.1 Valid: Namespace deklariert, ddstype://www.omg.org/<module path>

**Spec:** §7.4.4.1, S. 166 — "The XML document shall declare the namespace(s)
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

**Status:** done — `serialize_data_representation` mit `DataRepresentationForm::Valid` deklariert `ddstype://www.omg.org/<module path>` als Standard-Namespace des Wurzelelements, die Module von außen nach innen durch `/` getrennt (`implied_namespace`); `parse_data_representation` weist einen anderen Namespace ab und meldet die Variante. Ein ausdrücklicher Ziel-Namespace des XSD wird nicht gelesen, weil `zerodds-xml` das XSD nicht in dieser Form führt.

### §7.4.4.2 Well-formed: kein Namespace

**Spec:** §7.4.4.2, S. 166 — "The XML document shall not declare the
namespace(s) against which it may be validated, regardless of whether a target
namespace was specified in the XSD Type Representation".

**Repo:** `crates/xml/src/sample.rs::serialize_data_representation`

**Tests:** `crates/xml/tests/types_and_samples.rs::t20_sample_roundtrip`,
`crates/xml/src/sample.rs::tests::serialize_sample_uses_item_tag_for_sequence`,
`crates/xml/src/sample.rs::tests::the_well_formed_form_has_the_type_as_root_and_no_namespace`

**Status:** done — `DataRepresentationForm::WellFormed` schreibt kein `xmlns`, das Wurzelelement trägt den Namen des Typs.

### §7.4.4.2 Non-normative note: default namespace

**Spec:** §7.4.4.2, S. 167 — "Non-normative note: Valid XML data representation
can be nearly as compact [...] by using a default namespace."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — „Non-normative note“.

---

## §7.5 Language Binding

### §7.5 Zwei Bindungen: Plain Language Binding und Dynamic Language Binding

**Spec:** §7.5, S. 167–168 — "The specification defines two language bindings:
Plain Language Objects and Dynamic Data."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung; die Anforderungen stehen in §7.5.1
und §7.5.2.

### §7.5 Table 41 — Plain Language Binding

**Spec:** §7.5, Table 41, S. 168 — "Plain Language Binding | Each data type is
mapped into the most natural 'native' construct in the programming language of
choice."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Vor- und Nachteile.

### §7.5 Table 41 — Dynamic Language Binding

**Spec:** §7.5, Table 41, S. 168 — "Dynamic Language Binding | All data types
are mapped into a single Language 'Dynamic Data' construct".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Vor- und Nachteile.

---

## §7.5.1 Plain Language Binding

### §7.5.1 Schritte: IDL nach §7.3.1, OMG-IDL-Sprachabbildung, sprachspezifische Transformation

**Spec:** §7.5.1, S. 169 — "This mapping reuses the OMG-standard IDL language
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

**Status:** done — `zerodds-lang-rules` erzeugt die Bindungen je Sprache nach der Abbildung der OMG: C++ standardmäßig als DDS-PSM-Cxx (`std::map`, `dds::core::optional`; `--cpp-mapping psm`), als klassische Abbildung nach [C++-MAP] (OMG IDL to C++ 1.3, C++03; `--cpp-mapping classic`) und als native Struktur der Codecs (`--cpp-mapping native`); Java mit `octet` als `byte`, `fixed` als `BigDecimal`, der Aufzählung als Klasse nach CORBA 3.3 Teil 3 §1.5.9, Union mit Zugriffsfunktionen und Map als `java.util.Map`; C mit Map und Sequenz nach [C-MAP] und §7.5.1.3.2. Log: `.agent/REPORT.md`.

### §7.5.1 C: Sequenz nach [C-MAP]

**Spec:** §7.5.1, S. 169 — "This mapping reuses the OMG-standard IDL language
mappings [C-MAP, C++-MAP, JAVA-MAP]."

**Repo:** `crates/lang-rules/src/adapter/c.rs::sequence_decl`, `crates/lang-rules/src/emit/c.rs::make_room`, `crates/lang-rules/src/default_c.rs`, `tools/idlc/src/c_support.rs::buffer_walks`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_sequence_becomes_a_length_and_a_buffer`,
`tools/idlc/tests/c_typesupport.rs` (die Harness prüft Kapazität und `_release` einer dekodierten und einer freigegebenen Sequenz),
`crates/wire-probe/tests/three_column_gate.rs::the_three_columns_agree`

**Status:** done — `sequence<T>` ist `struct { uint32_t _maximum; uint32_t _length; T *_buffer; bool _release; }`; der Leser setzt die Kapazität und `_release = false` (die Arena gehört nicht der Sequenz), der Standardwert und die Freigabe setzen und löschen beide, die Proben und die Harness füllen sie. Log: `.agent/REPORT.md`.

### §7.5.1 Sprach-PSM darf die Bindung überschreiben

**Spec:** §7.5.1, S. 169 — "Note that any of the following language bindings may
be overridden in a language-specific PSM, such as [DDS-PSM-CXX]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis.

## §7.5.1.1 Primitive Types

### §7.5.1.1.1 Table 42 — Int32: DDS_Int32

**Spec:** §7.5.1.1.1, Table 42, S. 169 — "Int32 | DDS_Int32 | int32_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_Int32`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — UInt32: DDS_UInt32

**Spec:** §7.5.1.1.1, Table 42, S. 169 — "UInt32 | DDS_UInt32 | uint32_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_UInt32`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — Int8: DDS_Int8

**Spec:** §7.5.1.1.1, Table 42, S. 169 — "Int8 | DDS_Int8 | int8_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_Int8`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — UInt8: DDS_UInt8

**Spec:** §7.5.1.1.1, Table 42, S. 169 — "UInt8 | DDS_UInt8 | uint8_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_UInt8`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — Int16: DDS_Int16

**Spec:** §7.5.1.1.1, Table 42, S. 169 — "Int16 | DDS_Int16 | int16_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_Int16`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — UInt16: DDS_UInt16

**Spec:** §7.5.1.1.1, Table 42, S. 170 — "UInt16 | DDS_UInt16 | uint16_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_UInt16`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — Int64: DDS_Int64

**Spec:** §7.5.1.1.1, Table 42, S. 170 — "Int64 | DDS_Int64 | int64_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_Int64`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — UInt64: DDS_UInt64

**Spec:** §7.5.1.1.1, Table 42, S. 170 — "UInt64 | DDS_UInt64 | uint64_t".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_UInt64`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — Float32: DDS_Float32

**Spec:** §7.5.1.1.1, Table 42, S. 170 — "Float32 | DDS_Float32 |
(unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_Float32`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — Float64: DDS_Float64

**Spec:** §7.5.1.1.1, Table 42, S. 170 — "Float64 | DDS_Float64 |
(unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_Float64`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — Float128: DDS_Float128

**Spec:** §7.5.1.1.1, Table 42, S. 170 — "Float128 | DDS_Float128 |
(unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_Float128`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — Char8: DDS_Char8

**Spec:** §7.5.1.1.1, Table 42, S. 170 — "Char8 | DDS_Char8 | (unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_Char8`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — Char16: DDS_Char16

**Spec:** §7.5.1.1.1, Table 42, S. 170 — "Char16 | DDS_Char16 | (unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_Char16`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — Boolean: DDS_Boolean

**Spec:** §7.5.1.1.1, Table 42, S. 170 — "Boolean | DDS_Boolean | _Bool".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_Boolean`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Table 42 — Byte: DDS_Byte

**Spec:** §7.5.1.1.1, Table 42, S. 170 — "Byte | DDS_Byte | (unspecified)".

**Repo:** `runtime/c/include/zerodds_wire.h`

**Tests:** `runtime/c/tests/selftest.c::the_primitive_typedefs_of_table_42_exist`

**Status:** done — `runtime/c/include/zerodds_wire.h` deklariert `DDS_Byte`; der erzeugte Code darf nach §7.5.1.1.1 die nativen oder die C99-Typen schreiben und schreibt `int32_t` usw.

### §7.5.1.1.1 Freiheitsgrad: native oder C99-Typen im erzeugten Code

**Spec:** §7.5.1.1.1, S. 169 — "On platforms compliant with the C99
specification, the implementation may generate equivalent C99-compatible types."

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_primitive` (Z. 872)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::every_idl_primitive_reaches_a_c_type`

**Status:** done

### §7.5.1.1.1 Boolean: nur 0 und 1 definiert

**Spec:** §7.5.1.1.1, S. 170 — "With respect to DDS::Boolean, only the values 0
and 1 are defined. Other values result in unspecified behavior."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung an die Implementierung; das
Lesen eines anderen Oktetts ist unter Table 31 geführt.

### §7.5.1.1.1 Char16: wchar_t oder ein Ganzzahltyp

**Spec:** §7.5.1.1.1, S. 170 — "compliant implementations may consider wchar_t
to be an equivalent C type if the platform supports it and it is of sufficient
size. Otherwise, they may map Char16 to an equivalent integer type."

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_primitive` (Z. 872, Arm `PrimitiveKind::WChar`)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::every_idl_primitive_reaches_a_c_type`

**Status:** done — `uint16_t`.

### §7.5.1.1.2 Table 43 — Int32: DDS::Int32

**Spec:** §7.5.1.1.2, Table 43, S. 170 — "Int32 | DDS::Int32 | [std::]int32_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::Int32`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — UInt32: DDS::UInt32

**Spec:** §7.5.1.1.2, Table 43, S. 170 — "UInt32 | DDS::UInt32 |
[std::]uint32_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::UInt32`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — Int8: DDS::Int8

**Spec:** §7.5.1.1.2, Table 43, S. 170 — "Int8 | DDS::Int8 | [std::]int8_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::Int8`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — UInt8: DDS::UInt8

**Spec:** §7.5.1.1.2, Table 43, S. 170 — "UInt8 | DDS::UInt8 | [std::]uint8_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::UInt8`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — Int16: DDS::Int16

**Spec:** §7.5.1.1.2, Table 43, S. 170 — "Int16 | DDS::Int16 | [std::]int16_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::Int16`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — UInt16: DDS::UInt16

**Spec:** §7.5.1.1.2, Table 43, S. 170 — "UInt16 | DDS::UInt16 |
[std::]uint16_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::UInt16`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — Int64: DDS::Int64

**Spec:** §7.5.1.1.2, Table 43, S. 171 — "Int64 | DDS::Int64 | [std::]int64_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::Int64`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — UInt64: DDS::UInt64

**Spec:** §7.5.1.1.2, Table 43, S. 171 — "UInt64 | DDS::UInt64 |
[std::]uint64_t".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::UInt64`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — Float32: DDS::Float32

**Spec:** §7.5.1.1.2, Table 43, S. 171 — "Float32 | DDS::Float32 |
(unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::Float32`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — Float64: DDS::Float64

**Spec:** §7.5.1.1.2, Table 43, S. 171 — "Float64 | DDS::Float64 |
(unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::Float64`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — Float128: DDS::Float128

**Spec:** §7.5.1.1.2, Table 43, S. 171 — "Float128 | DDS::Float128 |
(unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::Float128`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — Char8: DDS::Char8

**Spec:** §7.5.1.1.2, Table 43, S. 171 — "Char8 | DDS::Char8 | (unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::Char8`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — Char16: DDS::Char16

**Spec:** §7.5.1.1.2, Table 43, S. 171 — "Char16 | DDS::Char16 | (unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::Char16`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — Boolean: DDS::Boolean

**Spec:** §7.5.1.1.2, Table 43, S. 171 — "Boolean | DDS::Boolean | bool or
_Bool".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::Boolean`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Table 43 — Byte: DDS::Byte

**Spec:** §7.5.1.1.2, Table 43, S. 171 — "Byte | DDS::Byte | (unspecified)".

**Repo:** `runtime/cpp/include/dds/types.hpp`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_primitive_typedefs_of_table_43_exist`

**Status:** done — `runtime/cpp/include/dds/types.hpp` deklariert `DDS::Byte`; der erzeugte Code darf nach §7.5.1.1.2 die nativen oder die C99-Typen schreiben und schreibt `std::int32_t` usw.

### §7.5.1.1.2 Freiheitsgrad: native oder C99-Typen im erzeugten C++

**Spec:** §7.5.1.1.2, S. 170 — "On platforms compliant with the C99
specification, the implementation may generate equivalent C99-compatible types."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::cpp_primitive`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::the_integers_are_spelled_with_the_c99_types_of_table_43`; `crates/lang-rules/src/adapter/cpp.rs::tests::a_long_double_is_the_runtime_type_and_never_the_compiler_s`

**Status:** done — die ganzzahligen Typen sind `std::int8_t` bis `std::uint64_t`; Log: `.agent/REPORT.md`.

### §7.5.1.1.2 Boolean: 0, 1, true, false

**Spec:** §7.5.1.1.2, S. 171 — "With respect to DDS::Boolean, only the values 0
and 1 are defined. Alternatively, the C++ keywords true and false may be used."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung an die Implementierung.

### §7.5.1.1.2 Char16 darf von Ganzzahltypen ununterscheidbar sein

**Spec:** §7.5.1.1.2, S. 171 — "they may map Char16 to an equivalent integer
type. This means that DDS::Char16 may not be distinguishable from integer types
for purposes of overloading."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::cpp_primitive` (Z. 858, Arm `PrimitiveKind::WChar`)

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::the_tag_keeps_apart_what_the_cpp_spelling_collapses`

**Status:** done — `std::uint16_t`, wie `UInt16`.

### §7.5.1.1.2 Überladung: nur Boolean, Char8 und Byte dürfen zusammenfallen

**Spec:** §7.5.1.1.2, S. 171 — "Types DDS::Boolean, DDS::Char8, and DDS::Byte
may all map to the same underlying C++ type. [...] All other mappings for basic
types shall be distinguishable for the purposes of overloading."

**Repo:** `runtime/cpp/include/zerodds/wire.hpp::Octet`; `crates/lang-rules/src/adapter/cpp.rs::octet`

**Tests:** `runtime/cpp/tests/selftest.cpp::the_types_of_table_43_are_distinguishable_for_overloading`; `tools/idlc/tests/cpp_inheritance.rs::a_derived_struct_inherits_and_the_octet_is_its_own_type`

**Status:** done — `DDS::Byte` und `octet` sind `zerodds::wire::Octet`, ein eigener Typ; `UInt8` ist `std::uint8_t`; Überladungen auf beide werden unterschieden; Log: `.agent/REPORT.md`.

---

## §7.5.1.2 Annotations and Built-in Annotations

### §7.5.1.2.1.1 C: Aufzählung als #define mit den tatsächlichen Werten

**Spec:** §7.5.1.2.1.1, S. 171 — "[C-MAP] transforms an IDL enumeration into a
series of #define directives [...] The values to which these definitions
correspond shall be the actual values of the enumerated literals [...], whether
implicitly or explicitly defined."

**Repo:** `crates/lang-rules/src/adapter/c.rs::declare` (Z. 937)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::an_enumeration_becomes_an_integer_and_constants_not_a_c_enum`

**Status:** done — `#define ACTIVE ((Mode)4)` nach `@value(4)`, `FAULT` danach
`5`.

### §7.5.1.2.1.2 C++: Enumerator mit = und dem Wert

**Spec:** §7.5.1.2.1.2, S. 171–172 — "for any enumerated literal in IDL that
bears the Value annotation, the corresponding C++ enumerated literal definition
shall be followed by an equals sign ('=') and the value of the data member of
the annotation."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::declare` (Z. 1203)

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::cpp_follows_an_enumerator_with_a_value_by_an_equals_sign_and_the_value`,
`crates/lang-rules/src/adapter/cpp.rs::tests::an_enumeration_takes_an_underlying_type_from_its_bit_bound`

**Status:** done — `enum Mode { IDLE, @value(4) ACTIVE, FAULT, @value(70000) HIGH }` ergibt `ACTIVE = 4`, `FAULT = 5` und `HIGH = 70000`; jeder Enumerator trägt `=` und seinen Wert.

### §7.5.1.2.1.3 Java: public static final int _<label> = <value>

**Spec:** §7.5.1.2.1.3, S. 172 — "The value of each IDL enumerated literal is
given in a Java integer constant of the following form: public static final int
_<label> = <value>; [...] that numeric value shall be set according to the
explicit or implicit value".

**Repo:** `crates/lang-rules/src/adapter/java.rs::enumeration_class_body`

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::java_has_an_int_constant_with_an_underscore_and_an_object_per_literal`,
`crates/lang-rules/src/adapter/java.rs::tests::an_explicit_enumerator_value_is_written_out`,
`crates/lang-rules/src/adapter/java.rs::tests::an_enumeration_is_a_class_of_instances_and_a_member_of_it_is_one`

**Status:** done — je Literal `public static final int _<label> = <Wert>` (der explizite oder implizite Wert) und ein Objekt `<label>` der Klasse nach CORBA 3.3 Teil 3 §1.5.9 mit `value()` und `from_int`; ein Member der Aufzählung ist eine Instanz dieser Klasse.

### §7.5.1.2.2 Bitmask: wie eine Aufzählung <TypeName>Bits mit @value(1 << position)

**Spec:** §7.5.1.2.2, S. 172 — "For each bitmask type defining flags FLAG_0
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

**Status:** done — C: Ganzzahltyp `<TypeName>Bits` mit `#define` je Flag; C++: `enum class <TypeName>Bits : <Halter>` mit `= 1 << position` je Flag; Java: typsichere Klasse `<TypeName>Bits` mit `_<label>` und Objekt je Flag. Ein Typ mit dem Namen der Flag-Aufzählung im selben Namensraum ist in C, C++ und Java eine Kollision (`SpellingCollision`).

### §7.5.1.2.2 Bitmask: typedef auf den vorzeichenlosen Ganzzahltyp

**Spec:** §7.5.1.2.2, S. 172 — "the language binding shall be as if there was a
typedef like the following [...] typedef <unsigned_integer_equivalent>
<TypeName>;"

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_holder` (Z. 897);
`crates/lang-rules/src/adapter/cpp.rs::cpp_holder` (Z. 884)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_bitmask_becomes_an_integer_and_one_constant_per_bit`

**Status:** done — C `typedef uint16_t Flags;`, C++ ein Alias; Java hat keinen
typedef und verwendet den Ganzzahltyp am Member.

### §7.5.1.2.2 Table 44 — Bound 1–8: uint8

**Spec:** §7.5.1.2.2, Table 44, S. 172 — "1–8 | uint8".

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_holder` (Z. 897);
`crates/lang-rules/src/adapter/cpp.rs::cpp_holder` (Z. 884)

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::table_44_one_to_eight_bits_is_an_8_bit_unsigned_integer`,
`crates/lang-rules/src/adapter/java.rs::tests::a_bitmask_takes_the_holder_its_bound_calls_for`

**Status:** done — `@bit_bound(8)` ergibt `typedef uint8_t` in C, `using … = std::uint8_t` und `enum class …Bits : std::uint8_t` in C++.

### §7.5.1.2.2 Table 44 — Bound 9–16: unsigned short

**Spec:** §7.5.1.2.2, Table 44, S. 172 — "9–16 | unsigned short".

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_holder` (Z. 897);
`crates/lang-rules/src/adapter/cpp.rs::cpp_holder` (Z. 884)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_bitmask_becomes_an_integer_and_one_constant_per_bit`

**Status:** done — `uint16_t`.

### §7.5.1.2.2 Table 44 — Bound 17–32: unsigned long

**Spec:** §7.5.1.2.2, Table 44, S. 172 — "17–32 | unsigned long".

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_holder`;
`crates/lang-rules/src/adapter/cpp.rs::cpp_holder`

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::table_44_seventeen_to_thirty_two_bits_is_an_unsigned_long`

**Status:** done — `@bit_bound(32)` ergibt `uint32_t` in C und C++ (Flag 31 als `1 << 31`), `long` in Java.

### §7.5.1.2.2 Table 44 — Bound 33–64: unsigned long long

**Spec:** §7.5.1.2.2, Table 44, S. 172 — "33–64 | unsigned long long".

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_holder` (Z. 897);
`crates/lang-rules/src/adapter/cpp.rs::cpp_holder` (Z. 884)

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::table_44_thirty_three_to_sixty_four_bits_is_an_unsigned_long_long`,
`crates/lang-rules/src/adapter/java.rs::tests::a_bitmask_takes_the_holder_its_bound_calls_for`

**Status:** done — `@bit_bound(64)` ergibt `uint64_t` in C und C++ (Flag 63), `long` in Java.

### §7.5.1.2.2 Beispiel MyFlags

**Spec:** §7.5.1.2.2, S. 173 — "@bit_bound(19) bitmask MyFlags { FIRST_FLAG,
@position(14) SECOND_FLAG, THIRD_FLAG, };" mit `typedef unsigned long MyFlags;`.

**Repo:** `crates/lang-rules/src/adapter/c.rs::declare`,
`crates/lang-rules/src/adapter/cpp.rs::declare`,
`crates/lang-rules/src/adapter/java.rs::declare`

**Tests:** `crates/lang-rules/tests/enumeration_and_bitmask_binding.rs::the_example_of_the_clause_myflags_binds_as_the_text_shows`

**Status:** done — `@bit_bound(19) bitmask MyFlags` ergibt in C und C++ `uint32_t` (`unsigned long`), `MyFlagsBits` mit FIRST_FLAG = 1 << 0, SECOND_FLAG = 1 << 14 und THIRD_FLAG = 1 << 15, in Java die Klasse `MyFlagsBits`.

### §7.5.1.2.3 External Members: Zweck

**Spec:** §7.5.1.2.3, S. 173 — "The storage for a member of an aggregated type
may be declared to be external to the storage of the enclosing object of that
type."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Anwendungsfälle.

### §7.5.1.2.3.1 C: externe Member als Zeiger

**Spec:** §7.5.1.2.3.1, S. 173 — "External members shall be represented using
pointers. [...] Other external members are mapped like non-external members
except that a member of type X shall instead be mapped as type pointer-to-X."

**Repo:** `crates/lang-rules/src/adapter/c.rs::c_held_member_type` (Z. 583)

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_non_recursive_external_member_is_a_pointer_in_c`,
`crates/lang-rules/tests/external_is_an_indirection.rs::the_c_column_declares_a_recursive_member_as_a_pointer`,
`crates/lang-rules/tests/external_is_an_indirection.rs::an_external_string_stays_the_pointer_it_is`

**Status:** done — Ein externer Member ist in C ein Zeiger (`string`, `wstring` und `fixed` bleiben `const char *`); `@external long a` wird `int32_t *a`.

### §7.5.1.2.3.1 C: nicht rekursiver externer Member ist ein Zeiger

**Spec:** §7.5.1.2.3.1, S. 173 — "a member of type X shall instead be mapped as
type pointer-to-X. For example, short shall be replaced by short*."

**Repo:** —

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_non_recursive_external_member_is_a_pointer_in_c`

**Status:** done — Auch ein nicht rekursiver externer Member ist ein Zeiger (`short` wird `short*`).

### §7.5.1.2.3.1 C: Initialisierung setzt externe Zeiger auf NULL

**Spec:** §7.5.1.2.3.1, S. 173 — "The constructor/initializer of the enclosing
object shall set the external member pointers to NULL."

**Repo:** `crates/lang-rules/src/adapter/c_lifecycle.rs`

**Tests:** `crates/lang-rules/tests/c_external_lifecycle.rs::the_initialiser_sets_the_external_pointers_to_null`,
`crates/lang-rules/tests/c_external_lifecycle.rs::an_aggregate_with_an_external_member_gets_the_four_functions`

**Status:** done — `zd_init_<T>` setzt die externen Zeiger auf NULL; für jeden Aggregattyp, der einen externen Member hält, erzeugt der C-Generator vier Funktionen (`zd_init_<T>`, `zd_fini_<T>`, `zd_copy_<T>`, `zd_copy_with_<T>`).

### §7.5.1.2.3.1 C: Destruktor löscht referenzierte Objekte

**Spec:** §7.5.1.2.3.1, S. 174 — "The destructor of the enclosing object shall
delete the objects referenced by non-NULL external member pointers."

**Repo:** —

**Tests:** `crates/lang-rules/tests/c_external_lifecycle.rs::the_destructor_deletes_what_a_non_null_pointer_references`,
`crates/lang-rules/tests/c_external_lifecycle.rs::an_external_branch_of_a_union_is_released_by_the_live_discriminator`

**Status:** done — `zd_fini_<T>` löscht die Objekte, auf die ein Zeiger ungleich NULL zeigt; bei einer Union wählt der aktive Diskriminator den Zweig.

### §7.5.1.2.3.1 C: Kopierfunktion kopiert externe Member tief

**Spec:** §7.5.1.2.3.1, S. 174 — "The copy function of the enclosing object
shall do a deep copy of the external members. If the destination external member
is NULL it shall be allocated. [...] If the (recursive call to the) copy
operation of the external member fails, then the copy function of the containing
object shall fail as well."

**Repo:** —

**Tests:** `crates/lang-rules/tests/c_external_lifecycle.rs::the_copy_allocates_a_null_destination_and_fills_it`,
`crates/lang-rules/tests/c_external_lifecycle.rs::the_copy_has_the_modes_of_table_45`

**Status:** done — `zd_copy_<T>` kopiert externe Member tief und legt einen NULL-Zielzeiger an; `zd_copy_with_<T>` hat die Tiefe und die Aktion aus Table 45. Schlägt die Kopie eines externen Members fehl, schlägt die des Aggregats fehl.

### §7.5.1.2.3.1 C: zusätzliche Kopierfunktion mit wählbarem Verhalten

**Spec:** §7.5.1.2.3.1, S. 174 — "There may be an additional copy function [...]
This additional copy function shall allow the user to choose whether a shallow
or deep copy is made as well as whether any existing memory pointed by the
member is reused, released, or replaced".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — optional („There may be“); die Anforderungen
gelten nur, wenn es die Funktion gibt. ZeroDDS hat sie nicht.

### §7.5.1.2.3.1 Table 45 — Shallow Copy, Replace

**Spec:** §7.5.1.2.3.1, Table 45, S. 174 — "Shallow Copy | Replace".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verhalten der optionalen zusätzlichen
Kopierfunktion, die ZeroDDS nicht hat.

### §7.5.1.2.3.1 Table 45 — Shallow Copy, Release

**Spec:** §7.5.1.2.3.1, Table 45, S. 174 — "Shallow Copy | Release".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verhalten der optionalen zusätzlichen
Kopierfunktion, die ZeroDDS nicht hat.

### §7.5.1.2.3.1 Table 45 — Deep Copy, Reuse

**Spec:** §7.5.1.2.3.1, Table 45, S. 175 — "Deep Copy | Reuse".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verhalten der optionalen zusätzlichen
Kopierfunktion, die ZeroDDS nicht hat.

### §7.5.1.2.3.1 Table 45 — Deep Copy, Replace

**Spec:** §7.5.1.2.3.1, Table 45, S. 175 — "Deep Copy | Replace".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verhalten der optionalen zusätzlichen
Kopierfunktion, die ZeroDDS nicht hat.

### §7.5.1.2.3.1 Table 45 — Deep Copy, Release

**Spec:** §7.5.1.2.3.1, Table 45, S. 175 — "Deep Copy | Release".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verhalten der optionalen zusätzlichen
Kopierfunktion, die ZeroDDS nicht hat.

### §7.5.1.2.3.1.1 C: externer optionaler Member darf beim Schreiben NULL sein

**Spec:** §7.5.1.2.3.1.1, S. 175 — "A member that is both external and optional
shall be mapped as if it was just external. [...] If the member is only external
but not optional, then it is not allowed for the member to be NULL at the time
of a write."

**Repo:** `crates/lang-rules/src/emit/c.rs::refuse_unset_external`;
`runtime/c/src/writer.c::zdw_writer_refuse`

**Tests:** `crates/lang-rules/tests/c_optional_and_external_run.rs::an_external_member_that_is_not_optional_may_not_be_null_at_a_write`,
`crates/lang-rules/tests/external_is_an_indirection.rs::the_c_encoder_hands_the_pointer_and_not_its_address`,
`crates/lang-rules/tests/external_is_an_indirection.rs::the_c_encoder_writes_through_an_external_pointer`

**Status:** done — ein optionaler externer Member ist ein Zeiger und darf beim Schreiben NULL sein (Marker 0); ein nur externer Member mit NULL setzt `ZDW_UNSET_EXTERNAL` im Writer (`zdw_writer_refuse`) und nichts weiter wird geschrieben.

### §7.5.1.2.3.2 Traditional C++: zeigerartiger Typ mit Zustand „unset“

**Spec:** §7.5.1.2.3.2, S. 175 — "External members shall be represented by any
type that behaves similarly to a pointer (e.g., a plain pointer or a _var type).
The chosen type must support the concept of being 'unset.'"

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::declare` (Z. 1203, `held_externally`);
`crates/lang-rules/src/adapter/cpp.rs::external_doc` (Z. 898)

**Tests:** `crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value`,
`crates/lang-rules/tests/external_is_an_indirection.rs::an_external_union_branch_takes_the_same_indirection`

**Status:** done — jeder `@external`-Member ist ein `std::shared_ptr`; leer ist
„unset“.

### §7.5.1.2.3.2 Traditional C++: Konstruktor, Destruktor, Kopie wie in C

**Spec:** §7.5.1.2.3.2, S. 175 — "The behavior of the constructor, destructor,
and copy functions shall be the same as specified for C."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::owning_members`

**Tests:** `crates/lang-rules/tests/cpp_optional_and_external_run.rs::a_copy_of_a_class_with_a_pointer_member_is_deep_and_the_destructor_deletes`,
`crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value`

**Status:** done — eine Klasse mit Zeigermember hat einen Standardkonstruktor (NULL), einen Kopierkonstruktor und eine Zuweisung mit tiefer Kopie, Verschiebeoperationen, die NULL zurücklassen, und einen Destruktor, der löscht.

### §7.5.1.2.3.2 Traditional C++: Kopie teilt den externen Member

**Spec:** §7.5.1.2.3.2, S. 175 — "The behavior of the [...] copy functions shall
be the same as specified for C" (§7.5.1.2.3.1: "deep copy of the external
members").

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::owning_members`;
`runtime/cpp/include/zerodds/wire.hpp::clone`

**Tests:** `crates/lang-rules/tests/cpp_optional_and_external_run.rs::a_copy_of_a_class_with_a_pointer_member_is_deep_and_the_destructor_deletes`

**Status:** done — die Kopie legt für `std::shared_ptr` und für Zeiger ein neues Objekt an (`zerodds::wire::clone`); der Test prüft, dass Quelle und Kopie verschiedene Objekte halten.

### §7.5.1.2.3.2.1 Traditional C++: externer optionaler Member

**Spec:** §7.5.1.2.3.2.1, S. 175 — "A member that is both external and optional
shall be mapped as if it was just external. [...] it is valid for the member to
be unset when writing".

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::external_doc`;
`crates/lang-rules/src/emit/cpp.rs::refuse_unset_external`

**Tests:** `crates/lang-rules/tests/cpp_optional_and_external_run.rs::an_external_member_that_is_not_optional_may_not_be_unset_at_a_write`,
`crates/lang-rules/tests/external_is_an_indirection.rs::a_recursive_external_member_is_a_shared_pointer_and_not_a_value`

**Status:** done — ein optionaler externer Member ist derselbe `std::shared_ptr`, leer heißt abwesend; ein leerer Zeiger an einem nur externen Member wirft beim Schreiben `zerodds::wire::UnsetExternal`.

### §7.5.1.2.3.3 Modern C++: Template external<T> in dds::core

**Spec:** §7.5.1.2.3.3, S. 176 — "External members shall be represented as an
instantiation of a template class external<T> [...] The external<T> class shall
be generated inside of an appropriate namespace. In the case of [DDS-PSM-CXX],
this namespace is dds::core."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`;
`crates/lang-rules/src/adapter/cpp_psm.rs::attribute`

**Tests:** `crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`,
`crates/lang-rules/tests/cpp_psm_external_run.rs::a_psm_external_member_is_dds_core_external_and_an_unset_one_is_refused_at_a_write`

**Status:** done — `dds::core::external<T>` in `runtime/cpp/include/dds/core/external.hpp` mit `ptr_` (shared pointer) und `locked_`; unter `CppMapping::Psm` ist ein `@external`-Member ein `dds::core::external<T>`.

### §7.5.1.2.3.3 Modern C++: locked_ steuert tiefe oder flache Kopie

**Spec:** §7.5.1.2.3.3, S. 176 — "The value of the locked_ attribute dictates
whether copying an external member performs a deep copy or shallow copy [...]
The locked_ attribute can only be set to true when the shared pointer is set to
a non-NULL value."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::copy_constructor`,
`runtime/cpp/tests/external_selftest.cpp::constructor_from_a_pointer`,
`runtime/cpp/tests/external_selftest.cpp::lock_and_is_locked`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — ungesperrt teilen Kopie und Zuweisung die Referenz, gesperrt kopieren sie tief; `locked_` kann nur bei einem Zeiger ungleich NULL wahr sein (Konstruktor und `lock()` werfen sonst).

### §7.5.1.2.3.3.1 external<T>: Default Constructor

**Spec:** §7.5.1.2.3.3.1, S. 177 — "Create an empty external<T> object with an
empty ptr_ and locked_ initialized to false."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::default_constructor`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — leeres `ptr_`, `locked_` ist false.

### §7.5.1.2.3.3.2 external<T>: Constructor from a T*

**Spec:** §7.5.1.2.3.3.2, S. 177 — "Create a new external<T> object referencing
the provided managed object. The attribute locked_ is set to false and ptr_ is
initialized with p."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::constructor_from_a_pointer`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — `ptr_` verwaltet `p`, `locked_` ist false (oder der Parameter `locked`).

### §7.5.1.2.3.3.3 external<T>: Constructor from a shared pointer to T object

**Spec:** §7.5.1.2.3.3.3, S. 177 — "Create a new external<T> object that
references the same object managed by the specified shared pointer p."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::constructor_from_a_shared_pointer`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — `ptr_` teilt das Objekt des übergebenen `shared_ptr`, `locked_` ist false.

### §7.5.1.2.3.3.4 external<T>: Copy Constructor

**Spec:** §7.5.1.2.3.3.4, S. 177 — "If other.is_locked() is false, then the new
external<T> object shares the reference with other. [...] If other.is_locked()
is true, then a new T object is created [...] deep copy."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::copy_constructor`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — ungesperrte Quelle: dieselbe Referenz; gesperrte Quelle: ein neues `T`, tief kopiert; `locked_` der Kopie ist in beiden Fällen false.

### §7.5.1.2.3.3.5 external<T>: Assignment Operator

**Spec:** §7.5.1.2.3.3.5, S. 178 — "The behavior specified in Table 46 below
shall be applied when assigning an external<T> object source to another
external<T> object destination"

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_destination_locked_is_an_error`,
`runtime/cpp/tests/external_selftest.cpp::assignment_source_empty_resets_the_destination`,
`runtime/cpp/tests/external_selftest.cpp::assignment_empty_destination_locked_source_creates_and_deep_copies`,
`runtime/cpp/tests/external_selftest.cpp::assignment_existing_destination_locked_source_reuses_and_deep_copies`,
`runtime/cpp/tests/external_selftest.cpp::assignment_unlocked_source_is_a_shallow_copy`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — die fünf Zeilen von Table 46 (siehe dort); ein gesperrtes Ziel wirft `external_locked_error`.

### §7.5.1.2.3.3.6 external<T>: Destructor

**Spec:** §7.5.1.2.3.3.6, S. 178 — "If ptr_ is the last reference to the managed
object, then the managed object will be released, otherwise the reference count
will simply be decreased."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::destructor`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — der Destruktor des `shared_ptr` gibt das Objekt mit der letzten Referenz frei.

### §7.5.1.2.3.3.7 external<T>: operator* (const and non-const versions)

**Spec:** §7.5.1.2.3.3.7, S. 178 — "Get a reference to the underlying managed
object that ptr_ points at."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::accessors`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — beide Fassungen liefern das verwaltete Objekt; ein leeres `external` wirft `external_empty_error`.

### §7.5.1.2.3.3.8 external<T>: get (const and non-const versions)

**Spec:** §7.5.1.2.3.3.8, S. 178 — "Obtains a pointer to the managed object."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::accessors`,
`runtime/cpp/tests/external_selftest.cpp::default_constructor`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — beide Fassungen liefern den Zeiger auf das verwaltete Objekt, NULL wenn leer.

### §7.5.1.2.3.3.9 external<T>: get_shared_ptr

**Spec:** §7.5.1.2.3.3.9, S. 178 — "Obtains a shared pointer to the managed
object."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::constructor_from_a_shared_pointer`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — liefert den `shared_ptr` des verwalteten Objekts.

### §7.5.1.2.3.3.10 external<T>: operator-> (const and non-const versions)

**Spec:** §7.5.1.2.3.3.10, S. 179 — "Allows accessing members of the managed
object."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::accessors`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — beide Fassungen greifen auf die Member des Objekts zu.

### §7.5.1.2.3.3.11 external<T>: operator==

**Spec:** §7.5.1.2.3.3.11, S. 179 — "Returns whether two external objects manage
the same object or are both empty."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::comparison_and_bool`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — gleich, wenn beide dasselbe Objekt verwalten oder beide leer sind.

### §7.5.1.2.3.3.12 external<T>: operator!=

**Spec:** §7.5.1.2.3.3.12, S. 179 — "Returns whether two external objects do not
manage the same object."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::comparison_and_bool`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — ungleich, wenn sie nicht dasselbe Objekt verwalten.

### §7.5.1.2.3.3.13 external<T>: operator bool

**Spec:** §7.5.1.2.3.3.13, S. 179 — "Checks if there is a managed object (is not
NULL) or not (is NULL)."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::comparison_and_bool`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — true, wenn ein Objekt verwaltet wird.

### §7.5.1.2.3.3.14 external<T>: is_locked

**Spec:** §7.5.1.2.3.3.14, S. 179 — "Indicates whether this object is locked or
not."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::lock_and_is_locked`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — gibt `locked_` zurück.

### §7.5.1.2.3.3.15 external<T>: lock

**Spec:** §7.5.1.2.3.3.15, S. 179 — "Sets the locked_ attribute to true."

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::lock_and_is_locked`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — setzt `locked_`; bei leerem `ptr_` wirft es.

### §7.5.1.2.3.3.5 Table 46 — Ziel locked_ TRUE, ptr_ <any>; Quelle locked_ <any>, ptr_ <any>

**Spec:** §7.5.1.2.3.3.5, Table 46, S. 178 — "TRUE | <any> | <any> | <any> |
Error. Operation cannot be called when destination.is_locked() == TRUE".

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_destination_locked_is_an_error`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — Fehler: `external_locked_error`, das Ziel bleibt unverändert.

### §7.5.1.2.3.3.5 Table 46 — Ziel locked_ FALSE, ptr_ <any>; Quelle locked_ <any>, ptr_ EMPTY

**Spec:** §7.5.1.2.3.3.5, Table 46, S. 178 — "FALSE | <any> | <any> | EMPTY |
The destination is reset.".

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_source_empty_resets_the_destination`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — das Ziel ist zurückgesetzt, `ptr_` ist leer.

### §7.5.1.2.3.3.5 Table 46 — Ziel locked_ FALSE, ptr_ EMPTY; Quelle locked_ TRUE, ptr_ Not EMPTY

**Spec:** §7.5.1.2.3.3.5, Table 46, S. 178 — "FALSE | EMPTY | TRUE | Not EMPTY |
Create new object for destination.ptr_ Perform deep copy".

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_empty_destination_locked_source_creates_and_deep_copies`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — ein neues Objekt, tief kopiert.

### §7.5.1.2.3.3.5 Table 46 — Ziel locked_ FALSE, ptr_ Not EMPTY; Quelle locked_ TRUE, ptr_ Not EMPTY

**Spec:** §7.5.1.2.3.3.5, Table 46, S. 178 — "FALSE | Not EMPTY | TRUE | Not
EMPTY | Reuse existing destination.ptr_ Perform deep copy".

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_existing_destination_locked_source_reuses_and_deep_copies`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — das vorhandene Objekt des Ziels wird wiederverwendet, tief kopiert.

### §7.5.1.2.3.3.5 Table 46 — Ziel locked_ FALSE, ptr_ <any>; Quelle locked_ FALSE, ptr_ Not EMPTY

**Spec:** §7.5.1.2.3.3.5, Table 46, S. 178 — "FALSE | <any> | FALSE | Not EMPTY
| Perform shallow copy.".

**Repo:** `runtime/cpp/include/dds/core/external.hpp`

**Tests:** `runtime/cpp/tests/external_selftest.cpp::assignment_unlocked_source_is_a_shallow_copy`,
`crates/lang-rules/tests/dds_core_external.rs::the_template_does_what_every_operation_and_every_row_of_table_46_says`

**Status:** done — flache Kopie: `ptr_` des Ziels ist `ptr_` der Quelle.

### §7.5.1.2.3.3.16 Modern C++: externer optionaler Member

**Spec:** §7.5.1.2.3.3.16, S. 179 — "A member that is both external and optional
shall be mapped as if it was just external. The difference is that it is valid
for ptr_ to be empty when writing".

**Repo:** `crates/lang-rules/src/adapter/cpp_psm.rs::attribute`;
`crates/cpp/include/zerodds/psm/convert.hpp::Conv<dds::core::external<E>>`

**Tests:** `crates/lang-rules/tests/cpp_psm_external_run.rs::a_psm_external_member_is_dds_core_external_and_an_unset_one_is_refused_at_a_write`

**Status:** done — ein optionaler externer Member ist ein `dds::core::external<T>`, das beim Schreiben leer sein darf (Marker 0); ein nur externer Member mit leerem `ptr_` wirft beim Schreiben `UnsetExternal`.

### §7.5.1.2.3.4 Java: externe Member als Objektreferenz, primitive Typen geboxt

**Spec:** §7.5.1.2.3.4, S. 179 — "External members shall be represented using
object references. [...] For IDL types that map to Java primitive types, those
Java primitive types shall be replaced by the corresponding object box types
from the java.lang package."

**Repo:** `crates/lang-rules/src/adapter/java.rs::declare`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_external_member_is_an_object_reference_and_a_primitive_one_is_boxed`

**Status:** done — ein primitiver `@external`-Member (und Union-Zweig) ist `java.lang.Short`, `Integer`, `Boolean` usw.; ein String und eine Sequenz bleiben die Referenz, die sie sind.

### §7.5.1.2.4 Optional Members: Wirkung auf die Bindungen

**Spec:** §7.5.1.2.4, S. 179 — "A member of an aggregated type may be declared
to be optional [...] This concept impacts the language bindings for C, C++, and
Java in the following ways."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §7.5.1.2.4.1 C: optionaler Member als Zeiger, NULL für abwesend

**Spec:** §7.5.1.2.4.1, S. 180 — "Optional members shall be represented using
pointers. [...] a member of type X shall instead be mapped as type pointer-to-X.
[...] A NULL pointer shall indicate an omitted value."

**Repo:** `crates/lang-rules/src/adapter/c.rs::pointer_member`;
`crates/lang-rules/src/emit/c.rs::present`

**Tests:** `crates/lang-rules/tests/c_optional_and_external_run.rs::an_optional_member_is_written_with_its_marker_and_read_back_through_a_pointer`,
`crates/lang-rules/tests/c_optional_and_external_run.rs::an_optional_string_is_the_pointer_it_already_is`,
`crates/lang-rules/tests/c_end_to_end.rs::an_optional_member_is_a_pointer_and_null_is_the_absent_one`

**Status:** done — ein optionaler Member ist `T *m`, ein `string` bleibt `const char *`; der Encoder fragt `m != NULL`, der Decoder legt den Wert im Arena des Readers an und setzt einen abwesenden Member auf NULL; das Flag `_has_` entfällt.

### §7.5.1.2.4.2 C++: optionaler Member als einfacher Zeiger

**Spec:** §7.5.1.2.4.2, S. 180 — "Optional members shall be represented using
plain pointers rather than automatic values or smart pointers. [...] A NULL
pointer shall indicate an omitted value."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::held`;
`crates/lang-rules/src/emit/cpp.rs::present`

**Tests:** `crates/lang-rules/tests/cpp_optional_and_external_run.rs::an_optional_member_is_written_with_its_marker_and_read_back_through_a_pointer`,
`crates/lang-rules/tests/cpp_optional_and_external_run.rs::an_optional_string_is_a_pointer_to_a_string`,
`crates/lang-rules/src/adapter/cpp.rs::tests::an_optional_member_is_a_plain_pointer_and_not_a_second_field`

**Status:** done — ein optionaler Member der nativen C++-Abbildung ist `T *m = nullptr`, kein `std::optional`; Encoder und Decoder fragen `!= nullptr`, die Klasse besitzt das Objekt (siehe §7.5.1.2.3.2). Die PSM-Abbildung (`CppMapping::Psm`) behält `dds::core::optional<T>` aus DDS-PSM-Cxx 1.0 §8.1.4 und konvertiert in den Zeiger.

### §7.5.1.2.4.3 Java: optionaler Member als Objektreferenz, null für abwesend

**Spec:** §7.5.1.2.4.3, S. 180 — "Optional members shall be represented using
object references. [...] For IDL types that map to Java primitive types, those
Java primitive types shall be replaced by the corresponding object box types.
[...] A null pointer shall indicate an omitted value."

**Repo:** `crates/lang-rules/src/adapter/java.rs::boxed` (Z. 1282)

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_optional_member_is_boxed_and_a_primitive_one_too`

**Status:** done

### §7.5.1.2.4.4 Optionale Arrays in C und C++: Zeiger auf Array

**Spec:** §7.5.1.2.4.4, S. 180–181 — "Optional arrays having element type 'T'
shall be mapped to type pointer-to-array-of-type-T rather than to type
array-of-pointers-to-type-T. [...] int32_t (*array_member)[10];"

**Repo:** `crates/lang-rules/src/adapter/c.rs::pointer_member`;
`crates/lang-rules/src/adapter/cpp.rs::held`

**Tests:** `crates/lang-rules/tests/c_optional_and_external_run.rs::an_optional_array_is_a_pointer_to_the_array_and_not_an_array_of_pointers`,
`crates/lang-rules/tests/cpp_optional_and_external_run.rs::an_optional_array_is_a_pointer_to_the_array_and_not_an_array_of_pointers`

**Status:** done — C: `@optional long array_member[3]` ist ein Zeiger auf den Array-Typ und lässt sich `int32_t (*p)[3]` zuweisen; C++: ein Zeiger auf `std::array<std::int32_t, 3>` mit dem Layout des C-Arrays.

### §7.5.1.2.5 Nested Types: keine TypeSupport, DataReader, DataWriter nötig

**Spec:** §7.5.1.2.5, S. 181 — "An IDL compiler need not (although it may)
generate TypeSupport, DataReader, or DataWriter classes for any nested type."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis.

### §7.5.1.2.6 User-Defined Annotation Types: Sprachen ohne Annotationen dürfen sie ignorieren

**Spec:** §7.5.1.2.6, S. 181 — "In programming languages that lack the concept
of annotations, an implementation of this specification may choose to ignore
user-defined annotations with respect to this language binding."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis.

### §7.5.1.2.6.1 Java: @interface je IDL-Annotationstyp und <TypeName>Group

**Spec:** §7.5.1.2.6.1, S. 181 — "Each user-defined IDL annotation type shall be
represented by a corresponding Java annotation type. [...] public @interface
<TypeName> { ... } public @interface <TypeName>Group { <TypeName>[] value(); }"

**Repo:** `crates/lang-rules/src/adapter/java.rs::annotation_types`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_user_annotation_is_an_interface_with_its_defaults_and_a_group`

**Status:** done — je Annotationsdeklaration `public @interface <TypeName>` mit den Membern und `public @interface <TypeName>Group { <TypeName>[] value(); }`.

### §7.5.1.2.6.1 Java: Default-Werte der Annotationsmember

**Spec:** §7.5.1.2.6.1, S. 181 — "If a default value is specified for a given
member, it shall be reflected in the Java definition. Otherwise, the Java
definition shall have no default value."

**Repo:** `crates/lang-rules/src/adapter/java.rs::annotation_types`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_user_annotation_is_an_interface_with_its_defaults_and_a_group`

**Status:** done — ein Member mit Default trägt `default <Wert>`, einer ohne nicht.

### §7.5.1.2.6.1 Java: angewandte Annotation am Java-Element, mehrfach über Group

**Spec:** §7.5.1.2.6.1, S. 181–182 — "For each IDL element to which a single
instance user-defined annotation is applied, the corresponding Java element
shall be annotated with the Java annotation of the same name. For each IDL
element to which multiple instances of the annotation are applied, the
corresponding Java element shall be annotated with the generated annotation
bearing the 'Group' suffix".

**Repo:** `crates/lang-rules/src/adapter/java.rs::user_annotations`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_applied_annotation_sits_on_the_java_element_and_several_go_through_the_group`,
`crates/lang-rules/src/adapter/java.rs::tests::an_annotation_on_a_module_and_on_a_constant_reaches_the_java_element`

**Status:** done — eine angewandte Annotation steht an Klasse, Feld, Enumerator-Konstante, Bit-Konstante, Modulklasse oder Konstante; mehrere Instanzen einer Annotation an einem Element gehen über `<TypeName>Group({…})`.

---

## §7.5.1.3 Map Types

### §7.5.1.3 Schlüsseltypen UInt32, UInt64, String<Char8> verpflichtend

**Spec:** §7.5.1.3, S. 182 — "Implementations are only required to support keys
of types UInt32, UInt64, and String<Char8>. Implementations may choose to
support other key types".

**Repo:** `crates/wire-codec/src/encode.rs::write_map` (Z. 2221);
`crates/lang-rules/src/adapter/c.rs::c_map_entry_name` (Z. 818)

**Tests:** `crates/wire-conformance/tests/map_bytes.rs::the_bytes_are_the_recorded_fast_dds_row`,
`crates/wire-probe/tests/spec_constructs.rs::a_map_of_primitives_is_a_length_and_the_pairs`

**Status:** done — Schlüssel jedes Typs, den das Modell zulässt, darunter
`string` und die ganzzahligen Typen. Die Tests verwenden `string` und `long`;
ein Test mit `unsigned long` oder `unsigned long long` als Schlüssel ist nicht
zitiert (Korpusfall `map-key-ulonglong` vorhanden).

### §7.5.1.3 Anonyme Map mit nicht unterstütztem Schlüssel: Fehler

**Spec:** §7.5.1.3, S. 182 — "If a Type Representation compiler encounters an
anonymous map with key type that it does not support, it shall fail with an
error."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — bedingt: der Generator unterstützt jeden
Schlüsseltyp des Modells, ein nicht unterstützter Schlüssel kommt nicht vor.
Unter `MapEntryOrder::AscendingKey` weist der Encoder einen Schlüssel ohne
Ordnung ab (§7.2.2.4.3).

### §7.5.1.3.2.2 Table 47 — Map-Operation new

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "new | map<KeyType, ElementType>";
§7.5.1.3.2.2, S. 184 — "Allocate a new map. If this operation fails in an
implementation-specific way, this operation shall return NULL."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_new` legt eine leere Map an, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.3 Table 47 — Map-Operation delete

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "delete | void"; §7.5.1.3.2.3, S.
184 — "Delete the map and all of its contents."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_delete` gibt die Map frei, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.4 Table 47 — Map-Operation initialize

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "initialize | void"; §7.5.1.3.2.4,
S. 184 — "Initialize the map. The initial size and capacity of the map shall be
0."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_initialize` setzt Größe und Kapazität auf 0, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.5 Table 47 — Map-Operation finalize

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "finalize | void"; §7.5.1.3.2.5, S.
184 — "Finalize the map. The entries in the map will be deleted, and both the
size and maximum size set to 0."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_finalize` gibt die Einträge frei, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.6 Table 47 — Map-Operation copy

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "copy | ReturnCode_t";
§7.5.1.3.2.6, S. 184 — "Overwrite the contents of this (destination) map with
the contents of another (source) map. [...] If autogrow is FALSE, the operation
shall fail and return DDS_RETCODE_PRECONDITION_NOT_MET."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_copy` kopiert, mit und ohne autogrow, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.7 Table 47 — Map-Operation get_size

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "get_size | unsigned int";
§7.5.1.3.2.7, S. 184 — "Get the current size of the map."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_get_size`, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.8 Table 47 — Map-Operation get_max_size

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "get_max_size | unsigned int";
§7.5.1.3.2.8, S. 185 — "Get the current maximum size of the map."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_get_max_size`, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.9 Table 47 — Map-Operation set_max_size

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "set_max_size | ReturnCode_t";
§7.5.1.3.2.9, S. 185 — "Set the maximum size of the map. [...] If the new
max_size is less than the current size of the map, the operation shall fail and
return DDS_RETCODE_BAD_PARAMETER."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_set_max_size` (nicht unter die Größe, nicht über den Bound), erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.10 Table 47 — Map-Operation clear

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "clear | void"; §7.5.1.3.2.10, S.
185 — "Clear all of the entries from the map. The size of the map is set to 0
and the maximum size does not change."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_clear`, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.11 Table 47 — Map-Operation insert

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "insert | ReturnCode_t";
§7.5.1.3.2.11, S. 185 — "Insert a new entry into the map [...] If the key
already exists in the map, the operation shall fail and return
DDS_RETCODE_BAD_PARAMETER."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_insert` (vorhandener Schlüssel: DDS_RETCODE_BAD_PARAMETER, voll: DDS_RETCODE_PRECONDITION_NOT_MET), erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.12 Table 47 — Map-Operation insert_or_assign

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "insert_or_assign | ReturnCode_t";
§7.5.1.3.2.12, S. 185–186 — "If the key already exists in the map, then the
corresponding element shall be replaced."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_insert_or_assign`, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.13 Table 47 — Map-Operation erase

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "erase | ReturnCode_t";
§7.5.1.3.2.13, S. 186 — "Remove the entry with the given key from the map. If
successful, the size of the map shall be decreased by 1."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_erase`, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.14 Table 47 — Map-Operation get_first

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "get_first | ReturnCode_t";
§7.5.1.3.2.14, S. 186 — "Retrieves a MapEntry referencing the first entry in the
map. The returned MapEntry may be a sentinel if the map is empty."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_get_first`, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.15 Table 47 — Map-Operation get_next

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "get_next | ReturnCode_t";
§7.5.1.3.2.15, S. 186 — "Advance the MapEntry to the next entry in the Map."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_get_next`, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.16 Table 47 — Map-Operation find_element

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "find_element | ElementType";
§7.5.1.3.2.16, S. 186 — "Retrieve the element whose key matches the specified
one from the map. If the key exists, then return the element corresponding to
the key, otherwise return NULL."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_find_element`, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.17 Table 47 — Map-Operation find_entry

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "find_entry | MapEntry";
§7.5.1.3.2.17, S. 186 — "Retrieve the MapEntry whose key matches the specified
one from the map."

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_find_entry`, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.18 Table 47 — Map-Operation get_pair

**Spec:** §7.5.1.3.1, Table 47, S. 182–183 — "get_pair | Boolean";
§7.5.1.3.2.18, S. 186–187 — "Retrieve the key and element associated with the
MapEntry. If the MapEntry was a sentinel the operation will return FALSE"

**Repo:** `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`; `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — C: `<MAP>_get_pair`, erzeugt für jede Map neben ihrem Typ; Log: `.agent/REPORT.md`.

### §7.5.1.3.2 C: Map als Sammlung von Strukturen aus Schlüssel und Element

**Spec:** §7.5.1.3.2, S. 183 — "Map types shall be represented as a collection
of structures that contain a member of the key type followed by a member of the
element type. A set of methods which create, delete and manipulate objects of
the map type shall also be generated."

**Repo:** `crates/lang-rules/src/adapter/c_map.rs::map_decl`; `runtime/c/include/zerodds_map.h::ZD_MAP_DEFINE`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_operations_of_table_47_answer_as_the_clause_says`

**Status:** done — `struct { _maximum; _length; Entry *_buffer; _release }` über `struct { K key; V value; }`, mit den Methoden von Table 47; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.1 C: Name [key_type][fully_qualified_element_type]Map

**Spec:** §7.5.1.3.2.1, S. 183 — "For maps whose key type is a Primitive Type
the name of the map type shall be constructed by combining the key type name
with the element type name. [...] [key_type][fully_qualified_element_type]Map
[...] StringFooMap UInt32FooMap UInt64FooMap".

**Repo:** `crates/lang-rules/src/adapter/c_map.rs::map_type_name`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_name_of_a_map_is_the_key_and_the_element`

**Status:** done — `UInt32m_FooMap`, `StringInt32Map`, `UInt64m_ColorMap`; mit Bound `Int32StringMap_b3`; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.1 C: Maps mit UInt32-, UInt64- und String-Schlüssel neben jedem Typ T

**Spec:** §7.5.1.3.2.1, S. 183 — "For any type T, the declaration and
implementation of the map types having element type T and key types UInt32,
UInt64, and String shall be generated alongside the implementation code for
element type T."

**Repo:** `crates/lang-rules/src/adapter/c_map.rs::maps`

**Tests:** `crates/lang-rules/tests/c_map_operations.rs::the_name_of_a_map_is_the_key_and_the_element`

**Status:** done — für jeden deklarierten Typ T stehen `UInt32TMap`, `UInt64TMap` und `StringTMap` im Header; Log: `.agent/REPORT.md`.

### §7.5.1.3.2.19 Beispiel UInt32MyModule_MyElementTypeMap

**Spec:** §7.5.1.3.2.19, S. 187–188 — "Example (Non-Normative)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel.

### §7.5.1.3.3 Traditional C++: Map-Klasse im Namensraum des Elementtyps

**Spec:** §7.5.1.3.3.1, S. 189 — "The map class shall be named the same as the C
structure [...] except that it is placed in the same namespace as the element
type declaration."

**Repo:** `runtime/cpp/include/zerodds/map.hpp`; `crates/lang-rules/src/adapter/cpp.rs::maps_alongside`

**Tests:** `tools/idlc/tests/cpp_map_class.rs::a_map_is_a_class_with_the_operations_of_table_47`

**Status:** done — `zerodds::Map<K, V, Bound>` mit den Operationen von Table 47; `UInt32FooMap`, `UInt64FooMap`, `StringFooMap` stehen im Namensraum von `Foo`; Log: `.agent/REPORT.md`.

### §7.5.1.3.3 Traditional C++: insert mit replace

**Spec:** §7.5.1.3.3.1, S. 189 — "The C++ operation insert behaves as the C
insert() if the replace() parameter is false and it behaves as the C
insert_or_assign() if replace parameter is true."

**Repo:** `runtime/cpp/include/zerodds/map.hpp::Map::insert`

**Tests:** `tools/idlc/tests/cpp_map_class.rs::a_map_is_a_class_with_the_operations_of_table_47`

**Status:** done — `insert(key, element, replace)`: mit `replace` wird ein vorhandener Eintrag überschrieben, ohne `replace` ist es DDS::RETCODE_BAD_PARAMETER; Log: `.agent/REPORT.md`.

### §7.5.1.3.4 Modern C++: std::map<K, T>

**Spec:** §7.5.1.3.4, S. 190 — "The Map type shall be bound to an instantiation
of the std::map template."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Map unter `CppMapping::Psm`), `cpp_psm.rs`

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader` (`crates/cpp/tests/generated_psm_roundtrip.cpp`)

**Status:** done — `map<K,V>` ist in der Standardabbildung `std::map<K,V>`; `--cpp-mapping native` (`zerodds::Map`, Operationen von Table 47) und `classic` sind benannte Optionen; Log: `.agent/REPORT.md`.

### §7.5.1.3.4 Modern C++: Compare über den Inhalt bei String-Schlüsseln

**Spec:** §7.5.1.3.4, S. 190 — "When a map has keys of a string type, the
Compare function shall operate on the character contents of the strings; it
shall not operate on the strings' pointer values".

**Repo:** `crates/cpp/include/zerodds/` (der `std::map`-Vergleich), `crates/cpp/tests/generated_psm_roundtrip.cpp`

**Tests:** `crates/cpp/tests/generated_type_via_cargo.rs::a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader`

**Status:** done — der Schlüssel `std::string` vergleicht über den Inhalt (`std::less<std::string>`), das prüft das Programm in `generated_psm_roundtrip.cpp`; Log: `.agent/REPORT.md`.

### §7.5.1.3.5 Java: Implementierung von java.util.Map, geboxte Schlüssel und Werte

**Spec:** §7.5.1.3.5, S. 190 — "An IDL map type shall be represented in Java by
an implementation of the standard java.util.Map interface. [...] If either of
these Java types is a primitive type, then the corresponding object box type
[...] shall be used in its place."

**Repo:** `crates/lang-rules/src/adapter/java.rs::map_class`, `map_key_supported`

**Tests:** `crates/lang-rules/tests/java_map_class.rs::a_map_is_a_java_util_map_with_the_operations_of_table_47`,
`crates/lang-rules/tests/java_map_class.rs::a_structure_or_union_is_a_key_of_a_java_util_map`

**Status:** done — `ZdMap_…` erweitert `java.util.LinkedHashMap` über den geboxten Typen und trägt die Operationen von Table 47; Schlüssel sind Primitive, String, Aufzählung, Bitmask, Bitset, Struktur und Union (erzeugte `equals`/`hashCode`); Log: `.agent/REPORT.md`.

### §7.5.1.3.5 Java: Map mit Schlüsseln, die weder Primitive, String, Aufzählung noch Bitmask sind

**Spec:** §7.5.1.3.5, S. 190 — "An IDL map type shall be represented in Java by an implementation of the standard java.util.Map interface."

**Repo:** `crates/lang-rules/src/adapter/java.rs::map_key_supported`, `equals_and_hash_code`

**Tests:** `crates/lang-rules/tests/java_map_class.rs::a_structure_or_union_is_a_key_of_a_java_util_map`

**Status:** done — eine Struktur, Union oder ein Bitset als Schlüssel gibt der erzeugten Klasse `equals` und `hashCode` über den Inhalt; Log: `.agent/REPORT.md`.

### §7.5.1.3.6 Andere Sprachen: Bindung nach der IDL-Äquivalenz in §7.4.1.1.4

**Spec:** §7.5.1.3.6, S. 190 — "In all languages for which no language-specific
mapping is specified, the language binding for map types shall be based on the
equivalent IDL definition given in 7.4.1.1.4."

**Repo:** `crates/lang-rules/src/adapter/cst.rs` (`RustMapForm`), `crates/wire/src/map_entries.rs`; die übrigen Sprachen in `crates/lang-rules/src/adapter/`

**Tests:** `crates/lang-rules/tests/rust_corpus_compiles.rs::a_map_is_a_sequence_of_entries_that_keeps_its_order`,
`crates/wire-conformance/tests/map_bytes.rs::the_bytes_are_the_recorded_fast_dds_row`,
`crates/lang-rules/tests/go_end_to_end.rs::a_map_becomes_a_slice_of_entries_and_never_a_go_map` (ebenso D, Nim, Zig, Swift, Kotlin, F#, Julia, Lua, OCaml, Ada, Ada83, Elixir)

**Status:** done — Rust hält eine Map standardmäßig als Sequenz von `MapEntry` (`--rust-map btree-map` ist die benannte Option); die übrigen Sprachen ohne eigene Abbildung binden eine Sequenz von Einträgen (Go, D, Nim, Zig, Swift, Kotlin, F#, Julia, Lua, OCaml, Ada, Ada83) oder eine Liste von Paaren (Python, Elixir); C# folgt der Form von Java; TypeScript nutzt die Abbildung von DDS-TS §7.9.1. Log: `.agent/REPORT.md`.

---

## §7.5.1.4 Structure and Union Types

### §7.5.1.4 Strukturen und Unions nach den IDL-Sprachabbildungen

**Spec:** §7.5.1.4, S. 190 — "The Plain Language Binding for structure and union
types shall correspond to the IDL language mappings for IDL structures and
unions as amended below."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::union_members`, `crates/lang-rules/src/adapter/java.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `tools/idlc/tests/cpp_union_accessors.rs::a_union_has_the_accessor_functions_and_goes_through_the_codec`,
`crates/lang-rules/tests/java_map_class.rs` (Union-Zugriffsfunktionen),
`crates/lang-rules/tests/c_end_to_end.rs::a_union_becomes_a_discriminator_beside_a_union`

**Status:** done — eine Struktur wird eine Struktur oder Klasse, eine Union eine Klasse mit Zugriffsfunktionen (`_d()` und je Fall Zugriff und Setzer; C++ und Java) oder in C `_d` mit `union _u`; Log: `.agent/REPORT.md`.

### §7.5.1.4.1.1 C++: abgeleitete Struktur erbt öffentlich

**Spec:** §7.5.1.4.1.1, S. 191 — "The C++ struct corresponding to the subtype
shall publicly inherit from the C++ struct corresponding to the supertype."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::class`

**Tests:** `tools/idlc/tests/cpp_inheritance.rs::a_derived_struct_inherits_and_the_octet_is_its_own_type`

**Status:** done — `struct Derived : public Base`, nur die eigenen Member stehen in der Klasse; die Bytes gleichen denen der flachen Struktur (§7.4.1.1.5.1); Log: `.agent/REPORT.md`.

### §7.5.1.4.1.2 Java: abgeleitete Klasse erweitert die Basisklasse

**Spec:** §7.5.1.4.1.2, S. 191 — "The Java class corresponding to the subtype
shall extend the Java class corresponding to the supertype."

**Repo:** `crates/lang-rules/src/adapter/java.rs::data_class`

**Tests:** `crates/lang-rules/tests/java_map_class.rs::a_derived_class_extends_its_base_class`

**Status:** done — `class Derived extends Base`, nur die eigenen Member stehen in der Klasse; Log: `.agent/REPORT.md`.

### §7.5.1.4.1.3 Andere Sprachen: Basis als erster Member parent

**Spec:** §7.5.1.4.1.3, S. 191 — "The language binding shall be generated as if
an instance of the base type were the first member of the subtype with the name
'parent,' as in the following equivalent IDL definition: struct <struct_name> {
<base_type_name> parent; ... };"

**Repo:** `crates/lang-rules/src/emit/mod.rs::reach`; die Adapter  `crates/lang-rules/src/adapter/c.rs`, `crates/lang-rules/src/adapter/cst.rs`, `crates/lang-rules/src/adapter/go.rs`, `crates/lang-rules/src/adapter/d.rs`, `crates/lang-rules/src/adapter/nim.rs`, `crates/lang-rules/src/adapter/zig.rs`, `crates/lang-rules/src/adapter/swift.rs`, `crates/lang-rules/src/adapter/kotlin.rs`, `crates/lang-rules/src/adapter/fsharp.rs`, `crates/lang-rules/src/adapter/julia.rs`, `crates/lang-rules/src/adapter/lua.rs`, `crates/lang-rules/src/adapter/python.rs`, `crates/lang-rules/src/adapter/elixir.rs`, `crates/lang-rules/src/adapter/node.rs`, `crates/lang-rules/src/adapter/ada.rs`, `crates/lang-rules/src/adapter/ada83.rs`, `crates/lang-rules/src/adapter/ocaml.rs`, `crates/lang-rules/src/adapter/csharp.rs`, `crates/lang-rules/src/adapter/typescript.rs` (`cst.rs` ist Rust; C# als Basisklasse nach IDL4 to C# §7.14.1)

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_derived_struct_holds_the_base_as_its_first_member_parent`,
`crates/lang-rules/tests/rust_corpus_compiles.rs::a_derived_structure_holds_its_base_as_its_first_member_parent`,
`crates/lang-rules/tests/typescript_descriptors.rs::a_derived_struct_holds_its_base_as_the_member_parent`,
`crates/wire-probe/tests/three_column_gate.rs::the_three_columns_agree` (Fall `inherit-two-levels`: Rust, C und Python),
die `*_end_to_end`-Tests von Go, D, Nim, Zig, Swift, Julia, Lua, OCaml, Ada, Ada83 und Node, `python_corpus_compiles`, `elixir_end_to_end`, `csharp_corpus_compiles`, `typescript_corpus_compiles`

**Status:** done — die Basis ist der erste Member `parent` in C, Rust, Go, D, Nim, Zig, Swift, Kotlin, F#, Julia, Lua, Python, Elixir, Node, Ada, Ada83, OCaml und TypeScript (im Deskriptor als Member mit `base: true`, dem `getKey` und `withDefaults` folgen); C# erbt von der Basisklasse (IDL4 to C# §7.14.1); die Bytes sind die der flachen Struktur (§7.4.1.1.5.1). `kotlin_end_to_end` und `fsharp_end_to_end` laufen in der Integrationsrunde. Log: `.agent/REPORT.md`.

### §7.5.1.4.1.3 Andere Sprachen außer C: Basis als erster Member parent

**Spec:** §7.5.1.4.1.3, S. 191 — "The language binding shall be generated as if an instance of the base type were the first member of the subtype with the name 'parent'"

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`, `crates/lang-rules/src/emit/typescript.rs`, `runtime/typescript/src/registry.ts`

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::a_derived_struct_holds_its_base_as_the_member_parent`,
`crates/lang-rules/tests/typescript_corpus_compiles.rs`

**Status:** done — TypeScript: die Schnittstelle und die Fabrik haben `parent` als ersten Member, der Deskriptor führt ihn mit `base: true`, die Prüffunktion ruft die der Basis, der Codec erreicht Basis-Member als `parent.m`. Log: `.agent/REPORT.md`.

---

## §7.5.2 Dynamic Language Binding

### §7.5.2 Dynamic Type und Dynamic Data Language Binding

**Spec:** §7.5.2, S. 191 — "The Dynamic Type Language Binding provides an API to
manipulate types. [...] The Dynamic Data Language Binding provides an API to
manipulate objects of any Type."

**Repo:** `crates/types/src/dynamic/type_.rs::DynamicType`; `crates/types/src/dynamic/data.rs::DynamicData`; `crates/types/src/dynamic/builder.rs::DynamicTypeBuilder`

**Tests:** `crates/dcps/tests/annex_c_idl_conformance.rs::every_operation_of_annex_c_has_its_counterpart`; `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`; `crates/types/tests/dynamic_corpus.rs::a_dynamic_data_writes_the_octets_a_json_value_writes`

**Status:** done — jede Operation der Tabellen 48 bis 58 und des Annex C hat ihr Gegenstück (Items unten, alle done oder n/a); `annex_c_idl_conformance` liest die IDL des Annex und prüft das; Log in `.agent/REPORT.md`.

### §7.5.2 Figure 26 — Klassen DynamicType, DynamicTypeBuilderFactory, DynamicData, DynamicDataFactory

**Spec:** §7.5.2, Figure 26, S. 192 — "DynamicTypeBuilderFactory: This type is
logically a singleton. [...] DynamicDataFactory: This type is logically a
singleton."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Klassen sind unten einzeln
geführt.

### §7.5.2.1 UML-Abbildung: Aufzählungen, Klassifizierer, Eigenschaften, Operationen

**Spec:** §7.5.2.1, S. 192–194 — "UML enumerations are mapped to IDL
enumerations. [...] UML classifiers with value semantics are represented as IDL
valuetypes. Classifiers with reference semantics are represented as local
interfaces. [...] Properties with multiplicity [0..1] are defined as @optional.
[...] UML operations are represented as IDL operations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — beschreibt, wie das IDL in Annex C aus dem
UML-Modell entsteht. ZeroDDS hat keine IDL-PSM des Dynamic Language Binding
(Annex C ist dort geführt).

### §7.5.2.1 Mehrwertige Eigenschaft: get_<p>_count, get_<p>, set_<p> mit RETCODE_BAD_PARAMETER

**Spec:** §7.5.2.1, S. 193 — "The 'get' operation shall fail with
RETCODE_BAD_PARAMETER if the given index is outside of the current range. The
'set' operation shall do the same with one exception: it shall allow an index
one past the end [...] Either operation shall fail with RETCODE_BAD_PARAMETER if
either argument is nil."

**Repo:** `crates/types/src/dynamic/type_.rs::annotation`,
`crates/types/src/dynamic/type_.rs::get_member_by_index`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_index_past_the_end_is_a_bad_parameter`,
`crates/types/tests/dynamic_annotation_descriptors.rs::the_member_lookups_report_a_bad_parameter`

**Status:** done — `annotation_count`/`annotation(index)`, `verbatim_text_count`/`verbatim_text(index)` und `member_count`/`get_member_by_index` folgen der Regel: ein Index hinter dem Ende ist `RETCODE_BAD_PARAMETER` (`DynamicError::BadParameter`, `return_code`). Angehängt wird mit `apply_annotation` und `add_member`.

### §7.5.2.1 Qualifizierte Assoziation: get_<p>, get_all_<p>, set_<p>, RETCODE_NO_DATA

**Spec:** §7.5.2.1, S. 193–194 — "The 'get' operation shall return with
RETCODE_NO_DATA if no value exists for the given key. Either operation shall
return with RETCODE_BAD_PARAMETER if either argument is nil."

**Repo:** `crates/types/src/dynamic/annotation.rs::get_value`,
`crates/types/src/dynamic/annotation.rs::set_value`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`

**Status:** done — `AnnotationDescriptor::get_value(key)`, `get_all_value()` und `set_value(key, value)`: ein Schlüssel ohne Wert ist `RETCODE_NO_DATA`, ein leerer Schlüssel oder Wert `RETCODE_BAD_PARAMETER`.

---

## §7.5.2.2 DynamicTypeBuilderFactory

### §7.5.2.2 DynamicTypeBuilderFactory: logisch ein Singleton

**Spec:** §7.5.2.2, S. 194 — "This class is logically a singleton (although it
need not technically be a singleton in practice). Its 'only' instance is the
starting point for creating and deleting DynamicTypeBuilder objects."

**Repo:** `crates/types/src/dynamic/factory.rs::DynamicTypeBuilderFactory`,
`crates/types/src/dynamic/import.rs::import`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`,
`crates/types/tests/dynamic_type_builder_factory.rs::a_copy_carries_the_state_and_leaves_the_original_alone`

**Status:** done — `DynamicTypeBuilderFactory` hat keinen Zustand; jede Instanz verhält sich wie jede andere. Ein `TypeBuilder` je Typ besitzt ein eigenes Modell; ein Typ, den ein Argument nennt, wird mit allem, was er erreicht, in dieses Modell kopiert (`DynamicTypeBuilder::import`, rekursive Typen eingeschlossen). `DynamicTypeBuilder` bleibt die Fabrik über einem gemeinsamen Modell.

### §7.5.2.2.11 Table 48 — get_instance

**Spec:** §7.5.2.2.11, Table 48, S. 194, 198 — "Return a
DynamicTypeBuilderFactory instance that behaves like a singleton [...] Calling
this operation is legal even after delete_instance has been called."

**Repo:** `crates/types/src/dynamic/factory.rs::get_instance`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_primitive_type_is_answered_and_a_composite_kind_is_not`

**Status:** done — `DynamicTypeBuilderFactory::get_instance()`; ein Aufruf nach `delete_instance` ist erlaubt.

### §7.5.2.2.9 Table 48 — delete_instance

**Spec:** §7.5.2.2.9, Table 48, S. 194, 198 — "Reclaim any resources associated
with any object(s) previously returned from get_instance. [...] This operation
shall fail with RETCODE_ERROR if it fails for any implementation-specific
reason."

**Repo:** `crates/types/src/dynamic/factory.rs::delete_instance`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_primitive_type_is_answered_and_a_composite_kind_is_not`

**Status:** done — `DynamicTypeBuilderFactory::delete_instance()` gelingt; es gibt nichts zurückzugeben.

### §7.5.2.2.12 Table 48 — get_primitive_type

**Spec:** §7.5.2.2.12, Table 48, S. 194, 198 — "Retrieve a DynamicType object
corresponding to the indicated primitive type kind. [...] If the given kind does
not correspond to a primitive type, the operation shall fail and return a nil
value."

**Repo:** `crates/types/src/dynamic/factory.rs::get_primitive_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_primitive_type_is_answered_and_a_composite_kind_is_not`

**Status:** done — `get_primitive_type(kind)` gibt den Typ; eine Art, die kein primitiver Typ ist, ist `RETCODE_BAD_PARAMETER`.

### §7.5.2.2.6 Table 48 — create_type(TypeDescriptor)

**Spec:** §7.5.2.2.6, Table 48, S. 194, 197 — "Create and return a new
DynamicTypeBuilder object as described by the given type descriptor. [...] If
this argument is nil or inconsistent (as indicated by its is_consistent
operation), this operation shall fail and return a nil value."

**Repo:** `crates/types/src/dynamic/factory.rs::create_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`,
`crates/types/tests/dynamic_type_builder_factory.rs::an_inconsistent_descriptor_creates_no_type`

**Status:** done — `create_type(&TypeDescriptor)` legt je Art Struktur, Union, Aufzählung, Bitmask, Bitset, Alias, String, Sequenz, Array und Map an; ein inkonsistenter Descriptor und ein Typ ohne Namen sind `RETCODE_BAD_PARAMETER`; eine Annotationsart ist kein Typ dieses Modells und wird mit `Unrepresentable` abgewiesen.

### §7.5.2.2.7 Table 48 — create_type_copy

**Spec:** §7.5.2.2.7, Table 48, S. 194, 197 — "Create and return a new
DynamicTypeBuilder object with a copy of the state of the given type."

**Repo:** `crates/types/src/dynamic/factory.rs::create_type_copy`,
`crates/types/src/dynamic/import.rs::import`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_copy_carries_the_state_and_leaves_the_original_alone`

**Status:** done — `create_type_copy(&DynamicType)` gibt einen Builder mit einer Kopie des Zustands; Änderungen am Builder erreichen den Typ nicht.

### §7.5.2.2.8 Table 48 — create_type_w_type_object

**Spec:** §7.5.2.2.8, Table 48, S. 194, 197 — "Create and return a new
DynamicTypeBuilder object that describes a type identical to that described by
the given TypeObject object. Subsequent changes to the new DynamicTypeBuilder
object shall not be reflected in the input TypeObject object."

**Repo:** `crates/types/src/dynamic/factory.rs::create_type_w_type_object`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_type_is_created_from_a_type_object`,
`crates/types/src/dynamic/to_ast.rs::tests::a_minimal_type_object_is_refused_by_name`

**Status:** done — `create_type_w_type_object(&TypeObject, &TypeRegistry)` gibt einen Builder für den Typ, den ein COMPLETE TypeObject beschreibt; Änderungen am Builder erreichen das Objekt nicht. Ein MINIMAL TypeObject hat keine Namen und wird mit `MinimalHasNoNames` abgewiesen.

### §7.5.2.2.5 Table 48 — create_string_type

**Spec:** §7.5.2.2.5, Table 48, S. 194, 197 — "The element type of the result
returned by create_string_type shall be Char8. [...] If this argument is equal
to LENGTH_UNLIMITED, the string type shall be considered to be unbounded."

**Repo:** `crates/types/src/dynamic/factory.rs::create_string_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::the_collection_and_string_creators_build_their_kinds`

**Status:** done — `create_string_type(bound)`; `LENGTH_UNLIMITED` ist unbeschränkt, das Element ist Char8.

### §7.5.2.2.5 Table 48 — create_wstring_type

**Spec:** §7.5.2.2.5, Table 48, S. 194, 197 — "The element type of the result
returned by create_wstring_type shall be Char16."

**Repo:** `crates/types/src/dynamic/factory.rs::create_wstring_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::the_collection_and_string_creators_build_their_kinds`

**Status:** done — `create_wstring_type(bound)`; das Element ist Char16.

### §7.5.2.2.4 Table 48 — create_sequence_type

**Spec:** §7.5.2.2.4, Table 48, S. 195–197 — "All sequence types having equal
element types and equal bounds shall be considered equal. [...] Parameter
element_type [...] If this argument is nil, the operation shall fail with
RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::create_sequence_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::the_collection_and_string_creators_build_their_kinds`

**Status:** done — `create_sequence_type(&DynamicType, bound)`; der Elementtyp ist eine Kopie des Arguments.

### §7.5.2.2.1 Table 48 — create_array_type

**Spec:** §7.5.2.2.1, Table 48, S. 195–196 — "All array types having equal
element types, an equal number of dimensions, and equal bounds in each dimension
shall be considered equal. [...] Parameter bound - A collection of unsigned
integers, the length of which is equal to the number of dimensions".

**Repo:** `crates/types/src/dynamic/factory.rs::create_array_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::the_collection_and_string_creators_build_their_kinds`

**Status:** done — `create_array_type(&DynamicType, &[bound])`; keine Dimension ist `RETCODE_BAD_PARAMETER`.

### §7.5.2.2.3 Table 48 — create_map_type

**Spec:** §7.5.2.2.3, Table 48, S. 195–196 — "All map types having equal key and
value element types and equal bounds shall be considered equal. [...] If this
argument is equal to LENGTH_UNLIMITED, the map type shall be considered to be
unbounded."

**Repo:** `crates/types/src/dynamic/factory.rs::create_map_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::the_collection_and_string_creators_build_their_kinds`

**Status:** done — `create_map_type(&DynamicType, &DynamicType, bound)`.

### §7.5.2.2.2 Table 48 — create_bitmask_type

**Spec:** §7.5.2.2.2, Table 48, S. 195–196 — "Parameter bound - The number of
reserved bits in the bitmask. If this value is out of range, the operation shall
fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::create_bitmask_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::an_enumeration_and_a_bitmask_take_literals_and_flags`

**Status:** done — `create_bitmask_type(bound)`; ein Bound außerhalb von 1 bis 64 ist `RETCODE_BAD_PARAMETER`.

### §7.5.2.2.13 Table 48 — create_type_w_uri

**Spec:** §7.5.2.2.13, Table 48, S. 195, 199 — "Create and return a new
DynamicType object by parsing the type description at the given URL. [...]
Implementations shall minimally support the file:// URL scheme [...] the XML
Type Description format [...] include_paths".

**Repo:** `crates/xml-compose/src/lib.rs::create_type_w_uri`

**Tests:** `crates/xml-compose/tests/create_type_operations.rs::a_type_is_created_from_a_file_url`

**Status:** done — `create_type_w_uri(url, type_name, include_paths)` in `zerodds-xml-compose`: eine `file://`-URL und ein Pfad werden gelesen, das Verzeichnis des Dokuments liegt implizit auf dem Suchpfad, ein anderes Schema ist ein Fehler.

### §7.5.2.2.14 Table 48 — create_type_w_document

**Spec:** §7.5.2.2.14, Table 48, S. 195, 199–200 — "Create and return a new
DynamicType object by parsing the type description contained in the given
string. [...] If no type exists of this name in the document [...] the operation
shall fail and return a nil result."

**Repo:** `crates/xml-compose/src/lib.rs::create_type_w_document`

**Tests:** `crates/xml-compose/tests/create_type_operations.rs::a_type_is_created_from_a_document`,
`crates/xml-compose/tests/create_type_operations.rs::a_missing_type_and_a_bad_document_fail`,
`crates/xml-compose/tests/create_type_operations.rs::the_include_paths_are_searched`

**Status:** done — `create_type_w_document(document, type_name, include_paths)` in `zerodds-xml-compose`: der Typ dieses Namens aus dem XML-Dokument; ein fehlender Typ und ein Dokument mit Fehler geben einen Fehler.

### §7.5.2.2.10 Table 48 — delete_type

**Spec:** §7.5.2.2.10, Table 48, S. 195, 198 — "Delete the given DynamicType
object [...] Deletions of nil [...] Deletions of objects returned by
get_primitive_type [...] shall always succeed but shall have no observable
effect [...] RETCODE_ALREADY_DELETED".

**Repo:** `crates/types/src/dynamic/factory.rs::delete_type`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_primitive_type_is_answered_and_a_composite_kind_is_not`

**Status:** done — `delete_type(DynamicType)` gelingt; der Typ wird mit seinem letzten Halter freigegeben.

---

## §7.5.2.3 AnnotationDescriptor

### §7.5.2.3.4 Table 49 — Property type

**Spec:** §7.5.2.3.4, Table 49, S. 200–202 — "The type property contains a
reference to the annotation type [...] When an annotation descriptor is newly
created, this reference shall be nil."

**Repo:** `crates/types/src/dynamic/annotation.rs::AnnotationDescriptor`,
`crates/types/src/dynamic/annotation.rs::AnnotationType`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`,
`crates/types/tests/dynamic_annotation_descriptors.rs::a_type_reports_its_builtin_and_declared_annotations`

**Status:** done — `AnnotationDescriptor::annotation_type()` ist ein `AnnotationType` der Art `ANNOTATION_TYPE`; ein neuer Descriptor hat keinen.

### §7.5.2.3.5 Table 49 — Property value

**Spec:** §7.5.2.3.5, Table 49, S. 200, 202 — "This property contains a mapping
from the names of attributes defined by type to valid values of that type. Any
attribute defined by type but for which no name appears in this property shall
be considered to have its default value."

**Repo:** `crates/types/src/dynamic/annotation.rs::get_all_value`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_reports_its_builtin_and_declared_annotations`

**Status:** done — `value` ist eine Abbildung von Attributnamen auf Zeichenketten; ein Attribut ohne Eintrag hat seinen Standardwert.

### §7.5.2.3.1 Table 49 — Operation copy_from

**Spec:** §7.5.2.3.1, Table 49, S. 200–201 — "Overwrite the contents of this
descriptor with those of another descriptor such that subsequent calls to
equals, passing the same argument as to this method, return true."

**Repo:** `crates/types/src/dynamic/annotation.rs::copy_from`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`

**Status:** done — `AnnotationDescriptor::copy_from(&other)`.

### §7.5.2.3.2 Table 49 — Operation equals

**Spec:** §7.5.2.3.2, Table 49, S. 200–201 — "Two annotation descriptors ad1 and
ad2 are considered equal if and only if all of the following apply: Their type
properties refer to equal types. [...]"

**Repo:** `crates/types/src/dynamic/annotation.rs::equals`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`

**Status:** done — `AnnotationDescriptor::equals(&other)`: gleiche Typen und gleiche Attribute mit gleichen Werten.

### §7.5.2.3.3 Table 49 — Operation is_consistent

**Spec:** §7.5.2.3.3, Table 49, S. 200–201 — "The type property refers to a
non-nil type of kind ANNOTATION_TYPE. [...] String s2 is a well-formed string
representation of an object of the type of the attribute named by s1."

**Repo:** `crates/types/src/dynamic/annotation.rs::is_consistent`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`

**Status:** done — `AnnotationDescriptor::is_consistent()`: der Typ ist nicht nil und eine Annotationsart, jedes Attribut gehört zur Art, jeder Wert ist eine wohlgeformte Zeichenkette seines Typs.

### §7.5.2.3.5 AnnotationDescriptor: Wohlgeformte Werte: keine Anführungszeichen, ausgewertete Ausdrücke

**Spec:** §7.5.2.3.5, Table 49, S. 202 — "String and character literals shall
not be surrounded by quotation characters [...] All expressions shall be fully
evaluated"

**Repo:** `crates/types/src/dynamic/annotation.rs::is_well_formed`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::an_annotation_descriptor_has_value_semantics`

**Status:** done — Ein Wert ohne Anführungszeichen und ohne Operatoren: `"5"` ist wohlgeformt für ein Ganzzahlattribut, `"2 + 3"` nicht (`ValueKind::is_well_formed`).

---

## §7.5.2.4 TypeDescriptor

### §7.5.2.4 TypeDescriptor: Wertsemantik

**Spec:** §7.5.2.4, S. 202 — "A TypeDescriptor packages together the state of a
type. TypeDescriptor objects have value semantics, allowing them to be deeply
copied and compared."

**Repo:** `crates/types/src/dynamic/descriptor.rs::TypeDescriptor` (Z. 381);
`crates/types/src/dynamic/type_.rs::descriptor` (Z. 189)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`

**Status:** done — ein Wert (`Clone`, `PartialEq`), aus dem Modell gelesen.

### §7.5.2.4.9 Table 50 — kind

**Spec:** §7.5.2.4.9, Table 50, S. 203, 205 — "An enumerated value that
indicates what 'kind' of type this descriptor describes".

**Repo:** `crates/types/src/dynamic/type_.rs::kind` (Z. 153);
`crates/types/src/dynamic/descriptor.rs::TypeKind` (Z. 48)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`,
`crates/types/src/dynamic/descriptor.rs::tests::octet_and_uint8_are_two_kinds`

**Status:** done — Konstrukte ohne XTypes-Art (`fixed`, `valuetype`, `any` u.
a.) geben `TypeKind::NoType`.

### §7.5.2.4.10 Table 50 — name: voll qualifiziert

**Spec:** §7.5.2.4.10, Table 50, S. 203, 205 — "The fully qualified name of the
type described by this descriptor. To be consistent, this name must be a valid
identifier for the given type kind".

**Repo:** `crates/types/src/dynamic/type_.rs::qualified_name`,
`crates/types/src/dynamic/descriptor.rs::is_consistent`

**Tests:** `crates/types/tests/dynamic_corpus.rs::a_dynamic_type_names_the_node_the_encoder_encodes`,
`crates/types/tests/dynamic_annotation_descriptors.rs::the_descriptor_of_a_union_and_a_map_and_the_names`,
`crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `name` ist der voll qualifizierte Name ohne führendes `::` (`qualified_name`); `is_consistent` verlangt für benannte Arten Bezeichner, durch `::` getrennt.

### §7.5.2.4.1 Table 50 — base_type: Supertyp einer Struktur, Ziel eines Alias

**Spec:** §7.5.2.4.1, Table 50, S. 203 — "If this descriptor represents a
structure type, base_type indicates the supertype of that type. [...] If this
descriptor represents an alias type, base_type indicates the type being aliased.
[...] In all other cases, a consistent descriptor shall have a nil value for
this property."

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor` (Z. 189)

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_alias_names_its_target_as_the_base_type`,
`crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`

**Status:** done — Für eine Struktur der Supertyp, für einen Alias das Ziel (`base_type`, Table 50); ein Bitset trägt seine Basis ebenfalls dort.

### §7.5.2.4.1 base_type: Ziel eines Alias steht in base_type

**Spec:** §7.5.2.4.1, S. 203 — "If this descriptor represents an alias type,
base_type indicates the type being aliased. A nil value for this property is not
considered consistent."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_alias_names_its_target_as_the_base_type`

**Status:** done — `descriptor` setzt das Ziel eines Alias in `base_type`; `is_consistent` prüft es dort.

### §7.5.2.4.4 Table 50 — discriminator_type

**Spec:** §7.5.2.4.4, Table 50, S. 203–204 — "If this descriptor represents a
union type, discriminator_type indicates the type of the discriminator of the
union. It must not be nil for the descriptor to be consistent."

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor`,
`crates/types/src/dynamic/descriptor.rs::is_consistent`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::the_descriptor_of_a_union_and_a_map_and_the_names`

**Status:** done — `discriminator_type` einer Union ist der Typ des Diskriminators; ohne ihn ist der Descriptor inkonsistent.

### §7.5.2.4.2 Table 50 — bound

**Spec:** §7.5.2.4.2, Table 50, S. 203 — "If this descriptor represents an array
type, the length of the property value indicates the number of dimensions [...]
If this descriptor represents a sequence, map, bitmask, or string type, the
length of the property value is one and the integral value in that property
indicates the bound of the collection."

**Repo:** `crates/types/src/dynamic/type_.rs::one_bound`

**Tests:** `crates/types/tests/dynamic_annex_c.rs::the_bound_of_an_unbounded_collection_is_one_value`; `crates/types/tests/dynamic_type_descriptors.rs::bounds_are_checked_and_unlimited_is_unbounded`

**Status:** done — Array: eine Zahl je Dimension; Sequenz, Map, String, Bitmask: eine Zahl; unbeschränkt ist `LENGTH_UNLIMITED`; Log: `.agent/REPORT.md`.

### §7.5.2.4.2 bound einer Bitmask steht in bound

**Spec:** §7.5.2.4.2, S. 203 — "If this descriptor represents a sequence, map,
bitmask, or string type, the length of the property value is one".

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — Für eine Bitmask steht der Bound in `bound`; `bit_bound` bleibt leer.

### §7.5.2.4.5 Table 50 — element_type

**Spec:** §7.5.2.4.5, Table 50, S. 203–204 — "If this descriptor represents an
array, sequence, or string type, this property indicates the element type of the
collection. It must not be nil [...] If this descriptor represents a map type,
this property indicates the value element type [...] If this descriptor
represents a bitmask type, this property must indicate a Boolean type".

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor` (Z. 189)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`,
`crates/types/tests/dynamic_type_descriptors.rs::a_string_has_a_character_element_type`,
`crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — Array, Sequenz: das Element; Map: der Wert; String: `Char8` oder `Char16`; Bitmask: Boolean.

### §7.5.2.4.5 element_type: für String und Bitmask gesetzt

**Spec:** §7.5.2.4.5, S. 204 — "[array, sequence, or string type] It must not be
nil for the descriptor to be consistent. [...] If this descriptor represents a
bitmask type, this property must indicate a Boolean type for the descriptor to
be consistent."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_string_has_a_character_element_type`,
`crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — `descriptor` setzt für String `Char8` oder `Char16` und für Bitmask Boolean als `element_type`.

### §7.5.2.4.8 Table 50 — key_element_type

**Spec:** §7.5.2.4.8, Table 50, S. 203–205 — "If this descriptor represents a
map type, this property indicates the value element type of the map. It must not
be nil for the descriptor to be consistent."

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor`,
`crates/types/src/dynamic/descriptor.rs::is_consistent`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::the_descriptor_of_a_union_and_a_map_and_the_names`

**Status:** done — `key_element_type` einer Map ist der Schlüsseltyp; ohne ihn ist der Descriptor inkonsistent. (Der Text der Spec sagt "value element type" für den Schlüssel; Schreibfehler, in der Befunddatei aufgeführt.)

### §7.5.2.4.3 Table 50 — copy_from

**Spec:** §7.5.2.4.3, Table 50, S. 203–204 — "Overwrite the contents of this
descriptor with those of another descriptor such that subsequent calls to
equals, passing the same argument as to this method, return true."

**Repo:** `crates/types/src/dynamic/descriptor.rs::copy_from`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_descriptor_has_value_semantics`

**Status:** done — `TypeDescriptor::copy_from(&other)`.

### §7.5.2.4.6 Table 50 — equals

**Spec:** §7.5.2.4.6, Table 50, S. 203–204 — "Two type descriptors are
considered equal if and only if the values of all of the properties identified
in Table 50 above are equal in each of them."

**Repo:** `crates/types/src/dynamic/descriptor.rs::equals`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_descriptor_has_value_semantics`

**Status:** done — `TypeDescriptor::equals(&other)`: alle Eigenschaften von Table 50 gleich, die Typen als Typen.

### §7.5.2.4.7 Table 50 — is_consistent

**Spec:** §7.5.2.4.7, Table 50, S. 203–204 — "Indicates whether the states of
all of this descriptor's properties are consistent."

**Repo:** `crates/types/src/dynamic/descriptor.rs::is_consistent`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::the_descriptor_of_a_union_and_a_map_and_the_names`,
`crates/types/tests/dynamic_type_descriptors.rs::an_alias_names_its_target_as_the_base_type`

**Status:** done — `TypeDescriptor::is_consistent()` prüft jede Eigenschaft gegen die Arten, die sie tragen, und den Namen.

---

## §7.5.2.5 MemberId

### §7.5.2.5 MemberId: Alias auf UInt32, MEMBER_ID_INVALID

**Spec:** §7.5.2.5, S. 205 — "The type MemberId is an alias to UInt32 [...] It
is also used to type the constant MEMBER_ID_INVALID, which is a sentinel
indicating a member ID that is missing, irrelevant, or otherwise invalid in a
given context."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_enumerator_has_no_member_id_and_the_enumeration_as_its_type`,
`crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — `MemberId` ist `u32`, `MEMBER_ID_INVALID` ist `0x0FFF_FFFF`; die Member von Aufzählung und Bitmask tragen ihn.

---

## §7.5.2.6 DynamicTypeMember

### §7.5.2.6 Table 51 — ANNOTATION_TYPE, STRUCTURE_TYPE, UNION_TYPE: Member wie bei Aggregaten

**Spec:** §7.5.2.6, Table 51, S. 205 — "ANNOTATION_TYPE STRUCTURE_TYPE
UNION_TYPE | For these aggregated types, a 'member' in this sense has the same
meaning as it does in the definition of aggregated types generally."

**Repo:** `crates/types/src/dynamic/annotation.rs::AnnotationType::members`

**Tests:** `crates/types/tests/dynamic_annex_c.rs::an_annotation_type_has_members`

**Status:** done — `AnnotationType::members`, `member_count`, `get_member_by_index`, `get_member_by_name`, `get_member`, `get_all_members*` und `AnnotationMember`; Log: `.agent/REPORT.md`.

### §7.5.2.6 Table 51 — BITMASK_TYPE: Flags als Member vom Typ Boolean

**Spec:** §7.5.2.6, Table 51, S. 205 — "BITMASK_TYPE | Each named flag in a
bitmask shall be considered to be a 'member' of that bitmask with Boolean type."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — Die Flags einer Bitmask sind Member vom Typ Boolean; `literal_value` trägt die Position.

### §7.5.2.6 Table 51 — ENUMERATION_TYPE: Literale als Member vom Typ der Aufzählung

**Spec:** §7.5.2.6, Table 51, S. 205 — "ENUMERATION_TYPE | Each literal in the
enumeration shall be considered a 'member' of the type. These members shall have
the type of the enclosing enumeration itself."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_enumerator_has_no_member_id_and_the_enumeration_as_its_type`

**Status:** done — Die Literale einer Aufzählung sind Member vom Typ der Aufzählung ohne Member-ID; `literal_value` trägt den Wert.

### §7.5.2.6 Table 51 — ALIAS_TYPE: wie der Basistyp

**Spec:** §7.5.2.6, Table 51, S. 205 — "ALIAS_TYPE | The behavior is as it would
be for the alias's base type."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_alias_has_the_members_of_its_base_and_a_bitset_has_none`

**Status:** done — Ein Alias hat die Member seines Basistyps.

### §7.5.2.6 Table 51 — keine anderen Arten haben Member

**Spec:** §7.5.2.6, Table 51, S. 205 — "No other type kinds are considered to
have members."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_alias_has_the_members_of_its_base_and_a_bitset_has_none`

**Status:** done — Ein Bitset hat keine Member; andere Arten außer den genannten auch nicht.

### §7.5.2.6.1 Table 52 — Property annotation

**Spec:** §7.5.2.6.1, Table 52, S. 206–207 — "This property provides all
annotations previously applied to this member."

**Repo:** `crates/types/src/dynamic/type_.rs::annotations`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_reports_its_annotations`

**Status:** done — `DynamicTypeMember::annotations()`, `annotation_count()` und `annotation(index)`: die eingebauten und die deklarierten Annotationen des Members.

### §7.5.2.6.2 Table 52 — get_descriptor

**Spec:** §7.5.2.6.2, Table 52, S. 206–207 — "This operation provides a summary
of the state of this type. It overwrites the state of an application-provided
object."

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor` (Z. 853)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done — gibt einen neuen `MemberDescriptor` zurück statt ein Argument
zu überschreiben.

### §7.5.2.6.3 Table 52 — equals

**Spec:** §7.5.2.6.3, Table 52, S. 206–207 — "Two members shall be considered
equal if and only if they belong to the same type and all of their respective
properties, as identified in Table 52 above, are equal."

**Repo:** `crates/types/src/dynamic/type_.rs::equals_at`

**Tests:** `crates/types/tests/dynamic_annex_c.rs::members_of_different_types_are_not_equal`; `crates/types/tests/dynamic_type_builder_factory.rs::equals_includes_the_applied_annotations`

**Status:** done — verglichen werden der Besitzertyp, Name, ID, Index, Labels, Flags, Typ, Annotationen und `@verbatim`-Texte; Log: `.agent/REPORT.md`.

### §7.5.2.6.4 Table 52 — get_id

**Spec:** §7.5.2.6.4, Table 52, S. 206–207 — "This convenience operation
provides the member ID of this member. Its result shall be identical to the ID
value that is a member of the descriptor property."

**Repo:** `crates/types/src/dynamic/type_.rs::id` (Z. 701)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

### §7.5.2.6.5 Table 52 — get_name

**Spec:** §7.5.2.6.5, Table 52, S. 206–207 — "This convenience operation
provides the name of this member. Its result shall be identical to the name
string that is a member of the descriptor property."

**Repo:** `crates/types/src/dynamic/type_.rs::name` (Z. 658)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

---

## §7.5.2.7 MemberDescriptor

### §7.5.2.7 MemberDescriptor: Wertsemantik

**Spec:** §7.5.2.7, S. 207 — "A MemberDescriptor packages together the state of
a DynamicTypeMember. MemberDescriptor objects have value semantics".

**Repo:** `crates/types/src/dynamic/descriptor.rs::MemberDescriptor` (Z. 462)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

### §7.5.2.7.9 Table 53 — name

**Spec:** §7.5.2.7.9, Table 53, S. 207, 210 — "This property indicates the name
of this member. The value must be a well-formed member name."

**Repo:** `crates/types/src/dynamic/descriptor.rs::is_consistent_in`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::the_descriptor_of_a_union_and_a_map_and_the_names`

**Status:** done — `name` des Members; `is_consistent_in` verlangt einen Bezeichner.

### §7.5.2.7.5 Table 53 — id

**Spec:** §7.5.2.7.5, Table 53, S. 207, 209 — "When a descriptor is retrieved
from an existing member, this property shall reflect the actual ID of the
member. [...] If this member does not belong to an aggregated type, this
property must be MEMBER_ID_INVALID, or the descriptor is not consistent."

**Repo:** `crates/types/src/dynamic/type_.rs::id` (Z. 701)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`,
`crates/types/tests/dynamic_type_descriptors.rs::an_enumerator_has_no_member_id_and_the_enumeration_as_its_type`

**Status:** done — Für Struktur- und Union-Member die effektive ID; ein Enumerator, ein Bit und ein Bitfeld melden `MEMBER_ID_INVALID`.

### §7.5.2.7.5 id: Member außerhalb von Aggregaten tragen MEMBER_ID_INVALID

**Spec:** §7.5.2.7.5, S. 209 — "If this member does not belong to an aggregated
type, this property must be MEMBER_ID_INVALID".

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::an_enumerator_has_no_member_id_and_the_enumeration_as_its_type`,
`crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`

**Status:** done — `id` gibt `MEMBER_ID_INVALID` für Member außerhalb von Aggregaten.

### §7.5.2.7.5 id beim Hinzufügen: MEMBER_ID_INVALID ergibt Maximum + 1

**Spec:** §7.5.2.7.5, S. 209 — "When a descriptor is used to add a new member to
a type, this property may be set to MEMBER_ID_INVALID; in that case, the
implementation shall select an ID for the new member that is one more than the
current maximum member ID in the type."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_member_with_no_id_gets_the_maximum_plus_one`

**Status:** done — Ein Member ohne ID erhält das Maximum der vorhandenen IDs plus 1.

### §7.5.2.7.10 Table 53 — type

**Spec:** §7.5.2.7.10, Table 53, S. 207, 210 — "This property indicates the type
of the member's value. It must not be nil and must indicate a type that can
legally type a member".

**Repo:** `crates/types/src/dynamic/type_.rs::dynamic_type` (Z. 767)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done — ein Handle, nie nil; ein nicht aufgelöster Typ ist
`DynamicError::Unresolved`.

### §7.5.2.7.3 Table 53 — default_value als String

**Spec:** §7.5.2.7.3, Table 53, S. 207–208 — "This property provides the
member's default value in string form. [...] A nil or empty string indicates
that the member takes the 'default default' value for its type."

**Repo:** `crates/types/src/dynamic/descriptor.rs::MemberDescriptor`,
`crates/types/src/dynamic/type_.rs::descriptor`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`,
`crates/types/tests/dynamic_type_builder_factory.rs::an_enumeration_and_a_bitmask_take_literals_and_flags`

**Status:** done — `MemberDescriptor::default_value` ist das `@default` als Zeichenkette (ausgewertet, ohne Anführungszeichen); leer heißt "default default". Bei einem Literal einer Aufzählung ist es der Wert, bei einem Flag die Position.

### §7.5.2.7.6 Table 53 — index

**Spec:** §7.5.2.7.6, Table 53, S. 207, 209 — "The first member shall have index
zero, the next one, and so on. [...] When a descriptor is retrieved from an
existing member, this property shall always reflect the actual index at which
the member exists."

**Repo:** `crates/types/src/dynamic/type_.rs::index` (Z. 748)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

### §7.5.2.7.6 index beim Hinzufügen: anhängen oder einschieben

**Spec:** §7.5.2.7.6, S. 209 — "any value greater than the current largest index
value in the type shall be taken to indicate that the new member will become the
last member [...] if this property is set to an index at which a member already
exists, that member and all those after it shall be shifted up".

**Repo:** `crates/types/src/dynamic/builder.rs::add_member_at`

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_member_added_at_an_index_shifts_the_others`

**Status:** done — `add_member_at` und `add_case_at` schieben die Member ab dem Index weiter; ein Index hinter dem Ende hängt an.

### §7.5.2.7.8 Table 53 — label: Int64[*]

**Spec:** §7.5.2.7.8, Table 53, S. 207, 209–210 — "label | Int64 [*]" — "If the
type to which the member belongs is a union, this property indicates the case
labels that apply to this member. If default_label is false, it must not be
empty. In addition, no two members of the same union can specify the same label
value."

**Repo:** `crates/types/src/dynamic/descriptor.rs::MemberDescriptor` (Z. 462, Feld `label`)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`,
`crates/types/tests/dynamic_type_descriptors.rs::a_union_label_past_32_bits_is_kept`

**Status:** done — `label` ist `Vec<i64>` (Table 53, `Int64 [*]`); Figure 29 nennt `Int32 [*]` (Widerspruch der Spec, in der Befunddatei aufgeführt).

### §7.5.2.7.8 label: Wert außerhalb von i32 bleibt erhalten

**Spec:** §7.5.2.7.8, S. 209 — "this property indicates the case labels that
apply to this member".

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_union_label_past_32_bits_is_kept`

**Status:** done — Ein Label jenseits von 32 Bit bleibt im Descriptor erhalten.

### §7.5.2.7.2 Table 53 — default_label

**Spec:** §7.5.2.7.2, Table 53, S. 207–208 — "For this descriptor to be
consistent, this property must be true if this descriptor identifies the default
member of a union type or false if not."

**Repo:** `crates/types/src/dynamic/descriptor.rs::is_consistent_in`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`,
`crates/types/tests/dynamic_type_builder_factory.rs::add_member_follows_table_55`

**Status:** done — `is_default_label` eines Union-Members; `is_consistent_in` verlangt Labels, wenn es nicht `default:` ist, und kein Label und kein `default:` bei anderen Arten.

### §7.5.2.7.1 Table 53 — copy_from

**Spec:** §7.5.2.7.1, Table 53, S. 208 — "Overwrite the contents of this
descriptor with those of another descriptor such that subsequent calls to equals
[...] return true."

**Repo:** `crates/types/src/dynamic/descriptor.rs::copy_from`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`

**Status:** done — `MemberDescriptor::copy_from(&other)`.

### §7.5.2.7.4 Table 53 — equals

**Spec:** §7.5.2.7.4, Table 53, S. 208–209 — "Two descriptors are considered
equal if and only if the values of all of the properties identified in Table 53
above are equal in each of them."

**Repo:** `crates/types/src/dynamic/descriptor.rs::equals`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`

**Status:** done — `MemberDescriptor::equals(&other)`.

### §7.5.2.7.7 Table 53 — is_consistent

**Spec:** §7.5.2.7.7, Table 53, S. 208–209 — "A descriptor shall be considered
consistent if and only if all of the values of its properties are considered
consistent."

**Repo:** `crates/types/src/dynamic/descriptor.rs::is_consistent_in`,
`crates/types/src/dynamic/descriptor.rs::is_consistent_for_add`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`,
`crates/types/tests/dynamic_type_builder_factory.rs::add_member_follows_table_55`

**Status:** done — `MemberDescriptor::is_consistent_in(owner)`: Id, Name, Labels und Default nach der Art des Typs, zu dem das Member gehört; `is_consistent_for_add` erlaubt `MEMBER_ID_INVALID`.

---

## §7.5.2.8 DynamicType

### §7.5.2.8 DynamicType: Referenzsemantik, Vergleich nach Wert

**Spec:** §7.5.2.8, S. 210 — "DynamicType objects have reference semantics [...]
However, the type nevertheless provides operations to allow copying and
comparison by value."

**Repo:** `crates/types/src/dynamic/type_.rs::DynamicType` (Z. 58);
`crates/types/src/dynamic/type_.rs::equals` (Z. 492)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::equality_is_structural_and_not_by_pointer`

**Status:** done — ein `Arc`-Handle; `equals` vergleicht strukturell.

### §7.5.2.8.1 Table 54 — Property annotation

**Spec:** §7.5.2.8.1, Table 54, S. 211–212 — "This property provides all
annotations that have previously been applied to this type."

**Repo:** `crates/types/src/dynamic/type_.rs::annotations`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_reports_its_builtin_and_declared_annotations`

**Status:** done — `DynamicType::annotations()`, `annotation_count()` und `annotation(index)`.

### §7.5.2.8.2 Table 54 — Property member: nach Member-ID, nur bei Aggregaten

**Spec:** §7.5.2.8.2, Table 54, S. 211–212 — "This property contains a mapping
from the member ID of a member of this (aggregated) type to the member itself.
[...] If this type is not an aggregated type, the collection of members
available through this property shall be empty."

**Repo:** `crates/types/src/dynamic/type_.rs::member_by_id` (Z. 480)

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags`,
`crates/types/tests/dynamic_type_descriptors.rs::an_enumerator_has_no_member_id_and_the_enumeration_as_its_type`

**Status:** done — `member_by_id` gilt nur für Aggregate; für Aufzählung, Bitmask und Bitset ist die Abbildung leer.

### §7.5.2.8.3 Table 54 — Property member_by_name

**Spec:** §7.5.2.8.3, Table 54, S. 211, 213 — "This property contains a mapping
from the name of a member of this type to the member itself. [...] the
constituents of enumerations, bitmasks, and other kinds of types are also
considered to be 'members'".

**Repo:** `crates/types/src/dynamic/type_.rs::member_by_name` (Z. 472)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

### §7.5.2.8.3 Table 55 — ANNOTATION_TYPE

**Spec:** §7.5.2.8.3, Table 55, S. 213 — "ANNOTATION_TYPE | The member
descriptor must describe a consistent annotation type member. If the descriptor
does not satisfy these constraints, the operation shall fail with
RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::create_annotation_type`,
`crates/types/src/dynamic/factory.rs::add_member_descriptor`,
`crates/types/src/dynamic/factory.rs::AnnotationTypeBuilder`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::an_annotation_type_is_built_and_applied`

**Status:** done — `DynamicTypeBuilderFactory::create_annotation_type(name)` gibt einen `AnnotationTypeBuilder`; `add_member_descriptor(&MemberDescriptor)` nimmt den Descriptor eines Annotation-Members (Name, Typ, `default_value`) und weist einen ab, der Key, Optional, Shared, Must Understand oder Labels setzt, einen Typ, den ein Attribut nicht haben kann, ein doppeltes Member und einen Default, der kein Wert des Typs ist, mit `RETCODE_BAD_PARAMETER`; `build` gibt den `AnnotationType`, den `apply_annotation` anwendet. Ein Annotationstyp ist kein `TypeNode` des Modells, deshalb nennt `create_type` mit `TypeKind::Annotation` die Operation und bleibt `Unrepresentable`.

### §7.5.2.8.3 Table 55 — ALIAS_TYPE: Member gehen an den Basistyp

**Spec:** §7.5.2.8.3, Table 55, S. 213 — "ALIAS_TYPE | The behavior is as it
would be for the alias's base type. If adding a member is not defined for the
alias's base type, this operation shall fail with RETCODE_PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/types/tests/dynamic_type_descriptors.rs::a_member_added_to_an_alias_goes_to_the_base_type`

**Status:** done — `add_member` auf einen Alias fügt den Member dem Basistyp hinzu.

### §7.5.2.8.3 Table 55 — BITMASK_TYPE

**Spec:** §7.5.2.8.3, Table 55, S. 213 — "BITMASK_TYPE | The member descriptor
must describe a Boolean flag with a value within the bound of this bitmask type.
If the descriptor does not satisfy these constraints, the operation shall fail
with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::add_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::an_enumeration_and_a_bitmask_take_literals_and_flags`

**Status:** done — Ein Flag muss den Typ Boolean haben und eine Position innerhalb des Bounds; sonst `RETCODE_BAD_PARAMETER`.

### §7.5.2.8.3 Table 55 — ENUMERATION_TYPE

**Spec:** §7.5.2.8.3, Table 55, S. 213 — "ENUMERATION_TYPE | The member
descriptor must describe a literal with the type of this enumeration. If the
descriptor does not satisfy these constraints, the operation shall fail with
RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::add_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::an_enumeration_and_a_bitmask_take_literals_and_flags`

**Status:** done — Ein Literal muss den Typ der Aufzählung haben; sein Wert ist `default_value`, sonst positional; sonst `RETCODE_BAD_PARAMETER`.

### §7.5.2.8.3 Table 55 — STRUCTURE_TYPE

**Spec:** §7.5.2.8.3, Table 55, S. 213 — "STRUCTURE_TYPE | The member descriptor
must describe a consistent structure member. If the descriptor does not satisfy
this constraint, the operation shall fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::add_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`,
`crates/types/tests/dynamic_type_builder_factory.rs::add_member_follows_table_55`

**Status:** done — Ein Strukturmember wird mit `is_consistent_for_add` geprüft (Name, Id, kein Label, `@optional` ohne `@default`); der Default muss im Typ des Members lesen.

### §7.5.2.8.3 Table 55 — UNION_TYPE

**Spec:** §7.5.2.8.3, Table 55, S. 213 — "UNION_TYPE | The member descriptor
must describe a consistent union member. If the descriptor does not satisfy this
constraint, the operation shall fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::add_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::add_member_follows_table_55`

**Status:** done — Ein Unionmember braucht Labels oder `default:`; sonst `RETCODE_BAD_PARAMETER`.

### §7.5.2.8.3 Table 55 trägt den Titel member_by_name, beschreibt aber add_member

**Spec:** §7.5.2.8.3, Table 55, S. 213 — "Table 55 – DynamicType::member_by_name
behavior"; §7.5.2.9.4, S. 217 — "the behavior shall be as described in Table 55
in Clause 7.5.2.8.3".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Fehler der Spec (in der Befunddatei
aufgeführt); die Zeilen sind oben als Regeln für das Hinzufügen geführt.

### §7.5.2.8.3 Lebensdauer von DynamicTypeMember

**Spec:** §7.5.2.8.3, S. 213 — "Users of the DynamicTypeMember class are not
required to explicitly delete objects of that class. [...] Changes to one
DynamicTypeMember object representing a given member shall be reflected in all
observable DynamicTypeMember objects representing the same member. [...] All
DynamicTypeMember objects representing the same member shall compare as equal".

**Repo:** `crates/types/src/dynamic/type_.rs::DynamicTypeMember` (Z. 635)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::equality_is_structural_and_not_by_pointer`

**Status:** done — ein Member ist eine Sicht auf das unveränderliche Modell;
zwei Sichten auf denselben Member sind gleich.

### §7.5.2.8.4 Table 54 — equals

**Spec:** §7.5.2.8.4, Table 54, S. 212, 214 — "Two types shall be considered
equal if and only if all of their respective properties, as identified in Table
54 above, are equal."

**Repo:** `crates/types/src/dynamic/type_.rs::equals`,
`crates/types/src/dynamic/type_.rs::equals_ignoring_annotations`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::equals_includes_the_applied_annotations`,
`crates/types/src/dynamic/type_.rs::tests::equality_is_structural_and_not_by_pointer`

**Status:** done — `DynamicType::equals` vergleicht Descriptor, Member, Basistyp, Diskriminator-, Element- und Schlüsseltyp, Zyklen eingeschlossen, und die angewandten Annotationen und `@verbatim`-Texte des Typs und jedes Members (ohne Rücksicht auf die Reihenfolge, nach `AnnotationDescriptor::equals`). Eingebaute Annotationen, die eine verglichene Eigenschaft wiederholen (`@key`, `@id`, `@final`, `@optional`, `@bit_bound`, …), zählen einmal. `equals_ignoring_annotations` ist der Vergleich ohne Annotationen, für die Prüfung auf Verträglichkeit bei `create_sample`.

### §7.5.2.8.5 Table 54 — get_annotation

**Spec:** §7.5.2.8.5, Table 54, S. 212, 214 — "This operation returns the
annotation that corresponds to the specified index, if any. The operation shall
fail if the specified index is greater than the current annotation count."

**Repo:** `crates/types/src/dynamic/type_.rs::annotation`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_reports_its_builtin_and_declared_annotations`

**Status:** done — `DynamicType::annotation(index)`.

### §7.5.2.8.6 Table 54 — get_annotation_count

**Spec:** §7.5.2.8.6, Table 54, S. 212, 214 — "This operation returns the
current number of annotations applied to the type."

**Repo:** `crates/types/src/dynamic/type_.rs::annotation_count`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::a_type_reports_its_builtin_and_declared_annotations`

**Status:** done — `DynamicType::annotation_count()`.

### §7.5.2.8.7 Table 54 — get_descriptor

**Spec:** §7.5.2.8.7, Table 54, S. 212, 214 — "This operation provides a summary
of the state of this type. It overwrites the state of an application-provided
object."

**Repo:** `crates/types/src/dynamic/type_.rs::descriptor` (Z. 189)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`

**Status:** done — gibt einen neuen `TypeDescriptor` zurück.

### §7.5.2.8.8 Table 54 — get_kind

**Spec:** §7.5.2.8.8, Table 54, S. 212, 214 — "Its result shall be the same as
the kind indicated by the type's descriptor property."

**Repo:** `crates/types/src/dynamic/type_.rs::kind` (Z. 153)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`,
`crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`

**Status:** done

### §7.5.2.8.9 Table 54 — get_member

**Spec:** §7.5.2.8.9, Table 54, S. 212, 214 — "This operation returns the member
that corresponds to the specified member ID, if any. If there is no member with
the specified member ID, the operation shall fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/type_.rs::member_by_id` (Z. 480)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done — `None` statt `RETCODE_BAD_PARAMETER`.

### §7.5.2.8.10 Table 54 — get_member_by_index

**Spec:** §7.5.2.8.10, Table 54, S. 212, 214 — "This operation returns the
member that corresponds to the specified index, if any. The operation shall fail
if the specified index is greater than the current member count."

**Repo:** `crates/types/src/dynamic/type_.rs::get_member_by_index`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::the_member_lookups_report_a_bad_parameter`

**Status:** done — `DynamicType::get_member_by_index(index)`: ein Index ab der Anzahl ist `RETCODE_BAD_PARAMETER`. (Die Spec sagt "greater than"; in der Befunddatei aufgeführt.)

### §7.5.2.8.11 Table 54 — get_member_by_name

**Spec:** §7.5.2.8.11, Table 54, S. 212, 214 — "If there is no member with the
specified name, the operation shall fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/type_.rs::member_by_name` (Z. 472)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done — `None` statt `RETCODE_BAD_PARAMETER`.

### §7.5.2.8.12 Table 54 — get_member_count

**Spec:** §7.5.2.8.12, Table 54, S. 212, 214 — "This operation returns the
current number of members."

**Repo:** `crates/types/src/dynamic/type_.rs::member_count` (Z. 405)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done

### §7.5.2.8.13 Table 54 — get_name: voll qualifiziert

**Spec:** §7.5.2.8.13, Table 54, S. 212, 215 — "This convenience operation
provides the fully qualified name of this type. It shall be identical to the
name string that is a member of the descriptor property."

**Repo:** `crates/types/src/dynamic/type_.rs::qualified_name`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `get_name` ist der voll qualifizierte Name, gleich dem Namen des Descriptors (`qualified_name`); `name` bleibt der einfache Name.

---

## §7.5.2.9 DynamicTypeBuilder

### §7.5.2.9 DynamicTypeBuilder: Übergangszustand eines Typs

**Spec:** §7.5.2.9, S. 215 — "A DynamicTypeBuilder object represents a
transitional state of a particular type defined according to the Type System. It
is used to instantiate concrete DynamicType objects."

**Repo:** `crates/types/src/dynamic/factory.rs::TypeBuilder`,
`crates/types/src/dynamic/factory.rs::build`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder` ist der Übergangszustand eines Typs mit eigenem Modell; `build()` gibt den Typ über einem Schnappschuss, spätere Änderungen erreichen ihn nicht.

### §7.5.2.9.1 Table 56 — Property annotation

**Spec:** §7.5.2.9.1, Table 56, S. 215–216 — "This property provides all
annotations that have previously been applied to this type with
apply_annotation."

**Repo:** `crates/types/src/dynamic/factory.rs::get_annotation_count`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::get_annotation_count()` und `get_annotation(index)`.

### §7.5.2.9.2 Table 56 — Property member

**Spec:** §7.5.2.9.2, Table 56, S. 215–216 — "This property contains a mapping
from the member ID of a member of this (aggregated) type to the member itself."

**Repo:** `crates/types/src/dynamic/factory.rs::all_members_by_id`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::all_members_by_id()` und `get_member(id)`.

### §7.5.2.9.3 Table 56 — Property member_by_name

**Spec:** §7.5.2.9.3, Table 56, S. 215–216 — "This property contains a mapping
from the name of a member of this type to the member itself."

**Repo:** `crates/types/src/dynamic/factory.rs::all_members_by_name`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::all_members_by_name()` und `get_member_by_name(name)`.

### §7.5.2.9.10 Table 56 — get_annotation

**Spec:** §7.5.2.9.10, Table 56, S. 215, 218 — "This operation returns the
annotation that corresponds to the specified index, if any."

**Repo:** `crates/types/src/dynamic/factory.rs::get_annotation`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::get_annotation(index)`.

### §7.5.2.9.11 Table 56 — get_annotation_count

**Spec:** §7.5.2.9.11, Table 56, S. 215, 218 — "This operation returns the
current number of annotations applied to the type."

**Repo:** `crates/types/src/dynamic/factory.rs::get_annotation_count`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::get_annotation_count()`.

### §7.5.2.9.8 Table 56 — get_descriptor

**Spec:** §7.5.2.9.8, Table 56, S. 215, 217 — "This operation provides a summary
of the state of this type."

**Repo:** `crates/types/src/dynamic/factory.rs::get_descriptor`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::get_descriptor()`.

### §7.5.2.9.12 Table 56 — get_kind

**Spec:** §7.5.2.9.12, Table 56, S. 215, 218 — "This convenience operation
indicates the kind of this type"

**Repo:** `crates/types/src/dynamic/factory.rs::get_kind`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_kind()`.

### §7.5.2.9.13 Table 56 — get_member

**Spec:** §7.5.2.9.13, Table 56, S. 215–216, 218 — "This operation returns the
member that corresponds to the specified member ID, if any."

**Repo:** `crates/types/src/dynamic/factory.rs::get_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_member(id)`.

### §7.5.2.9.14 Table 56 — get_member_by_index

**Spec:** §7.5.2.9.14, Table 56, S. 216, 218 — "This operation returns the
member that corresponds to the specified index, if any."

**Repo:** `crates/types/src/dynamic/factory.rs::get_member_by_index`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_member_by_index(index)`.

### §7.5.2.9.15 Table 56 — get_member_by_name

**Spec:** §7.5.2.9.15, Table 56, S. 216, 218 — "This operation returns the
member that corresponds to the specified name, if any."

**Repo:** `crates/types/src/dynamic/factory.rs::get_member_by_name`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_member_by_name(name)`.

### §7.5.2.9.16 Table 56 — get_member_count

**Spec:** §7.5.2.9.16, Table 56, S. 216, 218 — "This operation returns the
current number of members."

**Repo:** `crates/types/src/dynamic/factory.rs::get_member_count`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_member_count()`.

### §7.5.2.9.17 Table 56 — get_name

**Spec:** §7.5.2.9.17, Table 56, S. 216, 218 — "This convenience operation
provides the fully qualified name of this type."

**Repo:** `crates/types/src/dynamic/factory.rs::get_name`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`

**Status:** done — `TypeBuilder::get_name()`.

### §7.5.2.9.9 Table 56 — equals

**Spec:** §7.5.2.9.9, Table 56, S. 215, 217 — "Two types shall be considered
equal if and only if all of their respective properties, as identified in Table
56 above, are equal."

**Repo:** `crates/types/src/dynamic/factory.rs::equals`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`,
`crates/types/tests/dynamic_type_builder_factory.rs::a_copy_carries_the_state_and_leaves_the_original_alone`

**Status:** done — `TypeBuilder::equals(&DynamicType)`: der Typ im Bau gleicht dem Typ.

### §7.5.2.9.4 Table 56 — add_member

**Spec:** §7.5.2.9.4, Table 56, S. 215, 217 — "Add a 'member' to this type [...]
For type kinds not given in that table, this operation shall fail with
RETCODE_PRECONDITION_NOT_MET. [...] If this argument is nil, the operation shall
fail with RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::add_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::a_structure_is_created_from_a_descriptor_and_members_are_added`,
`crates/types/tests/dynamic_type_builder_factory.rs::add_member_follows_table_55`

**Status:** done — `TypeBuilder::add_member(&MemberDescriptor)` nach Table 55; die Nummerierung ohne Id ist Maximum plus 1, der Index schiebt die übrigen.

### §7.5.2.9.5 Table 56 — apply_annotation

**Spec:** §7.5.2.9.5, Table 56, S. 215, 217 — "Apply the given annotation to
this type. It shall subsequently appear in the annotation property. [...] If
this argument is not consistent, the operation shall fail with
RETCODE_BAD_PARAMETER."

**Repo:** `crates/types/src/dynamic/factory.rs::apply_annotation`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::apply_annotation(&AnnotationDescriptor)`: eingebaute Annotationen setzen die typisierten Felder, eine deklarierte wird mit ihrer Deklaration kopiert; ein inkonsistenter Descriptor ist `RETCODE_BAD_PARAMETER`.

### §7.5.2.9.6 Table 56 — apply_annotation_to_member

**Spec:** §7.5.2.9.6, Table 56, S. 215, 217 — "Apply the given annotation to
this member. It shall subsequently appear in the annotation property of the
identified member."

**Repo:** `crates/types/src/dynamic/factory.rs::apply_annotation_to_member`

**Tests:** `crates/types/tests/dynamic_type_builder_factory.rs::annotations_are_applied_to_the_type_and_to_a_member`

**Status:** done — `TypeBuilder::apply_annotation_to_member(id, &AnnotationDescriptor)`; eine Id ohne Member ist `RETCODE_BAD_PARAMETER`.

### §7.5.2.9.7 Table 56 — build: unveränderlicher Schnappschuss

**Spec:** §7.5.2.9.7, Table 56, S. 215, 217 — "Create an immutable DynamicType
object containing a snapshot of this builder's current state. Subsequent changes
to this builder, if any, shall have no observable effect on the states of any
previously created DynamicTypes."

**Repo:** `crates/types/src/dynamic/builder.rs::finish` (Z. 236);
`crates/types/src/dynamic/builder.rs::finish_as` (Z. 244)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::equality_is_structural_and_not_by_pointer`,
`crates/types/src/dynamic/type_.rs::tests::a_view_follows_an_alias_and_an_element`

**Status:** done — `finish` verbraucht den Builder; das Modell ist danach
unveränderlich.

---

## §7.5.2.10 DynamicDataFactory

### §7.5.2.10 DynamicDataFactory: logisch ein Singleton

**Spec:** §7.5.2.10, S. 218 — "This class is logically a singleton [...] Its
'only' instance is the starting point for creating and deleting DynamicData and
objects".

**Repo:** `crates/types/src/dynamic/sample.rs::DynamicDataFactory`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — `DynamicDataFactory` hat keinen Zustand.

### §7.5.2.10.4 Table 57 — get_instance

**Spec:** §7.5.2.10.4, Table 57, S. 219 — "Return a DynamicDataFactory instance
that behaves like a singleton [...] It is legal to call this operation even
after delete_instance has been called."

**Repo:** `crates/types/src/dynamic/sample.rs::get_instance`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — `DynamicDataFactory::get_instance()`.

### §7.5.2.10.3 Table 57 — delete_instance

**Spec:** §7.5.2.10.3, Table 57, S. 219 — "Reclaim any resources associated with
the object(s) previously returned from get_instance."

**Repo:** `crates/types/src/dynamic/sample.rs::delete_instance`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sample_with_a_loan_out_is_not_deleted`

**Status:** done — `DynamicDataFactory::delete_instance()` gelingt.

### §7.5.2.10.1 Table 57 — create_data

**Spec:** §7.5.2.10.1, Table 57, S. 219 — "Create and return a new data sample.
All objects returned by this operation should eventually be deleted by calling
delete_data. Parameter type - The type of the sample to create."

**Repo:** `crates/types/src/dynamic/sample.rs::create_data`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — `create_data(&DynamicType)`: jedes nicht optionale Member auf dem Default (Table 9), optionale Member abwesend.

### §7.5.2.10.2 Table 57 — delete_data

**Spec:** §7.5.2.10.2, Table 57, S. 219 — "Dispose of a data sample, reclaiming
any associated resources."

**Repo:** `crates/types/src/dynamic/sample.rs::delete_data`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sample_with_a_loan_out_is_not_deleted`

**Status:** done — `delete_data(DynamicSample)`; ein Sample mit offener Leihe ist `RETCODE_PRECONDITION_NOT_MET`.

---

## §7.5.2.11 DynamicData

### §7.5.2.11 DynamicData: Objekt des Typs, Referenzsemantik, equals

**Spec:** §7.5.2.11, S. 220 — "Each object of the DynamicData class represents a
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

**Status:** done — `DynamicSample` trägt seinen `DynamicType` und hat Referenzsemantik: der Wert, den `get_complex_value` oder `loan_value` gibt, ist der Wert im Sample, und ein Setter wirkt in beide Richtungen; `clone` ist die Kopie (§7.5.2.11.4), `share` ein weiterer Griff auf dasselbe Sample, `equals` vergleicht Typen und Werte nach §7.5.2.11.5. Ein Zeichen eines Strings, ein Flag einer Bitmask und ein nicht gewählter Member einer Union haben keinen Speicher und kommen als Kopie bzw. als `RETCODE_NO_DATA`; ein Griff, dessen Wert entfernt wurde, liest und schreibt `RETCODE_NO_DATA`.

### §7.5.2.11.8 Table 58 — Property type: nie nil

**Spec:** §7.5.2.11.8, Table 58, S. 220, 224 — "This property provides the type
that defines the values within this sample. Its value shall not be nil."

**Repo:** `crates/types/src/dynamic/sample.rs::type_`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — `DynamicSample::type_()` gibt den Typ; ein Sample ohne Typ gibt es nicht.

### §7.5.2.11.1 Table 58 — Property value: Werte nach Name, Member-ID oder Index

**Spec:** §7.5.2.11.1, Table 58, S. 220–221 — "Many of the properties and
operations defined by this class refer to values within the sample, which are
identified by name, member ID, or index."

**Repo:** `crates/types/src/dynamic/sample.rs::get_member_id_by_name`,
`crates/types/src/dynamic/sample.rs::get_member_id_at_index`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`,
`crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`

**Status:** done — Die Werte eines Samples nach Name, Member-Id oder Index; welche Id welcher Wert ist, legt der Kopf von `sample.rs` je Art fest.

### §7.5.2.11.1 Werte: Aggregat: Name, Member-ID oder Index

**Spec:** §7.5.2.11.1, S. 221 — "If this object is of an aggregated type, values
correspond to the type's members and can be accessed by name, member ID, or
index."

**Repo:** `crates/types/src/dynamic/sample.rs::get_member_id_at_index`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Aggregat: `get_member_id_by_name`, die Id des Members, `get_member_id_at_index` über die vorhandenen Member.

### §7.5.2.11.1 Werte: Sequenz, String: Elemente nach Index

**Spec:** §7.5.2.11.1, S. 221 — "If this object is of a sequence or string type,
values correspond to the elements of the collection. These elements must be
accessed by index; the mapping from index to member ID is unspecified."

**Repo:** `crates/types/src/dynamic/sample.rs::write_element`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::strings_keep_their_bound_and_their_width`

**Status:** done — Sequenz, String: Elemente nach Index; der Index eine hinter dem Ende hängt an, bis zum Bound.

### §7.5.2.11.1 Werte: Map: Schlüssel als String, Werte nach Name

**Spec:** §7.5.2.11.1, S. 221 — "If this object is of a map type, values
correspond to the values of the map. Map keys are implicitly converted to
strings and can thus be used to look up map values by name."

**Repo:** `crates/types/src/dynamic/sample.rs::write_entry`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_map_is_reached_by_key`

**Status:** done — Map: `get_member_id_by_name(key)` gibt den Eintrag oder den Index hinter dem Ende, der den Schlüssel für den nächsten Setter merkt.

### §7.5.2.11.1 Werte: Array: flach, nach Index

**Spec:** §7.5.2.11.1, S. 221 — "If the object is of an array type, values
correspond to the elements of the array. [...] If the array is
multi-dimensional, elements are accessed as if they were 'flattened' into a
single-dimensional array"

**Repo:** `crates/types/src/dynamic/sample.rs::flat_get`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_array_is_flattened`

**Status:** done — Array: flach nach Index, ein mehrdimensionales in der Reihenfolge der IDL.

### §7.5.2.11.1 Werte: Bitmask: Flags als Boolean nach Name oder Index

**Spec:** §7.5.2.11.1, S. 221 — "If the object is of a bitmask type, values
correspond to the flags within the bitmask and are all of Boolean type. Named
flags can be accessed using that name; any bit within the bound of the bitmask
may be accessed by its index."

**Repo:** `crates/types/src/dynamic/sample.rs::resolve`,
`crates/rmw-zerodds-shim/src/dynamic.rs::flag_name`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_bitmask_has_boolean_flags_and_reads_as_an_unsigned_integer`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_bitmask_flag_is_a_boolean_by_bit`

**Status:** done — Bitmask: Flags als Boolean nach Name oder Bit; ein Bit ohne benanntes Flag ist `RETCODE_BAD_PARAMETER`, weil ein Bitmask-Wert die Namen der gesetzten Flags hält.

### §7.5.2.11.1 Werte: Aufzählung, primitiver Typ: der Wert selbst

**Spec:** §7.5.2.11.1, S. 221 — "If the object is of an enumeration or primitive
type, it has no contained values. However, the value of the sample itself may be
indicated by 'name' using a nil or empty string, by 'ID' by passing
MEMBER_ID_INVALID, or by 'index' by passing index 0."

**Repo:** `crates/types/src/dynamic/sample.rs::resolve`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — Aufzählung, primitiver Typ: der Wert selbst, `MEMBER_ID_INVALID`, nil als Name, Index 0.

### §7.5.2.11.1 Indizes relativ zum Sample

**Spec:** §7.5.2.11.1, S. 222 — "Note that indices used here are always relative
to other values in a particular DynamicData object."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erläuterung.

### §7.5.2.11.1 Table 58 — get_member_id_by_name

**Spec:** §7.5.2.11.1, Table 58, S. 220–221 — "get_member_id_by_name | MemberId
| name string<Char8,256>".

**Repo:** `crates/types/src/dynamic/sample.rs::get_member_id_by_name`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`,
`crates/types/tests/dynamic_sample.rs::a_map_is_reached_by_key`

**Status:** done — `get_member_id_by_name`.

### §7.5.2.11.1 Table 58 — get_member_id_at_index

**Spec:** §7.5.2.11.1, Table 58, S. 220–221 — "get_member_id_at_index | MemberId
| index UInt32".

**Repo:** `crates/types/src/dynamic/sample.rs::get_member_id_at_index`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`,
`crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`

**Status:** done — `get_member_id_at_index`; ein Index hinter dem Ende (außer bei Sammlungen variabler Länge um eins) ist `RETCODE_BAD_PARAMETER`.

### §7.5.2.11.2 Table 58 — Property descriptor je Wert

**Spec:** §7.5.2.11.2, Table 58, S. 220, 222 — "This property shall contain a
descriptor for each value in this object, identified by the member ID."

**Repo:** `crates/types/src/dynamic/sample.rs::get_descriptor`,
`crates/types/src/dynamic/sample.rs::set_descriptor`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_descriptor_of_a_value_is_its_types`

**Status:** done — `get_descriptor(id)` gibt den Descriptor des Members oder, bei einer Sammlung, einen mit dem Elementtyp; `set_descriptor` gelingt für den gleichen Descriptor, weil der Typ eingefroren ist.

### §7.5.2.11.3 Table 58 — clear_value

**Spec:** §7.5.2.11.3, Table 58, S. 221–222 — "clear_value | ReturnCode_t | id
MemberId".

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_optional_member_is_set_and_removed`,
`crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::an_array_is_flattened`

**Status:** done — `clear_value(id)` nach Art des Samples.

### §7.5.2.11.3 Table 58 — clear_all_values

**Spec:** §7.5.2.11.3, Table 58, S. 221–222 — "The clear_all_members takes the
above action for each value in turn."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_all_values`

**Tests:** `crates/types/tests/dynamic_sample.rs::clearing_all_values_and_the_nonkey_values`

**Status:** done — `clear_all_values()`: jeder Wert auf den Default (Table 9).

### §7.5.2.11.3 Table 58 — clear_nonkey_values: Key-Member behalten ihren Wert

**Spec:** §7.5.2.11.3, Table 58, S. 221–222 — "The clear_nonkey_value operation
has exactly the same effect as clear_all_values with one exception: the values
of key fields of aggregated types retain their values."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_nonkey_values`,
`crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_clear_nonkey_values`

**Tests:** `crates/types/tests/dynamic_sample.rs::clearing_all_values_and_the_nonkey_values`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::clear_nonkey_values_keeps_the_key_members`

**Status:** done — `clear_nonkey_values()` behält die Key-Member; in der ROS-2-Schnittstelle ebenso (`zerodds_dyn_data_clear_nonkey_values`).

### §7.5.2.11.3 Löschen: Aggregat: optional entfernen, sonst Default

**Spec:** §7.5.2.11.3, S. 222 — "If this sample is of an aggregated type, and
the indicated member is optional, remove it. If the indicated member is not
optional, set it to its default value."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`,
`crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_optional_member_is_set_and_removed`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::clear_value_removes_an_optional_member_and_a_sequence_element`

**Status:** done — Aggregat: ein optionales Member wird entfernt, ein anderes auf den Default gesetzt.

### §7.5.2.11.3 Löschen: Sammlung variabler Länge: Element entfernen, Rest rückt nach

**Spec:** §7.5.2.11.3, S. 222 — "If this sample is of a variable-length
collection type, remove the indicated element, shifting any subsequent elements
to the next-lowest index."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`,
`crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::a_map_is_reached_by_key`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::clear_value_removes_an_optional_member_and_a_sequence_element`

**Status:** done — String, Sequenz, Map: das Element wird entfernt und der Rest rückt nach.

### §7.5.2.11.3 Löschen: Array: Element auf Default

**Spec:** §7.5.2.11.3, S. 222 — "If the sample is of an array type, set the
indicated element to its default value."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_array_is_flattened`

**Status:** done — Array: das Element bekommt den Default, die Größe bleibt.

### §7.5.2.11.3 Löschen: Bitmask: Bit löschen

**Spec:** §7.5.2.11.3, S. 222 — "If the sample is of a bitmask type, clear the
indicated bit."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`,
`crates/rmw-zerodds-shim/src/dynamic.rs::write_slot`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_bitmask_has_boolean_flags_and_reads_as_an_unsigned_integer`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_bitmask_flag_is_a_boolean_by_bit`

**Status:** done — Bitmask: das Bit wird gelöscht.

### §7.5.2.11.3 Löschen: Aufzählung: erstes Literal

**Spec:** §7.5.2.11.3, S. 222 — "If the sample is of an enumerated type, set it
to the first value of the enumerated type."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_enumeration_is_a_signed_integer`

**Status:** done — Aufzählung: der erste Wert.

### §7.5.2.11.3 Löschen: primitiver Typ: Default

**Spec:** §7.5.2.11.3, S. 222 — "If the sample is of a primitive type, set it to
its default value."

**Repo:** `crates/types/src/dynamic/sample.rs::clear_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_new_sample_has_its_type_and_defaults`

**Status:** done — Primitiver Typ: der Default.

### §7.5.2.11.4 Table 58 — clone

**Spec:** §7.5.2.11.4, Table 58, S. 221, 223 — "Create and return a new data
sample with the same contents as this one. A comparison of this object and the
clone using equals immediately following this call will return true."

**Repo:** `crates/types/src/dynamic/data.rs::DynamicData` (Z. 81, `Clone`);
`crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_clone` (Z. 1269)

**Tests:** `crates/rmw-zerodds-shim/src/dynamic.rs::tests::clone_and_equals_agree`

**Status:** done

### §7.5.2.11.5 Table 58 — equals

**Spec:** §7.5.2.11.5, Table 58, S. 220, 223 — "Their respective type
definitions are equal. All contained values are equal and occur in the same
order. [...] Members shall be compared without regard to their order. One of the
samples may omit a non-optional member that is present in the other if that
member takes its default value in the latter sample."

**Repo:** `crates/types/src/dynamic/sample.rs::equals`,
`crates/types/src/dynamic/data_equals.rs::values_equal`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_optional_member_is_set_and_removed`,
`crates/types/tests/dynamic_corpus.rs::a_dynamic_data_writes_the_octets_a_json_value_writes`

**Status:** done — `equals`: gleiche Typen, Member ohne Rücksicht auf die Reihenfolge, ein weggelassenes nicht optionales Member gleich seinem Default (`DynamicType::values_equal`).

### §7.5.2.11.6 Table 58 — get_item_count

**Spec:** §7.5.2.11.6, Table 58, S. 220, 223 — "If the object is of a collection
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

**Status:** done — `get_item_count`: Sammlung (Array: Produkt der Bounds), gesetzte Flags, Member eines Aggregats, zwei für eine Union mit gewähltem Member, eins für einen atomaren Wert.

### §7.5.2.11.7 Table 58 — loan_value: höchstens eine offene Leihe

**Spec:** §7.5.2.11.7, Table 58, S. 221, 223–224 — "A given DynamicData object
may support only a single outstanding loan at a time. [...] If an application
violates this constraint, the loan operation shall return a nil value. A loan
operation shall also return a nil value if the indicated value does not exist."

**Repo:** `crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_loan` (Z. 2396)

**Tests:** `crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_loan_writes_back_at_its_own_origin`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_second_loan_and_a_foreign_return_are_refused`

**Status:** done — `zerodds_dyn_data_loan` gibt nil, solange eine Leihe offen ist.

### §7.5.2.11.7 loan_value: zweite offene Leihe wird abgewiesen

**Spec:** §7.5.2.11.7, S. 223 — "If an application violates this constraint, the
loan operation shall return a nil value."

**Repo:** —

**Tests:** `crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_second_loan_and_a_foreign_return_are_refused`

**Status:** done — Eine zweite offene Leihe wird abgewiesen.

### §7.5.2.11.7 Table 58 — return_loaned_value

**Spec:** §7.5.2.11.7, Table 58, S. 221, 224 — "The return_loaned_value
operation shall return RETCODE_PRECONDITION_NOT_MET if the provided sample
object does not represent an outstanding loan from the sample on which the
operation is invoked."

**Repo:** `crates/rmw-zerodds-shim/src/dynamic.rs::zerodds_dyn_data_return_loan` (Z. 2420)

**Tests:** `crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_loan_writes_back_at_its_own_origin`,
`crates/rmw-zerodds-shim/src/dynamic.rs::tests::a_second_loan_and_a_foreign_return_are_refused`

**Status:** done — `zerodds_dyn_data_return_loan` weist einen Wert ab, der nicht die offene Leihe dieses Samples ist (Token, das kein anderes Sample teilt).

### §7.5.2.11.9 PSM: Methoden je Typ nach dem IDL von Annex C

**Spec:** §7.5.2.11.9, S. 224 — "The programming language-specific APIs for the
Dynamic Type and Dynamic Data classes and their companion classes shall be based
on the following IDL definitions, transformed according to the IDL language
mapping described above".

**Repo:** `crates/types/src/dynamic/sample.rs::DynamicSample`

**Tests:** `crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`,
`crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`

**Status:** done — Die Getter und Setter von Annex C als Methoden von `DynamicSample`, je Typ eine (`get_int32_value`, `set_int32_value`, … `get_complex_value`).

### §7.5.2.11.9 Erweiterungen: je primitiver Typ eine Methode

**Spec:** §7.5.2.11.9, S. 224 — "Each primitive type has its own expansion."

**Repo:** `crates/types/src/dynamic/sample.rs::integer_accessors`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`

**Status:** done — Je primitiver Typ ein Getter und ein Setter.

### §7.5.2.11.9 Erweiterungen: string und wstring

**Spec:** §7.5.2.11.9, S. 224 — "Strings of Char8 and Char16 elements have their
own expansions qualified by 'string' and 'wstring' respectively."

**Repo:** `crates/types/src/dynamic/sample.rs::get_string_value`,
`crates/types/src/dynamic/sample.rs::set_string_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::strings_keep_their_bound_and_their_width`

**Status:** done — `get_string_value`, `set_string_value`, `get_wstring_value`, `set_wstring_value`; der Bound des Strings gilt.

### §7.5.2.11.9 Erweiterungen: Aufzählung über vorzeichenbehaftete Ganzzahlen

**Spec:** §7.5.2.11.9, S. 224 — "Enumerated types shall be implicitly converted
to any signed integer type having at least as many bits as the enumerated type's
@bit_bound."

**Repo:** `crates/types/src/dynamic/sample.rs::effective_kind`

**Tests:** `crates/types/tests/dynamic_sample.rs::an_enumeration_is_a_signed_integer`

**Status:** done — Eine Aufzählung liest und setzt als vorzeichenbehaftete Ganzzahl, die so breit ist wie ihr `@bit_bound` (Standard 32); ein Wert ohne Enumerator wird abgewiesen.

### §7.5.2.11.9 Erweiterungen: Bitmask über vorzeichenlose Ganzzahlen

**Spec:** §7.5.2.11.9, S. 224 — "Bitmasks shall be implicitly converted to any
unsigned integer type having at least as many bits as the bitmask's @bit_bound."

**Repo:** `crates/types/src/dynamic/sample.rs::effective_kind`,
`crates/types/src/dynamic/sample.rs::store_integer`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_bitmask_has_boolean_flags_and_reads_as_an_unsigned_integer`

**Status:** done — Eine Bitmask liest und setzt als vorzeichenlose Ganzzahl mindestens der Breite ihres Bounds; ein gesetztes Bit ohne Flag wird abgewiesen.

### §7.5.2.11.9 Erweiterungen: Alias über den Basistyp

**Spec:** §7.5.2.11.9, S. 224 — "Alias types shall be implicitly converted to
their ultimate base type and are thus accessible through the methods appropriate
for that type."

**Repo:** `crates/types/src/dynamic/sample.rs::resolve`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`

**Status:** done — Ein Alias wird über seinen Basistyp gelesen und gesetzt (`resolve_alias`).

### §7.5.2.11.9 Erweiterungen: Sequenzen primitiver Typen im Plural, auch für Arrays

**Spec:** §7.5.2.11.9, S. 224 — "Sequences of primitive types and strings have
their own expansions in which the name of the property has been made plural.
Arrays shall also be accessible through these methods."

**Repo:** `crates/types/src/dynamic/sample.rs::get_integers`,
`crates/types/src/dynamic/sample.rs::set_values`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::an_array_is_flattened`

**Status:** done — Die Mehrzahl-Formen (`get_int32_values`, `set_int32_values`, …, `get_string_values`) für Sequenzen und Arrays, mit den Promotionen der Elemente.

### §7.5.2.11.9 Erweiterungen: complex für die übrigen Fälle

**Spec:** §7.5.2.11.9, S. 224 — "Expansions that operate on DynamicData objects,
qualified by 'complex,' catch the remaining cases"

**Repo:** `crates/types/src/dynamic/sample.rs::get_complex_value`,
`crates/types/src/dynamic/sample.rs::set_complex_value`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_loan_is_taken_and_returned`,
`crates/types/tests/dynamic_sample.rs::a_bitmask_has_boolean_flags_and_reads_as_an_unsigned_integer`

**Status:** done — `get_complex_value` und `set_complex_value` für alle übrigen Fälle; der Typ muss gleich sein.

### §7.5.2.11.9 Erweiterungen: Setter hängen an Sammlungen variabler Länge an

**Spec:** §7.5.2.11.9, S. 224 — "If a DynamicData object represents an object of
a resizable collection type (string, sequence, or map), these setters may also
be used to append new elements to the collection."

**Repo:** `crates/types/src/dynamic/sample.rs::write_element`,
`crates/types/src/dynamic/sample.rs::write_entry`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::a_map_is_reached_by_key`,
`crates/types/tests/dynamic_sample.rs::strings_keep_their_bound_and_their_width`

**Status:** done — Ein Setter hängt an einen String, eine Sequenz oder eine Map an, wenn die Id der Index hinter dem Ende ist; der Bound gilt.

### §7.5.2.11.9 Neue Elemente: ID über get_member_id_at_index oder get_member_id_by_name

**Spec:** §7.5.2.11.9, S. 225 — "For a string or sequence type, use
get_member_id_at_index to obtain an ID for the index one greater than the
current length. For a map type, use get_member_id_by_name to obtain an ID for
the new map key."

**Repo:** `crates/types/src/dynamic/sample.rs::get_member_id_at_index`,
`crates/types/src/dynamic/sample.rs::get_member_id_by_name`

**Tests:** `crates/types/tests/dynamic_sample.rs::a_sequence_grows_shrinks_and_is_read_whole`,
`crates/types/tests/dynamic_sample.rs::a_map_is_reached_by_key`

**Status:** done — Die Id eines neuen Elements kommt aus `get_member_id_at_index` (Sequenz, String) oder `get_member_id_by_name` (Map).

### §7.5.2.11.9 Promotion Int8 → Int16, Int32, Int64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Int8 → Int16, Int32, Int64, Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von Int8 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion Int16 → Int32, Int64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Int16 → Int32, Int64, Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von Int16 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion Int32 → Int64, Float64, Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Int32 → Int64, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von Int32 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion Int64 → Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Int64 → Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von Int64 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion UInt8 → Int16,Int32, Int64, UInt16, UInt32, UInt64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] UInt8 → Int16,Int32, Int64, UInt16, UInt32, UInt64, Float32, Float64,
Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von UInt8 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion UInt16 → Int32, Int64, UInt32, UInt64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] UInt16 → Int32, Int64, UInt32, UInt64, Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von UInt16 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion UInt32 → Int64, UInt64, Float64, Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] UInt32 → Int64, UInt64, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von UInt32 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion UInt64 → Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] UInt64 → Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von UInt64 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion Float32 → Float64, Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Float32 → Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von Float32 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion Float64 → Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Float64 → Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von Float64 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion Float128 → (none)

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Float128 → (none)"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Float128 hat keine Promotion; ein Getter oder Setter von Float128 arbeitet nur an einem Float128-Member (und der Setter an keinem anderen).

### §7.5.2.11.9 Promotion Char8 → Char16, Int16, Int32, Int64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Char8 → Char16, Int16, Int32, Int64, Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von Char8 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion Char16 → Int32, Int64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Char16 → Int32, Int64, Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von Char16 nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion Byte → (any)

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Byte → (any)"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von Byte nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

### §7.5.2.11.9 Promotion Boolean → Int8, Int16, Int32, Int64, UInt8, UInt16, UInt32, UInt64, Float32, Float64, Float128

**Spec:** §7.5.2.11.9, S. 225 — "it shall be possible to implicitly promote
integral types. These shall be supported during both 'get' and 'set' operations
[...] Boolean → Int8, Int16, Int32, Int64, UInt8, UInt16, UInt32, UInt64,
Float32, Float64, Float128"

**Repo:** `crates/types/src/dynamic/sample.rs::promotes`

**Tests:** `crates/types/tests/dynamic_sample.rs::the_promotion_table_of_the_specification`,
`crates/types/tests/dynamic_sample.rs::integers_promote_upwards_only`

**Status:** done — Die Promotion von Boolean nach der Liste der Spec: ein Getter des größeren Typs liest, ein Setter des kleineren setzt; umgekehrt ist es `RETCODE_BAD_PARAMETER`. Alle 15 mal 15 Paare sind gegen die Liste geprüft.

---

## §7.6 Use of the Type System by DDS

### §7.6.1 Topic Model: Topic im Netz und in der Anwendung

**Spec:** §7.6.1, S. 225–226 — "A DDS topic exists in two senses of the word: 1.
On the network, with respect to interoperability [...] 2. In application code,
with respect to portability".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung.

### §7.6.1 Ein Topic im Netz mit mehreren Typen, Prüfung konsistenter Typen beim Matching

**Spec:** §7.6.1, S. 226 — "On the network, a given topic is associated with one
or more types. [...] the Service evaluates the two endpoints to make sure that
they specify consistent types (see Clause 7.6.3.4.2 [...]) and compatible QoS".

**Repo:** `crates/dcps/src/runtime.rs::decide_type_consistency` (Z. 18903)

**Tests:** `crates/dcps/src/runtime.rs::tests::xtypes_step2_falls_back_to_the_type_name_for_a_peer_without_type_information`,
`crates/dcps/tests/xtypes_aware_match.rs::appendable_compatible_matches_at_match_time`

**Status:** done

### §7.6.1 Generische Dienste ohne Typwissen

**Spec:** §7.6.1, S. 226 — "Generic services (e.g., logger, monitor) may
discover a topic associated with one or more types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Anwendungsfall.

### §7.6.2 Nur Strukturen und Unions als Topic-Typen

**Spec:** §7.6.2, S. 226 — "The only types that may be associated with a DDS
Topic are the Aggregated Types defined in 7.2.2.4.4, that is, Structure types
[...] and Union types".

**Repo:** —

**Tests:** `crates/zerodds-build/tests/dds_type.rs::an_enumeration_and_a_bitset_are_no_topic_types`

**Status:** done — `zerodds-build` erzeugt für `enum` und `bitset` keine `DdsType`-Implementierung; Strukturen und Unions bleiben Topic-Typen.

### §7.6.2 Jeder Typ darf geschachtelt im Topic-Typ vorkommen

**Spec:** §7.6.2, S. 226 — "Any type in the type system may appear nested within
the (Aggregated Type) associated with a Topic."

**Repo:** `crates/wire-codec/src/encode.rs::write_aggregate` (Z. 931)

**Tests:** `crates/wire-probe/tests/interop_corpus.rs::every_case_produces_the_bytes_its_readme_records_in_all_four_modes`

**Status:** done

### §7.6.3 Discovery und Matching: drei Erweiterungen

**Spec:** §7.6.3, S. 226–227 — "Data Representation [...] Discovery-Time Data
Typing [...] Type Consistency Enforcement".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung.

---

## §7.6.3.1 Data Representation QoS Policy

### §7.6.3.1 DataRepresentationQosPolicy für die Aushandlung

**Spec:** §7.6.3.1, S. 227 — "This negotiation shall occur based on a new QoS
policy: DataRepresentationQosPolicy."

**Repo:** `crates/dcps/src/qos.rs::DataWriterQos` (Z. 26, Feld `data_representation` Z. 73);
`crates/dcps/src/qos.rs::DataReaderQos` (Z. 114, Feld Z. 152)

**Tests:** `crates/dcps/src/qos.rs::data_representation_tests::data_representation_defaults_none_and_settable`

**Status:** done

### §7.6.3.1.1 Kennung: vorzeichenbehaftete 2-Oktett-Zahl, negative Werte für Hersteller

**Spec:** §7.6.3.1.1, S. 227 — "Each data representation shall be identified by
a two-byte signed integer value [...] the negative values shall be reserved for
definition by DDS implementations."

**Repo:** `crates/types/src/qos.rs::DataRepresentationId` (Z. 174);
`crates/rtps/src/publication_data.rs::negotiate` (Z. 152)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::publication_data_roundtrip_preserves_data_representation`

**Status:** done — `i16`; eine unbekannte Kennung wird gelesen und bei der
Aushandlung nicht angenommen.

### §7.6.3.1.1 XCDR = 0, XML = 1, XCDR2 = 2

**Spec:** §7.6.3.1.1, S. 227 — "XCDR [...] takes the value 0. XML [...] takes
the value 1. XCDR2 [...] takes the value 2."

**Repo:** `crates/types/src/qos.rs::DataRepresentationId` (Z. 174);
`crates/rtps/src/publication_data.rs::DEFAULT_OFFER` (Z. 108)

**Tests:** `crates/dcps/src/runtime.rs::tests::data_repr_offer_str_uses_spec_ids`

**Status:** done

### §7.6.3.1.1 Policy an Topic, DataReader und DataWriter

**Spec:** §7.6.3.1.1, S. 227 — "Each Topic, DataReader and DataWriter shall have
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

**Status:** done — Topic, DataWriter und DataReader haben die Policy in Rust, in den C-Strukturen (`ZeroDdsTopicQos`, `ZeroDdsDataWriterQos`, `ZeroDdsDataReaderQos` enden in `ZeroDdsDataRepresentationQosPolicy`: `is_set`, `count`, `ids[8]`; eine Struktur aus Nullen ist der Default), in der Java-PSM und in den QoS-Sätzen der C++-PSM (`dds::topic::qos::TopicQos`, `dds::pub::qos::DataWriterQos`, `dds::sub::qos::DataReaderQos`; `DataRepresentation::is_set()` sagt, ob die Anwendung eine Liste gegeben hat; eine Liste über acht Kennungen weist die C++-PSM mit `InvalidArgumentError` ab). Das erste Element des Topics gilt für Writer, die ganze Liste für Reader; nach dem Enable ist sie nicht änderbar. Die Built-in-Topic-Daten tragen das Feld.

### §7.6.3.1.1 DataRepresentation und TypeConsistencyEnforcement in den QoS-Sätzen der C++-PSM

**Spec:** §7.6.3.1.1, S. 227 — "Each Topic, DataReader and DataWriter shall have a QoS policy DataRepresentationQosPolicy"; Annex D, S. 329–331.

**Repo:** `crates/cpp/include/dds/core/policy/CorePolicy.hpp::DataRepresentation`;
`crates/cpp/include/dds/core/policy/CorePolicy.hpp::TypeConsistencyEnforcement`;
`crates/cpp/include/dds/core/detail/policy_bridge.hpp`;
`crates/cpp/include/dds/core/qos_bridge.hpp`

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_data_representation_and_type_consistency_in_the_qos_sets`

**Status:** done — Die Typlisten von `dds::topic::qos::TopicQos`, `dds::pub::qos::DataWriterQos` und `dds::sub::qos::DataReaderQos` nehmen `DataRepresentation` auf, die von `DataReaderQos` auch `TypeConsistencyEnforcement`; `qos_bridge.hpp` reicht beide an die C-ABI und liest sie zurück. `TypeConsistencyEnforcement` hat die fünf Flags von Annex D (`ignore_sequence_bounds`, `ignore_string_bounds`, `ignore_member_names`, `prevent_type_widening`, `force_type_validation`) mit den Defaults von §7.6.3.4.1. Die PSM-Klasse hat vier Werte der Art, Annex D zwei (`DISALLOW_TYPE_COERCION`, `ALLOW_TYPE_COERCION`): `ASSIGNABLE_TYPE_CONSISTENCY` ist `ALLOW_TYPE_COERCION`, die drei anderen sind `DISALLOW_TYPE_COERCION`, und zurück gelesen wird `DISALLOW_TYPE_COERCION` als `EXACT_TYPE_TYPE_CONSISTENCY`. Auf Topic und DataWriter hat Annex D die Policy nicht (`TypeConsistencyEnforcement` nur in `DataReaderQos`).

### §7.6.3.1.1 TopicQos trägt die DataRepresentationQosPolicy

**Spec:** §7.6.3.1.1, S. 228 — "When representations are specified in the
TopicQos, the first element of the sequence applies to writers of the Topic, and
the whole sequence applies to readers of the Topic."

**Repo:** —

**Tests:** `crates/dcps/tests/topic_data_representation.rs::the_topic_qos_names_a_representation_for_its_writers_and_readers`,
`crates/dcps/tests/topic_data_representation.rs::the_default_topic_leaves_the_default_representation`

**Status:** done — `TopicQos::data_representation`; `copy_from_topic_qos` übernimmt sie für Writer und Reader.

### §7.6.3.1.1 Request-Offer, nach enable nicht änderbar

**Spec:** §7.6.3.1.1, S. 227, 229 — "This policy has request-offer semantics,
and its value cannot be changed after the entity in question has been enabled
[...] The DataRepresentationQosPolicy shall not be changeable after its
corresponding Entity has been enabled."

**Repo:** `crates/dcps/src/qos.rs::immutable_writer_change` (Z. 206);
`crates/dcps/src/qos.rs::immutable_reader_change` (Z. 241)

**Tests:** `crates/dcps/src/entity.rs::tests::immutable_if_enabled_returns_correct_error`,
`crates/dcps/tests/topic_data_representation.rs::an_enabled_topic_refuses_a_new_representation`

**Status:** done — Request-Offer; nach dem Enable ist die Policy an Writer, Reader und Topic nicht änderbar.

### §7.6.3.1.1 Writer bietet eine Darstellung an, weitere Elemente ohne Bedeutung

**Spec:** §7.6.3.1.1, S. 227–228 — "Writers offer a single representation. A
writer will use its offered policy to communicate with its matched readers.
[...] this specification does not specify any meaning for the representation
identifiers after the first, and implementations may ignore them."

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (Z. 12220)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/sedp_data_representation_pid.rs::default_offer_is_xcdr1_alone_and_both_is_a_setting`

**Status:** done — der Writer schreibt in der ersten angebotenen Darstellung und
matcht nur Reader, die diese annehmen. Der Default bietet `[XCDR1]` an (§7.6.3.1.1); `[XCDR1, XCDR2]` ist die Einstellung `wire.data_representation_offer`.

### §7.6.3.1.1 Writer ab XTYPES 1.2: Version 2, Version 1 wahlweise und zur Laufzeit konfigurierbar

**Spec:** §7.6.3.1.1, S. 228 — "Shall generate or include run-code that can
serialize using version 2 encodings. Optionally may generate or include run-code
that can serialize using version 1 encodings. In this case, they shall offer the
means to configure at run-time the encoding version used by the DataWriter".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728);
`crates/zerodds-build/src/lib.rs::encoding_for` (Z. 983)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_offer_user_override_xcdr2_only`

**Status:** done — über `DataWriterQos::data_representation` und die
Runtime-Konfiguration.

### §7.6.3.1.1 Writer und Reader bis XTYPES 1.1 kündigen XCDR2 nicht an

**Spec:** §7.6.3.1.1, S. 228 — "Writers belonging to implementations of XTYPES
version 1.1 or earlier shall not announce the XCDR2 representation identifier."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — betrifft ältere Implementierungen.

### §7.6.3.1.1 Reader ab XTYPES 1.2: Version 2 lesen, XCDR2 anfordern, XCDR wahlweise


**Spec:** §7.6.3.1.1, S. 228 — "Shall generate or include run-time code that can
deserialize version 2 encodings. Shall request XCDR2 encoding. Optionally may
[...] deserialize version 1 encodings. In this case they shall also request XCDR
encoding in addition to XCDR2 encoding."

**Repo:** `crates/dcps/src/runtime.rs::reader_accept_repr`;
`crates/dcps/src/runtime.rs::wire_reader_to_remote_writer`

**Tests:** `crates/dcps/src/runtime.rs::tests::reader_accept_repr_requests_xcdr2_and_xcdr`,
`crates/dcps/src/subscriber.rs::tests::decode_for_encap_dispatches_on_representation_and_endianness`

**Status:** done — Der Reader kündigt `[XCDR2, XCDR1]` an, ohne Einstellung: er liest beide Versionen (die Lesepfade sind getestet) und fordert XCDR2 und zusätzlich XCDR an. Ein Element des konfigurierten Angebots, das keines von beiden ist (XML), folgt. Das Angebot `wire.data_representation_offer` (Vorgabe `[XCDR1]`) gilt für die Writer. Die Vorgabe der Policy ist die leere Liste, gleich `[XCDR]`; die Klausel für Reader verlangt mehr, und der Reader im Default folgt der Klausel (siehe "Widersprüche innerhalb der Spec" in der Findings-Datei).

### §7.6.3.1.1 XML-Reader: valide und wohlgeformte Dokumente

**Spec:** §7.6.3.1.1, S. 228 — "Readers requesting the XML Data Representation
shall be prepared to receive either valid or merely well-formed XML documents."

**Repo:** `crates/dcps/src/xml_data.rs::from_xml`,
`crates/dcps/src/runtime.rs::xml_sample_as_cdr`

**Tests:** `crates/dcps/src/xml_data.rs::tests::a_value_is_written_as_its_document_and_read_back`,
`crates/dcps/src/xml_data.rs::tests::entities_and_prefixes_are_read`,
`crates/dcps/tests/dynamic_data_writer_reader.rs::linux_e2e::dynamic_data_goes_over_xml`

**Status:** done — Ein Reader, der XML anfordert (`DataRepresentationQosPolicy` mit `XML`, `wire.data_representation_offer=XML`), liest ein Dokument in der Variante Valid (mit dem Namespace `ddstype://…`, mit Präfix) und in der Variante Well-formed (ohne Namespace); Präfixe und Namespace-Deklarationen werden überlesen, Kommentare und die XML-Deklaration auch. Der Writer sendet die Variante Well-formed, weil der Empfänger den Namespace ableitet (§7.4.4.2).

### §7.6.3.1.1 Kompatibel, wenn die Darstellung des Writers in der Liste des Readers steht

**Spec:** §7.6.3.1.1, S. 228 — "If a writer's offered representation is
contained within a reader's sequence, the offer satisfies the request and the
policies are compatible. Otherwise, they are incompatible."

**Repo:** `crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (Z. 12220);
`crates/dcps/src/runtime.rs::wire_reader_to_remote_writer` (Z. 12732);
`crates/rtps/src/publication_data.rs::negotiate` (Z. 152)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_match_mode_default_is_strict`,
`crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_match_mode_tolerant_is_selectable`,
`crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_match_mode_user_strict`

**Status:** done — Der Writer verlangt, dass seine Darstellung in der Liste des Readers steht; der lokale Reader matcht im Default (`DataRepMatchMode::Strict`) nur einen Writer, dessen Darstellung in der eigenen Liste steht. Die Einstellung `wire.data_representation_match: tolerant` gibt das alte Verhalten (Überschneidung genügt) für Peers zurück, die so senden.

### §7.6.3.1.1 Reader matcht nur bei enthaltener Darstellung, Überschneidung ist eine Einstellung

**Spec:** §7.6.3.1.1, S. 228 — "Otherwise, they are incompatible."

**Repo:** —

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_match_mode_default_is_strict`,
`crates/dcps/tests/sedp_data_representation_pid.rs::runtime_config_data_rep_match_mode_tolerant_is_selectable`

**Status:** done — Im Default matcht der Reader nur, wenn die Darstellung des Writers in seiner Liste steht; die Überschneidung ist die benannte Einstellung `wire.data_representation_match: tolerant`.

### §7.6.3.1.1 Default: leere Liste, gleich [XCDR]

**Spec:** §7.6.3.1.1, S. 228 — "The default value of the
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

**Status:** done — Eine leere Liste gilt als `[XCDR]` im Abgleich und im Angebot; `create_datawriter` nimmt eine ausdrücklich leere Liste an und der Writer kodiert dann XCDR1. Die Policy bleibt im Default `None` und wird von der Runtime auf ihr Angebot gelegt, dessen erstes Element XCDR ist (`DEFAULT_OFFER = [XCDR1]`, Einstellung `wire.data_representation_offer`); weitere Elemente nach dem ersten haben nach §7.6.3.1.1 keine Bedeutung.

### §7.6.3.1.1 Table 59 — Writer XCDR, Reader XCDR: Compatible

**Spec:** §7.6.3.1.1, Table 59, S. 230 — "XCDR | XCDR | Compatible."

**Repo:** `crates/rtps/src/publication_data.rs::negotiate` (Z. 152);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (Z. 12220)

**Tests:** `crates/types/src/qos.rs::tests::an_empty_list_of_preferences_is_the_single_element_xcdr`,
`crates/discovery/tests/representation_and_type_information.rs::negotiation_matches_xcdr2_when_both_support`

**Status:** done

### §7.6.3.1.1 Table 59 — Writer XCDR, Reader XCDR and XCDR2: Compatible

**Spec:** §7.6.3.1.1, Table 59, S. 230 — "XCDR | XCDR and XCDR2 | Compatible."

**Repo:** `crates/rtps/src/publication_data.rs::negotiate` (Z. 152);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (Z. 12220)

**Tests:** `crates/types/src/qos.rs::tests::an_empty_list_of_preferences_is_the_single_element_xcdr`,
`crates/discovery/tests/representation_and_type_information.rs::negotiation_matches_xcdr2_when_both_support`

**Status:** done

### §7.6.3.1.1 Table 59 — Writer XCDR2, Reader XCDR: Not Compatible

**Spec:** §7.6.3.1.1, Table 59, S. 230 — "XCDR2 | XCDR | Not Compatible."

**Repo:** `crates/rtps/src/publication_data.rs::negotiate` (Z. 152);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (Z. 12220)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::strict_writer_xcdr2_first_reader_xcdr1_only_no_match`,
`crates/dcps/tests/sedp_data_representation_pid.rs::tolerant_writer_xcdr2_first_reader_xcdr1_only_falls_back`,
`crates/types/src/qos.rs::tests::an_empty_list_of_preferences_is_the_single_element_xcdr`

**Status:** done — Writer `[XCDR2]` gegen Reader `[XCDR]` matcht nicht, auf beiden Seiten; ein Reader `[XCDR1]` und ein Writer `[XCDR2, XCDR1]` matchen im Default nicht, unter `tolerant` ja.

### §7.6.3.1.1 Table 59 — Writer XCDR2, Reader XCDR and XCDR2: Compatible

**Spec:** §7.6.3.1.1, Table 59, S. 230 — "XCDR2 | XCDR and XCDR2 | Compatible."

**Repo:** `crates/rtps/src/publication_data.rs::negotiate` (Z. 152);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (Z. 12220)

**Tests:** `crates/types/src/qos.rs::tests::an_empty_list_of_preferences_is_the_single_element_xcdr`,
`crates/discovery/tests/representation_and_type_information.rs::negotiation_matches_xcdr2_when_both_support`

**Status:** done

### §7.6.3.1.2 Kennungen der Encapsulation 0xC0 bis 0xFF für Hersteller

**Spec:** §7.6.3.1.2, S. 231 — "encapsulation identifiers where the first byte
is in the range 0xC0 to 0xFF (both included) shall be reserved for definition by
DDS implementations and shall be interpreted based on the RTPS vendor ID."

**Repo:** `crates/dcps/src/runtime.rs::user_encap_body` (Z. 20061)

**Tests:** `crates/dcps/src/runtime.rs::tests::strip_user_encap_rejects_unknown_scheme`

**Status:** done — ZeroDDS definiert keine eigene Kennung; eine Kennung mit
erstem Oktett ungleich 0 wird abgewiesen.

### §7.6.3.1.2 XML: {0x00, 0x04}

**Spec:** §7.6.3.1.2, S. 231 — "an additional encapsulation identifier
corresponding to the XML Data Representation: XML, with the value {0x00, 0x04}."

**Repo:** `crates/dcps/src/runtime.rs::check_emittable_offer`,
`crates/dcps/src/runtime.rs::parse_data_repr_offer_str`,
`crates/dcps/src/xml_data.rs::is_xml_payload`,
`crates/dcps/src/publisher.rs::validate_writer_data_representation`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_writer_registered_with_xml_first_is_accepted`,
`crates/dcps/src/runtime.rs::tests::data_repr_offer_str_uses_spec_ids`,
`crates/dcps/src/runtime.rs::tests::an_xml_sample_reaches_the_reader_as_its_cdr_body`,
`crates/dcps/src/publisher.rs::tests::create_datawriter_rejects_invalid_data_representation_offers`

**Status:** done — `user_encap_body` nimmt `{0x00, 0x04}` nicht als CDR an; `xml_data::is_xml_payload` erkennt es, `delivered_to_user_sample` liest das Dokument über `xml_sample_as_cdr`. Ein Writer, dessen Angebot mit XML beginnt, wird angenommen (`check_emittable_offer`, `validate_writer_data_representation`, `parse_data_repr_offer_str` mit `XML`) und schreibt `{0x00, 0x04}` mit den zwei Optionsoktetten (die Zahl der Füllbytes in den zwei niedrigen Bits, §7.6.3.1.2).

### §7.6.3.1.2 Sechs Kennungen für PLAIN_CDR2, DELIMITED_CDR, PL_CDR2

**Spec:** §7.6.3.1.2, S. 231 — "Version 1.2 of this specification adds six
additional encapsulation identifiers corresponding to PLAIN_CDR2, DELIMITED_CDR,
and PL_CDR2 each with big endian or little endian encoding".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728);
`crates/wire-codec/src/value.rs::encapsulation_header` (Z. 527)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Kennung nach Version und Darstellung des äußersten Objekts

**Spec:** §7.6.3.1.2, S. 231 — "The encapsulation identifier field in an RTPS
data sub-message shall be set such that it corresponds to the encoding version
and the data representation of the outermost object".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — FINAL, Version 1, Big Endian: CDR_BE {0x00, 0x00}

**Spec:** §7.6.3.1.2, Table 60, S. 231 — "XCDR | FINAL | 1 | Big Endian | CDR_BE
| {0x00, 0x00}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — FINAL, Version 1, Little Endian: CDR_LE {0x00, 0x01}

**Spec:** §7.6.3.1.2, Table 60, S. 231 — "XCDR | FINAL | 1 | Little Endian |
CDR_LE | {0x00, 0x01}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — APPENDABLE, Version 1, Big Endian: CDR_BE {0x00, 0x00}

**Spec:** §7.6.3.1.2, Table 60, S. 231 — "XCDR | APPENDABLE | 1 | Big Endian |
CDR_BE | {0x00, 0x00}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — APPENDABLE, Version 1, Little Endian: CDR_LE {0x00, 0x01}

**Spec:** §7.6.3.1.2, Table 60, S. 232 — "XCDR | APPENDABLE | 1 | Little Endian
| CDR_LE | {0x00, 0x01}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — MUTABLE, Version 1, Big Endian: PL_CDR_BE {0x00, 0x02}

**Spec:** §7.6.3.1.2, Table 60, S. 232 — "XCDR | MUTABLE | 1 | Big Endian |
PL_CDR_BE | {0x00, 0x02}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — MUTABLE, Version 1, Little Endian: PL_CDR_LE {0x00, 0x03}

**Spec:** §7.6.3.1.2, Table 60, S. 232 — "XCDR | MUTABLE | 1 | Little Endian |
PL_CDR_LE | {0x00, 0x03}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — FINAL, Version 2, Big Endian: CDR2_BE {0x00, 0x06}

**Spec:** §7.6.3.1.2, Table 60, S. 232 — "XCDR | FINAL | 2 | Big Endian |
CDR2_BE | {0x00, 0x06}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — FINAL, Version 2, Little Endian: CDR2_LE {0x00, 0x07}

**Spec:** §7.6.3.1.2, Table 60, S. 232 — "XCDR | FINAL | 2 | Little Endian |
CDR2_LE | {0x00, 0x07}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — APPENDABLE, Version 2, Big Endian: D_CDR2_BE {0x00, 0x08}

**Spec:** §7.6.3.1.2, Table 60, S. 232 — "XCDR | APPENDABLE | 2 | Big Endian |
D_CDR2_BE | {0x00, 0x08}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — APPENDABLE, Version 2, Little Endian: D_CDR2_LE {0x00, 0x09}

**Spec:** §7.6.3.1.2, Table 60, S. 232 — "XCDR | APPENDABLE | 2 | Little Endian
| D_CDR2_LE | {0x00, 0x09}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — MUTABLE, Version 2, Big Endian: PL_CDR2_BE {0x00, 0x0a}

**Spec:** §7.6.3.1.2, Table 60, S. 232 — "XCDR | MUTABLE | 2 | Big Endian |
PL_CDR2_BE | {0x00, 0x0a}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done

### §7.6.3.1.2 Table 60 — MUTABLE, Version 2, Little Endian: PL_CDR_LE {0x00, 0x0b}

**Spec:** §7.6.3.1.2, Table 60, S. 232 — "XCDR | MUTABLE | 2 | Little Endian |
PL_CDR_LE | {0x00, 0x0b}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap` (Z. 728)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** done — die Zeile nennt `PL_CDR_LE` statt `PL_CDR2_LE` (DDSXTY14-42;
in der Befunddatei aufgeführt).

### §7.6.3.1.2 Table 60 — XML, any: XML {0x00, 0x04}

**Spec:** §7.6.3.1.2, Table 60, S. 232 — "XML | any | any | any | XML | {0x00,
0x04}".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap`

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/src/runtime.rs::tests::an_xml_sample_reaches_the_reader_as_its_cdr_body`

**Status:** done — Die Zeile XML von Table 60: `user_payload_encap(XML, …)` gibt `{0x00, 0x04, 0x00, 0x00}` für jede Extensibility und jede Byte-Reihenfolge zurück; ein Reader liest sie.

### §7.6.3.1.2 Feld options von 2 Oktetten nach der Kennung, auch bei XML

**Spec:** §7.6.3.1.2, S. 232 — "the Encapsulation Identifier is followed by a 2
byte options field. [...] The XML encapsulation identifier is also followed by a
2 byte options field".

**Repo:** `crates/dcps/src/runtime.rs::user_payload_encap`,
`crates/dcps/src/runtime.rs::pad_user_payload`

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/src/runtime.rs::tests::an_xml_sample_reaches_the_reader_as_its_cdr_body`,
`crates/dcps/src/runtime.rs::tests::an_xml_writer_sends_the_document_of_the_sample`

**Status:** done — Zwei Oktette Optionen folgen der Kennung, für XCDR und für XML; die zwei niedrigen Bits der Optionen zählen die Füllbytes nach dem Dokument (`pad_user_payload`), der Reader zieht sie ab (`padding_present`).

### §7.6.3.1.2 Options: die zwei niedrigsten Bits zählen die Füllbytes bis zur 4-Oktett-Grenze

**Spec:** §7.6.3.1.2, S. 232 — "Implementations of this specification shall set
the least significant two bits in the second byte of the options field to a
value that encodes the number of padding bytes needed after the end of the
serialized payload in order to reach the next 4-byte aligned offset. [...] These
bits in the options field shall be interpreted by the receiver to determine
where the serialized data exactly ended."

**Repo:** `crates/dcps/src/runtime.rs::user_encap_body` (Z. 20061);
`crates/wire-codec/src/decode.rs::padding_present` (Z. 281);
`crates/wire-codec/src/value.rs::encapsulation_header` (Z. 527)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_user_payload_names_its_padding_in_the_options`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_drops_the_padding_the_options_name`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_keeps_a_body_whose_counted_padding_is_absent`

**Status:** done — Der DataWriter füllt die Nutzlast mit Nullen auf ein Vielfaches von vier auf und schreibt die Anzahl in `options`; der Empfänger wertet die Bits aus.

### §7.6.3.1.2 Der DataWriter setzt die Füllbyte-Bits

**Spec:** §7.6.3.1.2, S. 232 — "shall set the least significant two bits [...]
binary 01, 10, or 11 if, respectively, there would be one, two, or three bytes
of padding needed."

**Repo:** —

**Tests:** `crates/dcps/src/runtime.rs::tests::a_user_payload_names_its_padding_in_the_options`

**Status:** done — `runtime.rs::pad_user_payload` hängt die Füllbytes an, und `user_payload_encap` setzt die zwei niedrigsten Bits von `options`.

### §7.6.3.1.2 Beispiel TypeA, TypeB, TypeC

**Spec:** §7.6.3.1.2, S. 232–233 — Beispiel mit `options` 0x0002, 0x0001,
0x0000.

**Repo:** `crates/wire-codec/src/encode.rs::encode`

**Tests:** `crates/wire-codec/tests/encapsulation_options_example.rs::the_options_count_the_padding_of_the_three_objects`

**Status:** done — Der Test kodiert `TypeA`, `TypeB` und `TypeC` mit den Werten des Beispiels in CDR Big Endian mit Encapsulation und liest die Oktette zurück: `options` 0x0002, 0x0001, 0x0000 und die Füllbytes aus Nullen. Die Zeile für O3 nennt in der Spec zweimal `O3.member2`; gemeint ist `member3` (Schreibfehler der Spec, in der Befunddatei).

### §7.6.3.1.3 Built-in-Topics: @id(0x0073) representation in Topic-, Publication- und Subscription-Daten

**Spec:** §7.6.3.1.3, S. 234 — "The topic, publication, and subscription
built-in topic data types shall each indicate the data representation of the
associated entity with a new member: @id(0x0073)
DDS::DataRepresentationQosPolicy representation;"

**Repo:** `crates/rtps/src/parameter_list.rs` (`DATA_REPRESENTATION` Z. 193);
`crates/rtps/src/topic_data.rs::TopicBuiltinTopicData` (Z. 57);
`crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData` (Z. 207, Feld `data_representation` Z. 274);
`crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData` (Z. 37)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::publication_data_emits_data_representation_pid_when_set`,
`crates/dcps/tests/sedp_data_representation_pid.rs::publication_data_roundtrip_preserves_data_representation`

**Status:** done

---

## §7.6.3.2 Discovery Built-in Topics

### §7.6.3.2.1 TypeInformation statt TypeObject in den Built-in-Topics

**Spec:** §7.6.3.2.1, S. 234 — "Send TypeInformation that include
TypeIdentifiers (instead of TypeObjects) in the discovery built-in topics. [...]
Uses a new pair of built-in endpoints to request the TypeObjects".

**Repo:** `crates/dcps/src/runtime.rs::announced_type_information` (Z. 8241)

**Tests:** `crates/dcps/src/runtime.rs::tests::issue42_publication_announces_standard_type_information`,
`crates/dcps/src/runtime.rs::tests::issue42_subscription_announces_standard_type_information`

**Status:** done

### §7.6.3.2.1 IDL: TypeIdentfierWithSize, TypeIdentifierWithDependencies, TypeInformation

**Spec:** §7.6.3.2.1, S. 234–235 — "@extensibility(APPENDABLE) struct
TypeIdentfierWithSize [...] @extensibility(MUTABLE) struct TypeInformation {
@id(0x1001) TypeIdentifierWithDependencies minimal; @id(0x1002)
TypeIdentifierWithDependencies complete; };"

**Repo:** `crates/types/src/type_information.rs::TypeIdentifierWithSize` (Z. 70);
`crates/types/src/type_information.rs::TypeIdentifierWithDependencies` (Z. 138);
`crates/types/src/type_information.rs::TypeInformation` (Z. 209)

**Tests:** `crates/discovery/tests/representation_and_type_information.rs::type_information_is_built_from_a_keyed_appendable_struct`,
`crates/types/src/type_information.rs::tests::negative_dependent_typeid_count_roundtrips`

**Status:** done — die Spec schreibt `TypeIdentfierWithSize` (Schreibfehler der
Spec, in der Befunddatei aufgeführt).

### §7.6.3.2.1 minimal.typeid_with_size und complete.typeid_with_size

**Spec:** §7.6.3.2.1, S. 235 — "The field minimal.typeid_with_size shall contain
the MINIMAL Hash TypeIdentifier of the TopicType and the serialized size of the
associated TypeObject. [...] The field complete.typeid_with_size shall contain
the COMPLETE Hash TypeIdentifier".

**Repo:** `crates/types/src/type_information.rs::from_minimal_and_complete` (Z. 223)

**Tests:** `crates/discovery/tests/representation_and_type_information.rs::type_information_is_built_from_a_keyed_appendable_struct`

**Status:** done

### §7.6.3.2.1 dependent_typeid_count: Anzahl der Abhängigkeiten oder -1

**Spec:** §7.6.3.2.1, S. 235 — "The field minimal.dependent_typeid_count shall
contain the total number of other MINIMAL Hash TypeIdentifiers that correspond
to data-types the TopicType depends on. This field may be set to -1 to indicate
it is not being provided."

**Repo:** `crates/types/src/type_information.rs::from_registry`

**Tests:** `crates/types/tests/type_information_dependencies.rs::the_count_is_the_number_of_types_the_topic_type_depends_on`,
`crates/types/tests/type_information_dependencies.rs::a_dependency_the_registry_lacks_gives_minus_one`,
`crates/types/tests/type_information_dependencies.rs::a_type_without_dependencies_has_count_zero`

**Status:** done — `TypeInformation::from_registry` füllt `dependent_typeid_count` und die Listen beider Seiten mit den abhängigen Typen samt Größe; `-1`, wenn die Registry einen Typ nicht kennt.

### §7.6.3.2.1 dependent_typeids: Auswahl frei, Länge höchstens dependent_typeid_count

**Spec:** §7.6.3.2.1, S. 235–236 — "The decision of which types to include in
the dependent_typeids is left to the implementation [...] If
dependent_typeid_count is not -1, then length of the dependent_typeids sequence
shall be less or equal to dependent_typeid_count."

**Repo:** `crates/types/src/type_information.rs::add_dependency` (Z. 248)

**Tests:** `crates/types/src/type_information.rs::tests::negative_dependent_typeid_count_roundtrips`

**Status:** done — angekündigt wird die leere Liste.

### §7.6.3.2.1 Nur direkte Hash-Identifier, SCC als Ganzes

**Spec:** §7.6.3.2.1, S. 236 — "The TypeIdentifiers included in the
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

**Status:** done — Angekündigt werden direkte Hash-Identifier. Ein Typ in einer SCC nennt sich mit dem Identifier der Komponente und seiner Position, die `SCCIdentifier` (`scc_index = 0`) der eigenen und jeder erreichten Komponente stehen unter den Abhängigkeiten, einzelne Identifier von Typen einer Komponente nicht (`TypeInformation::from_component`, `DcpsRuntime::register_type_component`).

### §7.6.3.2.1 Unbekannte Identifier über TypeLookup holen

**Spec:** §7.6.3.2.1, S. 236 — "In case some of the TypeIdentifiers announced by
a remote endpoint are not known to a DomainParticipant, it can use the built-in
TypeLookup Service to retrieve the TypeObject".

**Repo:** `crates/dcps/src/runtime.rs::send_type_lookup_request` (Z. 8277);
`crates/dcps/src/runtime.rs::send_type_dependencies_request` (Z. 8412)

**Tests:** `crates/dcps/tests/typelookup_resolution_e2e.rs::get_types_answers_the_complete_form_of_a_hash`

**Status:** done

### §7.6.3.2.2 @id(0x0007) type_name

**Spec:** §7.6.3.2.2, S. 236 — "@id(0x0007) ObjectName type_name;"

**Repo:** `crates/rtps/src/parameter_list.rs` (`TYPE_NAME` Z. 92)

**Tests:** `crates/dcps/tests/shapes_type_wire.rs::shape_type_type_name_matches_interop_convention`

**Status:** done

### §7.6.3.2.2 @id(0x0072) @optional TypeObjectV1 type (XTYPES 1.1)

**Spec:** §7.6.3.2.2, S. 236 — "@id(0x0072) @optional TypeObjectV1 type; //
XTYPES 1.1".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — optionaler Member für XTYPES 1.1; ZeroDDS
schreibt und liest ihn nicht, das Weglassen ist zulässig.

### §7.6.3.2.2 @id(0x0075) @optional TypeInformation type_information

**Spec:** §7.6.3.2.2, S. 236 — "@id(0x0075) @optional XTypes::TypeInformation
type_information; // XTYPES 1.2".

**Repo:** `crates/rtps/src/parameter_list.rs` (`TYPE_INFORMATION` Z. 195);
`crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData` (Z. 207, Feld `type_information` Z. 268)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::publication_standard_type_information_yields_type_identifier`,
`crates/dcps/tests/xtypes_aware_match.rs::subscription_standard_type_information_yields_type_identifier`

**Status:** done

### §7.6.3.2.2 Non-normative note: ohne Typinformation nur type_name

**Spec:** §7.6.3.2.2, S. 236 — "Non-normative note: When the TypeObjectV1 and
TypeInformation members [...] are omitted [...] type_name is the only way to
resolve entity matching".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — „Non-normative note“.

---

## §7.6.3.3 Built-in TypeLookup service

### §7.6.3.3.1 Zwei Built-in-Topics für Anfrage und Antwort

**Spec:** §7.6.3.3.1, S. 236 — "One built-in topic is used for TypeLookup
requests. It has two built-in endpoints [...] The second built-in topic is used
for TypeLookup replies."

**Repo:** `crates/discovery/src/type_lookup/endpoints.rs::format_service_instance_name` (Z. 31);
`crates/dcps/src/type_lookup_transport.rs::new` (Z. 25)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::typelookup_endpoints_4_distinct_guids_per_participant`

**Status:** done

### §7.6.3.3.1 DDS-RPC nicht verlangt

**Spec:** §7.6.3.3.1, S. 236 — "It is not, however, a requirement to implement
the DDS-RPC specification in order to claim compliance with this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Klarstellung.

### §7.6.3.3.2 Typen aus DDS-RPC: SampleIdentity, RequestHeader, ReplyHeader

**Spec:** §7.6.3.3.2, S. 237–238 — "struct SampleIdentity { GUID_t writer_guid;
SequenceNumber_t sequence_number; }; [...] struct RequestHeader {
SampleIndentity_t requestId; InstanceName instanceName; }; struct ReplyHeader {
dds::SampleIdentity relatedRequestId; dds::rpc::RemoteExceptionCode_t remoteEx;
};"

**Repo:** `crates/types/src/type_lookup.rs::RequestHeader` (Z. 742);
`crates/types/src/type_lookup.rs::ReplyHeader` (Z. 797)

**Tests:** `crates/types/tests/type_lookup_vendor_envelope.rs::all_four_service_envelopes_match_cyclone_serializer`

**Status:** done — der Block beginnt mit "/* END of definitions copied from
DDS-RPC */" und schreibt `SampleIndentity_t` (Schreibfehler der Spec, in der
Befunddatei aufgeführt).

### §7.6.3.3.3 Table 61 — TypeLookupServiceRequestDataWriter: ENTITYID_TL_SVC_REQ_WRITER = {{00, 03, 00}, c3}

**Spec:** §7.6.3.3.3, Table 61, S. 238 — "TypeLookupServiceRequestDataWriter |
ENTITYID_TL_SVC_REQ_WRITER = {{00, 03, 00}, c3} | TypeLookup_Request".

**Repo:** `crates/rtps/src/wire_types.rs::TL_SVC_REQ_WRITER` (Z. 506)

**Tests:** `crates/discovery/src/type_lookup/endpoints.rs::tests::entity_ids_match_spec_constants`

**Status:** done

### §7.6.3.3.3 Table 61 — TypeLookupServiceRequestDataReader: ENTITYID_TL_SVC_REQ_READER = {{00, 03, 00}, c4}

**Spec:** §7.6.3.3.3, Table 61, S. 238 — "TypeLookupServiceRequestDataReader |
ENTITYID_TL_SVC_REQ_READER = {{00, 03, 00}, c4} | TypeLookup_Request".

**Repo:** `crates/rtps/src/wire_types.rs::TL_SVC_REQ_READER` (Z. 511)

**Tests:** `crates/discovery/src/type_lookup/endpoints.rs::tests::entity_ids_match_spec_constants`

**Status:** done

### §7.6.3.3.3 Table 61 — TypeLookupServiceReplyDataWriter: ENTITYID_TL_SVC_REPLY_WRITER = {{00, 03, 01}, c3}

**Spec:** §7.6.3.3.3, Table 61, S. 238 — "TypeLookupServiceReplyDataWriter |
ENTITYID_TL_SVC_REPLY_WRITER = {{00, 03, 01}, c3} | TypeLookup_Reply".

**Repo:** `crates/rtps/src/wire_types.rs::TL_SVC_REPLY_WRITER` (Z. 516)

**Tests:** `crates/discovery/src/type_lookup/endpoints.rs::tests::entity_ids_match_spec_constants`

**Status:** done

### §7.6.3.3.3 Table 61 — TypeLookupServiceReplyDataReader: ENTITYID_TL_SVC_REPLY_READER = {{00, 03, 01}, c4}

**Spec:** §7.6.3.3.3, Table 61, S. 238 — "TypeLookupServiceReplyDataReader |
ENTITYID_TL_SVC_REPLY_READER = {{00, 03, 01}, c4} | TypeLookup_Reply".

**Repo:** `crates/rtps/src/wire_types.rs::TL_SVC_REPLY_READER` (Z. 521)

**Tests:** `crates/discovery/src/type_lookup/endpoints.rs::tests::entity_ids_match_spec_constants`

**Status:** done

### §7.6.3.3.3 Paare zum Aufrufen und zum Bedienen

**Spec:** §7.6.3.3.3, S. 239 — "The pair TypeLookupServiceRequestDataWriter and
TypeLookupServiceReplyDataReader is used to invoke the built-in TypeLookup
Service [...] The pair TypeLookupServiceRequestDataReader and
TypeLookupServiceReplyDataWriter is used to implement the TypeLookup Service".

**Repo:** `crates/dcps/src/runtime.rs::send_type_lookup_request` (Z. 8277);
`crates/dcps/src/runtime.rs::dispatch_type_lookup_datagram` (Z. 22895);
`crates/dcps/src/runtime.rs::send_type_lookup_reply` (Z. 23189)

**Tests:** `crates/dcps/tests/type_lookup_e2e.rs::server_handles_inbound_get_types_request_via_dispatch`

**Status:** done

### §7.6.3.3.3 QoS der vier Endpunkte: RELIABLE, KEEP_ALL, VOLATILE

**Spec:** §7.6.3.3.3, S. 239 — "the RELIABILITY policy shall be
DDS_RELIABLE_RELIABILITY_QOS, the HISTORY policy to DDS_KEEP_ALL_HISTORY_QOS and
the DURABILITY policy to DDS_VOLATILE_DURABILITY_QOS."

**Repo:** `crates/dcps/src/type_lookup_transport.rs::Channel::new`

**Tests:** `crates/dcps/src/type_lookup_transport.rs::tests::the_endpoints_are_reliable_keep_all_and_volatile`,
`crates/dcps/src/type_lookup_transport.rs::tests::the_history_of_a_type_lookup_endpoint_keeps_all`

**Status:** done — Die vier Endpunkte sind RELIABLE (zuverlässige Proxys, HEARTBEATs), KEEP_ALL (40 unbestätigte Samples bleiben im Cache) und VOLATILE (ein Peer, der nach einem Write passt, bekommt das Sample nicht nachgeliefert).

### §7.6.3.3.3 Hash-IDs getTypes 0x018252d3, getDependencies 0x05aafb31

**Spec:** §7.6.3.3.3, S. 239 — "const unsigned long TypeLookup_getTypes_HashId =
0x018252d3; [...] const unsigned long TypeLookup_getDependencies_HashId =
0x05aafb31;"

**Repo:** `crates/types/src/type_lookup.rs::OPERATION_GET_TYPES` (Z. 140);
`crates/types/src/type_lookup.rs::OPERATION_GET_DEPENDENCIES` (Z. 148)

**Tests:** `crates/types/src/type_lookup.rs::tests::the_operation_hashes_are_the_ones_the_clause_prints`

**Status:** done — die Unions nennen `TypeLookup_getTypes_Hash` statt `…_HashId`
(Schreibfehler der Spec, in der Befunddatei aufgeführt).

### §7.6.3.3.3 TypeLookup_getTypes_In und _Out: MUTABLE mit @hashid

**Spec:** §7.6.3.3.3, S. 239 — "@extensibility(MUTABLE) struct
TypeLookup_getTypes_In { @hashid sequence<TypeIdentifier> type_ids; };
@extensibility(MUTABLE) struct TypeLookup_getTypes_Out { @hashid
sequence<TypeIdentifierTypeObjectPair> types; @hashid
sequence<TypeIdentifierPair> complete_to_minimal; };"

**Repo:** `crates/types/src/type_lookup.rs::GetTypesRequest` (Z. 240);
`crates/types/src/type_lookup.rs::GetTypesReply` (Z. 282);
`crates/types/src/type_lookup.rs::member_ids` (Z. 122)

**Tests:** `crates/types/src/type_lookup.rs::tests::member_ids_are_the_hashid_of_their_name`,
`crates/types/src/type_lookup.rs::tests::get_types_reply_carries_types_and_complete_to_minimal`

**Status:** done

### §7.6.3.3.3 TypeLookup_getTypeDependencies_In und _Out mit continuation_point

**Spec:** §7.6.3.3.3, S. 239–240 — "struct TypeLookup_getTypeDependencies_In {
@hashid sequence<TypeIdentifier> type_ids; @hashid sequence<octet, 32>
continuation_point; }; [...] struct TypeLookup_getTypeDependencies_Out { @hashid
sequence<TypeIdentifierWithSize> dependent_typeids; @hashid sequence<octet, 32>
continuation_point; };"

**Repo:** `crates/types/src/type_lookup.rs::GetTypeDependenciesRequest` (Z. 442);
`crates/types/src/type_lookup.rs::GetTypeDependenciesReply` (Z. 491);
`crates/types/src/type_lookup.rs::ContinuationPoint` (Z. 407)

**Tests:** `crates/types/src/type_lookup.rs::tests::get_type_dependencies_request_reply_roundtrip`,
`crates/types/src/type_lookup.rs::tests::continuation_point_len_max_plus_one_on_decode_rejected`

**Status:** done

### §7.6.3.3.3 Result-, Call- und Return-Unions

**Spec:** §7.6.3.3.3, S. 239–240 — "union TypeLookup_getTypes_Result
switch(long) { case DDS_RETCODE_OK: [...] union TypeLookup_Call switch(long) {
[...] union TypeLookup_Return switch(long) {".

**Repo:** `crates/types/src/type_lookup.rs::TypeLookupCall` (Z. 605);
`crates/types/src/type_lookup.rs::TypeLookupReturn` (Z. 670)

**Tests:** `crates/types/src/type_lookup.rs::tests::type_lookup_call_roundtrips_both_operations`,
`crates/types/src/type_lookup.rs::tests::type_lookup_return_roundtrips_both_operations`,
`crates/types/src/type_lookup.rs::tests::get_types_result_roundtrips_both_arms`

**Status:** done — die Unions sind appendable (Default der Spec).

### §7.6.3.3.3 TypeLookup_Request und TypeLookup_Reply

**Spec:** §7.6.3.3.3, S. 240–241 — "@RPCRequestType struct TypeLookup_Request {
dds::rpc::RequestHeader header; TypeLookup_Call data; }; [...] @RPCReplyType
struct TypeLookup_Reply { dds::rpc::RequestHeader header; TypeLookup_Return
return; };"

**Repo:** `crates/types/src/type_lookup.rs::TypeLookupRequest` (Z. 851);
`crates/types/src/type_lookup.rs::TypeLookupReplyMessage` (Z. 886)

**Tests:** `crates/types/src/type_lookup.rs::tests::type_lookup_request_and_reply_roundtrip`,
`crates/types/tests/type_lookup_vendor_envelope.rs::all_four_service_envelopes_match_cyclone_serializer`

**Status:** done — die Antwort trägt einen `ReplyHeader`; die Spec schreibt
`RequestHeader` (Fehler der Spec, in der Befunddatei aufgeführt). Beide Hüllen
sind `@final` nach DDSXTY14-30, nicht appendable wie der Default der Spec;
DDS-XTypes 1.4 Beta 1 §7.6.3.3.3 schreibt sie so. Auf der Leitung gehen beide
standardmäßig als CDR2_LE mit der Codierungsversion 2 (1.4 Beta 1
§7.6.3.3.3, OMG-Issue DDSXTY14-29), unter der Einstellung
`wire.type_lookup_encoding = xcdr1` als CDR_LE mit Version 1 (Table 60 für
einen appendable Typ unter der Vorgabe XCDR, §7.6.3.1.1; unter XCDR1 sind
final und appendable dieselben Oktette; DDSI-RTPS 2.5, Item „§10.4
TypeLookup-Endpoints mit CDR2_LE“; Tests `crates/types/tests/type_lookup_xcdr1.rs`).

### §7.6.3.3.3 Erweiterungen der _In/_Out-Typen mit @hashid unter eigener Domain

**Spec:** §7.6.3.3.3, S. 241 — "Implementers may add their own members to these
MUTABLE types. If they do they shall use member IDs obtained using the @hashid
annotation with a string value that has an Internet domain name owned by the
implementor prefix."

**Repo:** `crates/types/src/type_lookup.rs::GetTypesRequest` (Z. 240)

**Tests:** `crates/types/src/type_lookup.rs::tests::an_unknown_member_id_is_skipped_rather_than_refused`

**Status:** done — ZeroDDS fügt keine Member hinzu; ein unbekannter Member eines
anderen Herstellers wird übersprungen.

### §7.6.3.3.4 availableBuiltinEndpoints: Bits für den TypeLookup-Dienst

**Spec:** §7.6.3.3.4, S. 241 — "This DDS XTypes specification reserves
additional bits to indicate the presence of the corresponding built-in end
points for the TypeObjectLookup Service. These bits shall be set on the
availableBuiltinEndpoints."

**Repo:** `crates/rtps/src/participant_data.rs` (`TYPE_LOOKUP_REQUEST_DATA_WRITER` Z. 71)

**Tests:** `crates/dcps/tests/type_lookup_e2e.rs::announces_type_lookup_endpoints_in_spdp`

**Status:** done

### §7.6.3.3.4 Table 62 — TypeLookupServiceRequestDataWriter: 0x00000001 << 12

**Spec:** §7.6.3.3.4, Table 62, S. 242 — "TypeLookupServiceRequestDataWriter |
(0x00000001 << 12)".

**Repo:** `crates/rtps/src/participant_data.rs` (Z. 71)

**Tests:** `crates/discovery/src/capabilities.rs::tests::type_lookup_capabilities_require_the_four_distinct_vendor_bits`

**Status:** done

### §7.6.3.3.4 Table 62 — TypeLookupServiceRequestDataReader: 0x00000001 << 13

**Spec:** §7.6.3.3.4, Table 62, S. 242 — "TypeLookupServiceRequestDataReader |
(0x00000001 << 13)".

**Repo:** `crates/rtps/src/participant_data.rs` (Z. 73)

**Tests:** `crates/discovery/src/capabilities.rs::tests::type_lookup_capabilities_require_the_four_distinct_vendor_bits`

**Status:** done

### §7.6.3.3.4 Table 62 — TypeLookupServiceReplyDataWriter: 0x00000001 << 14

**Spec:** §7.6.3.3.4, Table 62, S. 242 — "TypeLookupServiceReplyDataWriter |
(0x00000001 << 14)".

**Repo:** `crates/rtps/src/participant_data.rs` (Z. 75)

**Tests:** `crates/discovery/src/capabilities.rs::tests::type_lookup_capabilities_require_the_four_distinct_vendor_bits`

**Status:** done

### §7.6.3.3.4 Table 62 — TypeLookupServiceReplyDataReader: 0x00000001 << 15

**Spec:** §7.6.3.3.4, Table 62, S. 242 — "TypeLookupServiceReplyDataReader |
(0x00000001 << 15)".

**Repo:** `crates/rtps/src/participant_data.rs` (Z. 77)

**Tests:** `crates/discovery/src/capabilities.rs::tests::type_lookup_capabilities_require_the_four_distinct_vendor_bits`

**Status:** done

### §7.6.3.3.4 Server: Request-Reader und Reply-Writer, Client: Request-Writer und Reply-Reader

**Spec:** §7.6.3.3.4, S. 242 — "Participants implementing (as a server) the
TypeLookup service shall implement the TypeObjectServiceRequestDataReader and
TypeObjectServiceReplyDataWriter. [...] Participants using (as a client) the
TypeLookup shall implement the TypeObjectServiceRequestDataWriter and
TypeObjectServiceReplyDataReader."

**Repo:** `crates/dcps/src/type_lookup_transport.rs::new` (Z. 25)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::typelookup_endpoints_4_distinct_guids_per_participant`

**Status:** done — ZeroDDS ist Server und Client. Die Spec nennt die Endpunkte
hier `TypeObjectService…` statt `TypeLookupService…` (in der Befunddatei
aufgeführt).

### §7.6.3.3.4 instanceName: dds.builtin.TOS. und GUID in kleinen Hexziffern

**Spec:** §7.6.3.3.4, S. 242 — "The Service instanceName [...] shall be set to
the string obtained by concatenating the prefix 'dds.builtin.TOS .' With the
16-character string version of the DomainParticipant GUID encoded using
hexadecimal digits with lower case letters. There shall be no '0x' ahead of the
hexadecimal digits."

**Repo:** `crates/types/src/type_lookup.rs::service_instance_name` (Z. 838);
`crates/discovery/src/type_lookup/endpoints.rs::format_service_instance_name` (Z. 31)

**Tests:** `crates/types/src/type_lookup.rs::tests::the_service_instance_name_is_the_prefix_and_lowercase_hex`,
`crates/discovery/tests/type_lookup_service.rs::service_instance_name_format_matches_spec`

**Status:** done — 32 Hexziffern: die GUID hat 16 Oktette. Die Spec nennt 16
Zeichen, ihr Beispiel hat 15 und der Präfix ein Leerzeichen vor dem Punkt (in
der Befunddatei aufgeführt).

### §7.6.3.3.4 Antwort auf jeden angekündigten TypeIdentifier

**Spec:** §7.6.3.3.4, S. 242 — "If a participant implements the TypeLookup it
shall respond to requests for any TypeIdentifier that it announced within the
TypeInformation".

**Repo:** `crates/discovery/src/type_lookup/server.rs::handle_get_types` (Z. 94);
`crates/dcps/src/runtime.rs::announced_type_information` (Z. 8241)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::server_handle_get_types_happy_path_one_type`,
`crates/dcps/tests/typelookup_resolution_e2e.rs::get_types_answers_the_complete_form_of_a_hash`

**Status:** done — angekündigt wird nur, was die Registry des Servers hält.

### §7.6.3.3.4 RequestHeader nach DDS-RPC

**Spec:** §7.6.3.3.4, S. 242 — "The dds::rpc::RequestHeader in the
TypeLookup_Request and the TypeLookup_Reply shall be set as specified in the
[DDS-RPC] specification."

**Repo:** `crates/types/src/type_lookup.rs::RequestHeader` (Z. 742);
`crates/types/src/type_lookup.rs::ReplyHeader` (Z. 797)

**Tests:** `crates/discovery/src/type_lookup/client.rs::reply_correlation_requires_original_peer_and_operation`

**Status:** done

### §7.6.3.3.4.1 getTypeDependencies: Anfrage mit direkten Hashes einer Art, SCC als Ganzes

**Spec:** §7.6.3.3.4.1, S. 242 — "The TypeIdentifiers shall be only direct HASH
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

**Status:** done — Der Server beantwortet `getTypeDependencies` für einen Hash und für den Identifier einer Komponente als Ganzes: die Hash-Identifier der Typen, die die Member nennen, und die `SCCIdentifier` der übrigen erreichten Komponenten; der eigene steht nicht darunter. Zwei Runtimes: die Anfrage nach den Abhängigkeiten von `Outer` bringt `Inner` in die Registry des Anfragenden und nicht `Outer`; die nach einer Komponente bringt den Typ außerhalb, den ein Member nennt.

### §7.6.3.3.4.1 continuation_point in der Anfrage

**Spec:** §7.6.3.3.4.1, S. 243 — "The field continuation_point shall not be
present if the requester wants the response to include all the types [...]
Otherwise it shall be set to the continuation_point of the
TypeLookup_getTypeDependencies_Out received in response to a previous call".

**Repo:** `crates/types/src/type_lookup.rs::GetTypeDependenciesRequest` (Z. 442)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::server_continuation_point_paginates_200_dependencies`,
`crates/types/src/type_lookup.rs::tests::continuation_point_len_zero_encodes_to_four_zero_bytes`

**Status:** done — „nicht vorhanden“ ist eine leere Sequenz; der Member ist
nicht `@optional`.

### §7.6.3.3.4.1 Antwort: nur direkte Hashes, continuation_point bei unvollständiger Liste

**Spec:** §7.6.3.3.4.1, S. 243 — "The field dependent_typeids shall exclusively
contain of direct HASH TypeIdentifiers that are recursive dependencies [...] The
field continuation_point shall not be present if the response contains the
complete list of types, otherwise it shall contain an opaque value".

**Repo:** `crates/discovery/src/type_lookup/server.rs::handle_get_type_dependencies` (Z. 250)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::server_handle_get_type_dependencies_struct_with_5_deps`,
`crates/discovery/tests/type_lookup_service.rs::server_continuation_point_paginates_200_dependencies`

**Status:** done

### §7.6.3.3.4.2 getTypes: Anfrage mit direkten Hashes, SCC als Ganzes

**Spec:** §7.6.3.3.4.2, S. 243 — "The field type_ids shall contain the direct
HASH TypeIdentifiers for which the participant is requesting the TypeObjects.
The field type_ids shall not include individual TypeIdentifiers belonging to a
Strongly Connected Component".

**Repo:** `crates/dcps/src/runtime.rs::send_type_lookup_request` (Z. 8277)

**Tests:** `crates/discovery/tests/type_lookup_requests.rs::a_get_types_request_and_its_reply_round_trip_between_two_stacks`

**Status:** done

### §7.6.3.3.4.2 Antwort: COMPLETE auf COMPLETE, auf MINIMAL wahlweise

**Spec:** §7.6.3.3.4.2, S. 243 — "If the request had a COMPLETE TypeIdentifiers,
the types shall contain COMPLETE TypeObjects. If the request had MINIMAL
TypeIdentifiers the types may contain either MINIMAL or COMPLETE TypeObjects."

**Repo:** `crates/discovery/src/type_lookup/server.rs::handle_get_types` (Z. 94)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::server_handle_get_types_happy_path_one_type`,
`crates/discovery/src/type_lookup/server.rs::tests::handle_get_types_pairs_a_complete_identifier_with_its_minimal_hash`

**Status:** done

### §7.6.3.3.4.2 complete_to_minimal

**Spec:** §7.6.3.3.4.2, S. 243–244 — "The field complete_to_minimal shall
contain the mapping from COMPLETE TypeIdentifiers to MINIMAL TypeIdentifiers for
any COMPLETE TypeIdentifiers that appear within COMPLETE TypeObjects that were
sent in response to a query for a MINIMAL TypeIdentfier."

**Repo:** `crates/discovery/src/type_lookup/server.rs::handle_get_types` (Z. 94)

**Tests:** `crates/discovery/tests/type_lookup_service.rs::typeidentifier_with_minimal_hash_yields_complete_to_minimal_pair`

**Status:** done — der Server füllt die Abbildung auch für eine Anfrage mit
COMPLETE-Identifier; der Kommentar dort zitiert §7.6.3.3.4.1 statt §7.6.3.3.4.2.

### §7.6.3.3.4.2 SCC-Identifier: alle TypeObjects der Komponente oder keines

**Spec:** §7.6.3.3.4.2, S. 244 — "If a TypeIdentifier was a SCCIdentifier [...],
then the response shall threat the TypeObjects within the Strongly Connected
Components atomically. Either include all in the reply or none."

**Repo:** `crates/discovery/src/type_lookup/server.rs::handle_get_types`

**Tests:** `crates/discovery/tests/type_lookup_scc_atomic.rs::an_scc_identifier_gets_every_member_of_the_component`,
`crates/discovery/tests/type_lookup_scc_atomic.rs::an_unknown_component_or_a_wrong_length_gets_none`

**Status:** done — `getTypes` beantwortet einen SCCIdentifier mit allen TypeObjects der Komponente oder mit keinem; eine unbekannte Komponente oder eine falsche Länge bekommt keines.

---

## §7.6.3.4 Type Consistency Enforcement QoS Policy

### §7.6.3.4 TypeConsistencyEnforcementQosPolicy für DataReader

**Spec:** §7.6.3.4, S. 244 — "The Type Consistency Enforcement QoS Policy
defines the rules for determining whether the type used to publish a given data
stream is consistent with that used to subscribe to it. It applies to
DataReaders."

**Repo:** `crates/dcps/src/qos.rs::DataReaderQos` (Z. 114, Feld `type_consistency` Z. 162);
`crates/types/src/qos.rs::TypeConsistencyEnforcement` (Z. 52)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::datareader_qos_exposes_type_consistency_enforcement`

**Status:** done

### §7.6.3.4.1 Arten DISALLOW_TYPE_COERCION und ALLOW_TYPE_COERCION

**Spec:** §7.6.3.4.1, S. 244 — "DISALLOW_TYPE_COERCION: The DataWriter and the
DataReader must support the same data type in order for them to communicate.
[...] ALLOW_TYPE_COERCION: [...] as long as the reader's type is assignable from
the writer's type."

**Repo:** `crates/types/src/qos.rs::TypeConsistencyKind` (Z. 13)

**Tests:** `crates/types/src/qos.rs::tests::type_consistency_kind_has_the_two_values_of_annex_d`,
`crates/types/src/type_matcher.rs::tests::disallow_type_coercion_needs_equivalent_types`,
`crates/types/src/type_matcher_vectors.rs::disallow_coercion_rejects_int16_int32`

**Status:** done — `TypeConsistencyKind` hat die zwei Werte von Annex D, `DISALLOW_TYPE_COERCION` und `ALLOW_TYPE_COERCION`; `force_type_validation` ist ein Feld der Policy. Unter DISALLOW verlangt das Matching äquivalente Typen.

### §7.6.3.4.1 DISALLOW_TYPE_COERCION verlangt gleiche Typen

**Spec:** §7.6.3.4.1, S. 244 — "DISALLOW_TYPE_COERCION: The DataWriter and the
DataReader must support the same data type in order for them to communicate."

**Repo:** `crates/types/src/type_matcher.rs::build_config`

**Tests:** `crates/types/src/type_matcher.rs::tests::disallow_type_coercion_needs_equivalent_types`

**Status:** done — `type_matcher.rs::build_config` prüft unter DISALLOW die MINIMAL-Äquivalenz nach §7.3.4.7; ein Reader mit DISALLOW nimmt einen zuweisbaren, nicht äquivalenten Typ des Writers nicht an.

### §7.6.3.4.1 Nur für DataReader, kein RxO, nach enable nicht änderbar

**Spec:** §7.6.3.4.1, S. 244 — "This policy applies only to DataReaders; it does
not have request-offer (RxO) semantics [DDS]. The value of this policy cannot be
changed after the entity in question has been enabled."

**Repo:** `crates/dcps/src/qos.rs::immutable_reader_change` (Z. 241)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::a_readers_type_consistency_policy_survives_its_announcement`

**Status:** done — der Writer entscheidet mit der Policy, die der Reader
ankündigt.

### §7.6.3.4.1 Default ALLOW_TYPE_COERCION

**Spec:** §7.6.3.4.1, S. 244 — "The default enforcement kind shall be
ALLOW_TYPE_COERCION."

**Repo:** `crates/types/src/qos.rs::TypeConsistencyEnforcement` (Z. 67, `Default`)

**Tests:** `crates/types/src/qos.rs::tests::consistency_roundtrip_default`

**Status:** done

### §7.6.3.4.1 Fehlende Policy des entfernten Endpunkts gilt als DISALLOW_TYPE_COERCION

**Spec:** §7.6.3.4.1, S. 244 — "if it observes that no
TypeConsistencyEnforcementQosPolicy value is provided [...] it shall assume a
kind of DISALLOW_TYPE_COERCION".

**Repo:** `crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData`,
`crates/types/src/qos.rs::TypeConsistencyEnforcement`

**Tests:** `crates/rtps/src/subscription_data.rs::tests::a_reader_announcing_no_type_consistency_is_read_as_disallow`,
`crates/dcps/tests/runtime_config_layering.rs::the_xtypes_defaults_are_the_clauses_and_each_has_a_named_way_back`

**Status:** done — Ein Endpunkt ohne PID `0x0074` gilt als `DISALLOW_TYPE_COERCION` (`TypeConsistencyEnforcement::not_announced`); `SubscriptionBuiltinTopicData::type_consistency_announced` unterscheidet das von einem angekündigten DISALLOW. Der alte Zustand ist mit `xtypes.type_consistency_absent: allow` (Umgebung `ZERODDS_TYPE_CONSISTENCY_ABSENT`) erreichbar.

### §7.6.3.4.1 Erweiterung eines Typs (widening)

**Spec:** §7.6.3.4.1, S. 245 — "A type T2 is said to widen type T1 when type T2
contains non-optional fields that are not present in T1. [...] Note that
optional members in T2 that are not present on T1 do not make T2 'wider' than
T1".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung.

### §7.6.3.4.1 prevent_type_widening: Default FALSE, TRUE verbietet Zuweisung an den engeren Typ

**Spec:** §7.6.3.4.1, S. 245 — "If the option is set to FALSE (the default),
type widening is permitted. If the option is set to TRUE, it shall cause a wider
type to not be assignable to a narrower type."

**Repo:** `crates/types/src/type_matcher.rs::build_config` (Z. 119)

**Tests:** `crates/types/src/assignability.rs::tests::prevent_type_widening_refuses_a_reader_that_grew_a_member`,
`crates/types/src/type_matcher_vectors.rs::prevent_widening_rejects_int16_int32`

**Status:** done

### §7.6.3.4.1 ignore_sequence_bounds: Default TRUE

**Spec:** §7.6.3.4.1, S. 245 — "If the option is set to TRUE (the default),
sequence bounds (maximum lengths) are not considered as part of the type
assignability. [...] If the option is set to false [...] it is required that
L1>= L2."

**Repo:** `crates/types/src/qos.rs::TypeConsistencyEnforcement` (Z. 67);
`crates/types/src/type_matcher.rs::build_config` (Z. 119)

**Tests:** `crates/types/src/type_matcher_vectors.rs::sequence_bounds_ignored_by_default_tce`,
`crates/types/src/assignability.rs::tests::sequence_writer_bound_exceeds_reader_bound_is_no`

**Status:** done

### §7.6.3.4.1 ignore_string_bounds: Default TRUE

**Spec:** §7.6.3.4.1, S. 245 — "If the option is set to TRUE (the default),
string bounds (maximum lengths) are not considered as part of the type
assignability."

**Repo:** `crates/types/src/qos.rs::TypeConsistencyEnforcement` (Z. 67)

**Tests:** `crates/types/src/type_matcher_vectors.rs::string_bounds_ignored_by_default_tce`,
`crates/types/src/type_matcher_vectors.rs::string_bound_overflow_rejected_when_strict`

**Status:** done

### §7.6.3.4.1 ignore_member_names: Default FALSE

**Spec:** §7.6.3.4.1, S. 245 — "If the option is set to TRUE, member names are
considered as part of assignability in addition to member IDs [...] If the
option is set to FALSE (the default), then member names are not ignored."

**Repo:** `crates/types/src/qos.rs::TypeConsistencyEnforcement` (Z. 67);
`crates/types/src/type_matcher.rs::build_config` (Z. 119)

**Tests:** `crates/types/src/assignability.rs::tests::a_member_name_at_two_different_ids_is_refused`

**Status:** done — TRUE lässt die Namen außer Acht, wie Step 1 in §7.6.3.4.2 es
sagt. Der Satz in §7.6.3.4.1 sagt für TRUE das Gegenteil (Widerspruch der Spec,
in der Befunddatei aufgeführt).

### §7.6.3.4.1 Flags nur unter ALLOW_TYPE_COERCION wirksam

**Spec:** §7.6.3.4.1, S. 245 — "The values of prevent_type_widening,
ignore_sequence_bounds, ignore_string_bounds, and ignore_member_names only apply
when the type consistency kind is ALLOW_TYPE_COERCION, otherwise the fields are
treated as though prevent_type_widening is set to true and the others are set to
false."

**Repo:** `crates/types/src/type_matcher.rs::build_config`

**Tests:** `crates/types/src/type_matcher.rs::tests::the_flags_apply_only_under_allow_type_coercion`,
`crates/types/src/type_matcher.rs::tests::disallow_type_coercion_needs_equivalent_types`

**Status:** done — Unter DISALLOW_TYPE_COERCION lesen sich `prevent_type_widening`, `ignore_sequence_bounds`, `ignore_string_bounds` und `ignore_member_names` als true, false, false, false, auch wenn sie anders gesetzt sind; unter ALLOW_TYPE_COERCION gelten sie wie gesetzt.

### §7.6.3.4.1 force_type_validation: Default FALSE, TRUE verlangt Typinformation

**Spec:** §7.6.3.4.1, S. 246 — "The force_type_validation option requires type
information to be available in order to complete matching between a DataWriter
and DataReader when set to TRUE [...] The default value is false."

**Repo:** `crates/dcps/src/runtime.rs::decide_type_consistency` (Z. 18903)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::type_matcher_rejects_incompatible_primitives_under_force_validation`

**Status:** done

### §7.6.3.4.2 Step 1: Zuweisbarkeit unter den Flags, ohne Coercion Äquivalenz

**Spec:** §7.6.3.4.2, S. 246 — "If the Subscription allows type coercion, then
the_type indicated there must be assignable from the_type of the Publication,
taking into account the values of prevent_type_widening, ignore_sequence_bounds,
ignore_string_bounds, and ignore_member_names. If the Subscription does not
allow type coercion, then its type must be equivalent to the type of the
Publication."

**Repo:** `crates/dcps/src/runtime.rs::decide_type_consistency` (Z. 18903);
`crates/types/src/type_matcher.rs::build_config` (Z. 119)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::final_vs_appendable_mismatch_rejected_at_match_time`,
`crates/dcps/tests/xtypes_aware_match.rs::appendable_compatible_matches_at_match_time`,
`crates/types/src/type_matcher.rs::tests::disallow_type_coercion_needs_equivalent_types`

**Status:** done — Unter ALLOW gilt die Zuweisbarkeit mit den Flags, unter DISALLOW die Äquivalenz.

### §7.6.3.4.2 Step 1: ignore_member_names lässt Namen außer Acht

**Spec:** §7.6.3.4.2, S. 246 — "If the subscription allows type coercion and the
ignore_member_names flag is true [...] assignability checking shall ignore the
member names in both Subscription and Publication types. That is, only member
IDs will impact assignability."

**Repo:** `crates/types/src/type_matcher.rs::build_config`

**Tests:** `crates/types/src/type_matcher.rs::tests::ignore_member_names_lets_only_the_member_ids_decide`,
`crates/types/src/assignability.rs::tests::a_member_name_at_two_different_ids_is_refused`

**Status:** done — Mit `ignore_member_names = true` unter ALLOW_TYPE_COERCION entscheiden nur die Member-Ids: derselbe Name `x1` an Id 1 und Id 2 ist dann zuweisbar, ohne das Flag nicht; unter DISALLOW_TYPE_COERCION bleibt das Flag ohne Wirkung.

### §7.6.3.4.2 Step 2: ohne TypeObject gleiche type_name, scheitert unter force_type_validation

**Spec:** §7.6.3.4.2, S. 246 — "If either the Publication or the Subscription
does not provide a TypeObject definition, then the type names are consulted. The
Subscription and Publication type_name fields must match exactly [...] This step
will fail if force_type_validation is true, regardless of the type names."

**Repo:** `crates/dcps/src/runtime.rs::decide_type_consistency` (Z. 18903)

**Tests:** `crates/dcps/src/runtime.rs::tests::xtypes_step2_falls_back_to_the_type_name_for_a_peer_without_type_information`,
`crates/dcps/tests/typelookup_resolution_e2e.rs::an_unanswered_lookup_falls_back_to_the_type_names_after_the_grace`

**Status:** done

### §7.6.3.4.2 Fehlschlag: keine Kommunikation, INCONSISTENT_TOPIC an beiden Topics

**Spec:** §7.6.3.4.2, S. 246 — "If either Step 1 or Step 2 fails [...] the
DataWriter and DataReader shall not communicate with each other, and the Service
shall trigger an INCONSISTENT_TOPIC status change for both the DataReader's
Topic and the DataWriter's Topic."

**Repo:** `crates/dcps/src/runtime.rs::record_inconsistent_topic` (Z. 18314);
`crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (Z. 12220)

**Tests:** `crates/dcps/tests/listener_trigger_c22c.rs::inconsistent_topic_listener_fires_on_qos_mismatch_of_a_name`,
`crates/dcps/tests/topics_of_one_name.rs::an_inconsistent_qos_is_refused_and_counted`

**Status:** done — Jede Seite meldet `INCONSISTENT_TOPIC` für ihr eigenes Topic. Ein weiterer Typ unter einem benutzten Namen ist ein weiteres Topic; eine andere QoS unter demselben Namen wird abgewiesen und gezählt.

### §7.6.3.4.2 Erfolg: weiter mit der QoS-Prüfung; beide Seiten kommen zum selben Ergebnis

**Spec:** §7.6.3.4.2, S. 246 — "If both Step 1 and Step 2 succeed, then the
Topics are considered to be consistent, and the matching shall proceed to check
other aspects of endpoint matching [...] the algorithm is such that both sides
are guaranteed to arrive at the same conclusion."

**Repo:** `crates/dcps/src/runtime.rs::wire_writer_to_remote_reader` (Z. 12220);
`crates/dcps/src/runtime.rs::wire_reader_to_remote_writer` (Z. 12732)

**Tests:** `crates/dcps/src/runtime.rs::tests::xtypes_step2_falls_back_to_the_type_name_for_a_peer_without_type_information`,
`crates/dcps/tests/xtypes_aware_match.rs::a_readers_type_consistency_policy_survives_its_announcement`,
`crates/rtps/src/subscription_data.rs::tests::a_reader_announcing_no_type_consistency_is_read_as_disallow`

**Status:** done — Beide Seiten entscheiden mit der Policy des Readers; der Reader kündigt sie an (PID `0x0074`), und ein fehlendes PID gilt als DISALLOW wie in §7.6.3.4.1.

### §7.6.3.4.3 Built-in-Subscription: @id(0x0074) type_compatibility

**Spec:** §7.6.3.4.3, S. 247 — "The subscription built-in topic data type shall
indicate the type consistency requirements of the corresponding reader:
@id(0x0074) DDS::TypeConsistencyEnforcementQosPolicy type_compatibility;"

**Repo:** `crates/rtps/src/parameter_list.rs` (`TYPE_CONSISTENCY_ENFORCEMENT` Z. 198);
`crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData` (Z. 37, Feld `type_consistency` Z. 111)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::a_readers_type_consistency_policy_survives_its_announcement`

**Status:** done — immer geschrieben, mit `kind` als `unsigned short`.

---

## §7.6.4 Local API Extensions

### §7.6.4.1 create_topic: mehrere Topic-Objekte desselben Namens

**Spec:** §7.6.4.1, S. 247 — "In implementations conforming to this
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

**Status:** done — `create_topic` legt unter einem benutzten Namen einen weiteren Topic für einen weiteren Typ an, wenn die QoS gleich ist; bei abweichender QoS wird abgewiesen und `INCONSISTENT_TOPIC` gezählt. `delete_topic` wartet auf die Endpunkte des eigenen Topics und nicht auf die eines anderen Topics desselben Namens (die Nutzer werden nach Name und Typname gezählt; ein MultiTopic nennt seine Topics nur mit Namen und zählt für alle). `zerodds_dp_create_topic` der C-ABI, die C++-PSM (über die C-ABI) und die Java-PSM (`DomainParticipantImpl`) tun dasselbe.

### §7.6.4.2 lookup_topicdescription mit index

**Spec:** §7.6.4.2, S. 247 — "The lookup_topicdescription operation shall accept
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

**Status:** done — `lookup_topicdescription_at(name, index)` läuft über die Topics eines Namens in der Reihenfolge ihrer Anlage; ein Index außerhalb ist nil. Dasselbe gilt für `zerodds_dp_lookup_topicdescription_at`, für `dp->lookup_topicdescription(name, index)` der C++-PSM und für `DomainParticipantImpl.lookupTopicDescription(name, index)` der Java-PSM. Die Schnittstellen der normativen Namensräume (C++ `dds::domain`, Java `org.omg.dds`) bleiben unverändert; die Erweiterung steht am Delegate beziehungsweise an der Implementierung.

---

## §7.6.5 Built-in Types

### §7.6.5 Vier eingebaute Typen, beim enable registriert

**Spec:** §7.6.5, S. 247–248 — "DDS shall provide a few types preregistered 'out
of the box' [...] The built-in types shall be defined as in the following
sections and shall be automatically registered by the Service under their fully
qualified physical names (as above) with each DomainParticipant at the time it
is enabled."

**Repo:** `crates/dcps/src/participant.rs::register_builtin_types` (Z. 820);
`crates/dcps/src/builtin_type_set.rs::build` (Z. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::participant_after_create_has_four_builtin_types_registered`,
`crates/dcps/tests/builtin_types_auto_register_c44b.rs::builtin_types_are_not_nested`

**Status:** done — Die vier Typen werden beim Anlegen des Participant in einem lokalen Typregister registriert; sie sind `@appendable` und nicht nested (Annex E).

### §7.6.5 Eingebaute Typen sind nicht nested deklariert

**Spec:** §7.6.5, S. 248 — "Like all non-nested types used with DDS, the
built-in types shall have corresponding type-specific DataWriter and DataReader
classes."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`,
`crates/types/src/dynamic/builder.rs::tests::the_builtin_types_are_those_of_annex_e`

**Status:** done — `builtin_type_set.rs::build` deklariert die vier Typen `@extensibility(APPENDABLE)` ohne `@nested`, mit den Membernamen von Annex E; `builtin_topic_types` im `DynamicTypeBuilder` definiert dieselben Typen mit denselben Namen.

### §7.6.5 Typspezifische DataWriter und DataReader mit zusätzlichen Überladungen

**Spec:** §7.6.5, S. 248 — "These shall instantiate the type-specific operations
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

**Status:** done — `dcps::builtin_types` hat die Aliase `StringDataWriter` usw. und die Operationen von Annex E; die C-ABI hat sie ebenfalls (`zerodds_string_writer_write`, `zerodds_keyed_string_writer_*`, `zerodds_bytes_writer_write_w_bytes`, `zerodds_keyed_bytes_writer_*` und die Reader `zerodds_*_reader_take_next`). In C++ sind die vier Writer und Reader Klassen über der C-ABI (`crates/cpp/include/zerodds/builtin_data_endpoints.hpp`), in Java die Schnittstellen `DataWriter<T>` und `DataReader<T>` des Java-PSM über `String`, `byte[]`, `KeyedString` und `KeyedBytes`. Log: `/root/zerodds-rc7/agent-logs/xt-bind-1018-051712.log` (Linux-Testhost, `builtin_data_endpoints_via_cargo` grün) und `.agent/REPORT.md`.

### §7.6.5.1 DDS::String

**Spec:** §7.6.5.1, S. 248 — "The DDS::String type is a simple structure wrapper
around a single unbounded string."

**Repo:** `crates/dcps/src/builtin_type_set.rs::build` (Z. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::dds_string_has_unbounded_string_value_member`,
`crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`,
`crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`

**Status:** done — Die Struktur ist `@appendable` und nicht nested, mit den Membern von Annex E; in `dcps::builtin_types` ist sie ein `DdsType` (XCDR1 und XCDR2, beide Byteordnungen) mit den typspezifischen Operationen (siehe §7.6.5).

### §7.6.5.2 DDS::KeyedString

**Spec:** §7.6.5.2, S. 248 — "The DDS::KeyedString type is similar to
DDS::String, but it is a keyed type; the key is an additional unbounded string."

**Repo:** `crates/dcps/src/builtin_type_set.rs::build` (Z. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::find_builtin_type_returns_correct_kind`,
`crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`,
`crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`

**Status:** done — Die Struktur ist `@appendable` und nicht nested, mit den Membern von Annex E; in `dcps::builtin_types` ist sie ein `DdsType` (XCDR1 und XCDR2, beide Byteordnungen) mit den typspezifischen Operationen (siehe §7.6.5).

### §7.6.5.3 DDS::Bytes

**Spec:** §7.6.5.3, S. 248 — "The DDS::Bytes type is a simple structure wrapper
around a single unbounded sequence of bytes."

**Repo:** `crates/dcps/src/builtin_type_set.rs::build` (Z. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::dds_bytes_has_octet_sequence_value_member`,
`crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`,
`crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`

**Status:** done — Die Struktur ist `@appendable` und nicht nested, mit den Membern von Annex E; in `dcps::builtin_types` ist sie ein `DdsType` (XCDR1 und XCDR2, beide Byteordnungen) mit den typspezifischen Operationen (siehe §7.6.5).

### §7.6.5.4 DDS::KeyedBytes

**Spec:** §7.6.5.4, S. 248 — "The DDS::KeyedBytes type is similar to DDS::Bytes,
but it is a keyed type; the key is an unbounded string."

**Repo:** `crates/dcps/src/builtin_type_set.rs::build` (Z. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::dds_keyed_bytes_has_key_member`,
`crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`,
`crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`

**Status:** done — Die Struktur ist `@appendable` und nicht nested, mit den Membern von Annex E; in `dcps::builtin_types` ist sie ein `DdsType` (XCDR1 und XCDR2, beide Byteordnungen) mit den typspezifischen Operationen (siehe §7.6.5).

---

## §7.6.6 Use of Dynamic Data and Dynamic Type

### §7.6.6 Dynamische Typen beim DomainParticipant registrieren und verwenden

**Spec:** §7.6.6, S. 249 — "The DynamicType interface can be used to construct
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

**Status:** done — Ein Typ entsteht mit `DynamicTypeBuilderFactory`, `SerializedTypeSupport::create_type_support` macht daraus eine Type Support mit einer Kopie des Typs, `register_type(participant, type_name)` registriert sie beim Participant (die TypeObjects gehen in die TypeLookup-Registry), `create_registered_topic` und `create_dynamic_topic` legen Topics des Typs an, und `DynamicDataWriter` und `DynamicDataReader` schreiben und lesen `DynamicData`.

### §7.6.6.1 Umwandlung zwischen statischer und dynamischer Bindung über TypeSupport

**Spec:** §7.6.6.1, S. 249 — "it is desirable to allow conversions among static
and dynamic bindings for the same types and samples. These conversions shall be
provided by operations on the generic TypeSupport interface and its extended
interfaces."

**Repo:** `crates/dcps/src/serialized_type.rs::get_type`,
`crates/dcps/src/serialized_type.rs::create_sample`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::a_sample_converts_to_dynamic_data_and_back`

**Status:** done — `SerializedTypeSupport` wandelt zwischen statischer und dynamischer Bindung: `get_type`, `create_sample`, `create_dynamic_sample`.

### §7.6.6.1.1 Table 63 — TypeSupport::get_type

**Spec:** §7.6.6.1.1, Table 63, S. 249 — "get_type | DynamicType" — "Get a
DynamicType object corresponding to the TypeSupport's data type."

**Repo:** `crates/dcps/src/serialized_type.rs::get_type`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::create_type_support_refuses_a_nested_type_and_a_non_aggregate`

**Status:** done — `SerializedTypeSupport::get_type()` gibt den `DynamicType` des Datentyps der Type Support.

### §7.6.6.1.2 Table 64 — FooTypeSupport::create_sample

**Spec:** §7.6.6.1.2, Table 64, S. 249–250 — "Create a sample of the
TypeSupport's data type with the contents of an input DynamicData object. [...]
This method shall fail with a nil return result if this object is nil or if the
DynamicType of this object is not compatible".

**Repo:** `crates/dcps/src/serialized_type.rs::create_sample`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::a_sample_converts_to_dynamic_data_and_back`

**Status:** done — `create_sample(&DynamicSample)`: ein Sample mit dem Inhalt; ein `DynamicData` eines anderen Typs wird abgewiesen.

### §7.6.6.1.2 Table 64 — FooTypeSupport::create_dynamic_sample

**Spec:** §7.6.6.1.2, Table 64, S. 250 — "Create a DynamicData object with the
contents of an input sample of the TypeSupport's data type."

**Repo:** `crates/dcps/src/serialized_type.rs::create_dynamic_sample`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::a_sample_converts_to_dynamic_data_and_back`

**Status:** done — `create_dynamic_sample(&SerializedSample)`: ein `DynamicSample` mit dem Inhalt; ein Sample eines anderen Typs wird abgewiesen.

### §7.6.6.1.3 DynamicTypeSupport erweitert FooTypeSupport mit Foo = DynamicData

**Spec:** §7.6.6.1.3, S. 250 — "The DynamicTypeSupport interface extends the
FooTypeSupport interface defined by the DDS specification where 'Foo' is the
type DynamicData."

**Repo:** `crates/dcps/src/serialized_type.rs::SerializedTypeSupport`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::register_type_registers_a_type_under_a_name`,
`crates/dcps/src/serialized_type.rs::tests::create_type_support_refuses_a_nested_type_and_a_non_aggregate`,
`crates/dcps/src/serialized_type.rs::tests::runtime_support_preserves_identity_before_instance_mutation`,
`crates/dcps/tests/dynamic_data_writer_reader.rs::a_dynamic_topic_takes_writers_and_readers_of_dynamic_data`

**Status:** done — `SerializedTypeSupport` ist die `DynamicTypeSupport`: `get_type_name`, `register_type`, `get_type`, `create_sample`, `create_dynamic_sample`, die statischen `create_type_support` und `delete_type_support`; als `TopicCodec<DynamicSample>` ist sie die FooTypeSupport für Foo = `DynamicData`, und `DynamicDataWriter` und `DynamicDataReader` sind `DataWriter<DynamicSample>` und `DataReader<DynamicSample>`.

### §7.6.6.1.4 Table 65 — register_type, get_type_name

**Spec:** §7.6.6.1.4, Table 65, S. 250–251 — "register_type | ReturnCode_t |
participant DomainParticipant | type_name string<Char8,256> [...] get_type_name
| string<Char8,256>" — "These operations are defined by, and described in, the
DDS specification."

**Repo:** `crates/dcps/src/serialized_type.rs::register_type`,
`crates/dcps/src/serialized_type.rs::with_type_name`,
`crates/dcps/src/participant.rs::create_registered_topic`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::register_type_registers_a_type_under_a_name`,
`crates/dcps/src/serialized_type.rs::tests::create_type_support_refuses_a_nested_type_and_a_non_aggregate`

**Status:** done — `SerializedTypeSupport::register_type(&participant, type_name)` registriert den Typ beim Participant unter seinem Namen oder unter `type_name` (`with_type_name`; `string<Char8,256>`) und legt seine `TypeObject`s in die TypeLookup-Registry; dasselbe noch einmal ist kein Fehler, ein anderer Typ unter einem vergebenen Namen ist `PreconditionNotMet`. `DomainParticipant::create_registered_topic(name, type_name, qos)` legt ein Topic des registrierten Typs an und weist einen nicht registrierten Namen ab. `get_type_name()` gibt den Namen, unter dem der Typ registriert ist.

### §7.6.6.1.5 Table 65 — create_type_support: Kopie des Typs, nil oder nested abgewiesen

**Spec:** §7.6.6.1.5, Table 65, S. 251 — "The implementation shall ensure that
the new type support has a 'copy' of the given type object [...] If this
argument is nil or is a nested type, the operation shall fail and return a nil
value."

**Repo:** `crates/dcps/src/serialized_type.rs::create_type_support`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::create_type_support_refuses_a_nested_type_and_a_non_aggregate`

**Status:** done — `create_type_support(&DynamicType)`: eine Type Support mit einer Kopie des Typs (aus seinen TypeObjects gebaut); ein `@nested` Typ und ein Typ, der keine Struktur und keine Union ist, werden abgewiesen.

### §7.6.6.1.6 Table 65 — delete_type_support

**Spec:** §7.6.6.1.6, Table 65, S. 251 — "Delete the given type support object
[...] If this argument is nil, the operation shall return successfully without
having any observable effect."

**Repo:** `crates/dcps/src/serialized_type.rs::delete_type_support`

**Tests:** `crates/dcps/src/serialized_type.rs::tests::create_type_support_refuses_a_nested_type_and_a_non_aggregate`

**Status:** done — `delete_type_support(Option<Arc<..>>)`: nil hat keine Wirkung, eine Type Support wird mit ihrem letzten Halter freigegeben.

### §7.6.6.2 DynamicDataWriter und DynamicDataReader

**Spec:** §7.6.6.2, S. 251 — "The DynamicDataWriter interface instantiates the
FooDataWriter interface defined by the DDS specification where 'Foo' is the type
DynamicData."

**Repo:** `crates/dcps/src/serialized_type.rs::DynamicDataWriter`,
`crates/dcps/src/serialized_type.rs::DynamicDataReader`,
`crates/dcps/src/participant.rs::create_dynamic_topic`

**Tests:** `crates/dcps/tests/dynamic_data_writer_reader.rs::a_dynamic_topic_takes_writers_and_readers_of_dynamic_data`,
`crates/dcps/tests/dynamic_data_writer_reader.rs::linux_e2e::dynamic_data_goes_over_xcdr`,
`crates/dcps/tests/dynamic_data_writer_reader.rs::linux_e2e::dynamic_data_goes_over_xml`

**Status:** done — `DynamicDataWriter` ist `DataWriter<DynamicSample>`, `DynamicDataReader` `DataReader<DynamicSample>` (`FooDataWriter` und `FooDataReader` mit "Foo" gleich `DynamicData`), über ein Topic von `DomainParticipant::create_dynamic_topic`; sie haben keine zusätzlichen Eigenschaften oder Operationen. Der Schlüssel, die Instanz und die Filter lesen den Wert des `DynamicSample` (`DdsType for DynamicSample`, `TopicCodec<DynamicSample>`). Der Pfad mit `SerializedSample` (`create_serialized_topic`) bleibt, wie er ist.

---

## §7.6.7 DCPS Queries and Filters

### §7.6.7.1 Member Names: Arrays und Sequenzen: Index in eckigen Klammern

**Spec:** §7.6.7.1, S. 252 — "Arrays and sequences: Elements in these ordered
collections shall be indicated by a zero-based subscript enclosed in square
brackets, e.g. my_collection[0]."

**Repo:** —

**Tests:** `crates/sql-filter/src/evaluator.rs::tests::subscripts_reach_the_row_as_written`,
`crates/sql-filter/src/path.rs::tests::subscripts_are_indices_and_keys`,
`crates/dcps/src/serialized_type.rs::tests::a_filter_subscripts_a_sequence_and_a_bitmask`

**Status:** done — Der Filter liest `a[0]`: ein Index in eckigen Klammern an einem Array oder einer Sequenz (`sql-filter::path::split_path`, `PathStep`); auf einem `SerializedTypeSupport`-Sample erreicht er die Elemente.

### §7.6.7.1 Member Names: Maps: Schlüssel als STRING in eckigen Klammern

**Spec:** §7.6.7.1, S. 252 — "Maps: Value elements in these unordered
collections shall be indicated by a string representation of a corresponding key
element [...] e.g. my_map['key']."

**Repo:** `crates/dcps/src/serialized_type.rs::field_value`

**Tests:** `crates/sql-filter/src/evaluator.rs::tests::subscripts_reach_the_row_as_written`,
`crates/sql-filter/src/path.rs::tests::subscripts_are_indices_and_keys`,
`crates/dcps/src/serialized_type.rs::tests::a_filter_reaches_a_map_value_and_tells_an_absent_optional_member_from_no_field`

**Status:** done — Der Filter liest `a['key']`; auf einem `SerializedTypeSupport`-Sample entscheidet der Typ, dass eine Collection eine Map ist, und `m['key']` (bei einem ganzzahligen Schlüssel auch `m[3]`) findet den Wert über den Schlüssel.

### §7.6.7.1 Member Names: Bitmasks: Flag als ENUMERATEDVALUE in eckigen Klammern, Boolean

**Spec:** §7.6.7.1, S. 252 — "Bitmasks: A flag in a bitmask shall be indicated
by its name [...] enclosed in square brackets, e.g. my_bitmask['MY_FLAG']. Such
an expression shall be considered to have a Boolean type [...] Comparisons with
the integer literals 1 and 0 shall also be allowed."

**Repo:** —

**Tests:** `crates/sql-filter/src/evaluator.rs::tests::a_bitmask_flag_compares_with_true_and_with_one_and_zero`,
`crates/dcps/src/serialized_type.rs::tests::a_filter_subscripts_a_sequence_and_a_bitmask`

**Status:** done — Ein Bitmask-Flag wird als `m['READ']` gelesen, ist Boolean und mit `TRUE`, 1 und 0 vergleichbar.

### §7.6.7.2 Optionale Member gegen null

**Spec:** §7.6.7.2, S. 252 — "If the member is optional, and it takes no value
in the given object, it shall be considered equal to null. [...] No non-optional
member shall ever be considered equal to null. Inequalities expressed relative
to null shall never evaluate to true".

**Repo:** `crates/dcps/src/serialized_type.rs::field_value`,
`crates/zerodds-build/src/dds_type.rs::field_value_item`

**Tests:** `crates/sql-filter/src/evaluator.rs::tests::null_follows_the_rules_for_optional_members`,
`crates/dcps/src/serialized_type.rs::tests::a_filter_reaches_a_map_value_and_tells_an_absent_optional_member_from_no_field`,
`crates/zerodds-build/tests/dds_type.rs::an_absent_optional_member_is_null_to_a_filter`,
`crates/zerodds-build/tests/dds_type.rs::the_generated_filter_value_of_an_optional_member_is_null_or_the_value`

**Status:** done — Das Literal `null` gilt für optionale Member nach §7.6.7.2: ein Member ohne Wert ist gleich `null`, einer mit Wert nicht, keine Ungleichung gegen `null` ist wahr. Das Sample kennt seinen Typ: ein abwesender optionaler Member ist `null`, ein Name, den der Typ nicht hat, ist kein Feld. Der erzeugte `field_value` eines optionalen skalaren oder String-Members gibt den Wert oder `null`.

### §7.6.7.3 Grammatik: BOOLEANVALUE true oder false ohne Groß-/Kleinschreibung

**Spec:** §7.6.7.3, S. 253 — "The BOOLEANVALUE token shall be either true or
false (case-insensitive)."

**Repo:** `crates/sql-filter/src/lexer.rs::tokenize`

**Tests:** `crates/sql-filter/src/lexer.rs::tests::a_boolean_literal_is_true_or_false_in_any_case`,
`crates/sql-filter/src/evaluator.rs::tests::a_boolean_literal_compares_with_a_boolean_member_in_any_case`

**Status:** done — `true` und `false` werden in jeder Schreibweise als BOOLEANVALUE gelesen (`true`, `TRUE`, `True`, `tRuE`) und im Vergleich mit einem Boolean-Member ausgewertet; `null` ist NULLVALUE.

### §7.6.7.3 Grammatik: NULLVALUE null wird gelesen

**Spec:** §7.6.7.3, S. 253 — "The NULLVALUE token shall always be null."

**Repo:** —

**Tests:** `crates/sql-filter/src/lexer.rs::tests::null_is_a_token_of_its_own`

**Status:** done — Der Lexer kennt `null` als eigenes Token, der Evaluator `Value::Null`.

### §7.6.7.3 Parameter-Produktion ohne INTEGERVALUE

**Spec:** §7.6.7.3, S. 252–253 — "Parameter ::= | CHARVALUE | FLOATVALUE |
STRING | ENUMERATEDVALUE | BOOLEANVALUE | NULLVALUE | PARAMETER".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Produktion beginnt mit einer leeren
Alternative, INTEGERVALUE aus DDS Annex A fehlt (Fehler der Spec, in der
Befunddatei aufgeführt). Ganzzahlen werden gelesen.

---

## §7.6.8 Interoperability of Keyed Topics

### §7.6.8 KeyHash nach RTPS §9.6.3.8, ergänzt

**Spec:** §7.6.8, S. 253 — "As described in [RTPS] Clause 9.6.3.8 [...] the key
hash for a given object of a keyed type is obtained by first serializing the
values of the key members in their declaration order. The algorithm described in
that clause shall be amended as described below."

**Repo:** `crates/wire-codec/src/encode.rs::key_holder` (Z. 617);
`crates/ast-layout/src/keyhash.rs::key_holder_plan` (Z. 41)

**Tests:** `crates/wire-conformance/tests/bytes.rs::the_key_hash_covers_the_key_member_and_not_the_others`

**Status:** done

### §7.6.8 Step 1: FooKeyHolder FINAL, nur Key-Member, nach Member-ID sortiert

**Spec:** §7.6.8, S. 253 — "Change FooKeyHolder extensibility kind to FINAL
[...] If there are any key members, then remove the non-key members from
FooKeyHolder. Otherwise do not remove any members. Reorder the members in
ascending order of their memberId values."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_members` (Z. 254);
`crates/ast-layout/src/keyhash.rs::holder_keeps_only_keys` (Z. 133)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::key_members_come_back_in_member_id_order_not_declaration_order`,
`crates/ast-layout/tests/wire_vectors.rs::a_type_with_no_key_member_has_no_key_members_and_a_whole_type_holder`,
`crates/wire-probe/tests/vendor_bytes.rs::a_key_holders_member_order_is_the_one_each_vendor_hashes`

**Status:** done

### §7.6.8 Step 2 und 3: Werte übernehmen, rekursiv für geschachtelte Aggregate

**Spec:** §7.6.8, S. 253 — "Step 2. Define a new object 'FooKeyHolderObject'
from the FooObject [...] Step 3. Apply steps 1 and 2 recursively to the members
of FooKeyHolder if they are themselves AggregatedTypes."

**Repo:** `crates/ast-layout/src/keyhash.rs::member_key` (Z. 161);
`crates/wire-codec/src/encode.rs::key_holder` (Z. 617)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::nested_keyed_members_contribute_their_own_keys`,
`crates/wire-probe/tests/silent_drop.rs::a_key_member_behind_a_typedef_keeps_its_nested_keys`

**Status:** done

### §7.6.8 Step 4: PLAIN_CDR2 Big Endian, Ausrichtung 4, Füllbytes null

**Spec:** §7.6.8, S. 253 — "Compute the PLAIN_CDR2 Big Endian Serialization (see
7.4.2) of FooKeyHolderObject. The serialization shall be performed on a buffer
that is initially aligned to the maximum alignment in PLAIN_CDR2 (i.e., 4).
Furthermore, any padding bytes added due to alignment rules shall be set to
zero."

**Repo:** `crates/wire-codec/src/encode.rs::key_holder` (Z. 617)

**Tests:** `crates/wire-conformance/tests/bytes.rs::the_key_hash_is_big_endian_whatever_the_sample_is`,
`crates/ast-layout/tests/wire_vectors.rs::v8_keyed_sensor_plans_a_four_byte_zero_padded_key_holder`

**Status:** done — der Default `KeyHolderAlignment::Aligned` deckelt bei 4;
andere Werte sind Dialekte.

### §7.6.8 Step 5.1: höchstens 16 Oktette, mit Nullen auf 16 aufgefüllt

**Spec:** §7.6.8, S. 253 — "If the FooKeyHolder has a maximum serialized size
that is less than or equal to 16 bytes, then then the KeyHash of FooObject shall
be set to the result of Step 4, extended to 16 bytes."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_hash_form` (Z. 85)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::the_key_hash_of_a_small_key_is_the_zero_padded_holder`,
`crates/dcps/src/dds_type.rs::tests::small_keyed_produces_zero_padded_keyhash`

**Status:** done

### §7.6.8 Step 5.2: mehr als 16 Oktette, MD5

**Spec:** §7.6.8, S. 253 — "If the FooKeyHolder has a maximum serialized size
that is greater than 16 bytes, then the KeyHash of FooObject shall be set to the
MD5 Hash of the serialized bytes obtained from Step 4."

**Repo:** `crates/ast-layout/src/keyhash.rs::key_hash_form` (Z. 85)

**Tests:** `crates/wire-probe/tests/vendor_bytes.rs::a_key_holder_past_sixteen_octets_is_md5_hashed`,
`crates/ast-layout/tests/wire_vectors.rs::a_key_holder_over_sixteen_bytes_is_hashed`,
`crates/ast-layout/tests/key_maximum.rs::bounded_string_threshold_is_maximum_not_fixed_extent`

**Status:** done — maßgeblich ist die größte mögliche Größe des Holders, nicht
die des Samples.

### §7.6.8 Ohne Encapsulation-, Typ- und Member-Header

**Spec:** §7.6.8, S. 254 — "the serialized bytes obtained in step 4 do not
include any encapsulation header, type header, or member headers and use a
maximum alignment of 4."

**Repo:** `crates/wire-codec/src/encode.rs::key_holder` (Z. 617)

**Tests:** `crates/ast-layout/tests/wire_vectors.rs::v8_keyed_sensor_plans_a_four_byte_zero_padded_key_holder`

**Status:** done

### §7.6.8 Beispiel 1: @key long id

**Spec:** §7.6.8, S. 254 — KeyHash `{ 0x12, 0x34, 0x56, 0x78, 0x00, ... }`.

**Repo:** `crates/wire-codec/src/encode.rs::key_hash`

**Tests:** `crates/wire-codec/tests/keyhash_xtypes_7_6_8_examples.rs::example_1_a_four_octet_key_is_padded_with_zeros`

**Status:** done — Der Test rechnet Beispiel 1 mit den Werten der Spec (`id = 0x12345678`, `x = 10`, `y = 20`): Schlüsselstrom `12 34 56 78`, KeyHash `12 34 56 78` und zwölf Nullen.

### §7.6.8 Beispiel 2: string<12> label und long long id, MD5

**Spec:** §7.6.8, S. 254–255 — KeyHash `{ 0xf9, 0x1a, 0x59, 0xe3, ... }`.

**Repo:** `crates/wire-codec/src/encode.rs::key_hash`

**Tests:** `crates/wire-codec/tests/keyhash_xtypes_7_6_8_examples.rs::example_2_a_key_of_at_most_28_octets_is_hashed_with_md5`

**Status:** done — Der Test rechnet Beispiel 2 (`label = "BLUE"`, `id = 0x123456789abcdef0`): der Strom hat die 20 Oktette der Spec, der KeyHash ist `f9 1a 59 e3 2e 45 35 d9 a6 9c d5 d9 f5 b6 e3 6e` (MD5, die Maximalgröße 28 liegt über 16).

### §7.6.8 Beispiel 3: @mutable mit geschachteltem Key, MD5

**Spec:** §7.6.8, S. 255–257 — KeyHash `{ 0x37, 0x4b, 0x96, 0xe2, ... }`.

**Repo:** `crates/wire-codec/src/encode.rs::key_hash`

**Tests:** `crates/wire-codec/tests/keyhash_xtypes_7_6_8_examples.rs::example_3_the_key_members_are_reordered_by_member_id_and_nested_keys_reduced`

**Status:** done — Der Test rechnet Beispiel 3 (`@mutable`, `label` mit `@id(40)`, `m_nested` mit `@id(30)`): die Key-Member stehen nach Member-Id geordnet, der geschachtelte Key ist auf `m_long` reduziert, der Strom hat die 13 Oktette der Spec, der KeyHash ist `37 4b 96 e2 e7 27 23 7f 01 6c c4 ce bb 6e b7 1e`. `Foo` deklariert `string<16>`, der KeyHolder der Spec `string<12>` (Widerspruch im Beispiel, in der Befunddatei); der Hash hängt von der Schranke nicht ab, solange der Strom über 16 Oktetten liegt.

### §7.6.8 Dispose mit serializedKey: Instanz aus dem Key, nicht aus PID_KEY_HASH

**Spec:** §7.6.8, S. 253 — "the key hash for a given object of a keyed type is
obtained by first serializing the values of the key members".

**Repo:** `crates/dcps/src/subscriber.rs::lifecycle_instance` (Z. 4638)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::a_dispose_with_a_foreign_key_hash_disposes_the_instance_of_its_key`

**Status:** done — der Reader dekodiert den `serializedKey` eines Dispose oder
Unregister und bildet den KeyHash selbst; nur ein Marker ohne `serializedKey`
behält den übertragenen Hash.

---

## Annex A: XML Type Representation Schema

### Annex A Schema: Wurzelelement types, Namensraum

**Spec:** Annex A, S. 259 — XSD der XML Type Representation mit dem Element
`types` vom Typ `typeLibrary`.

**Repo:** `crates/xml/schemas/dds-xtypes_type_definition.xsd`;
`crates/xml/schemas/dds-xtypes_type_definition_nonamespace.xsd`;
`crates/xml/src/schemas.rs`

**Tests:** `crates/xml/tests/annex_a_types.rs::annex_a_schema_compiles_and_rejects_what_it_should`,
`crates/xml/tests/annex_a_types.rs::documents_annex_a_accepts_are_read`,
`crates/xml/tests/annex_a_types.rs::documents_annex_a_rejects_are_rejected`,
`crates/xml/tests/annex_a_types.rs::a_document_in_the_xtypes_namespace_is_read_by_the_annex_a_schema`,
`crates/xml/src/schemas.rs::tests::annex_a_schema_has_root_element_types_in_the_xtypes_namespace`

**Status:** done — Die beiden XSD-Dateien von Annex A (Namensraum `http://www.omg.org/dds`, Wurzelelement `types` vom Typ `typeLibrary`, und die Datei ohne Namensraum) liegen im Baum (`XTYPES_TYPES_XSD`, `XTYPES_TYPES_NONAMESPACE_XSD`; vier Tippfehler des PDF in `xs:attribute` sind geschlossen). Dokumente, die das Schema annimmt, liest `zerodds-xml`; Dokumente, die es abweist, weist `zerodds-xml` ab (`xmllint` auf dem Linux-Testhost). Ein Element im Namensraum von Annex A wird nach dessen Schema gelesen (`appendable`, nicht `append`), sonst nach dem Schema von DDS-XML 1.0.

### Annex A Schema: identifierName, fileName

**Spec:** Annex A, S. 259–260 — `xs:simpleType name="identifierName"`,
`xs:simpleType name="fileName"`.

**Repo:** `crates/xml/src/xtypes_parser.rs::is_identifier_name`;
`crates/xml-compose/src/lower.rs::check_identifier_names`

**Tests:** `crates/xml/tests/annex_a_types.rs::identifier_name_is_the_pattern_of_annex_a`,
`crates/xml/tests/annex_a_types.rs::a_name_that_is_no_identifier_name_is_refused`,
`crates/xml/tests/annex_a_types.rs::a_file_name_is_any_string`,
`crates/xml-compose/tests/annex_a_declarations.rs::a_name_that_is_no_identifier_name_is_an_error`

**Status:** done — `name`, `baseType`, `nonBasicTypeName` und `mapKeyNonBasicTypeName` werden in beiden Lesern gegen `([a-zA-Z]|::)([a-zA-Z_0-9]|::)*` geprüft; ein Name außerhalb nennt Attribut und Element. `fileName` ist ein beliebiger String.

### Annex A Schema: includeDecl

**Spec:** Annex A, S. 260 — `xs:complexType name="includeDecl"` mit Attribut
`file`.

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_include`,
`crates/xml-compose/src/lower.rs::load_units`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_include_element`,
`crates/xml/src/xtypes_parser.rs::tests::parse_include_missing_file_attribute_rejected`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_include_brings_the_named_document_into_the_model_once`

**Status:** done — `zerodds-xml` liest den Eintrag, `zerodds-xml-compose` folgt `file` (§7.3.2.1.1).

### Annex A Schema: forwardDecl, forwardDeclTypeKind enum/struct/union

**Spec:** Annex A, S. 260 — `xs:simpleType name="forwardDeclTypeKind"` mit
`enum`, `struct`, `union`; `xs:complexType name="forwardDecl"`.

**Repo:** `crates/xml-compose/src/lower.rs::forward_declaration` (Z. 630);
`crates/xml/src/xtypes_parser.rs::parse_forward_dcl` (Z. 167)

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_forward_dcl_element`,
`crates/xml/src/xtypes_parser.rs::tests::parse_forward_dcl_default_kind_struct`

**Status:** done

### Annex A Schema: allTypeKind

**Spec:** Annex A, S. 261 — `xs:simpleType name="allTypeKind"`: `boolean`,
`byte`, `char8`, `char16`, `int8`, `uint8`, `int16`, `uint16`, `int32`,
`uint32`, `int64`, `uint64`, `float32`, `float64`, `float128`, `string`,
`wstring`, `nonBasic`.

**Repo:** `crates/xml-compose/src/lower.rs::primitive_of` (Z. 1449)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::every_primitive_reaches_the_same_type_from_both_front_ends`,
`crates/xml-compose/src/lower.rs::tests::the_two_primitive_vocabularies_agree`

**Status:** done — zusätzlich die IDL-Namen (`long`, `octet` u. a.).

### Annex A Schema: arrayDimensionsKind

**Spec:** Annex A, S. 261 — `xs:simpleType name="arrayDimensionsKind"`.

**Repo:** `crates/xml-compose/src/lower.rs::dimensions`

**Tests:** `crates/xml-compose/tests/idl_parity.rs::arrays_reach_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/constants_and_annotations.rs::bounds_are_const_expressions_over_constants`

**Status:** done — Die Dimensionen sind eine Liste von `<const_exp>`, durch Kommas getrennt.

### Annex A Schema: constDecl

**Spec:** Annex A, S. 262 — `xs:complexType name="constDecl"` mit `name`,
`type`, `nonBasicTypeName`, `value`.

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_const`,
`crates/xml-compose/src/lower.rs::constant`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::parse_const_element`,
`crates/xml/src/xtypes_parser.rs::tests::parse_const_missing_value_attribute_rejected`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_constant_is_found_whichever_side_of_its_use_it_is_declared`

**Status:** done — `zerodds-xml` liest Konstanten, `zerodds-xml-compose` legt sie mit Typ und Ausdruck ins Modell (§7.3.2.1.3).

### Annex A Schema: memberId, simpleMemberDecl, memberDecl mit Bound- und Map-Attributen

**Spec:** Annex A, S. 262–263 — `simpleMemberDecl` (`name`, `type`,
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

**Status:** done — `memberId` liegt zwischen 0 und 0x0FFFFFFF (außerhalb ein Fehler); `simpleMemberDecl` (`name`, `type`, `nonBasicTypeName`) und `memberDecl` (`annotate`, `external`, `tryConstruct`, `mapKeyType`, `mapKeyNonBasicTypeName`, `stringMaxLength`, `mapKeyStringMaxLength`, `sequenceMaxLength`, `mapMaxLength`, `arrayDimensions`) werden von beiden Lesern gelesen.

### Annex A Schema: tryConstructKind discard, use_default, trim

**Spec:** Annex A, S. 263 — `xs:simpleType name="tryConstructKind"` mit
`discard`, `use_default`, `trim`.

**Repo:** `crates/xml-compose/src/lower.rs::try_construct` (Z. 972)

**Tests:** `crates/xml-compose/tests/idl_parity.rs::try_construct_reaches_the_same_type_from_both_front_ends`

**Status:** done

### Annex A Schema: verbatimDecl

**Spec:** Annex A, S. 264 — `xs:complexType name="verbatimDecl"` mit `text`,
`language`, `placement`.

**Repo:** `crates/xml-compose/src/lower.rs::verbatim_directives`;
`crates/xml/src/xtypes_parser.rs::parse_verbatim`

**Tests:** `crates/xml/tests/normative_forms.rs::annotations_verbatims_and_flags_are_read`,
`crates/xml-compose/tests/constants_and_annotations.rs::verbatim_text_reaches_the_type`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_verbatim_placement_that_is_none_of_the_six_is_reported`

**Status:** done — `verbatimDecl` (`text`, `language`, `placement`) wird von beiden Lesern gelesen; `language` ist ohne Angabe `*`, `placement` `before-declaration`.

### Annex A Schema: extensibilityKind final, appendable, mutable

**Spec:** Annex A, S. 264 — `xs:simpleType name="extensibilityKind"` mit
`final`, `appendable`, `mutable`.

**Repo:** `crates/xml-compose/src/lower.rs::extensibility_of` (Z. 1480)

**Tests:** `crates/xml/tests/types_and_samples.rs::t03_extensibility_final_appendable_mutable`,
`crates/xml-compose/tests/loader.rs::the_default_extensibility_only_reaches_types_the_document_left_open`

**Status:** done — zusätzlich `extensible` für appendable.

### Annex A Schema: autoIdKind hash, sequencial wird gelesen

**Spec:** Annex A, S. 265 — `xs:simpleType name="autoIdKind"` mit `hash` und
`sequencial`.

**Repo:** `crates/xml-compose/src/lower.rs::record`

**Tests:** `crates/xml-compose/tests/annex_a_attributes.rs::autoid_reads_annex_as_spelling`

**Status:** done — `record` nimmt `hash`, `sequential` und die Schreibweise `sequencial` aus Annex A an.

### Annex A Schema: structOrUnionTypeDecl mit nested, extensibility, autoid

**Spec:** Annex A, S. 265 — `structOrUnionTypeDecl` (`annotate`, `verbatim`,
`name`, `nested`, `extensibility`, `autoid`).

**Repo:** `crates/xml-compose/src/lower.rs::record`;
`crates/xml/src/xtypes_parser.rs::parse_struct`

**Tests:** `crates/xml/tests/annex_a_types.rs::a_struct_declaration_keeps_everything_annex_a_lets_it_say`,
`crates/xml/tests/normative_forms.rs::annotations_verbatims_and_flags_are_read`,
`crates/xml-compose/tests/idl_parity.rs::member_ids_reach_the_same_numbers_from_both_front_ends`

**Status:** done — `nested`, `extensibility`, `autoid`, `annotate` und `verbatim` an Struktur und Union werden von beiden Lesern gelesen; `autoid="sequencial"` und `sequential` sind beide gültig.

### Annex A Schema: annotationTypeDecl, annotationMemberValueDecl, annotationDecl

**Spec:** Annex A, S. 266 — `annotationTypeDecl` (`member`, `name`, `baseType`),
`annotationMemberValueDecl`, `annotationDecl`.

**Repo:** `crates/xml-compose/src/lower.rs::annotation_declaration`;
`crates/xml/src/xtypes_parser.rs::parse_annotation_type`

**Tests:** `crates/xml/tests/annex_a_types.rs::an_annotation_type_and_its_application_are_read`,
`crates/xml-compose/tests/annex_a_declarations.rs::an_annotation_with_a_base_type_has_the_members_of_its_base_first`,
`crates/xml-compose/tests/annex_a_declarations.rs::an_annotation_base_that_is_not_declared_is_an_error`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_annotation_is_declared_and_applied`

**Status:** done — `annotationTypeDecl` (`member`, `name`, `baseType`), `annotationMemberValueDecl` und `annotationDecl` werden von beiden Lesern gelesen. Das Modell hat für Annotationstypen keine Basis; `zerodds-xml-compose` setzt die Member der Basis vor die eigenen (IDL 4.2 §7.4.15.3), eine Basis, die nicht vorher deklariert ist, ist ein Fehler.

### Annex A Schema: structMemberDecl mit id, optional, mustUnderstand, nonSerialized, key

**Spec:** Annex A, S. 267 — `structMemberDecl` (`id`, `optional`,
`mustUnderstand`, `nonSerialized`, `key`).

**Repo:** `crates/xml-compose/src/lower.rs::member_spec`;
`crates/xml/src/xtypes_parser.rs::parse_member`

**Tests:** `crates/xml/tests/annex_a_types.rs::a_struct_declaration_keeps_everything_annex_a_lets_it_say`,
`crates/xml/tests/types_and_samples.rs::t05_struct_member_key_true`,
`crates/xml/tests/types_and_samples.rs::t06_struct_member_optional_true`,
`crates/xml/tests/types_and_samples.rs::t07_struct_member_id_override`,
`crates/xml-compose/tests/idl_parity.rs::keys_reach_the_same_type_from_both_front_ends`

**Status:** done — `id`, `optional`, `mustUnderstand`, `nonSerialized` und `key` werden von beiden Lesern gelesen (`zerodds-xml` mit `non_serialized`).

### Annex A Schema: structDecl mit member, const, baseType

**Spec:** Annex A, S. 268 — `structDecl` (`member`, `const`, `baseType`).

**Repo:** `crates/xml-compose/src/lower.rs::structure`;
`crates/xml/src/xtypes_parser.rs::parse_struct`

**Tests:** `crates/xml/tests/annex_a_types.rs::a_struct_declaration_keeps_everything_annex_a_lets_it_say`,
`crates/xml/tests/types_and_samples.rs::t04_struct_inheritance_basetype`,
`crates/xml-compose/tests/constants_and_annotations.rs::a_constant_inside_a_structure_is_in_its_scope`

**Status:** done — `structDecl` (`member`, `const`, `baseType`) wird von beiden Lesern gelesen; `zerodds-xml` legt die Konstanten der Struktur in `StructType::constants`.

### Annex A Schema: unionDecl, discriminatorDecl, caseDiscriminatorDecl, caseDecl

**Spec:** Annex A, S. 268–269 — `unionMemberDecl`, `discriminatorDecl`
(`annotate`, `type`, `nonBasicTypeName`, `key`), `caseDiscriminatorDecl`
(`value`), `caseDecl`, `unionDecl` (`discriminator`, `case`).

**Repo:** `crates/xml-compose/src/lower.rs::union` (Z. 308);
`crates/xml/src/xtypes_parser.rs::parse_union` (Z. 315)

**Tests:** `crates/xml/tests/types_and_samples.rs::t12_union_with_cases`,
`crates/xml/tests/types_and_samples.rs::t13_union_with_default_case`,
`crates/xml-compose/tests/idl_parity.rs::unions_reach_the_same_type_from_both_front_ends`

**Status:** done — ein `key` am Diskriminator wird im Default abgewiesen
(§7.2.2.4.4.4.8).

### Annex A Schema: typedefDecl

**Spec:** Annex A, S. 270 — `typedefDecl` (`name`, `type`, `nonBasicTypeName`,
Bound-Attribute, `arrayDimensions`, `external`).

**Repo:** `crates/xml-compose/src/lower.rs::alias` (Z. 588);
`crates/xml/src/xtypes_parser.rs::parse_typedef` (Z. 356)

**Tests:** `crates/xml/tests/types_and_samples.rs::t15_typedef_simple_and_array`

**Status:** done

### Annex A Schema: enumBitBound, enumeratorDecl mit value und defaultLiteral, enumDecl

**Spec:** Annex A, S. 271–272 — `enumBitBound`, `enumeratorDecl` (`annotate`,
`name`, `value`, `defaultLiteral`), `enumDecl` (`annotate`, `verbatim`,
`enumerator`, `name`, `bitBound`).

**Repo:** `crates/xml-compose/src/lower.rs::enumeration`;
`crates/xml/src/xtypes_parser.rs::parse_enum`

**Tests:** `crates/xml/tests/annex_a_types.rs::bit_bounds_and_flag_positions_are_within_their_ranges`,
`crates/xml/tests/types_and_samples.rs::t10_enum_with_explicit_values`,
`crates/xml-compose/tests/idl_parity.rs::a_narrow_bit_bound_reaches_the_same_type_from_both_front_ends`,
`crates/xml-compose/tests/constants_and_annotations.rs::an_enumerator_value_is_a_const_expression`

**Status:** done — `enumBitBound` (1 bis 32), `bitmaskBitBound` (1 bis 64) und `flagIndex` (0 bis 63) werden geprüft, `position` einer Flagge ist Pflicht; `enumeratorDecl` (`annotate`, `name`, `value`, `defaultLiteral`) und `enumDecl` (`annotate`, `verbatim`, `enumerator`, `name`, `bitBound`) werden von beiden Lesern gelesen.

### Annex A Schema: bitmaskBitBound, flagIndex, flagDecl mit position, bitmaskDecl

**Spec:** Annex A, S. 273–274 — `bitmaskBitBound`, `flagIndex`, `flagDecl`
(`annotate`, `name`, `position`), `bitmaskDecl` (`annotate`, `flag`, `name`,
`bitBound`).

**Repo:** `crates/xml-compose/src/lower.rs::bitmask` (Z. 534);
`crates/xml/src/xtypes_parser.rs::parse_bitmask` (Z. 376)

**Tests:** `crates/xml/tests/types_and_samples.rs::t14_bitmask_and_bitset`,
`crates/xml-compose/tests/loader.rs::both_bitmask_spellings_reach_the_same_bits`

**Status:** done

### Annex A Schema: moduleElements, moduleDecl, typeLibrary

**Spec:** Annex A, S. 274–276 — `moduleElements` (`include`, `forward_dcl`,
`const`, `module`, `struct`, `union`, `annotation`, `typedef`, `enum`,
`bitmask`), `moduleDecl` (`name`, `autoid`), `typeLibrary`.

**Repo:** `crates/xml-compose/src/lower.rs::module`;
`crates/xml/src/xtypes_parser.rs::parse_module`

**Tests:** `crates/xml/tests/annex_a_types.rs::a_module_holds_every_module_element_in_any_order`,
`crates/xml-compose/tests/annex_a_declarations.rs::a_module_autoid_governs_the_types_declared_in_it`,
`crates/xml/tests/types_and_samples.rs::t16_module_three_levels_deep`,
`crates/xml-compose/tests/loader.rs::two_documents_reopen_one_module`

**Status:** done — `moduleElements` (`include`, `forward_dcl`, `const`, `module`, `struct`, `union`, `annotation`, `typedef`, `enum`, `bitmask`), `moduleDecl` mit `autoid` und `typeLibrary` werden von beiden Lesern gelesen; `autoid` an einem Modul gilt für die Typen darin, die keins haben (§7.3.1.2.1.1). `bitset` steht nicht in `moduleElements`; beide Leser lesen es als Herstellerform.

---

## Annex B: Representing Types with TypeObject

### Annex B Serialisierung mit XCDR Version 2

**Spec:** Annex B, S. 277 — "// The types in this file shall be serialized with
XCDR encoding version 2".

**Repo:** `crates/types/src/type_object/mod.rs::TypeObject` (Z. 36);
`crates/types/src/hash.rs`

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::a_type_object_opens_with_a_dheader_over_the_rest`

**Status:** done

### Annex B EquivalenceKind EK_MINIMAL = 0xF1

**Spec:** Annex B, S. 277 — "const octet EK_MINIMAL = 0xF1;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::EK_MINIMAL` (Z. 90)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::equivalence_kind_roundtrip`,
`crates/types/src/type_identifier/mod.rs::tests::equivalence_hash_wire_is_discriminator_plus_14_bytes`

**Status:** done

### Annex B EquivalenceKind EK_COMPLETE = 0xF2

**Spec:** Annex B, S. 277 — "const octet EK_COMPLETE = 0xF2;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::EK_COMPLETE` (Z. 92)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::equivalence_kind_roundtrip`,
`crates/types/src/type_identifier/mod.rs::tests::equivalence_hash_wire_is_discriminator_plus_14_bytes`

**Status:** done

### Annex B EquivalenceKind EK_BOTH = 0xF3

**Spec:** Annex B, S. 277 — "const octet EK_BOTH = 0xF3;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::EK_BOTH` (Z. 98)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::equivalence_kind_roundtrip`,
`crates/types/src/type_identifier/mod.rs::tests::equivalence_hash_wire_is_discriminator_plus_14_bytes`

**Status:** done

### Annex B TypeKind TK_NONE = 0x00

**Spec:** Annex B, S. 277–278 — "const octet TK_NONE = 0x00;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_NONE` (Z. 19)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_BOOLEAN = 0x01

**Spec:** Annex B, S. 277–278 — "const octet TK_BOOLEAN = 0x01;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_BOOLEAN` (Z. 21)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_BYTE = 0x02

**Spec:** Annex B, S. 277–278 — "const octet TK_BYTE = 0x02;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_BYTE` (Z. 23)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_INT16 = 0x03

**Spec:** Annex B, S. 277–278 — "const octet TK_INT16 = 0x03;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_INT16` (Z. 25)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_INT32 = 0x04

**Spec:** Annex B, S. 277–278 — "const octet TK_INT32 = 0x04;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_INT32` (Z. 27)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::primitive_int32_discriminator_is_spec_value`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done

### Annex B TypeKind TK_INT64 = 0x05

**Spec:** Annex B, S. 277–278 — "const octet TK_INT64 = 0x05;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_INT64` (Z. 29)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_UINT16 = 0x06

**Spec:** Annex B, S. 277–278 — "const octet TK_UINT16 = 0x06;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_UINT16` (Z. 31)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_UINT32 = 0x07

**Spec:** Annex B, S. 277–278 — "const octet TK_UINT32 = 0x07;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_UINT32` (Z. 33)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_UINT64 = 0x08

**Spec:** Annex B, S. 277–278 — "const octet TK_UINT64 = 0x08;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_UINT64` (Z. 35)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_FLOAT32 = 0x09

**Spec:** Annex B, S. 277–278 — "const octet TK_FLOAT32 = 0x09;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_FLOAT32` (Z. 37)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_FLOAT64 = 0x0A

**Spec:** Annex B, S. 277–278 — "const octet TK_FLOAT64 = 0x0A;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_FLOAT64` (Z. 39)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_FLOAT128 = 0x0B

**Spec:** Annex B, S. 277–278 — "const octet TK_FLOAT128 = 0x0B;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_FLOAT128` (Z. 41)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_INT8 = 0x0C

**Spec:** Annex B, S. 277–278 — "const octet TK_INT8 = 0x0C;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_INT8` (Z. 43)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_UINT8 = 0x0D

**Spec:** Annex B, S. 277–278 — "const octet TK_UINT8 = 0x0D;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_UINT8` (Z. 45)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_CHAR8 = 0x10

**Spec:** Annex B, S. 277–278 — "const octet TK_CHAR8 = 0x10;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_CHAR8` (Z. 47)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_CHAR16 = 0x11

**Spec:** Annex B, S. 277–278 — "const octet TK_CHAR16 = 0x11;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_CHAR16` (Z. 49)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_STRING8 = 0x20

**Spec:** Annex B, S. 277–278 — "const octet TK_STRING8 = 0x20;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_STRING8` (Z. 54)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_STRING16 = 0x21

**Spec:** Annex B, S. 277–278 — "const octet TK_STRING16 = 0x21;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TK_STRING16` (Z. 56)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::all_primitives_roundtrip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — der Wert ist aus dem Code gelesen; der Rundlauf prüft die
Zuordnung, nicht den Zahlenwert. Die Octet-Tests gegen Cyclone enthalten die
Kennungen der Member ihrer Typen.

### Annex B TypeKind TK_ALIAS = 0x30

**Spec:** Annex B, S. 278 — "const octet TK_ALIAS = 0x30;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_ALIAS` (Z. 15)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Diskriminator von `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_ENUM = 0x40

**Spec:** Annex B, S. 278 — "const octet TK_ENUM = 0x40;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_ENUM` (Z. 18)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Diskriminator von `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_BITMASK = 0x41

**Spec:** Annex B, S. 278 — "const octet TK_BITMASK = 0x41;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_BITMASK` (Z. 20)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Diskriminator von `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_ANNOTATION = 0x50

**Spec:** Annex B, S. 278 — "const octet TK_ANNOTATION = 0x50;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_ANNOTATION` (Z. 23)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Diskriminator von `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_STRUCTURE = 0x51

**Spec:** Annex B, S. 278 — "const octet TK_STRUCTURE = 0x51;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_STRUCTURE` (Z. 26)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Diskriminator von `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_UNION = 0x52

**Spec:** Annex B, S. 278 — "const octet TK_UNION = 0x52;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_UNION` (Z. 28)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Diskriminator von `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_BITSET = 0x53

**Spec:** Annex B, S. 278 — "const octet TK_BITSET = 0x53;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_BITSET` (Z. 30)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Diskriminator von `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_SEQUENCE = 0x60

**Spec:** Annex B, S. 278 — "const octet TK_SEQUENCE = 0x60;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_SEQUENCE` (Z. 33)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Diskriminator von `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_ARRAY = 0x61

**Spec:** Annex B, S. 278 — "const octet TK_ARRAY = 0x61;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_ARRAY` (Z. 35)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Diskriminator von `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeKind TK_MAP = 0x62

**Spec:** Annex B, S. 278 — "const octet TK_MAP = 0x62;"

**Repo:** `crates/types/src/type_object/kinds.rs::TK_MAP` (Z. 37)

**Tests:** `crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Diskriminator von `CompleteTypeObject`/`MinimalTypeObject`.

### Annex B TypeIdentifier TI_STRING8_SMALL = 0x70

**Spec:** Annex B, S. 278 — "const octet TI_STRING8_SMALL = 0x70;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING8_SMALL` (Z. 63)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string8_small_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_STRING8_LARGE = 0x71

**Spec:** Annex B, S. 278 — "const octet TI_STRING8_LARGE = 0x71;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING8_LARGE` (Z. 65)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string8_large_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_STRING16_SMALL = 0x72

**Spec:** Annex B, S. 278 — "const octet TI_STRING16_SMALL = 0x72;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING16_SMALL` (Z. 67)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string16_small_and_large_roundtrip`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_STRING16_LARGE = 0x73

**Spec:** Annex B, S. 278 — "const octet TI_STRING16_LARGE = 0x73;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRING16_LARGE` (Z. 69)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::string16_small_and_large_roundtrip`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_SEQUENCE_SMALL = 0x80

**Spec:** Annex B, S. 278 — "const octet TI_PLAIN_SEQUENCE_SMALL = 0x80;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_SEQUENCE_SMALL` (Z. 72)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_sequence_of_int32_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_SEQUENCE_LARGE = 0x81

**Spec:** Annex B, S. 278 — "const octet TI_PLAIN_SEQUENCE_LARGE = 0x81;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_SEQUENCE_LARGE` (Z. 74)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_sequence_large_of_string_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_ARRAY_SMALL = 0x90

**Spec:** Annex B, S. 278 — "const octet TI_PLAIN_ARRAY_SMALL = 0x90;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_ARRAY_SMALL` (Z. 77)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_array_small_3d_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_ARRAY_LARGE = 0x91

**Spec:** Annex B, S. 278 — "const octet TI_PLAIN_ARRAY_LARGE = 0x91;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_ARRAY_LARGE` (Z. 79)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_array_large_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_MAP_SMALL = 0xA0

**Spec:** Annex B, S. 278 — "const octet TI_PLAIN_MAP_SMALL = 0xA0;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_MAP_SMALL` (Z. 82)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_map_small_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_PLAIN_MAP_LARGE = 0xA1

**Spec:** Annex B, S. 278 — "const octet TI_PLAIN_MAP_LARGE = 0xA1;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_PLAIN_MAP_LARGE` (Z. 84)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_map_large_with_nested_map_value_roundtrips`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B TypeIdentifier TI_STRONGLY_CONNECTED_COMPONENT = 0xB0

**Spec:** Annex B, S. 279 — "const octet TI_STRONGLY_CONNECTED_COMPONENT =
0xB0;"

**Repo:** `crates/types/src/type_identifier/kinds.rs::TI_STRONGLY_CONNECTED_COMPONENT` (Z. 87)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::a_component_identifier_is_the_thirty_two_octets_of_annex_b`,
`crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_sequence_small_minimal_wire_vector`

**Status:** done

### Annex B MEMBER_NAME_MAX_LENGTH und TYPE_NAME_MAX_LENGTH = 256 werden geprüft

**Spec:** Annex B, S. 279 — "const long MEMBER_NAME_MAX_LENGTH = 256; typedef
string<MEMBER_NAME_MAX_LENGTH> MemberName; [...] const long TYPE_NAME_MAX_LENGTH
= 256; typedef string<TYPE_NAME_MAX_LENGTH> QualifiedTypeName;"

**Repo:** —

**Tests:** `crates/types/tests/annex_b_limits_and_masks.rs::the_limits_are_the_constants_of_annex_b`,
`crates/types/tests/annex_b_limits_and_masks.rs::a_member_name_holds_at_most_256_octets`,
`crates/types/tests/annex_b_limits_and_masks.rs::a_member_name_over_256_octets_is_refused_on_reading`,
`crates/types/tests/annex_b_limits_and_masks.rs::a_type_name_holds_at_most_256_octets`,
`crates/types/tests/annex_b_limits_and_masks.rs::an_object_with_a_long_type_name_does_not_encode`

**Status:** done — Ein Member- oder Typname über 256 Oktette wird beim Schreiben und beim Lesen abgewiesen; die Konstanten stehen in `type_object::common`.

### Annex B EquivalenceHash[14], NameHash[4]

**Spec:** Annex B, S. 279 — "typedef octet EquivalenceHash[14]; [...] typedef
octet NameHash[4];"

**Repo:** `crates/types/src/type_identifier/mod.rs::EquivalenceHash` (Z. 54);
`crates/types/src/type_object/common.rs::NameHash` (Z. 26)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::equivalence_hash_wire_is_discriminator_plus_14_bytes`,
`crates/types/tests/compliance_typeobject.rs::md5_empty_string_first_14_bytes_golden_vector`

**Status:** done

### Annex B LBound, SBound, INVALID_LBOUND, INVALID_SBOUND

**Spec:** Annex B, S. 279 — "typedef unsigned long LBound; [...] const LBound
INVALID_LBOUND = 0; [...] typedef octet SBound; [...] const SBound
INVALID_SBOUND = 0;"

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (Z. 197)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::plain_sequence_large_with_u32_max_bound_roundtrips`,
`crates/types/src/type_identifier/mod.rs::tests::string8_small_roundtrips`

**Status:** done — 0 steht für unbeschränkt.

### Annex B TypeObjectHashId

**Spec:** Annex B, S. 280 — "union TypeObjectHashId switch (octet) { case
EK_COMPLETE: case EK_MINIMAL: EquivalenceHash hash; };"

**Repo:** `crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId` (Z. 176)

**Tests:** `crates/types/src/type_identifier/mod.rs::tests::a_component_identifier_takes_only_the_two_equivalence_kinds`

**Status:** done

### Annex B MemberFlag TRY_CONSTRUCT1 @position(0)

**Spec:** Annex B, S. 280 — "@position(0) TRY_CONSTRUCT1, // T1".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (Z. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag TRY_CONSTRUCT2 @position(1)

**Spec:** Annex B, S. 280 — "@position(1) TRY_CONSTRUCT2, // T2".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (Z. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag IS_EXTERNAL @position(2)

**Spec:** Annex B, S. 280 — "@position(2) IS_EXTERNAL, // X".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (Z. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag IS_OPTIONAL @position(3)

**Spec:** Annex B, S. 280 — "@position(3) IS_OPTIONAL, // O".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (Z. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag IS_MUST_UNDERSTAND @position(4)

**Spec:** Annex B, S. 280 — "@position(4) IS_MUST_UNDERSTAND, // M".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (Z. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag IS_KEY @position(5)

**Spec:** Annex B, S. 280 — "@position(5) IS_KEY, // K".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (Z. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag IS_DEFAULT @position(6)

**Spec:** Annex B, S. 280 — "@position(6) IS_DEFAULT, // D".

**Repo:** `crates/types/src/type_object/flags.rs::StructMemberFlag` (Z. 39)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`

**Status:** done

### Annex B MemberFlag: anwendbare Flags je Member-Art

**Spec:** Annex B, S. 280 — "typedef MemberFlag CollectionElementFlag; // T1,
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

**Status:** done — Struktur-Member tragen nur T1, T2, O, M, K, X; Union-Member T1, T2, D, X; ein Enum-Literal nur D; Collection-Elemente T1, T2, X; Alias-Member, Bitflags und Bitfelder 0. Der Union-Diskriminator trägt zusätzlich M (Annex B nennt T1, T2, K): §7.2.2.4.4.4.6 verlangt "always true", Cyclone DDS und Fast DDS schreiben `0x11` (`docs/omg-xtypes-issues.md` §B2); der Widerspruch steht in der Befunddatei. Ein Key-Member trägt M nach §7.2.2.4.4.4.8. Erwartung bei Cyclone DDS, Fast DDS und OpenDDS für das M am Key-Member: nicht gemessen.

### Annex B MemberFlagMinimalMask = 0x003f, TypeFlagMinimalMask = 0x0007

**Spec:** Annex B, S. 280–281 — "// Selects T1, T2, O, M, K, D const unsigned
short MemberFlagMinimalMask = 0x003f; [...] const unsigned short
TypeFlagMinimalMask = 0x0007; // Selects M, A, F".

**Repo:** `crates/types/src/type_object/flags.rs::minimal_member_flags`;
`crates/types/src/type_object/flags.rs::minimal_union_member_flags`

**Tests:** `crates/types/tests/annex_b_flags.rs::the_minimal_object_keeps_the_flags_that_decide_assignability`,
`crates/types/tests/annex_b_limits_and_masks.rs::the_masks_are_the_constants_of_annex_b`,
`crates/types/tests/annex_b_limits_and_masks.rs::the_projection_masks_member_and_type_flags`,
`crates/types/tests/annex_b_limits_and_masks.rs::a_minimal_struct_carries_only_the_extensibility_flags`

**Status:** done — `MemberFlagMinimalMask = 0x003f` und `TypeFlagMinimalMask = 0x0007` wirken in der MINIMAL-Projektion. Der Kommentar in Annex B nennt D unter den gewählten Flags, der Wert `0x003f` wählt T1, T2, X, O, M, K (Widerspruch in der Befunddatei): ein minimales Union-Member und ein Enum-Literal behalten `IS_DEFAULT`, weil das minimale Union keinen anderen Platz für den Default-Zweig hat und die Zuweisbarkeit ihn liest.

### Annex B TypeFlag IS_FINAL @position(0)

**Spec:** Annex B, S. 281 — "@position(0) IS_FINAL, // F".

**Repo:** `crates/types/src/type_object/flags.rs::StructTypeFlag` (Z. 10)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_mutable_final_flags`

**Status:** done

### Annex B TypeFlag IS_APPENDABLE @position(1)

**Spec:** Annex B, S. 281 — "@position(1) IS_APPENDABLE, // A".

**Repo:** `crates/types/src/type_object/flags.rs::StructTypeFlag` (Z. 10)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_mutable_final_flags`

**Status:** done

### Annex B TypeFlag IS_MUTABLE @position(2)

**Spec:** Annex B, S. 281 — "@position(2) IS_MUTABLE, // M".

**Repo:** `crates/types/src/type_object/flags.rs::StructTypeFlag` (Z. 10)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_mutable_final_flags`

**Status:** done

### Annex B TypeFlag IS_NESTED @position(3)

**Spec:** Annex B, S. 281 — "@position(3) IS_NESTED, // N".

**Repo:** `crates/types/src/type_object/flags.rs::StructTypeFlag` (Z. 10)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_mutable_final_flags`

**Status:** done

### Annex B TypeFlag IS_AUTOID_HASH @position(4)

**Spec:** Annex B, S. 281 — "@position(4) IS_AUTOID_HASH, // H".

**Repo:** `crates/types/src/type_object/flags.rs::StructTypeFlag` (Z. 10)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_mutable_final_flags`

**Status:** done

### Annex B TypeFlag: nur Struct und Union, sonst unbenutzt

**Spec:** Annex B, S. 281 — "typedef TypeFlag StructTypeFlag; // All flags apply
typedef TypeFlag UnionTypeFlag; // All flags apply typedef TypeFlag
CollectionTypeFlag; // Unused. No flags apply [...] EnumTypeFlag; // Unused".

**Repo:** `crates/types/src/type_object/flags.rs::EnumTypeFlag`

**Tests:** `crates/types/tests/annex_b_flags.rs::an_enumeration_writes_its_extensibility_and_the_other_unused_type_flags_are_zero`,
`crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`

**Status:** done — Struktur und Union tragen alle Flags; Collection, Annotation, Alias, Bitmask und Bitset 0 ("Unused"). Eine Aufzählung schreibt ihre Extensibility in `enum_flags`, obwohl Annex B das Wort unbenutzt nennt: Table 18 vergleicht die Extensibility zweier Aufzählungen, RTI Connext, OpenDDS und Cyclone DDS schreiben das Bit (`docs/omg-xtypes-issues.md` §B8); Fast DDS schreibt 0. Der Widerspruch steht in der Befunddatei.

### Annex B Bodies der TypeIdentifier und union TypeIdentifier

**Spec:** Annex B, S. 281–284 — `StringSTypeDefn`, `StringLTypeDefn`,
`PlainCollectionHeader`, `PlainSequenceSElemDefn`, `PlainSequenceLElemDefn`,
`PlainArraySElemDefn`, `PlainArrayLElemDefn`, `PlainMapSTypeDefn`,
`PlainMapLTypeDefn`, `StronglyConnectedComponentId`, `ExtendedTypeDefn`, `union
TypeIdentifier switch (octet)`.

**Repo:** `crates/types/src/type_identifier/mod.rs::TypeIdentifier` (Z. 197);
`crates/types/src/type_identifier/mod.rs::PlainCollectionHeader` (Z. 122);
`crates/types/src/type_identifier/mod.rs::StronglyConnectedComponentId` (Z. 176)

**Tests:** `crates/types/tests/compliance_typeobject.rs::indirect_hash_plain_map_small_minimal_keys_complete_values_wire_vector`,
`crates/types/src/type_identifier/mod.rs::tests::the_extended_arm_carries_its_body`

**Status:** done — die einzelnen Formen sind unter §7.3.4.2 Table 29 geführt.

### Annex B ANNOTATION_STR_VALUE_MAX_LEN, ANNOTATION_OCTETSEC_VALUE_MAX_LEN = 128 werden geprüft

**Spec:** Annex B, S. 286 — "const unsigned long ANNOTATION_STR_VALUE_MAX_LEN =
128; const unsigned long ANNOTATION_OCTETSEC_VALUE_MAX_LEN = 128;"

**Repo:** —

**Tests:** `crates/types/tests/annex_b_limits_and_masks.rs::an_annotation_string_value_holds_at_most_128_octets`,
`crates/types/tests/annex_b_limits_and_masks.rs::an_annotation_wstring_value_holds_at_most_128_code_units`

**Status:** done — Ein String-Wert über 128 Oktette und ein wstring-Wert über 128 Codeeinheiten wird beim Schreiben und beim Lesen abgewiesen.

### Annex B AnnotationParameterValue und ExtendedAnnotationParameterValue

**Spec:** Annex B, S. 286 — "union AnnotationParameterValue switch (octet) {
case TK_BOOLEAN: [...] case TK_FLOAT128: [...] case TK_CHAR16: [...] case
TK_STRING16: [...] default: ExtendedAnnotationParameterValue extended_value; };"

**Repo:** `crates/types/src/type_object/common.rs::AnnotationParameterValue` (Z. 488)

**Tests:** `crates/types/tests/typeobject_annotation_annexb.rs::the_default_value_is_the_annex_b_union_and_never_empty`,
`crates/types/tests/annotation_parameter_value_union.rs::an_unknown_discriminator_is_the_extended_value`,
`crates/types/tests/annotation_parameter_value_union.rs::an_extended_value_with_the_discriminator_of_a_case_is_refused`

**Status:** done — `AnnotationParameterValue` hat alle Fälle der Union (`Float128Bits`, `Char16`, `String16`) und den erweiterten Fall mit DHEADER; ein unbekannter Diskriminator ist der erweiterte Wert. Unter XCDR1 ist der Zweig eine Parameterliste, die `PID_LIST_END` schließt (der Typ ist MUTABLE und leer); ein unbekanntes Member darin wird übersprungen, ein must-understand-Member weist den Wert ab, und Member, die unter XCDR2 für einen Typ einer späteren Revision gelesen wurden, haben keine XCDR1-Form (`320c16503d2`, Tests in `crates/types/tests/type_lookup_xcdr1.rs`).

### Annex B AppliedAnnotationParameter, AppliedAnnotation, AppliedAnnotationSeq

**Spec:** Annex B, S. 287–288 — `struct AppliedAnnotationParameter { NameHash
paramname_hash; AnnotationParameterValue value; }`, `struct AppliedAnnotation {
TypeIdentifier annotation_typeid; @optional AppliedAnnotationParameterSeq
param_seq; }`.

**Repo:** `crates/types/src/type_object/common.rs::AppliedAnnotationParameter` (Z. 313);
`crates/types/src/type_object/common.rs::AppliedAnnotation` (Z. 348)

**Tests:** `crates/types/tests/type_object_custom_annotations.rs::the_applied_annotation_octets_are_annex_b`,
`crates/types/tests/type_object_custom_annotations.rs::a_parameterless_annotation_is_written_without_a_param_seq`

**Status:** done

### Annex B AppliedVerbatimAnnotation

**Spec:** Annex B, S. 288 — "struct AppliedVerbatimAnnotation { string<32>
placement; string<32> language; string text; };"

**Repo:** `crates/types/src/type_object/common.rs::AppliedVerbatimAnnotation` (Z. 207)

**Tests:** `crates/types/src/type_object/mod.rs::tests::complete_verbatim_placement_variants_roundtrip`,
`crates/types/src/type_object/common.rs::safe_capacity_tests::verbatim_placement_roundtrip_other_forward_compat`

**Status:** done — `placement` als `VerbatimPlacement` mit unbekanntem Wert als
eigene Variante.

### Annex B AppliedBuiltinMemberAnnotations

**Spec:** Annex B, S. 288 — "struct AppliedBuiltinMemberAnnotations { @optional
string unit; @optional AnnotationParameterValue min; @optional
AnnotationParameterValue max; @optional string hash_id; };"

**Repo:** `crates/types/src/type_object/common.rs::AppliedBuiltinMemberAnnotations` (Z. 640)

**Tests:** `crates/types/tests/type_object_annotations.rs::the_builtin_member_annotations_reach_the_object`,
`crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`

**Status:** done

### Annex B CommonStructMember, CompleteMemberDetail, MinimalMemberDetail, Complete/MinimalStructMember

**Spec:** Annex B, S. 288–289 — `CommonStructMember { MemberId member_id;
StructMemberFlag member_flags; TypeIdentifier member_type_id; }`,
`CompleteMemberDetail { MemberName name; @optional
AppliedBuiltinMemberAnnotations ann_builtin; @optional AppliedAnnotationSeq
ann_custom; }`, `MinimalMemberDetail { NameHash name_hash; }`.

**Repo:** `crates/types/src/type_object/common.rs::CommonStructMember` (Z. 89);
`crates/types/src/type_object/common.rs::CompleteMemberDetail` (Z. 753);
`crates/types/src/type_object/complete/struct_type.rs::CompleteStructMember` (Z. 28);
`crates/types/src/type_object/minimal/struct_type.rs::MinimalStructMember` (Z. 46)

**Tests:** `crates/types/tests/typeobject_member_annotations.rs::the_complete_object_is_cyclones_octet_for_octet`,
`crates/types/tests/typeobject_member_annotations.rs::the_minimal_object_is_cyclones_octet_for_octet`

**Status:** done

### Annex B AppliedBuiltinTypeAnnotations, Minimal/CompleteTypeDetail, Struct-Header und Struct-Typen

**Spec:** Annex B, S. 289–290 — `AppliedBuiltinTypeAnnotations { @optional
AppliedVerbatimAnnotation verbatim; }`, `MinimalTypeDetail { }`,
`CompleteTypeDetail { @optional AppliedBuiltinTypeAnnotations ann_builtin;
@optional AppliedAnnotationSeq ann_custom; QualifiedTypeName type_name; }`,
`CompleteStructHeader`, `MinimalStructHeader`, `CompleteStructType`,
`MinimalStructType`.

**Repo:** `crates/types/src/type_object/common.rs::AppliedBuiltinTypeAnnotations` (Z. 262);
`crates/types/src/type_object/common.rs::CompleteTypeDetail` (Z. 434);
`crates/types/src/type_object/complete/struct_type.rs::CompleteStructType` (Z. 37);
`crates/types/src/type_object/minimal/struct_type.rs::MinimalStructType` (Z. 92)

**Tests:** `crates/types/tests/typeobject_vendor_dheader.rs::a_serialized_type_object_is_cyclones_octet_for_octet`,
`crates/types/src/type_object/mod.rs::tests::minimal_struct_with_base_type`,
`crates/types/src/type_object/mod.rs::tests::complete_struct_sensor_with_names_and_annotations`

**Status:** done

### Annex B Union: UnionCaseLabelSeq, Member, Diskriminator, Header, Typ

**Spec:** Annex B, S. 291–292 — `typedef sequence<long> UnionCaseLabelSeq`,
`CommonUnionMember`, `Complete/MinimalUnionMember`, `CommonDiscriminatorMember`,
`Complete/MinimalDiscriminatorMember`, `Complete/MinimalUnionHeader`,
`Complete/MinimalUnionType`.

**Repo:** `crates/types/src/type_object/common.rs::CommonUnionMember` (Z. 127);
`crates/types/src/type_object/complete/union_type.rs::CompleteUnionType` (Z. 47);
`crates/types/src/type_object/minimal/union_type.rs::MinimalUnionType` (Z. 119)

**Tests:** `crates/types/src/type_object/mod.rs::tests::minimal_union_with_int_discriminator_two_cases`,
`crates/types/src/type_object/mod.rs::tests::complete_union_roundtrips`,
`crates/types/tests/typeobject_serialized_size.rs::the_announced_size_opens_with_the_dheader_of_the_appendable_union`

**Status:** done — Labels sind `long`; ein Diskriminator `long long` mit
größerem Label lässt sich so nicht darstellen (Grenze der Spec).

### Annex B Annotation: Parameter, Header, Typ

**Spec:** Annex B, S. 293–294 — `CommonAnnotationParameter`,
`Complete/MinimalAnnotationParameter` ("Ordered by
CompleteAnnotationParameter.name" bzw. "MinimalAnnotationParameter.name_hash"),
`Complete/MinimalAnnotationHeader`, `Complete/MinimalAnnotationType`.

**Repo:** `crates/types/src/type_object/complete/annotation_type.rs::CompleteAnnotationType` (Z. 129);
`crates/types/src/type_object/minimal/annotation_type.rs::MinimalAnnotationType` (Z. 141)

**Tests:** `crates/types/tests/typeobject_annotation_annexb.rs::the_complete_object_is_annex_bs_octet_for_octet`,
`crates/types/tests/typeobject_annotation_annexb.rs::the_minimal_object_is_annex_bs_octet_for_octet`,
`crates/types/tests/typeobject_annotation_annexb.rs::each_sequence_is_ordered_by_its_own_key`

**Status:** done

### Annex B Alias: CommonAliasBody, Body, Header, Typ

**Spec:** Annex B, S. 294–295 — `CommonAliasBody { AliasMemberFlag
related_flags; TypeIdentifier related_type; }`, `Complete/MinimalAliasBody`,
`Complete/MinimalAliasHeader`, `Complete/MinimalAliasType`.

**Repo:** `crates/types/src/type_object/minimal/alias_type.rs::CommonAliasBody` (Z. 12);
`crates/types/src/type_object/complete/alias_type.rs::CompleteAliasType` (Z. 36)

**Tests:** `crates/types/src/type_object/mod.rs::tests::minimal_alias_int64_typedef`,
`crates/types/src/type_object/mod.rs::tests::complete_alias_typedef`

**Status:** done

### Annex B Collections: Element, Header, Sequence, Array, Map

**Spec:** Annex B, S. 295–298 — `CompleteElementDetail`,
`CommonCollectionElement`, `Complete/MinimalCollectionElement`,
`CommonCollectionHeader`, `Complete/MinimalCollectionHeader`,
`Complete/MinimalSequenceType`, `CommonArrayHeader`,
`Complete/MinimalArrayHeader`, `Complete/MinimalArrayType`,
`Complete/MinimalMapType`.

**Repo:** `crates/types/src/type_object/complete/collection_types.rs::CompleteSequenceType` (Z. 58);
`crates/types/src/type_object/complete/collection_types.rs::CompleteArrayType` (Z. 93);
`crates/types/src/type_object/complete/collection_types.rs::CompleteMapType` (Z. 131);
`crates/types/src/type_object/minimal/collection_types.rs::MinimalMapType` (Z. 155)

**Tests:** `crates/types/tests/typeobject_collections_annex_b.rs` (sechs Tests,
gegen den Serialisierer von Cyclone DDS 11.0.1),
`crates/types/src/type_object/mod.rs::tests::minimal_sequence_of_floats`,
`crates/types/src/type_object/mod.rs::tests::minimal_array_3d_doubles`,
`crates/types/src/type_object/mod.rs::tests::minimal_map_string_to_int64`,
`crates/types/src/type_object/mod.rs::tests::complete_map_roundtrips`

**Status:** done — Header und Element sind in die Typen eingebettet, nicht als
eigene Rust-Typen benannt; die Rahmung ist die von Annex B (`cb58f485ebd`):
`CompleteCollectionElement`, `CompleteCollectionHeader`, `CompleteArrayHeader`,
`CompleteArrayType`, `MinimalCollectionElement`, `MinimalCollectionHeader` und
`MinimalArrayHeader` sind appendable (unter XCDR2 je ein `DHEADER`), das
`detail` von `CompleteCollectionHeader` ist `@optional` und fehlt bei einem
anonymen Typ („not present for anonymous“), wie bei jeder Sequence und Map der
IDL (`CompleteSequenceType::detail`, `CompleteMapType::detail` als `Option`).
Vorher schrieben die Codecs Header und Element ohne `DHEADER` und das `detail`
immer, mit einem aus Besitzer und Member gebildeten Namen; die Tests waren
Rundläufe des eigenen Codecs und sahen das nicht. Gemessen gegen
`dds_stream_write_sampleLE` von Cyclone DDS 11.0.1 mit dessen installiertem
`DDS_XTypes_TypeObject`-Deskriptor (`internal/release/evidence/rc7-typeobject-collections`):
die acht Objekte (Sequence benannt und anonym, Element mit `@unit`, Array, Map,
und die drei minimalen) unter XCDR2 oktettgleich, die drei minimalen auch unter
XCDR1; Cyclone schreibt eine COMPLETE-Collection unter XCDR1 nicht (der Aufruf
gibt `false` zurück), dort gilt der eigene Rundlauf. Der Name im `detail` eines
Arrays (immer vorhanden) ist der aus Besitzer und Member, `Holder.arr`;
Cyclone setzt dort den Scoped Name des Deklarators (Quelle gelesen,
`descriptor_type_meta.c`, nicht gemessen).

### Annex B Enumeration: BitBound, Literal, Header, Typ

**Spec:** Annex B, S. 298–299 — `typedef unsigned short BitBound`,
`CommonEnumeratedLiteral { long value; EnumeratedLiteralFlag flags; }`,
`Complete/MinimalEnumeratedLiteral` ("Ordered by
EnumeratedLiteral.common.value"), `CommonEnumeratedHeader { BitBound bit_bound;
}`, `Complete/MinimalEnumeratedHeader`, `Complete/MinimalEnumeratedType`.

**Repo:** `crates/types/src/type_object/minimal/enum_type.rs::CommonEnumeratedLiteral` (Z. 28);
`crates/types/src/type_object/complete/enum_type.rs::CompleteEnumeratedType` (Z. 36);
`crates/types/src/type_object/minimal/enum_type.rs::MinimalEnumeratedType` (Z. 76)

**Tests:** `crates/types/src/type_object/mod.rs::tests::minimal_enum_color_3_literals`,
`crates/types/src/type_object/mod.rs::tests::complete_enum_with_literal_names`,
`crates/types/tests/literal_and_flag_order.rs::enumerated_literals_are_written_in_increasing_value_order`

**Status:** done — Die Literale werden nach aufsteigendem Wert geschrieben.

### Annex B Bitmask: Bitflag, Header, Typ

**Spec:** Annex B, S. 300–301 — `CommonBitflag { unsigned short position;
BitflagFlag flags; }`, `Complete/MinimalBitflag` ("Ordered by
Bitflag.position"), `CommonBitmaskHeader`, `Complete/MinimalBitmaskHeader`,
`Complete/MinimalBitmaskType`.

**Repo:** `crates/types/src/type_object/minimal/bitmask_type.rs::CommonBitflag` (Z. 14);
`crates/types/src/type_object/complete/bitmask_type.rs::CompleteBitmaskType` (Z. 27)

**Tests:** `crates/types/src/type_object/mod.rs::tests::minimal_bitmask_3_flags`,
`crates/types/src/type_object/mod.rs::tests::complete_bitmask_roundtrips`,
`crates/types/tests/literal_and_flag_order.rs::bitmask_flags_are_written_in_increasing_position_order`

**Status:** done — Die Flags werden nach aufsteigender Position geschrieben.

### Annex B Bitset: Bitfield, Header, Typ

**Spec:** Annex B, S. 301–302 — `CommonBitfield { unsigned short position;
BitsetMemberFlag flags; octet bitcount; TypeKind holder_type; }`,
`Complete/MinimalBitfield`, `Complete/MinimalBitsetHeader`,
`Complete/MinimalBitsetType`.

**Repo:** `crates/types/src/type_object/minimal/bitset_type.rs::CommonBitfield` (Z. 17);
`crates/types/src/type_object/complete/bitset_type.rs::CompleteBitsetType` (Z. 27)

**Tests:** `crates/types/src/type_object/mod.rs::tests::minimal_bitset_roundtrips`,
`crates/types/src/type_object/mod.rs::tests::complete_bitset_roundtrips`

**Status:** done

### Annex B CompleteTypeObject, MinimalTypeObject, ExtendedType

**Spec:** Annex B, S. 302–303 — `union CompleteTypeObject switch (octet) { case
TK_ALIAS: [...] default: CompleteExtendedType extended_type; }`, `union
MinimalTypeObject switch (octet)`.

**Repo:** `crates/types/src/type_object/complete/mod.rs::CompleteTypeObject` (Z. 44);
`crates/types/src/type_object/minimal/mod.rs::MinimalTypeObject` (Z. 47)

**Tests:** `crates/types/src/type_object/mod.rs::tests::an_extended_type_kind_keeps_the_member_list_it_arrived_with`,
`crates/types/src/type_object/mod.rs::tests::a_type_kind_this_revision_does_not_define_survives_the_round_trip`

**Status:** done — Unter XCDR1 ist der Zweig eine Parameterliste, die `PID_LIST_END` schließt (der Typ ist MUTABLE und leer); ein unbekanntes Member darin wird übersprungen, ein must-understand-Member weist den Wert ab, und Member, die unter XCDR2 für einen Typ einer späteren Revision gelesen wurden, haben keine XCDR1-Form (`320c16503d2`, Tests in `crates/types/tests/type_lookup_xcdr1.rs`).

### Annex B union TypeObject, TypeObjectSeq, StronglyConnectedComponent

**Spec:** Annex B, S. 304 — "union TypeObject switch (octet) { //
EquivalenceKind case EK_COMPLETE: CompleteTypeObject complete; case EK_MINIMAL:
MinimalTypeObject minimal; }; typedef sequence<TypeObject> TypeObjectSeq; [...]
typedef TypeObjectSeq StronglyConnectedComponent;"

**Repo:** `crates/types/src/type_object/mod.rs::TypeObject` (Z. 36)

**Tests:** `crates/types/src/type_object/mod.rs::tests::unknown_equivalence_kind_fails_gracefully`,
`crates/types/tests/strongly_connected_components.rs::a_component_members_type_object_builds`

**Status:** done

### Annex B TypeIdentifierTypeObjectPair, TypeIdentifierPair, TypeIdentfierWithSize, TypeIdentifierWithDependencies, TypeInformation

**Spec:** Annex B, S. 305–306 — `TypeIdentifierTypeObjectPair`,
`TypeIdentifierPair`, `TypeIdentfierWithSize`, `TypeIdentifierWithDependencies`,
`@extensibility(MUTABLE) struct TypeInformation`.

**Repo:** `crates/types/src/type_lookup.rs::TypeIdentifierTypeObjectPair` (Z. 166);
`crates/types/src/type_lookup.rs::TypeIdentifierPair` (Z. 205);
`crates/types/src/type_information.rs::TypeInformation` (Z. 209)

**Tests:** `crates/types/src/type_lookup.rs::tests::a_reply_pair_names_the_identifier_it_answers`,
`crates/discovery/tests/representation_and_type_information.rs::type_information_is_built_from_a_keyed_appendable_struct`

**Status:** done — siehe §7.6.3.2.1 und §7.6.3.3.3.

---

## Annex C: Dynamic Language Binding

### Annex C IDL-PSM des Dynamic Language Binding

**Spec:** Annex C, S. 307–322 — IDL von `DynamicTypeBuilderFactory`,
`TypeSupport`, `DynamicTypeSupport`, `AnnotationDescriptor`,
`VerbatimTextDescriptor`, `TypeDescriptor`, `MemberDescriptor`,
`DynamicTypeMember`, `DynamicTypeBuilder`, `DynamicType`, `DynamicDataFactory`,
`DynamicData`.

**Repo:** `crates/types/src/dynamic/`

**Tests:** `crates/dcps/tests/annex_c_idl_conformance.rs::every_operation_of_annex_c_has_its_counterpart`; `crates/dcps/tests/annex_c_idl_conformance.rs::the_state_of_the_valuetypes_is_in_the_records`; `crates/dcps/tests/annex_c_idl_conformance.rs::the_type_kind_constants_of_the_idl_are_the_rust_constants`

**Status:** done — der Test liest die IDL des Annex (`crates/types/tests/fixtures/annex-c-dynamic-language-binding.idl`) und prüft, dass jede Konstante, Operation und jedes Zustandsfeld der Valuetypes eine Entsprechung in der Rust-API hat; Log: `.agent/REPORT.md`.

### Annex C TypeKind-Konstanten

**Spec:** Annex C, S. 307–308 — "const TypeKind TK_NONE = 0x00; [...] const
TypeKind TK_MAP = 0x62;"

**Repo:** `crates/types/src/dynamic/descriptor.rs::TK_NONE`; `TypeKindValue`

**Tests:** `crates/types/tests/dynamic_annex_c.rs::the_type_kind_constants_have_the_numbers_of_annex_c`

**Status:** done — `TK_NONE` bis `TK_MAP` mit den Zahlen des Annex; Log: `.agent/REPORT.md`.

### Annex C VerbatimTextDescriptor und get_verbatim_text

**Spec:** Annex C, S. 310–312 — "valuetype VerbatimTextDescriptor { public
string placement; public string text; [...] }";
`DynamicTypeMember::get_verbatim_text_count`, `get_verbatim_text`.

**Repo:** `crates/types/src/dynamic/type_.rs::verbatim_text_count`

**Tests:** `crates/types/tests/dynamic_annotation_descriptors.rs::verbatim_text_is_reported_with_its_placement`

**Status:** done — `get_verbatim_text_count` und `get_verbatim_text` an `DynamicType` und `DynamicTypeMember`; Log: `.agent/REPORT.md`.

### Annex C ExtensibilityKind und TryConstructKind

**Spec:** Annex C, S. 311 — "enum ExtensibilityKind { FINAL, APPENDABLE, MUTABLE
}; enum TryConstructKind { USE_DEFAULT, DISCARD, TRIM };"

**Repo:** `crates/types/src/dynamic/descriptor.rs::ExtensibilityKind` (Z. 275);
`crates/types/src/dynamic/descriptor.rs::TryConstructKind` (Z. 331)

**Tests:** `crates/types/src/dynamic/descriptor.rs::tests::the_two_renames_round_trip`

**Status:** done

### Annex C TypeDescriptor mit extensibility_kind und is_nested

**Spec:** Annex C, S. 311 — "valuetype TypeDescriptor { public TypeKind kind;
public ObjectName name; public DynamicType base_type; public DynamicType
discriminator_type; public BoundSeq bound; @optional public DynamicType
element_type; @optional public DynamicType key_element_type; public
ExtensibilityKind extensibility_kind; public boolean is_nested; [...] };"

**Repo:** `crates/types/src/dynamic/descriptor.rs::TypeDescriptor` (Z. 381)

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`

**Status:** done — Table 50 nennt `extensibility_kind` und `is_nested` nicht (in
der Befunddatei aufgeführt).

### Annex C MemberDescriptor mit try_construct_kind und den Flags

**Spec:** Annex C, S. 311–312 — "valuetype MemberDescriptor { public ObjectName
name; public MemberId id; public DynamicType type; public string default_value;
public unsigned long index; public UnionCaseLabelSeq label; public
TryConstructKind try_construct_kind; public boolean is_key; public boolean
is_optional; public boolean is_must_understand; public boolean is_shared; public
boolean is_default_label; [...] };"

**Repo:** `crates/types/src/dynamic/descriptor.rs::MemberDescriptor`

**Tests:** `crates/types/src/dynamic/type_.rs::tests::a_view_reports_what_the_model_holds`; `crates/types/tests/dynamic_type_descriptors.rs::a_union_label_past_32_bits_is_kept`; `crates/types/tests/dynamic_annotation_descriptors.rs::a_member_descriptor_has_a_default_string_and_value_semantics`; `crates/dcps/tests/annex_c_idl_conformance.rs::the_state_of_the_valuetypes_is_in_the_records`

**Status:** done — `MemberDescriptor` trägt `try_construct_kind`, die Flags und `default_value` als String (§7.5.2.7.3, eigenes Item: done); `label` ist `Vec<i64>` wie Table 53; Log in `.agent/REPORT.md`.

### Annex C Sequenztypen Int32Seq bis WstringSeq für DynamicData

**Spec:** Annex C, S. 315 — "typedef sequence<long> Int32Seq; [...] typedef
sequence<wstring> WstringSeq;"

**Repo:** `crates/types/src/dynamic/sample.rs::Int32Seq`

**Tests:** `crates/types/tests/dynamic_annex_c.rs::the_typed_sequences_are_the_types_of_the_values_operations`

**Status:** done — `Int32Seq` bis `WstringSeq`; Log: `.agent/REPORT.md`.

---

## Annex D: DDS Built-in Topic Data Types

### Annex D Serialisierung mit XCDR Version 1

**Spec:** Annex D, S. 323 — "// The types in this file shall be serialized with
XCDR encoding version 1".

**Repo:** `crates/rtps/src/parameter_list.rs`

**Tests:** `crates/rtps/src/parameter_list.rs::tests::pid_ignore_skipped_in_pl_cdr_decode`

**Status:** done — die Built-in-Topic-Daten laufen als PL_CDR (Parameterliste
der RTPS).

### Annex D QoS-Strukturen und Aufzählungen der Built-in-Topics

**Spec:** Annex D, S. 323–327 — `BuiltinTopicKey_t`, `Duration_t`,
`DeadlineQosPolicy`, `DestinationOrderQosPolicy`, `DurabilityQosPolicy`,
`HistoryQosPolicy`, `DurabilityServiceQosPolicy`, `GroupDataQosPolicy`,
`LatencyBudgetQosPolicy`, `LifespanQosPolicy`, `LivelinessQosPolicy`,
`OwnershipQosPolicy`, `OwnershipStrengthQosPolicy`, `PartitionQosPolicy`,
`PresentationQosPolicy`, `ReliabilityQosPolicy` (`@value(1)`, `@value(2)`),
`ResourceLimitsQosPolicy`, `TimeBasedFilterQosPolicy`, `TopicDataQosPolicy`,
`TransportPriorityQosPolicy`, `UserDataQosPolicy`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Wiedergabe der DDS- und RTPS-Definitionen;
geprüft im Audit von DDS 1.4 und DDSI-RTPS 2.5.

### Annex D ParticipantBuiltinTopicData: key 0x0050, user_data 0x002C

**Spec:** Annex D, S. 327 — "@extensibility(MUTABLE) struct
ParticipantBuiltinTopicData { @id(0x0050) @key BuiltinTopicKey_t key;
@id(0x002C) UserDataQosPolicy user_data; };"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — RTPS-Parameter; geprüft im Audit von DDSI-RTPS
2.5.

### Annex D DataRepresentationId_t und Konstanten 0, 1, 2

**Spec:** Annex D, S. 327 — "typedef short DataRepresentationId_t; const
DataRepresentationId_t XCDR_DATA_REPRESENTATION = 0; const
DataRepresentationId_t XML_DATA_REPRESENTATION = 1; const DataRepresentationId_t
XCDR2_DATA_REPRESENTATION = 2;"

**Repo:** `crates/rtps/src/publication_data.rs::DEFAULT_OFFER` (Z. 108);
`crates/types/src/qos.rs::DataRepresentationId` (Z. 174)

**Tests:** `crates/dcps/src/runtime.rs::tests::data_repr_offer_str_uses_spec_ids`

**Status:** done

### Annex D DATA_REPRESENTATION_QOS_POLICY_ID = 23, TYPE_CONSISTENCY_ENFORCEMENT_QOS_POLICY_ID = 24

**Spec:** Annex D, S. 327–328 — "const QosPolicyId_t
DATA_REPRESENTATION_QOS_POLICY_ID = 23; [...] const QosPolicyId_t
TYPE_CONSISTENCY_ENFORCEMENT_QOS_POLICY_ID = 24;"

**Repo:** `crates/dcps/src/psm_constants.rs`
(`qos_policy_id::DATA_REPRESENTATION`,
`qos_policy_id::TYPE_CONSISTENCY_ENFORCEMENT`)

**Tests:** `crates/dcps/src/psm_constants.rs::tests::the_xtypes_policy_ids_are_those_of_its_idl`

**Status:** `done` — 23 und 24 wie in Annex D.

### Annex D Namen DataRepresentation und TypeConsistencyEnforcement

**Spec:** Annex D, S. 327–328 — "const string
DATA_REPRESENTATION_QOS_POLICY_NAME = 'DataRepresentation'; [...] const string
TYPE_CONSISTENCY_ENFORCEMENT_QOS_POLICY_NAME = 'TypeConsistencyEnforcement';"

**Repo:** `crates/dcps/src/psm_constants.rs`

**Tests:** `crates/dcps/src/psm_constants.rs::tests::the_xtypes_policy_names_and_representation_ids_are_those_of_annex_d`

**Status:** done — `psm_constants::qos_policy_name` liefert `DataRepresentation` und `TypeConsistencyEnforcement`; `psm_constants::data_representation_id` gibt `XCDR` 0, `XML` 1 und `XCDR2` 2.

### Annex D DataRepresentationQosPolicy { DataRepresentationIdSeq value; }

**Spec:** Annex D, S. 328 — "@extensibility(APPENDABLE) @nested struct
DataRepresentationQosPolicy { DataRepresentationIdSeq value; };"

**Repo:** `crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData` (Z. 207, Feld `data_representation` Z. 274)

**Tests:** `crates/dcps/tests/sedp_data_representation_pid.rs::publication_data_roundtrip_preserves_data_representation`

**Status:** done

### Annex D TypeConsistencyKind @bit_bound(16) und TypeConsistencyEnforcementQosPolicy

**Spec:** Annex D, S. 328 — "@bit_bound(16) enum TypeConsistencyKind {
DISALLOW_TYPE_COERCION, ALLOW_TYPE_COERCION }; [...] struct
TypeConsistencyEnforcementQosPolicy { TypeConsistencyKind kind; boolean
ignore_sequence_bounds; boolean ignore_string_bounds; boolean
ignore_member_names; boolean prevent_type_widening; boolean
force_type_validation; };"

**Repo:** `crates/types/src/qos.rs::TypeConsistencyEnforcement` (Z. 52);
`crates/types/src/qos.rs::from_bytes` (Z. 148)

**Tests:** `crates/types/src/qos.rs::tests::consistency_roundtrip_default`,
`crates/types/src/qos.rs::tests::type_consistency_kind_has_the_two_values_of_annex_d`,
`crates/dcps/tests/xtypes_aware_match.rs::a_readers_type_consistency_policy_survives_its_announcement`

**Status:** done — `kind` hat zwei Oktette, dann folgen die fünf Booleans in dieser Reihenfolge; `TypeConsistencyKind` hat die zwei Werte von Annex D, ein unbekannter Wert wird abgewiesen.

### Annex D TopicBuiltinTopicData mit type_information 0x0075 und representation 0x0073

**Spec:** Annex D, S. 328–329 — "struct TopicBuiltinTopicData { @id(0x005A) @key
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

**Status:** done — Die Topic-Daten schreiben `type_information` unter `0x0075`, `representation` unter `0x0073`, `type_id` (`TypeIdV1`) unter `0x0069` und `type` (`TypeObjectV1`) unter `0x0072` und lesen sie zurück; die Publication- und Subscription-Daten tragen dieselben Felder (`type_id_v1`, `type_object_v1`). Ein Parameter, der nicht dekodiert, bleibt aus den Daten heraus (beide Member sind `@optional`).

### Annex D TypeIdV1 (0x0069) und TypeObjectV1 (0x0072) aus XTYPES 1.1

**Spec:** Annex D, S. 328 — "@id(0x0069) @optional TypeIdV1 type_id; // XTYPES 1.1" und "@id(0x0072) @optional TypeObjectV1 type; // XTYPES 1.1".

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

**Status:** done — Annex D schreibt `TypeIdV1` und `TypeObjectV1` ohne Definition in 1.3; `TypeObjectV1` ist das `TypeObject` aus Annex B von XTypes 1.1 und `TypeIdV1` dessen `_TypeId` (die einzige Kennung von 1.1). Das Modell, die XCDR1-Serialisierung (Parameterlisten für die mutablen Strukturen, einfaches CDR für die erweiterbaren; eine mutable Union als Diskriminator und gewähltes Member nach XTypes 1.3 §7.4.3.5.3 Regel (28)), die Typkennungen von 1.1 §7.3.4.1.2 (die unteren 64 Bit des MD5 über die Big-Endian-Serialisierung) und die Umwandlung von und zu Complete TypeObjects stehen in `zerodds_types::type_object_v1`. Die Entscheidungen, wo der Text offen lässt, stehen in der Modul-Dokumentation: Member-IDs der Union `TypeLibraryElement` nach Deklarationsreihenfolge, `MODULE_ELEMENT` = 24 (der Text gibt dem Enumerator keinen Wert, der nächste wäre 22 und kollidiert), die `type_id` des Typs selbst ist beim Hash `NO_TYPE`. Die Einstellung `discovery.type_object_v1` (Default aus) lässt Endpunkte und Topics beide Member beilegen; die `TypeObjectV1` eines entfernten Endpunkts ohne `PID_TYPE_INFORMATION` wird als Complete TypeObjects in die Registry gelegt und gibt dem Endpunkt seine Kennung. Was 1.1 nicht tragen kann, wird nicht mitgenommen (`try_construct`, `@must_understand` außer dem eines Keys, `@autoid(HASH)`, `@verbatim`, `@unit`, `@min`, `@max`, `@hashid`, Default-Werte, `@ignore_literal_names`) oder gibt einen benannten Fehler (`int8`, Bitset mit Bitfeldern, ein Typ, der sich selbst erreicht: seine Kennung wäre der Hash einer Serialisierung, die sie enthält).

### Annex D PublicationBuiltinTopicData mit type_information und representation

**Spec:** Annex D, S. 329–330 — "struct PublicationBuiltinTopicData { [...]
@id(0x0075) @optional XTypes::TypeInformation type_information; [...]
@id(0x0073) DataRepresentationQosPolicy representation; };"

**Repo:** `crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData` (Z. 207)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::publication_standard_type_information_yields_type_identifier`,
`crates/dcps/tests/sedp_data_representation_pid.rs::publication_data_emits_data_representation_pid_when_set`

**Status:** done

### Annex D SubscriptionBuiltinTopicData mit type_information, representation und type_consistency 0x0074

**Spec:** Annex D, S. 330–331 — "struct SubscriptionBuiltinTopicData { [...]
@id(0x0073) DataRepresentationQosPolicy representation; @id(0x0074)
TypeConsistencyEnforcementQosPolicy type_consistency; };"

**Repo:** `crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData` (Z. 37)

**Tests:** `crates/dcps/tests/xtypes_aware_match.rs::subscription_standard_type_information_yields_type_identifier`,
`crates/dcps/tests/xtypes_aware_match.rs::a_readers_type_consistency_policy_survives_its_announcement`

**Status:** done

### Annex D TopicQos, DataWriterQos, DataReaderQos mit representation und type_consistency

**Spec:** Annex D, S. 329–331 — "struct TopicQos { // ...
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

**Status:** done — `TopicQos`, `DataWriterQos` und `DataReaderQos` tragen `representation` und `DataReaderQos` `type_consistency`, in Rust, in den C-Strukturen, in der Java-PSM und in den QoS-Sätzen der C++-PSM; eine `TypeConsistencyKind`, die Annex D nicht kennt, weist `zerodds_sub_create_datareader` ab.

---

## Annex E: Built-in Types

### Annex E DDS::_String, KeyedString, Bytes, KeyedBytes als APPENDABLE

**Spec:** Annex E, S. 333–336 — "@extensibility(APPENDABLE) struct _String {
string value; }; [...] @extensibility(APPENDABLE) struct KeyedString { @key
string key; string value; }; [...] struct Bytes { ByteSeq value; }; [...] struct
KeyedBytes { @key string key; ByteSeq value; };"

**Repo:** `crates/dcps/src/builtin_type_set.rs::build` (Z. 98)

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs::dds_string_has_unbounded_string_value_member`,
`crates/dcps/tests/builtin_types_auto_register_c44b.rs::dds_keyed_bytes_has_key_member`,
`crates/dcps/src/builtin_type_set.rs::tests::each_one_is_an_appendable_non_nested_struct_under_its_spec_name`

**Status:** done — Member wie in Annex E, `@extensibility(APPENDABLE)`, nicht nested.

### Annex E Fußnote 9: führender Unterstrich bei _String

**Spec:** Annex E, Fußnote 9, S. 333 — "The leading underscore in the
declaration of the String structure is necessary to prevent collision with the
IDL keyword 'string.'"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erläuterung; der Typname ist `DDS::String`.

### Annex E StringDataWriter, StringDataReader, StringTypeSupport

**Spec:** Annex E, S. 333 — "interface StringDataWriter : DataWriter { /* This
interface shall instantiate the type FooDataWriter [...] where 'Foo' is an
unbounded string. */ };"

**Repo:** `crates/dcps/src/builtin_types.rs::StringTypeSupport`; `crates/zerodds-c-api/src/builtin_type_ffi.rs`; `crates/cpp/include/zerodds/builtin_type_support.hpp`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::StringDataWriter`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::StringDataReader`

**Tests:** `crates/dcps/tests/builtin_type_support.rs::each_type_support_names_its_type`; `crates/dcps/tests/builtin_type_support.rs::register_type_registers_each_type_under_a_name`; `crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`; `crates/zerodds-c-api/tests/builtin_types_c.rs::the_type_supports_name_and_register_their_types`; `crates/cpp/tests/builtin_data_endpoints_via_cargo.rs::the_data_writers_and_readers_of_the_built_in_types_write_and_take`; `crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java`

**Status:** done — `StringTypeSupport` mit `get_type_name` und `register_type` in Rust, C-ABI, C++ und Java; `StringDataWriter` und `StringDataReader` in Rust (Alias auf `DataWriter<…>`, `DataReader<…>`), in der C-ABI und in C++ (Klassen über der C-ABI). In Java sind die Schnittstellen nach DDS-Java-PSM 1.0 §7.8.2 `DataWriter<T>` und `DataReader<T>` über `String`, `byte[]`, `KeyedString` und `KeyedBytes`; Log in `.agent/REPORT.md`.

### Annex E KeyedStringDataWriter: *_w_key-Operationen

**Spec:** Annex E, S. 334–335 — `register_instance_w_key`,
`register_instance_w_key_w_timestamp`, `unregister_instance_w_key`,
`unregister_instance_w_key_w_timestamp`, `write_string_w_key`,
`write_string_w_key_w_timestamp`, `dispose_w_key`, `dispose_w_key_w_timestamp`,
`get_key_value_w_key`, `lookup_instance_w_key`.

**Repo:** `crates/dcps/src/builtin_types.rs::KeyedStringDataWriterOperations`

**Tests:** `crates/dcps/tests/builtin_types_writers.rs::keyed_string_operations_work_on_the_key`

**Status:** done — Die Operationen von Annex E stehen im Trait `KeyedStringDataWriterOperations` für `DataWriter<…>` (`*_w_key`, jeweils mit `_w_timestamp`).

### Annex E KeyedStringDataReader, KeyedStringTypeSupport

**Spec:** Annex E, S. 335 — "interface KeyedStringDataReader : DataReader {
[...] }; interface KeyedStringTypeSupport : TypeSupport { [...] };"

**Repo:** `crates/dcps/src/builtin_types.rs::KeyedStringTypeSupport`; `crates/zerodds-c-api/src/builtin_type_ffi.rs`; `crates/cpp/include/zerodds/builtin_type_support.hpp`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::KeyedStringDataWriter`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::KeyedStringDataReader`

**Tests:** `crates/dcps/tests/builtin_type_support.rs::each_type_support_names_its_type`; `crates/dcps/tests/builtin_type_support.rs::register_type_registers_each_type_under_a_name`; `crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`; `crates/zerodds-c-api/tests/builtin_types_c.rs::the_type_supports_name_and_register_their_types`; `crates/cpp/tests/builtin_data_endpoints_via_cargo.rs::the_data_writers_and_readers_of_the_built_in_types_write_and_take`; `crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java`

**Status:** done — `KeyedStringTypeSupport` mit `get_type_name` und `register_type` in Rust, C-ABI, C++ und Java; `KeyedStringDataWriter` und `KeyedStringDataReader` in Rust (Alias auf `DataWriter<…>`, `DataReader<…>`), in der C-ABI und in C++ (Klassen über der C-ABI). In Java sind die Schnittstellen nach DDS-Java-PSM 1.0 §7.8.2 `DataWriter<T>` und `DataReader<T>` über `String`, `byte[]`, `KeyedString` und `KeyedBytes`; Log in `.agent/REPORT.md`.

### Annex E BytesDataWriter: write_w_bytes, write_w_bytes_w_timestamp

**Spec:** Annex E, S. 335–336 — "ReturnCode_t write_w_bytes(in ByteArray bytes,
in long offset, in long length, in InstanceHandle_t handle); ReturnCode_t
write_w_bytes_w_timestamp(...)".

**Repo:** `crates/dcps/src/builtin_types.rs::BytesDataWriterOperations`

**Tests:** `crates/dcps/tests/builtin_types_writers.rs::bytes_are_written_from_an_offset_and_a_length`

**Status:** done — Die Operationen von Annex E stehen im Trait `BytesDataWriterOperations` für `DataWriter<…>` (`*_w_key`, jeweils mit `_w_timestamp`).

### Annex E BytesDataReader, BytesTypeSupport

**Spec:** Annex E, S. 336 — "interface BytesDataReader : DataReader { [...] };
interface BytesTypeSupport : TypeSupport { [...] };"

**Repo:** `crates/dcps/src/builtin_types.rs::BytesTypeSupport`; `crates/zerodds-c-api/src/builtin_type_ffi.rs`; `crates/cpp/include/zerodds/builtin_type_support.hpp`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::BytesDataWriter`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::BytesDataReader`

**Tests:** `crates/dcps/tests/builtin_type_support.rs::each_type_support_names_its_type`; `crates/dcps/tests/builtin_type_support.rs::register_type_registers_each_type_under_a_name`; `crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`; `crates/zerodds-c-api/tests/builtin_types_c.rs::the_type_supports_name_and_register_their_types`; `crates/cpp/tests/builtin_data_endpoints_via_cargo.rs::the_data_writers_and_readers_of_the_built_in_types_write_and_take`; `crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java`

**Status:** done — `BytesTypeSupport` mit `get_type_name` und `register_type` in Rust, C-ABI, C++ und Java; `BytesDataWriter` und `BytesDataReader` in Rust (Alias auf `DataWriter<…>`, `DataReader<…>`), in der C-ABI und in C++ (Klassen über der C-ABI). In Java sind die Schnittstellen nach DDS-Java-PSM 1.0 §7.8.2 `DataWriter<T>` und `DataReader<T>` über `String`, `byte[]`, `KeyedString` und `KeyedBytes`; Log in `.agent/REPORT.md`.

### Annex E KeyedBytesDataWriter: *_w_key-Operationen

**Spec:** Annex E, S. 336–337 — `register_instance_w_key`,
`unregister_instance_w_key`, `write_bytes_w_key`,
`write_bytes_w_key_w_timestamp`, `dispose_w_key`, `get_key_value_w_key`,
`lookup_instance_w_key` und die `_w_timestamp`-Formen.

**Repo:** `crates/dcps/src/builtin_types.rs::KeyedBytesDataWriterOperations`

**Tests:** `crates/dcps/tests/builtin_types_writers.rs::keyed_bytes_operations_work_on_the_key`

**Status:** done — Die Operationen von Annex E stehen im Trait `KeyedBytesDataWriterOperations` für `DataWriter<…>` (`*_w_key`, jeweils mit `_w_timestamp`).

### Annex E KeyedBytesDataReader, KeyedBytesTypeSupport

**Spec:** Annex E, S. 337–338 — "interface KeyedBytesDataReader : DataReader {
[...] }; interface KeyedBytesTypeSupport : TypeSupport { [...] };"

**Repo:** `crates/dcps/src/builtin_types.rs::KeyedBytesTypeSupport`; `crates/zerodds-c-api/src/builtin_type_ffi.rs`; `crates/cpp/include/zerodds/builtin_type_support.hpp`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::KeyedBytesDataWriter`; `crates/cpp/include/zerodds/builtin_data_endpoints.hpp::DDS::KeyedBytesDataReader`

**Tests:** `crates/dcps/tests/builtin_type_support.rs::each_type_support_names_its_type`; `crates/dcps/tests/builtin_type_support.rs::register_type_registers_each_type_under_a_name`; `crates/dcps/tests/builtin_types_writers.rs::the_four_types_are_topic_types_with_a_string_writer`; `crates/zerodds-c-api/tests/builtin_types_c.rs::the_type_supports_name_and_register_their_types`; `crates/cpp/tests/builtin_data_endpoints_via_cargo.rs::the_data_writers_and_readers_of_the_built_in_types_write_and_take`; `crates/java-omgdds/java/src/test/java/org/omg/dds/TypesBuiltinTest.java`

**Status:** done — `KeyedBytesTypeSupport` mit `get_type_name` und `register_type` in Rust, C-ABI, C++ und Java; `KeyedBytesDataWriter` und `KeyedBytesDataReader` in Rust (Alias auf `DataWriter<…>`, `DataReader<…>`), in der C-ABI und in C++ (Klassen über der C-ABI). In Java sind die Schnittstellen nach DDS-Java-PSM 1.0 §7.8.2 `DataWriter<T>` und `DataReader<T>` über `String`, `byte[]`, `KeyedString` und `KeyedBytes`; Log in `.agent/REPORT.md`.

---

## Annex F: Characterizing Legacy DDS Implementations

### Annex F Einleitung: nicht normativ

**Spec:** Annex F, S. 339 — "This annex provides a non-normative description of
what is believed to be the consensus Type System [...] of DDS implementations
that do not conform to this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — „non-normative“.

### Annex F.1 Type System

**Spec:** Annex F.1, S. 339 — "The following portions of the Type System are
believed to be supported by the majority of DDS implementations".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — nicht normativ.

### Annex F.2 Type Representation

**Spec:** Annex F.2, S. 339 — "The IDL Type Representations of those portions of
the Type System enumerated above are generally supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — nicht normativ.

### Annex F.3 Data Representation

**Spec:** Annex F.3, S. 340 — "The exception is the extended parameter ID and
length facility based on PID_EXTENDED, which is not generally supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — nicht normativ.

### Annex F.4 Language Binding

**Spec:** Annex F.4, S. 340 — "The Plain Language Bindings of those portions of
the Type System enumerated above are generally supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — nicht normativ.

---

## Audit-Status

1105 done / 0 partial / 0 open / 93 n/a (informative) / 0 n/a (rejected).

Test-Lauf: wird auf dem Linux-Testhost ergänzt.

