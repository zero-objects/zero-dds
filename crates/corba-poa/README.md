# `zerodds-corba-poa`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-poa/badge.svg)](https://docs.rs/zerodds-corba-poa)

OMG CORBA 3.3 Part 1 §11 — Portable Object Adapter (POA). Voller
Stack mit allen 7 Policies in allen Modi, POAManager-State-Machine,
POA-Hierarchie, Active-Object-Map, ServantManager-Hooks und Policy-
Compatibility-Validator. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CORBA 3.3 Part 1 | §11 POA, §11.3.4 POAManager, §11.3.5 Operations |
| OMG CORBA 3.3 Part 1 | §11.3.6 Policy-Validierung, §11.3.7 Policies |
| OMG CORBA 3.3 Part 1 | §11.3.3 Servant, §11.3.5.7-8 ServantManagers |

## Was ist drin

- **`Poa` + `PoaConfig`** — POA-Instanz mit Hierarchie-Awareness.
- **`PoaManager`** — 4-State-Machine (Holding/Active/Discarding/Inactive).
- **`PolicySet`** + 7 Policies (Lifespan/IdAssignment/IdUniqueness/
  ImplicitActivation/ServantRetention/RequestProcessing/Thread).
- **`Servant`-Trait** — `primary_interface` / `primary_repository_id`
  (typisiert via `corba-ir`) / `is_a` / `is_a_typed` / `invoke`.
- **`ActiveObjectMap`**, **`ObjectId`**, **`ServantActivator`**,
  **`ServantLocator`**.

## Was nicht abgedeckt ist

- IIOP-Wire-Encoding der POA-Operations: gehoert in `corba-iiop` /
  `corba-giop`.
- ORB-Singleton-Lifecycle: liegt in den hosting Anwendungen
  (`Orb::init/shutdown` ist nicht POA-Scope).

## Beispiel

```rust
use zerodds_corba_poa::policies::PolicySet;
let policies = PolicySet::default();
assert!(policies.validate().is_ok());
```

## Tests

```bash
cargo test -p zerodds-corba-poa
```

## See also

- [Architecture](../../docs/architecture/02_architecture.md)
- [`zerodds-corba-ir`](../corba-ir/README.md) — fuer typisierte
  RepositoryId-Validierung.
