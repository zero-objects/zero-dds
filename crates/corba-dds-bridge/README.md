# `zerodds-corba-dds-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-dds-bridge/badge.svg)](https://docs.rs/zerodds-corba-dds-bridge)

Bidirectional CORBA-Object ↔ DDS-Topic bridge: GIOP request → DDS
sample (servant mode) and DDS sample → GIOP request (forwarder mode).
Many-to-many `BridgeMapping` with `BridgeServant` + `LifecycleSync` and
wire helpers to `corba-giop` + `corba-ior`. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG CORBA 3.3 Part 1 | §11 POA servant (bridge path) |
| OMG CORBA 3.3 Part 2 | §15 GIOP request, §13.6 IOR object_key |
| OMG DDS 1.4 | §2.2.2.2.1 register/unregister_instance |

## What's included

- **`BridgeMapping`** + **`BridgeRoute`** — many-to-many CORBA-Object
  ↔ DDS-Topic.
- **`Direction`** — `CorbaToDds` / `DdsToCorba` / `Bidirectional`.
- **`OperationMapping`** + **`TopicQosRef`**.
- **`BridgeServant`** — a `corba-poa::Servant`-compatible
  bridge servant.
- **`LifecycleSync`** + **`LifecycleEvent`** — CORBA activate ↔
  DDS register_instance.
- **`wire::decode_giop_request_bytes`** + **`wire::object_key_from_ior`**
  — cross-crate hooks to `corba-giop` + `corba-ior`.

## What's not covered

- DDS reader/writer instantiation: resolved at the caller layer via
  the `DdsPublishSink` trait (against `dcps`).
- Wire encoding of the reply sides: caller-layer.

## Example

```rust
use zerodds_corba_dds_bridge::Direction;
assert_ne!(Direction::CorbaToDds, Direction::DdsToCorba);
```

## Tests

```bash
cargo test -p zerodds-corba-dds-bridge
```

## See also

- [`zerodds-corba-poa`](../corba-poa/README.md) — Servant trait.
- [`zerodds-corba-giop`](../corba-giop/README.md) — GIOP request.
- [`zerodds-corba-ior`](../corba-ior/README.md) — IOR / object_key.
- [Architecture](../../docs/architecture/02_architecture.md)
