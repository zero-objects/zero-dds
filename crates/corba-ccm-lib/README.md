# `zerodds-corba-ccm-lib`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-ccm-lib/badge.svg)](https://docs.rs/zerodds-corba-ccm-lib)

Production-ready CCM-Components fuer ZeroDDS-Hosting: bidirektionale
CCM↔DDS-Bridge, Persistent-State-Storage (CCM 4.0 §10) und
Component-Lifecycle-Telemetrie auf DCPS-Monitor-Topic. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CCM 4.0 | §6 (Component-Model), §10 (Persistent State), §6.10 (Events) |
| OMG DDS 1.4 | §2.2 (DCPS Topic-Mapping) |
| ZeroDDS Monitor | `__ZeroDDS_CcmTelemetry`-Topic |

## Was ist drin

- **`DdsBridgeComponent`** — mappt CCM-EventSinks/Sources auf DDS-
  DataReader/Writer per Topic-Liste.
- **`PersistenceStorageComponent`** — In-Memory Storage-Home (§10).
- **`TelemetryComponent`** — emittiert Lifecycle-Events
  (Activated/Passivated/Removed/ConfigurationCompleted).

## Was nicht abgedeckt ist

- Persistent-Storage mit Disk-Backend: Caller-Layer.
- DDS-Reader/Writer-Instanziierung: ueber `dcps`-Handles in Hosting.

## Beispiel

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
