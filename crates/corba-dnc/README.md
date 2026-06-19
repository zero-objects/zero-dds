# `zerodds-corba-dnc`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-dnc/badge.svg)](https://docs.rs/zerodds-corba-dnc)

OMG Deployment & Configuration 4.0 (`formal/2006-04-02`) — full
D&C stack with plan data model (DPD/CPD/IDD/PSD), XML plan loader (§10),
RepositoryManager (§8), ExecutionManager + NodeManager (§9), and a
ContainerHost bridge to `corba-ccm`. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG D&C 4.0 | §6 Domain Data, §7 Component Data |
| OMG D&C 4.0 | §8 RepositoryManager, §9 Execution/Node Manager |
| OMG D&C 4.0 | §10 XML Encoding |

## What's included

- **`DeploymentPlan`** + 6 more plan-model types.
- **`parse_plan_xml`** — §10 XML loader.
- **`RepositoryManager`** — §8 plan/implementation repository.
- **`ExecutionManager`** + **`DomainApplicationManager`** — §9
  domain-level application layer.
- **`NodeManager`** + **`NodeApplicationManager`** — §9 node level.
- **`ContainerHost`** — binds a `corba-ccm::Container` to a
  plan-application run.

## What's not covered

- ORB wire wiring of the manager operations: caller layer.
- Persistent plan storage: caller layer.

## Example

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
