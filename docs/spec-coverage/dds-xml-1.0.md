# DDS Consolidated XML Syntax 1.0 — Spec-Coverage

**Spec:** [OMG DDS-XML 1.0](https://www.omg.org/spec/DDS-XML/1.0/PDF) (33 Seiten, OMG formal/24-04-04)

Implementation:

- `crates/xml/` — DDS-XML-Konfigurationssyntax + Loader.

Audit Item-für-Item
gegen die Spec; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

---

## §1 Scope

### 1.1 Konsolidierung existierender XML-Specs

**Spec:** §1, S. 1 — "Historically various specifications have defined
XML syntax to represent particular subsets of DDS-related resources:
[DDS4CCM]/QoS, [DDS-XTYPES]/Types+Samples, [DDS-WEB]/Apps+Domains+Entities.
This specification consolidates all this XML syntax into a single document."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Position-Statement der Spec
über die eigene Konsolidierungs-Rolle.

### 1.2 Keine syntaktischen Aenderungen vs. Referenzen

**Spec:** §1, S. 1 — "There are no significant syntactic changes in
this document relative to referenced specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Position-Statement.

---

## §2 Conformance Criteria

### 2.1 Keine eigenständigen Conformance-Punkte

**Spec:** §2, S. 1 — "This document contains no independent conformance
points. Rather, it defines XML Schemas to be used to describe DDS
resources such that they can be referenced by other specifications
leaving the definition of conformance criteria to the referencing
specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-Aussage über das
eigene Conformance-Modell ("delegated to referencing specs").

### 2.2 Future-Specs SHALL referenzieren diese Spec

**Spec:** §2, S. 1 — "Future specifications that describe DDS resources
in XML shall reference this specification or a future revision thereof."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — bindet das OMG-Spec-Komitee,
keine Repo-Anforderung.

### 2.3 Future-Revisions current Specs SHOULD referenzieren

**Spec:** §2, S. 1 — "Future revisions of current specifications that
describe DDS resources in XML should reference this specification or
a future revision thereof."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — bindet das OMG-Spec-Komitee.

### 2.4 Atomic Building-Block-Selection

**Spec:** §2, S. 1 — "Reference to this standard shall result in a
selection of building blocks where all selected building blocks shall
be supported entirely."

**Repo:** `crates/xml/src/conformance.rs::SUPPORTED_BUILDING_BLOCKS`
— atomare Liste der 6 unterstützten Building-Blocks
(QoS/Types/Domains/DomainParticipants/Applications/DataSamples) mit
Modul-Verweis + Top-Level-Element. Module:
`crates/xml/src/{qos,xtypes_def,domain,participant,application,sample}.rs`.

**Tests:** `conformance::tests::supported_blocks_match_spec_count`
(verifiziert 6 Blocks), `supported_blocks_have_unique_modules`,
`supported_blocks_have_unique_root_elements`.

**Status:** done — Conformance-Marker pro Block ist exponiert; die
3 Tabelle-Tests stellen Vollständigkeit + Eindeutigkeit sicher.

---

## §3 Normative References

### 3.1 [XML] Extensible Markup Language 1.0 Fifth Edition

**Spec:** §3, S. 1 — "[XML] Extensible Markup Language (XML) 1.0
(Fifth Edition) Specification."

**Repo:** `crates/xml/Cargo.toml` — `quick-xml`-Dependency liefert
W3C-XML-1.0-konformen Parser.

**Tests:** `crates/xml/src/parser.rs::tests::parse_with_xml_declaration`,
`comments_stripped`, `attributes_preserved`, `nested_structure`.

**Status:** done

### 3.2 [XSD-1] XML Schema Definition Language 1.1 Part 1: Structures

**Spec:** §3, S. 1 — "[XSD-1] XML Schema Definition Language (XSD)
1.1 Part 1: Structures."

**Repo:** `crates/xml/src/xsd_loader.rs` — XSD-Loader (data:/file:-
URI, Base64) + `crates/xml/src/schemas.rs` — embedded normative
XSD-Schemas (Common-Datatypes + 6 Building-Blocks + DDSSystem,
chameleon + namespaced) als `include_str!`. XSD-1.1-Strukturen
(`xs:complexType`, `xs:element`, `xs:attribute`, `xs:sequence`,
`xs:all`, `xs:any`, `xs:include`) werden im DDS-XML-Footprint
verwendet und durch `roxmltree::Document::parse` strukturell
geprüft.

**Tests:** `xsd_loader::tests::data_uri_plain_loads`,
`data_uri_base64_loads`, `file_uri_loads_existing_file`,
`unsupported_uri_scheme_rejected` +
`schemas::tests::nonamespace_xsds_can_be_loaded_by_xsd_loader`,
`namespaced_xsds_can_be_loaded_by_xsd_loader` (alle 14 embedded
XSD-Files parsen syntaktisch wohlgeformt).

**Status:** done — XSD-1.1-Subset (alle in DDS-XML 1.0 §7-§8
verwendeten Strukturen) deckt unseren Footprint vollständig ab.

### 3.3 [XSD-2] XML Schema Definition Language 1.1 Part 2: Datatypes

**Spec:** §3, S. 1 — "[XSD-2] XML Schema Definition Language (XSD)
1.1 Part 2: Datatypes."

**Repo:** XSD-1.1-Datatypes via `crates/xml/schemas/dds-xml_common.xsd`
(`boolean`, `nonNegativeInteger_UNLIMITED`, `positiveInteger_UNLIMITED`,
`nonNegativeInteger_Duration_SEC/_NSEC`, `duration`, `octetSequence`)
+ Custom-Datatype-Parser in `crates/xml/src/types.rs` und
`crates/xml/src/qos_parser.rs` für Tab.7.1+7.2.

**Tests:** `types::tests::*` (41 Tests):
`bool_false_variants`, `bool_invalid`, `bool_true_variants`,
`duration_constants`, `duration_nsec_infinite`,
`duration_nsec_normal`, `duration_nsec_out_of_range`,
`duration_sec_infinite_symbols`, `duration_sec_normal`,
`duration_sec_overflow`, `enum_match`, `enum_no_match`,
`long_decimal`, `long_hex`, `long_invalid`,
`long_length_unlimited_symbol`, `octet_sequence_decimal_basic`,
`octet_sequence_double_comma_rejected`,
`octet_sequence_empty_string_returns_empty_vec`,
`octet_sequence_hex_above_255_rejected`,
`octet_sequence_hex_basic`,
`octet_sequence_mixed_decimal_and_hex`,
`octet_sequence_negative_rejected`,
`octet_sequence_non_numeric_token_rejected`,
`octet_sequence_trailing_comma_rejected`,
`octet_sequence_value_above_255_rejected`,
`octet_sequence_whitespace_around_commas`,
`positive_unlimited_empty_rejected`,
`positive_unlimited_hex_rejected`,
`positive_unlimited_leading_zero_rejected`,
`positive_unlimited_negative_rejected`,
`positive_unlimited_one_to_max_passes`,
`positive_unlimited_overflow_rejected`,
`positive_unlimited_symbol_passes`,
`positive_unlimited_zero_rejected`, `spec_constants`,
`string_short_passes`, `string_too_long_rejected`,
`time_invalid_constants`, `ulong_decimal_and_hex`, `ulong_invalid`.
Plus `schemas::tests::namespaced_xsds_can_be_loaded_by_xsd_loader`.

**Status:** done — alle in DDS-XML 1.0 §7.1.4 Tab.7.1 + §7.2.2.x
gelisteten Datatypes haben einen Parser + XSD-1.1-konforme
SimpleType-Definition.

### 3.4 [DDS4CCM] (informativ)

**Spec:** §3, S. 1 — "[DDS4CCM] DDS for Lightweight CCM (DDS4CCM),
Version 1.1." Markiert als "input to this specification".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec selbst markiert als
informativ.

### 3.5 [DDS-XTYPES] (informativ)

**Spec:** §3, S. 1 — "[DDS-XTYPES] Extensible and Dynamic Topic Types
for DDS, Version 1.2."

**Repo:** `crates/types/`, `crates/xml/src/xtypes_def.rs`,
`crates/xml/src/xtypes_parser.rs`.

**Tests:** siehe `dds-xtypes-1.3.md`.

**Status:** `n/a (informative)` — Cross-Spec-Linkage; Coverage
in `dds-xtypes-1.3.md`.

### 3.6 [DDS-WEB] (informativ)

**Spec:** §3, S. 1 — "[DDS-WEB] Web-Enabled DDS, Version 1.0."

**Repo:** `crates/web/`.

**Tests:** siehe `zerodds-web-1.0.md`.

**Status:** `n/a (informative)` — Cross-Spec-Linkage; Coverage
in `zerodds-web-1.0.md`.

### 3.7 [DDS] (informativ)

**Spec:** §3, S. 2 — "[DDS] Data Distribution Service, Version 1.4."

**Repo:** `crates/dcps/` — DDS 1.4 Implementation.

**Tests:** siehe `zerodds-dcps-1.4.md`.

**Status:** `n/a (informative)` — Cross-Spec-Linkage.

### 3.8 [IDL] (informativ)

**Spec:** §3, S. 2 — "[IDL] Interface Definition Language (IDL),
Version 4.2."

**Repo:** `crates/idl/src/grammar/idl42.rs` — IDL-4.2-Parser.

**Tests:** siehe `idl-4.2.md`.

**Status:** `n/a (informative)` — Cross-Spec-Linkage.

---

## §4 Terms and Definitions

### 4.1 building block

**Spec:** §4, S. 2 — "A building block is a consistent set of XML
schemas that together can be used to describe the syntax of XML
documents that represent a set of DDS resources. Building blocks are
atomic, which means that if selected they must be totally supported."

**Repo:** `crates/xml/src/{qos,types,domain,participant,application,sample}.rs`
— Sechs Module entsprechen den sechs BBs in §7.3.

**Tests:** siehe Tests pro BB unten.

**Status:** done

### 4.2 building block set

**Spec:** §4, S. 2 — "A building block set is a selection of building
blocks that determines a specific XSD schema usage. Building block
sets are described in clause 8."

**Repo:** `crates/xml/src/zerodds_xml.rs` — `DdsXml`-Struct vereint die
sechs BB-Module zu einem System-Set (§8.1).

**Tests:** `crates/xml/src/zerodds_xml.rs::tests::parse_empty_dds`,
`parse_mixed_top_level`, `non_dds_root_rejected`.

**Status:** done

---

## §5 Symbols

### 5.1 Akronym-Tabelle Tab.5.1

**Spec:** §5, Tab.5.1, S. 2 — Acronyms: DDS, ISO, LwCCM, OMG, QoS,
UTF, XML, XSD, XTypes.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Akronym-Tabelle.

---

## §6 Additional Information

### 6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, S. 3 — "This specification does not change any
adopted OMG specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial-Statement.

### 6.2 Acknowledgments

**Spec:** §6.2, S. 3 — "Real-Time Innovations, Inc.; Twin Oaks
Computing, Inc.; Jackrabbit Consulting."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Acknowledgments.

---

## §7.1 XML Representation Syntax

### 7.1.1.1 Well-formed XML nach §2.1 von [XML]

**Spec:** §7.1.1, S. 5 — "It shall be a well-formed XML document
according to the criteria defined in clause 2.1 of [XML]."

**Repo:** `crates/xml/src/parser.rs::parse_xml_tree` — quick-xml
Parser; rejected mal-formed XML.

**Tests:** `crates/xml/src/parser.rs::tests::invalid_xml_rejected`,
`empty_rejected`, `dtd_rejected`, `parse_minimal_document`.

**Status:** done

### 7.1.1.2 UTF-8 Character Encoding

**Spec:** §7.1.1, S. 5 — "It shall use UTF-8 character encoding for
XML elements and values."

**Repo:** `crates/xml/src/parser.rs` — quick-xml liefert UTF-8-Strings;
keine separate Encoding-Detection.

**Tests:** `crates/xml/src/parser.rs::tests::parse_with_xml_declaration`
(deklariert `encoding="UTF-8"`).

**Status:** done

### 7.1.2 XML Schema Definition Files

**Spec:** §7.1.2, S. 5 — "This specification makes use of XML Schema
Definition (XSD) language specified in [XSD-1] and [XSD-2] to
represent the syntax of the different building blocks. Each building
block contains two normative XSD files."

**Repo:** Alle 14 normativen XSD-Files in
`crates/xml/schemas/`: `dds-xml_common.xsd` +
`dds-xml_<bb>_definitions[_nonamespace].xsd` für 6 Building-Blocks
(QoS/Types/Domains/DomainParticipants/Applications/DataSamples) +
`dds-xml_dds_system_definitions[_nonamespace].xsd`. Embedded via
`include_str!` in `crates/xml/src/schemas.rs`. Loader weiterhin in
`crates/xml/src/xsd_loader.rs`.

**Tests:** `schemas::tests::*` (7 Tests:
all_schemas_includes_six_building_blocks_plus_system,
nonamespace_xsds_omit_target_namespace,
namespaced_xsds_include_target_namespace,
namespaced_xsds_define_top_level_element, +
nonamespace/namespaced_can_be_loaded_by_xsd_loader,
common_xsd_starts_with_xml_declaration) +
`xsd_loader::tests::*`.

**Status:** done — alle 14 normativen XSD-Files sind embedded;
strukturelle Tests verifizieren targetNamespace + Top-Level-Elemente
pro Block.

### 7.1.3 XML Chameleon Schema Definition Pattern (non-normativ)

**Spec:** §7.1.3, S. 5 — Erklärung des Chameleon-Pattern (XSD ohne
`targetNamespace` als wiederverwendbarer Bestandteil). Section ist
explizit "(non-normative)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec markiert §7.1.3 explizit als "(non-normative)"; Chameleon-Pattern ist Erklärung der XSD-Wiederverwendung, keine Implementierungs-Anforderung.

---

## §7.1.4 XML Element Values (Tab.7.1)

### 7.1.4.1 Element-Wert `boolean` case-sensitive

**Spec:** §7.1.4 Tab.7.1, S. 7 — "boolean: Yes: 1 or true. No: 0 or
false. Values are case sensitive."

**Repo:** `crates/xml/src/qos_parser.rs::parse_bool_strict` —
akzeptiert nur lowercase `true`/`false`/`1`/`0`.

**Tests:** `crates/xml/src/qos_parser.rs::tests::parse_bool_strict_accepts_only_lowercase`,
`boolean_case_sensitive_rejected`, `bool_true_variants`,
`bool_false_variants`, `bool_invalid`.

**Status:** done

### 7.1.4.2 Element-Wert `enum` als String aus DCPS-IDL

**Spec:** §7.1.4 Tab.7.1, S. 7 — "enum: A string. Legal values are
the ones defined for QoS Policies in the DCPS IDL of DDS specification
[DDS]. Must be specified as a string. (Do not use numeric values.)"

**Repo:** `crates/xml/src/qos_parser.rs` — Reliability/History/
Durability-Kind-Parser akzeptieren Spec-Strings (`RELIABLE_RELIABILITY`
etc.).

**Tests:** `qos_parser::tests::parse_reliability_and_history`,
`enum_match`, `enum_no_match`.

**Status:** done

### 7.1.4.3 Element-Wert `long` (32-bit signed) + LENGTH_UNLIMITED

**Spec:** §7.1.4 Tab.7.1, S. 7 — "long: -2147483648 to 2147483647 or
0x80000000 to 0x7fffffff or LENGTH_UNLIMITED. A 32-bit signed integer."

**Repo:** `crates/xml/src/qos_parser.rs` — Decimal/Hex-Parser plus
LENGTH_UNLIMITED-Symbol.

**Tests:** `qos_parser::tests::long_decimal`, `long_hex`,
`long_invalid`, `long_length_unlimited_symbol`.

**Status:** done

### 7.1.4.4 Element-Wert `unsigned long` (32-bit unsigned)

**Spec:** §7.1.4 Tab.7.1, S. 7 — "unsigned long: 0 to 4294967296 or
0 to 0xffffffff. A 32-bit unsigned integer."

**Repo:** `crates/xml/src/qos_parser.rs` — Decimal+Hex-Parser für
ulong.

**Tests:** `qos_parser::tests::ulong_decimal_and_hex`, `ulong_invalid`.

**Status:** done

### 7.1.4.5 Element-Wert `string` mit XML-Escaping

**Spec:** §7.1.4 Tab.7.1, S. 7 — "string: The string with the
reserved XML characters escaped according to the standard rules for
element content [XML]. Per the XML rules only `<` and `&` are required
to be escaped within an element content. The characters `>`, `'`, and
`"` may be escaped."

**Repo:** quick-xml-Library macht XML-Entity-Decoding (`&lt;`, `&amp;`,
`&gt;`, `&apos;`, `&quot;`).

**Tests:** `qos_parser::tests::string_short_passes`,
`string_too_long_rejected`.

**Status:** done

---

## §7.1.5 XML Attribute Values (Tab.7.2)

### 7.1.5.1 Attribute-Wert `boolean` case-sensitive

**Spec:** §7.1.5 Tab.7.2, S. 8 — Identisch zu §7.1.4.1 für Attribute.

**Repo:** `crates/xml/src/parser.rs::XmlElement::attribute` liefert
String; `qos_parser` reuse `parse_bool_strict` für Attribute.

**Tests:** dieselben Boolean-Tests wie §7.1.4.1.

**Status:** done

### 7.1.5.2 Attribute-Wert `enum`

**Spec:** §7.1.5 Tab.7.2, S. 8 — Identisch zu §7.1.4.2.

**Repo:** Kein QoS-Enum-Attribute im Spec-Vocabulary; greift bei
type_ref/register_type-Attributen mit String-Values.

**Tests:** `qos.rs::tests::topic_filter_inherited_and_overridden`.

**Status:** done

### 7.1.5.3 Attribute-Wert `long` + LENGTH_UNLIMITED

**Spec:** §7.1.5 Tab.7.2, S. 8 — Identisch zu §7.1.4.3.

**Repo:** `qos_parser` Long-Parser, vom Attribut-Pfad geteilt.

**Tests:** dieselben Long-Tests wie §7.1.4.3.

**Status:** done

### 7.1.5.4 Attribute-Wert `unsigned long`

**Spec:** §7.1.5 Tab.7.2, S. 8 — Identisch zu §7.1.4.4.

**Repo:** `domain_id`-Attribut wird als ulong geparsed in
`crates/xml/src/domain.rs`.

**Tests:** `qos_parser::tests::ulong_decimal_and_hex`,
`domain.rs::tests::domain_id_out_of_range`.

**Status:** done

### 7.1.5.5 Attribute-Wert `string`

**Spec:** §7.1.5 Tab.7.2, S. 8 — Identisch zu §7.1.4.5.

**Repo:** `parser.rs::XmlElement::attribute` liefert dekodierte
Attribute (XML-Entities decoded).

**Tests:** `parser.rs::tests::attributes_preserved`.

**Status:** done

---

## §7.2 XML Representation of Resources Defined in the DDS IDL PSM

### 7.2.0 1-zu-1-Mapping IDL-Datentypen

**Spec:** §7.2, S. 8 — "The XML representation of resources that
correspond to data-types defined in the DDS IDL PSM [DDS] is obtained
by performing a 1-to-1 mapping of the corresponding IDL data type."

**Repo:** `crates/xml/src/conformance.rs::IDL_TO_XML_MAPPING` —
explizite Tabelle mit 17 Einträgen (boolean/long/ulong/string/enum/
LENGTH_UNLIMITED/DURATION_INFINITE_*/Duration_ZERO_*/non/
positiveInteger_UNLIMITED/Duration_SEC/_NSEC/struct/sequence<T>/
sequence<octet>/T[N]/Duration_t) jeweils mit Spec-Sektion + Repo-
Pfad. QoS-Policy-Mapping in `qos.rs::EntityQos::into_writer_qos` /
`into_reader_qos`.

**Tests:** `conformance::tests::idl_mapping_covers_required_categories`
(Pflicht-Kategorien aus §7.1.4 + §7.2.x), `idl_mapping_entries_unique`
(keine Duplikate), `idl_mapping_includes_section_7_2_x_items` (alle
§7.2.x-Items live), plus `qos.rs::tests::entity_qos_into_*_uses_*`
für QoS-Mapping.

**Status:** done — Mapping-Tabelle deckt alle in DDS-XML 1.0 §7.2
gelisteten IDL-Datentyp-Kategorien ab.

### 7.2.1 IDL Enumeration -> XSD simpleType

**Spec:** §7.2.1, S. 8 — "IDL Enumerations are represented in XML
according to a schema defined as an XSD simpleType defined as a
restriction of a string that can take values of the enumeration
literals." Beispiel `historyKind` mit `KEEP_LAST_HISTORY_QOS`/
`KEEP_ALL_HISTORY_QOS` (non-normativ).

**Repo:** `crates/xml/src/qos_parser.rs` — Enum-Parsing für
HistoryKind/ReliabilityKind/DurabilityKind als String-Restriktionen.

**Tests:** `qos_parser::tests::parse_reliability_and_history`,
`enum_match`.

**Status:** done

---

## §7.2.2 XML Representation of Primitive Constants

### 7.2.2.1 Konstante `LENGTH_UNLIMITED = -1`

**Spec:** §7.2.2.1, S. 8 — "const long LENGTH_UNLIMITED = -1;"

**Repo:** `crates/xml/src/qos_parser.rs` (Import von
`LENGTH_UNLIMITED`); Symbol-String wird in long-Parser akzeptiert.

**Tests:** `qos_parser::tests::long_length_unlimited_symbol`.

**Status:** done

### 7.2.2.2 Konstante `DURATION_INFINITE_SEC = 0x7fffffff`

**Spec:** §7.2.2.1, S. 8 — "const long DURATION_INFINITE_SEC =
0x7fffffff;"

**Repo:** `crates/xml/src/qos_parser.rs` (Import von
`DURATION_INFINITE_SEC`).

**Tests:** `qos_parser::tests::duration_sec_infinite_symbols`,
`duration_sec_normal`, `duration_sec_overflow`.

**Status:** done

### 7.2.2.3 Konstante `DURATION_INFINITE_NSEC = 0x7fffffff`

**Spec:** §7.2.2.1, S. 8 — "const unsigned long DURATION_INFINITE_NSEC
= 0x7fffffff;"

**Repo:** `qos_parser.rs` (Import).

**Tests:** `qos_parser::tests::duration_nsec_infinite`,
`duration_nsec_normal`, `duration_nsec_out_of_range`.

**Status:** done

### 7.2.2.4 Konstante `DURATION_ZERO_SEC = 0`

**Spec:** §7.2.2.1, S. 8 — "const long DURATION_ZERO_SEC = 0;"

**Repo:** `qos_parser.rs` — Default-Pfad ohne Symbol.

**Tests:** `qos.rs::tests::duration_constants`.

**Status:** done

### 7.2.2.5 Konstante `DURATION_ZERO_NSEC = 0`

**Spec:** §7.2.2.1, S. 8 — "const unsigned long DURATION_ZERO_NSEC
= 0;"

**Repo:** `qos_parser.rs` — Default-Pfad.

**Tests:** `qos.rs::tests::duration_constants`.

**Status:** done

### 7.2.2.6 Konstante `TIME_INVALID_SEC = -1`

**Spec:** §7.2.2.1, S. 8 — "const long TIME_INVALID_SEC = -1;"

**Repo:** `crates/xml/src/types.rs::TIME_INVALID_SEC` — Konstante
exportiert via `crates/xml/src/lib.rs`.

**Tests:** `types::tests::time_invalid_constants` (verifiziert Wert
+ Unterscheidung zu DURATION_INFINITE/DURATION_ZERO).

**Status:** done

### 7.2.2.7 Konstante `TIME_INVALID_NSEC = 0xffffffff`

**Spec:** §7.2.2.1, S. 8 — "const unsigned long TIME_INVALID_NSEC =
0xffffffff;"

**Repo:** `crates/xml/src/types.rs::TIME_INVALID_NSEC` — Konstante
exportiert via `crates/xml/src/lib.rs`.

**Tests:** `types::tests::time_invalid_constants`.

**Status:** done

### 7.2.2.8 simpleType `nonNegativeInteger_UNLIMITED`

**Spec:** §7.2.2.1, S. 8 — Pattern `(LENGTH_UNLIMITED|([0-9])*)?`.

**Repo:** Implementiert als kombinierter Parser im qos_parser
(Number-or-Symbol).

**Tests:** `qos_parser::tests::long_length_unlimited_symbol`,
`long_decimal`.

**Status:** done

### 7.2.2.9 simpleType `positiveInteger_UNLIMITED`

**Spec:** §7.2.2.1, S. 8 — Pattern `(LENGTH_UNLIMITED|[1-9]([0-9])*)?`.

**Repo:** `crates/xml/src/types.rs::parse_positive_long_unlimited`
— eigener Parser, der das Spec-Pattern `(LENGTH_UNLIMITED|[1-9]([0-9])*)?`
durchsetzt: Wert `0`, führende Nullen, Hex und negative Werte
werden mit `XmlError::ValueOutOfRange` abgelehnt; nur
`LENGTH_UNLIMITED`-Symbol oder Dezimalwerte ab `1` passieren.

**Tests:** `types::tests::positive_unlimited_symbol_passes`,
`positive_unlimited_one_to_max_passes`,
`positive_unlimited_zero_rejected`,
`positive_unlimited_negative_rejected`,
`positive_unlimited_leading_zero_rejected`,
`positive_unlimited_hex_rejected`,
`positive_unlimited_overflow_rejected`,
`positive_unlimited_empty_rejected` (8 Tests).

**Status:** done — positive-Constraint durchgesetzt mit
positivem UND negativem Test pro Pattern-Bestandteil.

### 7.2.2.10 simpleType `nonNegativeInteger_Duration_SEC`

**Spec:** §7.2.2.1, S. 8 — Pattern `(DURATION_INFINITY|
DURATION_INFINITE_SEC|([0-9])*)?`.

**Repo:** `qos_parser.rs::parse_duration_sec` — akzeptiert beide
Symbol-Varianten.

**Tests:** `qos_parser::tests::duration_sec_infinite_symbols`.

**Status:** done

### 7.2.2.11 simpleType `nonNegativeInteger_Duration_NSEC`

**Spec:** §7.2.2.1, S. 8 — Pattern `(DURATION_INFINITY|
DURATION_INFINITE_NSEC|([0-9])*)?`.

**Repo:** `qos_parser.rs::parse_duration_nsec`.

**Tests:** `qos_parser::tests::duration_nsec_infinite`.

**Status:** done

---

## §7.2.3 XML Representation of Structure Types

### 7.2.3 IDL-Struct -> XSD complexType (default-Werte erhalten)

**Spec:** §7.2.3, S. 9 — "IDL structures are represented in XML
according to a schema defined as an XSD complexType. The fields in an
IDL structure become unordered elements of the complexType with the
field name appearing as the corresponding element name. This mapping
is applied recursively for nested structures. If the DDS specification
defines default values for the structure fields, the corresponding
XSD element definition shall provide the same default value."

**Repo:** `crates/xml/src/qos_parser.rs` — Reliability/History/
Resource-Limits werden mit Default-Konstanten gefüllt wenn Element
fehlt; rekursive QoS-Container.

**Tests:** `qos.rs::tests::entity_qos_into_writer_uses_defaults_for_unset`,
`merge_none_does_not_clobber`.

**Status:** done

---

## §7.2.4 XML Representation of Sequences

### 7.2.4.1 Sequenz-Mapping mit `<element>`-Tag

**Spec:** §7.2.4.1, S. 10 — "The general XML representation of IDL
sequences is done following a schema defined as an XSD complexType.
The complexType contains zero or more elements named element. Nested
inside each element is the XSD schema obtained from mapping the IDL
type of the element itself."

**Repo:** `crates/xml/src/parser.rs::XmlElement::sequence_elements`
— generischer Iterator über `<element>`-Kinder, nutzbar für
beliebige IDL-Sequenzen. Existing QoS-Pfade (z.B. `<partition>`)
bleiben kompatibel; neuer Top-Level-Helper.

**Tests:** `parser::tests::sequence_elements_iterates_element_tag_children`,
`sequence_elements_skips_non_element_tagged_children`,
`sequence_elements_empty_for_zero_children`.

**Status:** done — verallgemeinerter Helper + 3 Tests (positiv +
nicht-element-skip + leere Sequenz).

### 7.2.4.2 Sequenzen von Octets (decimal/hex oder Base64)

**Spec:** §7.2.4.2, S. 11 — "Sequences of octets are represented
either as a comma-separated list of the value of each octet
represented in decimal or hexadecimal, or alternatively using Base64
binary. Differentiated by element name (`value` vs. `valueB64`)."

**Repo:** Base64-Pfad in `crates/xml/src/qos_parser.rs::base64_decode`
(für `<valueB64>`-Elemente); Decimal/Hex-Comma-Liste in
`crates/xml/src/types.rs::parse_octet_sequence` (für
`<value>`-Elemente). Caller wählt anhand des Element-Namens.

**Tests:** Base64-Pfad: `qos_parser::tests::base64_decode_basic`,
`base64_decode_with_padding`, `base64_decode_invalid_returns_none`.
Decimal/Hex-Pfad: `types::tests::octet_sequence_decimal_basic`,
`octet_sequence_hex_basic`, `octet_sequence_mixed_decimal_and_hex`,
`octet_sequence_whitespace_around_commas`,
`octet_sequence_empty_string_returns_empty_vec`,
`octet_sequence_value_above_255_rejected`,
`octet_sequence_negative_rejected`,
`octet_sequence_trailing_comma_rejected`,
`octet_sequence_double_comma_rejected`,
`octet_sequence_non_numeric_token_rejected`,
`octet_sequence_hex_above_255_rejected` (11 Tests).

**Status:** done — beide Spec-konformen Pfade
(Comma-Liste + Base64) live, Wertbereich `0..=255` durchgesetzt.

---

## §7.2.5 XML Representation of Arrays

### 7.2.5 Array-Mapping = Sequenz-Mapping

**Spec:** §7.2.5, S. 11 — "The XML representation of IDL arrays is
the same as it would be for IDL sequences of the same element type."

**Repo:** Re-use des `<element>`-Iterators
`crates/xml/src/parser.rs::XmlElement::sequence_elements` —
identische API für fixed-size Arrays + variable Sequences. QoS-
Pfade in `qos_parser` nutzen denselben Mechanismus.

**Tests:** `parser::tests::array_uses_same_element_tag_as_sequence`
— expliziter IDL-`coords_3d[3]`-Test, plus indirekt
`qos.rs::tests::*partition*` (Partition als Sequence-of-Strings).

**Status:** done — Re-use explizit dokumentiert + getestet.

---

## §7.2.6 XML Representation of Duration

### 7.2.6 Duration_t -> XSD complexType `duration` mit Symbolen

**Spec:** §7.2.6, S. 11 — "The IDL structure Duration_t is represented
in XML following the general rules for structures defined in sub
clause 7.2.3, except that the schema provides the option to use the
symbolic defined in the IDL to set the values of the intended
elements." Schema mit `<sec>` (Type `nonNegativeInteger_Duration_SEC`)
und `<nanosec>` (Type `nonNegativeInteger_Duration_NSEC`).

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration` —
verwendet `parse_duration_sec`+`parse_duration_nsec`; mappt
DURATION_INFINITY/DURATION_INFINITE_SEC/_NSEC-Symbole.

**Tests:** `qos_parser::tests::duration_inline_infinity_sentinel`,
`duration_sec_infinite_symbols`, `duration_nsec_infinite`,
`duration_sec_normal`, `duration_nsec_normal`.

**Status:** done

---

## §7.3 Building Blocks

### 7.3.1.1 Sechs Building Blocks

**Spec:** §7.3.1, S. 12 — "This specification breaks the syntax used
to represent DDS resources in XML into the six different building
blocks: Building Block QoS, Types, Domains, DomainParticipants,
Applications, Data Samples."

**Repo:** `crates/xml/src/{qos,types,domain,participant,application,sample}.rs`
— sechs Module.

**Tests:** je BB unten.

**Status:** done

### 7.3.1.2 XSD-Naming `<bb>_definitions_nonamespace.xsd` (Chameleon)

**Spec:** §7.3.1, S. 13 — "dds-xml_<building_block_name>_definitions_
nonamespace.xsd contains the type declarations for all the constructs
the building block defines. This XSD file specifies neither a
targetNamespace nor a root element."

**Repo:** Alle 7 chameleon-Files (ohne targetNamespace) in
`crates/xml/schemas/dds-xml_<bb>_definitions_nonamespace.xsd` —
QoS, Types, Domains, DomainParticipants, Applications, DataSamples,
DDSSystem.

**Tests:** `schemas::tests::nonamespace_xsds_omit_target_namespace`
(prüft pro File, dass das `<xs:schema>`-Tag KEIN
`targetNamespace`-Attribut hat).

**Status:** done

### 7.3.1.3 XSD-Naming `<bb>_definitions.xsd` mit targetNamespace

**Spec:** §7.3.1, S. 13 — "dds-xml_<building_block_name>_definitions.xsd
includes the XSD with no targetNamespace, defines the top level
element for the building block, and sets targetNamespace to
`http://www.omg.org/spec/DDS-XML`."

**Repo:** `crates/xml/src/parser.rs::DDS_XML_NAMESPACE` Konstante +
alle 7 namespaced-XSD-Files in `crates/xml/schemas/
dds-xml_<bb>_definitions.xsd` mit
`targetNamespace="http://www.omg.org/spec/DDS-XML"`.

**Tests:** `parser.rs::tests::dds_xml_namespace_constant_matches_spec`,
`strict_mode_accepts_xml_with_correct_namespace`,
`strict_mode_rejects_xml_without_namespace`,
`lax_mode_accepts_xml_without_namespace`,
`validation_mode_default_is_lax` +
`schemas::tests::namespaced_xsds_include_target_namespace`,
`namespaced_xsds_define_top_level_element`.

**Status:** done — Namespace-Konstante, Validation-Modus UND
XSD-Files alle live.

---

## §7.3.2 Building Block QoS

### 7.3.2.1 Purpose: DDS QoS in XML repräsentieren

**Spec:** §7.3.2.1, S. 13 — "This building block defines the syntax
to represent DDS QoS in XML."

**Repo:** `crates/xml/src/qos.rs`, `crates/xml/src/qos_parser.rs`,
`crates/xml/src/qos_inheritance.rs`.

**Tests:** 30+ Tests in `qos.rs` + `qos_parser.rs`.

**Status:** done

### 7.3.2.2 Keine Abhängigkeit zu anderen BBs

**Spec:** §7.3.2.2, S. 13 — "This building block has no dependencies
on other building blocks."

**Repo:** `qos.rs` importiert nur `parser.rs`+`errors.rs`+`inheritance.rs`,
keine andere BB-Module.

**Tests:** n/a (Architektur-Aussage).

**Status:** done

### 7.3.2.3 Syntax: zwei XSD-Files mit/ohne targetNamespace

**Spec:** §7.3.2.3, S. 14 — "dds-xml_qos_definitions_nonamespace.xsd"
+ "dds-xml_qos_definitions.xsd" mit `<qos_library>` als Root.

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`
+ `dds-xml_qos_definitions.xsd` (mit `<qos_library>` als Root via
`xs:element name="qos_library"`); embedded via
`schemas::QOS_NONAMESPACE_XSD` / `QOS_NAMESPACED_XSD`.
`<qos_library>`-Root wird vom Parser (`qos.rs::QosLibrary`)
akzeptiert.

**Tests:** `qos_parser::tests::parse_minimal_library`,
`missing_library_name_rejected` +
`schemas::tests::namespaced_xsds_define_top_level_element` (prüft
explizit, dass das namespaced-XSD `xs:element name="qos_library"`
enthält).

**Status:** done — beide XSD-Files embedded + Parser live.

### 7.3.2.4.1 QoS Libraries und QoS Profiles

**Spec:** §7.3.2.4.1, S. 14 — "QoS Libraries are the top level
element of the Building Block QoS. They are collections of QoS
profiles, which group a set of related QoS — usually one per entity."

**Repo:** `crates/xml/src/qos.rs::QosLibrary` mit `name`-Feld und
`profiles: Vec<QosProfile>`; `QosLibrary::profile(name)`-Lookup.

**Tests:** `qos.rs::tests::library_profile_lookup`,
`qos_profile_shape_roundtrip`, `qos_parser::parse_minimal_library`.

**Status:** done

### 7.3.2.4.2 QoS Profile Inheritance via `base_name`

**Spec:** §7.3.2.4.2, S. 14 — "A QoS Profile can inherit from other
QoS Profiles using the `base_name` XML attribute. A QoS profile can
only inherit from QoS profiles that have been defined [before] its
definition."

**Repo:** `crates/xml/src/qos.rs::QosProfile::base_name`,
`crates/xml/src/inheritance.rs::resolve_chain` mit Cycle-Detection.

**Tests:** `inheritance.rs::tests::no_inheritance`,
`three_level_chain`, `cycle_detected`, `self_cycle`,
`two_node_cycle`, `unresolved_base_name_errors`,
`deep_inheritance_cap_enforced`, `depth_cap_enforced`,
`missing_base_propagates`.
`qos_inheritance::tests::cross_library_base_name_two_segment`,
`three_level_inheritance_propagates`,
`detect_cycle_between_profiles`,
`child_inherits_parent_reliability`.

**Status:** done

### 7.3.2.4.3 QoS Profile Topic-name Filters via `topic_filter`

**Spec:** §7.3.2.4.3, S. 14 — "A QoS Profile may contain several
DataWriter, DataReader, and Topic QoS settings that are selected
based on the evaluation of a filter expression on the topic name.
The filter expression is specified via the `topic_filter` XML
attribute. If unspecified, `*` is assumed. QoS with explicit
`topic_filter` attribute is evaluated in order; takes precedence
over a QoS without filter."

**Repo:** `crates/xml/src/qos.rs::QosProfile::topic_filter`,
`crates/xml/src/qos.rs::topic_filter_matches` (POSIX-fnmatch:
`*`, `?`, exact).

**Tests:** `qos.rs::tests::glob_star_matches_all`,
`glob_prefix`, `glob_question_mark`, `glob_exact`,
`topic_filter_inherited_and_overridden`.

**Status:** done

### 7.3.2.4.4 QoS Profiles mit Single QoS

**Spec:** §7.3.2.4.4, S. 15 — "The definition of an individual QoS
is a shortcut for defining a QoS profile with a single QoS." Beispiel
`<datawriter_qos name="..."/>` äquivalent zu `<qos_profile
name="..."/><datawriter_qos>...</datawriter_qos></qos_profile>`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_single_qos_shortcut`
— erkennt `<datawriter_qos>` / `_reader` / `_topic` /
`_publisher` / `_subscriber` / `_(domain)participant_qos` direkt
unter `<qos_library>` mit `name`-Attribut und wickelt sie in einen
impliziten `QosProfile`.

**Tests:** `qos_parser::tests::single_qos_shortcut_datawriter_creates_implicit_profile`,
`single_qos_shortcut_topic_creates_implicit_profile`,
`single_qos_shortcut_without_name_is_ignored`,
`single_qos_shortcut_multiple_kinds_in_same_library` (4 Tests).

**Status:** done

---

## §7.3.3 Building Block Types

### 7.3.3.1 Purpose: DDS Types in XML

**Spec:** §7.3.3.1, S. 15 — "This building block gathers the syntax
used to represent DDS Types in XML. Additionally, it provides
capabilities that are necessary or convenient for the organization
and management of types."

**Repo:** `crates/xml/src/types.rs`, `crates/xml/src/xtypes_def.rs`,
`crates/xml/src/xtypes_parser.rs`, `crates/xml/src/typeobject_bridge.rs`.

**Tests:** 30+ Tests für Struct/Union/Enum/Bitset/Bitmask/Typedef.

**Status:** done

### 7.3.3.2 Keine Abhängigkeit zu anderen BBs

**Spec:** §7.3.3.2, S. 15 — "This building block has no dependencies
on other building blocks."

**Repo:** `types.rs` importiert nur basis-Module (parser/errors).

**Tests:** n/a.

**Status:** done

### 7.3.3.3 Syntax: zwei XSD-Files mit/ohne targetNamespace, `<types>` als Root

**Spec:** §7.3.3.3, S. 16 — "dds-xml_type_definitions_nonamespace.xsd"
+ "dds-xml_type_definitions.xsd" mit `<types>` als Root.

**Repo:** `crates/xml/schemas/dds-xml_types_definitions_nonamespace.xsd`
+ `dds-xml_types_definitions.xsd` mit `<types>` als Root; embedded
via `schemas::TYPES_NONAMESPACE_XSD` / `TYPES_NAMESPACED_XSD`.
`<types>`-Root vom Parser akzeptiert (`xtypes_parser.rs`).

**Tests:** `xtypes_parser::tests::parse_simple_struct`,
`parse_module_nested`, `module_at_top_level_is_error`,
`parse_namespace_aware` +
`schemas::tests::namespaced_xsds_define_top_level_element` (verifiziert
`xs:element name="types"`).

**Status:** done — beide XSD-Files embedded.

---

## §7.3.4 Building Block Domains

### 7.3.4.1 Purpose: DDS Domains in XML

**Spec:** §7.3.4.1, S. 16 — "This building block defines the syntax
used to represent DDS Domains in XML. Domains provide a data space
where information can be shared by reading and writing a set of
Topics, which are associated to registered data types."

**Repo:** `crates/xml/src/domain.rs::DomainEntry` mit `register_types`
und `topics`.

**Tests:** `domain.rs::tests::parse_minimal_domain_library`,
`parse_domain_with_topic`.

**Status:** done

### 7.3.4.2 Abhängigkeiten: BB QoS + BB Types

**Spec:** §7.3.4.2, S. 16 — "This building block depends on the
Building Block QoS and the Building Block Types."

**Repo:** `domain.rs` referenziert `register_type_ref` (Types)
und nimmt inline-`<topic_qos>` an (QoS).

**Tests:** `zerodds_xml.rs::tests::resolve_*` (Cross-BB-Resolution).

**Status:** done

### 7.3.4.3 Syntax: zwei XSD-Files mit/ohne targetNamespace, `<domain_library>` als Root

**Spec:** §7.3.4.3, S. 16 — "dds-xml_domain_definitions_nonamespace.xsd"
+ "dds-xml_domain_definitions.xsd" mit `<domain_library>` als Root.

**Repo:** `crates/xml/schemas/dds-xml_domains_definitions_nonamespace.xsd`
+ `dds-xml_domains_definitions.xsd` mit `<domain_library>` als Root;
embedded via `schemas::DOMAINS_NONAMESPACE_XSD` /
`DOMAINS_NAMESPACED_XSD`. `<domain_library>`-Root vom Parser
akzeptiert.

**Tests:** `domain.rs::tests::parse_minimal_domain_library` +
`schemas::tests::namespaced_xsds_define_top_level_element`.

**Status:** done — beide XSD-Files embedded.

### 7.3.4.4.1 Defining a Domain

**Spec:** §7.3.4.4.1, S. 16 — "A Domain includes a set of Topics and
Registered Types that can be read and written. Register types shall
provide a reference to data types via `type_ref`. Topics shall refer
to a registered type via `register_type_ref`. Topics may also specify
QoS settings inline. QoS profile inheritance through `base_name`
attribute may be used."

**Repo:** `crates/xml/src/domain.rs::RegisterTypeEntry`+`TopicEntry`
mit `type_ref`/`register_type_ref`/`base_name`.

**Tests:** `domain.rs::tests::parse_domain_with_topic`,
`parse_topic_with_inline_qos`, `topic_missing_register_type_ref_rejected`.

**Status:** done

### 7.3.4.4.2 Domain Inheritance via `base_name`

**Spec:** §7.3.4.4.2, S. 17 — "A Domain can inherit from other
Domains using the `base_name` XML attribute. A Domain can only
inherit from domains that have been defined before."

**Repo:** `crates/xml/src/domain.rs::DomainEntry::base_name` (Optional<
String>) + Parser liest `base_name`-Attribut. Resolution via
`inheritance::resolve_chain` (gleicher Mechanismus wie qos_profile).

**Tests:** `domain::tests::parse_domain_with_base_name`,
`domain_inheritance_chain_resolves_via_resolve_chain`,
`domain_inheritance_cycle_detected` (3 Tests: feld-presence + 3-tier
chain A→B→C + cycle detection).

**Status:** done

---

## §7.3.5 Building Block DomainParticipants

### 7.3.5.1 Purpose

**Spec:** §7.3.5.1, S. 17 — "This block defines the syntax to
represent DDS DomainParticipants and their contained entities (i.e.,
Publishers, Subscribers, DataWriters, and DataReaders) in XML."

**Repo:** `crates/xml/src/participant.rs::DomainParticipantEntry`
mit Publishers/Subscribers; `PublisherEntry`/`SubscriberEntry` mit
DataWriters/DataReaders.

**Tests:** `participant.rs::tests::parse_minimal_participant`,
`parse_pub_with_writer`.

**Status:** done

### 7.3.5.2 Abhängigkeiten: BB QoS + BB Types + BB Domains

**Spec:** §7.3.5.2, S. 17 — "This building block depends on the
Building Block QoS, the Building Block Types, and the Building Block
Domains."

**Repo:** `participant.rs` referenziert `domain_ref` und nimmt
inline `<domain_participant_qos>` an.

**Tests:** `zerodds_xml.rs::tests::resolve_participant_*`.

**Status:** done

### 7.3.5.3 Syntax: zwei XSD-Files mit/ohne targetNamespace, `<domain_participant_library>` als Root

**Spec:** §7.3.5.3, S. 17 — "dds-xml_domainparticipant_defintions_
nonamespace.xsd" + "dds-xml_domainparticipant_definitions.xsd" mit
`<domain_participant_library>` als Root.

**Repo:** `crates/xml/schemas/dds-xml_domain_participants_definitions_nonamespace.xsd`
+ `dds-xml_domain_participants_definitions.xsd` mit
`<domain_participant_library>` als Root; embedded via
`schemas::DOMAIN_PARTICIPANTS_NONAMESPACE_XSD` /
`DOMAIN_PARTICIPANTS_NAMESPACED_XSD`. Root vom Parser akzeptiert.

**Tests:** `participant.rs::tests::parse_minimal_participant` +
`schemas::tests::namespaced_xsds_define_top_level_element`.

**Status:** done — beide XSD-Files embedded.

### 7.3.5.4.1 DomainParticipant Libraries + Hierarchy

**Spec:** §7.3.5.4.1, S. 17 — "Domain Participant Libraries are
collections of DomainParticipants and contained entities. Each
entity is declared as a nested XML tag under the declaration of its
parent entity."

**Repo:** `participant.rs::DomainParticipantLibrary` mit
`participants: Vec<DomainParticipantEntry>`; verschachtelte
Pub/Sub/DW/DR-Entries.

**Tests:** `participant.rs::tests::parse_pub_with_writer`,
`dw_missing_topic_ref_rejected`,
`missing_domain_ref_rejected`.

**Status:** done

### 7.3.5.4.2 `domain_ref` + `domain_id`-Override

**Spec:** §7.3.5.4.2, S. 18 — "DomainParticipants may refer to a
Domain declared via `domain_ref` XML attribute. The Domain Id
specified in the parent Domain can be overridden via the `domain_id`
XML attribute."

**Repo:** `participant.rs::DomainParticipantEntry::domain_ref` +
`domain_id_override`.

**Tests:** `zerodds_xml.rs::tests::resolve_participant_with_domain_id_override`,
`missing_domain_ref_rejected`.

**Status:** done

### 7.3.5.4.3 DomainParticipant Inheritance via `base_name`

**Spec:** §7.3.5.4.3, S. 18 — "DomainParticipants may inherit from
DomainParticipants defined in the context of a DomainParticipant
Library using the `base_name` XML attribute."

**Repo:** `participant.rs::DomainParticipantEntry::base_name`,
`zerodds_xml.rs::resolve_participant` walked Base-Chain.

**Tests:** `zerodds_xml.rs::tests::resolve_*` (Inheritance-Pfade).

**Status:** done

### 7.3.5.4.4 Inline Entity QoS Settings + `base_name`

**Spec:** §7.3.5.4.4, S. 18 — "Inline QoS setting definition is
allowed in the context of an entity's definition. Inline entities may
inherit from an existing QoS Profile using the `base_name` XML
attribute."

**Repo:** Inline-QoS-Felder in DataWriterEntry/DataReaderEntry +
`base_name`-Resolution via `qos_inheritance.rs`.

**Tests:** `qos_inheritance::tests::child_inherits_parent_reliability`,
`participant.rs::tests::parse_pub_with_writer`.

**Status:** done

---

## §7.3.6 Building Block Applications

### 7.3.6.1 Purpose

**Spec:** §7.3.6.1, S. 18 — "This block defines the XML syntax to
represent DDS applications that participate (or may be participating)
in the DDS Global Data Space."

**Repo:** `crates/xml/src/application.rs::ApplicationLibrary` mit
`applications: Vec<ApplicationEntry>`.

**Tests:** `application.rs::tests::parse_minimal_application`,
`missing_app_name_rejected`, `missing_dp_ref_rejected`.

**Status:** done

### 7.3.6.2 Abhängigkeiten: BB QoS + BB Types + BB Domains + BB DomainParticipants

**Spec:** §7.3.6.2, S. 18 — "This building block depends on the
Building Block QoS, the Building Block Types, the Building Block
Domains, and the Building Block DomainParticipants."

**Repo:** `application.rs` referenziert `domain_participants: Vec<String>`-
Refs (loose-coupled to DomainParticipantLibrary).

**Tests:** `zerodds_xml.rs::tests::resolve_application` walks DPLib.

**Status:** done

### 7.3.6.3 Syntax: zwei XSD-Files mit/ohne targetNamespace, `<application_library>` als Root

**Spec:** §7.3.6.3, S. 19 — "dds-xml_application_definitions_
nonamespace.xsd" + "dds-xml_application_definititons.xsd" (sic, mit
Tippfehler in Spec) mit `<application_library>` als Root.

**Repo:** `crates/xml/schemas/dds-xml_applications_definitions_nonamespace.xsd`
+ `dds-xml_applications_definitions.xsd` mit `<application_library>`
als Root; embedded via `schemas::APPLICATIONS_NONAMESPACE_XSD` /
`APPLICATIONS_NAMESPACED_XSD`. Root vom Parser akzeptiert.

**Tests:** `application.rs::tests::parse_minimal_application` +
`schemas::tests::namespaced_xsds_define_top_level_element`.

**Status:** done — beide XSD-Files embedded.

### 7.3.6.4.1 Applications + DomainParticipants + Contained Entities

**Spec:** §7.3.6.4.1, S. 19 — "Application Libraries are collections
of Applications, which are composed of a set of DomainParticipants
and contained entities."

**Repo:** `application.rs::ApplicationEntry::domain_participants:
Vec<String>` mit `<domain_participant ref="..."/>`-Children.

**Tests:** `application.rs::tests::parse_minimal_application`.

**Status:** done

### 7.3.6.4.2 DomainParticipants from DomainParticipant-Libraries via `base_name`

**Spec:** §7.3.6.4.2, S. 19 — "DomainParticipants defined in the
context of an Application may inherit from DomainParticipants defined
in the context of a DomainParticipant Library using the `base_name`
XML attribute."

**Repo:** Cross-Library-`base_name`-Resolution via `zerodds_xml.rs::resolve_*`.

**Tests:** `zerodds_xml.rs::tests::resolve_application` (resolved via
DP-Library-Lookup).

**Status:** done

---

## §7.3.7 Building Block Data Samples

### 7.3.7.1 Purpose

**Spec:** §7.3.7.1, S. 19 — "This block defines XML syntax to
represent Data Samples that may be exchanged between different DDS
applications."

**Repo:** `crates/xml/src/sample.rs` — Sample-Parsing für
Struct/Union/Sequence/Array (alle 4 SampleValue-Varianten).

**Tests:** `sample::tests::parse_simple_struct_sample`,
`parse_sample_with_union`,
`parse_sample_with_sequence_using_item_tag`,
`parse_sample_with_array_using_item_tag`,
`parse_sample_with_empty_sequence`,
`serialize_sample_uses_item_tag_for_sequence` (6 Tests).

**Status:** done

### 7.3.7.2 Keine Abhängigkeit zu anderen BBs

**Spec:** §7.3.7.2, S. 19 — "This building block has no dependencies
on other building blocks."

**Repo:** `sample.rs` importiert nur parser/errors.

**Tests:** n/a.

**Status:** done

### 7.3.7.3.1 Syntax General Rules: nutzt §7.1, §7.1.2, §7.2.2, §7.2.3, §7.2.6

**Spec:** §7.3.7.3.1, S. 20 — "the syntax to represent Data Samples
is based on the XML representation rules specified in sub clauses
7.1, 7.1.2, 7.2.2, 7.2.3, and 7.2.6."

**Repo:** `sample.rs` deleguert an `parser`+`qos_parser`-Helpers
(boolean/long/duration). Sequence/Array via `<item>`-Tag (Spec
§7.3.7.3.2 Re-use), Struct via Member-Map, Union via discriminator.

**Tests:** `sample::tests::*` (alle 7 Tests, siehe §7.3.7.1).

**Status:** done

### 7.3.7.3.2 Sequenzen/Arrays mit `<item>`-Tag (nicht `<element>` wie §7.2.4)

**Spec:** §7.3.7.3.2, S. 20 — "The general XML representation of IDL
sequences and arrays is done following a schema defined as an XSD
complexType. The complexType contains zero or more elements named
**item**. Nested inside each item is the XSD schema obtained from
mapping the IDL type of the element itself."

**Repo:** `crates/xml/src/sample.rs::parse_member_value` —
Sequence/Array iteriert via `el.children_named("item")`;
`serialize_sample` emittiert `<item>` für beide Fälle. Empty-
Sequence (`<seq></seq>`) wird via `sequence_max_length`-Hint
erkannt.

**Tests:** `sample::tests::parse_sample_with_sequence_using_item_tag`,
`parse_sample_with_array_using_item_tag`,
`parse_sample_with_empty_sequence`,
`serialize_sample_uses_item_tag_for_sequence` (4 Tests).

**Status:** done

### 7.3.7.4 Examples (non-normativ)

**Spec:** §7.3.7.4, S. 20-22 — Beispiele für Struct/Union/Sequence/
Array/Primitive-Samples. Section ist explizit "(non-normative)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec markiert §7.3.7.4 explizit als "(non-normative)"; Beispiel-Samples illustrieren das Mapping ohne neue normative Regel.

---

## §8 Building Block Sets

### 8.1.1 DDS System Block Set umfasst BBQoS+BBTypes+BBDomains+BBDPs+BBApplications

**Spec:** §8.1, S. 23 — "This block set offers the ability to
describe a complete DDS system. It contains: Building Block QoS,
Types, Domains, DomainParticipants, Applications." (BB DataSamples
NICHT enthalten.)

**Repo:** `crates/xml/src/zerodds_xml.rs::DdsXml` mit Feldern für
`qos_libraries`, `type_libraries`, `domain_libraries`,
`participant_libraries`, `application_libraries`.

**Tests:** `zerodds_xml.rs::tests::parse_empty_dds`,
`parse_mixed_top_level`, `dds_root_with_multiple_types_blocks`,
`resolve_application`.

**Status:** done

### 8.1.2 XSD-Files `dds-xml_dds_system_definitions[_nonamespace].xsd`

**Spec:** §8.1, S. 23 — "dds-xml_dds_system_definitions_nonamespace.xsd"
+ "dds-xml_dds_system_definitions.xsd" mit `<dds>` als Top-Level.

**Repo:** `<dds>`-Root von `parser.rs::parse_xml_tree` akzeptiert;
`DDS_XML_NAMESPACE` Konstante +
`crates/xml/schemas/dds-xml_dds_system_definitions_nonamespace.xsd`
+ `dds-xml_dds_system_definitions.xsd` mit `<dds>` als Top-Level
(via `xs:element name="dds"`). Embedded via
`schemas::DDS_SYSTEM_NONAMESPACE_XSD` / `DDS_SYSTEM_NAMESPACED_XSD`.

**Tests:** `parser.rs::tests::strict_mode_accepts_xml_with_correct_namespace`,
`zerodds_xml.rs::tests::non_dds_root_rejected`, `unknown_root_rejected`
+ `schemas::tests::namespaced_xsds_define_top_level_element`
(verifiziert `xs:element name="dds"`).

**Status:** done — beide XSD-Files embedded.

---

## Audit-Status

73 done / 0 partial / 0 open / 15 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-xml` — 221 lib + 23 + 10 = 254 Tests grün
  (XML-QoS-Loader + XSD-Validator).
* `cargo test -p zerodds-xml-wire` — 40 Tests grün (XML↔CDR-Codec für
  XTypes-XML-Wire-PSM, siehe `dds-xtypes-1.3.md` §2.4).
