# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-rtc` crate.

### Spec-Referenzen

- **OMG RTC 1.0** (`formal/2008-04-04`): §5.2 Lightweight RTC,
  §5.2.1 ReturnCode_t, §5.2.2.2 LightweightRTObject, §5.2.2.3
  LifeCycle, §5.2.2.4 Component-Action-Trait, §5.2.2.5/§5.2.2.6
  ExecutionContext, §5.2.2.7 ExecutionKind, §5.2.2.8
  ExecutionContextHandle_t, §5.3 Execution-Semantics
  (Periodic/Stimulus/Modes), §5.4 Resource Data Model
  (ComponentProfile/PortProfile/ConnectorProfile), §6.3 Local PSM.

### Public-API

**`return_code` module (spec §5.2.1):**
- `ReturnCode::{Ok, Error, BadParameter, Unsupported, OutOfResources,
  PreconditionNotMet}` — all 6 status codes.
- `ReturnCode::is_ok()` and `ReturnCode::into_result()` — ergonomic
  helper for the `?` operator.

**`lifecycle` module (spec §5.2.2.3 + §5.2.2.4 + §5.2.2.7):**
- `LifeCycleState::{Created, Inactive, Active, Error}`.
- `ExecutionKind::{Periodic, EventDriven, Other}`.
- `ComponentAction` trait — the 7 lifecycle operations
  (`on_initialize`, `on_finalize`, `on_startup`, `on_shutdown`,
  `on_activated`, `on_deactivated`, `on_aborting`, `on_error`,
  `on_reset`) plus state-machine enforcement.

**`object` module (spec §5.2.2.2 + §5.2.2.8):**
- `LightweightRtObject` — component model with lifecycle state and
  ExecutionContext list.
- `ExecutionContextHandle` — Spec §5.2.2.8.

**`execution` module (spec §5.2.2.5 + §5.2.2.6):**
- `ExecutionContext` + `ExecutionContextOperations`-Trait —
  `start`/`stop`/`reset_component`/`tick` etc.

**`semantics` module (spec §5.3):**
- `DataFlowComponentAction` — Periodic-Profile.
- `FsmComponentAction` — Stimulus-Response-Profile.
- `MultiModeComponentAction` + `ModeOfOperation` — Modes-Profile.

**`resource` module (spec §5.4):**
- `Introspection` + `ComponentProfile` + `PortProfile` +
  `ConnectorProfile` + `PortDirection` + `ProfileId` — resource data
  model for discovery/introspection.

### Implementation

`#![cfg_attr(not(feature = "std"), no_std)]` (the default feature `std`
pulls in `alloc`); `#![forbid(unsafe_code)]`.

Substrate crate: no workspace deps. Implements the Local PSM
(spec §6.3) + spec §5.2 + §5.3 + §5.4 as pure data types + traits;
RTC container hosting is the caller layer.

### Architecture

- **Layer:** 8 (CORBA stack, tier A).
- **Dependencies (in):** none.
- **Dependents (out):** no external production ones (RTC is a plugin API
  for RTC frameworks like OpenRTM-aist; see the audit file
  `docs/spec-coverage/omg-rtc-1.0.md`).
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- ReturnCode values: fixed by OMG spec §5.2.1.
- LifeCycle state machine: fixed by OMG spec §5.2.2.3.
- ExecutionKind profiles: fixed by OMG spec §5.3.

### Spec coverage limitations

Spec §6.4 Lightweight CCM PSM and §6.5 CORBA PSM are explicitly `n/a`
in this crate — they require an LwCCM container resp. CORBA ORB
runtime, which ZeroDDS deliberately does not host itself. The §5.4
discovery/wire aspect is partial (data model present, wire format not).
Rationale in the audit file `docs/spec-coverage/omg-rtc-1.0.md`.
