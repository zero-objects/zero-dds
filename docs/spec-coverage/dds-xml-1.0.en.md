# DDS Consolidated XML Syntax 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xml-1.0.pdf` (33 pages, OMG
formal/18-10-01, December 2018) —
[OMG DDS-XML 1.0](https://www.omg.org/spec/DDS-XML/1.0/)

Item-by-item audit against the PDF following `internal/spec-coverage/PROCESS.md`. Page numbers are PDF pages (PDF page = printed page + 9). Every normative sentence of §1 to §8, every row of Table 7.1 and Table 7.2, every constant of §7.2.2 and every attribute of the building blocks is an item of its own. §7.2 and §7.2.3 require a 1-to-1 mapping of the types of the DDS IDL; the QoS policies, their enumerations and the QoS structures of the DDS 1.4 IDL (`internal/standards/cache/omg/dds-dcps-1.4.pdf`, p. 157–161) are therefore one item each under §7.2.1 and §7.2.3; their defaults are in DDS 1.4 §2.2.3 (p. 104–113 of the same PDF). `done` is used only where code and a test cover the requirement; a cited test names the test function as `file::module::name`. A repository reference names the file and the symbol, no line. The evidence is read from the code and compiled; which tests were run is stated under "Audit-Status". Where code implements a requirement but no test checks it, the item is `open` and the note names the code location.

**Context:** DDS-XML 1.0 is spread over these parts of ZeroDDS:

- `crates/xml/` (`zerodds-xml`) — the reader for all building blocks: the
  document tree (`parser`), the values of §7.1.4 and §7.2.2 (`types`), QoS
  libraries and profiles (`qos`, `qos_parser`, `qos_inheritance`,
  `registry`), domains (`domain`), DomainParticipants (`participant`),
  applications (`application`), the reader for `<dds>` with the resolution
  of references (`zerodds_xml`), types (`xtypes_def`, `xtypes_parser`), data
  samples (`sample`) and the XSD files under `crates/xml/schemas/`
  (`schemas`).
- `crates/xml-compose/` (`zerodds-xml-compose`) — a second reader for
  `<types>` that brings the types into the common model of the code
  generators.
- Callers: `tools/xmlc` (`zerodds-xmlc validate|types|render`),
  `tools/admin` (`qos validate`, `qos check`), `crates/rpc`
  (`RpcQos::from_xml_profile`) and `crates/webdds-bridge` (QoS profiles and
  entities for DDS-WEB).

`zerodds-dcps`, the C API and the language bindings read no DDS-XML.
`crates/dcps/src/qos.rs` takes a `zerodds_qos::WriterQos` or `ReaderQos` as
`QosProfileRegistry` returns it into `DataWriterQos` and `DataReaderQos`
(`From` implementations). The specification defines no programming interface
(§2 leaves conformance to the referencing specifications); items therefore
exist only for the syntax and its meaning.

The PDF names XSD files under `https://www.omg.org/spec/DDS-XML/20180501/` on p. 1 as normative machine-readable files. The twelve files for QoS, Types, Domains, DomainParticipants, Applications and DDS System are in `crates/xml/schemas/` unchanged (byte for byte the OMG's); Data Samples have no normative pair (§7.3.7.3.1), ZeroDDS ships its own. The example documents `dds-xml_*_example.xml` are not in the repository: the tests in `crates/xml/tests/omg_example_documents.rs` and part of `crates/xml/tests/schema_validation.rs` read them from the directory in `ZERODDS_OMG_DDSXML_DIR` and return without checking when the variable is unset; `schema_validation.rs` also needs `xmllint`. Where the text and the XSD of the spec contradict each other, the item states the decision: `user_data`, `topic_data` and `group_data` (XSD: one `value` as Base64; text: a list in `value` or `valueB64`; all forms are read), `base_name` at `<domain>` (text only; read) and `type_ref` at `register_type` (text: required; XSD: optional; required).

The grammar of the types in `<types>` is that of [DDS-XTYPES]; it is listed
item by item in [`dds-xtypes-1.3.en.md`](dds-xtypes-1.3.en.md) under §7.3.2.
Here only the requirements DDS-XML places on the building block Types are
listed.

**Open items:** none; every item is `done` or `n/a (informative)`.

---

## §1 Scope

### §1 Consolidation of the XML syntax

**Spec:** §1, p. 10 — "Historically various specifications have defined XML
syntax to represent particular subsets of DDS-related resources [...] This
specification consolidates all this XML syntax into a single document. There
are no significant syntactic changes in this document relative to referenced
specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the syntax is in the items from
§7 on.

---

## §2 Conformance Criteria

### §2 No independent conformance points

**Spec:** §2, p. 10 — "This document contains no independent conformance
points. Rather, it defines XML Schemas to be used to describe DDS resources
such that they can be referenced by other specifications leaving the
definition of conformance criteria to the referencing specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — note on the conformance model.

### §2 Rule 1: future specifications reference DDS-XML

**Spec:** §2, p. 10 — "Future specifications that describe DDS resources in
XML shall reference this specification or a future revision thereof."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — addressed to OMG specifications, not to an
implementation.

### §2 Rule 2: revisions of current specifications

**Spec:** §2, p. 10 — "Future revisions of current specifications that
describe DDS resources in XML should reference this specification or a future
revision thereof."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — addressed to OMG specifications.

### §2 Selected building blocks supported entirely

**Spec:** §2, p. 10 — "Reference to this standard shall result in a selection
of building blocks where all selected building blocks shall be supported
entirely."

**Repo:** `crates/xml/src/conformance.rs::SUPPORTED_BUILDING_BLOCKS`

**Tests:** `crates/xml/src/conformance.rs::tests::supported_blocks_match_spec_count`, `crates/xml/tests/schema_validation.rs::xsd_valid_documents_are_read`, `crates/xml/tests/omg_example_documents.rs::dds_system_example`

**Status:** done — all six building blocks are read, with the exceptions the items under §7.3 name one by one: the documents of the `XSD_VALID` table and the five OMG examples are read; what the XSDs reject, the reader rejects (`schema_invalid_documents`). One form the XSD allows and the reader does not read exists: `register_type` without `type_ref` (the text of §7.3.4.4.1 requires it, the XSD has `use="optional"`). Commits `8bf46985f11`, `efd043b8ec4`.

---

## §3 Normative References

### §3 [XML] Extensible Markup Language 1.0 (Fifth Edition)

**Spec:** §3, p. 10 — "[XML] Extensible Markup Language (XML) 1.0 (Fifth
Edition) Specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the requirement is in the item
on §7.1.1.

### §3 [XSD-1] XML Schema Definition Language 1.1 Part 1

**Spec:** §3, p. 10 — "[XSD-1] XML Schema Definition Language (XSD) 1.1 Part
1: Structures."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the schema files are in the
items on §7.1.2 and §7.3.1.

### §3 [XSD-2] XML Schema Definition Language 1.1 Part 2

**Spec:** §3, p. 10 — "[XSD-2] XML Schema Definition Language (XSD) 1.1 Part
2: Datatypes."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the values are in the items on
§7.1.4, §7.1.5 and §7.2.2.

### §3 [DDS4CCM] DDS for Lightweight CCM 1.1

**Spec:** §3, p. 10 — "The following referenced documents were used as input
to this specification: [DDS4CCM] DDS for Lightweight CCM (DDS4CCM), Version
1.1."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — source of the QoS syntax.

### §3 [DDS-XTYPES] Extensible and Dynamic Topic Types 1.2

**Spec:** §3, p. 10 — "[DDS-XTYPES] Extensible and Dynamic Topic Types for
DDS, Version 1.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — source of the syntax for types and samples;
types in the items on §7.3.3, samples in §7.3.7.

### §3 [DDS-WEB] Web-Enabled DDS 1.0

**Spec:** §3, p. 10 — "[DDS-WEB] Web-Enabled DDS, Version 1.0."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — source of the syntax for applications,
domains and entities.

### §3 [DDS] Data Distribution Service 1.4

**Spec:** §3, p. 11 — "[DDS] Data Distribution Service, Version 1.4. OMG
Document: formal/15-04-10."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the QoS IDL is in the items on
§7.2.1 and §7.2.3.

### §3 [IDL] Interface Definition Language 4.2

**Spec:** §3, p. 11 — "[IDL] Interface Definition Language (IDL), Version
4.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference.

---

## §4 Terms and Definitions

### §4 Building block

**Spec:** §4, p. 11 — "A building block is a consistent set of XML schemas
that together can be used to describe the syntax of XML documents that
represent a set of DDS resources. Building blocks are atomic, which means that
if selected they must be totally supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — term; the consequence for support is in the
item on §2.

### §4 Building block set

**Spec:** §4, p. 11 — "A building block set is a selection of building blocks
that determines a specific XSD schema usage."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — term; the only block set is in the items on
§8.1.

---

## §5 Symbols

### §5 Table 5.1 Acronyms

**Spec:** §5, p. 11 — DDS, ISO, LwCCM, OMG, QoS, UTF, XML, XSD, XTypes.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acronyms.

---

## §6 Additional Information

### §6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, p. 12 — "This specification does not change any adopted OMG
specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — note.

### §6.2 Acknowledgments

**Spec:** §6.2, p. 12 — submitting companies (Real-Time Innovations, Twin
Oaks Computing, Jackrabbit Consulting).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acknowledgments.

---

## §7.1.1 General Rules

### §7.1.1 Well-formed document according to [XML] clause 2.1

**Spec:** §7.1.1, p. 14 — "The XML representation of DDS-related resources
must follow these syntax rules: It shall be a well-formed XML document
according to the criteria defined in clause 2.1 of [XML]."

**Repo:** `crates/xml/src/parser.rs::parse_xml_tree` (via `roxmltree`), `precheck_depth`, `check_tree_depth`, `check_entity_declarations`; `crates/xml/src/sample.rs::serialize_sample` (the only writer)

**Tests:** `crates/xml/src/parser.rs::tests::invalid_xml_rejected`, `crates/xml/src/parser.rs::tests::parse_with_xml_declaration`, `crates/xml/src/parser.rs::tests::dtd_and_internal_entity_accepted`, `crates/xml/src/parser.rs::tests::entity_expansion_is_bounded`, `crates/xml/src/parser.rs::tests::external_entity_reference_rejected`, `crates/xml/src/parser.rs::tests::deep_nesting_in_cdata_does_not_fool_the_depth_check`, `crates/xml/tests/fuzz_smoke.rs::deep_nesting_is_refused_by_every_entry_point`, `crates/xml/tests/schema_validation.rs::serialize_sample_output_validates`

**Status:** done — a document that is not well-formed ends with `XmlError::InvalidXml`. A DTD is allowed ([XML] 2.1); internal entities are bounded (value at most `MAX_ENTITY_VALUE_BYTES`, at most `MAX_ENTITY_DECLARATIONS` declarations, no entity referring to another), external ones are not loaded. The reader also rejects well-formed documents that exceed its limits: more than 64 levels (`MAX_TREE_DEPTH`; `precheck_depth` counts the open elements iteratively before `roxmltree` runs, `check_tree_depth` checks the tree afterwards), more than 1024 children of an element (`MAX_LIST_ELEMENTS`) or more than 65,536 elements (`MAX_TOTAL_ELEMENTS`). Commits `3aec071e6f8`, `80ea5f7343e`, `24780539a95`. The test `serialize_sample_output_validates` needs `xmllint`.

### §7.1.1 UTF-8

**Spec:** §7.1.1, p. 14 — "It shall use UTF-8 character encoding for XML
elements and values."

**Repo:** `crates/xml/src/parser.rs::parse_xml_tree` (takes `&str`); `crates/xml/src/registry.rs::QosProfileRegistry::from_file` (`read_to_string`); `crates/xml/src/xsd_loader.rs::load_type_libraries_from_uri` (`from_utf8`); `crates/xml/src/sample.rs::xml_escape`

**Tests:** `crates/xml/tests/types_and_samples.rs::t24_sample_string_escaping_roundtrip`, `crates/xml/src/xsd_loader.rs::tests::data_uri_non_utf8_body_rejected`

**Status:** done — the input is a Rust string and thus UTF-8; `from_file` and `load_type_libraries_from_uri` reject bytes that are not UTF-8; the writer emits every character above U+007F as a character reference. The `encoding` declaration is in the item below. Commit `80ea5f7343e`.

### §7.1.1 Declaration of another encoding

**Spec:** §7.1.1, p. 14 — UTF-8 for elements and values.

**Repo:** `crates/xml/src/parser.rs::check_declared_encoding`; `crates/xml/src/xsd_loader.rs` (`percent_decode`)

**Tests:** `crates/xml/src/parser.rs::tests::non_utf8_declared_encoding_rejected`, `crates/xml/src/xsd_loader.rs::tests::data_uri_non_utf8_body_rejected`

**Status:** done — an XML declaration with an `encoding` other than UTF-8 (`utf-8` and `utf8`, in any case) ends with `XmlError::InvalidXml`, because `roxmltree` reads the value and discards it. A `data:` URI with bytes outside UTF-8 is rejected, not replaced by U+FFFD. Commit `80ea5f7343e`.

---

## §7.1.2 XML Schema Definition Files

### §7.1.2 Two normative XSD files per building block

**Spec:** §7.1.2, p. 14 — "This specification makes use of XML Schema
Definition (XSD) language specified in [XSD-1] and [XSD-2] to represent the
syntax of the different building blocks that define DDS resources. In
particular, each building block contains two normative XSD files that define
their syntax (see sub clause 7.3.1)."

**Repo:** `crates/xml/src/schemas.rs::ALL_SCHEMAS`; `crates/xml/schemas/`

**Tests:** `crates/xml/src/schemas.rs::tests::all_schemas_includes_six_building_blocks_plus_system`, `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/tests/schema_validation.rs::omg_examples_validate_against_in_tree_schemas`

**Status:** done — each of the six building blocks and the block set of §8.1 has two files under `crates/xml/schemas/`; the twelve files for QoS, Types, Domains, DomainParticipants, Applications and DDS System are the OMG's files, byte for byte (`dds-xml_<block>_definitions.xsd` and `…_nonamespace.xsd`). Data Samples have no normative pair (§7.3.7.3.1: no generic XSD); ZeroDDS ships its own pair and `dds-xml_common.xsd`, which only that pair includes. The OMG files include each other by the absolute URL `http://www.omg.org/spec/DDS-XML/20180501/`; `schemas::building_block_includes` maps it to file names. Commits `8bf46985f11`, `efd043b8ec4`. `every_in_tree_schema_compiles` and `omg_examples_validate_against_in_tree_schemas` need `xmllint`.

### §7.1.2 Names of the normative XSD files

**Spec:** §7.3.3.3, p. 25; §7.3.4.3, p. 25; §7.3.5.3, p. 26; §7.3.6.3,
p. 28 — `dds-xml_type_definitions[_nonamespace].xsd`,
`dds-xml_domain_definitions[_nonamespace].xsd`,
`dds-xml_domainparticipant_definitions[_nonamespace].xsd`,
`dds-xml_application_definitions[_nonamespace].xsd`.

**Repo:** `crates/xml/src/schemas.rs::ALL_SCHEMAS`; `crates/xml/schemas/`

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`, `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`

**Status:** done — the files carry the OMG's names: `dds-xml_type_definitions*.xsd`, `dds-xml_domain_definitions*.xsd`, `dds-xml_domainparticipant_definitions*.xsd`, `dds-xml_application_definitions*.xsd`, `dds-xml_qos_definitions*.xsd`, `dds-xml_dds_system_definitions*.xsd`. The spec is itself inconsistent (p. 1 lists `dds-xml_dds_qos_definitions_nonamespace.xsd` beside `dds-xml_qos_definitions.xsd`; §7.3.5.3 writes `…domainparticipant_defintions…`, §7.3.6.3 `…definititons…`); the names under which the OMG files include each other are the reference. Commit `efd043b8ec4`.

---

## §7.1.3 XML Chameleon Schema Definition Pattern (non-normative)

### §7.1.3.1 Motivation

**Spec:** §7.1.3.1, p. 14 — namespaces in XSD and the pattern of chameleon
schemas without `targetNamespace`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — "(non-normative)" in the title of §7.1.3.
How the normative files use the pattern is in the items on §7.3.1.

### §7.1.3.2.1 Example with several namespaces

**Spec:** §7.1.3.2.1, p. 14–15 — schema and document with three namespaces.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example. It writes `<kind>RELIABLE_RELIABILITY</kind>` (p. 15); the DDS 1.4 IDL names the literal `RELIABLE_RELIABILITY_QOS`, and §7.3.2.4.1.1 (p. 23) writes it so. `parse_reliability` rejects `RELIABLE_RELIABILITY` with `XmlError::BadEnum`.

### §7.1.3.2.2 Example with chameleon files

**Spec:** §7.1.3.2.2, p. 15–16 — included files without `targetNamespace`,
document with a single namespace.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example. ZeroDDS's readers do not check the
namespace (item on §7.3.1); a document without a namespace is read like one in
the specification's namespace.

---

## §7.1.4 XML Element Values

### §7.1.4 Table 7.1 boolean: yes as `1` or `true`

**Spec:** §7.1.4, Table 7.1, p. 16 — "boolean | Yes: 1 or true. | Values are
case sensitive."

**Repo:** `crates/xml/src/qos_parser.rs::parse_bool_strict`, called in `parse_presentation`, `parse_entity_factory`, `parse_writer_data_lifecycle` and for Data Samples in `crates/xml/src/sample.rs::parse_primitive_value`; `crates/xml/src/types.rs::parse_bool`

**Tests:** `crates/xml/src/qos_parser.rs::tests::parse_bool_strict_accepts_only_lowercase`, `crates/xml/src/types.rs::tests::bool_invalid`, `crates/xml/tests/qos_profile.rs::t14_boolean_case_sensitive_strict`

**Status:** done — `True` and `TRUE` are rejected. `crates/xml/src/types.rs::parse_bool`, exported by `lib.rs`, takes only `1`, `true`, `0` and `false`. Commit `80ea5f7343e`.

### §7.1.4 Table 7.1 boolean: no as `0` or `false`

**Spec:** §7.1.4, Table 7.1, p. 16 — "No: 0 or false. | Values are case
sensitive."

**Repo:** `crates/xml/src/qos_parser.rs::parse_bool_strict`

**Tests:** `crates/xml/src/qos_parser.rs::tests::parse_bool_strict_accepts_only_lowercase`,
`crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — `False` and `FALSE` are rejected.

### §7.1.4 Table 7.1 enum: DCPS IDL literal as a string

**Spec:** §7.1.4, Table 7.1, p. 16 — "enum | A string. Legal values are the
ones defined for QoS Policies in the DCPS IDL of DDS specification [DDS]. |
Must be specified as a string. (Do not use numeric values.)"

**Repo:** `crates/xml/src/qos_parser.rs`: `parse_durability`, `parse_history`, `parse_reliability`, `parse_liveliness`, `parse_destination_order`, `parse_ownership`, `parse_presentation`, `parse_durability_service`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`, `crates/xml/src/registry.rs::tests::resolves_base_writer_qos`

**Status:** done — every reader takes the DCPS IDL literal as a string; `every_literal_of_the_dcps_enumerations_is_read` reads all literals of the seven enumerations (items under §7.2.1). Commit `d2c3d3a9bef`.

### §7.1.4 Table 7.1 enum: only DCPS IDL literals

**Spec:** §7.1.4, Table 7.1, p. 16 — "Legal values are the ones defined for
QoS Policies in the DCPS IDL".

**Repo:** `crates/xml/src/qos_parser.rs`: the readers per policy

**Tests:** `crates/xml/src/qos_parser.rs::tests::short_enum_spellings_rejected`, `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — the short forms (`VOLATILE`, `KEEP_LAST`, `RELIABLE`, …) are no values; they end with `XmlError::BadEnum`. A `<history>` without `<kind>` and an empty `<kind/>` count as the DDS default (`KEEP_LAST_HISTORY_QOS`), not as a short form. They are not a vendor form either: the short forms are not read even with `VendorForms::Accept`. Commit `d2c3d3a9bef`.

### §7.1.4 Table 7.1 enum: no numeric values

**Spec:** §7.1.4, Table 7.1, p. 16 — "(Do not use numeric values.)"

**Repo:** `crates/xml/src/qos_parser.rs`: the readers per policy

**Tests:** `crates/xml/tests/spec_items.rs::a_number_is_no_enumeration_value`

**Status:** done — a number as the value of a `kind` element ends with `XmlError::BadEnum`. Commit `d2c3d3a9bef`.

### §7.1.4 Table 7.1 long: decimal -2147483648 to 2147483647

**Spec:** §7.1.4, Table 7.1, p. 16 — "long | -2147483648 to 2147483647 [...]
| A 32-bit signed integer."

**Repo:** `crates/xml/src/types.rs::parse_long`, called for the
`long` fields of the policies (`crates/xml/src/qos_parser.rs`)

**Tests:** `crates/xml/src/types.rs::tests::long_decimal`,
`crates/xml/src/types.rs::tests::long_invalid`,
`crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — data samples read `long` with code of their own (items on
§7.3.7.3.1).

### §7.1.4 Table 7.1 long: hexadecimal 0x80000000 to 0x7fffffff

**Spec:** §7.1.4, Table 7.1, p. 16 — "or 0x80000000 to 0x7fffffff".

**Repo:** `crates/xml/src/types.rs::parse_long`

**Tests:** `crates/xml/src/types.rs::tests::long_hex`

**Status:** done — `0x80000000` to `0xFFFFFFFF` are read as two's complement
(`0x80000000` = -2147483648); `0x0` to `0x7FFFFFFF` as positive values.

### §7.1.4 Table 7.1 long: LENGTH_UNLIMITED

**Spec:** §7.1.4, Table 7.1, p. 16 — "or LENGTH_UNLIMITED."

**Repo:** `crates/xml/src/types.rs::parse_long`

**Tests:** `crates/xml/src/types.rs::tests::long_length_unlimited_symbol`,
`crates/xml/tests/qos_profile.rs::t11_length_unlimited_in_resource_limits`

**Status:** done — the name yields -1.

### §7.1.4 Table 7.1 unsigned long: decimal

**Spec:** §7.1.4, Table 7.1, p. 16 — "unsigned long | 0 to 4294967296 [...] |
A 32-bit unsigned integer."

**Repo:** `crates/xml/src/types.rs::parse_ulong`

**Tests:** `crates/xml/src/types.rs::tests::ulong_decimal_and_hex`,
`crates/xml/src/types.rs::tests::ulong_invalid`

**Status:** done — range 0 to 4294967295. The table gives 4294967296 as the
upper bound; that is 2^32 and does not fit into 32 unsigned bits (computed).
`ulong_invalid` rejects 4294967296. In the QoS IDL only `Duration_t.nanosec`
has the type `unsigned long`; the item on §7.2.6 applies to it.

### §7.1.4 Table 7.1 unsigned long: hexadecimal

**Spec:** §7.1.4, Table 7.1, p. 16 — "or 0 to 0xffffffff."

**Repo:** `crates/xml/src/types.rs::parse_ulong`

**Tests:** `crates/xml/src/types.rs::tests::ulong_decimal_and_hex`

**Status:** done

### §7.1.4 Table 7.1 string: escaping according to [XML]

**Spec:** §7.1.4, Table 7.1, p. 16 — "string | The string with the reserved
XML characters escaped according to the standard rules for element content
[XML]. | Per the XML rules only < and & are required to be escaped within an
element content. The characters >, ', and " may be escaped."

**Repo:** `crates/xml/src/parser.rs::build_element`; `crates/xml/src/sample.rs::xml_escape`

**Tests:** `crates/xml/tests/types_and_samples.rs::t24_sample_string_escaping_roundtrip`,
`crates/xml/tests/schema_validation.rs::serialize_sample_output_validates`

**Status:** done — escaped characters and character references are resolved by `roxmltree`; the writer escapes `<`, `>`, `&`, `"`, `'` and every character above U+007F. The reader does not change the whitespace of a string (item below).

### §7.1.4 Table 7.1 string: white space and comments in the value

**Spec:** §7.1.4, Table 7.1, p. 16 — "The string with the reserved XML
characters escaped".

**Repo:** `crates/xml/src/parser.rs::build_element`; `crates/xml/src/sample.rs::parse_primitive_value`

**Tests:** `crates/xml/src/parser.rs::tests::element_text_preserves_whitespace`, `crates/xml/src/parser.rs::tests::element_text_joins_runs_across_comments`

**Status:** done — `build_element` stores the text of an element unchanged, with its whitespace at the edges; runs of text around a comment are joined (`<a>x<!-- c -->y</a>` is `xy`). A string in Data Samples keeps its whitespace (`PrimitiveValue::Str`). Numbers and names in QoS are trimmed when read. Commit `8771c676858`.

---

## §7.1.5 XML Attribute Values

### §7.1.5 Table 7.2 boolean: yes as `1` or `true`

**Spec:** §7.1.5, Table 7.2, p. 17 — "boolean | Yes: 1 or true. | Values are
case sensitive."

**Repo:** `crates/xml/src/xtypes_parser.rs::opt_bool` (via `qos_parser::parse_bool_strict`) for `key`, `optional`, `mustUnderstand`, `nested`, `external`, `nonSerialized`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::spec_7_1_5_1_boolean_attribute_is_case_sensitive`

**Status:** done — the building blocks QoS to Applications have no attribute of type `boolean` in the PDF; the attributes in `<types>` follow [DDS-XTYPES] and are read with `parse_bool_strict` (`True` is rejected).

### §7.1.5 Table 7.2 boolean: no as `0` or `false`

**Spec:** §7.1.5, Table 7.2, p. 17 — "No: 0 or false. | Values are case
sensitive."

**Repo:** `crates/xml/src/xtypes_parser.rs::opt_bool`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::spec_7_1_5_1_boolean_attribute_is_case_sensitive`

**Status:** done — as the item for `1` or `true`.

### §7.1.5 Table 7.2 enum: string

**Spec:** §7.1.5, Table 7.2, p. 17 — "enum | A string. Legal values are the
ones defined for QoS Policies in the DCPS IDL of DDS specification [DDS]. |
Must be specified as a string. (Do not use numeric values.)"

**Repo:** `crates/xml/src/xtypes_parser.rs::extensibility_attr`, `parse_try_construct`, `parse_auto_id`, `parse_forward_dcl`

**Tests:** `crates/xml/tests/types_and_samples.rs::t03_extensibility_final_appendable_mutable`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`

**Status:** done — the PDF names no attribute whose value is a literal of the QoS IDL. The attributes with enumeration values in `<types>` (`extensibility`, `tryConstruct`, `autoid`, `kind` of `forward_dcl`) are read as strings and checked against the XSD's values; spellings outside the XSD are vendor forms (`extensibility="appendable"`, `kind="STRUCT"`). Commits `32aa062decf`, `af517d979de`, `e34118cc9a6`.

### §7.1.5 Table 7.2 enum: spelling and numbers

**Spec:** §7.1.5, Table 7.2, p. 17 — "(Do not use numeric values.)"

**Repo:** `crates/xml/src/xtypes_parser.rs::extensibility_attr`, `parse_forward_dcl`

**Tests:** `crates/xml/tests/types_and_samples.rs::t03_extensibility_uppercase_rejected`, `crates/xml/tests/spec_items.rs::a_number_is_no_extensibility`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`

**Status:** done — `FINAL`, `APPENDABLE`, `MUTABLE` and a number end with `XmlError::BadEnum`; `appendable` is a vendor form of `append`; `kind` of `forward_dcl` is checked against `enum`, `struct`, `union`, `bitmask`. Commits `d2c3d3a9bef`, `af517d979de`, `e34118cc9a6`.

### §7.1.5 Table 7.2 long

**Spec:** §7.1.5, Table 7.2, p. 17 — "long | -2147483648 to 2147483647 or
0x80000000 to 0x7fffffff or LENGTH_UNLIMITED. | A 32-bit signed integer."

**Repo:** `crates/xml/src/types.rs::parse_long`, for the `value`
attribute of `<enumerator>` (`crates/xml/src/xtypes_parser.rs`)

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::spec_7_1_5_3_long_attribute_takes_decimal_hex_and_length_unlimited`

**Status:** done

### §7.1.5 Table 7.2 unsigned long

**Spec:** §7.1.5, Table 7.2, p. 17 — "unsigned long | 0 to 4294967296 or 0 to
0xffffffff. | A 32-bit unsigned integer."

**Repo:** `crates/xml/src/types.rs::parse_ulong`, for `domain_id` (`crates/xml/src/domain.rs::parse_domain_element`, `crates/xml/src/participant.rs::parse_dp_element`) and for `id` and `bitBound` in `<types>`

**Tests:** `crates/xml/src/domain.rs::tests::domain_id_accepts_the_unsigned_long_range`, `crates/xml/src/types.rs::tests::ulong_decimal_and_hex`

**Status:** done — `domain_id` and `id` and `bitBound` in `<types>` are read with `parse_ulong` (decimal or hexadecimal, up to 4294967295). The limit 232 for `domain_id` is gone: it comes from RTPS, not DDS-XML. Commit `80ea5f7343e`.

### §7.1.5 Table 7.2 string

**Spec:** §7.1.5, Table 7.2, p. 17 — "string | The string with the reserved
XML characters escaped according to the standard rules for element content
[XML]."

**Repo:** `crates/xml/src/parser.rs::build_element`

**Tests:** `crates/xml/tests/spec_items.rs::attribute_values_keep_their_characters_and_spaces`, `crates/xml/src/parser.rs::tests::attributes_preserved`

**Status:** done — `build_element` takes attribute values as `roxmltree` returns them after resolving entities and character references, without trimming. The attributes of the `xsi` and `xml` namespaces are no content and are left out. Commit `8bf46985f11`.

---

## §7.2 XML Representation of Resources Defined in the DDS IDL PSM

### §7.2 1-to-1 mapping of the DDS IDL types

**Spec:** §7.2, p. 17 — "The XML representation of resources that correspond
to data-types defined in the DDS IDL PSM [DDS] is obtained by performing a
1-to-1 mapping of the corresponding IDL data type."

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_qos`; `crates/xml/src/qos.rs::EntityQos`; `crates/xml/src/conformance.rs::IDL_TO_XML_MAPPING`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`, `crates/xml/tests/qos_profile.rs::t09b_policy_outside_the_container_is_refused`, `crates/xml/src/conformance.rs::tests::idl_mapping_names_the_sections_of_7_2`

**Status:** done — the 22 policies of DDS 1.4 are read under the IDL's names, per container only the policies that are a field of the IDL structure; the enumerations, constants, sequences and `Duration_t` are in the items below. `IDL_TO_XML_MAPPING` names the function per category. Commits `d2c3d3a9bef`, `8771c676858`.

---

## §7.2.1 XML Representation of Enumeration types

### §7.2.1 Enumeration as a simpleType in the XSD

**Spec:** §7.2.1, p. 17 — "IDL Enumerations are represented in XML according
to a schema defined as an XSD simpleType defined as a restriction of a string
that can take values of the enumeration literals."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`

**Tests:** `crates/xml/tests/schema_validation.rs::xsd_valid_documents_are_read`

**Status:** done — the OMG's XSD defines the simpleTypes of the enumerations (`durabilityKind`, `historyKind`, …) and is in the tree unchanged; the reader checks the literals itself. Commit `8bf46985f11`.

### §7.2.1 DurabilityQosPolicyKind

**Spec:** §7.2.1, p. 17; DDS 1.4 IDL, p. 157 — `VOLATILE_DURABILITY_QOS`,
`TRANSIENT_LOCAL_DURABILITY_QOS`, `TRANSIENT_DURABILITY_QOS`,
`PERSISTENT_DURABILITY_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_durability`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`

**Status:** done — `parse_durability` reads `VOLATILE_DURABILITY_QOS`, `TRANSIENT_LOCAL_DURABILITY_QOS`, `TRANSIENT_DURABILITY_QOS`, `PERSISTENT_DURABILITY_QOS`; `every_literal_of_the_dcps_enumerations_is_read` reads each. Commit `d2c3d3a9bef`.

### §7.2.1 PresentationQosPolicyAccessScopeKind

**Spec:** §7.2.1, p. 17; DDS 1.4 IDL, p. 158 — `INSTANCE_PRESENTATION_QOS`,
`TOPIC_PRESENTATION_QOS`, `GROUP_PRESENTATION_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_presentation`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`

**Status:** done — `parse_presentation` reads `INSTANCE_PRESENTATION_QOS`, `TOPIC_PRESENTATION_QOS`, `GROUP_PRESENTATION_QOS`; `every_literal_of_the_dcps_enumerations_is_read` reads each. Commit `d2c3d3a9bef`.

### §7.2.1 OwnershipQosPolicyKind

**Spec:** §7.2.1, p. 17; DDS 1.4 IDL, p. 158 — `SHARED_OWNERSHIP_QOS`,
`EXCLUSIVE_OWNERSHIP_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_ownership`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`

**Status:** done — `parse_ownership` reads `SHARED_OWNERSHIP_QOS`, `EXCLUSIVE_OWNERSHIP_QOS`; `every_literal_of_the_dcps_enumerations_is_read` reads each. Commit `d2c3d3a9bef`.

### §7.2.1 LivelinessQosPolicyKind

**Spec:** §7.2.1, p. 17; DDS 1.4 IDL, p. 158 — `AUTOMATIC_LIVELINESS_QOS`,
`MANUAL_BY_PARTICIPANT_LIVELINESS_QOS`, `MANUAL_BY_TOPIC_LIVELINESS_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_liveliness`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`

**Status:** done — `parse_liveliness` reads `AUTOMATIC_LIVELINESS_QOS`, `MANUAL_BY_PARTICIPANT_LIVELINESS_QOS`, `MANUAL_BY_TOPIC_LIVELINESS_QOS`; `every_literal_of_the_dcps_enumerations_is_read` reads each. Commit `d2c3d3a9bef`.

### §7.2.1 ReliabilityQosPolicyKind

**Spec:** §7.2.1, p. 17; DDS 1.4 IDL, p. 158–159 —
`BEST_EFFORT_RELIABILITY_QOS`, `RELIABLE_RELIABILITY_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_reliability`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`, `crates/xml/tests/qos_profile.rs::t19_empty_policies_use_spec_defaults`, `crates/xml/src/registry.rs::tests::resolves_base_writer_qos`

**Status:** done — `parse_reliability` reads both literals; a test reads each. Without `<kind>` the DDS default applies: `RELIABLE` at a DataWriter, otherwise `BEST_EFFORT` (DDS 1.4 §2.2.3.14). Commits `d2c3d3a9bef`, `b956dbaceed`.

### §7.2.1 ReliabilityQosPolicyKind: BEST_EFFORT_RELIABILITY_QOS

**Spec:** §7.2.1, p. 17; DDS 1.4 IDL, p. 158 — `BEST_EFFORT_RELIABILITY_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_reliability`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`, `crates/xml/tests/qos_profile.rs::t18_into_writer_and_reader_qos_use_spec_defaults`

**Status:** done — `parse_reliability` reads the literal; `every_literal_of_the_dcps_enumerations_is_read` and `t18_into_writer_and_reader_qos_use_spec_defaults` read it.

### §7.2.1 DestinationOrderQosPolicyKind

**Spec:** §7.2.1, p. 17; DDS 1.4 IDL, p. 159 —
`BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS`,
`BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_destination_order`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`

**Status:** done — `parse_destination_order` reads `BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS`, `BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS`; `every_literal_of_the_dcps_enumerations_is_read` reads each. Commit `d2c3d3a9bef`.

### §7.2.1 HistoryQosPolicyKind

**Spec:** §7.2.1, p. 17; DDS 1.4 IDL, p. 159 — `KEEP_LAST_HISTORY_QOS`,
`KEEP_ALL_HISTORY_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_history`; `crates/xml/src/qos_parser.rs::parse_durability_service`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`, `crates/xml/src/registry.rs::tests::resolves_base_writer_qos`

**Status:** done — `parse_history` and `history_kind` of `parse_durability_service` read both literals; `every_literal_of_the_dcps_enumerations_is_read` reads each at both places. Commit `d2c3d3a9bef`.

### §7.2.1 HistoryQosPolicyKind: KEEP_ALL_HISTORY_QOS

**Spec:** §7.2.1, p. 17; DDS 1.4 IDL, p. 159 — `KEEP_ALL_HISTORY_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_history`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`, `crates/xml/tests/spec_items.rs::qos_library_example`

**Status:** done — `parse_history` reads the literal; `every_literal_of_the_dcps_enumerations_is_read`, `t21_repeated_containers_select_by_topic_filter` and `qos_library_example` read it.

### §7.2.1.1 Example (Non-normative)

**Spec:** §7.2.1.1, p. 17 — `HistoryQosPolicyKind` as simpleType
`historyKind`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example. The text names the simpleType
`historyQosPolicyKind`, the XSD below it `historyKind`.

---

## §7.2.2 XML Representation of Primitive Constants

### §7.2.2 Constants as simpleType: number or name

**Spec:** §7.2.2, p. 17–18 — "These constant definitions appear in the XSD as
simpleType schemas defined as restrictions of xs:string to provide custom
syntax allowing an element to have a value that is either given as a number
or as the name of the constant."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`; `crates/xml/src/types.rs`

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/src/types.rs::tests::duration_accepts_the_named_constants`

**Status:** done — the four simpleTypes are in the OMG's XSD (`nonNegativeInteger_UNLIMITED`, `positiveInteger_UNLIMITED`, `nonNegativeInteger_Duration_SEC`, `nonNegativeInteger_Duration_NSEC`) and used by the policies' elements. The reader takes a number or a name; `max_samples`, `max_instances` and `max_samples_per_instance` are read as `positiveInteger_UNLIMITED` (1 or more, or the name; commit `2120724498f`); the names the pattern does not list (`DURATION_ZERO_*`, `TIME_INVALID_*`) it accepts because the sentence of §7.2.2 allows "the name of the constant". Commits `8bf46985f11`, `4e479ef9e18`.

### §7.2.2 LENGTH_UNLIMITED = -1

**Spec:** §7.2.2.1, p. 18 — "const long LENGTH_UNLIMITED = -1; [...] The
constant LENGTH_UNLIMITED is intended to initialize structure members that
represent lengths."

**Repo:** `crates/xml/src/types.rs::LENGTH_UNLIMITED`; `crates/xml/src/types.rs::parse_long`

**Tests:** `crates/xml/src/types.rs::tests::long_length_unlimited_symbol`,
`crates/xml/src/types.rs::tests::spec_constants`,
`crates/xml/tests/qos_profile.rs::t11_length_unlimited_in_resource_limits`

**Status:** done — `parse_long` accepts the name at every field of type `long`, also at fields that are no length (e.g. `ownership_strength`); the OMG's XSD has the name only at lengths. The field is a `long` in the IDL, and §7.2.2 allows the name at any.

### §7.2.2 DURATION_INFINITE_SEC = 0x7fffffff

**Spec:** §7.2.2.1, p. 18 — "const long DURATION_INFINITE_SEC = 0x7fffffff;"

**Repo:** `crates/xml/src/types.rs::DURATION_INFINITE_SEC`;
`crates/xml/src/types.rs::parse_duration_sec`

**Tests:** `crates/xml/src/types.rs::tests::duration_sec_infinite_symbols`,
`crates/xml/src/types.rs::tests::spec_constants`

**Status:** done

### §7.2.2 DURATION_INFINITE_NSEC = 0x7fffffff

**Spec:** §7.2.2.1, p. 18 — "const unsigned long DURATION_INFINITE_NSEC =
0x7fffffff;"

**Repo:** `crates/xml/src/types.rs::DURATION_INFINITE_NSEC`;
`crates/xml/src/types.rs::parse_duration_nsec`

**Tests:** `crates/xml/src/types.rs::tests::duration_nsec_infinite`,
`crates/xml/src/types.rs::tests::spec_constants`

**Status:** done

### §7.2.2 DURATION_ZERO_SEC = 0

**Spec:** §7.2.2.1, p. 18 — "const long DURATION_ZERO_SEC = 0;"

**Repo:** `crates/xml/src/types.rs::DURATION_ZERO_SEC`; `crates/xml/src/types.rs::parse_duration_sec`

**Tests:** `crates/xml/src/types.rs::tests::duration_accepts_the_named_constants`, `crates/xml/src/types.rs::tests::duration_constants`

**Status:** done — `parse_duration_sec` accepts the name. Commit `4e479ef9e18`.

### §7.2.2 DURATION_ZERO_NSEC = 0

**Spec:** §7.2.2.1, p. 18 — "const unsigned long DURATION_ZERO_NSEC = 0;"

**Repo:** `crates/xml/src/types.rs::DURATION_ZERO_NSEC`; `crates/xml/src/types.rs::parse_duration_nsec`

**Tests:** `crates/xml/src/types.rs::tests::duration_accepts_the_named_constants`, `crates/xml/src/types.rs::tests::duration_constants`

**Status:** done — `parse_duration_nsec` accepts the name. Commit `4e479ef9e18`.

### §7.2.2 TIME_INVALID_SEC = -1

**Spec:** §7.2.2.1, p. 18 — "const long TIME_INVALID_SEC = -1; [...] the
constants with the prefix TIME_ are intended to initialize the members
(second and nanosecond) of the structure Time_t."

**Repo:** `crates/xml/src/types.rs::TIME_INVALID_SEC`; `crates/xml/src/sample.rs::parse_long_sample`

**Tests:** `crates/xml/src/types.rs::tests::time_invalid_constants`, `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`

**Status:** done — `parse_duration_sec` and, for a field of type `long` in Data Samples, `parse_long_sample` accept the name. The PDF names no element of type `Time_t` in the building blocks QoS to Applications. Commits `4e479ef9e18`, `8771c676858`.

### §7.2.2 TIME_INVALID_NSEC = 0xffffffff

**Spec:** §7.2.2.1, p. 18 — "const unsigned long TIME_INVALID_NSEC =
0xffffffff;"

**Repo:** `crates/xml/src/types.rs::TIME_INVALID_NSEC`; `crates/xml/src/sample.rs::parse_ulong_sample`

**Tests:** `crates/xml/src/types.rs::tests::time_invalid_constants`, `crates/xml/src/sample.rs::tests::parse_sample_ulong_constant_names`, `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`

**Status:** done — `parse_ulong_sample` accepts the name (`unsigned long` in Data Samples). Commit `8771c676858`.

### §7.2.2.1 Example (Non-Normative)

**Spec:** §7.2.2.1, p. 18 — the IDL constants and the simpleTypes
`nonNegativeInteger_UNLIMITED`, `positiveInteger_UNLIMITED`,
`nonNegativeInteger_Duration_SEC`, `nonNegativeInteger_Duration_NSEC`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example. The patterns contain `DURATION_INFINITY`, which is no constant of the IDL; the reader accepts it (`parse_duration_sec`, `parse_duration_nsec`). The patterns allow an empty string; an empty element counts as the default of its field (`empty_numeric_elements_are_the_default`). `max_samples` and its siblings are read as `positiveInteger_UNLIMITED` (`parse_positive_long_unlimited`), `parse_long` reads the other fields of type `long`.

---

## §7.2.3 XML Representation of Structure Types

### §7.2.3 Structure as a complexType in the XSD

**Spec:** §7.2.3, p. 18 — "In general, IDL structures are represented in XML
according to a schema defined as an XSD complexType."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`

**Tests:** `crates/xml/tests/schema_validation.rs::xsd_valid_documents_are_read`

**Status:** done — the OMG's XSD has a complexType per policy (e.g. `partitionQosPolicy`, `durabilityServiceQosPolicy`) and one per container (`datawriterQos`, …); it is in the tree unchanged. Commit `8bf46985f11`.

### §7.2.3 Element name equals field name

**Spec:** §7.2.3, p. 18 — "The fields in an IDL structure become unordered
elements of the complexType with the field name appearing as the
corresponding element name."

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_qos` and the readers per policy; `crates/xml/src/qos_parser.rs::only_children`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`, `crates/xml/src/qos_parser.rs::tests::unread_elements_are_refused_at_every_level`, `crates/xml/tests/normative_forms.rs::report_and_skip_lists_everything_it_leaves_out`

**Status:** done — an unknown child element of a policy or a container ends with `XmlError::UnknownElement` and the element's path; with `ReadOptions::report_and_skip()` it is left out and reported as a `Diagnostic`. Commits `8771c676858`, `8bf46985f11`.

### §7.2.3 Fields unordered

**Spec:** §7.2.3, p. 18 — "become unordered elements of the complexType".

**Repo:** `crates/xml/src/qos_parser.rs::check_children`; `crates/xml/src/parser.rs::XmlElement::child`

**Tests:** `crates/xml/tests/spec_items.rs::fields_of_a_structure_come_in_any_order`, `crates/xml/tests/spec_items.rs::a_field_given_twice_is_refused`, `crates/xml/tests/qos_profile.rs::t20_duplicate_structure_field_rejected`

**Status:** done — the readers look up the children by name, the order does not matter (`fields_of_a_structure_come_in_any_order`); a field that occurs twice ends with `XmlError::InvalidXml`. Commit `8771c676858`.

### §7.2.3 Nested structures

**Spec:** §7.2.3, p. 18 — "This mapping is applied recursively for nested
structures."

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration_element`, called for every field of type `Duration_t`

**Tests:** `crates/xml/tests/qos_profile.rs::t01_happy_path_reliability_and_history_roundtrip`,
`crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 DDS defaults in the XSD

**Spec:** §7.2.3, p. 18 — "If the DDS specification defines default values
for the structure fields, the corresponding XSD element definition shall
provide the same default value."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`

**Status:** done — the OMG's XSD carries the DDS defaults as `default=` of the elements (e.g. `INSTANCE_PRESENTATION_QOS` for `access_scope`) and is in the tree unchanged. Commit `8bf46985f11`.

### §7.2.3 Missing policy: DDS default

**Spec:** §7.2.3, p. 18 — "If the DDS specification defines default values
for the structure fields, the corresponding XSD element definition shall
provide the same default value."

**Repo:** `crates/xml/src/qos.rs::EntityQos::into_writer_qos`, `into_reader_qos`; `crates/webdds-bridge/src/qos_update.rs::topic_qos_from`

**Tests:** `crates/xml/src/qos.rs::tests::entity_qos_into_writer_uses_defaults_for_unset`, `crates/xml/src/qos.rs::tests::entity_qos_into_reader_uses_defaults_for_unset`, `crates/xml/tests/qos_profile.rs::t18_into_writer_and_reader_qos_use_spec_defaults`

**Status:** done — a policy missing from the document stays `None` in `EntityQos` and counts as the DDS default. `into_writer_qos` and `into_reader_qos` apply it (reliability `RELIABLE` at a writer, `BEST_EFFORT` at a reader, history `KEEP_LAST` with depth 1); for Topic, Publisher, Subscriber and DomainParticipant the caller merges `EntityQos` over the entity's default (`webdds-bridge`: `topic_qos_from`, `publisher_qos_from`, `subscriber_qos_from`, `participant_qos_from`). Commit `b956dbaceed`.

### §7.2.3 Missing field of a policy

**Spec:** §7.2.3, p. 18 — defaults of the fields; §7.2.3.1, p. 19 (fields
with `minOccurs="0"` and `default`).

**Repo:** `crates/xml/src/qos_parser.rs`: the readers per policy; `numeric_text`

**Tests:** `crates/xml/tests/qos_profile.rs::t19_empty_policies_use_spec_defaults`, `crates/xml/tests/qos_profile.rs::t19b_duration_without_sec_is_zero`, `crates/xml/tests/normative_forms.rs::empty_numeric_elements_are_the_default`

**Status:** done — a field that is missing has the DDS default; `minOccurs="0"` is at every field of the XSD. A policy without children (`<deadline/>`, `<history/>`) reads as a policy with all defaults, an element with empty text (`<depth/>`) as the default of its field, a `Duration_t` without `sec` as 0. An enumeration without a literal has the DDS default. Commits `4e479ef9e18`, `b956dbaceed`.

### §7.2.3.1 Example (Non-normative)

**Spec:** §7.2.3.1, p. 19 — complexType `historyQosPolicy` with `kind` and
`depth`, both `minOccurs="0"` with `default`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example. It types `depth` as `xs:positiveInteger`; the IDL has `long`. `parse_history` requires 1 or more (`numbers_outside_the_schemas_types_are_refused`).

### §7.2.3 UserDataQosPolicy: `user_data`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 157 — `struct UserDataQosPolicy {
sequence<octet> value; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_user_data`, `parse_octet_value`

**Tests:** `crates/xml/tests/qos_profile.rs::t13_user_data_base64_to_bytes`, `crates/xml/tests/qos_profile.rs::t13b_user_data_comma_list_to_bytes`, `crates/xml/tests/normative_forms.rs::user_data_value_is_base64_and_may_be_empty`

**Status:** done — element `user_data` with `value` (Base64 or list) or `valueB64` per §7.2.4.2 (items there). Commit `5dcde9b5717`.

### §7.2.3 TopicDataQosPolicy: `topic_data`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 157 — `struct TopicDataQosPolicy {
sequence<octet> value; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_topic_data`, `parse_octet_value`

**Tests:** `crates/xml/tests/spec_items.rs::topic_data_and_group_data_hold_octets_in_both_forms`, `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — element `topic_data` with `value` (Base64 or list) or `valueB64` per §7.2.4.2 (items there). Commit `5dcde9b5717`.

### §7.2.3 GroupDataQosPolicy: `group_data`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 157 — `struct GroupDataQosPolicy {
sequence<octet> value; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_group_data`, `parse_octet_value`

**Tests:** `crates/xml/tests/spec_items.rs::topic_data_and_group_data_hold_octets_in_both_forms`, `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — element `group_data` with `value` (Base64 or list) or `valueB64` per §7.2.4.2 (items there). Commit `5dcde9b5717`.

### §7.2.3 TransportPriorityQosPolicy: `transport_priority`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 157 — `struct
TransportPriorityQosPolicy { long value; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_transport_priority`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 LifespanQosPolicy: `lifespan`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 157 — `struct LifespanQosPolicy {
Duration_t duration; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_lifespan`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 DurabilityQosPolicy: `durability`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 157–158 — `struct
DurabilityQosPolicy { DurabilityQosPolicyKind kind; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_durability`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`,
`crates/xml/tests/qos_profile.rs::t04_three_level_inheritance_propagates`

**Status:** done — the literals are in the item on §7.2.1.

### §7.2.3 PresentationQosPolicy: `presentation`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 158 — `access_scope`,
`coherent_access`, `ordered_access`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_presentation`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 DeadlineQosPolicy: `deadline`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 158 — `struct DeadlineQosPolicy {
Duration_t period; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_deadline`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`,
`crates/xml/tests/qos_profile.rs::t10_duration_infinity_string_parses_to_infinite`

**Status:** done

### §7.2.3 LatencyBudgetQosPolicy: `latency_budget`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 158 — `struct
LatencyBudgetQosPolicy { Duration_t duration; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_latency_budget`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 OwnershipQosPolicy: `ownership`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 158 — `struct OwnershipQosPolicy {
OwnershipQosPolicyKind kind; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_ownership`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 OwnershipStrengthQosPolicy: `ownership_strength`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 158 — `struct
OwnershipStrengthQosPolicy { long value; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_ownership_strength`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 LivelinessQosPolicy: `liveliness`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 158 — `kind`, `lease_duration`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_liveliness`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 TimeBasedFilterQosPolicy: `time_based_filter`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 158 — `struct
TimeBasedFilterQosPolicy { Duration_t minimum_separation; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_time_based_filter`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 PartitionQosPolicy: `partition`

**Spec:** §7.2.3, p. 18; §7.2.4.1, p. 19; DDS 1.4 IDL, p. 158 — `struct
PartitionQosPolicy { StringSeq name; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_partition`

**Tests:** `crates/xml/tests/qos_profile.rs::t12_partition_multiple_names_become_vec`, `crates/xml/tests/normative_forms.rs::empty_partition_name_is_an_empty_sequence`

**Status:** done — `parse_partition` reads the sequence per §7.2.4.1: one `<name>` with zero or more `<element>`. A second `<name>` ends with `XmlError::InvalidXml`; the form with a repeated `<name>` and the one with `<name_list>` are no forms of the spec. Commits `5dcde9b5717`, `7efd1017dd5`.

### §7.2.3 ReliabilityQosPolicy: `reliability`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 159 — `kind`, `max_blocking_time`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_reliability`

**Tests:** `crates/xml/tests/qos_profile.rs::t01_happy_path_reliability_and_history_roundtrip`,
`crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 DestinationOrderQosPolicy: `destination_order`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 159 — `struct
DestinationOrderQosPolicy { DestinationOrderQosPolicyKind kind; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_destination_order`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 HistoryQosPolicy: `history`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 159 — `kind`, `depth`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_history`

**Tests:** `crates/xml/tests/qos_profile.rs::t01_happy_path_reliability_and_history_roundtrip`,
`crates/xml/tests/qos_profile.rs::t03_inheritance_child_overrides_history_keeps_reliability`

**Status:** done

### §7.2.3 ResourceLimitsQosPolicy: `resource_limits`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 159 — `max_samples`,
`max_instances`, `max_samples_per_instance`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_resource_limits`; `crates/xml/src/qos.rs::InitialResourceLimits`

**Tests:** `crates/xml/tests/normative_forms.rs::resource_limits_initial_fields_are_carried`, `crates/xml/tests/qos_profile.rs::t11_length_unlimited_in_resource_limits`

**Status:** done — `initial_samples` and `initial_instances` of the example in §7.3.2.4.3.1 are in the OMG's XSD (`resourceLimitsQosPolicy`) and are read and carried in `EntityQos::initial_resource_limits`; the DDS 1.4 IDL does not have them, so they reach neither `WriterQos` nor `ReaderQos`. Commit `8bf46985f11`.

### §7.2.3 EntityFactoryQosPolicy: `entity_factory`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 159 — `struct
EntityFactoryQosPolicy { boolean autoenable_created_entities; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_factory`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`,
`crates/xml/tests/qos_profile.rs::t14_boolean_case_sensitive_strict`

**Status:** done

### §7.2.3 WriterDataLifecycleQosPolicy: `writer_data_lifecycle`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 159 — `struct
WriterDataLifecycleQosPolicy { boolean autodispose_unregistered_instances;
};`

**Repo:** `crates/xml/src/qos_parser.rs::parse_writer_data_lifecycle`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 ReaderDataLifecycleQosPolicy: `reader_data_lifecycle`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 159 —
`autopurge_nowriter_samples_delay`, `autopurge_disposed_samples_delay`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_reader_data_lifecycle`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 DurabilityServiceQosPolicy: `durability_service`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 159 — `service_cleanup_delay`,
`history_kind`, `history_depth`, `max_samples`, `max_instances`,
`max_samples_per_instance`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_durability_service`

**Tests:** `crates/xml/tests/spec_items.rs::durability_service_reads_all_its_fields`, `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — `parse_durability_service` reads all six fields; `durability_service_reads_all_its_fields` reads them and checks four, `t09_all_22_policies_parse` reads the policy with the duration.

### §7.2.3 DurabilityServiceQosPolicy: `max_instances`, `max_samples_per_instance`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 159 — `long max_instances; long
max_samples_per_instance;`

**Repo:** `crates/xml/src/qos_parser.rs::parse_durability_service`

**Tests:** `crates/xml/tests/spec_items.rs::durability_service_reads_all_its_fields`

**Status:** done — `parse_durability_service` reads both fields; `durability_service_reads_all_its_fields` checks the values.

### §7.2.3 DomainParticipantQos: `domain_participant_qos`

**Spec:** §7.2.3, p. 18; §7.3.5.4.4.1, p. 27; DDS 1.4 IDL, p. 160 —
`user_data`, `entity_factory`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`, `parse_single_qos_shortcut`; `crates/xml/src/participant.rs::parse_dp_element`

**Tests:** `crates/xml/tests/dds_xml_full.rs::participant_inheritance_one_level`, `crates/xml/tests/qos_profile.rs::t02_all_six_containers_in_a_profile`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`

**Status:** done — `domain_participant_qos` is read, as the OMG's XSD writes it; `domainparticipant_qos` and `participant_qos` are vendor forms (`VendorForms`). Commits `e34118cc9a6`, `2de529e05a6`.

### §7.2.3 TopicQos: `topic_qos`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 160 — the 13 policies of
`TopicQos`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`; `crates/xml/src/domain.rs::parse_topic_entry`

**Tests:** `crates/xml/tests/qos_profile.rs::t02_all_six_containers_in_a_profile`,
`crates/xml/tests/dds_xml_full.rs::topic_qos_inline_and_profile_ref_both_resolve`

**Status:** done

### §7.2.3 DataWriterQos: `datawriter_qos`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 160 — the 15 policies of
`DataWriterQos`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`; `crates/xml/src/qos.rs::into_writer_qos`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`,
`crates/xml/tests/qos_profile.rs::t18_into_writer_and_reader_qos_use_spec_defaults`

**Status:** done

### §7.2.3 PublisherQos: `publisher_qos`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 160 — `presentation`, `partition`,
`group_data`, `entity_factory`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`; `crates/xml/src/participant.rs::parse_publisher_element`

**Tests:** `crates/xml/tests/qos_profile.rs::t02_all_six_containers_in_a_profile`,
`crates/xml/tests/qos_profile.rs::t12_partition_multiple_names_become_vec`

**Status:** done

### §7.2.3 DataReaderQos: `datareader_qos`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 160–161 — the 12 policies of
`DataReaderQos`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`; `crates/xml/src/qos.rs::into_reader_qos`

**Tests:** `crates/xml/tests/qos_profile.rs::t04_three_level_inheritance_propagates`,
`crates/xml/src/registry.rs::tests::reader_qos_resolves`

**Status:** done

### §7.2.3 SubscriberQos: `subscriber_qos`

**Spec:** §7.2.3, p. 18; DDS 1.4 IDL, p. 161 — `presentation`, `partition`,
`group_data`, `entity_factory`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`; `crates/xml/src/participant.rs::parse_subscriber_element`

**Tests:** `crates/xml/tests/qos_profile.rs::t02_all_six_containers_in_a_profile`

**Status:** done

### §7.2.3 Policies outside the IDL of a QoS structure

**Spec:** §7.2, p. 17 (1-to-1 mapping); DDS 1.4 IDL, p. 160–161.

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_qos`, `allowed_policies`

**Tests:** `crates/xml/tests/qos_profile.rs::t09b_policy_outside_the_container_is_refused`, `crates/xml/tests/normative_forms.rs::report_and_skip_lists_everything_it_leaves_out`

**Status:** done — `parse_entity_qos` reads per container only the policies that are a field of the structure in DDS 1.4 §2.2.3 (`allowed_policies`); any other (e.g. `ownership_strength` in `datareader_qos`) ends with `XmlError::UnknownElement`, under `report_and_skip` as `DiagnosticKind::MisplacedElement`. Nothing is read and then dropped when the QoS is taken over. Commits `8771c676858`, `8bf46985f11`.

---

## §7.2.4 XML Representation of Sequences

### §7.2.4.1 Sequence: zero or more `element` elements

**Spec:** §7.2.4.1, p. 19 — "The general XML representation of IDL sequences
is done following a schema defined as an XSD complexType. The complexType
contains zero or more elements named element. Nested inside each element is
the XSD schema obtained from mapping the IDL type of the element itself."

**Repo:** `crates/xml/src/qos_parser.rs::parse_partition`; `crates/xml/src/parser.rs::XmlElement::sequence_elements`

**Tests:** `crates/xml/tests/qos_profile.rs::t12_partition_multiple_names_become_vec`, `crates/xml/tests/normative_forms.rs::empty_partition_name_is_an_empty_sequence`, `crates/xml/src/parser.rs::tests::sequence_elements_iterates_element_tag_children`

**Status:** done — the only sequence in the QoS policies is `PartitionQosPolicy.name` (`StringSeq`); `parse_partition` reads it as `<name>` with `<element>` children. `XmlElement::sequence_elements` returns the `element` children for `webdds-bridge`. Commit `5dcde9b5717`.

### §7.2.4.1.1 Example (Non-normative)

**Spec:** §7.2.4.1.1, p. 19–20 — `QosPolicyCountSeq` with `element`
elements.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example. It types the fields as
`xs:Integer`; XSD has `xs:integer`.

### §7.2.4.2 Octet sequence as a comma-separated list in `value`

**Spec:** §7.2.4.2, p. 20 — "As a special case, sequences of octets are
represented either as a comma-separated list of the value of each octet
represented in decimal or hexadecimal, or alternatively using Base64 binary.
The two options are differentiated by the element name. The latter having
the suffix B64."

**Repo:** `crates/xml/src/qos_parser.rs::parse_octet_value`, `parse_octet_value_text`

**Tests:** `crates/xml/src/qos_parser.rs::tests::octet_value_is_base64_or_a_list`, `crates/xml/tests/qos_profile.rs::t13b_user_data_comma_list_to_bytes`

**Status:** done — `parse_octet_value_text` reads `<value>` as a list when the text is a list of octets (decimal or `0x…`, each 0 to 255); otherwise as Base64, as the OMG's XSD types `value` (`xs:base64Binary`). A digit string that is no list stays Base64. Commits `5dcde9b5717`, `7efd1017dd5`.

### §7.2.4.2 Octet sequence as Base64 in `valueB64`

**Spec:** §7.2.4.2, p. 20 — "alternatively using Base64 binary [...] The
latter having the suffix B64."

**Repo:** `crates/xml/src/qos_parser.rs::parse_octet_value`, `decode_b64_text`

**Tests:** `crates/xml/tests/qos_profile.rs::t13_user_data_base64_to_bytes`, `crates/xml/tests/qos_profile.rs::t13c_user_data_both_forms_rejected`, `crates/xml/tests/spec_items.rs::topic_data_and_group_data_hold_octets_in_both_forms`

**Status:** done — `valueB64` is read; `value` and `valueB64` together end with `XmlError::InvalidXml`. Commit `5dcde9b5717`.

### §7.2.4.2 Choice by element name

**Spec:** §7.2.4.2, p. 20 — "The two options are differentiated by the
element name."

**Repo:** `crates/xml/src/qos_parser.rs::parse_octet_value`

**Tests:** `crates/xml/tests/qos_profile.rs::t13_user_data_base64_to_bytes`, `crates/xml/tests/qos_profile.rs::t13b_user_data_comma_list_to_bytes`

**Status:** done — `parse_octet_value` tells `value` and `valueB64` apart by the element's name.

### §7.2.4.2.1 Example (Non-normative)

**Spec:** §7.2.4.2.1, p. 20 — simpleType `ddsBinary`, complexType
`userDataQosPolicy` with an `xs:choice` of `value` and `valueB64`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example. The name is `"ddsBinary "` with a
space; the pattern `[0-2]?[0-9]?[0-9]` admits 256 to 299 (checked with Python
`re` as an approximation of the XSD regex, measured); the example documents
close with `<user_data_qos>` instead of `</user_data_qos>` and are not
well-formed; the element is named `user_data_qos`, the IDL field `user_data`.
The two alternatives do not represent the same value: the list yields
"The quic", `VGhlJTIwcXVpY2sl` yields "The%20quick%" (decoded with Python,
measured).

---

## §7.2.5 XML Representation of Arrays

### §7.2.5 Arrays like sequences

**Spec:** §7.2.5, p. 20 — "The XML representation of IDL arrays is the same
as it would be for IDL sequences of the same element type."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no type of the DDS IDL that DDS-XML maps
(QoS policies, QoS structures, `Duration_t`) contains an array; the rule has
no application in the building blocks QoS to Applications. For data samples
§7.3.7.3.2 applies.

---

## §7.2.6 XML Representation of Duration

### §7.2.6 Duration_t with `sec` and `nanosec`

**Spec:** §7.2.6, p. 20–21 — "The IDL structure Duration_t is represented in
XML following the general rules for structures defined in sub clause 7.2.3,
except that the schema provides the option to use the symbolic defined in the
IDL to set the values of the intended elements." complexType `duration` with
`sec` (`nonNegativeInteger_Duration_SEC`) and `nanosec`
(`nonNegativeInteger_Duration_NSEC`), both `minOccurs="0"`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration_element`; `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd` (complexType `duration`)

**Tests:** `crates/xml/tests/qos_profile.rs::t01_happy_path_reliability_and_history_roundtrip`,
`crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — the OMG's XSD reproduces the complexType of p. 21 (`xs:all`, both `minOccurs="0"`) and is in the tree unchanged. The names of the constants are in the items under §7.2.2. `<period>DURATION_INFINITY</period>` without children is a vendor form. Commits `8bf46985f11`, `e34118cc9a6`.

### §7.2.6 `nanosec` optional

**Spec:** §7.2.6, p. 21 — `<xs:element name="nanosec" [...] minOccurs="0"
/>`

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration_element`

**Tests:** `crates/xml/tests/qos_profile.rs::t10_duration_infinity_string_parses_to_infinite`,
`crates/xml/src/qos_parser.rs::tests::duration_inline_infinity_sentinel`

**Status:** done — without `nanosec` the value is 0.

### §7.2.6 `sec` optional

**Spec:** §7.2.6, p. 21 — `<xs:element name="sec" [...] minOccurs="0" />`

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration_element`

**Tests:** `crates/xml/tests/qos_profile.rs::t19b_duration_without_sec_is_zero`

**Status:** done — without `sec` the value is 0 (the DDS default of `Duration_t.sec`). Commit `4e479ef9e18`.

### §7.2.6 Infinite duration

**Spec:** §7.2.6, p. 20; §7.2.2.1, p. 18 — `DURATION_INFINITE_SEC` and
`DURATION_INFINITE_NSEC` for the fields of `Duration_t`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration_element`

**Tests:** `crates/xml/tests/qos_profile.rs::t10_duration_infinity_string_parses_to_infinite`, `crates/xml/tests/qos_profile.rs::t10b_duration_infinity_numeric_parses_to_infinite`

**Status:** done — `sec` = 2147483647 with `nanosec` = `DURATION_INFINITE_NSEC` or without `nanosec` gives the infinite duration of `zerodds-qos`. `<sec>DURATION_INFINITE_SEC</sec><nanosec>0</nanosec>` gives 2147483647 s.

### §7.2.6 `nanosec` = 2147483647 as a number

**Spec:** §7.2.2.1, p. 18 — pattern `(DURATION_INFINITY|DURATION_INFINITE_NSEC|([0-9])*)?`
for `nanosec`.

**Repo:** `crates/xml/src/types.rs::parse_duration_nsec`; `crates/xml/src/qos_parser.rs::parse_duration_element`

**Tests:** `crates/xml/src/types.rs::tests::duration_nsec_accepts_the_full_unsigned_long_range`, `crates/xml/tests/qos_profile.rs::t10b_duration_infinity_numeric_parses_to_infinite`

**Status:** done — `parse_duration_nsec` takes every number of the `unsigned long` range, also 2147483647; `<sec>2147483647</sec><nanosec>2147483647</nanosec>` is the infinite duration. Commit `4e479ef9e18`.

### §7.2.6.1 Example (Non-normative)

**Spec:** §7.2.6.1, p. 21 — `<sec>DURATION_INFINITY</sec>`,
`<nanosec>DURATION_INFINITY_NSEC</nanosec>`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example. `DURATION_INFINITY_NSEC` matches
neither the pattern of p. 18 nor an IDL constant; `parse_duration_nsec`
rejects it.

---

## §7.3.1 Overview

### §7.3.1 Six building blocks

**Spec:** §7.3.1, p. 21 — "This specification breaks the syntax used to
represent DDS resources in XML into the six different building blocks as
shown in Figure 7.1: Building Block QoS, Building Block Types, Building Block
Domains, Building Block DomainParticipants, Building Block Applications,
Building Block Data Samples".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure; the building blocks are in the
items on §7.3.2 to §7.3.7.

### §7.3.1 Figure 7.1 Relationship between building blocks

**Spec:** §7.3.1, Figure 7.1, p. 22 — package diagram with BBQos, BBTypes,
BBDomains, BBDataSamples, BBDomainParticipants, BBApplications.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — figure; the dependencies are in the items
on §7.3.2.2 to §7.3.7.2.

### §7.3.1 Two schema files following the chameleon pattern

**Spec:** §7.3.1, p. 22 — "Each of these building blocks is associated with
two normative schema files in XSD format that are designed according to the
XML Chameleon Schema Definition pattern (see sub clause 7.1.3)."

**Repo:** `crates/xml/src/schemas.rs::ALL_SCHEMAS`

**Tests:** `crates/xml/src/schemas.rs::tests::all_schemas_includes_six_building_blocks_plus_system`,
`crates/xml/src/schemas.rs::tests::nonamespace_xsds_omit_target_namespace`

**Status:** done — for Data Samples §7.3.7.3.1 says no generic XSD can exist, and §7.3.7.3 names no file; ZeroDDS ships its own pair for this building block, in the same pattern.

### §7.3.1 `_nonamespace.xsd`: types without targetNamespace and without root element

**Spec:** §7.3.1, p. 22 — "dds-xml_<building_block_name>_definitions_nonamespace.xsd
contains the type declarations for all the constructs the building block
defines. This XSD file specifies neither a targetNamespace nor a root
element."

**Repo:** `crates/xml/schemas/` (seven files `*_nonamespace.xsd`)

**Tests:** `crates/xml/src/schemas.rs::tests::nonamespace_xsds_omit_target_namespace`, `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`

**Status:** done — none of the seven chameleon files has a `targetNamespace` or a global element; the Data Samples pair now carries its root elements in the namespaced file. Commits `8bf46985f11`, `a45564ecb6b`.

### §7.3.1 Root elements in the chameleon file for Data Samples

**Spec:** §7.3.1, p. 22 — "This XSD file specifies neither a targetNamespace
nor a root element."

**Repo:** `crates/xml/schemas/dds-xml_data_samples_definitions.xsd`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`, `crates/xml/tests/schema_validation.rs::serialize_sample_output_validates`

**Status:** done — `dds-xml_data_samples_definitions_nonamespace.xsd` declares no root element; `<data>` and `<sample>` are in `dds-xml_data_samples_definitions.xsd`, and `serialize_sample_output_validates` checks the output of `serialize_sample` in the namespace against that file. Commit `a45564ecb6b`.

### §7.3.1 `_definitions.xsd`: includes, root element, targetNamespace

**Spec:** §7.3.1, p. 22 — "dds-xml_<building_block_name>_definitions.xsd
includes the XSD with no targetNamespace, defines the top level element for
the building block, and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/` (seven files `*_definitions.xsd`)

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_include_target_namespace`,
`crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`

**Status:** done — each file includes its chameleon file with `xs:include`, declares the root element and sets the `targetNamespace`; so does the Data Samples file.

### §7.3.1 Documents in the namespace http://www.omg.org/spec/DDS-XML

**Spec:** §7.3.1, p. 22 — "sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/src/parser.rs::DDS_XML_NS`, `library_elements`; `crates/xml/src/xsd_loader.rs::load_type_libraries_from_string`

**Tests:** `crates/xml/src/parser.rs::tests::parse_namespace_aware`, `crates/xml/src/xsd_loader.rs::tests::strict_mode_ignores_the_namespace_string_outside_a_declaration`, `crates/xml/tests/dds_xml_full.rs::all_four_libraries_roundtrip`

**Status:** done — the readers compare local names: a document in the spec's namespace, one without a namespace (chameleon pattern, §7.1.3) and one in the user's own namespace are read alike. `ValidationMode::Strict` in `load_type_libraries_from_string` requires the namespace declaration at the root element; it is not found anywhere in the text. Commit `80ea5f7343e`.

---

## §7.3.2 Building Block QoS

### §7.3.2.1 Purpose

**Spec:** §7.3.2.1, p. 22 — "This building block defines the syntax to
represent DDS QoS in XML."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — purpose; the syntax is in the items on §7.2
and §7.3.2.3 to §7.3.2.4.4.

### §7.3.2.2 No dependencies on other building blocks

**Spec:** §7.3.2.2, p. 22 — "This building block has no dependencies on other
building blocks."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd` (includes no other file); `crates/xml/src/qos_parser.rs::parse_qos_libraries`

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`,
`crates/xml/tests/dds_xml_full.rs::independent_building_blocks_parse_without_the_other_blocks`

**Status:** done

### §7.3.2.3 Types of all QoS in `dds-xml_qos_definitions_nonamespace.xsd`

**Spec:** §7.3.2.3, p. 23 — "dds-xml_qos_definitions_nonamespace.xsd contains
the XSD type definition for all the DDS QoS. It defines no targetNamespace so
that it can be reused by other schemas following the XML Chameleon Schema
Definition pattern."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/tests/schema_validation.rs::xsd_valid_documents_are_read`

**Status:** done — the OMG's file is in the tree unchanged and defines all policies, fields, enumerations and containers. Commit `8bf46985f11`.

### §7.3.2.3 `<qos_library>` as top-level element in the namespace

**Spec:** §7.3.2.3, p. 23 — "dds-xml_qos_definitions.xsd defines the
<qos_library> top-level element, and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions.xsd`;
`crates/xml/src/parser.rs::library_elements`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`,
`crates/xml/tests/schema_validation.rs::topic_filter_attribute_is_read`,
`crates/xml/tests/omg_example_documents.rs::qos_example`

**Status:** done — the readers take `<qos_library>` as root and as a child
of `<dds>`. `topic_filter_attribute_is_read` does not need `xmllint`;
`qos_example` needs the OMG's files.

### §7.3.2.4.1 QoS libraries with QoS profiles

**Spec:** §7.3.2.4.1, p. 23 — "QoS Libraries are the top level element of the
Building Block QoS. They are collections of QoS profiles, which group a set
of related QoS—usually one per entity."

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_libraries`, `parse_qos_library_element`, `parse_qos_profile_element`

**Tests:** `crates/xml/src/qos_parser.rs::tests::parse_minimal_library`,
`crates/xml/src/qos_parser.rs::tests::missing_library_name_rejected`,
`crates/xml/src/qos_parser.rs::tests::missing_profile_name_rejected`,
`crates/xml/tests/qos_profile.rs::t17_multiple_libraries_per_document`

**Status:** done — `name` is required at `<qos_library>` and `<qos_profile>`; an unknown child of `<qos_library>` or `<qos_profile>` ends with `XmlError::UnknownElement`.

### §7.3.2.4.1.1 Example (Non-normative)

**Spec:** §7.3.2.4.1.1, p. 23 — library `ReliableProfilesLibrary` with one
profile for DataWriter and DataReader.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_libraries`

**Tests:** `crates/xml/tests/spec_items.rs::qos_library_example`, `crates/xml/tests/omg_example_documents.rs::qos_example`

**Status:** done — `qos_library_example` reads the example; `omg_example_documents::qos_example` reads the OMG's example document when `ZERODDS_OMG_DDSXML_DIR` is set.

### §7.3.2.4.2 QoS profile inheritance through `base_name`

**Spec:** §7.3.2.4.2, p. 23 — "A QoS Profile can inherit from other QoS
Profiles using the base_name XML attribute."

**Repo:** `crates/xml/src/qos_inheritance.rs::resolve_profile`; `crates/xml/src/registry.rs::QosProfileRegistry`; `crates/xml/src/qos.rs::EntityQos::merge`; `crates/xml/src/inheritance.rs::resolve_chain`; `crates/rpc/src/qos_profile.rs::RpcQos::from_xml_profile`

**Tests:** `crates/xml/tests/qos_profile.rs::t03_inheritance_child_overrides_history_keeps_reliability`,
`crates/xml/tests/qos_profile.rs::t04_three_level_inheritance_propagates`,
`crates/xml/tests/qos_profile.rs::t05_inheritance_cycle_rejected`,
`crates/xml/src/registry.rs::tests::inheritance_derived_overrides_base`,
`crates/xml/src/qos_inheritance.rs::tests::cross_library_base_name_two_segment`

**Status:** done — the policy of the derived profile replaces that of the base profile entirely, not field by field: a child with `<history><kind>KEEP_ALL_HISTORY_QOS</kind></history>` has depth 1, not the base's; the spec says nothing about it. Cycles and chains over 32 links (`MAX_INHERITANCE_DEPTH`) are rejected. `RpcQos::from_xml_profile` follows the `base_name` chain. Commits `f1a7a3ea573`, `563ab2e3a64`.

### §7.3.2.4.2 Only from profiles defined before

**Spec:** §7.3.2.4.2, p. 23 — "A QoS profile can only inherit from QoS
profiles that have been defined its definition".

**Repo:** `crates/xml/src/qos_inheritance.rs::check_definition_order`; `crates/xml/src/registry.rs::QosProfileRegistry::resolve`

**Tests:** `crates/xml/src/qos_inheritance.rs::tests::forward_base_name_rejected`, `crates/xml/src/registry.rs::tests::forward_base_name_rejected`

**Status:** done — a profile whose `base_name` points to a later-defined profile ends with `XmlError::InvalidXml`; the order is the document's, across libraries. The sentence of the spec is incomplete ("defined its definition"); read as "defined before its definition". Commit `5f5d7aaa3e9`.

### §7.3.2.4.2.1 Example (Non-normative)

**Spec:** §7.3.2.4.2.1, p. 23 — `<qos_profile name="MyProfile"
base_name="BaseProfile">`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example.

### §7.3.2.4.3 Several QoS of one kind, selected by topic name

**Spec:** §7.3.2.4.3, p. 23 — "A QoS Profile may contain several DataWriter,
DataReader, and Topic QoS settings that are selected based on the evaluation
of a filter expression on the topic name."

**Repo:** `crates/xml/src/qos_parser.rs::take_qos_container`; `crates/xml/src/qos.rs::select_qos_for_topic`; `crates/xml/src/qos_inheritance.rs::ResolvedQos`; `crates/xml/src/registry.rs::QosProfileRegistry::writer_qos_for_topic`

**Tests:** `crates/xml/tests/qos_profile.rs::t21_repeated_containers_select_by_topic_filter`, `crates/xml/src/qos_inheritance.rs::tests::topic_filter_selects_between_repeated_containers`, `crates/xml/src/registry.rs::tests::writer_qos_for_topic_selects_by_filter`, `crates/xml/tests/normative_forms.rs::repeated_containers_of_every_kind`

**Status:** done — a profile holds several containers per kind (`Vec<FilteredEntityQos>`), each with its `topic_filter`; `select_qos_for_topic`, `ResolvedQos::datawriter_qos_for_topic` and `QosProfileRegistry::writer_qos_for_topic` select by the topic's name. Several containers with the same filter (or several without) are merged policy by policy (`merge_filtered_slots`). Commit `f5f781d033b`.

### §7.3.2.4.3 Filter in the `topic_filter` attribute of the QoS

**Spec:** §7.3.2.4.3, p. 23 — "The filter expression is specified via the
topic_filter XML attribute in the QoS definition of the entity QoS."

**Repo:** `crates/xml/src/qos_parser.rs::take_qos_container`, `parse_single_qos_shortcut`; `crates/xml/src/qos.rs::FilteredEntityQos`, `topic_filter_matches`

**Tests:** `crates/xml/tests/schema_validation.rs::topic_filter_attribute_is_read`, `crates/xml/tests/qos_profile.rs::t08_topic_filter_glob_semantics`, `crates/xml/src/qos_inheritance.rs::tests::topic_filter_inherited_and_overridden`

**Status:** done — the attribute is read at the container and stored there (`FilteredEntityQos::topic_filter`); `topic_filter_matches` evaluates `*` and `?` (the PDF does not fix the syntax). The filter as a child element of `<qos_profile>` and the attribute on `<topic>` are vendor forms (`VendorForms`). Commits `f5f781d033b`, `e34118cc9a6`.

### §7.3.2.4.3 Without a filter `*` applies

**Spec:** §7.3.2.4.3, p. 23 — "If the topic filter is unspecified, the filter
"*" will be assumed."

**Repo:** `crates/xml/src/qos.rs::default_qos`, `select_qos_for_topic`

**Tests:** `crates/xml/tests/qos_profile.rs::t22_filterless_container_is_the_fallback`

**Status:** done — a container without a filter is the fallback for every topic that no filter matches (`default_qos`).

### §7.3.2.4.3 Order and precedence of filters

**Spec:** §7.3.2.4.3, p. 23 — "The QoS with an explicit topic_filter
attribute definition will be evaluated in order; they take precedence over a
QoS without a topic filter expression."

**Repo:** `crates/xml/src/qos.rs::select_qos_for_topic`

**Tests:** `crates/xml/tests/qos_profile.rs::t21_repeated_containers_select_by_topic_filter`, `crates/xml/src/qos_inheritance.rs::tests::no_matching_filter_has_no_qos`, `crates/xml/src/registry.rs::tests::writer_qos_for_topic_without_match_errors`

**Status:** done — a container with a matching filter takes precedence over the one without a filter; among several matching filters the document order selects; with no match the entity has no QoS from the profile. Commit `f5f781d033b`.

### §7.3.2.4.3.1 Example (Non-normative)

**Spec:** §7.3.2.4.3.1, p. 24 — two `<datawriter_qos>` with
`topic_filter="A*"` and `"B*"`, the second with `resource_limits`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example. The reader reads it: two `<datawriter_qos>` with different `topic_filter` in one profile, `initial_samples` and `initial_instances` in `resource_limits` (`resource_limits_initial_fields_are_carried`, `t21_repeated_containers_select_by_topic_filter`).

### §7.3.2.4.4 Single QoS as a profile

**Spec:** §7.3.2.4.4, p. 24 — "The definition of an individual QoS is a
shortcut for defining a QoS profile with a single QoS."

**Repo:** `crates/xml/src/qos_parser.rs::parse_single_qos_shortcut`, `parse_qos_library_element`

**Tests:** `crates/xml/src/qos_parser.rs::tests::single_qos_shortcut_datawriter_creates_implicit_profile`, `crates/xml/src/qos_parser.rs::tests::single_qos_shortcut_topic_creates_implicit_profile`, `crates/xml/src/qos_parser.rs::tests::single_qos_shortcut_multiple_kinds_in_same_library`, `crates/xml/src/qos_parser.rs::tests::single_qos_shortcut_without_name_is_kept`

**Status:** done — a container directly under `<qos_library>` is a profile with one QoS; without a `name` it is a profile nothing can refer to and is kept under the empty name. Commit `bf34c97b1eb`.

### §7.3.2.4.4.1 Example (Non-normative)

**Spec:** §7.3.2.4.4.1, p. 24 — `<datawriter_qos name="KeepAllWriter">`
equals `<qos_profile name="KeepAllWriter"><datawriter_qos>`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example.

---

## §7.3.3 Building Block Types

### §7.3.3.1 Types in XML

**Spec:** §7.3.3.1, p. 24 — "This building block gathers the syntax used to
represent DDS Types in XML."

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_types_element_ctx`, `parse_type_libraries_with`; `crates/xml/src/xtypes_expr.rs`; `crates/xml-compose/src/lib.rs::compose`

**Tests:** `crates/xml/tests/types_and_samples.rs::t01_struct_with_long_member`, `crates/xml/tests/types_and_samples.rs::t16_module_three_levels_deep`, `crates/xml/tests/normative_forms.rs::bounds_and_dimensions_are_evaluated`, `crates/xml-compose/tests/idl_parity.rs::every_primitive_reaches_the_same_type_from_both_front_ends`

**Status:** done — two readers: `zerodds-xml`, which `parse_dds_xml` uses, and `zerodds-xml-compose` for the code generators. The reader of `zerodds-xml` follows the grammar of XTypes 1.3 Annex A (`type`/`nonBasicTypeName`, maps, annotations, `<const>`, constant expressions as bounds); the grammar is covered item by item in `dds-xtypes-1.3.md` under §7.3.2. Commits `32aa062decf`, `3d330808107`, `af517d979de`.

### §7.3.3.1 `nonBasicTypeName` and maps in the reader of `zerodds-xml`

**Spec:** §7.3.3.1, p. 24 — syntax of types in XML.

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_type_attribute`, `parse_map_attributes`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::spec_nonbasic_type_is_named_by_nonbasic_type_name`, `crates/xml/src/xtypes_parser.rs::tests::spec_map_members_are_read`, `crates/xml/src/xtypes_parser.rs::tests::spec_map_typedef_is_read`

**Status:** done — `type="nonBasic"` names the type through `nonBasicTypeName`; maps read `mapMaxLength`, `mapKeyType`, `mapKeyNonBasicTypeName` and `mapKeyStringMaxLength`. A type name in `type` without `nonBasic` is a vendor form. Commits `4e479ef9e18`, `af517d979de`, `e34118cc9a6`.

### §7.3.3.1 Organization and management of types

**Spec:** §7.3.3.1, p. 24 — "Additionally, it provides capabilities that are
necessary or convenient for the organization and management of types and
other XML resource representations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — announcement without content in the PDF;
the constructs (`module`, `include`, `forward_dcl`, `const`) belong to the
grammar of [DDS-XTYPES] (`dds-xtypes-1.3.en.md` under §7.3.2.1).

### §7.3.3.2 No dependencies on other building blocks

**Spec:** §7.3.3.2, p. 24 — "This building block has no dependencies on other
building blocks."

**Repo:** `crates/xml/schemas/dds-xml_type_definitions_nonamespace.xsd` (includes no other file); `crates/xml/src/xtypes_parser.rs::parse_type_libraries`

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`,
`crates/xml/tests/dds_xml_full.rs::independent_building_blocks_parse_without_the_other_blocks`

**Status:** done

### §7.3.3.3 Types of all DDS types in the chameleon file

**Spec:** §7.3.3.3, p. 25 — "dds-xml_type_definitions_nonamespace.xsd
contains the XSD type definitions for all the DDS types. It defines no
targetNamespace so that it can be reused by other schemas following the XML
Chameleon Schema Definition pattern."

**Repo:** `crates/xml/schemas/dds-xml_type_definitions_nonamespace.xsd`

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/tests/schema_validation.rs::xsd_valid_documents_are_read`

**Status:** done — the OMG's file is in the tree unchanged and defines the types of all constructs (`structDecl`, `unionDecl`, `enumDecl`, …). Commits `32aa062decf`, `8bf46985f11`.

### §7.3.3.3 `<types>` as top-level element in the namespace

**Spec:** §7.3.3.3, p. 25 — "dds-xml_type_definitions.xsd defines the <types>
top-level element, and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_type_definitions.xsd`; `crates/xml/src/xtypes_parser.rs::parse_type_libraries`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`,
`crates/xml/tests/types_and_samples.rs::t26_sample_must_understand_attribute_parses`,
`crates/xml/src/xtypes_parser.rs::tests::dds_root_with_multiple_types_blocks`

**Status:** done — `<types>` as the root and as a child of `<dds>`.

---

## §7.3.4 Building Block Domains

### §7.3.4.1 Purpose

**Spec:** §7.3.4.1, p. 25 — "This building block defines the syntax used to
represent DDS Domains in XML. Domains provide a data space where information
can be shared by reading and writing a set of Topics, which are associated to
registered data types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — purpose; the syntax is in the items on
§7.3.4.3 to §7.3.4.4.2.

### §7.3.4.2 Depends on QoS and Types

**Spec:** §7.3.4.2, p. 25 — "This building block depends on the Building
Block QoS and the Building Block Types."

**Repo:** `crates/xml/schemas/dds-xml_domain_definitions_nonamespace.xsd` (includes the chameleon files of Types and QoS)

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`

**Status:** done — in the reader `<topic_qos>` uses the QoS reader; `type_ref` is checked against the `<types>` of the document when resolving (item under §7.3.4.4.1). The OMG's chameleon file includes Types and QoS.

### §7.3.4.3 Types for domains in the chameleon file

**Spec:** §7.3.4.3, p. 25 — "dds-xml_domain_definitions_nonamespace.xsd
contains the XSD type definition for domains and their contained entities."

**Repo:** `crates/xml/schemas/dds-xml_domain_definitions_nonamespace.xsd` (complexTypes `registerType`, `topicEntry`, `domainEntry`, `domainLibrary`)

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/tests/schema_validation.rs::schema_invalid_documents`, `crates/xml/tests/normative_forms.rs::registered_name_and_format_are_carried`

**Status:** done — the OMG's file is in the tree unchanged. The reader reads the element `registered_name` and the attribute `format` at `<register_type>` and the element `registered_name` at `<topic>`. `qos_profile_ref` and `topic_filter` at `<topic>` are not in the file; they are vendor forms. Commits `8bf46985f11`, `e34118cc9a6`. Both tests need `xmllint`.

### §7.3.4.3 `<domain_library>` as root element in the namespace

**Spec:** §7.3.4.3, p. 25 — "dds-xml_domain_definitions.xsd defines the
<domain_library> root element and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_domain_definitions.xsd`; `crates/xml/src/domain.rs::parse_domain_libraries`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`, `crates/xml/tests/spec_items.rs::domain_example`, `crates/xml/tests/omg_example_documents.rs::domains_example`

**Status:** done — `domain_example` and `domain_with_two_topics_and_register_types` read `<domain_library>` as the root; `domains_example` reads the OMG's document when `ZERODDS_OMG_DDSXML_DIR` is set.

### §7.3.4.4.1 Domain with topics and registered types

**Spec:** §7.3.4.4.1, p. 25 — "A Domain includes a set of Topics and
Registered Types that can be read and written in the Domain."

**Repo:** `crates/xml/src/domain.rs::parse_domain_element`

**Tests:** `crates/xml/tests/dds_xml_full.rs::domain_with_two_topics_and_register_types`, `crates/xml/src/domain.rs::tests::parse_domain_with_topic`, `crates/xml/tests/dds_xml_full.rs::misplaced_or_unknown_children_are_rejected`

**Status:** done — another child of `<domain>` or `<topic>` ends with `XmlError::UnknownElement`; with `report_and_skip` it is left out and reported. Commit `8771c676858`.

### §7.3.4.4.1 `register_type` with `type_ref`

**Spec:** §7.3.4.4.1, p. 25 — "Register types shall provide a reference to
data types that have been previously defined using the type_ref XML
attribute."

**Repo:** `crates/xml/src/domain.rs::parse_register_type`; `crates/xml/src/zerodds_xml.rs::DdsXml::resolve_participant`

**Tests:** `crates/xml/src/domain.rs::tests::parse_domain_with_topic`, `crates/xml/tests/dds_xml_full.rs::deployment_file_with_inheritance_and_cross_references`

**Status:** done — `type_ref` is read, resolved against the `<types>` of the document and passed on as the topic's type name. Commits `dd4ebb955e6`, `43146783064`.

### §7.3.4.4.1 `type_ref` is mandatory

**Spec:** §7.3.4.4.1, p. 25 — "Register types shall provide a reference [...]
using the type_ref XML attribute."

**Repo:** `crates/xml/src/domain.rs::parse_register_type`

**Tests:** `crates/xml/src/domain.rs::tests::register_type_missing_type_ref_rejected`, `crates/xml/src/participant.rs::tests::participant_register_type_without_type_ref_rejected`

**Status:** done — `parse_register_type` rejects a `register_type` without `type_ref` with `XmlError::MissingRequiredElement`: the text requires the reference ("shall provide"), the OMG's XSD has `use="optional"`. Decision for the text; the XSD-valid document without `type_ref` is not read. Commit `dd4ebb955e6`.

### §7.3.4.4.1 The type is defined before

**Spec:** §7.3.4.4.1, p. 25 — "data types that have been previously defined".

**Repo:** `crates/xml/src/zerodds_xml.rs::DdsXml::resolve_participant`, `resolve_topic_entry`, `resolve_type`; `crates/xml/src/domain.rs::DomainLibrary`

**Tests:** `crates/xml/tests/dds_xml_full.rs::register_type_with_dangling_type_ref_errors`, `crates/xml/tests/spec_items.rs::register_type_refers_to_a_type_defined_before_the_domain`

**Status:** done — `resolve_participant` resolves `type_ref` through `resolve_type`; a type that no `<types>` defines ends with `XmlError::UnresolvedReference`, so does a type defined only by a `<types>` after the domain library (`DomainLibrary::type_libraries_before`). Commits `dd4ebb955e6`, `43146783064`.

### §7.3.4.4.1 The registration name may differ from the type name

**Spec:** §7.3.4.4.1, p. 25 — "The name under which types are registered may
be different than original type name."

**Repo:** `crates/xml/src/domain.rs::RegisterType` (fields `name`,
`type_ref`)

**Tests:** `crates/xml/src/domain.rs::tests::parse_domain_with_topic`,
`crates/xml/tests/dds_xml_full.rs::deployment_file_with_inheritance_and_cross_references`

**Status:** done

### §7.3.4.4.1 A topic refers through `register_type_ref`

**Spec:** §7.3.4.4.1, p. 25 — "Topics shall refer to a registered type using
the register_type_ref XML attribute."

**Repo:** `crates/xml/src/domain.rs::parse_topic_entry`;
`crates/xml/src/zerodds_xml.rs::resolve_topic_entry`

**Tests:** `crates/xml/src/domain.rs::tests::topic_missing_register_type_ref_rejected`,
`crates/xml/tests/dds_xml_full.rs::topic_with_unknown_register_type_ref_errors`

**Status:** done

### §7.3.4.4.1 Inline QoS of a topic

**Spec:** §7.3.4.4.1, p. 25 — "Topics may also specify QoS settings inline
following the syntax defined in the Building Block QoS."

**Repo:** `crates/xml/src/domain.rs::parse_topic_entry`

**Tests:** `crates/xml/src/domain.rs::tests::parse_topic_with_inline_qos`,
`crates/xml/tests/dds_xml_full.rs::topic_qos_inline_and_profile_ref_both_resolve`

**Status:** done

### §7.3.4.4.1 `base_name` on `topic_qos`

**Spec:** §7.3.4.4.1, p. 25 — "QoS profile inheritance through the base_name
attribute may be used as defined in clause 7.3.2.4.2."

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_qos`; `crates/xml/src/zerodds_xml.rs::DdsXml::resolve_inline_qos`; `crates/xml/src/registry.rs::inline_over_base`

**Tests:** `crates/xml/tests/dds_xml_full.rs::inline_topic_qos_base_name_inherits_from_profile`, `crates/xml/src/registry.rs::tests::inline_container_inherits_base_name_profile`, `crates/xml/tests/spec_items.rs::domain_example`

**Status:** done — `parse_entity_qos` reads `base_name` at the QoS (`EntityQos::base_name`); when resolving, the profile is the base and the policies given inline override it. Commits `5dcde9b5717`, `b956dbaceed`.

### §7.3.4.4.1.1 Example (Non-normative)

**Spec:** §7.3.4.4.1.1, p. 25 — domain `MyDomain` with `domain_id="10"`, two
`register_type` and two `topic`.

**Repo:** `crates/xml/src/domain.rs::parse_domain_libraries`

**Tests:** `crates/xml/tests/spec_items.rs::domain_example`

**Status:** done — `domain_example` reads the example; the `base_name` of `topic_qos` is kept.

### §7.3.4.4.2 Domain inheritance through `base_name`

**Spec:** §7.3.4.4.2, p. 26 — "A Domain can inherit from other Domains using
the base_name XML attribute."

**Repo:** `crates/xml/src/domain.rs::DomainLibrary::effective_domain`, `effective_domain_id`

**Tests:** `crates/xml/src/domain.rs::tests::domain_id_inherited_through_base_name`, `crates/xml/src/domain.rs::tests::domain_inheritance_chain_resolves_via_resolve_chain`, `crates/xml/tests/dds_xml_full.rs::domain_inheritance_carries_topics_and_types`

**Status:** done — `domain_id`, the registered types and the topics are inherited through the chain; an entry of the same name in the derived domain replaces the base's. Commits `8771c676858`, `5f5d7aaa3e9`.

### §7.3.4.4.2 Topics and types of the base

**Spec:** §7.3.4.4.2, p. 26 — domain inheritance.

**Repo:** `crates/xml/src/domain.rs::DomainLibrary::effective_domain`; `crates/xml/src/zerodds_xml.rs::resolve_participant_chain`

**Tests:** `crates/xml/tests/dds_xml_full.rs::domain_inheritance_carries_topics_and_types`

**Status:** done — `effective_domain` merges `register_types` and `topics` of the chain; `resolve_participant` uses the effective domain, a writer at a topic of the base resolves. Commit `8771c676858`.

### §7.3.4.4.2 Only from domains defined before

**Spec:** §7.3.4.4.2, p. 26 — "A Domain can only inherit from domains that
have been defined before."

**Repo:** `crates/xml/src/domain.rs::DomainLibrary::effective_domain_id`

**Tests:** `crates/xml/src/domain.rs::tests::domain_forward_base_name_rejected`

**Status:** done — `effective_domain_id` and `effective_domain` reject a base that stands later in the library, with `XmlError::InvalidXml`. Commit `5f5d7aaa3e9`.

### §7.3.4.4.2.1 Example (Non-normative)

**Spec:** §7.3.4.4.2.1, p. 26 — `<domain name="MyDomain"
base_name="BaseDomain">`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example.

---

## §7.3.5 Building Block DomainParticipants

### §7.3.5.1 Purpose

**Spec:** §7.3.5.1, p. 26 — "This block defines the syntax to represent DDS
DomainParticipants and their contained entities (i.e., Publishers,
Subscribers, DataWriters, and DataReaders) in XML."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — purpose; the syntax is in the items on
§7.3.5.3 to §7.3.5.4.4.

### §7.3.5.2 Depends on QoS, Types and Domains

**Spec:** §7.3.5.2, p. 26 — "This building block depends on the Building
Block QoS, the Building Block Types, and the Building Block Domains."

**Repo:** `crates/xml/schemas/dds-xml_domainparticipant_definitions_nonamespace.xsd` (includes the chameleon file of Domains, which includes QoS and Types)

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`

**Status:** done

### §7.3.5.3 Types of the entities in the chameleon file

**Spec:** §7.3.5.3, p. 26 — "dds-xml_domainparticipant_defintions_nonamespace.xsd
contains the XSD type definition for all the DDS entities."

**Repo:** `crates/xml/schemas/dds-xml_domainparticipant_definitions_nonamespace.xsd` (complexTypes `dataWriterEntry`, `dataReaderEntry`, `publisherEntry`, `subscriberEntry`, `domainParticipantEntry`, `domainParticipantLibrary`)

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/tests/schema_validation.rs::schema_invalid_documents`, `crates/xml/tests/normative_forms.rs::data_reader_filters_and_conditions_are_read`

**Status:** done — the OMG's file is in the tree unchanged. The elements `content_filter`, `status_condition`, `read_condition` and `query_condition` at `<data_reader>` are read (`DataReaderEntry`). Commits `8bf46985f11`, `efd043b8ec4`. Both tests need `xmllint`.

### §7.3.5.3 `<domain_participant_library>` as root element in the namespace

**Spec:** §7.3.5.3, p. 26 — "dds-xml_domainparticipant_definitions.xsd defines
the <domain_participant_library> root element and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_domainparticipant_definitions.xsd`; `crates/xml/src/participant.rs::parse_domain_participant_libraries`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`, `crates/xml/tests/normative_forms.rs::the_participant_example_of_the_specification`, `crates/xml/tests/omg_example_documents.rs::domain_participants_example`

**Status:** done — `participant.rs` reads `<domain_participant_library>` as the root; `domain_participants_example` reads the OMG's document when `ZERODDS_OMG_DDSXML_DIR` is set, `the_participant_example_of_the_specification` the spec's.

### §7.3.5.4.1 Libraries of DomainParticipants

**Spec:** §7.3.5.4.1, p. 26 — "Domain Participant Libraries are collections
of DomainParticipants and contained entities. They are the top level elements
of the Building Block DomainParticipants."

**Repo:** `crates/xml/src/participant.rs::parse_dp_library_element`

**Tests:** `crates/xml/src/participant.rs::tests::parse_minimal_participant`,
`crates/xml/src/participant.rs::tests::missing_participant_name_rejected`

**Status:** done

### §7.3.5.4.1 Entities under their parent element

**Spec:** §7.3.5.4.1, p. 26 — "To represent this hierarchical relationship
between DDS entities, each entity is declared as a nested XML tag under the
declaration of its parent entity."

**Repo:** `crates/xml/src/participant.rs::parse_dp_element`, `parse_publisher_element`, `parse_subscriber_element`, `parse_dw_element`, `parse_dr_element`

**Tests:** `crates/xml/src/participant.rs::tests::parse_pub_with_writer`, `crates/xml/tests/dds_xml_full.rs::resolve_participant_returns_flat_snapshot`

**Status:** done — DataWriter under Publisher, DataReader under Subscriber, both under the DomainParticipant are read; an entity under another element ends with `XmlError::UnknownElement` (item below). Commit `8771c676858`.

### §7.3.5.4.1 Entity under a wrong element

**Spec:** §7.3.5.4.1, p. 26 — "each entity is declared as a nested XML tag
under the declaration of its parent entity."

**Repo:** `crates/xml/src/participant.rs::parse_publisher_element`, `parse_subscriber_element`, `parse_dp_element`

**Tests:** `crates/xml/tests/dds_xml_full.rs::misplaced_or_unknown_children_are_rejected`, `crates/xml/tests/schema_validation.rs::schema_invalid_documents`

**Status:** done — a `<data_reader>` under `<publisher>`, a `<data_writer>` under `<subscriber>` and any other child of `<domain_participant>`, `<data_writer>` or `<data_reader>` ends with `XmlError::UnknownElement` and the path; with `report_and_skip` it is left out and reported. The OMG's XSD rejects the same documents (`schema_invalid_documents`, row `data_reader_under_publisher`). Commit `8771c676858`.

### §7.3.5.4.1.1 Example (Non-normative)

**Spec:** §7.3.5.4.1.1, p. 26 — `<domain_participant_library="MyDomainParticipantLibrary">`
with publisher, subscriber, DataWriter and DataReader.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example. The start tag has no attribute name (`<domain_participant_library="…">`) and is not well-formed; `<data_reader name="MyDataReader"/>` has no `topic_ref`, which the OMG's XSD requires (`schema_invalid_documents`, row `data_reader_without_topic_ref`) and the reader rejects with `XmlError::MissingRequiredElement`; `domain_ref="MyDomain"` without a library the reader resolves (item under §7.3.5.4.2). `the_participant_example_of_the_specification` reads the example with these corrections.

### §7.3.5.4.2 `domain_ref` makes the domain's topics and types available

**Spec:** §7.3.5.4.2, p. 27 — "DomainParticipants may refer to a Domain
declared in the context of a Domain Library (see Building Block Domains) using
the domain_ref XML attribute. This makes the Topics and Register Types defined
in the Domain available for all the DataWriters and DataReaders defined in the
context of the DomainParticipant."

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_participant_chain`, `resolve_writer`, `resolve_reader`; `crates/xml/src/participant.rs::parse_dp_element`

**Tests:** `crates/xml/tests/dds_xml_full.rs::cross_library_domain_ref_resolves`, `crates/xml/tests/dds_xml_full.rs::deployment_file_with_inheritance_and_cross_references`, `crates/xml/tests/dds_xml_full.rs::participant_local_definitions_resolve_writers`, `crates/xml/tests/dds_xml_full.rs::participant_definition_overrides_the_domains`

**Status:** done — writers and readers find the domain's topics; `<topic>` and `<register_type>` in a participant are definitions of their own that join the domain's and replace those of the same name, as the OMG's XSD types them. Commits `5f5d7aaa3e9`, `8d65ac4cd8f`.

### §7.3.5.4.2 `domain_ref` without a library name

**Spec:** §7.3.5.4.2, p. 27 — "using the domain_ref XML attribute".

**Repo:** `crates/xml/src/zerodds_xml.rs::DdsXml::find_domain_in_library`

**Tests:** `crates/xml/tests/dds_xml_full.rs::unqualified_domain_ref_resolves`

**Status:** done — `find_domain_in_library` resolves `domain_ref="MyDomain"` (without `library::`) through the first library that has a domain of that name, in document order; `library::name` stays possible. The PDF does not fix the form of the reference. Commit `5f5d7aaa3e9`.

### §7.3.5.4.2 DomainParticipant without `domain_ref`

**Spec:** §7.3.5.4.2, p. 27 — "DomainParticipants may refer to a Domain".

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_participant_chain`

**Tests:** `crates/xml/tests/dds_xml_full.rs::participant_without_domain_ref_but_with_domain_id_resolves`, `crates/xml/tests/dds_xml_full.rs::participant_without_domain_resolves_its_own_topics`, `crates/xml/tests/dds_xml_full.rs::participant_domain_id_overrides_the_referenced_domain`, `crates/xml/src/participant.rs::tests::missing_domain_ref_parses_and_is_left_to_the_resolver`

**Status:** done — a participant without `domain_ref` but with `domain_id` resolves, without a domain and with its own definitions; without both attributes resolving ends with `XmlError::UnresolvedReference`. Commit `5f5d7aaa3e9`.

### §7.3.5.4.2 `domain_id` overrides the domain's

**Spec:** §7.3.5.4.2, p. 27 — "The Domain Id specified in the parent Domain
can be overridden via the domain_id XML attribute."

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_participant_chain`

**Tests:** `crates/xml/tests/dds_xml_full.rs::participant_domain_id_overrides_the_referenced_domain`

**Status:** done

### §7.3.5.4.2.1 Example (Non-normative)

**Spec:** §7.3.5.4.2.1, p. 27 — `<domain_participant name="MyDomain"
domain_ref="MyDomain" domain_id="32">`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example. The reader reads and resolves it: `domain_ref` without a library and `domain_id` as an override (`unqualified_domain_ref_resolves`, `participant_domain_id_overrides_the_referenced_domain`).

### §7.3.5.4.3 DomainParticipant inheritance through `base_name`

**Spec:** §7.3.5.4.3, p. 27 — "DomainParticipants may inherit from
DomainParticipants defined in the context of a DomainParticipant Library
using the base_name XML attribute."

**Repo:** `crates/xml/src/zerodds_xml.rs::participant_chain`, `resolve_participant_chain`

**Tests:** `crates/xml/tests/dds_xml_full.rs::participant_inheritance_one_level`,
`crates/xml/tests/dds_xml_full.rs::participant_inheritance_three_levels`,
`crates/xml/tests/dds_xml_full.rs::participant_inheritance_cycle_detected`

**Status:** done — `domain_ref` and `domain_id` of the derived participant override the base's, the QoS is merged policy by policy, a publisher or subscriber of the same name is replaced entirely.

### §7.3.5.4.3 Only from libraries defined before

**Spec:** §7.3.5.4.3, p. 27 — "A DomainParticipant can only inherit from
DomainParticipants in DomainParticipant Libraries that have been defined
before its own definition."

**Repo:** `crates/xml/src/zerodds_xml.rs::participant_chain`, `participant_position`

**Tests:** `crates/xml/tests/dds_xml_full.rs::participant_forward_base_name_rejected`

**Status:** done — `participant_chain` rejects a base that does not stand before the derived participant in document order (library, then entry), with `XmlError::InvalidXml`. Commit `5f5d7aaa3e9`.

### §7.3.5.4.3.1 Example (Non-normative)

**Spec:** §7.3.5.4.3.1, p. 27 — `base_name="AnotherParticipantLibrary::TheirParticipant"`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example.

### §7.3.5.4.4 Inline QoS in the context of the entity

**Spec:** §7.3.5.4.4, p. 27 — "Inline QoS setting definition is allowed in
the context an entity's definition."

**Repo:** `crates/xml/src/participant.rs::parse_dp_element`,
`parse_publisher_element`, `parse_subscriber_element`,
`parse_dw_element`, `parse_dr_element`

**Tests:** `crates/xml/src/participant.rs::tests::parse_pub_with_writer`,
`crates/xml/tests/dds_xml_full.rs::participant_inheritance_one_level`

**Status:** done

### §7.3.5.4.4 Inline QoS applies only to its own entity

**Spec:** §7.3.5.4.4, p. 27 — "Inline QoS settings apply only to the entity
in whose context they are being defined."

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_writer`, `resolve_reader`

**Tests:** `crates/xml/tests/dds_xml_full.rs::datawriter_does_not_inherit_publisher_qos`, `crates/xml/tests/dds_xml_full.rs::publisher_without_qos_yields_none`

**Status:** done — a `<data_writer>` without QoS of its own and without `base_name` has no QoS from its `<publisher>`; a `<data_reader>` likewise. Commit `8771c676858`.

### §7.3.5.4.4 `base_name` on inline QoS

**Spec:** §7.3.5.4.4, p. 27 — "Inline entities may inherit from an existing
QoS Profile using the base_name XML attribute."

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_qos`, `set_inline_qos`; `crates/xml/src/registry.rs::QosProfileRegistry::writer_qos_for_inline`, `reader_qos_for_inline`; `crates/webdds-bridge/src/qos_update.rs`

**Tests:** `crates/xml/tests/dds_xml_full.rs::inline_topic_qos_base_name_inherits_from_profile`, `crates/xml/tests/dds_xml_full.rs::repeated_inline_qos_container_is_rejected`, `crates/xml/src/registry.rs::tests::inline_container_inherits_base_name_profile`, `crates/xml/tests/normative_forms.rs::the_participant_example_of_the_specification`

**Status:** done — `parse_entity_qos` reads `base_name` at every inline QoS; the named profile is the base. A second QoS container of the same kind at an entity ends with `XmlError::InvalidXml`. `qos_profile_ref` at `<data_writer>`, `<data_reader>` and `<topic>` is a vendor form. Commits `5dcde9b5717`, `b956dbaceed`, `92451a0a65f`, `e34118cc9a6`.

### §7.3.5.4.4.1 Example (Non-normative)

**Spec:** §7.3.5.4.4.1, p. 27 — `<domain_participant_qos base_name="BaseProfile">`
with `entity_factory`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example; the `base_name` of the inline QoS is kept and names the base (`the_participant_example_of_the_specification`, `inline_topic_qos_base_name_inherits_from_profile`).

---

## §7.3.6 Building Block Applications

### §7.3.6.1 Purpose

**Spec:** §7.3.6.1, p. 27 — "This block defines the XML syntax to represent
DDS applications that participate (or may be participating) in the DDS
Global Data Space."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — purpose.

### §7.3.6.2 Depends on QoS, Types, Domains and DomainParticipants

**Spec:** §7.3.6.2, p. 27 — "This building block depends on the Building
Block QoS, the Building Block Types, the Building Block Domains, and the
Building Block DomainParticipants."

**Repo:** `crates/xml/schemas/dds-xml_application_definitions_nonamespace.xsd` (includes the chameleon file of DomainParticipants)

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`

**Status:** done

### §7.3.6.3 Types for applications in the chameleon file

**Spec:** §7.3.6.3, p. 28 — "dds-xml_application_definitions_nonamespace.xsd
contains the XSD type definition for applications and their contained
entities."

**Repo:** `crates/xml/schemas/dds-xml_application_definitions_nonamespace.xsd` (complexTypes `applicationEntry`, `applicationLibrary`)

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`

**Status:** done — the OMG's file is in the tree unchanged; the test needs `xmllint`.

### §7.3.6.3 `<application_library>` as root element in the namespace

**Spec:** §7.3.6.3, p. 28 — "dds-xml_application_definititons.xsd defines the
<application_library> root element and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_application_definitions.xsd`;
`crates/xml/src/application.rs::parse_application_libraries`

**Tests:** `crates/xml/src/application.rs::tests::bare_application_library_root_is_a_document`,
`crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`

**Status:** done

### §7.3.6.4.1 Libraries of applications with DomainParticipants

**Spec:** §7.3.6.4.1, p. 28 — "Application Libraries are collections of
Applications, which are composed of a set of DomainParticipants and contained
entities. They are the top level elements of the Building Block
Applications."

**Repo:** `crates/xml/src/application.rs::parse_app_library_element`, `parse_app_element`; `crates/xml/src/zerodds_xml.rs::DdsXml::resolve_application`

**Tests:** `crates/xml/src/application.rs::tests::parse_minimal_application`,
`crates/xml/src/application.rs::tests::application_participant_carries_its_whole_definition`,
`crates/xml/tests/dds_xml_full.rs::mixed_top_level_with_cross_refs`

**Status:** done — `parse_app_element` reads the `<domain_participant>` children; any other child ends with `XmlError::UnknownElement`.

### §7.3.6.4.1.1 Example (Non-normative)

**Spec:** §7.3.6.4.1.1, p. 28 — application `SimpleShapesDemoApplication`
with `<data_writer>` and `<data_reader>` under `<publisher>`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example. It puts `<data_reader>` under `<publisher>`, against the order of §7.3.5.4.1 (p. 26) and against the OMG's XSD; the reader rejects it with `XmlError::UnknownElement` (item under §7.3.5.4.1). The test `application_participant_carries_its_whole_definition` reproduces the example without that line.

### §7.3.6.4.2 DomainParticipants of an application inherit from libraries

**Spec:** §7.3.6.4.2, p. 28 — "DomainParticipants defined in the context of an
Application may inherit from DomainParticipants defined in the context of a
DomainParticipant Library using the base_name XML attribute as described in
the Building Block DomainParticipants."

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_application_participant`

**Tests:** `crates/xml/src/application.rs::tests::application_participant_may_inherit_instead_of_naming_a_domain`,
`crates/xml/tests/dds_xml_full.rs::mixed_top_level_with_cross_refs`,
`crates/xml/tests/dds_xml_full.rs::deployment_file_with_inheritance_and_cross_references`

**Status:** done — `base_name` must name the library (`library::name`).

### §7.3.6.4.2 Only from libraries defined before

**Spec:** §7.3.6.4.2, p. 28 — "A DomainParticipant can only inherit from
DomainParticipants in DomainParticipant Libraries that have been defined
before its own definition."

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_application_participant`, `participant_chain`

**Tests:** `crates/xml/tests/spec_items.rs::application_inherits_only_from_libraries_defined_before_it`, `crates/xml/tests/dds_xml_full.rs::participant_forward_base_name_rejected`

**Status:** done — the chain of the base is checked for document order as for DomainParticipants (`participant_chain`), and the base's library must stand before the application library (`ApplicationLibrary::participant_libraries_before`); otherwise `XmlError::InvalidXml`. Commits `5f5d7aaa3e9`, `a45564ecb6b`.

### §7.3.6.4.2.1 Example (Non-normative)

**Spec:** §7.3.6.4.2.1, p. 28 — `base_name="ShapesParticipantLibrary::MyParticipant"`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example.

---

## §7.3.7 Building Block Data Samples

### §7.3.7.1 Purpose

**Spec:** §7.3.7.1, p. 28 — "This block defines XML syntax to represent Data
Samples that may be exchanged between different DDS applications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — purpose.

### §7.3.7.2 No dependencies on other building blocks

**Spec:** §7.3.7.2, p. 28 — "This building block has no dependencies on other
building blocks."

**Repo:** `crates/xml/src/sample.rs::parse_sample` (takes the type as a parameter); `crates/xml/schemas/dds-xml_data_samples_definitions_nonamespace.xsd`

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`,
`crates/xml/tests/dds_xml_full.rs::independent_building_blocks_parse_without_the_other_blocks`

**Status:** done

### §7.3.7.3.1 No generic XSD for samples

**Spec:** §7.3.7.3.1, p. 29 — "Because it is impossible to define a generic
XSD file to represent Data Samples of all the possible Data Type combinations
in DDS, the syntax to represent Data Samples is based on the XML
representation rules specified in sub clauses 7.1, 7.1.2, 7.2.2, 7.2.3, and
7.2.6."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale; the rules are in the items
below. ZeroDDS ships an XSD of its own for the envelope of a sample (item on
§7.3.1).

### §7.3.7.3.1 Structure according to §7.2.3: element named after the field

**Spec:** §7.3.7.3.1, p. 29 — rules of §7.2.3 for data samples.

**Repo:** `crates/xml/src/sample.rs::parse_struct_value`

**Tests:** `crates/xml/tests/types_and_samples.rs::t19_sample_parse_struct`, `crates/xml/tests/types_and_samples.rs::t23_sample_with_nested_struct`, `crates/xml/tests/types_and_samples.rs::t25_sample_optional_member_missing_ok`, `crates/xml/src/sample.rs::tests::missing_member_rejected`, `crates/xml/src/sample.rs::tests::parse_sample_unknown_member_rejected`, `crates/xml/src/sample.rs::tests::parse_sample_duplicate_member_rejected`, `crates/xml/src/sample.rs::tests::parse_sample_reads_inherited_members`

**Status:** done — a field without an element is an error unless it is `optional`; a child without a field (looked up along the `baseType` chain too) and a second element of the same field end with `XmlError`. Commits `8771c676858`, `cbc4ce59003`.

### §7.3.7.3.1 Fields unordered

**Spec:** §7.3.7.3.1, p. 29; §7.2.3, p. 18 — "unordered elements".

**Repo:** `crates/xml/src/sample.rs::parse_struct_value`

**Tests:** `crates/xml/tests/spec_items.rs::sample_fields_come_in_any_order`, `crates/xml/src/sample.rs::tests::parse_sample_members_in_any_order`, `crates/xml/src/sample.rs::tests::parse_sample_duplicate_member_rejected`

**Status:** done — the fields are looked up by name, the order does not matter; a duplicate element ends with `XmlError`. Commit `cbc4ce59003`.

### §7.3.7.3.1 Values according to Table 7.1: boolean

**Spec:** §7.3.7.3.1, p. 29; Table 7.1, p. 16 — boolean, case sensitive.

**Repo:** `crates/xml/src/sample.rs::parse_primitive_value`

**Tests:** `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`, `crates/xml/tests/spec_items.rs::primitive_types_example`

**Status:** done — `parse_primitive_value` reads `boolean` with `parse_bool_strict` (`True` is rejected).

### §7.3.7.3.1 Values according to Table 7.1: enum

**Spec:** §7.3.7.3.1, p. 29; Table 7.1, p. 16 — enum as the literal's name.

**Repo:** `crates/xml/src/sample.rs::parse_value_for_typedef`

**Tests:** `crates/xml/src/sample.rs::tests::parse_sample_enum_rejects_unknown_literal`, `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`, `crates/xml/tests/spec_items.rs::primitive_types_example`

**Status:** done — the text of a field of an enumeration type is compared with the type's literals; a literal the type does not declare ends with `XmlError`. Commit `8771c676858`.

### §7.3.7.3.1 Values according to Table 7.1: long decimal

**Spec:** §7.3.7.3.1, p. 29; Table 7.1, p. 16 — long -2147483648 to
2147483647.

**Repo:** `crates/xml/src/sample.rs::parse_primitive_value`, `parse_long_sample`

**Tests:** `crates/xml/tests/types_and_samples.rs::t19_sample_parse_struct`, `crates/xml/src/sample.rs::tests::parse_simple_struct_sample`

**Status:** done — decimal values are read; the hexadecimal form and `LENGTH_UNLIMITED` are in the item below.

### §7.3.7.3.1 Values according to Table 7.1: long hexadecimal and LENGTH_UNLIMITED

**Spec:** §7.3.7.3.1, p. 29; Table 7.1, p. 16 — "or 0x80000000 to 0x7fffffff
or LENGTH_UNLIMITED."

**Repo:** `crates/xml/src/sample.rs::parse_long_sample`, `parse_i16`, `parse_i64`

**Tests:** `crates/xml/src/sample.rs::tests::parse_sample_long_hex_and_constant_names`, `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`

**Status:** done — `parse_long_sample` reads `0x10` and `LENGTH_UNLIMITED` (and the names of the constants of §7.2.2); `short` and `long long` read the hexadecimal form alike. Commit `8771c676858`.

### §7.3.7.3.1 Values according to Table 7.1: unsigned long

**Spec:** §7.3.7.3.1, p. 29; Table 7.1, p. 16 — unsigned long decimal and
hexadecimal.

**Repo:** `crates/xml/src/sample.rs::parse_ulong_sample`, `parse_uint`

**Tests:** `crates/xml/src/sample.rs::tests::parse_sample_ulong_constant_names`, `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`

**Status:** done — `parse_ulong_sample` reads the decimal and hexadecimal form (up to 4294967295) and the names `DURATION_INFINITE_NSEC`, `DURATION_ZERO_NSEC`, `TIME_INVALID_NSEC`; 4294967296 ends with `XmlError`.

### §7.3.7.3.1 Values according to Table 7.1: string

**Spec:** §7.3.7.3.1, p. 29; Table 7.1, p. 16 — string escaped according to
[XML].

**Repo:** `crates/xml/src/sample.rs::parse_primitive_value`; `crates/xml/src/sample.rs::xml_escape`

**Tests:** `crates/xml/tests/types_and_samples.rs::t24_sample_string_escaping_roundtrip`,
`crates/xml/tests/types_and_samples.rs::t20_sample_roundtrip`

**Status:** done — the string keeps its whitespace; escaped characters are resolved, the writer escapes them.

### §7.3.7.3.1 Constants according to §7.2.2 and durations according to §7.2.6

**Spec:** §7.3.7.3.1, p. 29 — rules of §7.2.2 and §7.2.6 for data samples.

**Repo:** `crates/xml/src/sample.rs::parse_long_sample`, `parse_ulong_sample`

**Tests:** `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`, `crates/xml/src/sample.rs::tests::parse_sample_long_hex_and_constant_names`

**Status:** done — `parse_long_sample` and `parse_ulong_sample` accept the names of the constants; a field of type `Duration_t` is a structure with `sec` and `nanosec`, `<sec>DURATION_INFINITE_SEC</sec>` gives 2147483647. Commit `8771c676858`.

### §7.3.7.3.1 Own XSD following the chameleon pattern

**Spec:** §7.3.7.3.1, p. 29 — "Users of this specification who may want to
define schema files to specify the syntax of specific Data Samples can follow
the XML Chameleon Schema Definition pattern."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — option for users.

### §7.3.7.3.2 Sequences and arrays with `item` elements

**Spec:** §7.3.7.3.2, p. 29 — "The general XML representation of IDL
sequences and arrays is done following a schema defined as an XSD
complexType. The complexType contains zero or more elements named item.
Nested inside each element is the XSD schema obtained from mapping the IDL
type of the element itself."

**Repo:** `crates/xml/src/sample.rs::parse_member_value`, `parse_array_items`; `crates/xml/src/sample.rs::serialize_value_body`

**Tests:** `crates/xml/src/sample.rs::tests::parse_sample_with_sequence_using_item_tag`, `crates/xml/src/sample.rs::tests::parse_sample_with_array_using_item_tag`, `crates/xml/src/sample.rs::tests::parse_sample_array_length_is_checked`, `crates/xml/src/sample.rs::tests::parse_sample_sequence_over_bound_rejected`, `crates/xml/src/sample.rs::tests::parse_sample_multidimensional_array_is_nested`, `crates/xml/src/sample.rs::tests::parse_sample_with_empty_sequence`, `crates/xml/tests/types_and_samples.rs::t21_sample_with_sequence_items`, `crates/xml/src/sample.rs::tests::serialize_sample_uses_item_tag_for_sequence`

**Status:** done — the length of a fixed-length field and the bound of a sequence are checked (`XmlError::ValueOutOfRange`); a multi-dimensional field is read as nested levels of `item`. Commit `8142acb0c21`.

### §7.3.7.3.2.1 Example (Non-normative)

**Spec:** §7.3.7.3.2.1, p. 29 — `CoordinatesSeq` with `item` elements.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example; it types `x` and `y` as
`xs:Integer` (XSD has `xs:integer`).

### §7.3.7.4.1 Structure Types (non-normative)

**Spec:** §7.3.7.4.1, p. 30 — `<aStruct>` with `<aLong>` twice and
`<aNestedStruct>`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example. An IDL structure has no two fields of the same name; the reader rejects the second `<aLong>` (`parse_sample_duplicate_member_rejected`).

### §7.3.7.4.2 Unions (non-normative)

**Spec:** §7.3.7.4.2, p. 30 — `<data><aUnion><aLong>0</aLong></aUnion></data>`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Example; §7.3.7.3 has no normative rule for unions. The example names the active field without a discriminator; `parse_union_value` reads it (`parse_sample_union_without_discriminator`), and the form with `<discriminator>` is read as well (`parse_sample_with_union`). Commit `cbc4ce59003`.

### §7.3.7.4.3 Sequences (non-normative)

**Spec:** §7.3.7.4.3, p. 30 — sequences of `double` and of structures with
`item`.

**Repo:** —

**Tests:** `crates/xml/tests/spec_items.rs::sequence_and_array_examples`

**Status:** done — `sequence_and_array_examples` reads the example.

### §7.3.7.4.4 Arrays (non-normative)

**Spec:** §7.3.7.4.4, p. 30 — arrays of `long` and of structures with
`item`.

**Repo:** —

**Tests:** `crates/xml/tests/spec_items.rs::sequence_and_array_examples`

**Status:** done — `sequence_and_array_examples` reads the example.

### §7.3.7.4.5 Primitive Types (non-normative)

**Spec:** §7.3.7.4.5, p. 30–31 — `aShort`, `aUShort`, `anEnum`, `aLong`,
`aULong`, `aLongLong`, `aULongLong`, `aFloat`, `aDouble`, `aBoolean`,
`aString`, `anotherString` with character references, `anOctet` as `0x00`,
`aChar`.

**Repo:** —

**Tests:** `crates/xml/tests/spec_items.rs::primitive_types_example`

**Status:** done — `primitive_types_example` reads all fields of the example and checks the values. Table 7.1 has no rows for `short`, `unsigned short`, `long long`, `unsigned long long`, `float`, `double`, `octet` and `char`; their form is described only by this example. Commit `a45564ecb6b`.

---

## §8 Building Block Sets

### §8 Block sets as combinations of building blocks

**Spec:** §8, p. 32 — "This Chapter defines some relevant combinations of
building blocks called building block sets. A block set is a collection of
building blocks. Block sets provide a convenient mechanism to group related
building blocks so that other specifications can reference the complete set
as opposed to the individual building blocks."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — term; the only block set is in the items on
§8.1.

### §8.1 DDS System Block Set with five building blocks

**Spec:** §8.1, p. 32 — "This block set offers the ability to describe a
complete DDS system. It contains: Building Block QoS, Building Block Types,
Building Block Domains, Building Block DomainParticipants, Building Block
Applications".

**Repo:** `crates/xml/src/zerodds_xml.rs::parse_dds_xml`, `parse_dds_xml_with`; `crates/xml/schemas/dds-xml_dds_system_definitions_nonamespace.xsd` (complexType `ddsSystem`)

**Tests:** `crates/xml/src/zerodds_xml.rs::tests::parse_mixed_top_level`,
`crates/xml/tests/dds_xml_full.rs::all_four_libraries_roundtrip`,
`crates/xml/tests/types_and_samples.rs::t17_dds_xml_resolve_type`,
`crates/xml/tests/dds_xml_full.rs::empty_dds_root_is_valid`

**Status:** done — `<dds>` takes the five libraries in any order and number; any other child ends with `XmlError::UnknownElement`. Data Samples are not part of it. The order of the libraries counts for the "previously defined" rules (items under §7.3.4.4.1 and §7.3.6.4.2).

### §8.1 Chameleon file of the block set

**Spec:** §8.1, p. 32 — "dds-xml_dds_system_definitions_nonamespace.xsd
contains all the XSD type definitions for the DDS System Block Set. It
defines no targetNamespace so that it can be reused by other schemas
following the XML Chameleon Schema Definition pattern."

**Repo:** `crates/xml/schemas/dds-xml_dds_system_definitions_nonamespace.xsd`

**Tests:** `crates/xml/src/schemas.rs::tests::nonamespace_xsds_omit_target_namespace`,
`crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`

**Status:** done — the OMG's file is in the tree unchanged; it includes the chameleon file of Applications, which includes the others, and has neither a `targetNamespace` nor a root element. `every_in_tree_schema_compiles` needs `xmllint`.

### §8.1 `<dds>` as top-level element in the namespace

**Spec:** §8.1, p. 32 — "dds-xml_dds_system_definitions.xsd defines the <dds>
top-level element, and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_dds_system_definitions.xsd`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`,
`crates/xml/src/schemas.rs::tests::namespaced_xsds_include_target_namespace`

**Status:** done

---

## Audit-Status

165 done / 0 partial / 0 open / 51 n/a (informative) / 0 n/a (rejected). Before: 79 / 23 / 63 / 51 / 0.

Test run: Linux test host, state `2de529e05a6`: `zerodds-xml` lib tests 313 passed; the run of the integration tests aborted in `fuzz_smoke` with a stack overflow (fixed in `24780539a95`). The tests created or changed since (`spec_items`, `fuzz_smoke`, `schema_validation`, `typeobject_bridge` and the tests of commits `a45564ecb6b`, `43146783064`, `2120724498f`) have only been compiled on the Mac (`cargo clippy --all-targets`), not run; a run on the Linux test host is pending.

Open items: none.
