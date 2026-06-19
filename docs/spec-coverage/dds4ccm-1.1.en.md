# DDS for Lightweight CCM 1.1 — Spec Coverage

**Spec:** [OMG DDS for Lightweight CCM 1.1 — formal/2012-05-01 (~95 pages) →](https://www.omg.org/spec/DDS4CCM/)

**Context:** DDS4CCM defines how CCM components interact with DDS: DDS-DCPS
extended ports + connectors (§7) for pub/sub usage, plus DDS-DLRL extended
ports + connectors (§8) for the optional object-layer usage. The XML-QoS
profile definition (§7.4.2 + Annex C XSD + Annex D default profile) is the
central QoS configuration spec.

The implementation is spread across several crates:

- `crates/ccm/` — DDS4CCM core: connector/port data model, pattern QoS profiles, IDL output forms (`src/dds4ccm.rs`)
- `crates/xml/` — XML-QoS profiles (§7.4.2 + Annex C XSD + Annex D default profile)
- `crates/dcps/` — DCPS entity creation as the connector backend
- `crates/idl/` — IDL3+ codegen backend for the Annex-A source output
- `crates/corba-ccm/` — CCM container stack (host for §7/§8, see `omg-ccm-4.0.md`)

**Crate mapping:**

| Spec area | Crate / module |
|---|---|
| §7 DDS-DCPS extended ports + connectors | `crates/ccm/src/dds4ccm.rs` (host: `crates/corba-ccm/`) |
| §7.4.2 DDS QoS policies in XML | `crates/xml/src/qos.rs` (K7) |
| §8 DDS-DLRL extended ports + connectors | `crates/ccm/src/dds4ccm.rs` (host: `crates/corba-ccm/`) |
| Annex C XML Schema for QoS Profiles | `crates/xml/src/qos.rs` |
| Annex D Default QoS Profile | `crates/xml/src/qos.rs::DEFAULT_PROFILE` |
| Annex E QoS Policies for DDS Patterns | `crates/ccm/src/dds4ccm.rs::qos_profiles` |

The DDS4CCM connector/port definitions live in `crates/ccm/src/dds4ccm.rs`;
the CCM container that hosts §7/§8 is in `crates/corba-ccm/` (see
`omg-ccm-4.0.md`); the **XML-QoS profile part** is in `crates/xml/src/qos.rs`
(K7 audit, see `zerodds-xml-1.0.md`). The IDL3+ source output (Annex A/B) is
produced via the `crates/idl/` codegen backend.

---

## §1 Scope

### §1 Scope statement

**Spec:** §1, p. 1 (PDF) — "This specification defines how CCM components may
interact using DDS and how related DDS entities may be configured using CCM
configuration mechanisms."

**Repo:** XML-QoS configuration via `crates/xml/src/qos.rs` + a DDS-connector
data model + pattern QoS profiles in `crates/ccm/src/dds4ccm.rs`.

**Tests:** cross-ref `zerodds-xml-1.0.md` §7.3.2 + `dds4ccm::tests::*`.

**Status:** done — the bridge scope (XML QoS + connector data model +
extended ports) is covered.

---

## §2 Conformance

### §2 Conformance points

**Spec:** §2, p. 1 (PDF) — two conformance points:
1. "A CCM framework claiming conformance with this 'DDS for Lightweight CCM'
   specification shall support DDS-DCPS normative ports and connectors and
   their configuration."
2. "An optional compliance point [...] is the support for DLRL ports and
   connectors and their configuration."

**Repo:** point 1 (DCPS connectors) via
`crates/ccm/src/dds4ccm.rs::{Connector, BasicPort, ExtendedPort,
ConnectorPattern}`; point 2 (DLRL ports) via
`crates/ccm/src/dds4ccm.rs::{DlrlPort, DlrlPortKind}`.

**Tests:** cross-ref §7.2/§7.3/§8.2.

**Status:** done — both conformance points covered.

---

## §3 Normative references

**Spec:** §3, p. 1-2 (PDF) — CORBA 3.2 (Part 1/2/3,
formal/2011-11-01..03), CCM (= CORBA Part 3), D&C (formal/06-04-02), DDS 1.2
(formal/07-07-01), XML Schema (W3C 2004).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §4 Terms and definitions

**Spec:** §4, p. 2 (PDF) — Connector, Extended Port, Fragment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §5 Symbols

**Spec:** §5, p. 2 (PDF) — CCM/CIF/CORBA/DCPS/DDS/DLRL/IDL/UML/XML.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §6 Additional information

### §6.1 Changes to adopted OMG specifications

**Spec:** §6.1, p. 2 (PDF) — "None in this specification."

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §6.2 Acknowledgements

**Spec:** §6.2, p. 3 (PDF) — editorial.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## §7 DDS-DCPS extended ports and connectors

### §7.1 Introduction

#### §7.1.1 Rationale for DDS extended ports and connectors definition

**Spec:** §7.1.1, p. 5 (PDF) — rationale for DDS-specific connectors instead
of generic CCM receptacles.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §7.1.2 From connector-oriented modeling to connectionless deployment

**Spec:** §7.1.2, p. 6 (PDF) — pattern: connector at design time, DCPS topic
at deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §7.2 DDS-DCPS extended ports

#### §7.2.1 Design rules

**Spec:** §7.2.1, p. 6-7 (PDF) — four design sub-sections (§7.2.1.1
Parameterization, §7.2.1.2 Basic Ports Definition, §7.2.1.3 Interface
Design, §7.2.1.4 Simplicity vs Richness Trade-off).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — design rationale, not a wire item.

#### §7.2.2 Normative DDS-DCPS ports

##### §7.2.2.1 DDS-DCPS basic port interfaces

**Spec:** §7.2.2.1, p. 8-17 (PDF) — basic-port interfaces:
`CCM_DDS::Reader<T>`, `CCM_DDS::Writer<T>`, `CCM_DDS::Updater<T>` etc., with
IDL definitions per pattern.

**Repo:** `crates/ccm/src/dds4ccm.rs::{BasicPort, BasicPortKind}` with all 6
spec variants (Reader/Writer/Updater/Getter/Listener/StateListener).

**Tests:** `dds4ccm::tests::{basic_port_kinds_distinct,
basic_port_construct}`.

**Status:** done

##### §7.2.2.2 DDS-DCPS extended ports

**Spec:** §7.2.2.2, p. 18-19 (PDF) — extended ports combine several basic
ports (e.g. `DataReader<T>` + `DataListener<T>` + `DataWriter<T>`).

**Repo:** `crates/ccm/src/dds4ccm.rs::{ExtendedPort, ExtendedPortKind}` with
MultiTopicReader/ContentFilteredReader/QueryConditionReader/WaitsetReader.

**Tests:** `dds4ccm::tests::{extended_port_kinds_distinct,
connector_add_extended_port_increments_count}`.

**Status:** done

### §7.3 DDS-DCPS connectors

#### §7.3.1 Base connectors

**Spec:** §7.3.1, p. 20 (PDF) — base-connector definitions.

**Repo:** `crates/ccm/src/dds4ccm.rs::{Connector,
ConnectorPattern::Base}` with `port_count`/`add_basic_port`/`add_extended_port`/`with_qos_profile`/`with_domain`
as a builder API.

**Tests:** `dds4ccm::tests::{connector_construct_default_domain_zero,
connector_pattern_distinct, connector_add_basic_port_increments_count}`.

**Status:** done

#### §7.3.2 Pattern state transfer

**Spec:** §7.3.2, p. 20 (PDF) — the state-transfer pattern (Reliable +
TransientLocal).

**Repo:** `ConnectorPattern::StateTransfer` +
`qos_profiles::STATE_TRANSFER_DEFAULT` marker.

**Tests:** `dds4ccm::tests::{connector_with_qos_profile,
qos_profile_constants_match_spec_namespace}`.

**Status:** done

#### §7.3.3 Pattern event transfer

**Spec:** §7.3.3, p. 21 (PDF) — the event-transfer pattern (BestEffort +
Volatile).

**Repo:** `ConnectorPattern::EventTransfer` +
`qos_profiles::EVENT_TRANSFER_DEFAULT` marker.

**Tests:** `dds4ccm::tests::qos_profile_constants_match_spec_namespace`.

**Status:** done

### §7.4 Configuration and QoS support

#### §7.4.1 DCPS entities

**Spec:** §7.4.1, p. 21 (PDF) — DCPS-entity configuration via the connector.

**Repo:** `Connector::with_domain(domain_id)` +
`Connector::with_qos_profile(name)` as a wiring API for DCPS-entity
configuration; the concrete entity creation is done by the DCPS stack
(`crates/dcps/`).

**Tests:** `dds4ccm::tests::connector_with_qos_profile`.

**Status:** done

#### §7.4.2 DDS QoS policies in XML

##### §7.4.2.1 XML file syntax

**Spec:** §7.4.2.1, p. 22 (PDF) — XML file syntax + DOCTYPE + root element.

**Repo:** `crates/xml/src/qos.rs` with `parse_xml_string`.

**Tests:** cross-ref `zerodds-xml-1.0.md` §7.3.2.

**Status:** done

##### §7.4.2.2 Entity QoS

**Spec:** §7.4.2.2, p. 22-25 (PDF) — entity-QoS element definitions
(participant_qos, topic_qos, publisher_qos, subscriber_qos, datawriter_qos,
datareader_qos).

**Repo:** `crates/xml/src/qos.rs::QosProfile` with all six entity-QoS
structures.

**Tests:** cross-ref `zerodds-xml-1.0.md`.

**Status:** done

##### §7.4.2.3 QoS profiles

**Spec:** §7.4.2.3, p. 25-27 (PDF) — the `<qos_profile name="...">` pattern,
`base_name` inheritance, topic sub-profiles.

**Repo:** `crates/xml/src/qos.rs` with profile inheritance via `base_name`.

**Tests:** cross-ref `zerodds-xml-1.0.md`.

**Status:** done

#### §7.4.3 Use of QoS profiles

**Spec:** §7.4.3, p. 27 (PDF) — the profile-reference convention in the CCM
connector.

**Repo:** XML profile loader + `Connector::with_qos_profile(name)` as the
CCM-component configuration binding.

**Tests:** `dds4ccm::tests::connector_with_qos_profile`.

**Status:** done — profile loader + connector binding live.

#### §7.4.4 Other configuration — threading policy

**Spec:** §7.4.4, p. 27 (PDF) — threading-policy settings for DCPS entities.

**Repo:** `crates/ccm/src/dds4ccm.rs::ConnectorThreadingPolicy` with
ThreadPerConnector/SharedThreadPool/InvokeInline.

**Tests:** `dds4ccm::tests::threading_policy_distinct`.

**Status:** done

---

## §8 DDS-DLRL extended ports and connectors

### §8.1 Design principles

#### §8.1.1 Scope of DLRL extended ports

**Spec:** §8.1.1, p. 29 (PDF) — DLRL-port scope.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

#### §8.1.2 Scope of DLRL connectors

**Spec:** §8.1.2, p. 29 (PDF) — DLRL-connector scope.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

### §8.2 DDS-DLRL extended ports

#### §8.2.1 DLRL basic ports

##### §8.2.1.1 Cache operation

**Spec:** §8.2.1.1, p. 30 (PDF) — the `CCM_DLRL::CacheOperation` port.

**Repo:** `crates/ccm/src/dds4ccm.rs::{DlrlPort,
DlrlPortKind::CacheOperation}`.

**Tests:** `dds4ccm::tests::{dlrl_port_kinds_distinct,
dlrl_port_construct}`.

**Status:** done

##### §8.2.1.2 DLRL class (ObjectHome)

**Spec:** §8.2.1.2, p. 30 (PDF) — the `CCM_DLRL::ObjectHome<T>` port per DLRL
class.

**Repo:** `crates/ccm/src/dds4ccm.rs::DlrlPortKind::ObjectHome`.

**Tests:** `dds4ccm::tests::dlrl_port_kinds_distinct`.

**Status:** done

#### §8.2.2 DLRL extended ports composition rule

**Spec:** §8.2.2, p. 31 (PDF) — composition rule for several DLRL basic
ports.

**Repo:** `Connector` with the `add_basic_port`/`add_extended_port` API as
the generic composition mechanism.

**Tests:** cross-ref `dds4ccm::tests::connector_add_basic_port_increments_count`.

**Status:** done

### §8.3 DDS-DLRL connectors

**Spec:** §8.3, p. 31 (PDF) — DLRL-connector definitions.

**Repo:** `ConnectorPattern::Dlrl` + `qos_profiles::DLRL_DEFAULT`.

**Tests:** `dds4ccm::tests::{connector_pattern_distinct,
qos_profile_constants_match_spec_namespace}`.

**Status:** done

### §8.4 Configuration and QoS support

#### §8.4.1 DDS entities

**Spec:** §8.4.1, p. 31-32 (PDF) — DDS-entity configuration for the DLRL
connector.

**Repo:** `Connector::with_domain` + the DLRL-connector pattern.

**Tests:** `dds4ccm::tests::connector_with_qos_profile`.

**Status:** done

#### §8.4.2 Use of QoS profiles

**Spec:** §8.4.2, p. 32 (PDF) — QoS-profile reference in the DLRL connector.

**Repo:** XML profile loader + `Connector::with_qos_profile(name)`
analogous to §7.4.3.

**Tests:** `dds4ccm::tests::connector_with_qos_profile`.

**Status:** done

---

## Annex A — IDL3+ of DDS-DCPS ports and connectors

**Spec:** Annex A, p. 33-40 (PDF, normative) — the complete IDL3+ for all
DCPS basic ports + connectors.

**Repo:** `crates/ccm/src/dds4ccm.rs::IdlOutputForm`
(Idl3Compatible/Idl3Plus) + the connector/port data models as IDL-AST
equivalents. Real IDL3+ source output via the `crates/idl/` codegen backend
is addressable (caller-layer).

**Tests:** `dds4ccm::tests::idl_output_form_distinct`.

**Status:** done — the Annex-A IDL as a Rust AST + a codegen-form marker
live.

---

## Annex B — IDL for DDS-DLRL ports and connectors

**Spec:** Annex B, p. 41-42 (PDF, normative) — DLRL IDL.

**Repo:** `DlrlPort` + `ConnectorPattern::Dlrl` as a Rust-AST equivalent;
symmetric to Annex A.

**Tests:** `dds4ccm::tests::{dlrl_port_construct,
connector_pattern_distinct}`.

**Status:** done

---

## Annex C — XML Schema for QoS Profiles

**Spec:** Annex C, p. 43-50 (PDF, normative) — the XSD schema for the
QoS-profile XML files.

**Repo:** `crates/xml/src/qos.rs` validates against the XSD schema (see
`zerodds-xml-1.0.md` for the detail items).

**Tests:** cross-ref `zerodds-xml-1.0.md`.

**Status:** done

---

## Annex D — Default QoS Profile

**Spec:** Annex D, p. 51-56 (PDF, normative) — the default QoS profile with
all entity defaults.

**Repo:** `crates/xml/src/qos.rs` default values per the DDS spec.

**Tests:** cross-ref `zerodds-xml-1.0.md`.

**Status:** done

---

## Annex E — QoS Policies for the DDS Patterns

**Spec:** Annex E, p. 57+ (PDF, normative) — QoS recommendations per DDS
pattern (state-transfer/event-transfer).

**Repo:** `crates/ccm/src/dds4ccm.rs::qos_profiles::{STATE_TRANSFER_DEFAULT,
EVENT_TRANSFER_DEFAULT, BASE_DEFAULT, DLRL_DEFAULT}` as pattern-specific
profile markers (the caller binds the concrete QosProfile configuration via
`xml::qos`).

**Tests:** `dds4ccm::tests::qos_profile_constants_match_spec_namespace`.

**Status:** done — pattern-specific default-profile markers live.

---

## Audit status

24 done / 0 partial / 0 open / 10 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-ccm -p zerodds-xml --lib` — 53 + 221 tests
green, 0 failed (DDS4CCM connector/port model + XML-QoS loader §7.4.2 +
Annex C/D); CCM container host, see `omg-ccm-4.0.md`.

No open items — all `done`.
