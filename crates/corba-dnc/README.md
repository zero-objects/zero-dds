# `zerodds-corba-dnc`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-dnc/badge.svg)](https://docs.rs/zerodds-corba-dnc)

OMG Deployment & Configuration 4.0 (`formal/2006-04-02`) — voller
D&C-Stack mit Plan-Datenmodell (DPD/CPD/IDD/PSD), XML-Plan-Loader (§10),
RepositoryManager (§8), ExecutionManager + NodeManager (§9) und
ContainerHost-Bridge zu `corba-ccm`. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG D&C 4.0 | §6 Domain Data, §7 Component Data |
| OMG D&C 4.0 | §8 RepositoryManager, §9 Execution/Node-Manager |
| OMG D&C 4.0 | §10 XML-Encoding |

## Was ist drin

- **`DeploymentPlan`** + 6 weitere Plan-Modell-Typen.
- **`parse_plan_xml`** — §10-XML-Loader.
- **`RepositoryManager`** — §8 Plan/Implementation-Repository.
- **`ExecutionManager`** + **`DomainApplicationManager`** — §9
  Domain-Level-Application-Layer.
- **`NodeManager`** + **`NodeApplicationManager`** — §9 Node-Level.
- **`ContainerHost`** — bindet einen `corba-ccm::Container` an einen
  Plan-Application-Run.

## Was nicht abgedeckt ist

- ORB-Wire-Anbindung der Manager-Operationen: Caller-Layer.
- Persistente Plan-Storage: Caller-Layer.

## Beispiel

```rust
use zerodds_corba_dnc::DeploymentPlan;
let plan = DeploymentPlan::default();
assert!(plan.uuid.is_empty());
```

## Tests

```bash
cargo test -p zerodds-corba-dnc
```

## See also

- [`zerodds-corba-ccm`](../corba-ccm/README.md) — Container-Runtime.
- [Architecture](../../docs/architecture/02_architecture.md)
