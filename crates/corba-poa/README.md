# `zerodds-corba-poa`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-poa/badge.svg)](https://docs.rs/zerodds-corba-poa)

OMG CORBA 3.3 Part 1 §11 — Portable Object Adapter (POA). Full
stack with all 7 policies in every mode, POAManager state machine,
POA hierarchy, active object map, ServantManager hooks, and a policy
compatibility validator. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CORBA 3.3 Part 1 | §11 POA, §11.3.4 POAManager, §11.3.5 Operations |
| OMG CORBA 3.3 Part 1 | §11.3.6 policy validation, §11.3.7 policies |
| OMG CORBA 3.3 Part 1 | §11.3.3 Servant, §11.3.5.7-8 ServantManagers |

## What's included

- **`Poa` + `PoaConfig`** — POA instance with hierarchy awareness.
- **`PoaManager`** — 4-state machine (Holding/Active/Discarding/Inactive).
- **`PolicySet`** + 7 policies (Lifespan/IdAssignment/IdUniqueness/
  ImplicitActivation/ServantRetention/RequestProcessing/Thread).
- **`Servant` trait** — `primary_interface` / `primary_repository_id`
  (typed via `corba-ir`) / `is_a` / `is_a_typed` / `invoke`.
- **`ActiveObjectMap`**, **`ObjectId`**, **`ServantActivator`**,
  **`ServantLocator`**.

## What's not covered

- IIOP wire encoding of the POA operations: belongs in `corba-iiop` /
  `corba-giop`.
- ORB singleton lifecycle: lives in the hosting applications
  (`Orb::init/shutdown` is not POA scope).

## Example

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
- [`zerodds-corba-ir`](../corba-ir/README.md) — for typed
  RepositoryId validation.
