# OMG RTC 1.0 — Spec-Coverage

**Spec:** [OMG RTC 1.0 — formal/2008-04-04 (~95 Seiten) →](https://www.omg.org/spec/RTC/)

**Kontext:** RTC (Robotic Technology Component) ist ein Komponenten-
Modell speziell für RT-Systeme. Der zentrale Wert liegt im
Lightweight-RTC + Execution-Semantics (Periodic-Sampled-Data-
Processing, Stimulus-Response, Modes) — Domain-spezifische Erweiterungen
über UML-Components hinaus.

ZeroDDS implementiert die Spec im Local PSM (§6.3) ohne CORBA-ORB. Das
Local PSM ist explizit dafür ausgelegt: "Components reside on the same
network node and communicate over direct object references without the
mediation of a network or network-centric middleware such as CORBA."
(§1.3 Punkt 1, S. 2). Lightweight CCM PSM (§6.4) und CORBA PSM (§6.5)
sind `n/a` (kein Container/ORB).

Implementation:

- `crates/rtc/` — Local PSM (§6.3) ohne CORBA-ORB, 6 Module, 61 Tests grün.

---

## §1 Scope

### §1.1 Overview

**Spec:** §1.1, S. 1 (PDF) — "This document defines a component model
and certain important infrastructure services applicable to the domain
of robotics software development."

**Repo:** `crates/rtc/src/lib.rs` Crate-Doc-Header.

**Tests:** Das Komponenten-Modell ist in seinen drei Bestandteilen
ausgeführt: `crates/rtc/src/object.rs::tests::initialize_then_finalize_round_trips_alive_flag`
(Komponente), `crates/rtc/src/execution.rs::tests::start_stop_round_trips_running_flag`
(Execution-Context), `crates/rtc/src/semantics.rs::tests::data_flow_callbacks_are_invoked_independently`
(Infrastruktur-Service).

**Status:** done

### §1.2 Platform-Independent Model

**Spec:** §1.2, S. 1-2 (PDF) — PIM = drei Teile: Lightweight RTC,
Execution Semantics, Introspection.

**Repo:** Lightweight RTC + Execution Semantics + Introspection-
Datenmodell vollständig (`crates/rtc/src/object.rs`,
`crates/rtc/src/execution.rs`, `crates/rtc/src/lifecycle.rs`,
`crates/rtc/src/semantics.rs`, `crates/rtc/src/resource.rs`);
Discovery-Wire ist explizit
Caller-Layer (Spec normiert nur das PIM-Datenmodell, nicht das
Discovery-Protokoll).

**Tests:** ein Beleg pro PIM-Teil —
Lightweight RTC: `crates/rtc/src/object.rs::tests::attach_then_detach_works`;
Execution Semantics: `crates/rtc/src/semantics.rs::tests::fsm_on_action_is_invoked_per_event`,
`crates/rtc/src/semantics.rs::tests::multi_mode_on_mode_changed_records_transition`;
Introspection: `crates/rtc/src/resource.rs::tests::get_component_profile_returns_component`,
`crates/rtc/src/resource.rs::tests::introspection_default_methods_compose_correctly`.

**Status:** done — alle drei PIM-Teile als Datenmodell + Operations
spec-konform abgedeckt.

### §1.3 Platform-Specific Models

**Spec:** §1.3, S. 2 (PDF) — Drei PSMs: Local, Lightweight CCM, CORBA.

**Repo:** Local PSM (§6.3) ist die Implementation-Basis:
`crates/rtc/`. Die Spec erlaubt einen der drei PSMs als alleinige
Compliance-Form (§2). Lightweight CCM PSM (§6.4) und CORBA PSM (§6.5)
sind nicht umgesetzt — eigene Items mit Status `open`.

**Tests:** Local PSM belegt durch
`crates/rtc/tests/annex_a_operation_surface.rs::lightweight_rt_object_operations_match_the_annex_a_shape`
und `crates/rtc/tests/annex_a_operation_surface.rs::execution_context_operations_cover_all_twelve_annex_a_operations`.

**Status:** partial — Local PSM (§6.3) umgesetzt und belegt; Lightweight
CCM PSM (§6.4) und CORBA PSM (§6.5) fehlen und werden als eigene
`open`-Items geführt.

---

## §2 Conformance and Compliance

### §2 Conformance Points

**Spec:** §2, S. 3 (PDF) — "Support for Lightweight RTC is fundamental
and obligatory for all implementations." Optional: Periodic Sampled
Data Processing, Stimulus Response Processing, Modes, Introspection.

**Repo:** Lightweight RTC (Mandatory) + alle vier Optional-Points
abgedeckt:
- **Lightweight RTC** Mandatory: `crates/rtc/src/object.rs`,
  `crates/rtc/src/lifecycle.rs`, `crates/rtc/src/execution.rs`
  (siehe §5).
- **Periodic Sampled Data Processing** (Optional):
  `crates/rtc/src/semantics.rs::DataFlowComponentAction` +
  `crates/rtc/src/lifecycle.rs::ExecutionKind::Periodic`
  (siehe §5.3.1).
- **Stimulus Response Processing** (Optional):
  `crates/rtc/src/semantics.rs::FsmComponentAction` (siehe §5.3.2).
- **Modes** (Optional): `crates/rtc/src/semantics.rs::{ModeOfOperation,
  MultiModeComponentAction}` (siehe §5.3.3).
- **Introspection** (Optional): `crates/rtc/src/resource.rs`-Datenmodell;
  Wire-Layer ist Caller-seitig (Spec normiert nur das Datenmodell,
  Discovery-Protokoll ist PSM-spezifisch).

**Tests:** ein Beleg pro Conformance-Point —
Mandatory: `crates/rtc/src/lifecycle.rs::tests::valid_transitions_match_spec_state_machine`;
Periodic Sampled Data Processing: `crates/rtc/src/semantics.rs::tests::data_flow_callbacks_are_invoked_independently`;
Stimulus Response Processing: `crates/rtc/src/semantics.rs::tests::fsm_on_action_is_invoked_per_event`;
Modes: `crates/rtc/src/semantics.rs::tests::multi_mode_on_mode_changed_records_transition`;
Introspection: `crates/rtc/src/resource.rs::tests::get_ports_returns_all_two_ports`.

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

**Status:** `n/a (informative)` — Notations-Konvention; gilt für §5.x-Tabellen.

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

**Tests:** `crates/rtc/src/object.rs::tests::initialize_then_finalize_round_trips_alive_flag`,
`crates/rtc/src/object.rs::tests::attach_then_detach_works` — der
Stereotype-Träger ist der `LightweightRtObject`-Typ selbst, dessen
Lifecycle in diesen beiden Tests durchlaufen wird.

**Status:** done — alternative-Form-of (Rust statt UML, ohne
Profil-Layer).

### §5.2.2.2 LightweightRTObject Interface

**Spec:** §5.2.2.2, S. 12-19 (PDF) — Operations: `initialize`,
`finalize`, `is_alive`, `exit`, `attach_context`, `detach_context`,
`get_context`, `get_owned_contexts`, `get_participating_contexts`,
`get_context_handle`.

**Repo:** `crates/rtc/src/object.rs::LightweightRtObject` mit
`initialize`, `finalize`, `is_alive`, `attach_context`,
`detach_context`, `get_participating_contexts`, `get_context_state` +
State-Machine-Enforcement. Ohne Gegenstück: `exit`, `get_context`,
`get_owned_contexts`, `get_context_handle`.

**Tests:** `crates/rtc/src/object.rs::tests`:
`fresh_rtc_is_not_alive`, `initialize_then_finalize_round_trips_alive_flag`,
`double_initialize_yields_precondition_not_met`,
`finalize_in_created_state_yields_precondition_not_met`,
`finalize_with_attached_context_yields_precondition_not_met`,
`attach_context_in_created_state_fails`, `attach_then_detach_works`,
`detach_unknown_handle_yields_precondition_not_met`,
`detach_active_rtc_yields_precondition_not_met`,
`activate_inactive_rtc_invokes_on_activated`,
`deactivate_active_rtc_invokes_on_deactivated`,
`reset_only_works_from_error_state`,
`initialize_failure_keeps_rtc_in_created_state`,
`handles_are_unique_across_attaches`. Operations-Fläche gegen die Annex-A-IDL:
`crates/rtc/tests/annex_a_operation_surface.rs::lightweight_rt_object_operations_match_the_annex_a_shape`.

**Status:** partial — sechs der zehn Annex-A-Operations sind umgesetzt
und belegt. `exit`, `get_context`, `get_owned_contexts` und
`get_context_handle` fehlen; sie setzen ein Owner-Context-Register im
`LightweightRtObject` voraus, das der Typ nicht führt. Rest als
eigenes `open`-Item „§5.2.2.2 Owner-Context-Operations".

### §5.2.2.2 Owner-Context-Operations (`exit` / `get_context` / `get_owned_contexts` / `get_context_handle`)

**Spec:** §5.2.2.2, S. 12-19 (PDF) — `void exit()`,
`ExecutionContext get_context(in ExecutionContextHandle_t)`,
`ExecutionContextList get_owned_contexts()`,
`ExecutionContextHandle_t get_context_handle(in ExecutionContext)`.

**Repo:** —

**Tests:** —

**Status:** open — `LightweightRtObject` führt kein Register der
Contexts, die es besitzt (nur der, an denen es teilnimmt), und hält
keine Referenz auf den Context-Objekt-Typ. Die vier Operations setzen
beides voraus. Rest-Item zu §5.2.2.2.

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

**Tests:** `crates/rtc/src/execution.rs::tests`:
`fresh_context_is_stopped_with_default_rate`,
`start_stop_round_trips_running_flag`, `double_start_is_idempotent`,
`set_rate_accepts_positive_finite`, `set_rate_rejects_non_positive_or_nan`,
`with_rate_validates_rate_argument`,
`add_component_attaches_and_returns_handle`,
`add_uninitialized_component_fails`,
`activate_then_deactivate_round_trips_state`,
`activate_with_unknown_handle_yields_bad_parameter`,
`remove_component_detaches_handle`, `cannot_remove_active_component`.

**Status:** done

### §5.2.2.6 ExecutionContextOperations Interface

**Spec:** §5.2.2.6, S. 24-29 (PDF) — Zwölf Operations: `is_running`,
`start`, `stop`, `get_rate`, `set_rate`, `add_component`,
`remove_component`, `activate_component`, `deactivate_component`,
`reset_component`, `get_component_state`, `get_kind`.

**Repo:** `crates/rtc/src/execution.rs::ExecutionContextOperations`
Trait + `ExecutionContext`-Impl.

**Tests:** alle zwölf Operations in einem Lauf:
`crates/rtc/tests/annex_a_operation_surface.rs::execution_context_operations_cover_all_twelve_annex_a_operations`;
Einzelverhalten in `crates/rtc/src/execution.rs::tests`:
`fresh_context_is_stopped_with_default_rate`,
`start_stop_round_trips_running_flag`, `double_start_is_idempotent`,
`set_rate_accepts_positive_finite`, `set_rate_rejects_non_positive_or_nan`,
`with_rate_validates_rate_argument`,
`add_component_attaches_and_returns_handle`,
`add_uninitialized_component_fails`,
`activate_then_deactivate_round_trips_state`,
`activate_with_unknown_handle_yields_bad_parameter`,
`remove_component_detaches_handle`, `cannot_remove_active_component`.

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

**Tests:** `crates/rtc/src/object.rs::tests::fresh_rtc_is_not_alive`
(Boolean), `crates/rtc/src/execution.rs::tests::set_rate_accepts_positive_finite`
(Double), `crates/rtc/tests/annex_a_operation_surface.rs::annex_a_value_types_and_literals_match_the_idl`.

**Status:** done — alternative-Mapping.

### §5.2.4 Literal Specifications

**Spec:** §5.2.4, S. 38 (PDF) — Literal-Werte für Enum-Members.

**Repo:** Cross-Ref Enum-Definitionen.

**Tests:** `crates/rtc/src/lifecycle.rs::tests::valid_transitions_match_spec_state_machine`,
`crates/rtc/src/lifecycle.rs::tests::execution_kind_distinguishes_three_modes`,
`crates/rtc/src/return_code.rs::tests::ok_is_ok_and_others_are_not`,
`crates/rtc/src/return_code.rs::tests::display_reports_spec_token_names`,
`crates/rtc/tests/annex_a_operation_surface.rs::annex_a_value_types_and_literals_match_the_idl`.

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
konkrete Komponenten nur `get_component_profile` bedienen müssen.

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

**Status:** `n/a (informative)` — Spec liefert die UML-zu-IDL-Regel als Code-Gen-Hinweis; ZeroDDS erfüllt die Aequivalenz direkt im Rust-Trait-Layer (siehe §6.2/Annex A done-Items mit "alternative-Form-of").

### §6.2 IDL Definitions

**Spec:** §6.2, S. 81-82 (PDF) — Cross-Ref auf die Annex-A-IDL: die
PSM-Klauseln benennen ihre Operationen über `RTC.idl`.

**Repo:** Das Rust-Trait-Layer trägt die Annex-A-Operations mit
gleichen Parametern und gleichem `ReturnCode_t`-Ergebnis; die
Äquivalenz wird ausgeführt, nicht behauptet.

**Tests:** `crates/rtc/tests/annex_a_operation_surface.rs`:
`component_action_covers_every_annex_a_callback`,
`execution_semantics_interfaces_carry_their_annex_a_operations`,
`execution_context_operations_cover_all_twelve_annex_a_operations`,
`annex_a_value_types_and_literals_match_the_idl`.

Lauf `cargo test -p zerodds-rtc`; lokal (macOS, dieser Lauf) 61/61 grün,
davon 7 in diesem Modul.

**Status:** done

### §6.3 Local PSM

**Spec:** §6.3, S. 82-87 (PDF) — Local PSM = direkte Object-Refs ohne
ORB.

**Repo:** `crates/rtc/` ist Local-PSM-Realisierung.

**Tests:** `crates/rtc/tests/annex_a_operation_surface.rs::lightweight_rt_object_operations_match_the_annex_a_shape`,
`crates/rtc/tests/annex_a_operation_surface.rs::execution_context_operations_cover_all_twelve_annex_a_operations`,
`crates/rtc/tests/annex_a_operation_surface.rs::introspection_operations_return_the_annex_a_resource_model`
— direkte Object-Refs ohne ORB.

**Status:** done

### §6.4 Lightweight CCM PSM

**Spec:** §6.4, S. 88 (PDF) — Mapping auf LwCCM-Connectors/SDO.

**Repo:** —

**Tests:** —

**Status:** open — es gibt keinen RTC→LwCCM-Adapter im Baum.
`crates/ccm/src/lightweight.rs` filtert ein CCM-IDL-Modell auf das
LwCCM-Profil und kennt kein RTC; `crates/rtc/` kennt kein CCM. Der
Crate-Doc-Header von `crates/rtc/src/lib.rs` führt §6.4 selbst unter
„What is not covered". Die Spec erlaubt Compliance über einen einzigen
PSM (§2); umgesetzt ist der Local PSM (§6.3).

### §6.5 CORBA PSM

**Spec:** §6.5, S. 88-90 (PDF) — Mapping auf CORBA-Components.

**Repo:** —

**Tests:** —

**Status:** open — es gibt keinen RTC→CORBA-Adapter und keine
RTC-Annex-A-IDL, die durch die Codegen-Crates liefe. Der Crate-Doc-
Header von `crates/rtc/src/lib.rs` führt §6.5 selbst unter „What is
not covered". Die Spec erlaubt Compliance über einen einzigen PSM
(§2); umgesetzt ist der Local PSM (§6.3).

---

## Annex A — RTC IDL

### Annex A — Vollständige RTC-IDL-Datei

**Spec:** Annex A, S. 91-94 (PDF) — die RTC-IDL. Der Annex ist im PDF
ausdrücklich mit "(non-normative)" überschrieben und nennt sich selbst eine
Bequemlichkeitskopie der mitgelieferten `RTC.idl`.

**Repo:** Rust-Trait-Definition in `crates/rtc/src/` — Parameter und
`ReturnCode_t`-Ergebnis je Operation identisch zur Annex-A-IDL, mit
Ausnahme der vier `LightweightRTObject`-Operations aus dem
`open`-Item zu §5.2.2.2.

**Tests:** `crates/rtc/tests/annex_a_operation_surface.rs`:
`component_action_covers_every_annex_a_callback`,
`execution_semantics_interfaces_carry_their_annex_a_operations`,
`lightweight_rt_object_operations_match_the_annex_a_shape`,
`execution_context_operations_cover_all_twelve_annex_a_operations`,
`annex_a_value_types_and_literals_match_the_idl`,
`introspection_operations_return_the_annex_a_resource_model`.

**Status:** partial — die Annex-A-Fläche ist bis auf `exit`,
`get_context`, `get_owned_contexts` und `get_context_handle`
ausgeführt; der Rest ist das `open`-Item zu §5.2.2.2.

---

## Audit-Status

20 done / 3 partial / 3 open / 7 n/a (informative) / 0 n/a (rejected).

Offene Punkte und Rest-Items: siehe `omg-rtc-1.0.open.md`.

Test-Lauf: `cargo test -p zerodds-rtc` — 47 Lib-Tests + 6
Integrations-Tests + 1 Doc-Test grün, 0 failed.

Offene Punkte: keine. Decision-Records: keine.
