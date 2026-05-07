# `zerodds-zenoh-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-zenoh-bridge/badge.svg)](https://docs.rs/zerodds-zenoh-bridge)

Bidirektionale Bridge zwischen ZeroDDS-DCPS und Eclipse-Zenoh.
Pure-Rust-Mapping-Layer (Topic-Namen ↔ Zenoh-Key-Expressions, QoS-
Translation) ohne externe Deps; optionaler voll-funktionaler Live-
Bridge-Pfad ueber Feature `zenoh-runtime`. Safety classification:
**STANDARD**.

## Architektur

```text
 DDS-Topic  ────────►  ZeroDDS DataReader
    │                       │
    │                       ▼ on_data
    │                  Bridge::forward
    │                       │
    │                       ▼ zenoh::publisher::put
    │                  Zenoh KeyExpr
    │                       │
    │  ◄────────────────────┤  Zenoh Subscriber
    │                       │
    │                  Bridge::reverse
    │                       │
    │                       ▼ DataWriter::write
    ▼
 DDS-Topic
```

## QoS-Mapping (DDS → Zenoh)

| DDS-QoS | Zenoh-Aequivalent |
|---------|-------------------|
| `Reliability::Reliable` | `Reliability::Reliable` (Zenoh) |
| `Reliability::BestEffort` | `Reliability::BestEffort` |
| `Durability::TransientLocal` | `CongestionControl::Block` + `Priority::DataHigh` |
| `Durability::Volatile` (Default) | `CongestionControl::Drop` |
| `History::KeepLast(n)` | (Zenoh hat kein History-Cache; n>1 ignoriert) |
| `Partition` | KeyExpr-Praefix (`<partition>/<topic>`) |

## Was ist drin

**Pure-Rust-Layer (default, ohne `zenoh`-Dep):**
- `TopicMap` — bidirektionale Topic↔KeyExpr-Mapping-Datenstruktur.
- `key_expr_for_topic(name, partition)` — DDS-Topic → Zenoh-KeyExpr-
  Mapping (mit optionalem Partition-Praefix).
- `dds_qos_to_zenoh(dds_qos)` — QoS-Tabelle wie oben.

**Live-Runtime (Feature `zenoh-runtime`):**
- `ZenohBridge` / `ZenohBridgeBuilder` / `BridgeError` — Async-Bridge-
  Lifecycle (start / shutdown / forward / reverse).

## Schichten-Position

Layer 5 — Bridges (Tier-C). Sitzt auf
[`zerodds-dcps`](../dcps) (DCPS-API) und
[`zerodds-qos`](../qos) (QoS-Policies). Live-Runtime nutzt `zenoh = 1`
+ `tokio`.

## Quickstart

Pure-Rust-Mapping (default):

```rust
use zerodds_zenoh_bridge::{TopicMap, key_expr_for_topic};

let mut map = TopicMap::new();
map.add("VehicleTracking.TrackUpdate".into(), "fleet/tracking".into());

assert_eq!(
    key_expr_for_topic("VehicleTracking.TrackUpdate", None),
    "VehicleTracking/TrackUpdate"
);
assert_eq!(
    key_expr_for_topic("VehicleTracking.TrackUpdate", Some("fleet")),
    "fleet/VehicleTracking/TrackUpdate"
);
```

Live-Runtime (Feature `zenoh-runtime`):

```toml
[dependencies]
zerodds-zenoh-bridge = { version = "1.0.0-rc.1", features = ["zenoh-runtime"] }
```

```rust,ignore
use zerodds_zenoh_bridge::{ZenohBridge, ZenohBridgeBuilder};

let bridge = ZenohBridgeBuilder::new()
    .with_zenoh_config(zenoh::config::default())
    .build()
    .await?;

bridge.forward_topic("VehicleTracking.TrackUpdate").await?;
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | `std`-Aufruf. |
| `zenoh-runtime` | ❌ | Live-Bridge-Pfad mit `zenoh = 1` + `tokio` (ggf. höhere MSRV als ZeroDDS-Default). |

## Stabilitaet

`1.0.0-rc.1`. Public-API + Mapping-Logik sind RC1-stabil. Live-
Runtime-API folgt `zenoh = 1.x` und kann sich mit Zenoh-Major-Bumps
aendern.

## Tests

```bash
cargo test -p zerodds-zenoh-bridge
```

6 Tests grün (5 unit + 1 doc).

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/zenoh-bridge.md`](../../docs/release/rc1-reviews/zenoh-bridge.md) — RC1-Review.
- [Eclipse Zenoh](https://zenoh.io/) — Upstream-Projekt.
