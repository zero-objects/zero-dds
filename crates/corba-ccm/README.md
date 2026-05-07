# `zerodds-corba-ccm`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-ccm/badge.svg)](https://docs.rs/zerodds-corba-ccm)

OMG CORBA Component Model 4.0 (CCM, `formal/2006-04-01`) §6 Komponenten-
Modell + §7 Container-Programmiermodell + §13 Lightweight-Profil.
Voller Stack mit CIDL-Datenmodell, CIF (Component Implementation
Framework), Component-/Home-Modell, Container-Runtime, ORB-Extensions,
Persistent-State-Service-Stub, Time-PSM und TimerEventService inkl.
CosEventService-Adapter (Feature `cos-event`). `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CCM 4.0 | §6 (Component Model), §7 (Container) |
| OMG CCM 4.0 | §13 (Lightweight CCM Profile) |
| OMG CORBA 3.3 Part 3 | §6.13 (CCM-Conformance), §7 (Generic-Interaction), §14 (LwCCM) |
| OMG Time Service 1.1 | §2.2 (TimerEventService) |

## Was ist drin

- **CIDL-Datenmodell** — `Composition`, `HomeExecutor`, `StorageHome`,
  `StorageType`.
- **CIF** — `ComponentExecutor`, `ExecutorLocator`, `KeyedExecutor`,
  `SessionExecutor`.
- **Component-Modell** — `ComponentDef`, `HomeDef`, `AttributeDef`,
  `FacetDef`, `ReceptacleDef`, `EventSinkDef`, `EventSourceDef`.
- **Container-Runtime** (Feature `std`) — `Container`, `LifecycleState`,
  `TimerEventService`, `pss::*`, `orb_core::Orb`.
- **CosEventService-Bridge** (Feature `cos-event`) —
  `EventChannelTimerCallback`: ein `CosEventComm::PushConsumer` wird
  als TimerEventHandler verdrahtet (Spec §2.2.4).
- **Conformance-Markers** — fuer Tooling-Capability-Detection.

## Was nicht abgedeckt ist

- ORB-Vendor-Wire-Stack: liegt bei `corba-iiop` / `corba-giop`.
- D&C-Plan-Loader: gehoert in `corba-dnc`.
- Java-Bean-Bridge: gehoert in `corba-ccm-ejb`.

## Beispiel

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
  Time-Service-Bridge (Feature `cos-event`).
- [`zerodds-corba-ccm-lib`](../corba-ccm-lib/README.md) — Production-
  ready CCM-Components.
- [`zerodds-corba-ccm-ejb`](../corba-ccm-ejb/README.md) — JEE-Bridge.
- [`zerodds-corba-dnc`](../corba-dnc/README.md) — D&C-Plan-Hosting.
