# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-corba-poa` crate.

### Spec references

- **OMG CORBA 3.3 Part 1**: §11 (Portable Object Adapter), §11.3.5
  (POA operations), §11.3.6 (policy validation), §11.3.7 (policies),
  §11.3.3 (servant), §11.3.5.7-8 (ServantActivator/ServantLocator).

### Public API

**`Poa` + `PoaConfig`** (Spec §11.3.5.6 `create_POA`).
**`PoaManager` + `PoaManagerState`** (Spec §11.3.4 — Holding/Active/
Discarding/Inactive state machine).
**`PolicySet` + 7 policies** (Spec §11.3.7):
- `LifespanPolicy::{Transient, Persistent}`
- `IdAssignmentPolicy::{System, User}`
- `IdUniquenessPolicy::{Unique, Multiple}`
- `ImplicitActivationPolicy::{Implicit, NoImplicit}`
- `ServantRetentionPolicy::{Retain, NonRetain}`
- `RequestProcessingPolicy::{UseActiveObjectMap, UseDefaultServant, UseServantManager}`
- `ThreadPolicy::{OrbControl, SingleThread, MainThread}`

**`Servant` trait** (Spec §11.3.3 / §11.3.5.20):
- `primary_interface() -> String`
- `primary_repository_id() -> IrResult<RepositoryId>` (typed via `corba-ir`).
- `all_interfaces() -> Vec<String>`
- `is_a(&str) -> bool` / `is_a_typed(&RepositoryId) -> bool`
- `invoke(operation, body) -> Vec<u8>`

**`ActiveObjectMap` + `ServantId`** (Spec §11.3.5).
**`ObjectId`** (Spec §11.2.1).
**`ServantActivator` + `ServantLocator` + `ServantLocatorCookie`**
(Spec §11.3.5.7-8 — ServantManager hooks; default impls return
`Default::default()` as a `noop` sentinel).
**`PoaError` / `PoaResult`** with the full spec exception surface.

### Implementation

`#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc`;
`#![forbid(unsafe_code)]`.

`PolicySet::validate` enforces the incompatibilities defined in
Spec §11.3.6 (e.g. `IMPLICIT_ACTIVATION` requires
`SYSTEM_ID + RETAIN`).

`Servant::primary_repository_id` binds in a typed manner to
`zerodds-corba-ir::RepositoryId` (Spec §10.7.3.1) — roundtrip
guarantee between the string and struct forms.

### Architecture

- **Layer:** 8 (CORBA stack, Tier-A).
- **Dependencies (in):** `zerodds-corba-ir` (RepositoryId for typed
  servant validation).
- **Dependents (out):** `zerodds-corba-iiop`, `zerodds-corba-dds-bridge`
  (POA for servant dispatch).
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Trait default methods on `ServantActivator`/`ServantLocator` are
  noop sentinels — implementers override them for their own policies.
- Spec §11.3.7 policy constants are fixed by OMG.
