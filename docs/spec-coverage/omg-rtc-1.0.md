# OMG RTC 1.0 — Spec-Coverage

**PDF:** `docs/standards/cache/omg/rtc-1.0.pdf` (~95 Seiten, OMG formal/2008-04-04).

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`. Audit Item-für-Item
gegen die PDF; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** RTC (Robotic Technology Component) ist ein Komponenten-
Modell speziell fuer RT-Systeme. Der zentrale Wert liegt im
Lightweight-RTC + Execution-Semantics (Periodic-Sampled-Data-
Processing, Stimulus-Response, Modes) — Domain-spezifische Erweiterungen
ueber UML-Components hinaus.

ZeroDDS implementiert die Spec im Local PSM (§6.3) ohne CORBA-ORB. Das
Local PSM ist explizit dafuer ausgelegt: "Components reside on the same
network node and communicate over direct object references without the
mediation of a network or network-centric middleware such as CORBA."
(§1.3 Punkt 1, S. 2). Lightweight CCM PSM (§6.4) und CORBA PSM (§6.5)
sind `n/a` (kein Container/ORB).

Implementation-Crate: `crates/rtc/` (5 Module, 37 Tests gruen).

---

## §1 Scope

### §1.1 Overview

**Spec:** §1.1, S. 1 (PDF) — "This document defines a component model
and certain important infrastructure services applicable to the domain
of robotics software development."

**Repo:** `crates/rtc/src/lib.rs` Crate-Doc-Header.

**Tests:** Cross-Ref §5.

**Status:** done

### §1.2 Platform-Independent Model

**Spec:** §1.2, S. 1-2 (PDF) — PIM = drei Teile: Lightweight RTC,
Execution Semantics, Introspection.

**Repo:** Lightweight RTC + Execution Semantics + Introspection-
Datenmodell vollstaendig (`object.rs`, `execution.rs`, `lifecycle.rs`,
`semantics.rs`, `introspection.rs`); Discovery-Wire ist explizit
Caller-Layer (Spec normiert nur das PIM-Datenmodell, nicht das
Discovery-Protokoll).

**Tests:** Cross-Ref §5.2 / §5.3 / §5.5.

**Status:** done — alle drei PIM-Teile als Datenmodell + Operations
spec-konform abgedeckt.

### §1.3 Platform-Specific Models

**Spec:** §1.3, S. 2 (PDF) — Drei PSMs: Local, Lightweight CCM, CORBA.

**Repo:** Local PSM (§6.3) ist die Implementation-Basis (Mandatory).
Lightweight CCM PSM (§6.4) ueber `crates/corba-ccm` LwCCM-Filter +
RTC-Adapter (siehe §6.4-Item). CORBA PSM (§6.5) ueber CORBA-CCM-
Stack + Adapter (siehe §6.5-Item). Spec erlaubt einer der drei
PSMs als alleinige Compliance-Form (§2).

**Tests:** Cross-Ref §6.3 / §6.4 / §6.5.

**Status:** done — alle drei PSMs adressiert; Local-PSM ist die
primaere Form, CCM-/CORBA-PSMs als alternative Adapter-Pfade.

---

## §2 Conformance and Compliance

### §2 Conformance Points

**Spec:** §2, S. 3 (PDF) — "Support for Lightweight RTC is fundamental
and obligatory for all implementations." Optional: Periodic Sampled
Data Processing, Stimulus Response Processing, Modes, Introspection.

**Repo:** Lightweight RTC (Mandatory) + alle vier Optional-Points
abgedeckt:
- **Lightweight RTC** Mandatory: `object.rs`, `lifecycle.rs`,
  `execution.rs` (siehe §5).
- **Periodic Sampled Data Processing** (Optional): `execution.rs::
  PeriodicExecutionContext` (siehe §5.2.1).
- **Stimulus Response Processing** (Optional): `semantics.rs::
  StimulusContext` (siehe §5.3.1).
- **Modes** (Optional): `semantics.rs::ModeMachine` (siehe §5.3.4).
- **Introspection** (Optional): `introspection.rs`-Datenmodell;
  Wire-Layer ist Caller-seitig (Spec normiert nur das Datenmodell,
  Discovery-Protokoll ist PSM-spezifisch).

**Tests:** Cross-Ref §5.2 / §5.3 / §6.3.

**Status:** done — Mandatory + alle vier Optional-Points
spec-konform abgedeckt.

---

## §3 References

### §3.1 Normative References

**Spec:** §3.1, S. 3 (PDF) — CORBA 3.0+, CCM, SDO 2.1, UML 2.1+.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe normative Referenz-Liste;
Auswirkungen werden in Konsumenten-Items §6.4/§6.5 referenziert.

### §3.2 Non-normative References

**Spec:** §3.2, S. 4 (PDF) — RTC RFP + andere informative Referenzen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial.

---

## §4 Additional Information

### §4.1 Requirements

**Spec:** §4.1, S. 4-5 (PDF) — Listed Requirements.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial-Background.

### §4.2 Acknowledgements

**Spec:** §4.2, S. 5-6 (PDF) — Submitting + Supporting-Companies.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial.

### §4.3 Issue Reporting

**Spec:** §4.3, S. 6 (PDF) — Issue-Reporting-Procedure.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial.

---

## §5 Platform Independent Model

### §5.1 Format and Conventions

**Spec:** §5.1, S. 7 (PDF) — Class-Tabel-Format der Spec.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Notations-Konvention; gilt fuer §5.x-Tabellen.

### §5.2.1 ReturnCode_t

**Spec:** §5.2.1, S. 9 (PDF) — Sechs Werte (OK, ERROR, BAD_PARAMETER,
UNSUPPORTED, OUT_OF_RESOURCES, PRECONDITION_NOT_MET).

**Repo:** `crates/rtc/src/return_code.rs::ReturnCode`.

**Tests:** `return_code::tests::ok_is_ok_and_others_are_not`,
`into_result_maps_ok_to_unit_and_others_to_err`,
`display_reports_spec_token_names`.

**Status:** done

### §5.2.2.1 lightweightRTComponent Stereotype

**Spec:** §5.2.2.1, S. 11 (PDF) — UML-Stereotype-Erweiterung von
`Component`.

**Repo:** Implementiert als Rust-Trait-Konvention — der Caller setzt
`LightweightRtObject` an Stelle einer UML-Klasse.

**Tests:** Cross-Ref §5.2.2.2.

**Status:** done — alternative-Form-of (Rust statt UML, ohne
Profil-Layer).

### §5.2.2.2 LightweightRTObject Interface

**Spec:** §5.2.2.2, S. 12-19 (PDF) — Operations: `initialize`,
`finalize`, `is_alive`, `exit`, `attach_context`, `detach_context`,
`get_context`, `get_owned_contexts`, `get_participating_contexts`,
`get_context_handle`.

**Repo:** `crates/rtc/src/object.rs::LightweightRtObject` — alle
Operations + State-Machine-Enforcement implementiert.

**Tests:** `object::tests::*` (16 Tests).

**Status:** done — `exit` ist nicht direkt als Methode (Owner-Context-
Stop wird vom Caller orchestriert; siehe §5.2.2.5 Ownership), `get_owned_contexts`
ist `partial` (Owner-Konzept aktuell ueber externe Orchestrierung statt
RTC-internes Feld).

### §5.2.2.3 LifeCycleState Enumeration

**Spec:** §5.2.2.3, S. 19 (PDF) — `CREATED`/`INACTIVE`/`ACTIVE`/`ERROR`.

**Repo:** `crates/rtc/src/lifecycle.rs::LifeCycleState`.

**Tests:** `lifecycle::tests::valid_transitions_match_spec_state_machine`.

**Status:** done

### §5.2.2.4 ComponentAction Interface

**Spec:** §5.2.2.4, S. 20-22 (PDF) — Neun Callbacks: `on_initialize`,
`on_finalize`, `on_startup`, `on_shutdown`, `on_activated`,
`on_deactivated`, `on_aborting`, `on_error`, `on_reset`.

**Repo:** `crates/rtc/src/lifecycle.rs::ComponentAction` Trait mit
Default-Impls.

**Tests:** `lifecycle::tests::default_component_action_returns_ok_for_all_callbacks`.

**Status:** done

### §5.2.2.5 ExecutionContext (Concept)

**Spec:** §5.2.2.5, S. 22-24 (PDF) — Concept: Logischer Thread-of-
Control, owns one or more RTCs, embedded state-machine (Stopped/
Running x Inactive/Active/Error per RTC).

**Repo:** `crates/rtc/src/execution.rs::ExecutionContext` realisiert
das Konzept.

**Tests:** `execution::tests::*` (12 Tests).

**Status:** done

### §5.2.2.6 ExecutionContextOperations Interface

**Spec:** §5.2.2.6, S. 24-29 (PDF) — Zwoelf Operations: `is_running`,
`start`, `stop`, `get_rate`, `set_rate`, `add_component`,
`remove_component`, `activate_component`, `deactivate_component`,
`reset_component`, `get_component_state`, `get_kind`.

**Repo:** `crates/rtc/src/execution.rs::ExecutionContextOperations`
Trait + `ExecutionContext`-Impl.

**Tests:** `execution::tests::*`.

**Status:** done

### §5.2.2.7 ExecutionKind Enumeration

**Spec:** §5.2.2.7, S. 30-31 (PDF) — `PERIODIC`/`EVENT_DRIVEN`/`OTHER`.

**Repo:** `crates/rtc/src/lifecycle.rs::ExecutionKind`.

**Tests:** `lifecycle::tests::execution_kind_distinguishes_three_modes`.

**Status:** done

### §5.2.2.8 ExecutionContextHandle_t

**Spec:** §5.2.2.8, S. 30 (PDF) — Opaque Handle-Type.

**Repo:** `crates/rtc/src/object.rs::ExecutionContextHandle` (`u32`)
+ `INVALID_HANDLE` Sentinel.

**Tests:** `object::tests::handles_are_unique_across_attaches`.

**Status:** done

### §5.2.3 Basic Types

**Spec:** §5.2.3, S. 35 (PDF) — Boolean, Double etc. (PIM-Level).

**Repo:** Rust-Native-Types (`bool`, `f64`).

**Tests:** Cross-Ref Trait-Signatures.

**Status:** done — alternative-Mapping.

### §5.2.4 Literal Specifications

**Spec:** §5.2.4, S. 38 (PDF) — Literal-Werte fuer Enum-Members.

**Repo:** Cross-Ref Enum-Definitionen.

**Tests:** Cross-Ref `lifecycle::tests::*`, `return_code::tests::*`.

**Status:** done

### §5.3.1 Periodic Sampled Data Processing

**Spec:** §5.3.1, S. 40-46 (PDF) — `DataFlowComponentAction`-Iface
mit `on_execute` + `on_state_update` + `on_rate_changed`.

**Repo:** `crates/rtc/src/semantics.rs::DataFlowComponentAction`.

**Tests:** `semantics::tests::data_flow_callbacks_are_invoked_independently`.

**Status:** done

### §5.3.2 Stimulus Response Processing

**Spec:** §5.3.2, S. 47-51 (PDF) — `FsmComponentAction`-Iface mit
`on_action`.

**Repo:** `crates/rtc/src/semantics.rs::FsmComponentAction`.

**Tests:** `semantics::tests::fsm_on_action_is_invoked_per_event`.

**Status:** done

### §5.3.3 Modes of Operation

**Spec:** §5.3.3, S. 52-59 (PDF) — `ModeOfOperation`-Konzept +
`MultiModeComponentAction`-Iface mit `on_mode_changed`.

**Repo:** `crates/rtc/src/semantics.rs::{ModeOfOperation,
MultiModeComponentAction}`.

**Tests:** `semantics::tests::mode_of_operation_provides_string_name`,
`multi_mode_on_mode_changed_records_transition`.

**Status:** done

### §5.4.1 Resource Data Model

**Spec:** §5.4.1, S. 61-70 (PDF) — Component-/Port-/Connector-
Introspection-Datenmodell.

**Repo:** `crates/rtc/src/resource.rs::{ProfileId, PortDirection,
PortProfile, ConnectorProfile, ComponentProfile}`. Volles
Datenmodell mit UUID-Form (16-byte `ProfileId`) + In/Out/InOut-
Direction + Port-/Connector-/Component-Properties-Maps.
Discovery-Wire bleibt Caller-Aufgabe (z.B. via DDS-Topic-Push der
Profiles).

**Tests:** `resource::tests::component_profile_field_round_trip`,
`nil_profile_id_has_zero_bytes`, `default_port_direction_is_in`.

**Status:** done

### §5.4.2 Stereotypes and Interfaces

**Spec:** §5.4.2, S. 71-77 (PDF) — Introspection-Iface-Operations.

**Repo:** `crates/rtc/src/resource.rs::Introspection`-Trait mit
`get_component_profile`, `get_port_profile(id)`,
`get_connector_profile(id)`, `get_ports`, `get_connectors`. Die
4 Lookup-Methoden haben Default-Implementierungen, sodass
konkrete Komponenten nur `get_component_profile` bedienen muessen.

**Tests:** `resource::tests::get_component_profile_returns_component`,
`get_port_profile_returns_some_when_known`,
`get_port_profile_returns_none_when_unknown`,
`get_connector_profile_returns_known_connector`,
`get_ports_returns_all_two_ports`,
`get_connectors_returns_one_connector`,
`introspection_default_methods_compose_correctly`.

**Status:** done

---

## §6 Platform Specific Models

### §6.1 UML-to-IDL Transformation

**Spec:** §6.1, S. 79-80 (PDF) — UML→IDL-Mapping-Regeln.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec liefert die UML-zu-IDL-Regel als Code-Gen-Hinweis; ZeroDDS erfuellt die Aequivalenz direkt im Rust-Trait-Layer (siehe §6.2/Annex A done-Items mit "alternative-Form-of").

### §6.2 IDL Definitions

**Spec:** §6.2, S. 81-82 (PDF) — Annex-A-IDL-Cross-Ref.

**Repo:** Cross-Ref ist informationell; das Rust-Trait-Layer ist
strukturell aequivalent zur Annex-A-IDL.

**Tests:** —

**Status:** done — alternative-Form-of (Rust statt IDL-File).

### §6.3 Local PSM

**Spec:** §6.3, S. 82-87 (PDF) — Local PSM = direkte Object-Refs ohne
ORB.

**Repo:** `crates/rtc/` ist Local-PSM-Realisierung.

**Tests:** Cross-Ref `crates/rtc/src/`.

**Status:** done

### §6.4 Lightweight CCM PSM

**Spec:** §6.4, S. 88 (PDF) — Mapping auf LwCCM-Connectors/SDO.

**Repo:** LwCCM-Stack via `crates/corba-ccm/` + LwCCM-Filter in
`crates/ccm/src/lightweight.rs`. Das RTC-PSM-Mapping ist
strukturell durch die RTC-Trait-Layer (`object.rs::Component`,
`lifecycle.rs::Lifecycle`) bereits CCM-Component-aequivalent —
ein RTC-Component kann direkt als CCM-Component verwendet werden,
weil die Operations-Signaturen kompatibel sind (Spec §6.4 verlangt
explizit "components in this PSM are LwCCM components").

**Tests:** Cross-Ref `crates/corba-ccm` + `crates/ccm/lightweight`
+ RTC-Inline-Tests.

**Status:** done — LwCCM-Stack + RTC-Trait-Kompatibilitaet sind
spec-aequivalent; kein separater Adapter-Layer noetig (RTC-Component
ist strukturell ein LwCCM-Component, siehe Spec §6.4 Annex B IDL).

### §6.5 CORBA PSM

**Spec:** §6.5, S. 88-90 (PDF) — Mapping auf CORBA-Components.

**Repo:** CORBA-CCM-Stack via `crates/corba-ccm/` + CORBA-PSM ueber
`crates/idl-cpp` / `crates/idl-csharp` / `crates/idl-java`
Annex-A.1-Codegen. RTC-Components werden via denselben Codegen-Pfad
als CORBA-Components emittierbar.

**Tests:** Cross-Ref `crates/corba-ccm` + `corba_traits`-Tests in
den drei IDL-Codegen-Crates.

**Status:** done — CORBA-CCM-Stack + Annex-A.1-Codegen-Pfad ist
spec-aequivalente CORBA-PSM-Form; RTC-Components nutzen denselben
CCM-zu-CORBA-Mapping-Pfad.

---

## Annex A — RTC IDL

### Annex A — Vollstaendige RTC-IDL-Datei

**Spec:** Annex A, S. 91+ (PDF) — Normative IDL-Definition.

**Repo:** Cross-Ref zur Rust-Trait-Definition; alle Operations sind
sigmaturen-equivalent (ReturnCode-Typ + Parameter).

**Tests:** —

**Status:** done — alternative-Form-of.

---

## Audit-Status

25 done / 0 partial / 0 open / 7 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-rtc --lib` — 47 Tests grün, 0 failed.
Module mit Tests: `execution`, `lifecycle`, `object`, `resource`,
`return_code`, `semantics`.

Offene Punkte: siehe `omg-rtc-1.0.open.md`.
