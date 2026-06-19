# `zerodds-corba-ccm`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-ccm/badge.svg)](https://docs.rs/zerodds-corba-ccm)

OMG CORBA Component Model 4.0 (CCM, `formal/2006-04-01`) §6 Component
Model + §7 Container Programming Model + §13 Lightweight Profile.
Full stack with CIDL data model, CIF (Component Implementation
Framework), Component/Home model, container runtime, ORB extensions,
Persistent State Service stub, Time PSM, and TimerEventService including
a CosEventService adapter (feature `cos-event`). `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CCM 4.0 | §6 (Component Model), §7 (Container) |
| OMG CCM 4.0 | §13 (Lightweight CCM Profile) |
| OMG CORBA 3.3 Part 3 | §6.13 (CCM conformance), §7 (Generic Interaction), §14 (LwCCM) |
| OMG Time Service 1.1 | §2.2 (TimerEventService) |

## What's included

- **CIDL data model** — `Composition`, `HomeExecutor`, `StorageHome`,
  `StorageType`.
- **CIF** — `ComponentExecutor`, `ExecutorLocator`, `KeyedExecutor`,
  `SessionExecutor`.
- **Component model** — `ComponentDef`, `HomeDef`, `AttributeDef`,
  `FacetDef`, `ReceptacleDef`, `EventSinkDef`, `EventSourceDef`.
- **Container runtime** (feature `std`) — `Container`, `LifecycleState`,
  `TimerEventService`, `pss::*`, `orb_core::Orb`.
- **CosEventService bridge** (feature `cos-event`) —
  `EventChannelTimerCallback`: a `CosEventComm::PushConsumer` is
  wired in as a TimerEventHandler (spec §2.2.4).
- **Conformance markers** — for tooling capability detection.

## What's not covered

- ORB vendor wire stack: lives in `corba-iiop` / `corba-giop`.
- D&C plan loader: belongs in `corba-dnc`.
- Java Bean bridge: belongs in `corba-ccm-ejb`.

## Example

```rust
use zerodds_corba_ccm::conformance::CCM_CONFORMANCE_BASIC_LEVEL;
assert_eq!(CCM_CONFORMANCE_BASIC_LEVEL, "OMG-CCM-4.0:Basic");
```

## Tests

```bash
cargo test -p zerodds-corba-ccm
```

## See also

- [Architecture](../../docs/architecture/02_architecture.md)
- [`zerodds-corba-cos-event`](../corba-cos-event/README.md) —
  Time Service bridge (feature `cos-event`).
- [`zerodds-corba-ccm-lib`](../corba-ccm-lib/README.md) — production-
  ready CCM components.
- [`zerodds-corba-ccm-ejb`](../corba-ccm-ejb/README.md) — JEE bridge.
- [`zerodds-corba-dnc`](../corba-dnc/README.md) — D&C plan hosting.
