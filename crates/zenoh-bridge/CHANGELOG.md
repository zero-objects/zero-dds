# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization der `zerodds-zenoh-bridge`-Crate.

### Spec references

There is no OMG spec for Zenoh bridges. This crate follows the de-facto pattern of ZettaScale's `zenoh-bridge-dds` (see <https://github.com/eclipse-zenoh/zenoh-plugin-dds>), but as a standalone Rust library instead of a plugin.

- **Eclipse Zenoh** (`zenoh = 1`) — Upstream-API.
- **OMG DDS 1.4** (via `zerodds-dcps`) + **OMG DDS-XTypes 1.3** (via `zerodds-qos`) — DDS-Side.

### Public-API

**Pure-Rust mapping layer (default, without `zenoh` dep):**
- `TopicMap` — bidirektionale Topic↔KeyExpr-Mapping-Datenstruktur.
- `key_expr_for_topic(name, partition: Option<&str>) -> String` — DDS topic → Zenoh KeyExpr mapping (with optional partition prefix).
- `dds_qos_to_zenoh(dds_qos)` — QoS translation function.

**Live-Runtime (Feature `zenoh-runtime`):**
- `ZenohBridge`, `ZenohBridgeBuilder`, `BridgeError`.

### Implementation

`mapping.rs` is pure-Rust no_std `alloc` and contains the complete mapping logic. Topic names are converted by a dot-to-slash convention (`VehicleTracking.TrackUpdate` → `VehicleTracking/TrackUpdate`); partitions are placed as a KeyExpr prefix (`<partition>/<topic>`).

`runtime.rs` is compiled only under feature `zenoh-runtime` and contains the async live path: spawns Tokio tasks per topic, connects `zerodds-dcps::DataReader` with `zenoh::Publisher` (forward) and `zenoh::Subscriber` with `zerodds-dcps::DataWriter` (reverse).

QoS mapping per DDS QoS policy follows the table in the README + `lib.rs` header. Most important mappings:
- `Reliability::Reliable` ↔ Zenoh `Reliability::Reliable`.
- `Reliability::BestEffort` ↔ Zenoh `Reliability::BestEffort`.
- `Durability::TransientLocal` → `CongestionControl::Block` + `Priority::DataHigh`.
- `Durability::Volatile` → `CongestionControl::Drop`.
- `History::KeepLast(n)` with n>1 is ignored (Zenoh has no history cache).
- `Partition` → KeyExpr prefix.

`#![forbid(unsafe_code)]` is set. `#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc` — the default variant is no_std-capable.

### Architecture

- **Layer:** 5 (bridges, tier C).
- **Dependencies (in):** `zerodds-dcps` (DCPS API), `zerodds-qos` (QoS policies). With `zenoh-runtime`: `zenoh = 1`, `tokio = 1`, `thiserror = 2`.
- **Dependents (out):** the caller layer (e.g. an edge-gateway daemon).
- **Feature flags:** `std` (default), `zenoh-runtime` (opt-in for the live bridge).

### Stability

- Public API + mapping logic: RC1-stable.
- The live runtime API follows `zenoh = 1.x`; Zenoh major bumps may change the live runtime API, the pure-Rust mapping layer stays stable.
