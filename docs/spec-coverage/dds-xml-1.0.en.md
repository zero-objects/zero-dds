# DDS Consolidated XML Syntax 1.0 — Spec Coverage

**Spec:** [OMG DDS-XML 1.0](https://www.omg.org/spec/DDS-XML/1.0/PDF) (33 pages, OMG formal/24-04-04)

Implementation:

- `crates/xml/` — DDS-XML configuration syntax + loader.

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

---

## §1 Scope

### 1.1 Consolidation of existing XML specs

**Spec:** §1, p. 1 — "Historically various specifications have defined
XML syntax to represent particular subsets of DDS-related resources:
[DDS4CCM]/QoS, [DDS-XTYPES]/Types+Samples, [DDS-WEB]/Apps+Domains+Entities.
This specification consolidates all this XML syntax into a single document."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — position statement of the spec
about its own consolidation role.

### 1.2 No syntactic changes vs. references

**Spec:** §1, p. 1 — "There are no significant syntactic changes in
this document relative to referenced specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — position statement.

---

## §2 Conformance Criteria

### 2.1 No independent conformance points

**Spec:** §2, p. 1 — "This document contains no independent conformance
points. Rather, it defines XML Schemas to be used to describe DDS
resources such that they can be referenced by other specifications
leaving the definition of conformance criteria to the referencing
specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec statement about its
own conformance model ("delegated to referencing specs").

### 2.2 Future specs SHALL reference this spec

**Spec:** §2, p. 1 — "Future specifications that describe DDS resources
in XML shall reference this specification or a future revision thereof."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — binds the OMG spec committee,
no repo requirement.

### 2.3 Future revisions of current specs SHOULD reference

**Spec:** §2, p. 1 — "Future revisions of current specifications that
describe DDS resources in XML should reference this specification or
a future revision thereof."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — binds the OMG spec committee.

### 2.4 Atomic building-block selection

**Spec:** §2, p. 1 — "Reference to this standard shall result in a
selection of building blocks where all selected building blocks shall
be supported entirely."

**Repo:** `crates/xml/src/conformance.rs::SUPPORTED_BUILDING_BLOCKS`
— atomic list of the 6 supported building blocks
(QoS/Types/Domains/DomainParticipants/Applications/DataSamples) with
a module reference + top-level element. Modules:
`crates/xml/src/{qos,xtypes_def,domain,participant,application,sample}.rs`.

**Tests:** `conformance::tests::supported_blocks_match_spec_count`
(verifies 6 blocks), `supported_blocks_have_unique_modules`,
`supported_blocks_have_unique_root_elements`.

**Status:** done — a conformance marker per block is exposed; the
3 table tests ensure completeness + uniqueness.

---

## §3 Normative References

### 3.1 [XML] Extensible Markup Language 1.0 Fifth Edition

**Spec:** §3, p. 1 — "[XML] Extensible Markup Language (XML) 1.0
(Fifth Edition) Specification."

**Repo:** `crates/xml/Cargo.toml` — the `quick-xml` dependency provides
a W3C XML 1.0-conformant parser.

**Tests:** `crates/xml/src/parser.rs::tests::parse_with_xml_declaration`,
`comments_stripped`, `attributes_preserved`, `nested_structure`.

**Status:** done

### 3.2 [XSD-1] XML Schema Definition Language 1.1 Part 1: Structures

**Spec:** §3, p. 1 — "[XSD-1] XML Schema Definition Language (XSD)
1.1 Part 1: Structures."

**Repo:** `crates/xml/src/xsd_loader.rs` — XSD loader (data:/file:
URI, Base64) + `crates/xml/src/schemas.rs` — embedded normative
XSD schemas (common datatypes + 6 building blocks + DDSSystem,
chameleon + namespaced) as `include_str!`. XSD-1.1 structures
(`xs:complexType`, `xs:element`, `xs:attribute`, `xs:sequence`,
`xs:all`, `xs:any`, `xs:include`) are used in the DDS-XML footprint
and structurally checked by `roxmltree::Document::parse`.

**Tests:** `xsd_loader::tests::data_uri_plain_loads`,
`data_uri_base64_loads`, `file_uri_loads_existing_file`,
`unsupported_uri_scheme_rejected` +
`schemas::tests::nonamespace_xsds_can_be_loaded_by_xsd_loader`,
`namespaced_xsds_can_be_loaded_by_xsd_loader` (all 14 embedded
XSD files parse syntactically well-formed).

**Status:** done — the XSD-1.1 subset (all structures used in DDS-XML 1.0 §7-§8)
covers our footprint completely.

### 3.3 [XSD-2] XML Schema Definition Language 1.1 Part 2: Datatypes

**Spec:** §3, p. 1 — "[XSD-2] XML Schema Definition Language (XSD)
1.1 Part 2: Datatypes."

**Repo:** XSD-1.1 datatypes via `crates/xml/schemas/dds-xml_common.xsd`
(`boolean`, `nonNegativeInteger_UNLIMITED`, `positiveInteger_UNLIMITED`,
`nonNegativeInteger_Duration_SEC/_NSEC`, `duration`, `octetSequence`)
+ custom datatype parsers in `crates/xml/src/types.rs` and
`crates/xml/src/qos_parser.rs` for Tab.7.1+7.2.

**Tests:** `types::tests::*` (41 tests):
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

**Status:** done — all datatypes listed in DDS-XML 1.0 §7.1.4 Tab.7.1 + §7.2.2.x
have a parser + an XSD-1.1-conformant
SimpleType definition.

### 3.4 [DDS4CCM] (informative)

**Spec:** §3, p. 1 — "[DDS4CCM] DDS for Lightweight CCM (DDS4CCM),
Version 1.1." Marked as "input to this specification".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec itself marks it as
informative.

### 3.5 [DDS-XTYPES] (informative)

**Spec:** §3, p. 1 — "[DDS-XTYPES] Extensible and Dynamic Topic Types
for DDS, Version 1.2."

**Repo:** `crates/types/`, `crates/xml/src/xtypes_def.rs`,
`crates/xml/src/xtypes_parser.rs`.

**Tests:** see `dds-xtypes-1.3.md`.

**Status:** `n/a (informative)` — cross-spec linkage; coverage
in `dds-xtypes-1.3.md`.

### 3.6 [DDS-WEB] (informative)

**Spec:** §3, p. 1 — "[DDS-WEB] Web-Enabled DDS, Version 1.0."

**Repo:** `crates/web/`.

**Tests:** see `zerodds-web-1.0.md`.

**Status:** `n/a (informative)` — cross-spec linkage; coverage
in `zerodds-web-1.0.md`.

### 3.7 [DDS] (informative)

**Spec:** §3, p. 2 — "[DDS] Data Distribution Service, Version 1.4."

**Repo:** `crates/dcps/` — DDS 1.4 implementation.

**Tests:** see `zerodds-dcps-1.4.md`.

**Status:** `n/a (informative)` — cross-spec linkage.

### 3.8 [IDL] (informative)

**Spec:** §3, p. 2 — "[IDL] Interface Definition Language (IDL),
Version 4.2."

**Repo:** `crates/idl/src/grammar/idl42.rs` — IDL 4.2 parser.

**Tests:** see `idl-4.2.md`.

**Status:** `n/a (informative)` — cross-spec linkage.

---

## §4 Terms and Definitions

### 4.1 building block

**Spec:** §4, p. 2 — "A building block is a consistent set of XML
schemas that together can be used to describe the syntax of XML
documents that represent a set of DDS resources. Building blocks are
atomic, which means that if selected they must be totally supported."

**Repo:** `crates/xml/src/{qos,types,domain,participant,application,sample}.rs`
— six modules correspond to the six BBs in §7.3.

**Tests:** see tests per BB below.

**Status:** done

### 4.2 building block set

**Spec:** §4, p. 2 — "A building block set is a selection of building
blocks that determines a specific XSD schema usage. Building block
sets are described in clause 8."

**Repo:** `crates/xml/src/zerodds_xml.rs` — the `DdsXml` struct unites the
six BB modules into a system set (§8.1).

**Tests:** `crates/xml/src/zerodds_xml.rs::tests::parse_empty_dds`,
`parse_mixed_top_level`, `non_dds_root_rejected`.

**Status:** done

---

## §5 Symbols

### 5.1 Acronym table Tab.5.1

**Spec:** §5, Tab.5.1, p. 2 — Acronyms: DDS, ISO, LwCCM, OMG, QoS,
UTF, XML, XSD, XTypes.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acronym table.

---

## §6 Additional Information

### 6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, p. 3 — "This specification does not change any
adopted OMG specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — editorial statement.

### 6.2 Acknowledgments

**Spec:** §6.2, p. 3 — "Real-Time Innovations, Inc.; Twin Oaks
Computing, Inc.; Jackrabbit Consulting."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acknowledgments.

---

## §7.1 XML Representation Syntax

### 7.1.1.1 Well-formed XML per §2.1 of [XML]

**Spec:** §7.1.1, p. 5 — "It shall be a well-formed XML document
according to the criteria defined in clause 2.1 of [XML]."

**Repo:** `crates/xml/src/parser.rs::parse_xml_tree` — quick-xml
parser; rejects mal-formed XML.

**Tests:** `crates/xml/src/parser.rs::tests::invalid_xml_rejected`,
`empty_rejected`, `dtd_rejected`, `parse_minimal_document`.

**Status:** done

### 7.1.1.2 UTF-8 Character Encoding

**Spec:** §7.1.1, p. 5 — "It shall use UTF-8 character encoding for
XML elements and values."

**Repo:** `crates/xml/src/parser.rs` — quick-xml provides UTF-8 strings;
no separate encoding detection.

**Tests:** `crates/xml/src/parser.rs::tests::parse_with_xml_declaration`
(declares `encoding="UTF-8"`).

**Status:** done

### 7.1.2 XML Schema Definition Files

**Spec:** §7.1.2, p. 5 — "This specification makes use of XML Schema
Definition (XSD) language specified in [XSD-1] and [XSD-2] to
represent the syntax of the different building blocks. Each building
block contains two normative XSD files."

**Repo:** all 14 normative XSD files in
`crates/xml/schemas/`: `dds-xml_common.xsd` +
`dds-xml_<bb>_definitions[_nonamespace].xsd` for 6 building blocks
(QoS/Types/Domains/DomainParticipants/Applications/DataSamples) +
`dds-xml_dds_system_definitions[_nonamespace].xsd`. Embedded via
`include_str!` in `crates/xml/src/schemas.rs`. The loader remains in
`crates/xml/src/xsd_loader.rs`.

**Tests:** `schemas::tests::*` (7 tests:
all_schemas_includes_six_building_blocks_plus_system,
nonamespace_xsds_omit_target_namespace,
namespaced_xsds_include_target_namespace,
namespaced_xsds_define_top_level_element, +
nonamespace/namespaced_can_be_loaded_by_xsd_loader,
common_xsd_starts_with_xml_declaration) +
`xsd_loader::tests::*`.

**Status:** done — all 14 normative XSD files are embedded;
structural tests verify targetNamespace + top-level elements
per block.

### 7.1.3 XML Chameleon Schema Definition Pattern (non-normative)

**Spec:** §7.1.3, p. 5 — explanation of the chameleon pattern (XSD without
`targetNamespace` as a reusable component). The section is
explicitly "(non-normative)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec marks §7.1.3 explicitly as "(non-normative)"; the chameleon pattern is an explanation of XSD reuse, not an implementation requirement.

---

## §7.1.4 XML Element Values (Tab.7.1)

### 7.1.4.1 Element value `boolean` case-sensitive

**Spec:** §7.1.4 Tab.7.1, p. 7 — "boolean: Yes: 1 or true. No: 0 or
false. Values are case sensitive."

**Repo:** `crates/xml/src/qos_parser.rs::parse_bool_strict` —
accepts only lowercase `true`/`false`/`1`/`0`.

**Tests:** `crates/xml/src/qos_parser.rs::tests::parse_bool_strict_accepts_only_lowercase`,
`boolean_case_sensitive_rejected`, `bool_true_variants`,
`bool_false_variants`, `bool_invalid`.

**Status:** done

### 7.1.4.2 Element value `enum` as a string from DCPS IDL

**Spec:** §7.1.4 Tab.7.1, p. 7 — "enum: A string. Legal values are
the ones defined for QoS Policies in the DCPS IDL of DDS specification
[DDS]. Must be specified as a string. (Do not use numeric values.)"

**Repo:** `crates/xml/src/qos_parser.rs` — Reliability/History/
Durability-kind parsers accept the spec strings (`RELIABLE_RELIABILITY`
etc.).

**Tests:** `qos_parser::tests::parse_reliability_and_history`,
`enum_match`, `enum_no_match`.

**Status:** done

### 7.1.4.3 Element value `long` (32-bit signed) + LENGTH_UNLIMITED

**Spec:** §7.1.4 Tab.7.1, p. 7 — "long: -2147483648 to 2147483647 or
0x80000000 to 0x7fffffff or LENGTH_UNLIMITED. A 32-bit signed integer."

**Repo:** `crates/xml/src/qos_parser.rs` — decimal/hex parser plus
the LENGTH_UNLIMITED symbol.

**Tests:** `qos_parser::tests::long_decimal`, `long_hex`,
`long_invalid`, `long_length_unlimited_symbol`.

**Status:** done

### 7.1.4.4 Element value `unsigned long` (32-bit unsigned)

**Spec:** §7.1.4 Tab.7.1, p. 7 — "unsigned long: 0 to 4294967296 or
0 to 0xffffffff. A 32-bit unsigned integer."

**Repo:** `crates/xml/src/qos_parser.rs` — decimal+hex parser for
ulong.

**Tests:** `qos_parser::tests::ulong_decimal_and_hex`, `ulong_invalid`.

**Status:** done

### 7.1.4.5 Element value `string` with XML escaping

**Spec:** §7.1.4 Tab.7.1, p. 7 — "string: The string with the
reserved XML characters escaped according to the standard rules for
element content [XML]. Per the XML rules only `<` and `&` are required
to be escaped within an element content. The characters `>`, `'`, and
`"` may be escaped."

**Repo:** the quick-xml library does XML entity decoding (`&lt;`, `&amp;`,
`&gt;`, `&apos;`, `&quot;`).

**Tests:** `qos_parser::tests::string_short_passes`,
`string_too_long_rejected`.

**Status:** done

---

## §7.1.5 XML Attribute Values (Tab.7.2)

### 7.1.5.1 Attribute value `boolean` case-sensitive

**Spec:** §7.1.5 Tab.7.2, p. 8 — identical to §7.1.4.1 for attributes.

**Repo:** `crates/xml/src/parser.rs::XmlElement::attribute` returns a
string; `qos_parser` reuses `parse_bool_strict` for attributes.

**Tests:** the same boolean tests as §7.1.4.1.

**Status:** done

### 7.1.5.2 Attribute value `enum`

**Spec:** §7.1.5 Tab.7.2, p. 8 — identical to §7.1.4.2.

**Repo:** no QoS enum attribute in the spec vocabulary; applies to
type_ref/register_type attributes with string values.

**Tests:** `qos.rs::tests::topic_filter_inherited_and_overridden`.

**Status:** done

### 7.1.5.3 Attribute value `long` + LENGTH_UNLIMITED

**Spec:** §7.1.5 Tab.7.2, p. 8 — identical to §7.1.4.3.

**Repo:** `qos_parser` long parser, shared with the attribute path.

**Tests:** the same long tests as §7.1.4.3.

**Status:** done

### 7.1.5.4 Attribute value `unsigned long`

**Spec:** §7.1.5 Tab.7.2, p. 8 — identical to §7.1.4.4.

**Repo:** the `domain_id` attribute is parsed as ulong in
`crates/xml/src/domain.rs`.

**Tests:** `qos_parser::tests::ulong_decimal_and_hex`,
`domain.rs::tests::domain_id_out_of_range`.

**Status:** done

### 7.1.5.5 Attribute value `string`

**Spec:** §7.1.5 Tab.7.2, p. 8 — identical to §7.1.4.5.

**Repo:** `parser.rs::XmlElement::attribute` returns decoded
attributes (XML entities decoded).

**Tests:** `parser.rs::tests::attributes_preserved`.

**Status:** done

---

## §7.2 XML Representation of Resources Defined in the DDS IDL PSM

### 7.2.0 1-to-1 mapping of IDL data types

**Spec:** §7.2, p. 8 — "The XML representation of resources that
correspond to data-types defined in the DDS IDL PSM [DDS] is obtained
by performing a 1-to-1 mapping of the corresponding IDL data type."

**Repo:** `crates/xml/src/conformance.rs::IDL_TO_XML_MAPPING` —
explicit table with 17 entries (boolean/long/ulong/string/enum/
LENGTH_UNLIMITED/DURATION_INFINITE_*/Duration_ZERO_*/non/
positiveInteger_UNLIMITED/Duration_SEC/_NSEC/struct/sequence<T>/
sequence<octet>/T[N]/Duration_t) each with a spec section + repo
path. QoS-policy mapping in `qos.rs::EntityQos::into_writer_qos` /
`into_reader_qos`.

**Tests:** `conformance::tests::idl_mapping_covers_required_categories`
(mandatory categories from §7.1.4 + §7.2.x), `idl_mapping_entries_unique`
(no duplicates), `idl_mapping_includes_section_7_2_x_items` (all
§7.2.x items live), plus `qos.rs::tests::entity_qos_into_*_uses_*`
for QoS mapping.

**Status:** done — the mapping table covers all IDL-data-type
categories listed in DDS-XML 1.0 §7.2.

### 7.2.1 IDL enumeration -> XSD simpleType

**Spec:** §7.2.1, p. 8 — "IDL Enumerations are represented in XML
according to a schema defined as an XSD simpleType defined as a
restriction of a string that can take values of the enumeration
literals." Example `historyKind` with `KEEP_LAST_HISTORY_QOS`/
`KEEP_ALL_HISTORY_QOS` (non-normative).

**Repo:** `crates/xml/src/qos_parser.rs` — enum parsing for
HistoryKind/ReliabilityKind/DurabilityKind as string restrictions.

**Tests:** `qos_parser::tests::parse_reliability_and_history`,
`enum_match`.

**Status:** done

---

## §7.2.2 XML Representation of Primitive Constants

### 7.2.2.1 Constant `LENGTH_UNLIMITED = -1`

**Spec:** §7.2.2.1, p. 8 — "const long LENGTH_UNLIMITED = -1;"

**Repo:** `crates/xml/src/qos_parser.rs` (import of
`LENGTH_UNLIMITED`); the symbol string is accepted in the long parser.

**Tests:** `qos_parser::tests::long_length_unlimited_symbol`.

**Status:** done

### 7.2.2.2 Constant `DURATION_INFINITE_SEC = 0x7fffffff`

**Spec:** §7.2.2.1, p. 8 — "const long DURATION_INFINITE_SEC =
0x7fffffff;"

**Repo:** `crates/xml/src/qos_parser.rs` (import of
`DURATION_INFINITE_SEC`).

**Tests:** `qos_parser::tests::duration_sec_infinite_symbols`,
`duration_sec_normal`, `duration_sec_overflow`.

**Status:** done

### 7.2.2.3 Constant `DURATION_INFINITE_NSEC = 0x7fffffff`

**Spec:** §7.2.2.1, p. 8 — "const unsigned long DURATION_INFINITE_NSEC
= 0x7fffffff;"

**Repo:** `qos_parser.rs` (import).

**Tests:** `qos_parser::tests::duration_nsec_infinite`,
`duration_nsec_normal`, `duration_nsec_out_of_range`.

**Status:** done

### 7.2.2.4 Constant `DURATION_ZERO_SEC = 0`

**Spec:** §7.2.2.1, p. 8 — "const long DURATION_ZERO_SEC = 0;"

**Repo:** `qos_parser.rs` — default path without symbol.

**Tests:** `qos.rs::tests::duration_constants`.

**Status:** done

### 7.2.2.5 Constant `DURATION_ZERO_NSEC = 0`

**Spec:** §7.2.2.1, p. 8 — "const unsigned long DURATION_ZERO_NSEC
= 0;"

**Repo:** `qos_parser.rs` — default path.

**Tests:** `qos.rs::tests::duration_constants`.

**Status:** done

### 7.2.2.6 Constant `TIME_INVALID_SEC = -1`

**Spec:** §7.2.2.1, p. 8 — "const long TIME_INVALID_SEC = -1;"

**Repo:** `crates/xml/src/types.rs::TIME_INVALID_SEC` — constant
exported via `crates/xml/src/lib.rs`.

**Tests:** `types::tests::time_invalid_constants` (verifies the value
+ distinction from DURATION_INFINITE/DURATION_ZERO).

**Status:** done

### 7.2.2.7 Constant `TIME_INVALID_NSEC = 0xffffffff`

**Spec:** §7.2.2.1, p. 8 — "const unsigned long TIME_INVALID_NSEC =
0xffffffff;"

**Repo:** `crates/xml/src/types.rs::TIME_INVALID_NSEC` — constant
exported via `crates/xml/src/lib.rs`.

**Tests:** `types::tests::time_invalid_constants`.

**Status:** done

### 7.2.2.8 simpleType `nonNegativeInteger_UNLIMITED`

**Spec:** §7.2.2.1, p. 8 — pattern `(LENGTH_UNLIMITED|([0-9])*)?`.

**Repo:** implemented as a combined parser in qos_parser
(number-or-symbol).

**Tests:** `qos_parser::tests::long_length_unlimited_symbol`,
`long_decimal`.

**Status:** done

### 7.2.2.9 simpleType `positiveInteger_UNLIMITED`

**Spec:** §7.2.2.1, p. 8 — pattern `(LENGTH_UNLIMITED|[1-9]([0-9])*)?`.

**Repo:** `crates/xml/src/types.rs::parse_positive_long_unlimited`
— a dedicated parser that enforces the spec pattern `(LENGTH_UNLIMITED|[1-9]([0-9])*)?`:
the value `0`, leading zeros, hex and negative values
are rejected with `XmlError::ValueOutOfRange`; only the
`LENGTH_UNLIMITED` symbol or decimal values from `1` pass.

**Tests:** `types::tests::positive_unlimited_symbol_passes`,
`positive_unlimited_one_to_max_passes`,
`positive_unlimited_zero_rejected`,
`positive_unlimited_negative_rejected`,
`positive_unlimited_leading_zero_rejected`,
`positive_unlimited_hex_rejected`,
`positive_unlimited_overflow_rejected`,
`positive_unlimited_empty_rejected` (8 tests).

**Status:** done — positive constraint enforced with a
positive AND negative test per pattern component.

### 7.2.2.10 simpleType `nonNegativeInteger_Duration_SEC`

**Spec:** §7.2.2.1, p. 8 — pattern `(DURATION_INFINITY|
DURATION_INFINITE_SEC|([0-9])*)?`.

**Repo:** `qos_parser.rs::parse_duration_sec` — accepts both
symbol variants.

**Tests:** `qos_parser::tests::duration_sec_infinite_symbols`.

**Status:** done

### 7.2.2.11 simpleType `nonNegativeInteger_Duration_NSEC`

**Spec:** §7.2.2.1, p. 8 — pattern `(DURATION_INFINITY|
DURATION_INFINITE_NSEC|([0-9])*)?`.

**Repo:** `qos_parser.rs::parse_duration_nsec`.

**Tests:** `qos_parser::tests::duration_nsec_infinite`.

**Status:** done

---

## §7.2.3 XML Representation of Structure Types

### 7.2.3 IDL struct -> XSD complexType (default values preserved)

**Spec:** §7.2.3, p. 9 — "IDL structures are represented in XML
according to a schema defined as an XSD complexType. The fields in an
IDL structure become unordered elements of the complexType with the
field name appearing as the corresponding element name. This mapping
is applied recursively for nested structures. If the DDS specification
defines default values for the structure fields, the corresponding
XSD element definition shall provide the same default value."

**Repo:** `crates/xml/src/qos_parser.rs` — Reliability/History/
ResourceLimits are filled with default constants when the element
is missing; recursive QoS containers.

**Tests:** `qos.rs::tests::entity_qos_into_writer_uses_defaults_for_unset`,
`merge_none_does_not_clobber`.

**Status:** done

---

## §7.2.4 XML Representation of Sequences

### 7.2.4.1 Sequence mapping with the `<element>` tag

**Spec:** §7.2.4.1, p. 10 — "The general XML representation of IDL
sequences is done following a schema defined as an XSD complexType.
The complexType contains zero or more elements named element. Nested
inside each element is the XSD schema obtained from mapping the IDL
type of the element itself."

**Repo:** `crates/xml/src/parser.rs::XmlElement::sequence_elements`
— a generic iterator over `<element>` children, usable for
any IDL sequence. Existing QoS paths (e.g. `<partition>`)
remain compatible; new top-level helper.

**Tests:** `parser::tests::sequence_elements_iterates_element_tag_children`,
`sequence_elements_skips_non_element_tagged_children`,
`sequence_elements_empty_for_zero_children`.

**Status:** done — generalized helper + 3 tests (positive +
non-element-skip + empty sequence).

### 7.2.4.2 Sequences of octets (decimal/hex or Base64)

**Spec:** §7.2.4.2, p. 11 — "Sequences of octets are represented
either as a comma-separated list of the value of each octet
represented in decimal or hexadecimal, or alternatively using Base64
binary. Differentiated by element name (`value` vs. `valueB64`)."

**Repo:** Base64 path in `crates/xml/src/qos_parser.rs::base64_decode`
(for `<valueB64>` elements); decimal/hex comma list in
`crates/xml/src/types.rs::parse_octet_sequence` (for
`<value>` elements). The caller selects based on the element name.

**Tests:** Base64 path: `qos_parser::tests::base64_decode_basic`,
`base64_decode_with_padding`, `base64_decode_invalid_returns_none`.
Decimal/hex path: `types::tests::octet_sequence_decimal_basic`,
`octet_sequence_hex_basic`, `octet_sequence_mixed_decimal_and_hex`,
`octet_sequence_whitespace_around_commas`,
`octet_sequence_empty_string_returns_empty_vec`,
`octet_sequence_value_above_255_rejected`,
`octet_sequence_negative_rejected`,
`octet_sequence_trailing_comma_rejected`,
`octet_sequence_double_comma_rejected`,
`octet_sequence_non_numeric_token_rejected`,
`octet_sequence_hex_above_255_rejected` (11 tests).

**Status:** done — both spec-conformant paths
(comma list + Base64) live, value range `0..=255` enforced.

---

## §7.2.5 XML Representation of Arrays

### 7.2.5 Array mapping = sequence mapping

**Spec:** §7.2.5, p. 11 — "The XML representation of IDL arrays is
the same as it would be for IDL sequences of the same element type."

**Repo:** reuse of the `<element>` iterator
`crates/xml/src/parser.rs::XmlElement::sequence_elements` —
identical API for fixed-size arrays + variable sequences. QoS
paths in `qos_parser` use the same mechanism.

**Tests:** `parser::tests::array_uses_same_element_tag_as_sequence`
— explicit IDL `coords_3d[3]` test, plus indirectly
`qos.rs::tests::*partition*` (partition as a sequence of strings).

**Status:** done — reuse explicitly documented + tested.

---

## §7.2.6 XML Representation of Duration

### 7.2.6 Duration_t -> XSD complexType `duration` with symbols

**Spec:** §7.2.6, p. 11 — "The IDL structure Duration_t is represented
in XML following the general rules for structures defined in sub
clause 7.2.3, except that the schema provides the option to use the
symbolic defined in the IDL to set the values of the intended
elements." Schema with `<sec>` (type `nonNegativeInteger_Duration_SEC`)
and `<nanosec>` (type `nonNegativeInteger_Duration_NSEC`).

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration` —
uses `parse_duration_sec`+`parse_duration_nsec`; maps the
DURATION_INFINITY/DURATION_INFINITE_SEC/_NSEC symbols.

**Tests:** `qos_parser::tests::duration_inline_infinity_sentinel`,
`duration_sec_infinite_symbols`, `duration_nsec_infinite`,
`duration_sec_normal`, `duration_nsec_normal`.

**Status:** done

---

## §7.3 Building Blocks

### 7.3.1.1 Six building blocks

**Spec:** §7.3.1, p. 12 — "This specification breaks the syntax used
to represent DDS resources in XML into the six different building
blocks: Building Block QoS, Types, Domains, DomainParticipants,
Applications, Data Samples."

**Repo:** `crates/xml/src/{qos,types,domain,participant,application,sample}.rs`
— six modules.

**Tests:** per BB below.

**Status:** done

### 7.3.1.2 XSD naming `<bb>_definitions_nonamespace.xsd` (chameleon)

**Spec:** §7.3.1, p. 13 — "dds-xml_<building_block_name>_definitions_
nonamespace.xsd contains the type declarations for all the constructs
the building block defines. This XSD file specifies neither a
targetNamespace nor a root element."

**Repo:** all 7 chameleon files (without targetNamespace) in
`crates/xml/schemas/dds-xml_<bb>_definitions_nonamespace.xsd` —
QoS, Types, Domains, DomainParticipants, Applications, DataSamples,
DDSSystem.

**Tests:** `schemas::tests::nonamespace_xsds_omit_target_namespace`
(checks per file that the `<xs:schema>` tag has NO
`targetNamespace` attribute).

**Status:** done

### 7.3.1.3 XSD naming `<bb>_definitions.xsd` with targetNamespace

**Spec:** §7.3.1, p. 13 — "dds-xml_<building_block_name>_definitions.xsd
includes the XSD with no targetNamespace, defines the top level
element for the building block, and sets targetNamespace to
`http://www.omg.org/spec/DDS-XML`."

**Repo:** `crates/xml/src/parser.rs::DDS_XML_NAMESPACE` constant +
all 7 namespaced XSD files in `crates/xml/schemas/
dds-xml_<bb>_definitions.xsd` with
`targetNamespace="http://www.omg.org/spec/DDS-XML"`.

**Tests:** `parser.rs::tests::dds_xml_namespace_constant_matches_spec`,
`strict_mode_accepts_xml_with_correct_namespace`,
`strict_mode_rejects_xml_without_namespace`,
`lax_mode_accepts_xml_without_namespace`,
`validation_mode_default_is_lax` +
`schemas::tests::namespaced_xsds_include_target_namespace`,
`namespaced_xsds_define_top_level_element`.

**Status:** done — namespace constant, validation mode AND
XSD files all live.

---

## §7.3.2 Building Block QoS

### 7.3.2.1 Purpose: represent DDS QoS in XML

**Spec:** §7.3.2.1, p. 13 — "This building block defines the syntax
to represent DDS QoS in XML."

**Repo:** `crates/xml/src/qos.rs`, `crates/xml/src/qos_parser.rs`,
`crates/xml/src/qos_inheritance.rs`.

**Tests:** 30+ tests in `qos.rs` + `qos_parser.rs`.

**Status:** done

### 7.3.2.2 No dependency on other BBs

**Spec:** §7.3.2.2, p. 13 — "This building block has no dependencies
on other building blocks."

**Repo:** `qos.rs` imports only `parser.rs`+`errors.rs`+`inheritance.rs`,
no other BB modules.

**Tests:** n/a (architecture statement).

**Status:** done

### 7.3.2.3 Syntax: two XSD files with/without targetNamespace

**Spec:** §7.3.2.3, p. 14 — "dds-xml_qos_definitions_nonamespace.xsd"
+ "dds-xml_qos_definitions.xsd" with `<qos_library>` as root.

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`
+ `dds-xml_qos_definitions.xsd` (with `<qos_library>` as root via
`xs:element name="qos_library"`); embedded via
`schemas::QOS_NONAMESPACE_XSD` / `QOS_NAMESPACED_XSD`.
The `<qos_library>` root is accepted by the parser (`qos.rs::QosLibrary`).

**Tests:** `qos_parser::tests::parse_minimal_library`,
`missing_library_name_rejected` +
`schemas::tests::namespaced_xsds_define_top_level_element` (checks
explicitly that the namespaced XSD contains `xs:element name="qos_library"`).

**Status:** done — both XSD files embedded + parser live.

### 7.3.2.4.1 QoS Libraries and QoS Profiles

**Spec:** §7.3.2.4.1, p. 14 — "QoS Libraries are the top level
element of the Building Block QoS. They are collections of QoS
profiles, which group a set of related QoS — usually one per entity."

**Repo:** `crates/xml/src/qos.rs::QosLibrary` with a `name` field and
`profiles: Vec<QosProfile>`; `QosLibrary::profile(name)` lookup.

**Tests:** `qos.rs::tests::library_profile_lookup`,
`qos_profile_shape_roundtrip`, `qos_parser::parse_minimal_library`.

**Status:** done

### 7.3.2.4.2 QoS profile inheritance via `base_name`

**Spec:** §7.3.2.4.2, p. 14 — "A QoS Profile can inherit from other
QoS Profiles using the `base_name` XML attribute. A QoS profile can
only inherit from QoS profiles that have been defined [before] its
definition."

**Repo:** `crates/xml/src/qos.rs::QosProfile::base_name`,
`crates/xml/src/inheritance.rs::resolve_chain` with cycle detection.

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

### 7.3.2.4.3 QoS profile topic-name filters via `topic_filter`

**Spec:** §7.3.2.4.3, p. 14 — "A QoS Profile may contain several
DataWriter, DataReader, and Topic QoS settings that are selected
based on the evaluation of a filter expression on the topic name.
The filter expression is specified via the `topic_filter` XML
attribute. If unspecified, `*` is assumed. QoS with an explicit
`topic_filter` attribute is evaluated in order; takes precedence
over a QoS without filter."

**Repo:** `crates/xml/src/qos.rs::QosProfile::topic_filter`,
`crates/xml/src/qos.rs::topic_filter_matches` (POSIX fnmatch:
`*`, `?`, exact).

**Tests:** `qos.rs::tests::glob_star_matches_all`,
`glob_prefix`, `glob_question_mark`, `glob_exact`,
`topic_filter_inherited_and_overridden`.

**Status:** done

### 7.3.2.4.4 QoS profiles with a single QoS

**Spec:** §7.3.2.4.4, p. 15 — "The definition of an individual QoS
is a shortcut for defining a QoS profile with a single QoS." Example
`<datawriter_qos name="..."/>` equivalent to `<qos_profile
name="..."/><datawriter_qos>...</datawriter_qos></qos_profile>`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_single_qos_shortcut`
— recognizes `<datawriter_qos>` / `_reader` / `_topic` /
`_publisher` / `_subscriber` / `_(domain)participant_qos` directly
under `<qos_library>` with a `name` attribute and wraps them in an
implicit `QosProfile`.

**Tests:** `qos_parser::tests::single_qos_shortcut_datawriter_creates_implicit_profile`,
`single_qos_shortcut_topic_creates_implicit_profile`,
`single_qos_shortcut_without_name_is_ignored`,
`single_qos_shortcut_multiple_kinds_in_same_library` (4 tests).

**Status:** done

---

## §7.3.3 Building Block Types

### 7.3.3.1 Purpose: DDS Types in XML

**Spec:** §7.3.3.1, p. 15 — "This building block gathers the syntax
used to represent DDS Types in XML. Additionally, it provides
capabilities that are necessary or convenient for the organization
and management of types."

**Repo:** `crates/xml/src/types.rs`, `crates/xml/src/xtypes_def.rs`,
`crates/xml/src/xtypes_parser.rs`, `crates/xml/src/typeobject_bridge.rs`.

**Tests:** 30+ tests for Struct/Union/Enum/Bitset/Bitmask/Typedef.

**Status:** done

### 7.3.3.2 No dependency on other BBs

**Spec:** §7.3.3.2, p. 15 — "This building block has no dependencies
on other building blocks."

**Repo:** `types.rs` imports only base modules (parser/errors).

**Tests:** n/a.

**Status:** done

### 7.3.3.3 Syntax: two XSD files with/without targetNamespace, `<types>` as root

**Spec:** §7.3.3.3, p. 16 — "dds-xml_type_definitions_nonamespace.xsd"
+ "dds-xml_type_definitions.xsd" with `<types>` as root.

**Repo:** `crates/xml/schemas/dds-xml_types_definitions_nonamespace.xsd`
+ `dds-xml_types_definitions.xsd` with `<types>` as root; embedded
via `schemas::TYPES_NONAMESPACE_XSD` / `TYPES_NAMESPACED_XSD`.
The `<types>` root is accepted by the parser (`xtypes_parser.rs`).

**Tests:** `xtypes_parser::tests::parse_simple_struct`,
`parse_module_nested`, `module_at_top_level_is_error`,
`parse_namespace_aware` +
`schemas::tests::namespaced_xsds_define_top_level_element` (verifies
`xs:element name="types"`).

**Status:** done — both XSD files embedded.

---

## §7.3.4 Building Block Domains

### 7.3.4.1 Purpose: DDS Domains in XML

**Spec:** §7.3.4.1, p. 16 — "This building block defines the syntax
used to represent DDS Domains in XML. Domains provide a data space
where information can be shared by reading and writing a set of
Topics, which are associated to registered data types."

**Repo:** `crates/xml/src/domain.rs::DomainEntry` with `register_types`
and `topics`.

**Tests:** `domain.rs::tests::parse_minimal_domain_library`,
`parse_domain_with_topic`.

**Status:** done

### 7.3.4.2 Dependencies: BB QoS + BB Types

**Spec:** §7.3.4.2, p. 16 — "This building block depends on the
Building Block QoS and the Building Block Types."

**Repo:** `domain.rs` references `register_type_ref` (Types)
and accepts inline `<topic_qos>` (QoS).

**Tests:** `zerodds_xml.rs::tests::resolve_*` (cross-BB resolution).

**Status:** done

### 7.3.4.3 Syntax: two XSD files with/without targetNamespace, `<domain_library>` as root

**Spec:** §7.3.4.3, p. 16 — "dds-xml_domain_definitions_nonamespace.xsd"
+ "dds-xml_domain_definitions.xsd" with `<domain_library>` as root.

**Repo:** `crates/xml/schemas/dds-xml_domains_definitions_nonamespace.xsd`
+ `dds-xml_domains_definitions.xsd` with `<domain_library>` as root;
embedded via `schemas::DOMAINS_NONAMESPACE_XSD` /
`DOMAINS_NAMESPACED_XSD`. The `<domain_library>` root is accepted by
the parser.

**Tests:** `domain.rs::tests::parse_minimal_domain_library` +
`schemas::tests::namespaced_xsds_define_top_level_element`.

**Status:** done — both XSD files embedded.

### 7.3.4.4.1 Defining a Domain

**Spec:** §7.3.4.4.1, p. 16 — "A Domain includes a set of Topics and
Registered Types that can be read and written. Register types shall
provide a reference to data types via `type_ref`. Topics shall refer
to a registered type via `register_type_ref`. Topics may also specify
QoS settings inline. QoS profile inheritance through the `base_name`
attribute may be used."

**Repo:** `crates/xml/src/domain.rs::RegisterTypeEntry`+`TopicEntry`
with `type_ref`/`register_type_ref`/`base_name`.

**Tests:** `domain.rs::tests::parse_domain_with_topic`,
`parse_topic_with_inline_qos`, `topic_missing_register_type_ref_rejected`.

**Status:** done

### 7.3.4.4.2 Domain inheritance via `base_name`

**Spec:** §7.3.4.4.2, p. 17 — "A Domain can inherit from other
Domains using the `base_name` XML attribute. A Domain can only
inherit from domains that have been defined before."

**Repo:** `crates/xml/src/domain.rs::DomainEntry::base_name` (Option<
String>) + the parser reads the `base_name` attribute. Resolution via
`inheritance::resolve_chain` (the same mechanism as qos_profile).

**Tests:** `domain::tests::parse_domain_with_base_name`,
`domain_inheritance_chain_resolves_via_resolve_chain`,
`domain_inheritance_cycle_detected` (3 tests: field presence + 3-tier
chain A→B→C + cycle detection).

**Status:** done

---

## §7.3.5 Building Block DomainParticipants

### 7.3.5.1 Purpose

**Spec:** §7.3.5.1, p. 17 — "This block defines the syntax to
represent DDS DomainParticipants and their contained entities (i.e.,
Publishers, Subscribers, DataWriters, and DataReaders) in XML."

**Repo:** `crates/xml/src/participant.rs::DomainParticipantEntry`
with Publishers/Subscribers; `PublisherEntry`/`SubscriberEntry` with
DataWriters/DataReaders.

**Tests:** `participant.rs::tests::parse_minimal_participant`,
`parse_pub_with_writer`.

**Status:** done

### 7.3.5.2 Dependencies: BB QoS + BB Types + BB Domains

**Spec:** §7.3.5.2, p. 17 — "This building block depends on the
Building Block QoS, the Building Block Types, and the Building Block
Domains."

**Repo:** `participant.rs` references `domain_ref` and accepts
inline `<domain_participant_qos>`.

**Tests:** `zerodds_xml.rs::tests::resolve_participant_*`.

**Status:** done

### 7.3.5.3 Syntax: two XSD files with/without targetNamespace, `<domain_participant_library>` as root

**Spec:** §7.3.5.3, p. 17 — "dds-xml_domainparticipant_defintions_
nonamespace.xsd" + "dds-xml_domainparticipant_definitions.xsd" with
`<domain_participant_library>` as root.

**Repo:** `crates/xml/schemas/dds-xml_domain_participants_definitions_nonamespace.xsd`
+ `dds-xml_domain_participants_definitions.xsd` with
`<domain_participant_library>` as root; embedded via
`schemas::DOMAIN_PARTICIPANTS_NONAMESPACE_XSD` /
`DOMAIN_PARTICIPANTS_NAMESPACED_XSD`. The root is accepted by the parser.

**Tests:** `participant.rs::tests::parse_minimal_participant` +
`schemas::tests::namespaced_xsds_define_top_level_element`.

**Status:** done — both XSD files embedded.

### 7.3.5.4.1 DomainParticipant Libraries + hierarchy

**Spec:** §7.3.5.4.1, p. 17 — "Domain Participant Libraries are
collections of DomainParticipants and contained entities. Each
entity is declared as a nested XML tag under the declaration of its
parent entity."

**Repo:** `participant.rs::DomainParticipantLibrary` with
`participants: Vec<DomainParticipantEntry>`; nested
Pub/Sub/DW/DR entries.

**Tests:** `participant.rs::tests::parse_pub_with_writer`,
`dw_missing_topic_ref_rejected`,
`missing_domain_ref_rejected`.

**Status:** done

### 7.3.5.4.2 `domain_ref` + `domain_id` override

**Spec:** §7.3.5.4.2, p. 18 — "DomainParticipants may refer to a
Domain declared via the `domain_ref` XML attribute. The Domain Id
specified in the parent Domain can be overridden via the `domain_id`
XML attribute."

**Repo:** `participant.rs::DomainParticipantEntry::domain_ref` +
`domain_id_override`.

**Tests:** `zerodds_xml.rs::tests::resolve_participant_with_domain_id_override`,
`missing_domain_ref_rejected`.

**Status:** done

### 7.3.5.4.3 DomainParticipant inheritance via `base_name`

**Spec:** §7.3.5.4.3, p. 18 — "DomainParticipants may inherit from
DomainParticipants defined in the context of a DomainParticipant
Library using the `base_name` XML attribute."

**Repo:** `participant.rs::DomainParticipantEntry::base_name`,
`zerodds_xml.rs::resolve_participant` walks the base chain.

**Tests:** `zerodds_xml.rs::tests::resolve_*` (inheritance paths).

**Status:** done

### 7.3.5.4.4 Inline entity QoS settings + `base_name`

**Spec:** §7.3.5.4.4, p. 18 — "Inline QoS setting definition is
allowed in the context of an entity's definition. Inline entities may
inherit from an existing QoS Profile using the `base_name` XML
attribute."

**Repo:** inline QoS fields in DataWriterEntry/DataReaderEntry +
`base_name` resolution via `qos_inheritance.rs`.

**Tests:** `qos_inheritance::tests::child_inherits_parent_reliability`,
`participant.rs::tests::parse_pub_with_writer`.

**Status:** done

---

## §7.3.6 Building Block Applications

### 7.3.6.1 Purpose

**Spec:** §7.3.6.1, p. 18 — "This block defines the XML syntax to
represent DDS applications that participate (or may be participating)
in the DDS Global Data Space."

**Repo:** `crates/xml/src/application.rs::ApplicationLibrary` with
`applications: Vec<ApplicationEntry>`.

**Tests:** `application.rs::tests::parse_minimal_application`,
`missing_app_name_rejected`, `missing_dp_ref_rejected`.

**Status:** done

### 7.3.6.2 Dependencies: BB QoS + BB Types + BB Domains + BB DomainParticipants

**Spec:** §7.3.6.2, p. 18 — "This building block depends on the
Building Block QoS, the Building Block Types, the Building Block
Domains, and the Building Block DomainParticipants."

**Repo:** `application.rs` references `domain_participants: Vec<String>`
refs (loosely coupled to DomainParticipantLibrary).

**Tests:** `zerodds_xml.rs::tests::resolve_application` walks DPLib.

**Status:** done

### 7.3.6.3 Syntax: two XSD files with/without targetNamespace, `<application_library>` as root

**Spec:** §7.3.6.3, p. 19 — "dds-xml_application_definitions_
nonamespace.xsd" + "dds-xml_application_definititons.xsd" (sic, with a
typo in the spec) with `<application_library>` as root.

**Repo:** `crates/xml/schemas/dds-xml_applications_definitions_nonamespace.xsd`
+ `dds-xml_applications_definitions.xsd` with `<application_library>`
as root; embedded via `schemas::APPLICATIONS_NONAMESPACE_XSD` /
`APPLICATIONS_NAMESPACED_XSD`. The root is accepted by the parser.

**Tests:** `application.rs::tests::parse_minimal_application` +
`schemas::tests::namespaced_xsds_define_top_level_element`.

**Status:** done — both XSD files embedded.

### 7.3.6.4.1 Applications + DomainParticipants + contained entities

**Spec:** §7.3.6.4.1, p. 19 — "Application Libraries are collections
of Applications, which are composed of a set of DomainParticipants
and contained entities."

**Repo:** `application.rs::ApplicationEntry::domain_participants:
Vec<String>` with `<domain_participant ref="..."/>` children.

**Tests:** `application.rs::tests::parse_minimal_application`.

**Status:** done

### 7.3.6.4.2 DomainParticipants from DomainParticipant libraries via `base_name`

**Spec:** §7.3.6.4.2, p. 19 — "DomainParticipants defined in the
context of an Application may inherit from DomainParticipants defined
in the context of a DomainParticipant Library using the `base_name`
XML attribute."

**Repo:** cross-library `base_name` resolution via `zerodds_xml.rs::resolve_*`.

**Tests:** `zerodds_xml.rs::tests::resolve_application` (resolved via
DP-library lookup).

**Status:** done

---

## §7.3.7 Building Block Data Samples

### 7.3.7.1 Purpose

**Spec:** §7.3.7.1, p. 19 — "This block defines XML syntax to
represent Data Samples that may be exchanged between different DDS
applications."

**Repo:** `crates/xml/src/sample.rs` — sample parsing for
Struct/Union/Sequence/Array (all 4 SampleValue variants).

**Tests:** `sample::tests::parse_simple_struct_sample`,
`parse_sample_with_union`,
`parse_sample_with_sequence_using_item_tag`,
`parse_sample_with_array_using_item_tag`,
`parse_sample_with_empty_sequence`,
`serialize_sample_uses_item_tag_for_sequence` (6 tests).

**Status:** done

### 7.3.7.2 No dependency on other BBs

**Spec:** §7.3.7.2, p. 19 — "This building block has no dependencies
on other building blocks."

**Repo:** `sample.rs` imports only parser/errors.

**Tests:** n/a.

**Status:** done

### 7.3.7.3.1 Syntax general rules: uses §7.1, §7.1.2, §7.2.2, §7.2.3, §7.2.6

**Spec:** §7.3.7.3.1, p. 20 — "the syntax to represent Data Samples
is based on the XML representation rules specified in sub clauses
7.1, 7.1.2, 7.2.2, 7.2.3, and 7.2.6."

**Repo:** `sample.rs` delegates to `parser`+`qos_parser` helpers
(boolean/long/duration). Sequence/array via the `<item>` tag (spec
§7.3.7.3.2 reuse), struct via a member map, union via discriminator.

**Tests:** `sample::tests::*` (all 7 tests, see §7.3.7.1).

**Status:** done

### 7.3.7.3.2 Sequences/arrays with the `<item>` tag (not `<element>` as in §7.2.4)

**Spec:** §7.3.7.3.2, p. 20 — "The general XML representation of IDL
sequences and arrays is done following a schema defined as an XSD
complexType. The complexType contains zero or more elements named
**item**. Nested inside each item is the XSD schema obtained from
mapping the IDL type of the element itself."

**Repo:** `crates/xml/src/sample.rs::parse_member_value` —
sequence/array iterates via `el.children_named("item")`;
`serialize_sample` emits `<item>` for both cases. An empty
sequence (`<seq></seq>`) is recognized via the `sequence_max_length`
hint.

**Tests:** `sample::tests::parse_sample_with_sequence_using_item_tag`,
`parse_sample_with_array_using_item_tag`,
`parse_sample_with_empty_sequence`,
`serialize_sample_uses_item_tag_for_sequence` (4 tests).

**Status:** done

### 7.3.7.4 Examples (non-normative)

**Spec:** §7.3.7.4, pp. 20-22 — examples for Struct/Union/Sequence/
Array/Primitive samples. The section is explicitly "(non-normative)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec marks §7.3.7.4 explicitly as "(non-normative)"; the example samples illustrate the mapping without a new normative rule.

---

## §8 Building Block Sets

### 8.1.1 DDS System Block Set covers BBQoS+BBTypes+BBDomains+BBDPs+BBApplications

**Spec:** §8.1, p. 23 — "This block set offers the ability to
describe a complete DDS system. It contains: Building Block QoS,
Types, Domains, DomainParticipants, Applications." (BB DataSamples
NOT included.)

**Repo:** `crates/xml/src/zerodds_xml.rs::DdsXml` with fields for
`qos_libraries`, `type_libraries`, `domain_libraries`,
`participant_libraries`, `application_libraries`.

**Tests:** `zerodds_xml.rs::tests::parse_empty_dds`,
`parse_mixed_top_level`, `dds_root_with_multiple_types_blocks`,
`resolve_application`.

**Status:** done

### 8.1.2 XSD files `dds-xml_dds_system_definitions[_nonamespace].xsd`

**Spec:** §8.1, p. 23 — "dds-xml_dds_system_definitions_nonamespace.xsd"
+ "dds-xml_dds_system_definitions.xsd" with `<dds>` as top-level.

**Repo:** the `<dds>` root is accepted by `parser.rs::parse_xml_tree`;
`DDS_XML_NAMESPACE` constant +
`crates/xml/schemas/dds-xml_dds_system_definitions_nonamespace.xsd`
+ `dds-xml_dds_system_definitions.xsd` with `<dds>` as top-level
(via `xs:element name="dds"`). Embedded via
`schemas::DDS_SYSTEM_NONAMESPACE_XSD` / `DDS_SYSTEM_NAMESPACED_XSD`.

**Tests:** `parser.rs::tests::strict_mode_accepts_xml_with_correct_namespace`,
`zerodds_xml.rs::tests::non_dds_root_rejected`, `unknown_root_rejected`
+ `schemas::tests::namespaced_xsds_define_top_level_element`
(verifies `xs:element name="dds"`).

**Status:** done — both XSD files embedded.

---

## Audit status

73 done / 0 partial / 0 open / 15 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-xml` — 221 lib + 23 + 10 = 254 tests green
  (XML-QoS loader + XSD validator).
* `cargo test -p zerodds-xml-wire` — 40 tests green (XML↔CDR codec for
  XTypes-XML wire PSM, see `dds-xtypes-1.3.md` §2.4).
