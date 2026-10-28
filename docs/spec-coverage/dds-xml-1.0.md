# DDS Consolidated XML Syntax 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xml-1.0.pdf` (33 Seiten, OMG
formal/18-10-01, Dezember 2018) —
[OMG DDS-XML 1.0](https://www.omg.org/spec/DDS-XML/1.0/)

Audit Item für Item gegen die PDF nach `internal/spec-coverage/PROCESS.md`. Seitenangaben sind PDF-Seiten (PDF-Seite = gedruckte Seite + 9). Jeder normative Satz von §1 bis §8, jede Zeile von Table 7.1 und Table 7.2, jede Konstante von §7.2.2 und jedes Attribut der Building Blocks ist ein eigenes Item. §7.2 und §7.2.3 verlangen eine 1:1-Abbildung der Typen der DDS-IDL; die QoS-Policies, ihre Aufzählungen und die QoS-Strukturen der IDL von DDS 1.4 (`internal/standards/cache/omg/dds-dcps-1.4.pdf`, S. 157–161) sind deshalb je ein Item unter §7.2.1 und §7.2.3; ihre Vorgaben stehen in DDS 1.4 §2.2.3 (S. 104–113 derselben PDF). `done` steht nur, wo Code und Test die Anforderung abdecken; ein zitierter Test nennt die Testfunktion als `datei::modul::name`. Eine Fundstelle im Repo nennt Datei und Symbol, keine Zeile. Die Belege sind aus dem Code gelesen und übersetzt; welche Tests gelaufen sind, steht unter „Audit-Status“. Wo Code eine Anforderung abbildet, aber kein Test sie prüft, ist das Item `open` und die Notiz nennt die Codestelle.

**Kontext:** DDS-XML 1.0 ist in ZeroDDS über diese Teile verteilt:

- `crates/xml/` (`zerodds-xml`) — der Leser für alle Building Blocks: der
  Baum des Dokuments (`parser`), die Werte von §7.1.4 und §7.2.2 (`types`),
  QoS-Bibliotheken und -Profile (`qos`, `qos_parser`, `qos_inheritance`,
  `registry`), Domains (`domain`), DomainParticipants (`participant`),
  Applications (`application`), der Leser für `<dds>` mit dem Auflösen der
  Verweise (`zerodds_xml`), Typen (`xtypes_def`, `xtypes_parser`), Data
  Samples (`sample`) und die XSD-Dateien unter `crates/xml/schemas/`
  (`schemas`).
- `crates/xml-compose/` (`zerodds-xml-compose`) — ein zweiter Leser für
  `<types>`, der die Typen in das gemeinsame Modell der Codegeneratoren
  bringt.
- Aufrufer: `tools/xmlc` (`zerodds-xmlc validate|types|render`),
  `tools/admin` (`qos validate`, `qos check`), `crates/rpc`
  (`RpcQos::from_xml_profile`) und `crates/webdds-bridge` (QoS-Profile und
  Entitäten nach DDS-WEB).

`zerodds-dcps`, die C-API und die Sprachbindungen lesen kein DDS-XML.
`crates/dcps/src/qos.rs` übernimmt ein `zerodds_qos::WriterQos` oder
`ReaderQos`, wie es `QosProfileRegistry` liefert, in `DataWriterQos` und
`DataReaderQos` (`From`-Implementierungen). Die Spec definiert keine
Programmierschnittstelle (§2 überlässt die Konformität den verweisenden
Specs); Items gibt es deshalb nur für die Syntax und ihre Bedeutung.

Die PDF nennt auf S. 1 XSD-Dateien unter `https://www.omg.org/spec/DDS-XML/20180501/` als normative maschinenlesbare Dateien. Die zwölf Dateien für QoS, Types, Domains, DomainParticipants, Applications und DDS System liegen unverändert unter `crates/xml/schemas/` (Byte für Byte die der OMG); für Data Samples gibt es kein normatives Paar (§7.3.7.3.1), ZeroDDS liefert ein eigenes. Die Beispieldokumente `dds-xml_*_example.xml` liegen nicht im Repository: die Tests in `crates/xml/tests/omg_example_documents.rs` und ein Teil von `crates/xml/tests/schema_validation.rs` lesen sie aus dem Verzeichnis in `ZERODDS_OMG_DDSXML_DIR` und kehren ohne die Variable ohne Prüfung zurück; `schema_validation.rs` braucht dazu `xmllint`. Wo Text und XSD der Spec sich widersprechen, steht die Entscheidung im Item: `user_data`, `topic_data` und `group_data` (XSD: ein `value` als Base64; Text: Liste in `value` oder `valueB64`; alle Formen werden gelesen), `base_name` an `<domain>` (nur im Text; gelesen) und `type_ref` an `register_type` (Text: Pflicht; XSD: optional; Pflicht).

Die Grammatik der Typen in `<types>` ist die von [DDS-XTYPES]; sie ist Item für
Item in [`dds-xtypes-1.3.md`](dds-xtypes-1.3.md) unter §7.3.2 geführt. Hier
stehen nur die Anforderungen, die DDS-XML an den Building Block Types stellt.

**Offene Punkte:** keine; jedes Item ist `done` oder `n/a (informative)`.

---

## §1 Scope

### §1 Zusammenführung der XML-Syntax

**Spec:** §1, S. 10 — "Historically various specifications have defined XML
syntax to represent particular subsets of DDS-related resources [...] This
specification consolidates all this XML syntax into a single document. There
are no significant syntactic changes in this document relative to referenced
specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Syntax steht in den Items ab
§7.

---

## §2 Conformance Criteria

### §2 Keine eigenen Konformitätspunkte

**Spec:** §2, S. 10 — "This document contains no independent conformance
points. Rather, it defines XML Schemas to be used to describe DDS resources
such that they can be referenced by other specifications leaving the
definition of conformance criteria to the referencing specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis auf das Modell der Konformität.

### §2 Regel 1: künftige Specs verweisen auf DDS-XML

**Spec:** §2, S. 10 — "Future specifications that describe DDS resources in
XML shall reference this specification or a future revision thereof."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — richtet sich an Specs der OMG, nicht an eine
Implementierung.

### §2 Regel 2: Revisionen bestehender Specs

**Spec:** §2, S. 10 — "Future revisions of current specifications that
describe DDS resources in XML should reference this specification or a future
revision thereof."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — richtet sich an Specs der OMG.

### §2 Ausgewählte Building Blocks vollständig unterstützt

**Spec:** §2, S. 10 — "Reference to this standard shall result in a selection
of building blocks where all selected building blocks shall be supported
entirely."

**Repo:** `crates/xml/src/conformance.rs::SUPPORTED_BUILDING_BLOCKS`

**Tests:** `crates/xml/src/conformance.rs::tests::supported_blocks_match_spec_count`, `crates/xml/tests/schema_validation.rs::xsd_valid_documents_are_read`, `crates/xml/tests/omg_example_documents.rs::dds_system_example`

**Status:** done — alle sechs Building Blocks werden gelesen, mit den Ausnahmen, die die Items zu §7.3 einzeln nennen: das Dokument der Tabelle `XSD_VALID` und die fünf Beispiele der OMG werden gelesen; was die XSD ablehnt, lehnt der Leser ab (`schema_invalid_documents`). Eine Form, die das XSD zulässt und der Leser nicht liest, gibt es einmal: `register_type` ohne `type_ref` (der Text von §7.3.4.4.1 verlangt es, das XSD hat `use="optional"`). Commits `8bf46985f11`, `efd043b8ec4`.

---

## §3 Normative References

### §3 [XML] Extensible Markup Language 1.0 (Fifth Edition)

**Spec:** §3, S. 10 — "[XML] Extensible Markup Language (XML) 1.0 (Fifth
Edition) Specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die Anforderung steht im Item zu
§7.1.1.

### §3 [XSD-1] XML Schema Definition Language 1.1 Part 1

**Spec:** §3, S. 10 — "[XSD-1] XML Schema Definition Language (XSD) 1.1 Part
1: Structures."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die Schema-Dateien stehen in den
Items zu §7.1.2 und §7.3.1.

### §3 [XSD-2] XML Schema Definition Language 1.1 Part 2

**Spec:** §3, S. 10 — "[XSD-2] XML Schema Definition Language (XSD) 1.1 Part
2: Datatypes."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die Werte stehen in den Items zu
§7.1.4, §7.1.5 und §7.2.2.

### §3 [DDS4CCM] DDS for Lightweight CCM 1.1

**Spec:** §3, S. 10 — "The following referenced documents were used as input
to this specification: [DDS4CCM] DDS for Lightweight CCM (DDS4CCM), Version
1.1."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Vorlage der Syntax für QoS.

### §3 [DDS-XTYPES] Extensible and Dynamic Topic Types 1.2

**Spec:** §3, S. 10 — "[DDS-XTYPES] Extensible and Dynamic Topic Types for
DDS, Version 1.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Vorlage der Syntax für Typen und Samples;
die Typen in den Items zu §7.3.3, die Samples in §7.3.7.

### §3 [DDS-WEB] Web-Enabled DDS 1.0

**Spec:** §3, S. 10 — "[DDS-WEB] Web-Enabled DDS, Version 1.0."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Vorlage der Syntax für Applications,
Domains und Entitäten.

### §3 [DDS] Data Distribution Service 1.4

**Spec:** §3, S. 11 — "[DDS] Data Distribution Service, Version 1.4. OMG
Document: formal/15-04-10."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur; die IDL der QoS steht in den
Items zu §7.2.1 und §7.2.3.

### §3 [IDL] Interface Definition Language 4.2

**Spec:** §3, S. 11 — "[IDL] Interface Definition Language (IDL), Version
4.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literatur.

---

## §4 Terms and Definitions

### §4 Building block

**Spec:** §4, S. 11 — "A building block is a consistent set of XML schemas
that together can be used to describe the syntax of XML documents that
represent a set of DDS resources. Building blocks are atomic, which means that
if selected they must be totally supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff; die Folge für die Unterstützung
steht im Item zu §2.

### §4 Building block set

**Spec:** §4, S. 11 — "A building block set is a selection of building blocks
that determines a specific XSD schema usage."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff; der einzige Block Set steht in den
Items zu §8.1.

---

## §5 Symbols

### §5 Table 5.1 Acronyms

**Spec:** §5, S. 11 — DDS, ISO, LwCCM, OMG, QoS, UTF, XML, XSD, XTypes.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Abkürzungen.

---

## §6 Additional Information

### §6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, S. 12 — "This specification does not change any adopted OMG
specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis.

### §6.2 Acknowledgments

**Spec:** §6.2, S. 12 — einreichende Firmen (Real-Time Innovations, Twin Oaks
Computing, Jackrabbit Consulting).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Danksagung.

---

## §7.1.1 General Rules

### §7.1.1 Wohlgeformtes Dokument nach [XML] Abschnitt 2.1

**Spec:** §7.1.1, S. 14 — "The XML representation of DDS-related resources
must follow these syntax rules: It shall be a well-formed XML document
according to the criteria defined in clause 2.1 of [XML]."

**Repo:** `crates/xml/src/parser.rs::parse_xml_tree` (über `roxmltree`), `precheck_depth`, `check_tree_depth`, `check_entity_declarations`; `crates/xml/src/sample.rs::serialize_sample` (der einzige Schreiber)

**Tests:** `crates/xml/src/parser.rs::tests::invalid_xml_rejected`, `crates/xml/src/parser.rs::tests::parse_with_xml_declaration`, `crates/xml/src/parser.rs::tests::dtd_and_internal_entity_accepted`, `crates/xml/src/parser.rs::tests::entity_expansion_is_bounded`, `crates/xml/src/parser.rs::tests::external_entity_reference_rejected`, `crates/xml/src/parser.rs::tests::deep_nesting_in_cdata_does_not_fool_the_depth_check`, `crates/xml/tests/fuzz_smoke.rs::deep_nesting_is_refused_by_every_entry_point`, `crates/xml/tests/schema_validation.rs::serialize_sample_output_validates`

**Status:** done — ein nicht wohlgeformtes Dokument endet mit `XmlError::InvalidXml`. Eine DTD ist erlaubt ([XML] 2.1); interne Entitäten sind begrenzt (Wert höchstens `MAX_ENTITY_VALUE_BYTES`, höchstens `MAX_ENTITY_DECLARATIONS` Deklarationen, kein Verweis einer Entität auf eine andere), externe werden nicht geladen. Der Leser weist daneben wohlgeformte Dokumente ab, die seine Grenzen überschreiten: mehr als 64 Ebenen (`MAX_TREE_DEPTH`; `precheck_depth` zählt die offenen Elemente iterativ, bevor `roxmltree` läuft, `check_tree_depth` prüft den Baum danach), mehr als 1024 Kinder eines Elements (`MAX_LIST_ELEMENTS`) oder mehr als 65 536 Elemente (`MAX_TOTAL_ELEMENTS`). Commits `3aec071e6f8`, `80ea5f7343e`, `24780539a95`. Der Test `serialize_sample_output_validates` braucht `xmllint`.

### §7.1.1 UTF-8

**Spec:** §7.1.1, S. 14 — "It shall use UTF-8 character encoding for XML
elements and values."

**Repo:** `crates/xml/src/parser.rs::parse_xml_tree` (nimmt `&str`); `crates/xml/src/registry.rs::QosProfileRegistry::from_file` (`read_to_string`); `crates/xml/src/xsd_loader.rs::load_type_libraries_from_uri` (`from_utf8`); `crates/xml/src/sample.rs::xml_escape`

**Tests:** `crates/xml/tests/types_and_samples.rs::t24_sample_string_escaping_roundtrip`, `crates/xml/src/xsd_loader.rs::tests::data_uri_non_utf8_body_rejected`

**Status:** done — die Eingabe ist eine Zeichenkette von Rust und damit UTF-8; `from_file` und `load_type_libraries_from_uri` weisen Bytes ab, die kein UTF-8 sind; der Schreiber gibt jedes Zeichen über U+007F als Zeichenreferenz aus. Die Angabe `encoding` der XML-Deklaration steht im Item unten. Commit `80ea5f7343e`.

### §7.1.1 Angabe einer anderen Kodierung

**Spec:** §7.1.1, S. 14 — UTF-8 für Elemente und Werte.

**Repo:** `crates/xml/src/parser.rs::check_declared_encoding`; `crates/xml/src/xsd_loader.rs` (`percent_decode`)

**Tests:** `crates/xml/src/parser.rs::tests::non_utf8_declared_encoding_rejected`, `crates/xml/src/xsd_loader.rs::tests::data_uri_non_utf8_body_rejected`

**Status:** done — eine XML-Deklaration mit einem anderen `encoding` als UTF-8 (`utf-8` und `utf8` in beliebiger Schreibweise) endet mit `XmlError::InvalidXml`, denn `roxmltree` liest die Angabe und verwirft sie. Eine `data:`-URI mit Bytes außerhalb von UTF-8 wird abgewiesen, nicht durch U+FFFD ersetzt. Commit `80ea5f7343e`.

---

## §7.1.2 XML Schema Definition Files

### §7.1.2 Zwei normative XSD-Dateien je Building Block

**Spec:** §7.1.2, S. 14 — "This specification makes use of XML Schema
Definition (XSD) language specified in [XSD-1] and [XSD-2] to represent the
syntax of the different building blocks that define DDS resources. In
particular, each building block contains two normative XSD files that define
their syntax (see sub clause 7.3.1)."

**Repo:** `crates/xml/src/schemas.rs::ALL_SCHEMAS`; `crates/xml/schemas/`

**Tests:** `crates/xml/src/schemas.rs::tests::all_schemas_includes_six_building_blocks_plus_system`, `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/tests/schema_validation.rs::omg_examples_validate_against_in_tree_schemas`

**Status:** done — für jeden der sechs Building Blocks und für den Block Set von §8.1 liegen zwei Dateien unter `crates/xml/schemas/`; die zwölf Dateien für QoS, Types, Domains, DomainParticipants, Applications und DDS System sind die Dateien der OMG, Byte für Byte (`dds-xml_<block>_definitions.xsd` und `…_nonamespace.xsd`). Für Data Samples gibt es kein normatives Paar (§7.3.7.3.1: kein allgemeines XSD); ZeroDDS liefert ein eigenes Paar und `dds-xml_common.xsd`, das nur dieses Paar einbindet. Die XSD-Dateien der OMG binden sich über die absolute URL `http://www.omg.org/spec/DDS-XML/20180501/` ein; `schemas::building_block_includes` bildet sie auf Dateinamen ab. Commits `8bf46985f11`, `efd043b8ec4`. `every_in_tree_schema_compiles` und `omg_examples_validate_against_in_tree_schemas` brauchen `xmllint`.

### §7.1.2 Namen der normativen XSD-Dateien

**Spec:** §7.3.3.3, S. 25; §7.3.4.3, S. 25; §7.3.5.3, S. 26; §7.3.6.3,
S. 28 — `dds-xml_type_definitions[_nonamespace].xsd`,
`dds-xml_domain_definitions[_nonamespace].xsd`,
`dds-xml_domainparticipant_definitions[_nonamespace].xsd`,
`dds-xml_application_definitions[_nonamespace].xsd`.

**Repo:** `crates/xml/src/schemas.rs::ALL_SCHEMAS`; `crates/xml/schemas/`

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`, `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`

**Status:** done — die Dateien tragen die Namen der OMG: `dds-xml_type_definitions*.xsd`, `dds-xml_domain_definitions*.xsd`, `dds-xml_domainparticipant_definitions*.xsd`, `dds-xml_application_definitions*.xsd`, `dds-xml_qos_definitions*.xsd`, `dds-xml_dds_system_definitions*.xsd`. Die Spec ist selbst uneinheitlich (S. 1 führt `dds-xml_dds_qos_definitions_nonamespace.xsd` neben `dds-xml_qos_definitions.xsd`; §7.3.5.3 schreibt `…domainparticipant_defintions…`, §7.3.6.3 `…definititons…`); maßgeblich sind die Namen, mit denen die Dateien der OMG sich gegenseitig einbinden. Commit `efd043b8ec4`.

---

## §7.1.3 XML Chameleon Schema Definition Pattern (non-normative)

### §7.1.3.1 Motivation

**Spec:** §7.1.3.1, S. 14 — Namensräume in XSD und das Muster der
Chameleon-Schemas ohne `targetNamespace`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — „(non-normative)“ im Titel von §7.1.3. Wie
die normativen Dateien das Muster verwenden, steht in den Items zu §7.3.1.

### §7.1.3.2.1 Beispiel mit mehreren Namensräumen

**Spec:** §7.1.3.2.1, S. 14–15 — Schema und Dokument mit drei Namensräumen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Es schreibt `<kind>RELIABLE_RELIABILITY</kind>` (S. 15); die IDL von DDS 1.4 nennt das Literal `RELIABLE_RELIABILITY_QOS`, und so schreibt es §7.3.2.4.1.1 (S. 23). `parse_reliability` weist `RELIABLE_RELIABILITY` mit `XmlError::BadEnum` ab.

### §7.1.3.2.2 Beispiel mit Chameleon-Dateien

**Spec:** §7.1.3.2.2, S. 15–16 — eingebundene Dateien ohne
`targetNamespace`, Dokument mit nur einem Namensraum.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Die Leser von ZeroDDS prüfen den
Namensraum nicht (Item zu §7.3.1); ein Dokument ohne Namensraum wird gelesen
wie eines im Namensraum der Spec.

---

## §7.1.4 XML Element Values

### §7.1.4 Table 7.1 boolean: ja als `1` oder `true`

**Spec:** §7.1.4, Table 7.1, S. 16 — "boolean | Yes: 1 or true. | Values are
case sensitive."

**Repo:** `crates/xml/src/qos_parser.rs::parse_bool_strict`, aufgerufen in `parse_presentation`, `parse_entity_factory`, `parse_writer_data_lifecycle` und für Data Samples in `crates/xml/src/sample.rs::parse_primitive_value`; `crates/xml/src/types.rs::parse_bool`

**Tests:** `crates/xml/src/qos_parser.rs::tests::parse_bool_strict_accepts_only_lowercase`, `crates/xml/src/types.rs::tests::bool_invalid`, `crates/xml/tests/qos_profile.rs::t14_boolean_case_sensitive_strict`

**Status:** done — `True` und `TRUE` werden abgewiesen. `crates/xml/src/types.rs::parse_bool`, von `lib.rs` exportiert, nimmt nur `1`, `true`, `0` und `false` an. Commit `80ea5f7343e`.

### §7.1.4 Table 7.1 boolean: nein als `0` oder `false`

**Spec:** §7.1.4, Table 7.1, S. 16 — "No: 0 or false. | Values are case
sensitive."

**Repo:** `crates/xml/src/qos_parser.rs::parse_bool_strict`

**Tests:** `crates/xml/src/qos_parser.rs::tests::parse_bool_strict_accepts_only_lowercase`,
`crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — `False` und `FALSE` werden abgewiesen.

### §7.1.4 Table 7.1 enum: Literal der DCPS-IDL als Zeichenkette

**Spec:** §7.1.4, Table 7.1, S. 16 — "enum | A string. Legal values are the
ones defined for QoS Policies in the DCPS IDL of DDS specification [DDS]. |
Must be specified as a string. (Do not use numeric values.)"

**Repo:** `crates/xml/src/qos_parser.rs`: `parse_durability`, `parse_history`, `parse_reliability`, `parse_liveliness`, `parse_destination_order`, `parse_ownership`, `parse_presentation`, `parse_durability_service`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`, `crates/xml/src/registry.rs::tests::resolves_base_writer_qos`

**Status:** done — jeder Leser nimmt das Literal der DCPS-IDL als Zeichenkette; alle Literale der sieben Aufzählungen liest `every_literal_of_the_dcps_enumerations_is_read` (Items zu §7.2.1). Commit `d2c3d3a9bef`.

### §7.1.4 Table 7.1 enum: nur Literale der DCPS-IDL

**Spec:** §7.1.4, Table 7.1, S. 16 — "Legal values are the ones defined for
QoS Policies in the DCPS IDL".

**Repo:** `crates/xml/src/qos_parser.rs`: die Leser je Policy

**Tests:** `crates/xml/src/qos_parser.rs::tests::short_enum_spellings_rejected`, `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — die Kurzformen (`VOLATILE`, `KEEP_LAST`, `RELIABLE`, …) sind keine Werte; sie enden mit `XmlError::BadEnum`. Ein `<history>` ohne `<kind>` und ein leeres `<kind/>` gelten als Vorgabe der DDS (`KEEP_LAST_HISTORY_QOS`), nicht als Kurzform. Eine Herstellerform ist das nicht: die Kurzformen sind auch mit `VendorForms::Accept` nicht zu lesen. Commit `d2c3d3a9bef`.

### §7.1.4 Table 7.1 enum: keine Zahlen

**Spec:** §7.1.4, Table 7.1, S. 16 — "(Do not use numeric values.)"

**Repo:** `crates/xml/src/qos_parser.rs`: die Leser je Policy

**Tests:** `crates/xml/tests/spec_items.rs::a_number_is_no_enumeration_value`

**Status:** done — eine Zahl als Wert eines Elements `kind` endet mit `XmlError::BadEnum`. Commit `d2c3d3a9bef`.

### §7.1.4 Table 7.1 long: dezimal -2147483648 bis 2147483647

**Spec:** §7.1.4, Table 7.1, S. 16 — "long | -2147483648 to 2147483647 [...]
| A 32-bit signed integer."

**Repo:** `crates/xml/src/types.rs::parse_long`, aufgerufen für die
Felder vom Typ `long` der Policies (`crates/xml/src/qos_parser.rs`)

**Tests:** `crates/xml/src/types.rs::tests::long_decimal`,
`crates/xml/src/types.rs::tests::long_invalid`,
`crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — Data Samples lesen `long` mit eigenem Code (Items zu
§7.3.7.3.1).

### §7.1.4 Table 7.1 long: hexadezimal 0x80000000 bis 0x7fffffff

**Spec:** §7.1.4, Table 7.1, S. 16 — "or 0x80000000 to 0x7fffffff".

**Repo:** `crates/xml/src/types.rs::parse_long`

**Tests:** `crates/xml/src/types.rs::tests::long_hex`

**Status:** done — `0x80000000` bis `0xFFFFFFFF` werden als Zweierkomplement
gelesen (`0x80000000` = -2147483648); `0x0` bis `0x7FFFFFFF` als positiver
Wert.

### §7.1.4 Table 7.1 long: LENGTH_UNLIMITED

**Spec:** §7.1.4, Table 7.1, S. 16 — "or LENGTH_UNLIMITED."

**Repo:** `crates/xml/src/types.rs::parse_long`

**Tests:** `crates/xml/src/types.rs::tests::long_length_unlimited_symbol`,
`crates/xml/tests/qos_profile.rs::t11_length_unlimited_in_resource_limits`

**Status:** done — der Name ergibt -1.

### §7.1.4 Table 7.1 unsigned long: dezimal

**Spec:** §7.1.4, Table 7.1, S. 16 — "unsigned long | 0 to 4294967296 [...] |
A 32-bit unsigned integer."

**Repo:** `crates/xml/src/types.rs::parse_ulong`

**Tests:** `crates/xml/src/types.rs::tests::ulong_decimal_and_hex`,
`crates/xml/src/types.rs::tests::ulong_invalid`

**Status:** done — Bereich 0 bis 4294967295. Die Tabelle nennt 4294967296
als obere Grenze; das ist 2^32 und passt nicht in 32 Bit ohne Vorzeichen
(nachgerechnet). `ulong_invalid` weist 4294967296 ab. In der QoS-IDL hat nur
`Duration_t.nanosec` den Typ `unsigned long`; dafür gilt das Item zu §7.2.6.

### §7.1.4 Table 7.1 unsigned long: hexadezimal

**Spec:** §7.1.4, Table 7.1, S. 16 — "or 0 to 0xffffffff."

**Repo:** `crates/xml/src/types.rs::parse_ulong`

**Tests:** `crates/xml/src/types.rs::tests::ulong_decimal_and_hex`

**Status:** done

### §7.1.4 Table 7.1 string: Maskierung nach [XML]

**Spec:** §7.1.4, Table 7.1, S. 16 — "string | The string with the reserved
XML characters escaped according to the standard rules for element content
[XML]. | Per the XML rules only < and & are required to be escaped within an
element content. The characters >, ', and " may be escaped."

**Repo:** `crates/xml/src/parser.rs::build_element`; `crates/xml/src/sample.rs::xml_escape`

**Tests:** `crates/xml/tests/types_and_samples.rs::t24_sample_string_escaping_roundtrip`,
`crates/xml/tests/schema_validation.rs::serialize_sample_output_validates`

**Status:** done — maskierte Zeichen und Zeichenreferenzen löst `roxmltree` auf; der Schreiber maskiert `<`, `>`, `&`, `"`, `'` und jedes Zeichen über U+007F. Der Leser verändert den Leerraum einer Zeichenkette nicht (Item unten).

### §7.1.4 Table 7.1 string: Leerraum und Kommentar im Wert

**Spec:** §7.1.4, Table 7.1, S. 16 — "The string with the reserved XML
characters escaped".

**Repo:** `crates/xml/src/parser.rs::build_element`; `crates/xml/src/sample.rs::parse_primitive_value`

**Tests:** `crates/xml/src/parser.rs::tests::element_text_preserves_whitespace`, `crates/xml/src/parser.rs::tests::element_text_joins_runs_across_comments`

**Status:** done — `build_element` speichert den Text eines Elements unverändert, mit seinem Leerraum am Rand; die Läufe von Text um einen Kommentar herum werden zusammengefügt (`<a>x<!-- c -->y</a>` ist `xy`). Eine Zeichenkette in Data Samples behält ihren Leerraum (`PrimitiveValue::Str`). Die Zahlen und Namen in QoS werden beim Lesen gekürzt. Commit `8771c676858`.

---

## §7.1.5 XML Attribute Values

### §7.1.5 Table 7.2 boolean: ja als `1` oder `true`

**Spec:** §7.1.5, Table 7.2, S. 17 — "boolean | Yes: 1 or true. | Values are
case sensitive."

**Repo:** `crates/xml/src/xtypes_parser.rs::opt_bool` (über `qos_parser::parse_bool_strict`) für `key`, `optional`, `mustUnderstand`, `nested`, `external`, `nonSerialized`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::spec_7_1_5_1_boolean_attribute_is_case_sensitive`

**Status:** done — die Building Blocks QoS bis Applications haben in der PDF kein Attribut vom Typ `boolean`; die Attribute in `<types>` folgen [DDS-XTYPES] und werden mit `parse_bool_strict` gelesen (`True` wird abgewiesen).

### §7.1.5 Table 7.2 boolean: nein als `0` oder `false`

**Spec:** §7.1.5, Table 7.2, S. 17 — "No: 0 or false. | Values are case
sensitive."

**Repo:** `crates/xml/src/xtypes_parser.rs::opt_bool`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::spec_7_1_5_1_boolean_attribute_is_case_sensitive`

**Status:** done — wie das Item zu `1` oder `true`.

### §7.1.5 Table 7.2 enum: Zeichenkette

**Spec:** §7.1.5, Table 7.2, S. 17 — "enum | A string. Legal values are the
ones defined for QoS Policies in the DCPS IDL of DDS specification [DDS]. |
Must be specified as a string. (Do not use numeric values.)"

**Repo:** `crates/xml/src/xtypes_parser.rs::extensibility_attr`, `parse_try_construct`, `parse_auto_id`, `parse_forward_dcl`

**Tests:** `crates/xml/tests/types_and_samples.rs::t03_extensibility_final_appendable_mutable`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`

**Status:** done — die PDF nennt kein Attribut, dessen Wert ein Literal der QoS-IDL ist. Die Attribute mit Aufzählungswerten in `<types>` (`extensibility`, `tryConstruct`, `autoid`, `kind` von `forward_dcl`) werden als Zeichenketten gelesen und gegen die Werte der XSD geprüft; die Schreibweisen außerhalb der XSD sind Herstellerformen (`extensibility="appendable"`, `kind="STRUCT"`). Commits `32aa062decf`, `af517d979de`, `e34118cc9a6`.

### §7.1.5 Table 7.2 enum: Schreibweise und Zahlen

**Spec:** §7.1.5, Table 7.2, S. 17 — "(Do not use numeric values.)"

**Repo:** `crates/xml/src/xtypes_parser.rs::extensibility_attr`, `parse_forward_dcl`

**Tests:** `crates/xml/tests/types_and_samples.rs::t03_extensibility_uppercase_rejected`, `crates/xml/tests/spec_items.rs::a_number_is_no_extensibility`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`

**Status:** done — `FINAL`, `APPENDABLE`, `MUTABLE` und eine Zahl enden mit `XmlError::BadEnum`; `appendable` ist eine Herstellerform von `append`; `kind` von `forward_dcl` wird gegen `enum`, `struct`, `union`, `bitmask` geprüft. Commits `d2c3d3a9bef`, `af517d979de`, `e34118cc9a6`.

### §7.1.5 Table 7.2 long

**Spec:** §7.1.5, Table 7.2, S. 17 — "long | -2147483648 to 2147483647 or
0x80000000 to 0x7fffffff or LENGTH_UNLIMITED. | A 32-bit signed integer."

**Repo:** `crates/xml/src/types.rs::parse_long`, für das Attribut
`value` von `<enumerator>` (`crates/xml/src/xtypes_parser.rs`)

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::spec_7_1_5_3_long_attribute_takes_decimal_hex_and_length_unlimited`

**Status:** done

### §7.1.5 Table 7.2 unsigned long

**Spec:** §7.1.5, Table 7.2, S. 17 — "unsigned long | 0 to 4294967296 or 0 to
0xffffffff. | A 32-bit unsigned integer."

**Repo:** `crates/xml/src/types.rs::parse_ulong`, für `domain_id` (`crates/xml/src/domain.rs::parse_domain_element`, `crates/xml/src/participant.rs::parse_dp_element`) und für `id` und `bitBound` in `<types>`

**Tests:** `crates/xml/src/domain.rs::tests::domain_id_accepts_the_unsigned_long_range`, `crates/xml/src/types.rs::tests::ulong_decimal_and_hex`

**Status:** done — `domain_id` und `id` und `bitBound` in `<types>` werden mit `parse_ulong` gelesen (dezimal oder hexadezimal, bis 4294967295). Die Grenze 232 für `domain_id` ist entfallen: sie stammt aus RTPS, nicht aus DDS-XML. Commit `80ea5f7343e`.

### §7.1.5 Table 7.2 string

**Spec:** §7.1.5, Table 7.2, S. 17 — "string | The string with the reserved
XML characters escaped according to the standard rules for element content
[XML]."

**Repo:** `crates/xml/src/parser.rs::build_element`

**Tests:** `crates/xml/tests/spec_items.rs::attribute_values_keep_their_characters_and_spaces`, `crates/xml/src/parser.rs::tests::attributes_preserved`

**Status:** done — `build_element` übernimmt Attributwerte so, wie `roxmltree` sie nach dem Auflösen der Entitäten und der Zeichenreferenzen liefert, ohne Kürzen. Die Attribute der Namensräume `xsi` und `xml` sind kein Inhalt und fallen weg. Commit `8bf46985f11`.

---

## §7.2 XML Representation of Resources Defined in the DDS IDL PSM

### §7.2 1:1-Abbildung der Typen der DDS-IDL

**Spec:** §7.2, S. 17 — "The XML representation of resources that correspond
to data-types defined in the DDS IDL PSM [DDS] is obtained by performing a
1-to-1 mapping of the corresponding IDL data type."

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_qos`; `crates/xml/src/qos.rs::EntityQos`; `crates/xml/src/conformance.rs::IDL_TO_XML_MAPPING`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`, `crates/xml/tests/qos_profile.rs::t09b_policy_outside_the_container_is_refused`, `crates/xml/src/conformance.rs::tests::idl_mapping_names_the_sections_of_7_2`

**Status:** done — die 22 Policies von DDS 1.4 werden unter den Namen der IDL gelesen, je Container nur die Policies, die ein Feld der IDL-Struktur sind; die Aufzählungen, Konstanten, Folgen und `Duration_t` stehen in den Items darunter. `IDL_TO_XML_MAPPING` nennt je Kategorie die Funktion. Commits `d2c3d3a9bef`, `8771c676858`.

---

## §7.2.1 XML Representation of Enumeration types

### §7.2.1 Aufzählung als simpleType in der XSD

**Spec:** §7.2.1, S. 17 — "IDL Enumerations are represented in XML according
to a schema defined as an XSD simpleType defined as a restriction of a string
that can take values of the enumeration literals."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`

**Tests:** `crates/xml/tests/schema_validation.rs::xsd_valid_documents_are_read`

**Status:** done — die XSD der OMG definiert die simpleTypes der Aufzählungen (`durabilityKind`, `historyKind`, …) und liegt unverändert im Baum; der Leser prüft die Literale selbst. Commit `8bf46985f11`.

### §7.2.1 DurabilityQosPolicyKind

**Spec:** §7.2.1, S. 17; DDS 1.4 IDL, S. 157 — `VOLATILE_DURABILITY_QOS`,
`TRANSIENT_LOCAL_DURABILITY_QOS`, `TRANSIENT_DURABILITY_QOS`,
`PERSISTENT_DURABILITY_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_durability`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`

**Status:** done — `parse_durability` liest `VOLATILE_DURABILITY_QOS`, `TRANSIENT_LOCAL_DURABILITY_QOS`, `TRANSIENT_DURABILITY_QOS`, `PERSISTENT_DURABILITY_QOS`; `every_literal_of_the_dcps_enumerations_is_read` liest jedes. Commit `d2c3d3a9bef`.

### §7.2.1 PresentationQosPolicyAccessScopeKind

**Spec:** §7.2.1, S. 17; DDS 1.4 IDL, S. 158 — `INSTANCE_PRESENTATION_QOS`,
`TOPIC_PRESENTATION_QOS`, `GROUP_PRESENTATION_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_presentation`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`

**Status:** done — `parse_presentation` liest `INSTANCE_PRESENTATION_QOS`, `TOPIC_PRESENTATION_QOS`, `GROUP_PRESENTATION_QOS`; `every_literal_of_the_dcps_enumerations_is_read` liest jedes. Commit `d2c3d3a9bef`.

### §7.2.1 OwnershipQosPolicyKind

**Spec:** §7.2.1, S. 17; DDS 1.4 IDL, S. 158 — `SHARED_OWNERSHIP_QOS`,
`EXCLUSIVE_OWNERSHIP_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_ownership`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`

**Status:** done — `parse_ownership` liest `SHARED_OWNERSHIP_QOS`, `EXCLUSIVE_OWNERSHIP_QOS`; `every_literal_of_the_dcps_enumerations_is_read` liest jedes. Commit `d2c3d3a9bef`.

### §7.2.1 LivelinessQosPolicyKind

**Spec:** §7.2.1, S. 17; DDS 1.4 IDL, S. 158 — `AUTOMATIC_LIVELINESS_QOS`,
`MANUAL_BY_PARTICIPANT_LIVELINESS_QOS`, `MANUAL_BY_TOPIC_LIVELINESS_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_liveliness`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`

**Status:** done — `parse_liveliness` liest `AUTOMATIC_LIVELINESS_QOS`, `MANUAL_BY_PARTICIPANT_LIVELINESS_QOS`, `MANUAL_BY_TOPIC_LIVELINESS_QOS`; `every_literal_of_the_dcps_enumerations_is_read` liest jedes. Commit `d2c3d3a9bef`.

### §7.2.1 ReliabilityQosPolicyKind

**Spec:** §7.2.1, S. 17; DDS 1.4 IDL, S. 158–159 —
`BEST_EFFORT_RELIABILITY_QOS`, `RELIABLE_RELIABILITY_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_reliability`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`, `crates/xml/tests/qos_profile.rs::t19_empty_policies_use_spec_defaults`, `crates/xml/src/registry.rs::tests::resolves_base_writer_qos`

**Status:** done — `parse_reliability` liest beide Literale; ein Test liest jedes. Ohne `<kind>` gilt die Vorgabe der DDS: am DataWriter `RELIABLE`, sonst `BEST_EFFORT` (DDS 1.4 §2.2.3.14). Commits `d2c3d3a9bef`, `b956dbaceed`.

### §7.2.1 ReliabilityQosPolicyKind: BEST_EFFORT_RELIABILITY_QOS

**Spec:** §7.2.1, S. 17; DDS 1.4 IDL, S. 158 — `BEST_EFFORT_RELIABILITY_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_reliability`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`, `crates/xml/tests/qos_profile.rs::t18_into_writer_and_reader_qos_use_spec_defaults`

**Status:** done — `parse_reliability` liest das Literal; `every_literal_of_the_dcps_enumerations_is_read` und `t18_into_writer_and_reader_qos_use_spec_defaults` lesen es.

### §7.2.1 DestinationOrderQosPolicyKind

**Spec:** §7.2.1, S. 17; DDS 1.4 IDL, S. 159 —
`BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS`,
`BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_destination_order`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`

**Status:** done — `parse_destination_order` liest `BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS`, `BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS`; `every_literal_of_the_dcps_enumerations_is_read` liest jedes. Commit `d2c3d3a9bef`.

### §7.2.1 HistoryQosPolicyKind

**Spec:** §7.2.1, S. 17; DDS 1.4 IDL, S. 159 — `KEEP_LAST_HISTORY_QOS`,
`KEEP_ALL_HISTORY_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_history`; `crates/xml/src/qos_parser.rs::parse_durability_service`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`, `crates/xml/src/registry.rs::tests::resolves_base_writer_qos`

**Status:** done — `parse_history` und `history_kind` von `parse_durability_service` lesen beide Literale; `every_literal_of_the_dcps_enumerations_is_read` liest jedes an beiden Stellen. Commit `d2c3d3a9bef`.

### §7.2.1 HistoryQosPolicyKind: KEEP_ALL_HISTORY_QOS

**Spec:** §7.2.1, S. 17; DDS 1.4 IDL, S. 159 — `KEEP_ALL_HISTORY_QOS`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_history`

**Tests:** `crates/xml/tests/spec_items.rs::every_literal_of_the_dcps_enumerations_is_read`, `crates/xml/tests/spec_items.rs::qos_library_example`

**Status:** done — `parse_history` liest das Literal; `every_literal_of_the_dcps_enumerations_is_read`, `t21_repeated_containers_select_by_topic_filter` und `qos_library_example` lesen es.

### §7.2.1.1 Beispiel (Non-normative)

**Spec:** §7.2.1.1, S. 17 — `HistoryQosPolicyKind` als simpleType
`historyKind`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Der Text nennt den simpleType
`historyQosPolicyKind`, das XSD darunter `historyKind`.

---

## §7.2.2 XML Representation of Primitive Constants

### §7.2.2 Konstanten als simpleType: Zahl oder Name

**Spec:** §7.2.2, S. 17–18 — "These constant definitions appear in the XSD as
simpleType schemas defined as restrictions of xs:string to provide custom
syntax allowing an element to have a value that is either given as a number
or as the name of the constant."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`; `crates/xml/src/types.rs`

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/src/types.rs::tests::duration_accepts_the_named_constants`

**Status:** done — die vier simpleTypes stehen in der XSD der OMG (`nonNegativeInteger_UNLIMITED`, `positiveInteger_UNLIMITED`, `nonNegativeInteger_Duration_SEC`, `nonNegativeInteger_Duration_NSEC`) und werden von den Elementen der Policies benutzt. Der Leser nimmt Zahl oder Name; `max_samples`, `max_instances` und `max_samples_per_instance` lesen nach `positiveInteger_UNLIMITED` (1 oder mehr, oder der Name; Commit `2120724498f`); die Namen, die das Muster nicht nennt (`DURATION_ZERO_*`, `TIME_INVALID_*`), nimmt er an, weil der Satz von §7.2.2 „the name of the constant“ zulässt. Commits `8bf46985f11`, `4e479ef9e18`.

### §7.2.2 LENGTH_UNLIMITED = -1

**Spec:** §7.2.2.1, S. 18 — "const long LENGTH_UNLIMITED = -1; [...] The
constant LENGTH_UNLIMITED is intended to initialize structure members that
represent lengths."

**Repo:** `crates/xml/src/types.rs::LENGTH_UNLIMITED`; `crates/xml/src/types.rs::parse_long`

**Tests:** `crates/xml/src/types.rs::tests::long_length_unlimited_symbol`,
`crates/xml/src/types.rs::tests::spec_constants`,
`crates/xml/tests/qos_profile.rs::t11_length_unlimited_in_resource_limits`

**Status:** done — `parse_long` nimmt den Namen an jedem Feld vom Typ `long` an, auch an Feldern, die keine Länge sind (z. B. `ownership_strength`); die XSD der OMG hat den Namen nur an den Längen. Das Feld ist in der IDL ein `long`, §7.2.2 lässt den Namen an jedem zu.

### §7.2.2 DURATION_INFINITE_SEC = 0x7fffffff

**Spec:** §7.2.2.1, S. 18 — "const long DURATION_INFINITE_SEC = 0x7fffffff;"

**Repo:** `crates/xml/src/types.rs::DURATION_INFINITE_SEC`;
`crates/xml/src/types.rs::parse_duration_sec`

**Tests:** `crates/xml/src/types.rs::tests::duration_sec_infinite_symbols`,
`crates/xml/src/types.rs::tests::spec_constants`

**Status:** done

### §7.2.2 DURATION_INFINITE_NSEC = 0x7fffffff

**Spec:** §7.2.2.1, S. 18 — "const unsigned long DURATION_INFINITE_NSEC =
0x7fffffff;"

**Repo:** `crates/xml/src/types.rs::DURATION_INFINITE_NSEC`;
`crates/xml/src/types.rs::parse_duration_nsec`

**Tests:** `crates/xml/src/types.rs::tests::duration_nsec_infinite`,
`crates/xml/src/types.rs::tests::spec_constants`

**Status:** done

### §7.2.2 DURATION_ZERO_SEC = 0

**Spec:** §7.2.2.1, S. 18 — "const long DURATION_ZERO_SEC = 0;"

**Repo:** `crates/xml/src/types.rs::DURATION_ZERO_SEC`; `crates/xml/src/types.rs::parse_duration_sec`

**Tests:** `crates/xml/src/types.rs::tests::duration_accepts_the_named_constants`, `crates/xml/src/types.rs::tests::duration_constants`

**Status:** done — `parse_duration_sec` nimmt den Namen an. Commit `4e479ef9e18`.

### §7.2.2 DURATION_ZERO_NSEC = 0

**Spec:** §7.2.2.1, S. 18 — "const unsigned long DURATION_ZERO_NSEC = 0;"

**Repo:** `crates/xml/src/types.rs::DURATION_ZERO_NSEC`; `crates/xml/src/types.rs::parse_duration_nsec`

**Tests:** `crates/xml/src/types.rs::tests::duration_accepts_the_named_constants`, `crates/xml/src/types.rs::tests::duration_constants`

**Status:** done — `parse_duration_nsec` nimmt den Namen an. Commit `4e479ef9e18`.

### §7.2.2 TIME_INVALID_SEC = -1

**Spec:** §7.2.2.1, S. 18 — "const long TIME_INVALID_SEC = -1; [...] the
constants with the prefix TIME_ are intended to initialize the members
(second and nanosecond) of the structure Time_t."

**Repo:** `crates/xml/src/types.rs::TIME_INVALID_SEC`; `crates/xml/src/sample.rs::parse_long_sample`

**Tests:** `crates/xml/src/types.rs::tests::time_invalid_constants`, `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`

**Status:** done — `parse_duration_sec` und, für ein Feld vom Typ `long` in Data Samples, `parse_long_sample` nehmen den Namen an. Die PDF nennt kein Element vom Typ `Time_t` in den Building Blocks QoS bis Applications. Commits `4e479ef9e18`, `8771c676858`.

### §7.2.2 TIME_INVALID_NSEC = 0xffffffff

**Spec:** §7.2.2.1, S. 18 — "const unsigned long TIME_INVALID_NSEC =
0xffffffff;"

**Repo:** `crates/xml/src/types.rs::TIME_INVALID_NSEC`; `crates/xml/src/sample.rs::parse_ulong_sample`

**Tests:** `crates/xml/src/types.rs::tests::time_invalid_constants`, `crates/xml/src/sample.rs::tests::parse_sample_ulong_constant_names`, `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`

**Status:** done — `parse_ulong_sample` nimmt den Namen an (`unsigned long` in Data Samples). Commit `8771c676858`.

### §7.2.2.1 Beispiel (Non-Normative)

**Spec:** §7.2.2.1, S. 18 — die Konstanten der IDL und die simpleTypes
`nonNegativeInteger_UNLIMITED`, `positiveInteger_UNLIMITED`,
`nonNegativeInteger_Duration_SEC`, `nonNegativeInteger_Duration_NSEC`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Die Muster enthalten `DURATION_INFINITY`, das keine Konstante der IDL ist; der Leser nimmt es an (`parse_duration_sec`, `parse_duration_nsec`). Die Muster lassen eine leere Zeichenkette zu; ein leeres Element gilt als Vorgabe seines Feldes (`empty_numeric_elements_are_the_default`). `max_samples` und seine Geschwister lesen nach `positiveInteger_UNLIMITED` (`parse_positive_long_unlimited`), `parse_long` nimmt die übrigen Felder vom Typ `long`.

---

## §7.2.3 XML Representation of Structure Types

### §7.2.3 Struktur als complexType in der XSD

**Spec:** §7.2.3, S. 18 — "In general, IDL structures are represented in XML
according to a schema defined as an XSD complexType."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`

**Tests:** `crates/xml/tests/schema_validation.rs::xsd_valid_documents_are_read`

**Status:** done — die XSD der OMG hat je Policy einen complexType (z. B. `partitionQosPolicy`, `durabilityServiceQosPolicy`) und je Container einen (`datawriterQos`, …); sie liegt unverändert im Baum. Commit `8bf46985f11`.

### §7.2.3 Elementname gleich Feldname

**Spec:** §7.2.3, S. 18 — "The fields in an IDL structure become unordered
elements of the complexType with the field name appearing as the
corresponding element name."

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_qos` und die Leser je Policy; `crates/xml/src/qos_parser.rs::only_children`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`, `crates/xml/src/qos_parser.rs::tests::unread_elements_are_refused_at_every_level`, `crates/xml/tests/normative_forms.rs::report_and_skip_lists_everything_it_leaves_out`

**Status:** done — ein unbekanntes Kindelement einer Policy oder eines Containers endet mit `XmlError::UnknownElement` und dem Pfad des Elements; mit `ReadOptions::report_and_skip()` wird es ausgelassen und als `Diagnostic` gemeldet. Commits `8771c676858`, `8bf46985f11`.

### §7.2.3 Felder ungeordnet

**Spec:** §7.2.3, S. 18 — "become unordered elements of the complexType".

**Repo:** `crates/xml/src/qos_parser.rs::check_children`; `crates/xml/src/parser.rs::XmlElement::child`

**Tests:** `crates/xml/tests/spec_items.rs::fields_of_a_structure_come_in_any_order`, `crates/xml/tests/spec_items.rs::a_field_given_twice_is_refused`, `crates/xml/tests/qos_profile.rs::t20_duplicate_structure_field_rejected`

**Status:** done — die Leser suchen die Kinder nach Namen, die Reihenfolge spielt keine Rolle (`fields_of_a_structure_come_in_any_order`); ein Feld, das zweimal vorkommt, endet mit `XmlError::InvalidXml`. Commit `8771c676858`.

### §7.2.3 Geschachtelte Strukturen

**Spec:** §7.2.3, S. 18 — "This mapping is applied recursively for nested
structures."

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration_element`, aufgerufen für jedes Feld vom Typ `Duration_t`

**Tests:** `crates/xml/tests/qos_profile.rs::t01_happy_path_reliability_and_history_roundtrip`,
`crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 Vorgaben der DDS in der XSD

**Spec:** §7.2.3, S. 18 — "If the DDS specification defines default values
for the structure fields, the corresponding XSD element definition shall
provide the same default value."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`

**Status:** done — die XSD der OMG trägt die Vorgaben der DDS als `default=` der Elemente (z. B. `INSTANCE_PRESENTATION_QOS` für `access_scope`) und liegt unverändert im Baum. Commit `8bf46985f11`.

### §7.2.3 Fehlende Policy: Vorgabe der DDS

**Spec:** §7.2.3, S. 18 — "If the DDS specification defines default values
for the structure fields, the corresponding XSD element definition shall
provide the same default value."

**Repo:** `crates/xml/src/qos.rs::EntityQos::into_writer_qos`, `into_reader_qos`; `crates/webdds-bridge/src/qos_update.rs::topic_qos_from`

**Tests:** `crates/xml/src/qos.rs::tests::entity_qos_into_writer_uses_defaults_for_unset`, `crates/xml/src/qos.rs::tests::entity_qos_into_reader_uses_defaults_for_unset`, `crates/xml/tests/qos_profile.rs::t18_into_writer_and_reader_qos_use_spec_defaults`

**Status:** done — eine Policy, die im Dokument fehlt, bleibt `None` in `EntityQos` und gilt als Vorgabe der DDS. `into_writer_qos` und `into_reader_qos` setzen sie ein (Reliability `RELIABLE` am Writer, `BEST_EFFORT` am Reader, History `KEEP_LAST` mit Tiefe 1); für Topic, Publisher, Subscriber und DomainParticipant mischt der Aufrufer `EntityQos` über die Vorgabe der Entität (`webdds-bridge`: `topic_qos_from`, `publisher_qos_from`, `subscriber_qos_from`, `participant_qos_from`). Commit `b956dbaceed`.

### §7.2.3 Fehlendes Feld einer Policy

**Spec:** §7.2.3, S. 18 — Vorgaben der Felder; §7.2.3.1, S. 19 (Felder mit
`minOccurs="0"` und `default`).

**Repo:** `crates/xml/src/qos_parser.rs`: die Leser je Policy; `numeric_text`

**Tests:** `crates/xml/tests/qos_profile.rs::t19_empty_policies_use_spec_defaults`, `crates/xml/tests/qos_profile.rs::t19b_duration_without_sec_is_zero`, `crates/xml/tests/normative_forms.rs::empty_numeric_elements_are_the_default`

**Status:** done — ein Feld, das fehlt, hat die Vorgabe der DDS; `minOccurs="0"` steht an jedem Feld der XSD. Eine Policy ohne Kinder (`<deadline/>`, `<history/>`) liest als Policy mit allen Vorgaben, ein Element mit leerem Text (`<depth/>`) als Vorgabe seines Feldes, ein `Duration_t` ohne `sec` als 0. Eine Aufzählung ohne Literal hat die Vorgabe der DDS. Commits `4e479ef9e18`, `b956dbaceed`.

### §7.2.3.1 Beispiel (Non-normative)

**Spec:** §7.2.3.1, S. 19 — complexType `historyQosPolicy` mit `kind` und
`depth`, beide `minOccurs="0"` mit `default`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Es typisiert `depth` als `xs:positiveInteger`; die IDL hat `long`. `parse_history` verlangt 1 oder mehr (`numbers_outside_the_schemas_types_are_refused`).

### §7.2.3 UserDataQosPolicy: `user_data`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 157 — `struct UserDataQosPolicy {
sequence<octet> value; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_user_data`, `parse_octet_value`

**Tests:** `crates/xml/tests/qos_profile.rs::t13_user_data_base64_to_bytes`, `crates/xml/tests/qos_profile.rs::t13b_user_data_comma_list_to_bytes`, `crates/xml/tests/normative_forms.rs::user_data_value_is_base64_and_may_be_empty`

**Status:** done — Element `user_data` mit `value` (Base64 oder Liste) oder `valueB64` nach §7.2.4.2 (Items dort). Commit `5dcde9b5717`.

### §7.2.3 TopicDataQosPolicy: `topic_data`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 157 — `struct TopicDataQosPolicy {
sequence<octet> value; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_topic_data`, `parse_octet_value`

**Tests:** `crates/xml/tests/spec_items.rs::topic_data_and_group_data_hold_octets_in_both_forms`, `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — Element `topic_data` mit `value` (Base64 oder Liste) oder `valueB64` nach §7.2.4.2 (Items dort). Commit `5dcde9b5717`.

### §7.2.3 GroupDataQosPolicy: `group_data`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 157 — `struct GroupDataQosPolicy {
sequence<octet> value; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_group_data`, `parse_octet_value`

**Tests:** `crates/xml/tests/spec_items.rs::topic_data_and_group_data_hold_octets_in_both_forms`, `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — Element `group_data` mit `value` (Base64 oder Liste) oder `valueB64` nach §7.2.4.2 (Items dort). Commit `5dcde9b5717`.

### §7.2.3 TransportPriorityQosPolicy: `transport_priority`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 157 — `struct
TransportPriorityQosPolicy { long value; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_transport_priority`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 LifespanQosPolicy: `lifespan`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 157 — `struct LifespanQosPolicy {
Duration_t duration; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_lifespan`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 DurabilityQosPolicy: `durability`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 157–158 — `struct
DurabilityQosPolicy { DurabilityQosPolicyKind kind; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_durability`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`,
`crates/xml/tests/qos_profile.rs::t04_three_level_inheritance_propagates`

**Status:** done — die Literale im Item zu §7.2.1.

### §7.2.3 PresentationQosPolicy: `presentation`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 158 — `access_scope`,
`coherent_access`, `ordered_access`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_presentation`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 DeadlineQosPolicy: `deadline`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 158 — `struct DeadlineQosPolicy {
Duration_t period; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_deadline`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`,
`crates/xml/tests/qos_profile.rs::t10_duration_infinity_string_parses_to_infinite`

**Status:** done

### §7.2.3 LatencyBudgetQosPolicy: `latency_budget`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 158 — `struct
LatencyBudgetQosPolicy { Duration_t duration; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_latency_budget`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 OwnershipQosPolicy: `ownership`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 158 — `struct OwnershipQosPolicy {
OwnershipQosPolicyKind kind; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_ownership`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 OwnershipStrengthQosPolicy: `ownership_strength`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 158 — `struct
OwnershipStrengthQosPolicy { long value; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_ownership_strength`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 LivelinessQosPolicy: `liveliness`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 158 — `kind`, `lease_duration`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_liveliness`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 TimeBasedFilterQosPolicy: `time_based_filter`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 158 — `struct
TimeBasedFilterQosPolicy { Duration_t minimum_separation; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_time_based_filter`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 PartitionQosPolicy: `partition`

**Spec:** §7.2.3, S. 18; §7.2.4.1, S. 19; DDS 1.4 IDL, S. 158 — `struct
PartitionQosPolicy { StringSeq name; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_partition`

**Tests:** `crates/xml/tests/qos_profile.rs::t12_partition_multiple_names_become_vec`, `crates/xml/tests/normative_forms.rs::empty_partition_name_is_an_empty_sequence`

**Status:** done — `parse_partition` liest die Folge nach §7.2.4.1: ein `<name>` mit null oder mehr `<element>`. Ein zweites `<name>` endet mit `XmlError::InvalidXml`; die Form mit wiederholtem `<name>` und die mit `<name_list>` sind keine Formen der Spec. Commits `5dcde9b5717`, `7efd1017dd5`.

### §7.2.3 ReliabilityQosPolicy: `reliability`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 159 — `kind`, `max_blocking_time`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_reliability`

**Tests:** `crates/xml/tests/qos_profile.rs::t01_happy_path_reliability_and_history_roundtrip`,
`crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 DestinationOrderQosPolicy: `destination_order`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 159 — `struct
DestinationOrderQosPolicy { DestinationOrderQosPolicyKind kind; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_destination_order`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 HistoryQosPolicy: `history`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 159 — `kind`, `depth`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_history`

**Tests:** `crates/xml/tests/qos_profile.rs::t01_happy_path_reliability_and_history_roundtrip`,
`crates/xml/tests/qos_profile.rs::t03_inheritance_child_overrides_history_keeps_reliability`

**Status:** done

### §7.2.3 ResourceLimitsQosPolicy: `resource_limits`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 159 — `max_samples`,
`max_instances`, `max_samples_per_instance`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_resource_limits`; `crates/xml/src/qos.rs::InitialResourceLimits`

**Tests:** `crates/xml/tests/normative_forms.rs::resource_limits_initial_fields_are_carried`, `crates/xml/tests/qos_profile.rs::t11_length_unlimited_in_resource_limits`

**Status:** done — `initial_samples` und `initial_instances` aus dem Beispiel von §7.3.2.4.3.1 stehen in der XSD der OMG (`resourceLimitsQosPolicy`) und werden gelesen und in `EntityQos::initial_resource_limits` mitgeführt; die IDL von DDS 1.4 hat sie nicht, deshalb erreichen sie `WriterQos` und `ReaderQos` nicht. Commit `8bf46985f11`.

### §7.2.3 EntityFactoryQosPolicy: `entity_factory`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 159 — `struct
EntityFactoryQosPolicy { boolean autoenable_created_entities; };`

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_factory`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`,
`crates/xml/tests/qos_profile.rs::t14_boolean_case_sensitive_strict`

**Status:** done

### §7.2.3 WriterDataLifecycleQosPolicy: `writer_data_lifecycle`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 159 — `struct
WriterDataLifecycleQosPolicy { boolean autodispose_unregistered_instances;
};`

**Repo:** `crates/xml/src/qos_parser.rs::parse_writer_data_lifecycle`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 ReaderDataLifecycleQosPolicy: `reader_data_lifecycle`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 159 —
`autopurge_nowriter_samples_delay`, `autopurge_disposed_samples_delay`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_reader_data_lifecycle`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done

### §7.2.3 DurabilityServiceQosPolicy: `durability_service`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 159 — `service_cleanup_delay`,
`history_kind`, `history_depth`, `max_samples`, `max_instances`,
`max_samples_per_instance`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_durability_service`

**Tests:** `crates/xml/tests/spec_items.rs::durability_service_reads_all_its_fields`, `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — `parse_durability_service` liest alle sechs Felder; `durability_service_reads_all_its_fields` liest sie und prüft vier davon, `t09_all_22_policies_parse` liest die Policy mit der Dauer.

### §7.2.3 DurabilityServiceQosPolicy: `max_instances`, `max_samples_per_instance`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 159 — `long max_instances; long
max_samples_per_instance;`

**Repo:** `crates/xml/src/qos_parser.rs::parse_durability_service`

**Tests:** `crates/xml/tests/spec_items.rs::durability_service_reads_all_its_fields`

**Status:** done — `parse_durability_service` liest beide Felder; `durability_service_reads_all_its_fields` prüft die Werte.

### §7.2.3 DomainParticipantQos: `domain_participant_qos`

**Spec:** §7.2.3, S. 18; §7.3.5.4.4.1, S. 27; DDS 1.4 IDL, S. 160 —
`user_data`, `entity_factory`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`, `parse_single_qos_shortcut`; `crates/xml/src/participant.rs::parse_dp_element`

**Tests:** `crates/xml/tests/dds_xml_full.rs::participant_inheritance_one_level`, `crates/xml/tests/qos_profile.rs::t02_all_six_containers_in_a_profile`, `crates/xml/tests/normative_forms.rs::vendor_forms_are_refused_unless_accepted`

**Status:** done — gelesen wird `domain_participant_qos`, wie die XSD der OMG ihn schreibt; `domainparticipant_qos` und `participant_qos` sind Herstellerformen (`VendorForms`). Commits `e34118cc9a6`, `2de529e05a6`.

### §7.2.3 TopicQos: `topic_qos`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 160 — die 13 Policies von
`TopicQos`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`; `crates/xml/src/domain.rs::parse_topic_entry`

**Tests:** `crates/xml/tests/qos_profile.rs::t02_all_six_containers_in_a_profile`,
`crates/xml/tests/dds_xml_full.rs::topic_qos_inline_and_profile_ref_both_resolve`

**Status:** done

### §7.2.3 DataWriterQos: `datawriter_qos`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 160 — die 15 Policies von
`DataWriterQos`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`; `crates/xml/src/qos.rs::into_writer_qos`

**Tests:** `crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`,
`crates/xml/tests/qos_profile.rs::t18_into_writer_and_reader_qos_use_spec_defaults`

**Status:** done

### §7.2.3 PublisherQos: `publisher_qos`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 160 — `presentation`, `partition`,
`group_data`, `entity_factory`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`; `crates/xml/src/participant.rs::parse_publisher_element`

**Tests:** `crates/xml/tests/qos_profile.rs::t02_all_six_containers_in_a_profile`,
`crates/xml/tests/qos_profile.rs::t12_partition_multiple_names_become_vec`

**Status:** done

### §7.2.3 DataReaderQos: `datareader_qos`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 160–161 — die 12 Policies von
`DataReaderQos`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`; `crates/xml/src/qos.rs::into_reader_qos`

**Tests:** `crates/xml/tests/qos_profile.rs::t04_three_level_inheritance_propagates`,
`crates/xml/src/registry.rs::tests::reader_qos_resolves`

**Status:** done

### §7.2.3 SubscriberQos: `subscriber_qos`

**Spec:** §7.2.3, S. 18; DDS 1.4 IDL, S. 161 — `presentation`, `partition`,
`group_data`, `entity_factory`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_profile_element`; `crates/xml/src/participant.rs::parse_subscriber_element`

**Tests:** `crates/xml/tests/qos_profile.rs::t02_all_six_containers_in_a_profile`

**Status:** done

### §7.2.3 Policies außerhalb der IDL einer QoS-Struktur

**Spec:** §7.2, S. 17 (1:1-Abbildung); DDS 1.4 IDL, S. 160–161.

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_qos`, `allowed_policies`

**Tests:** `crates/xml/tests/qos_profile.rs::t09b_policy_outside_the_container_is_refused`, `crates/xml/tests/normative_forms.rs::report_and_skip_lists_everything_it_leaves_out`

**Status:** done — `parse_entity_qos` liest je Container nur die Policies, die in DDS 1.4 §2.2.3 ein Feld der Struktur sind (`allowed_policies`); eine andere (z. B. `ownership_strength` in `datareader_qos`) endet mit `XmlError::UnknownElement`, unter `report_and_skip` als `DiagnosticKind::MisplacedElement`. Nichts wird gelesen und beim Übernehmen verworfen. Commits `8771c676858`, `8bf46985f11`.

---

## §7.2.4 XML Representation of Sequences

### §7.2.4.1 Folge: null oder mehr Elemente `element`

**Spec:** §7.2.4.1, S. 19 — "The general XML representation of IDL sequences
is done following a schema defined as an XSD complexType. The complexType
contains zero or more elements named element. Nested inside each element is
the XSD schema obtained from mapping the IDL type of the element itself."

**Repo:** `crates/xml/src/qos_parser.rs::parse_partition`; `crates/xml/src/parser.rs::XmlElement::sequence_elements`

**Tests:** `crates/xml/tests/qos_profile.rs::t12_partition_multiple_names_become_vec`, `crates/xml/tests/normative_forms.rs::empty_partition_name_is_an_empty_sequence`, `crates/xml/src/parser.rs::tests::sequence_elements_iterates_element_tag_children`

**Status:** done — die einzige Folge in den QoS-Policies ist `PartitionQosPolicy.name` (`StringSeq`); `parse_partition` liest sie als `<name>` mit `<element>`-Kindern. `XmlElement::sequence_elements` liefert die Kinder `element` für `webdds-bridge`. Commit `5dcde9b5717`.

### §7.2.4.1.1 Beispiel (Non-normative)

**Spec:** §7.2.4.1.1, S. 19–20 — `QosPolicyCountSeq` mit Elementen
`element`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Es typisiert die Felder als
`xs:Integer`; XSD kennt `xs:integer`.

### §7.2.4.2 Octet-Folge als Liste mit Kommas in `value`

**Spec:** §7.2.4.2, S. 20 — "As a special case, sequences of octets are
represented either as a comma-separated list of the value of each octet
represented in decimal or hexadecimal, or alternatively using Base64 binary.
The two options are differentiated by the element name. The latter having
the suffix B64."

**Repo:** `crates/xml/src/qos_parser.rs::parse_octet_value`, `parse_octet_value_text`

**Tests:** `crates/xml/src/qos_parser.rs::tests::octet_value_is_base64_or_a_list`, `crates/xml/tests/qos_profile.rs::t13b_user_data_comma_list_to_bytes`

**Status:** done — `parse_octet_value_text` liest `<value>` als Liste, wenn der Text eine Liste von Oktetten (dezimal oder `0x…`, je 0 bis 255) ist; sonst als Base64, wie die XSD der OMG es für `value` typisiert (`xs:base64Binary`). Eine Ziffernfolge, die keine Liste ist, bleibt Base64. Commits `5dcde9b5717`, `7efd1017dd5`.

### §7.2.4.2 Octet-Folge als Base64 in `valueB64`

**Spec:** §7.2.4.2, S. 20 — "alternatively using Base64 binary [...] The
latter having the suffix B64."

**Repo:** `crates/xml/src/qos_parser.rs::parse_octet_value`, `decode_b64_text`

**Tests:** `crates/xml/tests/qos_profile.rs::t13_user_data_base64_to_bytes`, `crates/xml/tests/qos_profile.rs::t13c_user_data_both_forms_rejected`, `crates/xml/tests/spec_items.rs::topic_data_and_group_data_hold_octets_in_both_forms`

**Status:** done — `valueB64` wird gelesen; `value` und `valueB64` zugleich enden mit `XmlError::InvalidXml`. Commit `5dcde9b5717`.

### §7.2.4.2 Wahl über den Namen des Elements

**Spec:** §7.2.4.2, S. 20 — "The two options are differentiated by the
element name."

**Repo:** `crates/xml/src/qos_parser.rs::parse_octet_value`

**Tests:** `crates/xml/tests/qos_profile.rs::t13_user_data_base64_to_bytes`, `crates/xml/tests/qos_profile.rs::t13b_user_data_comma_list_to_bytes`

**Status:** done — `parse_octet_value` unterscheidet `value` und `valueB64` am Namen des Elements.

### §7.2.4.2.1 Beispiel (Non-normative)

**Spec:** §7.2.4.2.1, S. 20 — simpleType `ddsBinary`, complexType
`userDataQosPolicy` mit `xs:choice` aus `value` und `valueB64`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Der Name ist `"ddsBinary "` mit
Leerzeichen; das Muster `[0-2]?[0-9]?[0-9]` lässt 256 bis 299 zu (mit Python
`re` als Näherung der XSD-Regex geprüft, gemessen); die Beispieldokumente
schließen mit `<user_data_qos>` statt `</user_data_qos>` und sind nicht
wohlgeformt; das Element heißt `user_data_qos`, das Feld der IDL `user_data`.
Die beiden Alternativen stellen nicht denselben Wert dar: die Liste ergibt
„The quic“, `VGhlJTIwcXVpY2sl` ergibt „The%20quick%“ (mit Python dekodiert,
gemessen).

---

## §7.2.5 XML Representation of Arrays

### §7.2.5 Felder wie Folgen

**Spec:** §7.2.5, S. 20 — "The XML representation of IDL arrays is the same
as it would be for IDL sequences of the same element type."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — kein Typ der DDS-IDL, den DDS-XML abbildet
(QoS-Policies, QoS-Strukturen, `Duration_t`), enthält ein Feld fester Länge;
die Regel hat in den Building Blocks QoS bis Applications keinen
Anwendungsfall. Für Data Samples gilt §7.3.7.3.2.

---

## §7.2.6 XML Representation of Duration

### §7.2.6 Duration_t mit `sec` und `nanosec`

**Spec:** §7.2.6, S. 20–21 — "The IDL structure Duration_t is represented in
XML following the general rules for structures defined in sub clause 7.2.3,
except that the schema provides the option to use the symbolic defined in the
IDL to set the values of the intended elements." complexType `duration` mit
`sec` (`nonNegativeInteger_Duration_SEC`) und `nanosec`
(`nonNegativeInteger_Duration_NSEC`), beide `minOccurs="0"`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration_element`; `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd` (complexType `duration`)

**Tests:** `crates/xml/tests/qos_profile.rs::t01_happy_path_reliability_and_history_roundtrip`,
`crates/xml/tests/qos_profile.rs::t09_all_22_policies_parse`

**Status:** done — die XSD der OMG gibt das complexType von S. 21 wieder (`xs:all`, beide `minOccurs="0"`) und liegt unverändert im Baum. Die Namen der Konstanten stehen in den Items zu §7.2.2. `<period>DURATION_INFINITY</period>` ohne Kinder ist eine Herstellerform. Commits `8bf46985f11`, `e34118cc9a6`.

### §7.2.6 `nanosec` optional

**Spec:** §7.2.6, S. 21 — `<xs:element name="nanosec" [...] minOccurs="0"
/>`

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration_element`

**Tests:** `crates/xml/tests/qos_profile.rs::t10_duration_infinity_string_parses_to_infinite`,
`crates/xml/src/qos_parser.rs::tests::duration_inline_infinity_sentinel`

**Status:** done — ohne `nanosec` gilt 0.

### §7.2.6 `sec` optional

**Spec:** §7.2.6, S. 21 — `<xs:element name="sec" [...] minOccurs="0" />`

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration_element`

**Tests:** `crates/xml/tests/qos_profile.rs::t19b_duration_without_sec_is_zero`

**Status:** done — ohne `sec` gilt 0 (die Vorgabe der DDS für `Duration_t.sec`). Commit `4e479ef9e18`.

### §7.2.6 Unendliche Dauer

**Spec:** §7.2.6, S. 20; §7.2.2.1, S. 18 — `DURATION_INFINITE_SEC` und
`DURATION_INFINITE_NSEC` für die Felder von `Duration_t`.

**Repo:** `crates/xml/src/qos_parser.rs::parse_duration_element`

**Tests:** `crates/xml/tests/qos_profile.rs::t10_duration_infinity_string_parses_to_infinite`, `crates/xml/tests/qos_profile.rs::t10b_duration_infinity_numeric_parses_to_infinite`

**Status:** done — `sec` = 2147483647 mit `nanosec` = `DURATION_INFINITE_NSEC` oder ohne `nanosec` ergibt die unendliche Dauer von `zerodds-qos`. `<sec>DURATION_INFINITE_SEC</sec><nanosec>0</nanosec>` ergibt 2147483647 s.

### §7.2.6 `nanosec` = 2147483647 als Zahl

**Spec:** §7.2.2.1, S. 18 — Muster `(DURATION_INFINITY|DURATION_INFINITE_NSEC|([0-9])*)?`
für `nanosec`.

**Repo:** `crates/xml/src/types.rs::parse_duration_nsec`; `crates/xml/src/qos_parser.rs::parse_duration_element`

**Tests:** `crates/xml/src/types.rs::tests::duration_nsec_accepts_the_full_unsigned_long_range`, `crates/xml/tests/qos_profile.rs::t10b_duration_infinity_numeric_parses_to_infinite`

**Status:** done — `parse_duration_nsec` nimmt jede Zahl des Bereichs von `unsigned long`, auch 2147483647; `<sec>2147483647</sec><nanosec>2147483647</nanosec>` ist die unendliche Dauer. Commit `4e479ef9e18`.

### §7.2.6.1 Beispiel (Non-normative)

**Spec:** §7.2.6.1, S. 21 — `<sec>DURATION_INFINITY</sec>`,
`<nanosec>DURATION_INFINITY_NSEC</nanosec>`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. `DURATION_INFINITY_NSEC` passt
weder zum Muster von S. 18 noch zu einer Konstante der IDL;
`parse_duration_nsec` weist es ab.

---

## §7.3.1 Overview

### §7.3.1 Sechs Building Blocks

**Spec:** §7.3.1, S. 21 — "This specification breaks the syntax used to
represent DDS resources in XML into the six different building blocks as
shown in Figure 7.1: Building Block QoS, Building Block Types, Building Block
Domains, Building Block DomainParticipants, Building Block Applications,
Building Block Data Samples".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung; die Building Blocks stehen in
den Items zu §7.3.2 bis §7.3.7.

### §7.3.1 Figure 7.1 Beziehungen der Building Blocks

**Spec:** §7.3.1, Figure 7.1, S. 22 — Paketdiagramm mit BBQos, BBTypes,
BBDomains, BBDataSamples, BBDomainParticipants, BBApplications.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Abbildung; die Abhängigkeiten stehen in den
Items zu §7.3.2.2 bis §7.3.7.2.

### §7.3.1 Zwei Schema-Dateien nach dem Chameleon-Muster

**Spec:** §7.3.1, S. 22 — "Each of these building blocks is associated with
two normative schema files in XSD format that are designed according to the
XML Chameleon Schema Definition pattern (see sub clause 7.1.3)."

**Repo:** `crates/xml/src/schemas.rs::ALL_SCHEMAS`

**Tests:** `crates/xml/src/schemas.rs::tests::all_schemas_includes_six_building_blocks_plus_system`,
`crates/xml/src/schemas.rs::tests::nonamespace_xsds_omit_target_namespace`

**Status:** done — für Data Samples sagt §7.3.7.3.1, dass es kein allgemeines XSD geben kann, und §7.3.7.3 nennt keine Datei; ZeroDDS liefert für diesen Building Block ein eigenes Paar nach demselben Muster.

### §7.3.1 `_nonamespace.xsd`: Typen ohne targetNamespace und ohne Wurzelelement

**Spec:** §7.3.1, S. 22 — "dds-xml_<building_block_name>_definitions_nonamespace.xsd
contains the type declarations for all the constructs the building block
defines. This XSD file specifies neither a targetNamespace nor a root
element."

**Repo:** `crates/xml/schemas/` (sieben Dateien `*_nonamespace.xsd`)

**Tests:** `crates/xml/src/schemas.rs::tests::nonamespace_xsds_omit_target_namespace`, `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`

**Status:** done — keine der sieben Chameleon-Dateien hat einen `targetNamespace` oder ein globales Element; das Paar für Data Samples trägt die Wurzelelemente jetzt in der Datei mit Namensraum. Commits `8bf46985f11`, `a45564ecb6b`.

### §7.3.1 Wurzelelemente in der Chameleon-Datei für Data Samples

**Spec:** §7.3.1, S. 22 — "This XSD file specifies neither a targetNamespace
nor a root element."

**Repo:** `crates/xml/schemas/dds-xml_data_samples_definitions.xsd`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`, `crates/xml/tests/schema_validation.rs::serialize_sample_output_validates`

**Status:** done — `dds-xml_data_samples_definitions_nonamespace.xsd` deklariert kein Wurzelelement; `<data>` und `<sample>` stehen in `dds-xml_data_samples_definitions.xsd`, und `serialize_sample_output_validates` prüft die Ausgabe von `serialize_sample` im Namensraum gegen diese Datei. Commit `a45564ecb6b`.

### §7.3.1 `_definitions.xsd`: bindet ein, Wurzelelement, targetNamespace

**Spec:** §7.3.1, S. 22 — "dds-xml_<building_block_name>_definitions.xsd
includes the XSD with no targetNamespace, defines the top level element for
the building block, and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/` (sieben Dateien `*_definitions.xsd`)

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_include_target_namespace`,
`crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`

**Status:** done — jede Datei bindet ihre Chameleon-Datei mit `xs:include` ein, deklariert das Wurzelelement und setzt den `targetNamespace`; auch die für Data Samples.

### §7.3.1 Dokumente im Namensraum http://www.omg.org/spec/DDS-XML

**Spec:** §7.3.1, S. 22 — "sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/src/parser.rs::DDS_XML_NS`, `library_elements`; `crates/xml/src/xsd_loader.rs::load_type_libraries_from_string`

**Tests:** `crates/xml/src/parser.rs::tests::parse_namespace_aware`, `crates/xml/src/xsd_loader.rs::tests::strict_mode_ignores_the_namespace_string_outside_a_declaration`, `crates/xml/tests/dds_xml_full.rs::all_four_libraries_roundtrip`

**Status:** done — die Leser vergleichen lokale Namen: ein Dokument im Namensraum der Spec, eines ohne Namensraum (Chameleon-Muster, §7.1.3) und eines im eigenen Namensraum des Anwenders werden gleich gelesen. `ValidationMode::Strict` in `load_type_libraries_from_string` verlangt die Deklaration des Namensraums am Wurzelelement; sie steht nicht irgendwo im Text. Commit `80ea5f7343e`.

---

## §7.3.2 Building Block QoS

### §7.3.2.1 Zweck

**Spec:** §7.3.2.1, S. 22 — "This building block defines the syntax to
represent DDS QoS in XML."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweck; die Syntax steht in den Items zu
§7.2 und §7.3.2.3 bis §7.3.2.4.4.

### §7.3.2.2 Keine Abhängigkeit von anderen Building Blocks

**Spec:** §7.3.2.2, S. 22 — "This building block has no dependencies on other
building blocks."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd` (bindet keine andere Datei ein); `crates/xml/src/qos_parser.rs::parse_qos_libraries`

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`,
`crates/xml/tests/dds_xml_full.rs::independent_building_blocks_parse_without_the_other_blocks`

**Status:** done

### §7.3.2.3 Typen aller QoS in `dds-xml_qos_definitions_nonamespace.xsd`

**Spec:** §7.3.2.3, S. 23 — "dds-xml_qos_definitions_nonamespace.xsd contains
the XSD type definition for all the DDS QoS. It defines no targetNamespace so
that it can be reused by other schemas following the XML Chameleon Schema
Definition pattern."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions_nonamespace.xsd`

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/tests/schema_validation.rs::xsd_valid_documents_are_read`

**Status:** done — die Datei der OMG liegt unverändert im Baum und definiert alle Policies, Felder, Aufzählungen und Container. Commit `8bf46985f11`.

### §7.3.2.3 `<qos_library>` als oberstes Element im Namensraum

**Spec:** §7.3.2.3, S. 23 — "dds-xml_qos_definitions.xsd defines the
<qos_library> top-level element, and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_qos_definitions.xsd`;
`crates/xml/src/parser.rs::library_elements`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`,
`crates/xml/tests/schema_validation.rs::topic_filter_attribute_is_read`,
`crates/xml/tests/omg_example_documents.rs::qos_example`

**Status:** done — die Leser nehmen `<qos_library>` als Wurzel und als Kind
von `<dds>`. `topic_filter_attribute_is_read` braucht kein `xmllint`;
`qos_example` braucht die Dateien der OMG.

### §7.3.2.4.1 QoS-Bibliotheken mit QoS-Profilen

**Spec:** §7.3.2.4.1, S. 23 — "QoS Libraries are the top level element of the
Building Block QoS. They are collections of QoS profiles, which group a set
of related QoS—usually one per entity."

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_libraries`, `parse_qos_library_element`, `parse_qos_profile_element`

**Tests:** `crates/xml/src/qos_parser.rs::tests::parse_minimal_library`,
`crates/xml/src/qos_parser.rs::tests::missing_library_name_rejected`,
`crates/xml/src/qos_parser.rs::tests::missing_profile_name_rejected`,
`crates/xml/tests/qos_profile.rs::t17_multiple_libraries_per_document`

**Status:** done — `name` ist an `<qos_library>` und `<qos_profile>` Pflicht; ein unbekanntes Kind von `<qos_library>` oder `<qos_profile>` endet mit `XmlError::UnknownElement`.

### §7.3.2.4.1.1 Beispiel (Non-normative)

**Spec:** §7.3.2.4.1.1, S. 23 — Bibliothek `ReliableProfilesLibrary` mit
einem Profil für DataWriter und DataReader.

**Repo:** `crates/xml/src/qos_parser.rs::parse_qos_libraries`

**Tests:** `crates/xml/tests/spec_items.rs::qos_library_example`, `crates/xml/tests/omg_example_documents.rs::qos_example`

**Status:** done — `qos_library_example` liest das Beispiel; `omg_example_documents::qos_example` liest das Beispieldokument der OMG, wenn `ZERODDS_OMG_DDSXML_DIR` gesetzt ist.

### §7.3.2.4.2 Vererbung von QoS-Profilen über `base_name`

**Spec:** §7.3.2.4.2, S. 23 — "A QoS Profile can inherit from other QoS
Profiles using the base_name XML attribute."

**Repo:** `crates/xml/src/qos_inheritance.rs::resolve_profile`; `crates/xml/src/registry.rs::QosProfileRegistry`; `crates/xml/src/qos.rs::EntityQos::merge`; `crates/xml/src/inheritance.rs::resolve_chain`; `crates/rpc/src/qos_profile.rs::RpcQos::from_xml_profile`

**Tests:** `crates/xml/tests/qos_profile.rs::t03_inheritance_child_overrides_history_keeps_reliability`,
`crates/xml/tests/qos_profile.rs::t04_three_level_inheritance_propagates`,
`crates/xml/tests/qos_profile.rs::t05_inheritance_cycle_rejected`,
`crates/xml/src/registry.rs::tests::inheritance_derived_overrides_base`,
`crates/xml/src/qos_inheritance.rs::tests::cross_library_base_name_two_segment`

**Status:** done — die Policy des abgeleiteten Profils ersetzt die des Basisprofils ganz, nicht Feld für Feld: ein Kind mit `<history><kind>KEEP_ALL_HISTORY_QOS</kind></history>` hat die Tiefe 1, nicht die der Basis; die Spec sagt dazu nichts. Zyklen und Ketten über 32 Glieder (`MAX_INHERITANCE_DEPTH`) werden abgewiesen. `RpcQos::from_xml_profile` folgt der Kette `base_name`. Commits `f1a7a3ea573`, `563ab2e3a64`.

### §7.3.2.4.2 Nur von vorher definierten Profilen

**Spec:** §7.3.2.4.2, S. 23 — "A QoS profile can only inherit from QoS
profiles that have been defined its definition".

**Repo:** `crates/xml/src/qos_inheritance.rs::check_definition_order`; `crates/xml/src/registry.rs::QosProfileRegistry::resolve`

**Tests:** `crates/xml/src/qos_inheritance.rs::tests::forward_base_name_rejected`, `crates/xml/src/registry.rs::tests::forward_base_name_rejected`

**Status:** done — ein Profil, dessen `base_name` auf ein später definiertes Profil zeigt, endet mit `XmlError::InvalidXml`; die Ordnung ist die des Dokuments, über Bibliotheken hinweg. Der Satz der Spec ist unvollständig („defined its definition“); gelesen wird „defined before its definition“. Commit `5f5d7aaa3e9`.

### §7.3.2.4.2.1 Beispiel (Non-normative)

**Spec:** §7.3.2.4.2.1, S. 23 — `<qos_profile name="MyProfile"
base_name="BaseProfile">`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel.

### §7.3.2.4.3 Mehrere QoS einer Art, Auswahl nach dem Namen des Topic

**Spec:** §7.3.2.4.3, S. 23 — "A QoS Profile may contain several DataWriter,
DataReader, and Topic QoS settings that are selected based on the evaluation
of a filter expression on the topic name."

**Repo:** `crates/xml/src/qos_parser.rs::take_qos_container`; `crates/xml/src/qos.rs::select_qos_for_topic`; `crates/xml/src/qos_inheritance.rs::ResolvedQos`; `crates/xml/src/registry.rs::QosProfileRegistry::writer_qos_for_topic`

**Tests:** `crates/xml/tests/qos_profile.rs::t21_repeated_containers_select_by_topic_filter`, `crates/xml/src/qos_inheritance.rs::tests::topic_filter_selects_between_repeated_containers`, `crates/xml/src/registry.rs::tests::writer_qos_for_topic_selects_by_filter`, `crates/xml/tests/normative_forms.rs::repeated_containers_of_every_kind`

**Status:** done — ein Profil hält je Art mehrere Container (`Vec<FilteredEntityQos>`), jeder mit seinem `topic_filter`; `select_qos_for_topic`, `ResolvedQos::datawriter_qos_for_topic` und `QosProfileRegistry::writer_qos_for_topic` wählen nach dem Namen des Topic. Mehrere Container mit demselben Filter (oder mehrere ohne) werden Policy für Policy zusammengeführt (`merge_filtered_slots`). Commit `f5f781d033b`.

### §7.3.2.4.3 Filter im Attribut `topic_filter` der QoS

**Spec:** §7.3.2.4.3, S. 23 — "The filter expression is specified via the
topic_filter XML attribute in the QoS definition of the entity QoS."

**Repo:** `crates/xml/src/qos_parser.rs::take_qos_container`, `parse_single_qos_shortcut`; `crates/xml/src/qos.rs::FilteredEntityQos`, `topic_filter_matches`

**Tests:** `crates/xml/tests/schema_validation.rs::topic_filter_attribute_is_read`, `crates/xml/tests/qos_profile.rs::t08_topic_filter_glob_semantics`, `crates/xml/src/qos_inheritance.rs::tests::topic_filter_inherited_and_overridden`

**Status:** done — das Attribut wird am Container gelesen und dort gespeichert (`FilteredEntityQos::topic_filter`); `topic_filter_matches` wertet `*` und `?` aus (die PDF legt die Syntax nicht fest). Der Filter als Kindelement von `<qos_profile>` und das Attribut an `<topic>` sind Herstellerformen (`VendorForms`). Commits `f5f781d033b`, `e34118cc9a6`.

### §7.3.2.4.3 Ohne Filter gilt `*`

**Spec:** §7.3.2.4.3, S. 23 — "If the topic filter is unspecified, the filter
"*" will be assumed."

**Repo:** `crates/xml/src/qos.rs::default_qos`, `select_qos_for_topic`

**Tests:** `crates/xml/tests/qos_profile.rs::t22_filterless_container_is_the_fallback`

**Status:** done — ein Container ohne Filter ist der Ersatz für jedes Topic, auf das kein Filter passt (`default_qos`).

### §7.3.2.4.3 Reihenfolge und Vorrang der Filter

**Spec:** §7.3.2.4.3, S. 23 — "The QoS with an explicit topic_filter
attribute definition will be evaluated in order; they take precedence over a
QoS without a topic filter expression."

**Repo:** `crates/xml/src/qos.rs::select_qos_for_topic`

**Tests:** `crates/xml/tests/qos_profile.rs::t21_repeated_containers_select_by_topic_filter`, `crates/xml/src/qos_inheritance.rs::tests::no_matching_filter_has_no_qos`, `crates/xml/src/registry.rs::tests::writer_qos_for_topic_without_match_errors`

**Status:** done — ein Container mit passendem Filter hat Vorrang vor dem ohne Filter; mehrere passende Filter wählt die Reihenfolge im Dokument; ohne Treffer hat die Entität keine QoS aus dem Profil. Commit `f5f781d033b`.

### §7.3.2.4.3.1 Beispiel (Non-normative)

**Spec:** §7.3.2.4.3.1, S. 24 — zwei `<datawriter_qos>` mit `topic_filter="A*"`
und `"B*"`, der zweite mit `resource_limits`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Der Leser liest es: zwei `<datawriter_qos>` mit verschiedenen `topic_filter` in einem Profil, `initial_samples` und `initial_instances` in `resource_limits` (`resource_limits_initial_fields_are_carried`, `t21_repeated_containers_select_by_topic_filter`).

### §7.3.2.4.4 Einzelne QoS als Profil

**Spec:** §7.3.2.4.4, S. 24 — "The definition of an individual QoS is a
shortcut for defining a QoS profile with a single QoS."

**Repo:** `crates/xml/src/qos_parser.rs::parse_single_qos_shortcut`, `parse_qos_library_element`

**Tests:** `crates/xml/src/qos_parser.rs::tests::single_qos_shortcut_datawriter_creates_implicit_profile`, `crates/xml/src/qos_parser.rs::tests::single_qos_shortcut_topic_creates_implicit_profile`, `crates/xml/src/qos_parser.rs::tests::single_qos_shortcut_multiple_kinds_in_same_library`, `crates/xml/src/qos_parser.rs::tests::single_qos_shortcut_without_name_is_kept`

**Status:** done — ein Container direkt unter `<qos_library>` ist ein Profil mit einer QoS; ohne `name` ist es ein Profil, auf das nichts verweisen kann, und bleibt unter dem leeren Namen erhalten. Commit `bf34c97b1eb`.

### §7.3.2.4.4.1 Beispiel (Non-normative)

**Spec:** §7.3.2.4.4.1, S. 24 — `<datawriter_qos name="KeepAllWriter">`
gleich `<qos_profile name="KeepAllWriter"><datawriter_qos>`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel.

---

## §7.3.3 Building Block Types

### §7.3.3.1 Typen in XML

**Spec:** §7.3.3.1, S. 24 — "This building block gathers the syntax used to
represent DDS Types in XML."

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_types_element_ctx`, `parse_type_libraries_with`; `crates/xml/src/xtypes_expr.rs`; `crates/xml-compose/src/lib.rs::compose`

**Tests:** `crates/xml/tests/types_and_samples.rs::t01_struct_with_long_member`, `crates/xml/tests/types_and_samples.rs::t16_module_three_levels_deep`, `crates/xml/tests/normative_forms.rs::bounds_and_dimensions_are_evaluated`, `crates/xml-compose/tests/idl_parity.rs::every_primitive_reaches_the_same_type_from_both_front_ends`

**Status:** done — zwei Leser: `zerodds-xml`, den `parse_dds_xml` benutzt, und `zerodds-xml-compose` für die Codegeneratoren. Der Leser von `zerodds-xml` folgt der Grammatik von XTypes 1.3 Annex A (`type`/`nonBasicTypeName`, Maps, Annotationen, `<const>`, Konstantenausdrücke als Grenzen); die Grammatik steht Item für Item in `dds-xtypes-1.3.md` unter §7.3.2. Commits `32aa062decf`, `3d330808107`, `af517d979de`.

### §7.3.3.1 `nonBasicTypeName` und Maps im Leser von `zerodds-xml`

**Spec:** §7.3.3.1, S. 24 — Syntax der Typen in XML.

**Repo:** `crates/xml/src/xtypes_parser.rs::parse_type_attribute`, `parse_map_attributes`

**Tests:** `crates/xml/src/xtypes_parser.rs::tests::spec_nonbasic_type_is_named_by_nonbasic_type_name`, `crates/xml/src/xtypes_parser.rs::tests::spec_map_members_are_read`, `crates/xml/src/xtypes_parser.rs::tests::spec_map_typedef_is_read`

**Status:** done — `type="nonBasic"` benennt den Typ über `nonBasicTypeName`; Maps lesen `mapMaxLength`, `mapKeyType`, `mapKeyNonBasicTypeName` und `mapKeyStringMaxLength`. Ein Typname in `type` ohne `nonBasic` ist eine Herstellerform. Commits `4e479ef9e18`, `af517d979de`, `e34118cc9a6`.

### §7.3.3.1 Organisation und Verwaltung von Typen

**Spec:** §7.3.3.1, S. 24 — "Additionally, it provides capabilities that are
necessary or convenient for the organization and management of types and
other XML resource representations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Ankündigung ohne Inhalt in der PDF; die
Konstrukte (`module`, `include`, `forward_dcl`, `const`) gehören zur
Grammatik von [DDS-XTYPES] (`dds-xtypes-1.3.md` unter §7.3.2.1).

### §7.3.3.2 Keine Abhängigkeit von anderen Building Blocks

**Spec:** §7.3.3.2, S. 24 — "This building block has no dependencies on other
building blocks."

**Repo:** `crates/xml/schemas/dds-xml_type_definitions_nonamespace.xsd` (bindet keine andere Datei ein); `crates/xml/src/xtypes_parser.rs::parse_type_libraries`

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`,
`crates/xml/tests/dds_xml_full.rs::independent_building_blocks_parse_without_the_other_blocks`

**Status:** done

### §7.3.3.3 Typen aller DDS-Typen in der Chameleon-Datei

**Spec:** §7.3.3.3, S. 25 — "dds-xml_type_definitions_nonamespace.xsd
contains the XSD type definitions for all the DDS types. It defines no
targetNamespace so that it can be reused by other schemas following the XML
Chameleon Schema Definition pattern."

**Repo:** `crates/xml/schemas/dds-xml_type_definitions_nonamespace.xsd`

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/tests/schema_validation.rs::xsd_valid_documents_are_read`

**Status:** done — die Datei der OMG liegt unverändert im Baum und definiert die Typen aller Konstrukte (`structDecl`, `unionDecl`, `enumDecl`, …). Commit `32aa062decf`, `8bf46985f11`.

### §7.3.3.3 `<types>` als oberstes Element im Namensraum

**Spec:** §7.3.3.3, S. 25 — "dds-xml_type_definitions.xsd defines the <types>
top-level element, and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_type_definitions.xsd`; `crates/xml/src/xtypes_parser.rs::parse_type_libraries`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`,
`crates/xml/tests/types_and_samples.rs::t26_sample_must_understand_attribute_parses`,
`crates/xml/src/xtypes_parser.rs::tests::dds_root_with_multiple_types_blocks`

**Status:** done — `<types>` als Wurzel und als Kind von `<dds>`.

---

## §7.3.4 Building Block Domains

### §7.3.4.1 Zweck

**Spec:** §7.3.4.1, S. 25 — "This building block defines the syntax used to
represent DDS Domains in XML. Domains provide a data space where information
can be shared by reading and writing a set of Topics, which are associated to
registered data types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweck; die Syntax in den Items zu §7.3.4.3
bis §7.3.4.4.2.

### §7.3.4.2 Abhängig von QoS und Types

**Spec:** §7.3.4.2, S. 25 — "This building block depends on the Building
Block QoS and the Building Block Types."

**Repo:** `crates/xml/schemas/dds-xml_domain_definitions_nonamespace.xsd` (bindet die Chameleon-Dateien von Types und QoS ein)

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`

**Status:** done — im Leser nutzt `<topic_qos>` den Leser der QoS; `type_ref` wird beim Auflösen gegen die `<types>` des Dokuments geprüft (Item zu §7.3.4.4.1). Die Chameleon-Datei der OMG bindet Types und QoS ein.

### §7.3.4.3 Typen für Domains in der Chameleon-Datei

**Spec:** §7.3.4.3, S. 25 — "dds-xml_domain_definitions_nonamespace.xsd
contains the XSD type definition for domains and their contained entities."

**Repo:** `crates/xml/schemas/dds-xml_domain_definitions_nonamespace.xsd` (complexTypes `registerType`, `topicEntry`, `domainEntry`, `domainLibrary`)

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/tests/schema_validation.rs::schema_invalid_documents`, `crates/xml/tests/normative_forms.rs::registered_name_and_format_are_carried`

**Status:** done — die Datei der OMG liegt unverändert im Baum. Der Leser liest an `<register_type>` das Element `registered_name` und das Attribut `format` und an `<topic>` das Element `registered_name`. `qos_profile_ref` und `topic_filter` an `<topic>` stehen nicht in der Datei; sie sind Herstellerformen. Commits `8bf46985f11`, `e34118cc9a6`. Beide Tests brauchen `xmllint`.

### §7.3.4.3 `<domain_library>` als Wurzelelement im Namensraum

**Spec:** §7.3.4.3, S. 25 — "dds-xml_domain_definitions.xsd defines the
<domain_library> root element and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_domain_definitions.xsd`; `crates/xml/src/domain.rs::parse_domain_libraries`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`, `crates/xml/tests/spec_items.rs::domain_example`, `crates/xml/tests/omg_example_documents.rs::domains_example`

**Status:** done — `domain_example` und `domain_with_two_topics_and_register_types` lesen `<domain_library>` als Wurzel; `domains_example` liest das Dokument der OMG, wenn `ZERODDS_OMG_DDSXML_DIR` gesetzt ist.

### §7.3.4.4.1 Domain mit Topics und registrierten Typen

**Spec:** §7.3.4.4.1, S. 25 — "A Domain includes a set of Topics and
Registered Types that can be read and written in the Domain."

**Repo:** `crates/xml/src/domain.rs::parse_domain_element`

**Tests:** `crates/xml/tests/dds_xml_full.rs::domain_with_two_topics_and_register_types`, `crates/xml/src/domain.rs::tests::parse_domain_with_topic`, `crates/xml/tests/dds_xml_full.rs::misplaced_or_unknown_children_are_rejected`

**Status:** done — ein anderes Kind von `<domain>` oder `<topic>` endet mit `XmlError::UnknownElement`; mit `report_and_skip` wird es ausgelassen und gemeldet. Commit `8771c676858`.

### §7.3.4.4.1 `register_type` mit `type_ref`

**Spec:** §7.3.4.4.1, S. 25 — "Register types shall provide a reference to
data types that have been previously defined using the type_ref XML
attribute."

**Repo:** `crates/xml/src/domain.rs::parse_register_type`; `crates/xml/src/zerodds_xml.rs::DdsXml::resolve_participant`

**Tests:** `crates/xml/src/domain.rs::tests::parse_domain_with_topic`, `crates/xml/tests/dds_xml_full.rs::deployment_file_with_inheritance_and_cross_references`

**Status:** done — `type_ref` wird gelesen, gegen die `<types>` des Dokuments aufgelöst und als Typname des Topic weitergegeben. Commits `dd4ebb955e6`, `43146783064`.

### §7.3.4.4.1 `type_ref` ist Pflicht

**Spec:** §7.3.4.4.1, S. 25 — "Register types shall provide a reference [...]
using the type_ref XML attribute."

**Repo:** `crates/xml/src/domain.rs::parse_register_type`

**Tests:** `crates/xml/src/domain.rs::tests::register_type_missing_type_ref_rejected`, `crates/xml/src/participant.rs::tests::participant_register_type_without_type_ref_rejected`

**Status:** done — `parse_register_type` weist ein `register_type` ohne `type_ref` mit `XmlError::MissingRequiredElement` ab: der Text verlangt die Referenz („shall provide“), das XSD der OMG hat `use="optional"`. Entscheidung für den Text; das XSD-gültige Dokument ohne `type_ref` wird nicht gelesen. Commit `dd4ebb955e6`.

### §7.3.4.4.1 Der Typ ist vorher definiert

**Spec:** §7.3.4.4.1, S. 25 — "data types that have been previously defined".

**Repo:** `crates/xml/src/zerodds_xml.rs::DdsXml::resolve_participant`, `resolve_topic_entry`, `resolve_type`; `crates/xml/src/domain.rs::DomainLibrary`

**Tests:** `crates/xml/tests/dds_xml_full.rs::register_type_with_dangling_type_ref_errors`, `crates/xml/tests/spec_items.rs::register_type_refers_to_a_type_defined_before_the_domain`

**Status:** done — `resolve_participant` löst `type_ref` über `resolve_type` auf; ein Typ, den kein `<types>` definiert, endet mit `XmlError::UnresolvedReference`, ebenso ein Typ, den erst ein `<types>` nach der Domain-Bibliothek definiert (`DomainLibrary::type_libraries_before`). Commits `dd4ebb955e6`, `43146783064`.

### §7.3.4.4.1 Name der Registrierung kann vom Typnamen abweichen

**Spec:** §7.3.4.4.1, S. 25 — "The name under which types are registered may
be different than original type name."

**Repo:** `crates/xml/src/domain.rs::RegisterType` (Felder `name`,
`type_ref`)

**Tests:** `crates/xml/src/domain.rs::tests::parse_domain_with_topic`,
`crates/xml/tests/dds_xml_full.rs::deployment_file_with_inheritance_and_cross_references`

**Status:** done

### §7.3.4.4.1 Topic verweist mit `register_type_ref`

**Spec:** §7.3.4.4.1, S. 25 — "Topics shall refer to a registered type using
the register_type_ref XML attribute."

**Repo:** `crates/xml/src/domain.rs::parse_topic_entry`;
`crates/xml/src/zerodds_xml.rs::resolve_topic_entry`

**Tests:** `crates/xml/src/domain.rs::tests::topic_missing_register_type_ref_rejected`,
`crates/xml/tests/dds_xml_full.rs::topic_with_unknown_register_type_ref_errors`

**Status:** done

### §7.3.4.4.1 QoS eines Topic inline

**Spec:** §7.3.4.4.1, S. 25 — "Topics may also specify QoS settings inline
following the syntax defined in the Building Block QoS."

**Repo:** `crates/xml/src/domain.rs::parse_topic_entry`

**Tests:** `crates/xml/src/domain.rs::tests::parse_topic_with_inline_qos`,
`crates/xml/tests/dds_xml_full.rs::topic_qos_inline_and_profile_ref_both_resolve`

**Status:** done

### §7.3.4.4.1 `base_name` an `topic_qos`

**Spec:** §7.3.4.4.1, S. 25 — "QoS profile inheritance through the base_name
attribute may be used as defined in clause 7.3.2.4.2."

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_qos`; `crates/xml/src/zerodds_xml.rs::DdsXml::resolve_inline_qos`; `crates/xml/src/registry.rs::inline_over_base`

**Tests:** `crates/xml/tests/dds_xml_full.rs::inline_topic_qos_base_name_inherits_from_profile`, `crates/xml/src/registry.rs::tests::inline_container_inherits_base_name_profile`, `crates/xml/tests/spec_items.rs::domain_example`

**Status:** done — `parse_entity_qos` liest `base_name` an der QoS (`EntityQos::base_name`); beim Auflösen ist das Profil die Basis und die inline angegebenen Policies überschreiben sie. Commits `5dcde9b5717`, `b956dbaceed`.

### §7.3.4.4.1.1 Beispiel (Non-normative)

**Spec:** §7.3.4.4.1.1, S. 25 — Domain `MyDomain` mit `domain_id="10"`, zwei
`register_type` und zwei `topic`.

**Repo:** `crates/xml/src/domain.rs::parse_domain_libraries`

**Tests:** `crates/xml/tests/spec_items.rs::domain_example`

**Status:** done — `domain_example` liest das Beispiel; das `base_name` von `topic_qos` bleibt erhalten.

### §7.3.4.4.2 Vererbung von Domains über `base_name`

**Spec:** §7.3.4.4.2, S. 26 — "A Domain can inherit from other Domains using
the base_name XML attribute."

**Repo:** `crates/xml/src/domain.rs::DomainLibrary::effective_domain`, `effective_domain_id`

**Tests:** `crates/xml/src/domain.rs::tests::domain_id_inherited_through_base_name`, `crates/xml/src/domain.rs::tests::domain_inheritance_chain_resolves_via_resolve_chain`, `crates/xml/tests/dds_xml_full.rs::domain_inheritance_carries_topics_and_types`

**Status:** done — `domain_id`, die registrierten Typen und die Topics werden über die Kette geerbt; ein gleichnamiger Eintrag der abgeleiteten Domain ersetzt den der Basis. Commits `8771c676858`, `5f5d7aaa3e9`.

### §7.3.4.4.2 Topics und Typen der Basis

**Spec:** §7.3.4.4.2, S. 26 — Vererbung von Domains.

**Repo:** `crates/xml/src/domain.rs::DomainLibrary::effective_domain`; `crates/xml/src/zerodds_xml.rs::resolve_participant_chain`

**Tests:** `crates/xml/tests/dds_xml_full.rs::domain_inheritance_carries_topics_and_types`

**Status:** done — `effective_domain` führt `register_types` und `topics` der Kette zusammen; `resolve_participant` benutzt die wirksame Domain, ein Writer an einem Topic der Basis wird aufgelöst. Commit `8771c676858`.

### §7.3.4.4.2 Nur von vorher definierten Domains

**Spec:** §7.3.4.4.2, S. 26 — "A Domain can only inherit from domains that
have been defined before."

**Repo:** `crates/xml/src/domain.rs::DomainLibrary::effective_domain_id`

**Tests:** `crates/xml/src/domain.rs::tests::domain_forward_base_name_rejected`

**Status:** done — `effective_domain_id` und `effective_domain` weisen eine Basis ab, die später in der Bibliothek steht, mit `XmlError::InvalidXml`. Commit `5f5d7aaa3e9`.

### §7.3.4.4.2.1 Beispiel (Non-normative)

**Spec:** §7.3.4.4.2.1, S. 26 — `<domain name="MyDomain"
base_name="BaseDomain">`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel.

---

## §7.3.5 Building Block DomainParticipants

### §7.3.5.1 Zweck

**Spec:** §7.3.5.1, S. 26 — "This block defines the syntax to represent DDS
DomainParticipants and their contained entities (i.e., Publishers,
Subscribers, DataWriters, and DataReaders) in XML."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweck; die Syntax in den Items zu §7.3.5.3
bis §7.3.5.4.4.

### §7.3.5.2 Abhängig von QoS, Types und Domains

**Spec:** §7.3.5.2, S. 26 — "This building block depends on the Building
Block QoS, the Building Block Types, and the Building Block Domains."

**Repo:** `crates/xml/schemas/dds-xml_domainparticipant_definitions_nonamespace.xsd` (bindet die Chameleon-Datei von Domains ein, die QoS und Types einbindet)

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`

**Status:** done

### §7.3.5.3 Typen der Entitäten in der Chameleon-Datei

**Spec:** §7.3.5.3, S. 26 — "dds-xml_domainparticipant_defintions_nonamespace.xsd
contains the XSD type definition for all the DDS entities."

**Repo:** `crates/xml/schemas/dds-xml_domainparticipant_definitions_nonamespace.xsd` (complexTypes `dataWriterEntry`, `dataReaderEntry`, `publisherEntry`, `subscriberEntry`, `domainParticipantEntry`, `domainParticipantLibrary`)

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`, `crates/xml/tests/schema_validation.rs::schema_invalid_documents`, `crates/xml/tests/normative_forms.rs::data_reader_filters_and_conditions_are_read`

**Status:** done — die Datei der OMG liegt unverändert im Baum. Die Elemente `content_filter`, `status_condition`, `read_condition` und `query_condition` an `<data_reader>` werden gelesen (`DataReaderEntry`). Commits `8bf46985f11`, `efd043b8ec4`. Beide Tests brauchen `xmllint`.

### §7.3.5.3 `<domain_participant_library>` als Wurzelelement im Namensraum

**Spec:** §7.3.5.3, S. 26 — "dds-xml_domainparticipant_definitions.xsd defines
the <domain_participant_library> root element and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_domainparticipant_definitions.xsd`; `crates/xml/src/participant.rs::parse_domain_participant_libraries`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`, `crates/xml/tests/normative_forms.rs::the_participant_example_of_the_specification`, `crates/xml/tests/omg_example_documents.rs::domain_participants_example`

**Status:** done — `participant.rs` liest `<domain_participant_library>` als Wurzel; `domain_participants_example` liest das Dokument der OMG, wenn `ZERODDS_OMG_DDSXML_DIR` gesetzt ist, `the_participant_example_of_the_specification` das der Spec.

### §7.3.5.4.1 Bibliotheken von DomainParticipants

**Spec:** §7.3.5.4.1, S. 26 — "Domain Participant Libraries are collections
of DomainParticipants and contained entities. They are the top level elements
of the Building Block DomainParticipants."

**Repo:** `crates/xml/src/participant.rs::parse_dp_library_element`

**Tests:** `crates/xml/src/participant.rs::tests::parse_minimal_participant`,
`crates/xml/src/participant.rs::tests::missing_participant_name_rejected`

**Status:** done

### §7.3.5.4.1 Entitäten unter ihrem Elternelement

**Spec:** §7.3.5.4.1, S. 26 — "To represent this hierarchical relationship
between DDS entities, each entity is declared as a nested XML tag under the
declaration of its parent entity."

**Repo:** `crates/xml/src/participant.rs::parse_dp_element`, `parse_publisher_element`, `parse_subscriber_element`, `parse_dw_element`, `parse_dr_element`

**Tests:** `crates/xml/src/participant.rs::tests::parse_pub_with_writer`, `crates/xml/tests/dds_xml_full.rs::resolve_participant_returns_flat_snapshot`

**Status:** done — DataWriter unter Publisher, DataReader unter Subscriber, beide unter dem DomainParticipant werden gelesen; eine Entität unter einem anderen Element endet mit `XmlError::UnknownElement` (Item unten). Commit `8771c676858`.

### §7.3.5.4.1 Entität unter einem falschen Element

**Spec:** §7.3.5.4.1, S. 26 — "each entity is declared as a nested XML tag
under the declaration of its parent entity."

**Repo:** `crates/xml/src/participant.rs::parse_publisher_element`, `parse_subscriber_element`, `parse_dp_element`

**Tests:** `crates/xml/tests/dds_xml_full.rs::misplaced_or_unknown_children_are_rejected`, `crates/xml/tests/schema_validation.rs::schema_invalid_documents`

**Status:** done — ein `<data_reader>` unter `<publisher>`, ein `<data_writer>` unter `<subscriber>` und jedes andere Kind von `<domain_participant>`, `<data_writer>` oder `<data_reader>` endet mit `XmlError::UnknownElement` und dem Pfad; mit `report_and_skip` wird es ausgelassen und gemeldet. Das XSD der OMG lehnt dieselben Dokumente ab (`schema_invalid_documents`, Zeile `data_reader_under_publisher`). Commit `8771c676858`.

### §7.3.5.4.1.1 Beispiel (Non-normative)

**Spec:** §7.3.5.4.1.1, S. 26 — `<domain_participant_library="MyDomainParticipantLibrary">`
mit Publisher, Subscriber, DataWriter und DataReader.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Das Starttag hat keinen Attributnamen (`<domain_participant_library="…">`) und ist nicht wohlgeformt; `<data_reader name="MyDataReader"/>` hat kein `topic_ref`, das die XSD der OMG verlangt (`schema_invalid_documents`, Zeile `data_reader_without_topic_ref`) und der Leser mit `XmlError::MissingRequiredElement` abweist; `domain_ref="MyDomain"` ohne Bibliothek löst der Leser auf (Item zu §7.3.5.4.2). `the_participant_example_of_the_specification` liest das Beispiel mit diesen Berichtigungen.

### §7.3.5.4.2 `domain_ref` macht Topics und Typen der Domain verfügbar

**Spec:** §7.3.5.4.2, S. 27 — "DomainParticipants may refer to a Domain
declared in the context of a Domain Library (see Building Block Domains) using
the domain_ref XML attribute. This makes the Topics and Register Types defined
in the Domain available for all the DataWriters and DataReaders defined in the
context of the DomainParticipant."

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_participant_chain`, `resolve_writer`, `resolve_reader`; `crates/xml/src/participant.rs::parse_dp_element`

**Tests:** `crates/xml/tests/dds_xml_full.rs::cross_library_domain_ref_resolves`, `crates/xml/tests/dds_xml_full.rs::deployment_file_with_inheritance_and_cross_references`, `crates/xml/tests/dds_xml_full.rs::participant_local_definitions_resolve_writers`, `crates/xml/tests/dds_xml_full.rs::participant_definition_overrides_the_domains`

**Status:** done — Writer und Reader finden die Topics der Domain; `<topic>` und `<register_type>` in einem Participant sind eigene Definitionen, die zu denen der Domain hinzukommen und gleichnamige ersetzen, wie das XSD der OMG sie typisiert. Commits `5f5d7aaa3e9`, `8d65ac4cd8f`.

### §7.3.5.4.2 `domain_ref` ohne Namen der Bibliothek

**Spec:** §7.3.5.4.2, S. 27 — "using the domain_ref XML attribute".

**Repo:** `crates/xml/src/zerodds_xml.rs::DdsXml::find_domain_in_library`

**Tests:** `crates/xml/tests/dds_xml_full.rs::unqualified_domain_ref_resolves`

**Status:** done — `find_domain_in_library` löst `domain_ref="MyDomain"` (ohne `library::`) über die erste Bibliothek, die eine Domain dieses Namens hat, in Dokumentreihenfolge; `library::name` bleibt möglich. Die PDF legt die Form des Verweises nicht fest. Commit `5f5d7aaa3e9`.

### §7.3.5.4.2 DomainParticipant ohne `domain_ref`

**Spec:** §7.3.5.4.2, S. 27 — "DomainParticipants may refer to a Domain".

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_participant_chain`

**Tests:** `crates/xml/tests/dds_xml_full.rs::participant_without_domain_ref_but_with_domain_id_resolves`, `crates/xml/tests/dds_xml_full.rs::participant_without_domain_resolves_its_own_topics`, `crates/xml/tests/dds_xml_full.rs::participant_domain_id_overrides_the_referenced_domain`, `crates/xml/src/participant.rs::tests::missing_domain_ref_parses_and_is_left_to_the_resolver`

**Status:** done — ein Participant ohne `domain_ref` mit `domain_id` wird aufgelöst, ohne Domain und mit eigenen Definitionen; ohne beide Attribute endet das Auflösen mit `XmlError::UnresolvedReference`. Commit `5f5d7aaa3e9`.

### §7.3.5.4.2 `domain_id` überschreibt die Domain

**Spec:** §7.3.5.4.2, S. 27 — "The Domain Id specified in the parent Domain
can be overridden via the domain_id XML attribute."

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_participant_chain`

**Tests:** `crates/xml/tests/dds_xml_full.rs::participant_domain_id_overrides_the_referenced_domain`

**Status:** done

### §7.3.5.4.2.1 Beispiel (Non-normative)

**Spec:** §7.3.5.4.2.1, S. 27 — `<domain_participant name="MyDomain"
domain_ref="MyDomain" domain_id="32">`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Der Leser liest es und löst es auf: `domain_ref` ohne Bibliothek und `domain_id` als Überschreibung (`unqualified_domain_ref_resolves`, `participant_domain_id_overrides_the_referenced_domain`).

### §7.3.5.4.3 Vererbung von DomainParticipants über `base_name`

**Spec:** §7.3.5.4.3, S. 27 — "DomainParticipants may inherit from
DomainParticipants defined in the context of a DomainParticipant Library
using the base_name XML attribute."

**Repo:** `crates/xml/src/zerodds_xml.rs::participant_chain`, `resolve_participant_chain`

**Tests:** `crates/xml/tests/dds_xml_full.rs::participant_inheritance_one_level`,
`crates/xml/tests/dds_xml_full.rs::participant_inheritance_three_levels`,
`crates/xml/tests/dds_xml_full.rs::participant_inheritance_cycle_detected`

**Status:** done — `domain_ref` und `domain_id` des abgeleiteten Participant überschreiben die der Basis, die QoS wird Policy für Policy gemischt, ein Publisher oder Subscriber gleichen Namens wird ganz ersetzt.

### §7.3.5.4.3 Nur aus vorher definierten Bibliotheken

**Spec:** §7.3.5.4.3, S. 27 — "A DomainParticipant can only inherit from
DomainParticipants in DomainParticipant Libraries that have been defined
before its own definition."

**Repo:** `crates/xml/src/zerodds_xml.rs::participant_chain`, `participant_position`

**Tests:** `crates/xml/tests/dds_xml_full.rs::participant_forward_base_name_rejected`

**Status:** done — `participant_chain` weist eine Basis ab, die in der Dokumentreihenfolge nicht vor dem abgeleiteten Participant steht (Bibliothek, dann Eintrag), mit `XmlError::InvalidXml`. Commit `5f5d7aaa3e9`.

### §7.3.5.4.3.1 Beispiel (Non-normative)

**Spec:** §7.3.5.4.3.1, S. 27 — `base_name="AnotherParticipantLibrary::TheirParticipant"`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel.

### §7.3.5.4.4 QoS inline im Kontext der Entität

**Spec:** §7.3.5.4.4, S. 27 — "Inline QoS setting definition is allowed in
the context an entity's definition."

**Repo:** `crates/xml/src/participant.rs::parse_dp_element`,
`parse_publisher_element`, `parse_subscriber_element`,
`parse_dw_element`, `parse_dr_element`

**Tests:** `crates/xml/src/participant.rs::tests::parse_pub_with_writer`,
`crates/xml/tests/dds_xml_full.rs::participant_inheritance_one_level`

**Status:** done

### §7.3.5.4.4 QoS inline gilt nur für die eigene Entität

**Spec:** §7.3.5.4.4, S. 27 — "Inline QoS settings apply only to the entity
in whose context they are being defined."

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_writer`, `resolve_reader`

**Tests:** `crates/xml/tests/dds_xml_full.rs::datawriter_does_not_inherit_publisher_qos`, `crates/xml/tests/dds_xml_full.rs::publisher_without_qos_yields_none`

**Status:** done — ein `<data_writer>` ohne eigene QoS und ohne `base_name` hat keine QoS aus seinem `<publisher>`; ein `<data_reader>` ebenso. Commit `8771c676858`.

### §7.3.5.4.4 `base_name` an der QoS inline

**Spec:** §7.3.5.4.4, S. 27 — "Inline entities may inherit from an existing
QoS Profile using the base_name XML attribute."

**Repo:** `crates/xml/src/qos_parser.rs::parse_entity_qos`, `set_inline_qos`; `crates/xml/src/registry.rs::QosProfileRegistry::writer_qos_for_inline`, `reader_qos_for_inline`; `crates/webdds-bridge/src/qos_update.rs`

**Tests:** `crates/xml/tests/dds_xml_full.rs::inline_topic_qos_base_name_inherits_from_profile`, `crates/xml/tests/dds_xml_full.rs::repeated_inline_qos_container_is_rejected`, `crates/xml/src/registry.rs::tests::inline_container_inherits_base_name_profile`, `crates/xml/tests/normative_forms.rs::the_participant_example_of_the_specification`

**Status:** done — `parse_entity_qos` liest `base_name` an jeder QoS inline; das benannte Profil ist die Basis. Ein zweiter QoS-Container derselben Art an einer Entität endet mit `XmlError::InvalidXml`. `qos_profile_ref` an `<data_writer>`, `<data_reader>` und `<topic>` ist eine Herstellerform. Commits `5dcde9b5717`, `b956dbaceed`, `92451a0a65f`, `e34118cc9a6`.

### §7.3.5.4.4.1 Beispiel (Non-normative)

**Spec:** §7.3.5.4.4.1, S. 27 — `<domain_participant_qos base_name="BaseProfile">`
mit `entity_factory`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel; das `base_name` der QoS inline bleibt erhalten und benennt die Basis (`the_participant_example_of_the_specification`, `inline_topic_qos_base_name_inherits_from_profile`).

---

## §7.3.6 Building Block Applications

### §7.3.6.1 Zweck

**Spec:** §7.3.6.1, S. 27 — "This block defines the XML syntax to represent
DDS applications that participate (or may be participating) in the DDS
Global Data Space."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweck.

### §7.3.6.2 Abhängig von QoS, Types, Domains und DomainParticipants

**Spec:** §7.3.6.2, S. 27 — "This building block depends on the Building
Block QoS, the Building Block Types, the Building Block Domains, and the
Building Block DomainParticipants."

**Repo:** `crates/xml/schemas/dds-xml_application_definitions_nonamespace.xsd` (bindet die Chameleon-Datei von DomainParticipants ein)

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`

**Status:** done

### §7.3.6.3 Typen für Applications in der Chameleon-Datei

**Spec:** §7.3.6.3, S. 28 — "dds-xml_application_definitions_nonamespace.xsd
contains the XSD type definition for applications and their contained
entities."

**Repo:** `crates/xml/schemas/dds-xml_application_definitions_nonamespace.xsd` (complexTypes `applicationEntry`, `applicationLibrary`)

**Tests:** `crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`

**Status:** done — die Datei der OMG liegt unverändert im Baum; der Test braucht `xmllint`.

### §7.3.6.3 `<application_library>` als Wurzelelement im Namensraum

**Spec:** §7.3.6.3, S. 28 — "dds-xml_application_definititons.xsd defines the
<application_library> root element and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_application_definitions.xsd`;
`crates/xml/src/application.rs::parse_application_libraries`

**Tests:** `crates/xml/src/application.rs::tests::bare_application_library_root_is_a_document`,
`crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`

**Status:** done

### §7.3.6.4.1 Bibliotheken von Applications mit DomainParticipants

**Spec:** §7.3.6.4.1, S. 28 — "Application Libraries are collections of
Applications, which are composed of a set of DomainParticipants and contained
entities. They are the top level elements of the Building Block
Applications."

**Repo:** `crates/xml/src/application.rs::parse_app_library_element`, `parse_app_element`; `crates/xml/src/zerodds_xml.rs::DdsXml::resolve_application`

**Tests:** `crates/xml/src/application.rs::tests::parse_minimal_application`,
`crates/xml/src/application.rs::tests::application_participant_carries_its_whole_definition`,
`crates/xml/tests/dds_xml_full.rs::mixed_top_level_with_cross_refs`

**Status:** done — `parse_app_element` liest die Kinder `<domain_participant>`; ein anderes Kind endet mit `XmlError::UnknownElement`.

### §7.3.6.4.1.1 Beispiel (Non-normative)

**Spec:** §7.3.6.4.1.1, S. 28 — Application `SimpleShapesDemoApplication` mit
`<data_writer>` und `<data_reader>` unter `<publisher>`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Es setzt `<data_reader>` unter `<publisher>`, gegen die Ordnung von §7.3.5.4.1 (S. 26) und gegen die XSD der OMG; der Leser weist es mit `XmlError::UnknownElement` ab (Item zu §7.3.5.4.1). Der Test `application_participant_carries_its_whole_definition` gibt das Beispiel ohne diese Zeile wieder.

### §7.3.6.4.2 DomainParticipants einer Application erben aus Bibliotheken

**Spec:** §7.3.6.4.2, S. 28 — "DomainParticipants defined in the context of an
Application may inherit from DomainParticipants defined in the context of a
DomainParticipant Library using the base_name XML attribute as described in
the Building Block DomainParticipants."

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_application_participant`

**Tests:** `crates/xml/src/application.rs::tests::application_participant_may_inherit_instead_of_naming_a_domain`,
`crates/xml/tests/dds_xml_full.rs::mixed_top_level_with_cross_refs`,
`crates/xml/tests/dds_xml_full.rs::deployment_file_with_inheritance_and_cross_references`

**Status:** done — `base_name` muss die Bibliothek nennen (`library::name`).

### §7.3.6.4.2 Nur aus vorher definierten Bibliotheken

**Spec:** §7.3.6.4.2, S. 28 — "A DomainParticipant can only inherit from
DomainParticipants in DomainParticipant Libraries that have been defined
before its own definition."

**Repo:** `crates/xml/src/zerodds_xml.rs::resolve_application_participant`, `participant_chain`

**Tests:** `crates/xml/tests/spec_items.rs::application_inherits_only_from_libraries_defined_before_it`, `crates/xml/tests/dds_xml_full.rs::participant_forward_base_name_rejected`

**Status:** done — die Kette der Basis wird wie bei DomainParticipants auf Dokumentreihenfolge geprüft (`participant_chain`), und die Bibliothek der Basis muss vor der Application-Bibliothek stehen (`ApplicationLibrary::participant_libraries_before`); sonst `XmlError::InvalidXml`. Commits `5f5d7aaa3e9`, `a45564ecb6b`.

### §7.3.6.4.2.1 Beispiel (Non-normative)

**Spec:** §7.3.6.4.2.1, S. 28 — `base_name="ShapesParticipantLibrary::MyParticipant"`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel.

---

## §7.3.7 Building Block Data Samples

### §7.3.7.1 Zweck

**Spec:** §7.3.7.1, S. 28 — "This block defines XML syntax to represent Data
Samples that may be exchanged between different DDS applications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweck.

### §7.3.7.2 Keine Abhängigkeit von anderen Building Blocks

**Spec:** §7.3.7.2, S. 28 — "This building block has no dependencies on other
building blocks."

**Repo:** `crates/xml/src/sample.rs::parse_sample` (nimmt den Typ als Parameter); `crates/xml/schemas/dds-xml_data_samples_definitions_nonamespace.xsd`

**Tests:** `crates/xml/src/schemas.rs::tests::building_block_xsd_dependencies_match_spec`,
`crates/xml/tests/dds_xml_full.rs::independent_building_blocks_parse_without_the_other_blocks`

**Status:** done

### §7.3.7.3.1 Kein allgemeines XSD für Samples

**Spec:** §7.3.7.3.1, S. 29 — "Because it is impossible to define a generic
XSD file to represent Data Samples of all the possible Data Type combinations
in DDS, the syntax to represent Data Samples is based on the XML
representation rules specified in sub clauses 7.1, 7.1.2, 7.2.2, 7.2.3, and
7.2.6."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung; die Regeln stehen in den Items
unten. ZeroDDS liefert ein eigenes XSD für die Hülle eines Samples (Item zu
§7.3.1).

### §7.3.7.3.1 Struktur nach §7.2.3: Element mit dem Namen des Feldes

**Spec:** §7.3.7.3.1, S. 29 — Regeln von §7.2.3 für Data Samples.

**Repo:** `crates/xml/src/sample.rs::parse_struct_value`

**Tests:** `crates/xml/tests/types_and_samples.rs::t19_sample_parse_struct`, `crates/xml/tests/types_and_samples.rs::t23_sample_with_nested_struct`, `crates/xml/tests/types_and_samples.rs::t25_sample_optional_member_missing_ok`, `crates/xml/src/sample.rs::tests::missing_member_rejected`, `crates/xml/src/sample.rs::tests::parse_sample_unknown_member_rejected`, `crates/xml/src/sample.rs::tests::parse_sample_duplicate_member_rejected`, `crates/xml/src/sample.rs::tests::parse_sample_reads_inherited_members`

**Status:** done — ein Feld ohne Element ist ein Fehler, außer es ist `optional`; ein Kind ohne Feld (auch in der Kette von `baseType` gesucht) und ein zweites Element desselben Feldes enden mit `XmlError`. Commits `8771c676858`, `cbc4ce59003`.

### §7.3.7.3.1 Felder ungeordnet

**Spec:** §7.3.7.3.1, S. 29; §7.2.3, S. 18 — "unordered elements".

**Repo:** `crates/xml/src/sample.rs::parse_struct_value`

**Tests:** `crates/xml/tests/spec_items.rs::sample_fields_come_in_any_order`, `crates/xml/src/sample.rs::tests::parse_sample_members_in_any_order`, `crates/xml/src/sample.rs::tests::parse_sample_duplicate_member_rejected`

**Status:** done — die Felder werden nach Namen gesucht, die Reihenfolge spielt keine Rolle; ein doppeltes Element endet mit `XmlError`. Commit `cbc4ce59003`.

### §7.3.7.3.1 Werte nach Table 7.1: boolean

**Spec:** §7.3.7.3.1, S. 29; Table 7.1, S. 16 — boolean, Groß- und
Kleinschreibung beachtet.

**Repo:** `crates/xml/src/sample.rs::parse_primitive_value`

**Tests:** `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`, `crates/xml/tests/spec_items.rs::primitive_types_example`

**Status:** done — `parse_primitive_value` liest `boolean` mit `parse_bool_strict` (`True` wird abgewiesen).

### §7.3.7.3.1 Werte nach Table 7.1: enum

**Spec:** §7.3.7.3.1, S. 29; Table 7.1, S. 16 — enum als Name des Literals.

**Repo:** `crates/xml/src/sample.rs::parse_value_for_typedef`

**Tests:** `crates/xml/src/sample.rs::tests::parse_sample_enum_rejects_unknown_literal`, `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`, `crates/xml/tests/spec_items.rs::primitive_types_example`

**Status:** done — der Text eines Feldes vom Typ einer Aufzählung wird mit den Literalen des Typs verglichen; ein Literal, das der Typ nicht deklariert, endet mit `XmlError`. Commit `8771c676858`.

### §7.3.7.3.1 Werte nach Table 7.1: long dezimal

**Spec:** §7.3.7.3.1, S. 29; Table 7.1, S. 16 — long -2147483648 bis
2147483647.

**Repo:** `crates/xml/src/sample.rs::parse_primitive_value`, `parse_long_sample`

**Tests:** `crates/xml/tests/types_and_samples.rs::t19_sample_parse_struct`, `crates/xml/src/sample.rs::tests::parse_simple_struct_sample`

**Status:** done — dezimale Werte werden gelesen; die hexadezimale Form und `LENGTH_UNLIMITED` im Item unten.

### §7.3.7.3.1 Werte nach Table 7.1: long hexadezimal und LENGTH_UNLIMITED

**Spec:** §7.3.7.3.1, S. 29; Table 7.1, S. 16 — "or 0x80000000 to 0x7fffffff
or LENGTH_UNLIMITED."

**Repo:** `crates/xml/src/sample.rs::parse_long_sample`, `parse_i16`, `parse_i64`

**Tests:** `crates/xml/src/sample.rs::tests::parse_sample_long_hex_and_constant_names`, `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`

**Status:** done — `parse_long_sample` liest `0x10` und `LENGTH_UNLIMITED` (und die Namen der Konstanten von §7.2.2); `short` und `long long` lesen die hexadezimale Form ebenso. Commit `8771c676858`.

### §7.3.7.3.1 Werte nach Table 7.1: unsigned long

**Spec:** §7.3.7.3.1, S. 29; Table 7.1, S. 16 — unsigned long dezimal und
hexadezimal.

**Repo:** `crates/xml/src/sample.rs::parse_ulong_sample`, `parse_uint`

**Tests:** `crates/xml/src/sample.rs::tests::parse_sample_ulong_constant_names`, `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`

**Status:** done — `parse_ulong_sample` liest die dezimale und die hexadezimale Form (bis 4294967295) und die Namen `DURATION_INFINITE_NSEC`, `DURATION_ZERO_NSEC`, `TIME_INVALID_NSEC`; 4294967296 endet mit `XmlError`.

### §7.3.7.3.1 Werte nach Table 7.1: string

**Spec:** §7.3.7.3.1, S. 29; Table 7.1, S. 16 — string mit Maskierung nach
[XML].

**Repo:** `crates/xml/src/sample.rs::parse_primitive_value`; `crates/xml/src/sample.rs::xml_escape`

**Tests:** `crates/xml/tests/types_and_samples.rs::t24_sample_string_escaping_roundtrip`,
`crates/xml/tests/types_and_samples.rs::t20_sample_roundtrip`

**Status:** done — die Zeichenkette behält ihren Leerraum; maskierte Zeichen werden aufgelöst, der Schreiber maskiert sie.

### §7.3.7.3.1 Konstanten nach §7.2.2 und Duration nach §7.2.6

**Spec:** §7.3.7.3.1, S. 29 — Regeln von §7.2.2 und §7.2.6 für Data Samples.

**Repo:** `crates/xml/src/sample.rs::parse_long_sample`, `parse_ulong_sample`

**Tests:** `crates/xml/tests/spec_items.rs::sample_values_follow_table_7_1`, `crates/xml/src/sample.rs::tests::parse_sample_long_hex_and_constant_names`

**Status:** done — `parse_long_sample` und `parse_ulong_sample` nehmen die Namen der Konstanten an; ein Feld vom Typ `Duration_t` ist eine Struktur mit `sec` und `nanosec`, `<sec>DURATION_INFINITE_SEC</sec>` ergibt 2147483647. Commit `8771c676858`.

### §7.3.7.3.1 Eigene XSD nach dem Chameleon-Muster

**Spec:** §7.3.7.3.1, S. 29 — "Users of this specification who may want to
define schema files to specify the syntax of specific Data Samples can follow
the XML Chameleon Schema Definition pattern."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Möglichkeit für Anwender.

### §7.3.7.3.2 Folgen und Felder mit Elementen `item`

**Spec:** §7.3.7.3.2, S. 29 — "The general XML representation of IDL
sequences and arrays is done following a schema defined as an XSD
complexType. The complexType contains zero or more elements named item.
Nested inside each element is the XSD schema obtained from mapping the IDL
type of the element itself."

**Repo:** `crates/xml/src/sample.rs::parse_member_value`, `parse_array_items`; `crates/xml/src/sample.rs::serialize_value_body`

**Tests:** `crates/xml/src/sample.rs::tests::parse_sample_with_sequence_using_item_tag`, `crates/xml/src/sample.rs::tests::parse_sample_with_array_using_item_tag`, `crates/xml/src/sample.rs::tests::parse_sample_array_length_is_checked`, `crates/xml/src/sample.rs::tests::parse_sample_sequence_over_bound_rejected`, `crates/xml/src/sample.rs::tests::parse_sample_multidimensional_array_is_nested`, `crates/xml/src/sample.rs::tests::parse_sample_with_empty_sequence`, `crates/xml/tests/types_and_samples.rs::t21_sample_with_sequence_items`, `crates/xml/src/sample.rs::tests::serialize_sample_uses_item_tag_for_sequence`

**Status:** done — die Länge eines Feldes fester Länge und die Grenze einer Folge werden geprüft (`XmlError::ValueOutOfRange`); ein mehrdimensionales Feld wird als verschachtelte Ebenen `item` gelesen. Commit `8142acb0c21`.

### §7.3.7.3.2.1 Beispiel (Non-normative)

**Spec:** §7.3.7.3.2.1, S. 29 — `CoordinatesSeq` mit Elementen `item`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel; es typisiert `x` und `y` als
`xs:Integer` (XSD kennt `xs:integer`).

### §7.3.7.4.1 Structure Types (non-normative)

**Spec:** §7.3.7.4.1, S. 30 — `<aStruct>` mit zweimal `<aLong>` und
`<aNestedStruct>`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Eine Struktur der IDL hat keine zwei Felder gleichen Namens; der Leser weist das zweite `<aLong>` ab (`parse_sample_duplicate_member_rejected`).

### §7.3.7.4.2 Unions (non-normative)

**Spec:** §7.3.7.4.2, S. 30 — `<data><aUnion><aLong>0</aLong></aUnion></data>`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel; §7.3.7.3 hat keine normative Regel für Unions. Das Beispiel nennt das aktive Feld ohne Diskriminator; `parse_union_value` liest es (`parse_sample_union_without_discriminator`), und die Form mit `<discriminator>` liest es ebenso (`parse_sample_with_union`). Commit `cbc4ce59003`.

### §7.3.7.4.3 Sequences (non-normative)

**Spec:** §7.3.7.4.3, S. 30 — Folgen von `double` und von Strukturen mit
`item`.

**Repo:** —

**Tests:** `crates/xml/tests/spec_items.rs::sequence_and_array_examples`

**Status:** done — `sequence_and_array_examples` liest das Beispiel.

### §7.3.7.4.4 Arrays (non-normative)

**Spec:** §7.3.7.4.4, S. 30 — Felder von `long` und von Strukturen mit
`item`.

**Repo:** —

**Tests:** `crates/xml/tests/spec_items.rs::sequence_and_array_examples`

**Status:** done — `sequence_and_array_examples` liest das Beispiel.

### §7.3.7.4.5 Primitive Types (non-normative)

**Spec:** §7.3.7.4.5, S. 30–31 — `aShort`, `aUShort`, `anEnum`, `aLong`,
`aULong`, `aLongLong`, `aULongLong`, `aFloat`, `aDouble`, `aBoolean`,
`aString`, `anotherString` mit Zeichenreferenzen, `anOctet` als `0x00`,
`aChar`.

**Repo:** —

**Tests:** `crates/xml/tests/spec_items.rs::primitive_types_example`

**Status:** done — `primitive_types_example` liest alle Felder des Beispiels und prüft die Werte. Table 7.1 hat keine Zeilen für `short`, `unsigned short`, `long long`, `unsigned long long`, `float`, `double`, `octet` und `char`; ihre Form ist nur durch dieses Beispiel beschrieben. Commit `a45564ecb6b`.

---

## §8 Building Block Sets

### §8 Block Sets als Zusammenstellung von Building Blocks

**Spec:** §8, S. 32 — "This Chapter defines some relevant combinations of
building blocks called building block sets. A block set is a collection of
building blocks. Block sets provide a convenient mechanism to group related
building blocks so that other specifications can reference the complete set
as opposed to the individual building blocks."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriff; der einzige Block Set steht in den
Items zu §8.1.

### §8.1 DDS System Block Set mit fünf Building Blocks

**Spec:** §8.1, S. 32 — "This block set offers the ability to describe a
complete DDS system. It contains: Building Block QoS, Building Block Types,
Building Block Domains, Building Block DomainParticipants, Building Block
Applications".

**Repo:** `crates/xml/src/zerodds_xml.rs::parse_dds_xml`, `parse_dds_xml_with`; `crates/xml/schemas/dds-xml_dds_system_definitions_nonamespace.xsd` (complexType `ddsSystem`)

**Tests:** `crates/xml/src/zerodds_xml.rs::tests::parse_mixed_top_level`,
`crates/xml/tests/dds_xml_full.rs::all_four_libraries_roundtrip`,
`crates/xml/tests/types_and_samples.rs::t17_dds_xml_resolve_type`,
`crates/xml/tests/dds_xml_full.rs::empty_dds_root_is_valid`

**Status:** done — `<dds>` nimmt die fünf Bibliotheken in jeder Reihenfolge und Zahl an; ein anderes Kind endet mit `XmlError::UnknownElement`. Data Samples gehören nicht dazu. Die Reihenfolge der Bibliotheken zählt für die Regeln „vorher definiert“ (Items zu §7.3.4.4.1 und §7.3.6.4.2).

### §8.1 Chameleon-Datei des Block Set

**Spec:** §8.1, S. 32 — "dds-xml_dds_system_definitions_nonamespace.xsd
contains all the XSD type definitions for the DDS System Block Set. It
defines no targetNamespace so that it can be reused by other schemas
following the XML Chameleon Schema Definition pattern."

**Repo:** `crates/xml/schemas/dds-xml_dds_system_definitions_nonamespace.xsd`

**Tests:** `crates/xml/src/schemas.rs::tests::nonamespace_xsds_omit_target_namespace`,
`crates/xml/tests/schema_validation.rs::every_in_tree_schema_compiles`

**Status:** done — die Datei der OMG liegt unverändert im Baum; sie bindet die Chameleon-Datei von Applications ein, die die übrigen einbindet, und hat weder `targetNamespace` noch Wurzelelement. `every_in_tree_schema_compiles` braucht `xmllint`.

### §8.1 `<dds>` als oberstes Element im Namensraum

**Spec:** §8.1, S. 32 — "dds-xml_dds_system_definitions.xsd defines the <dds>
top-level element, and sets targetNamespace to
http://www.omg.org/spec/DDS-XML."

**Repo:** `crates/xml/schemas/dds-xml_dds_system_definitions.xsd`

**Tests:** `crates/xml/src/schemas.rs::tests::namespaced_xsds_define_top_level_element`,
`crates/xml/src/schemas.rs::tests::namespaced_xsds_include_target_namespace`

**Status:** done

---

## Audit-Status

165 done / 0 partial / 0 open / 51 n/a (informative) / 0 n/a (rejected). Zuvor: 79 / 23 / 63 / 51 / 0.

Test-Lauf: Linux-Testhost, Stand `2de529e05a6`: `zerodds-xml` Lib-Tests 313 bestanden; der Lauf der Integrationstests brach in `fuzz_smoke` mit einem Stack-Überlauf ab (behoben in `24780539a95`). Die Tests, die danach entstanden oder sich änderten (`spec_items`, `fuzz_smoke`, `schema_validation`, `typeobject_bridge` und die Tests der Commits `a45564ecb6b`, `43146783064`, `2120724498f`), sind auf dem Mac nur übersetzt (`cargo clippy --all-targets`), nicht ausgeführt; ein Lauf auf dem Linux-Testhost steht aus.

Offene Punkte: keine.
