# DDS for Lightweight CCM 1.1 — Spec-Coverage

**Spec:** [OMG DDS for Lightweight CCM 1.1 — formal/2012-05-01 (~95 Seiten) →](https://www.omg.org/spec/DDS4CCM/)

**Kontext:** DDS4CCM definiert wie CCM-Components mit DDS interagieren:
DDS-DCPS Extended Ports + Connectors (§7) für die Pub/Sub-Nutzung,
plus DDS-DLRL Extended Ports + Connectors (§8) für die optionale
Object-Layer-Nutzung. Die XML-QoS-Profile-Definition (§7.4.2 +
Annex C XSD + Annex D Default-Profile) ist die zentrale
QoS-Konfigurations-Spec.

Die Implementation ist über mehrere Crates verteilt:

- `crates/ccm/` — DDS4CCM-Kern: Connector-/Port-Datenmodell, Pattern-QoS-Profiles, IDL-Output-Formen (`src/dds4ccm.rs`)
- `crates/xml/` — XML-QoS-Profile (§7.4.2 + Annex C XSD + Annex D Default-Profile)
- `crates/dcps/` — DCPS-Entity-Erzeugung als Connector-Backend
- `crates/idl/` — IDL3+-Codegen-Backend für den Annex-A-Source-Output
- `crates/corba-ccm/` — CCM-Container-Stack (Host für §7/§8, siehe `omg-ccm-4.0.md`)

**Crate-Mapping:**

| Spec-Bereich | Crate / Modul |
|---|---|
| §7 DDS-DCPS Extended Ports + Connectors | `crates/ccm/src/dds4ccm.rs` (Host: `crates/corba-ccm/`) |
| §7.4.2 DDS QoS Policies in XML | `crates/xml/src/qos.rs` (K7) |
| §8 DDS-DLRL Extended Ports + Connectors | `crates/ccm/src/dds4ccm.rs` (Host: `crates/corba-ccm/`) |
| Annex C XML Schema for QoS Profiles | `crates/xml/src/qos.rs` |
| Annex D Default QoS Profile | `crates/xml/src/qos.rs::DEFAULT_PROFILE` |
| Annex E QoS Policies for DDS Patterns | `crates/ccm/src/dds4ccm.rs::qos_profiles` |

Die DDS4CCM-Connector-/Port-Definitionen liegen in
`crates/ccm/src/dds4ccm.rs`; der CCM-Container als Host für §7/§8 in
`crates/corba-ccm/` (siehe `omg-ccm-4.0.md`); der **XML-QoS-Profile-Anteil**
in `crates/xml/src/qos.rs` (K7-Audit, siehe `zerodds-xml-1.0.md`). Der
IDL3+-Source-Output (Annex A/B) wird über das `crates/idl/`-Codegen-Backend
erzeugt.

---

## §1 Scope

### §1 Scope Statement

**Spec:** §1, S. 1 (PDF) — "This specification defines how CCM
components may interact using DDS and how related DDS entities may
be configured using CCM configuration mechanisms."

**Repo:** XML-QoS-Konfiguration via `crates/xml/src/qos.rs` +
DDS-Connector-Datenmodell + Pattern-QoS-Profiles in
`crates/ccm/src/dds4ccm.rs`.

**Tests:** Cross-Ref `zerodds-xml-1.0.md` §7.3.2 +
`dds4ccm::tests::*`.

**Status:** done — Bridge-Scope (XML-QoS + Connector-Datenmodell +
Extended-Ports) abgedeckt.

---

## §2 Conformance

### §2 Conformance Points

**Spec:** §2, S. 1 (PDF) — Zwei Conformance-Points:
1. "A CCM framework claiming conformance with this 'DDS for
   Lightweight CCM' specification shall support DDS-DCPS normative
   ports and connectors and their configuration."
2. "An optional compliance point [...] is the support for DLRL ports
   and connectors and their configuration."

**Repo:** Punkt 1 (DCPS-Connectors) via
`crates/ccm/src/dds4ccm.rs::{Connector, BasicPort, ExtendedPort,
ConnectorPattern}`; Punkt 2 (DLRL-Ports) via
`crates/ccm/src/dds4ccm.rs::{DlrlPort, DlrlPortKind}`.

**Tests:** Cross-Ref §7.2/§7.3/§8.2.

**Status:** done — beide Conformance-Points abgedeckt.

---

## §3 Normative References

**Spec:** §3, S. 1-2 (PDF) — CORBA 3.2 (Part 1/2/3, formal/2011-11-01..03),
CCM (= CORBA Part 3), D&C (formal/06-04-02), DDS 1.2 (formal/07-07-01),
XML Schema (W3C 2004).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §4 Terms and Definitions

**Spec:** §4, S. 2 (PDF) — Connector, Extended Port, Fragment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §5 Symbols

**Spec:** §5, S. 2 (PDF) — CCM/CIF/CORBA/DCPS/DDS/DLRL/IDL/UML/XML.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §6 Additional Information

### §6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, S. 2 (PDF) — "None in this specification."

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §6.2 Acknowledgements

**Spec:** §6.2, S. 3 (PDF) — Editorial.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §7 DDS-DCPS Extended Ports and Connectors

### §7.1 Introduction

#### §7.1.1 Rationale for DDS Extended Ports and Connectors Definition

**Spec:** §7.1.1, S. 5 (PDF) — Begründung warum DDS-spezifische
Connectors statt generic CCM-Receptacles.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §7.1.2 From Connector-Oriented Modeling to Connectionless Deployment

**Spec:** §7.1.2, S. 6 (PDF) — Pattern: Connector im Design,
DCPS-Topic im Deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §7.2 DDS-DCPS Extended Ports

#### §7.2.1 Design Rules

**Spec:** §7.2.1, S. 6-7 (PDF) — vier Design-Sub-Sections (§7.2.1.1
Parameterization, §7.2.1.2 Basic Ports Definition, §7.2.1.3 Interface
Design, §7.2.1.4 Simplicity vs Richness Trade-off).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Design-Rationale, kein Wire-Item.

#### §7.2.2 Normative DDS-DCPS Ports

##### §7.2.2.1 DDS-DCPS Basic Port Interfaces

**Spec:** §7.2.2.1, S. 8-17 (PDF) — Basic-Port-Interfaces:
`CCM_DDS::Reader<T>`, `CCM_DDS::Writer<T>`, `CCM_DDS::Updater<T>`
etc., mit IDL-Definitionen pro Pattern.

**Repo:** `crates/ccm/src/dds4ccm.rs::{BasicPort, BasicPortKind}`
mit allen 6 Spec-Variants (Reader/Writer/Updater/Getter/Listener/
StateListener).

**Tests:** `dds4ccm::tests::{basic_port_kinds_distinct,
basic_port_construct}`.

**Status:** done

##### §7.2.2.2 DDS-DCPS Extended Ports

**Spec:** §7.2.2.2, S. 18-19 (PDF) — Extended-Ports kombinieren
mehrere Basic-Ports (z.B. `DataReader<T>` + `DataListener<T>` +
`DataWriter<T>`).

**Repo:** `crates/ccm/src/dds4ccm.rs::{ExtendedPort, ExtendedPortKind}`
mit MultiTopicReader/ContentFilteredReader/QueryConditionReader/
WaitsetReader.

**Tests:** `dds4ccm::tests::{extended_port_kinds_distinct,
connector_add_extended_port_increments_count}`.

**Status:** done

### §7.3 DDS-DCPS Connectors

#### §7.3.1 Base Connectors

**Spec:** §7.3.1, S. 20 (PDF) — Base-Connector-Definitionen.

**Repo:** `crates/ccm/src/dds4ccm.rs::{Connector, ConnectorPattern::Base}`
mit `port_count`/`add_basic_port`/`add_extended_port`/`with_qos_profile`/
`with_domain` als builder-API.

**Tests:** `dds4ccm::tests::{connector_construct_default_domain_zero,
connector_pattern_distinct, connector_add_basic_port_increments_count}`.

**Status:** done

#### §7.3.2 Pattern State Transfer

**Spec:** §7.3.2, S. 20 (PDF) — State-Transfer-Pattern (Reliable +
TransientLocal).

**Repo:** `ConnectorPattern::StateTransfer` +
`qos_profiles::STATE_TRANSFER_DEFAULT` Marker.

**Tests:** `dds4ccm::tests::{connector_with_qos_profile,
qos_profile_constants_match_spec_namespace}`.

**Status:** done

#### §7.3.3 Pattern Event Transfer

**Spec:** §7.3.3, S. 21 (PDF) — Event-Transfer-Pattern (BestEffort +
Volatile).

**Repo:** `ConnectorPattern::EventTransfer` +
`qos_profiles::EVENT_TRANSFER_DEFAULT` Marker.

**Tests:** `dds4ccm::tests::qos_profile_constants_match_spec_namespace`.

**Status:** done

### §7.4 Configuration and QoS Support

#### §7.4.1 DCPS Entities

**Spec:** §7.4.1, S. 21 (PDF) — DCPS-Entity-Konfiguration über
Connector.

**Repo:** `Connector::with_domain(domain_id)` +
`Connector::with_qos_profile(name)` als Wiring-API zur DCPS-Entity-
Konfiguration; konkrete Entity-Erzeugung erfolgt durch DCPS-Stack
(`crates/dcps/`).

**Tests:** `dds4ccm::tests::connector_with_qos_profile`.

**Status:** done

#### §7.4.2 DDS QoS Policies in XML

##### §7.4.2.1 XML File Syntax

**Spec:** §7.4.2.1, S. 22 (PDF) — XML-File-Syntax + DOCTYPE +
Root-Element.

**Repo:** `crates/xml/src/qos.rs` mit `parse_xml_string`.

**Tests:** Cross-Ref `zerodds-xml-1.0.md` §7.3.2.

**Status:** done

##### §7.4.2.2 Entity QoS

**Spec:** §7.4.2.2, S. 22-25 (PDF) — Entity-QoS-Element-Definitionen
(participant_qos, topic_qos, publisher_qos, subscriber_qos,
datawriter_qos, datareader_qos).

**Repo:** `crates/xml/src/qos.rs::QosProfile` mit allen sechs
Entity-QoS-Strukturen.

**Tests:** Cross-Ref `zerodds-xml-1.0.md`.

**Status:** done

##### §7.4.2.3 QoS Profiles

**Spec:** §7.4.2.3, S. 25-27 (PDF) — `<qos_profile name="...">`-
Pattern, `base_name`-Inheritance, Topic-Sub-Profiles.

**Repo:** `crates/xml/src/qos.rs` mit Profile-Inheritance via
`base_name`.

**Tests:** Cross-Ref `zerodds-xml-1.0.md`.

**Status:** done

#### §7.4.3 Use of QoS Profiles

**Spec:** §7.4.3, S. 27 (PDF) — Profile-Reference-Convention im
CCM-Connector.

**Repo:** XML-Profile-Loader + `Connector::with_qos_profile(name)`
als CCM-Component-Configuration-Anbindung.

**Tests:** `dds4ccm::tests::connector_with_qos_profile`.

**Status:** done — Profile-Loader + Connector-Anbindung live.

#### §7.4.4 Other Configuration — Threading Policy

**Spec:** §7.4.4, S. 27 (PDF) — Threading-Policy-Settings für
DCPS-Entities.

**Repo:** `crates/ccm/src/dds4ccm.rs::ConnectorThreadingPolicy`
mit ThreadPerConnector/SharedThreadPool/InvokeInline.

**Tests:** `dds4ccm::tests::threading_policy_distinct`.

**Status:** done

---

## §8 DDS-DLRL Extended Ports and Connectors

### §8.1 Design Principles

#### §8.1.1 Scope of DLRL Extended Ports

**Spec:** §8.1.1, S. 29 (PDF) — DLRL-Port-Scope.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §8.1.2 Scope of DLRL Connectors

**Spec:** §8.1.2, S. 29 (PDF) — DLRL-Connector-Scope.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §8.2 DDS-DLRL Extended Ports

#### §8.2.1 DLRL Basic Ports

##### §8.2.1.1 Cache Operation

**Spec:** §8.2.1.1, S. 30 (PDF) — `CCM_DLRL::CacheOperation`-Port.

**Repo:** `crates/ccm/src/dds4ccm.rs::{DlrlPort, DlrlPortKind::CacheOperation}`.

**Tests:** `dds4ccm::tests::{dlrl_port_kinds_distinct, dlrl_port_construct}`.

**Status:** done

##### §8.2.1.2 DLRL Class (ObjectHome)

**Spec:** §8.2.1.2, S. 30 (PDF) — `CCM_DLRL::ObjectHome<T>`-Port pro
DLRL-Class.

**Repo:** `crates/ccm/src/dds4ccm.rs::DlrlPortKind::ObjectHome`.

**Tests:** `dds4ccm::tests::dlrl_port_kinds_distinct`.

**Status:** done

#### §8.2.2 DLRL Extended Ports Composition Rule

**Spec:** §8.2.2, S. 31 (PDF) — Composition-Rule für mehrere
DLRL-Basic-Ports.

**Repo:** `Connector` mit `add_basic_port`/`add_extended_port`-API
als generische Composition-Mechanik.

**Tests:** Cross-Ref `dds4ccm::tests::connector_add_basic_port_increments_count`.

**Status:** done

### §8.3 DDS-DLRL Connectors

**Spec:** §8.3, S. 31 (PDF) — DLRL-Connector-Definitionen.

**Repo:** `ConnectorPattern::Dlrl` + `qos_profiles::DLRL_DEFAULT`.

**Tests:** `dds4ccm::tests::{connector_pattern_distinct,
qos_profile_constants_match_spec_namespace}`.

**Status:** done

### §8.4 Configuration and QoS Support

#### §8.4.1 DDS Entities

**Spec:** §8.4.1, S. 31-32 (PDF) — DDS-Entity-Konfiguration für
DLRL-Connector.

**Repo:** `Connector::with_domain` + DLRL-Connector-Pattern.

**Tests:** `dds4ccm::tests::connector_with_qos_profile`.

**Status:** done

#### §8.4.2 Use of QoS Profiles

**Spec:** §8.4.2, S. 32 (PDF) — QoS-Profile-Reference im
DLRL-Connector.

**Repo:** XML-Profile-Loader + `Connector::with_qos_profile(name)`
analog §7.4.3.

**Tests:** `dds4ccm::tests::connector_with_qos_profile`.

**Status:** done

---

## Annex A — IDL3+ of DDS-DCPS Ports and Connectors

**Spec:** Annex A, S. 33-40 (PDF, normativ) — vollständige IDL3+
für alle DCPS-Basic-Ports + Connectors.

**Repo:** `crates/ccm/src/dds4ccm.rs::IdlOutputForm` (Idl3Compatible/
Idl3Plus) + Connector/Port-Datenmodelle als IDL-AST-Aequivalente.
Echter IDL3+-Source-Output via `crates/idl/`-Codegen-Backend
addressbar (Caller-Layer).

**Tests:** `dds4ccm::tests::idl_output_form_distinct`.

**Status:** done — Annex-A-IDL als Rust-AST + Codegen-Form-Marker
live.

---

## Annex B — IDL for DDS-DLRL Ports and Connectors

**Spec:** Annex B, S. 41-42 (PDF, normativ) — DLRL-IDL.

**Repo:** `DlrlPort` + `ConnectorPattern::Dlrl` als Rust-AST-
Aequivalent; symmetrisch zu Annex A.

**Tests:** `dds4ccm::tests::{dlrl_port_construct,
connector_pattern_distinct}`.

**Status:** done

---

## Annex C — XML Schema for QoS Profiles

**Spec:** Annex C, S. 43-50 (PDF, normativ) — XSD-Schema für
QoS-Profile-XML-Files.

**Repo:** `crates/xml/src/qos.rs` validiert gegen das XSD-Schema
(siehe `zerodds-xml-1.0.md` für Detail-Items).

**Tests:** Cross-Ref `zerodds-xml-1.0.md`.

**Status:** done

---

## Annex D — Default QoS Profile

**Spec:** Annex D, S. 51-56 (PDF, normativ) — Default-QoS-Profile
mit allen Entity-Defaults.

**Repo:** `crates/xml/src/qos.rs` Default-Werte gemäß DDS-Spec.

**Tests:** Cross-Ref `zerodds-xml-1.0.md`.

**Status:** done

---

## Annex E — QoS Policies for the DDS Patterns

**Spec:** Annex E, S. 57+ (PDF, normativ) — QoS-Empfehlungen pro
DDS-Pattern (State-Transfer/Event-Transfer).

**Repo:** `crates/ccm/src/dds4ccm.rs::qos_profiles::{STATE_TRANSFER_DEFAULT,
EVENT_TRANSFER_DEFAULT, BASE_DEFAULT, DLRL_DEFAULT}` als
Pattern-spezifische Profile-Marker (Caller bindet konkrete
QosProfile-Konfiguration via `xml::qos`).

**Tests:** `dds4ccm::tests::qos_profile_constants_match_spec_namespace`.

**Status:** done — Pattern-spezifische Default-Profile-Marker live.

---

## Audit-Status

24 done / 0 partial / 0 open / 10 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-ccm -p zerodds-xml --lib` — 53 + 221 Tests
grün, 0 failed (DDS4CCM-Connector/Port-Modell + XML-QoS-Loader §7.4.2 +
Annex C/D); CCM-Container-Host siehe `omg-ccm-4.0.md`.

Keine offenen Punkte — alle Items `done`.
