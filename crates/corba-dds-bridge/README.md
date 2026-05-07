# `zerodds-corba-dds-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-dds-bridge/badge.svg)](https://docs.rs/zerodds-corba-dds-bridge)

Bidirektionale CORBA-Object ↔ DDS-Topic-Bridge: GIOP-Request → DDS-
Sample (Servant-Modus) und DDS-Sample → GIOP-Request (Forwarder-Modus).
Many-to-Many `BridgeMapping` mit `BridgeServant` + `LifecycleSync` und
Wire-Helpers zu `corba-giop` + `corba-ior`. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CORBA 3.3 Part 1 | §11 POA-Servant (Bridge-Pfad) |
| OMG CORBA 3.3 Part 2 | §15 GIOP-Request, §13.6 IOR-object_key |
| OMG DDS 1.4 | §2.2.2.2.1 register/unregister_instance |

## Was ist drin

- **`BridgeMapping`** + **`BridgeRoute`** — Many-to-Many CORBA-Object
  ↔ DDS-Topic.
- **`Direction`** — `CorbaToDds` / `DdsToCorba` / `Bidirectional`.
- **`OperationMapping`** + **`TopicQosRef`**.
- **`BridgeServant`** — `corba-poa::Servant`-kompatibler
  Bridge-Servant.
- **`LifecycleSync`** + **`LifecycleEvent`** — CORBA-Activate ↔
  DDS-RegisterInstance.
- **`wire::decode_giop_request_bytes`** + **`wire::object_key_from_ior`**
  — Cross-Crate-Hooks zu `corba-giop` + `corba-ior`.

## Was nicht abgedeckt ist

- DDS-Reader/Writer-Instanziierung: ueber `DdsPublishSink`-Trait
  Caller-Layer-resolved (gegen `dcps`).
- Wire-Encoding der Reply-Sides: Caller-Layer.

## Beispiel

```rust
use zerodds_corba_dds_bridge::Direction;
assert_ne!(Direction::CorbaToDds, Direction::DdsToCorba);
```

## Tests

```bash
cargo test -p zerodds-corba-dds-bridge
```

## See also

- [`zerodds-corba-poa`](../corba-poa/README.md) — Servant-Trait.
- [`zerodds-corba-giop`](../corba-giop/README.md) — GIOP-Request.
- [`zerodds-corba-ior`](../corba-ior/README.md) — IOR / object_key.
- [Architecture](../../docs/architecture/02_architecture.md)
