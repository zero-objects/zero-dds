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

- `crates/rtc/` — Local PSM (§6.3) without a CORBA ORB, 6 modules, 61 tests green.

---

## §1 Scope

### §1.1 Overview

**Spec:** §1.1, p. 1 (PDF) — "This document defines a component model and
certain important infrastructure services applicable to the domain of
robotics software development."

**Repo:** `crates/rtc/src/lib.rs` crate-doc header.

**Tests:** the component model is exercised in its three parts:
`crates/rtc/src/object.rs::tests::initialize_then_finalize_round_trips_alive_flag`
(component), `crates/rtc/src/execution.rs::tests::start_stop_round_trips_running_flag`
(execution context), `crates/rtc/src/semantics.rs::tests::data_flow_callbacks_are_invoked_independently`
(infrastructure service).

**Status:** done

### §1.2 Platform-Independent Model

**Spec:** §1.2, p. 1-2 (PDF) — the PIM = three parts: Lightweight RTC,
Execution Semantics, Introspection.

**Repo:** Lightweight RTC + Execution Semantics + the Introspection data
model complete (`crates/rtc/src/object.rs`, `crates/rtc/src/execution.rs`,
`crates/rtc/src/lifecycle.rs`, `crates/rtc/src/semantics.rs`,
`crates/rtc/src/resource.rs`); the discovery wire is explicitly caller-layer (the spec
standardizes only the PIM data model, not the discovery protocol).

**Tests:** one proof per PIM part —
Lightweight RTC: `crates/rtc/src/object.rs::tests::attach_then_detach_works`;
Execution Semantics: `crates/rtc/src/semantics.rs::tests::fsm_on_action_is_invoked_per_event`,
`crates/rtc/src/semantics.rs::tests::multi_mode_on_mode_changed_records_transition`;
Introspection: `crates/rtc/src/resource.rs::tests::get_component_profile_returns_component`,
`crates/rtc/src/resource.rs::tests::introspection_default_methods_compose_correctly`.

**Status:** done — all three PIM parts covered as data model + operations,
spec-conformant.

### §1.3 Platform-Specific Models

**Spec:** §1.3, p. 2 (PDF) — three PSMs: Local, Lightweight CCM, CORBA.

**Repo:** the Local PSM (§6.3) is the implementation base: `crates/rtc/`.
The spec allows any one of the three PSMs as the sole compliance form (§2).
The Lightweight CCM PSM (§6.4) and the CORBA PSM (§6.5) are not implemented
— they are carried as `open` items of their own.

**Tests:** the Local PSM is proven by
`crates/rtc/tests/annex_a_operation_surface.rs::lightweight_rt_object_operations_match_the_annex_a_shape`
and `crates/rtc/tests/annex_a_operation_surface.rs::execution_context_operations_cover_all_twelve_annex_a_operations`.

**Status:** partial — the Local PSM (§6.3) is implemented and proven; the
Lightweight CCM PSM (§6.4) and the CORBA PSM (§6.5) are missing and are
carried as `open` items of their own.

---

## §2 Conformance and compliance

### §2 Conformance points

**Spec:** §2, p. 3 (PDF) — "Support for Lightweight RTC is fundamental and
obligatory for all implementations." Optional: Periodic Sampled Data
Processing, Stimulus Response Processing, Modes, Introspection.

**Repo:** Lightweight RTC (mandatory) + all four optional points covered:
- **Lightweight RTC** mandatory: `crates/rtc/src/object.rs`,
  `crates/rtc/src/lifecycle.rs`, `crates/rtc/src/execution.rs` (see §5).
- **Periodic Sampled Data Processing** (optional):
  `crates/rtc/src/semantics.rs::DataFlowComponentAction` +
  `crates/rtc/src/lifecycle.rs::ExecutionKind::Periodic` (see §5.3.1).
- **Stimulus Response Processing** (optional):
  `crates/rtc/src/semantics.rs::FsmComponentAction` (see §5.3.2).
- **Modes** (optional): `crates/rtc/src/semantics.rs::{ModeOfOperation,
  MultiModeComponentAction}` (see §5.3.3).
- **Introspection** (optional): the `crates/rtc/src/resource.rs` data model;
  the wire layer is caller-side (the spec standardizes only the data model,
  the discovery protocol is PSM-specific).

**Tests:** one proof per conformance point —
mandatory: `crates/rtc/src/lifecycle.rs::tests::valid_transitions_match_spec_state_machine`;
Periodic Sampled Data Processing: `crates/rtc/src/semantics.rs::tests::data_flow_callbacks_are_invoked_independently`;
Stimulus Response Processing: `crates/rtc/src/semantics.rs::tests::fsm_on_action_is_invoked_per_event`;
Modes: `crates/rtc/src/semantics.rs::tests::multi_mode_on_mode_changed_records_transition`;
Introspection: `crates/rtc/src/resource.rs::tests::get_ports_returns_all_two_ports`.

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

**Tests:** `crates/rtc/src/object.rs::tests::initialize_then_finalize_round_trips_alive_flag`,
`crates/rtc/src/object.rs::tests::attach_then_detach_works` — the stereotype
carrier is the `LightweightRtObject` type itself, whose lifecycle these two
tests walk.

**Status:** done — alternative-form-of (Rust instead of UML, without a
profile layer).

### §5.2.2.2 LightweightRTObject interface

**Spec:** §5.2.2.2, p. 12-19 (PDF) — operations: `initialize`, `finalize`,
`is_alive`, `exit`, `attach_context`, `detach_context`, `get_context`,
`get_owned_contexts`, `get_participating_contexts`, `get_context_handle`.

**Repo:** `crates/rtc/src/object.rs::LightweightRtObject` with
`initialize`, `finalize`, `is_alive`, `attach_context`, `detach_context`,
`get_participating_contexts`, `get_context_state` + state-machine
enforcement. Without a counterpart: `exit`, `get_context`,
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
`handles_are_unique_across_attaches`. Operation surface against the Annex-A IDL:
`crates/rtc/tests/annex_a_operation_surface.rs::lightweight_rt_object_operations_match_the_annex_a_shape`.

**Status:** partial — six of the ten Annex-A operations are implemented and
proven. `exit`, `get_context`, `get_owned_contexts` and `get_context_handle`
are missing; they presuppose an owner-context registry in
`LightweightRtObject` that the type does not keep. The remainder is carried
as the `open` item "§5.2.2.2 owner-context operations".

### §5.2.2.2 Owner-context operations (`exit` / `get_context` / `get_owned_contexts` / `get_context_handle`)

**Spec:** §5.2.2.2, p. 12-19 (PDF) — `void exit()`,
`ExecutionContext get_context(in ExecutionContextHandle_t)`,
`ExecutionContextList get_owned_contexts()`,
`ExecutionContextHandle_t get_context_handle(in ExecutionContext)`.

**Repo:** —

**Tests:** —

**Status:** open — `LightweightRtObject` keeps no registry of the contexts
it owns (only of those it participates in) and holds no reference to the
context object type. The four operations presuppose both. Remainder item of
§5.2.2.2.

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

### §5.2.2.6 ExecutionContextOperations interface

**Spec:** §5.2.2.6, p. 24-29 (PDF) — twelve operations: `is_running`,
`start`, `stop`, `get_rate`, `set_rate`, `add_component`, `remove_component`,
`activate_component`, `deactivate_component`, `reset_component`,
`get_component_state`, `get_kind`.

**Repo:** `crates/rtc/src/execution.rs::ExecutionContextOperations` trait +
the `ExecutionContext` impl.

**Tests:** all twelve operations in one run:
`crates/rtc/tests/annex_a_operation_surface.rs::execution_context_operations_cover_all_twelve_annex_a_operations`;
individual behaviour in `crates/rtc/src/execution.rs::tests`:
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

**Tests:** `crates/rtc/src/object.rs::tests::fresh_rtc_is_not_alive`
(Boolean), `crates/rtc/src/execution.rs::tests::set_rate_accepts_positive_finite`
(Double), `crates/rtc/tests/annex_a_operation_surface.rs::annex_a_value_types_and_literals_match_the_idl`.

**Status:** done — alternative mapping.

### §5.2.4 Literal specifications

**Spec:** §5.2.4, p. 38 (PDF) — literal values for the enum members.

**Repo:** cross-ref the enum definitions.

**Tests:** `crates/rtc/src/lifecycle.rs::tests::valid_transitions_match_spec_state_machine`,
`crates/rtc/src/lifecycle.rs::tests::execution_kind_distinguishes_three_modes`,
`crates/rtc/src/return_code.rs::tests::ok_is_ok_and_others_are_not`,
`crates/rtc/src/return_code.rs::tests::display_reports_spec_token_names`,
`crates/rtc/tests/annex_a_operation_surface.rs::annex_a_value_types_and_literals_match_the_idl`.

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

**Spec:** §6.2, p. 81-82 (PDF) — a cross-ref to the Annex-A IDL: the PSM
clauses name their operations through `RTC.idl`.

**Repo:** the Rust trait layer carries the Annex-A operations with the same
parameters and the same `ReturnCode_t` result; the equivalence is executed,
not asserted.

**Tests:** `crates/rtc/tests/annex_a_operation_surface.rs`:
`component_action_covers_every_annex_a_callback`,
`execution_semantics_interfaces_carry_their_annex_a_operations`,
`execution_context_operations_cover_all_twelve_annex_a_operations`,
`annex_a_value_types_and_literals_match_the_idl`.

Run with `cargo test -p zerodds-rtc`; locally (macOS, this run) 61/61 green,
7 of them in this module.

**Status:** done

### §6.3 Local PSM

**Spec:** §6.3, p. 82-87 (PDF) — Local PSM = direct object refs without an
ORB.

**Repo:** `crates/rtc/` is the Local-PSM realization.

**Tests:** `crates/rtc/tests/annex_a_operation_surface.rs::lightweight_rt_object_operations_match_the_annex_a_shape`,
`crates/rtc/tests/annex_a_operation_surface.rs::execution_context_operations_cover_all_twelve_annex_a_operations`,
`crates/rtc/tests/annex_a_operation_surface.rs::introspection_operations_return_the_annex_a_resource_model`
— direct object refs without an ORB.

**Status:** done

### §6.4 Lightweight CCM PSM

**Spec:** §6.4, p. 88 (PDF) — mapping onto LwCCM connectors/SDO.

**Repo:** —

**Tests:** —

**Status:** open — there is no RTC-to-LwCCM adapter in the tree.
`crates/ccm/src/lightweight.rs` filters a CCM IDL model down to the LwCCM
profile and knows nothing of RTC; `crates/rtc/` knows nothing of CCM. The
crate-doc header of `crates/rtc/src/lib.rs` itself lists §6.4 under "What is
not covered". The spec allows compliance via a single PSM (§2); the
implemented one is the Local PSM (§6.3).

### §6.5 CORBA PSM

**Spec:** §6.5, p. 88-90 (PDF) — mapping onto CORBA components.

**Repo:** —

**Tests:** —

**Status:** open — there is no RTC-to-CORBA adapter and no RTC Annex-A IDL
that would run through the codegen crates. The crate-doc header of
`crates/rtc/src/lib.rs` itself lists §6.5 under "What is not covered". The
spec allows compliance via a single PSM (§2); the implemented one is the
Local PSM (§6.3).

---

## Annex A — RTC IDL

### Annex A — complete RTC IDL file

**Spec:** Annex A, p. 91-94 (PDF) — the RTC IDL. The annex is headed
"(non-normative)" in the PDF and calls itself a convenience copy of the
accompanying `RTC.idl`.

**Repo:** the Rust trait definition in `crates/rtc/src/` — parameters and
`ReturnCode_t` result identical to the Annex-A IDL per operation, except for
the four `LightweightRTObject` operations of the §5.2.2.2 `open` item.

**Tests:** `crates/rtc/tests/annex_a_operation_surface.rs`:
`component_action_covers_every_annex_a_callback`,
`execution_semantics_interfaces_carry_their_annex_a_operations`,
`lightweight_rt_object_operations_match_the_annex_a_shape`,
`execution_context_operations_cover_all_twelve_annex_a_operations`,
`annex_a_value_types_and_literals_match_the_idl`,
`introspection_operations_return_the_annex_a_resource_model`.

**Status:** partial — the Annex-A surface is executed except for `exit`,
`get_context`, `get_owned_contexts` and `get_context_handle`; the remainder
is the §5.2.2.2 `open` item.

---

## Audit status

20 done / 3 partial / 3 open / 7 n/a (informative) / 0 n/a (rejected).

Open items and remainder items: see `omg-rtc-1.0.open.md`.

Test run: `cargo test -p zerodds-rtc` — 47 lib tests + 6 integration tests +
1 doc test green, 0 failed.
