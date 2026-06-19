# `zerodds-corba-ccm-lib`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-ccm-lib/badge.svg)](https://docs.rs/zerodds-corba-ccm-lib)

Production-ready CCM components for ZeroDDS hosting: bidirectional
CCM↔DDS bridge, persistent state storage (CCM 4.0 §10), and
component lifecycle telemetry on a DCPS monitor topic. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CCM 4.0 | §6 (Component Model), §10 (Persistent State), §6.10 (Events) |
| OMG DDS 1.4 | §2.2 (DCPS topic mapping) |
| ZeroDDS Monitor | `__ZeroDDS_CcmTelemetry` topic |

## What's included

- **`DdsBridgeComponent`** — maps CCM EventSinks/Sources onto DDS
  DataReader/Writer via a topic list.
- **`PersistenceStorageComponent`** — in-memory storage home (§10).
- **`TelemetryComponent`** — emits lifecycle events
  (Activated/Passivated/Removed/ConfigurationCompleted).

## What is not covered

- Persistent storage with a disk backend: caller layer.
- DDS reader/writer instantiation: via `dcps` handles in the host.

## Example

```rust
use zerodds_corba_ccm_lib::{MappingDirection, TopicMapping};
let m = TopicMapping {
    port_name: "in".into(),
    topic_name: "Sensor".into(),
    type_name: "sensors::Tick".into(),
    direction: MappingDirection::SinkSubscribesTopic,
};
assert_eq!(m.direction, MappingDirection::SinkSubscribesTopic);
```

## Tests

```bash
cargo test -p zerodds-corba-ccm-lib
```

## See also

- [`zerodds-corba-ccm`](../corba-ccm/README.md) — Container-Runtime + CIF.
- [Architecture](../../docs/architecture/02_architecture.md)
