# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-corba-poa`-Crate.

### Spec-Referenzen

- **OMG CORBA 3.3 Part 1**: §11 (Portable Object Adapter), §11.3.5
  (POA-Operations), §11.3.6 (Policy-Validierung), §11.3.7 (Policies),
  §11.3.3 (Servant), §11.3.5.7-8 (ServantActivator/ServantLocator).

### Public-API

**`Poa` + `PoaConfig`** (Spec §11.3.5.6 `create_POA`).
**`PoaManager` + `PoaManagerState`** (Spec §11.3.4 — Holding/Active/
Discarding/Inactive State-Machine).
**`PolicySet` + 7 Policies** (Spec §11.3.7):
- `LifespanPolicy::{Transient, Persistent}`
- `IdAssignmentPolicy::{System, User}`
- `IdUniquenessPolicy::{Unique, Multiple}`
- `ImplicitActivationPolicy::{Implicit, NoImplicit}`
- `ServantRetentionPolicy::{Retain, NonRetain}`
- `RequestProcessingPolicy::{UseActiveObjectMap, UseDefaultServant, UseServantManager}`
- `ThreadPolicy::{OrbControl, SingleThread, MainThread}`

**`Servant`-Trait** (Spec §11.3.3 / §11.3.5.20):
- `primary_interface() -> String`
- `primary_repository_id() -> IrResult<RepositoryId>` (typisiert via `corba-ir`).
- `all_interfaces() -> Vec<String>`
- `is_a(&str) -> bool` / `is_a_typed(&RepositoryId) -> bool`
- `invoke(operation, body) -> Vec<u8>`

**`ActiveObjectMap` + `ServantId`** (Spec §11.3.5).
**`ObjectId`** (Spec §11.2.1).
**`ServantActivator` + `ServantLocator` + `ServantLocatorCookie`**
(Spec §11.3.5.7-8 — ServantManager-Hooks; Default-Impls liefern
`Default::default()` zwecks `noop`-Sentinel).
**`PoaError` / `PoaResult`** mit voller Spec-Exception-Surface.

### Implementierung

`#![cfg_attr(not(feature = "std"), no_std)]` mit `extern crate alloc`;
`#![forbid(unsafe_code)]`.

`PolicySet::validate` setzt die in Spec §11.3.6 definierten
Inkompatibilitaeten durch (z.B. `IMPLICIT_ACTIVATION` verlangt
`SYSTEM_ID + RETAIN`).

`Servant::primary_repository_id` bindet typisiert auf
`zerodds-corba-ir::RepositoryId` (Spec §10.7.3.1) — Roundtrip-
Garantie zwischen String- und Strukturform.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-A).
- **Dependencies (in):** `zerodds-corba-ir` (RepositoryId fuer typisierte
  Servant-Validierung).
- **Dependents (out):** `zerodds-corba-iiop`, `zerodds-corba-dds-bridge`
  (POA fuer Servant-Dispatch).
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Trait-Default-Methoden auf `ServantActivator`/`ServantLocator` sind
  noop-Sentinels — Implementer ueberschreiben fuer eigene Policies.
- Spec §11.3.7 Policy-Konstanten sind durch OMG fixiert.
