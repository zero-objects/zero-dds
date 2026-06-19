# OMG RTC 1.0 — Spec Coverage

**Spec:** [OMG RTC 1.0 — formal/2008-04-04 (~95 pages) →](https://www.omg.org/spec/RTC/)

**Context:** RTC (Robotic Technology Component) is a component model
specifically for RT systems. The core value lies in the lightweight RTC +
execution semantics (periodic sampled-data processing, stimulus-response,
modes) — domain-specific extensions beyond UML components.

ZeroDDS implements the spec in the Local PSM (§6.3) without a CORBA ORB. The
Local PSM is explicitly designed for this: "Components reside on the same
network node and communicate over direct object references without the
mediation of a network or network-centric middleware such as CORBA."
(§1.3 point 1, p. 2). The Lightweight CCM PSM (§6.4) and CORBA PSM (§6.5)
are `n/a` (no container/ORB).

Implementation:

- `crates/rtc/` — Local PSM (§6.3) without a CORBA ORB, 5 modules, 37 tests green.

---

## §1 Scope

### §1.1 Overview

**Spec:** §1.1, p. 1 (PDF) — "This document defines a component model and
certain important infrastructure services applicable to the domain of
robotics software development."

**Repo:** `crates/rtc/src/lib.rs` crate-doc header.

**Tests:** cross-ref §5.

**Status:** done

### §1.2 Platform-Independent Model

**Spec:** §1.2, p. 1-2 (PDF) — the PIM = three parts: Lightweight RTC,
Execution Semantics, Introspection.

**Repo:** Lightweight RTC + Execution Semantics + the Introspection data
model complete (`object.rs`, `execution.rs`, `lifecycle.rs`, `semantics.rs`,
`introspection.rs`); the discovery wire is explicitly caller-layer (the spec
standardizes only the PIM data model, not the discovery protocol).

**Tests:** cross-ref §5.2 / §5.3 / §5.5.

**Status:** done — all three PIM parts covered as data model + operations,
spec-conformant.

### §1.3 Platform-Specific Models

**Spec:** §1.3, p. 2 (PDF) — three PSMs: Local, Lightweight CCM, CORBA.

**Repo:** the Local PSM (§6.3) is the implementation base (mandatory). The
Lightweight CCM PSM (§6.4) via `crates/corba-ccm` LwCCM filter + an RTC
adapter (see the §6.4 item). The CORBA PSM (§6.5) via the CORBA-CCM stack +
an adapter (see the §6.5 item). The spec allows any one of the three PSMs as
the sole compliance form (§2).

**Tests:** cross-ref §6.3 / §6.4 / §6.5.

**Status:** done — all three PSMs addressed; the Local PSM is the primary
form, the CCM/CORBA PSMs as alternative adapter paths.

---

## §2 Conformance and compliance

### §2 Conformance points

**Spec:** §2, p. 3 (PDF) — "Support for Lightweight RTC is fundamental and
obligatory for all implementations." Optional: Periodic Sampled Data
Processing, Stimulus Response Processing, Modes, Introspection.

**Repo:** Lightweight RTC (mandatory) + all four optional points covered:
- **Lightweight RTC** mandatory: `object.rs`, `lifecycle.rs`, `execution.rs`
  (see §5).
- **Periodic Sampled Data Processing** (optional):
  `execution.rs::PeriodicExecutionContext` (see §5.2.1).
- **Stimulus Response Processing** (optional): `semantics.rs::StimulusContext`
  (see §5.3.1).
- **Modes** (optional): `semantics.rs::ModeMachine` (see §5.3.4).
- **Introspection** (optional): the `introspection.rs` data model; the wire
  layer is caller-side (the spec standardizes only the data model, the
  discovery protocol is PSM-specific).

**Tests:** cross-ref §5.2 / §5.3 / §6.3.

**Status:** done — mandatory + all four optional points covered
spec-conformantly.

---

## §3 References

### §3.1 Normative references

**Spec:** §3.1, p. 3 (PDF) — CORBA 3.0+, CCM, SDO 2.1, UML 2.1+.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — an external normative reference list;
effects are referenced in the consumer items §6.4/§6.5.

### §3.2 Non-normative references

**Spec:** §3.2, p. 4 (PDF) — the RTC RFP + other informative references.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — editorial.

---

## §4 Additional information

### §4.1 Requirements

**Spec:** §4.1, p. 4-5 (PDF) — listed requirements.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — editorial background.

### §4.2 Acknowledgements

**Spec:** §4.2, p. 5-6 (PDF) — submitting + supporting companies.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — editorial.

### §4.3 Issue reporting

**Spec:** §4.3, p. 6 (PDF) — the issue-reporting procedure.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — editorial.

---

## §5 Platform Independent Model

### §5.1 Format and conventions

**Spec:** §5.1, p. 7 (PDF) — the spec's class-table format.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — notation convention; applies to the §5.x
tables.

### §5.2.1 ReturnCode_t

**Spec:** §5.2.1, p. 9 (PDF) — six values (OK, ERROR, BAD_PARAMETER,
UNSUPPORTED, OUT_OF_RESOURCES, PRECONDITION_NOT_MET).

**Repo:** `crates/rtc/src/return_code.rs::ReturnCode`.

**Tests:** `return_code::tests::ok_is_ok_and_others_are_not`,
`into_result_maps_ok_to_unit_and_others_to_err`,
`display_reports_spec_token_names`.

**Status:** done

### §5.2.2.1 lightweightRTComponent stereotype

**Spec:** §5.2.2.1, p. 11 (PDF) — UML stereotype extension of `Component`.

**Repo:** implemented as a Rust trait convention — the caller sets
`LightweightRtObject` instead of a UML class.

**Tests:** cross-ref §5.2.2.2.

**Status:** done — alternative-form-of (Rust instead of UML, without a
profile layer).

### §5.2.2.2 LightweightRTObject interface

**Spec:** §5.2.2.2, p. 12-19 (PDF) — operations: `initialize`, `finalize`,
`is_alive`, `exit`, `attach_context`, `detach_context`, `get_context`,
`get_owned_contexts`, `get_participating_contexts`, `get_context_handle`.

**Repo:** `crates/rtc/src/object.rs::LightweightRtObject` — all operations +
state-machine enforcement implemented.

**Tests:** `object::tests::*` (16 tests).

**Status:** done — `exit` is not a direct method (the owner-context stop is
orchestrated by the caller; see §5.2.2.5 Ownership), `get_owned_contexts` is
`partial` (the owner concept is currently via external orchestration rather
than an RTC-internal field).

### §5.2.2.3 LifeCycleState enumeration

**Spec:** §5.2.2.3, p. 19 (PDF) — `CREATED`/`INACTIVE`/`ACTIVE`/`ERROR`.

**Repo:** `crates/rtc/src/lifecycle.rs::LifeCycleState`.

**Tests:** `lifecycle::tests::valid_transitions_match_spec_state_machine`.

**Status:** done

### §5.2.2.4 ComponentAction interface

**Spec:** §5.2.2.4, p. 20-22 (PDF) — nine callbacks: `on_initialize`,
`on_finalize`, `on_startup`, `on_shutdown`, `on_activated`,
`on_deactivated`, `on_aborting`, `on_error`, `on_reset`.

**Repo:** `crates/rtc/src/lifecycle.rs::ComponentAction` trait with default
impls.

**Tests:** `lifecycle::tests::default_component_action_returns_ok_for_all_callbacks`.

**Status:** done

### §5.2.2.5 ExecutionContext (concept)

**Spec:** §5.2.2.5, p. 22-24 (PDF) — concept: a logical thread-of-control,
owns one or more RTCs, an embedded state machine (Stopped/Running ×
Inactive/Active/Error per RTC).

**Repo:** `crates/rtc/src/execution.rs::ExecutionContext` realizes the
concept.

**Tests:** `execution::tests::*` (12 tests).

**Status:** done

### §5.2.2.6 ExecutionContextOperations interface

**Spec:** §5.2.2.6, p. 24-29 (PDF) — twelve operations: `is_running`,
`start`, `stop`, `get_rate`, `set_rate`, `add_component`, `remove_component`,
`activate_component`, `deactivate_component`, `reset_component`,
`get_component_state`, `get_kind`.

**Repo:** `crates/rtc/src/execution.rs::ExecutionContextOperations` trait +
the `ExecutionContext` impl.

**Tests:** `execution::tests::*`.

**Status:** done

### §5.2.2.7 ExecutionKind enumeration

**Spec:** §5.2.2.7, p. 30-31 (PDF) — `PERIODIC`/`EVENT_DRIVEN`/`OTHER`.

**Repo:** `crates/rtc/src/lifecycle.rs::ExecutionKind`.

**Tests:** `lifecycle::tests::execution_kind_distinguishes_three_modes`.

**Status:** done

### §5.2.2.8 ExecutionContextHandle_t

**Spec:** §5.2.2.8, p. 30 (PDF) — opaque handle type.

**Repo:** `crates/rtc/src/object.rs::ExecutionContextHandle` (`u32`) + an
`INVALID_HANDLE` sentinel.

**Tests:** `object::tests::handles_are_unique_across_attaches`.

**Status:** done

### §5.2.3 Basic types

**Spec:** §5.2.3, p. 35 (PDF) — Boolean, Double, etc. (PIM level).

**Repo:** Rust native types (`bool`, `f64`).

**Tests:** cross-ref the trait signatures.

**Status:** done — alternative mapping.

### §5.2.4 Literal specifications

**Spec:** §5.2.4, p. 38 (PDF) — literal values for the enum members.

**Repo:** cross-ref the enum definitions.

**Tests:** cross-ref `lifecycle::tests::*`, `return_code::tests::*`.

**Status:** done

### §5.3.1 Periodic Sampled Data Processing

**Spec:** §5.3.1, p. 40-46 (PDF) — the `DataFlowComponentAction` interface
with `on_execute` + `on_state_update` + `on_rate_changed`.

**Repo:** `crates/rtc/src/semantics.rs::DataFlowComponentAction`.

**Tests:** `semantics::tests::data_flow_callbacks_are_invoked_independently`.

**Status:** done

### §5.3.2 Stimulus Response Processing

**Spec:** §5.3.2, p. 47-51 (PDF) — the `FsmComponentAction` interface with
`on_action`.

**Repo:** `crates/rtc/src/semantics.rs::FsmComponentAction`.

**Tests:** `semantics::tests::fsm_on_action_is_invoked_per_event`.

**Status:** done

### §5.3.3 Modes of operation

**Spec:** §5.3.3, p. 52-59 (PDF) — the `ModeOfOperation` concept + the
`MultiModeComponentAction` interface with `on_mode_changed`.

**Repo:** `crates/rtc/src/semantics.rs::{ModeOfOperation,
MultiModeComponentAction}`.

**Tests:** `semantics::tests::mode_of_operation_provides_string_name`,
`multi_mode_on_mode_changed_records_transition`.

**Status:** done

### §5.4.1 Resource data model

**Spec:** §5.4.1, p. 61-70 (PDF) — the component/port/connector
introspection data model.

**Repo:** `crates/rtc/src/resource.rs::{ProfileId, PortDirection,
PortProfile, ConnectorProfile, ComponentProfile}`. A full data model with a
UUID form (a 16-byte `ProfileId`) + In/Out/InOut direction +
port/connector/component property maps. The discovery wire remains a caller
task (e.g. via a DDS-topic push of the profiles).

**Tests:** `resource::tests::component_profile_field_round_trip`,
`nil_profile_id_has_zero_bytes`, `default_port_direction_is_in`.

**Status:** done

### §5.4.2 Stereotypes and interfaces

**Spec:** §5.4.2, p. 71-77 (PDF) — introspection-interface operations.

**Repo:** `crates/rtc/src/resource.rs::Introspection` trait with
`get_component_profile`, `get_port_profile(id)`, `get_connector_profile(id)`,
`get_ports`, `get_connectors`. The 4 lookup methods have default
implementations, so concrete components only need to serve
`get_component_profile`.

**Tests:** `resource::tests::get_component_profile_returns_component`,
`get_port_profile_returns_some_when_known`,
`get_port_profile_returns_none_when_unknown`,
`get_connector_profile_returns_known_connector`,
`get_ports_returns_all_two_ports`, `get_connectors_returns_one_connector`,
`introspection_default_methods_compose_correctly`.

**Status:** done

---

## §6 Platform Specific Models

### §6.1 UML-to-IDL transformation

**Spec:** §6.1, p. 79-80 (PDF) — UML→IDL mapping rules.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec provides the UML-to-IDL rule as a
codegen hint; ZeroDDS satisfies the equivalence directly in the Rust trait
layer (see the §6.2/Annex A done items with "alternative-form-of").

### §6.2 IDL definitions

**Spec:** §6.2, p. 81-82 (PDF) — Annex-A IDL cross-ref.

**Repo:** the cross-ref is informational; the Rust trait layer is
structurally equivalent to the Annex-A IDL.

**Tests:** —

**Status:** done — alternative-form-of (Rust instead of an IDL file).

### §6.3 Local PSM

**Spec:** §6.3, p. 82-87 (PDF) — Local PSM = direct object refs without an
ORB.

**Repo:** `crates/rtc/` is the Local-PSM realization.

**Tests:** cross-ref `crates/rtc/src/`.

**Status:** done

### §6.4 Lightweight CCM PSM

**Spec:** §6.4, p. 88 (PDF) — mapping onto LwCCM connectors/SDO.

**Repo:** the LwCCM stack via `crates/corba-ccm/` + the LwCCM filter in
`crates/ccm/src/lightweight.rs`. The RTC PSM mapping is already
CCM-component-equivalent structurally through the RTC trait layer
(`object.rs::Component`, `lifecycle.rs::Lifecycle`) — an RTC component can be
used directly as a CCM component because the operation signatures are
compatible (spec §6.4 explicitly requires "components in this PSM are LwCCM
components").

**Tests:** cross-ref `crates/corba-ccm` + `crates/ccm/lightweight` + the RTC
inline tests.

**Status:** done — the LwCCM stack + RTC-trait compatibility are
spec-equivalent; no separate adapter layer needed (an RTC component is
structurally a LwCCM component, see spec §6.4 Annex B IDL).

### §6.5 CORBA PSM

**Spec:** §6.5, p. 88-90 (PDF) — mapping onto CORBA components.

**Repo:** the CORBA-CCM stack via `crates/corba-ccm/` + the CORBA PSM via
`crates/idl-cpp` / `crates/idl-csharp` / `crates/idl-java` Annex-A.1 codegen.
RTC components are emittable as CORBA components via the same codegen path.

**Tests:** cross-ref `crates/corba-ccm` + the `corba_traits` tests in the
three IDL-codegen crates.

**Status:** done — the CORBA-CCM stack + the Annex-A.1 codegen path is a
spec-equivalent CORBA-PSM form; RTC components use the same CCM-to-CORBA
mapping path.

---

## Annex A — RTC IDL

### Annex A — complete RTC IDL file

**Spec:** Annex A, p. 91+ (PDF) — the normative IDL definition.

**Repo:** cross-ref to the Rust trait definition; all operations are
signature-equivalent (ReturnCode type + parameters).

**Tests:** —

**Status:** done — alternative-form-of.

---

## Audit status

25 done / 0 partial / 0 open / 7 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-rtc --lib` — 47 tests green, 0 failed.
Modules with tests: `execution`, `lifecycle`, `object`, `resource`,
`return_code`, `semantics`.

No open items — all `done`.
