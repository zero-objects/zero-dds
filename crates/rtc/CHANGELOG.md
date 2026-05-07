# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-rtc`-Crate.

### Spec-Referenzen

- **OMG RTC 1.0** (`formal/2008-04-04`): §5.2 Lightweight RTC,
  §5.2.1 ReturnCode_t, §5.2.2.2 LightweightRTObject, §5.2.2.3
  LifeCycle, §5.2.2.4 Component-Action-Trait, §5.2.2.5/§5.2.2.6
  ExecutionContext, §5.2.2.7 ExecutionKind, §5.2.2.8
  ExecutionContextHandle_t, §5.3 Execution-Semantics
  (Periodic/Stimulus/Modes), §5.4 Resource Data Model
  (ComponentProfile/PortProfile/ConnectorProfile), §6.3 Local PSM.

### Public-API

**`return_code`-Modul (Spec §5.2.1):**
- `ReturnCode::{Ok, Error, BadParameter, Unsupported, OutOfResources,
  PreconditionNotMet}` — alle 6 Status-Codes.
- `ReturnCode::is_ok()` und `ReturnCode::into_result()` — Ergonomie-
  Helper fuer `?`-Operator.

**`lifecycle`-Modul (Spec §5.2.2.3 + §5.2.2.4 + §5.2.2.7):**
- `LifeCycleState::{Created, Inactive, Active, Error}`.
- `ExecutionKind::{Periodic, EventDriven, Other}`.
- `ComponentAction`-Trait — die 7 Lifecycle-Operations
  (`on_initialize`, `on_finalize`, `on_startup`, `on_shutdown`,
  `on_activated`, `on_deactivated`, `on_aborting`, `on_error`,
  `on_reset`) plus State-Machine-Enforcement.

**`object`-Modul (Spec §5.2.2.2 + §5.2.2.8):**
- `LightweightRtObject` — Komponenten-Modell mit Lifecycle-State und
  ExecutionContext-Liste.
- `ExecutionContextHandle` — Spec §5.2.2.8.

**`execution`-Modul (Spec §5.2.2.5 + §5.2.2.6):**
- `ExecutionContext` + `ExecutionContextOperations`-Trait —
  `start`/`stop`/`reset_component`/`tick` etc.

**`semantics`-Modul (Spec §5.3):**
- `DataFlowComponentAction` — Periodic-Profile.
- `FsmComponentAction` — Stimulus-Response-Profile.
- `MultiModeComponentAction` + `ModeOfOperation` — Modes-Profile.

**`resource`-Modul (Spec §5.4):**
- `Introspection` + `ComponentProfile` + `PortProfile` +
  `ConnectorProfile` + `PortDirection` + `ProfileId` — Resource-Data-
  Model fuer Discovery/Introspection.

### Implementierung

`#![cfg_attr(not(feature = "std"), no_std)]` (default-feature `std`
zieht `alloc` rein); `#![forbid(unsafe_code)]`.

Substrat-Crate: keine Workspace-Deps. Implementiert Local PSM
(Spec §6.3) + Spec §5.2 + §5.3 + §5.4 als reine Datentypen + Traits;
RTC-Container-Hosting ist Caller-Layer.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-A).
- **Dependencies (in):** keine.
- **Dependents (out):** keine produktiven extern (RTC ist Plugin-API
  fuer RTC-Frameworks wie OpenRTM-aist; siehe Audit-File
  `docs/spec-coverage/omg-rtc-1.0.md`).
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- ReturnCode-Werte: durch OMG-Spec §5.2.1 fixiert.
- LifeCycle-State-Machine: durch OMG-Spec §5.2.2.3 fixiert.
- ExecutionKind-Profile: durch OMG-Spec §5.3 fixiert.

### Spec-Coverage-Begrenzungen

Spec §6.4 Lightweight CCM PSM und §6.5 CORBA PSM sind explizit `n/a`
in dieser Crate — sie verlangen eine LwCCM-Container- bzw. CORBA-ORB-
Runtime, die ZeroDDS bewusst nicht selbst hostet. §5.4 Discovery-/
Wire-Aspekt ist partial (Datenmodell vorhanden, Wire-Format nicht).
Begruendung im Audit-File `docs/spec-coverage/omg-rtc-1.0.md`.
