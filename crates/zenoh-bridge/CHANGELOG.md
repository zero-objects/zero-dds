# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-zenoh-bridge`-Crate.

### Spec-Referenzen

Es gibt keine OMG-Spec fuer Zenoh-Bridges. Diese Crate folgt dem De-facto-Pattern von ZettaScale's `zenoh-bridge-dds` (siehe <https://github.com/eclipse-zenoh/zenoh-plugin-dds>), aber als eigenstaendige Rust-Library statt Plugin.

- **Eclipse Zenoh** (`zenoh = 1`) — Upstream-API.
- **OMG DDS 1.4** (via `zerodds-dcps`) + **OMG DDS-XTypes 1.3** (via `zerodds-qos`) — DDS-Side.

### Public-API

**Pure-Rust-Mapping-Layer (default, ohne `zenoh`-Dep):**
- `TopicMap` — bidirektionale Topic↔KeyExpr-Mapping-Datenstruktur.
- `key_expr_for_topic(name, partition: Option<&str>) -> String` — DDS-Topic → Zenoh-KeyExpr-Mapping (mit optionalem Partition-Praefix).
- `dds_qos_to_zenoh(dds_qos)` — QoS-Translation-Funktion.

**Live-Runtime (Feature `zenoh-runtime`):**
- `ZenohBridge`, `ZenohBridgeBuilder`, `BridgeError`.

### Implementierung

`mapping.rs` ist pure-Rust-no_std-`alloc` und enthaelt die komplette Mapping-Logik. Topic-Namen werden per Punkt-zu-Slash-Convention umgewandelt (`VehicleTracking.TrackUpdate` → `VehicleTracking/TrackUpdate`); Partitions werden als KeyExpr-Praefix gestellt (`<partition>/<topic>`).

`runtime.rs` ist nur unter Feature `zenoh-runtime` kompiliert und enthaelt den async Live-Pfad: spawnt Tokio-Tasks pro Topic, verbindet `zerodds-dcps::DataReader` mit `zenoh::Publisher` (forward) und `zenoh::Subscriber` mit `zerodds-dcps::DataWriter` (reverse).

QoS-Mapping pro DDS-QoS-Policy folgt der Tabelle im README + `lib.rs`-Header. Wichtigste Mappings:
- `Reliability::Reliable` ↔ Zenoh `Reliability::Reliable`.
- `Reliability::BestEffort` ↔ Zenoh `Reliability::BestEffort`.
- `Durability::TransientLocal` → `CongestionControl::Block` + `Priority::DataHigh`.
- `Durability::Volatile` → `CongestionControl::Drop`.
- `History::KeepLast(n)` mit n>1 wird ignoriert (Zenoh hat keinen History-Cache).
- `Partition` → KeyExpr-Praefix.

`#![forbid(unsafe_code)]` ist gesetzt. `#![cfg_attr(not(feature = "std"), no_std)]` mit `extern crate alloc` — die default-Variante ist no_std-fahig.

### Architektur

- **Layer:** 5 (Bridges, Tier-C).
- **Dependencies (in):** `zerodds-dcps` (DCPS-API), `zerodds-qos` (QoS-Policies). Mit `zenoh-runtime`: `zenoh = 1`, `tokio = 1`, `thiserror = 2`.
- **Dependents (out):** Caller-Layer (z.B. Edge-Gateway-Daemon).
- **Feature-Flags:** `std` (default), `zenoh-runtime` (opt-in fuer Live-Bridge).

### Stabilitaet

- Public-API + Mapping-Logik: RC1-stabil.
- Live-Runtime-API folgt `zenoh = 1.x`; Zenoh-Major-Bumps koennen Live-Runtime-API ändern, das Pure-Rust-Mapping-Layer bleibt stabil.
