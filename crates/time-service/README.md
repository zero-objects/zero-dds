# `zerodds-time-service`

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/zerodds-time-service.svg)](https://crates.io/crates/zerodds-time-service)
[![docs.rs](https://img.shields.io/docsrs/zerodds-time-service)](https://docs.rs/zerodds-time-service)

OMG Time Service 1.1 (formal/2002-05-07) — Datatypes + Operations + TimeService API.

Pure-Rust no_std + alloc, `forbid(unsafe_code)`. Part of [**ZeroDDS**](../../README.md). Safety classification: **STANDARD**.

## Scope

Implementiert die normativen Datentypen und Operationen der OMG Time Service Spec 1.1 als Standalone-Library:

- **TimeBase** (§1.3.2): `TimeT`, `InaccuracyT`, `TdfT`, `UtcT` (16-byte wire), `IntervalT`.
- **UTO** (§1.3.4): Universal Time Object mit `absolute_time`, `compare_time`, `time_to_interval`, `interval`.
- **TIO** (§1.3.5): Time Interval Object mit `overlaps`, `contains`, `spans`.
- **TimeService** (§2.1): `universal_time`, `secure_universal_time`, `new_universal_time`, `uto_from_utc`, `new_interval`.

**Out-of-Scope:** CORBA-`CosTimerEvent::TimerEventService` (§2.2/§2.4) — verlangt CORBA-Event-Channel-ORB; wird im `corba-ccm`-CCM-PSM adressiert.

## Verhältnis zu DDS-DCPS Time_t

OMG Time Service 1.1 und DDS-DCPS 1.4 §2.3.3 sind **spec-distinkt**:

| Aspekt | OMG Time Service 1.1 | DDS-DCPS 1.4 |
|---|---|---|
| Wire-Format | `UtcT` 16 byte | `Time_t` 8 byte |
| Epoch | 15 October 1582 | 1 January 1970 (UNIX) |
| Auflösung | 100ns Ticks | 1ns (sec + nanosec) |
| Inaccuracy / TDF | ja | nein |

ZeroDDS-DDS-DCPS verwendet ausschließlich das spec-mandate `Time_t`-Format. `zerodds-time-service` ist dafür **NICHT** der Backing-Layer — es ist eine eigene Public-API für Anwendungen, die OMG-Time-Service-1.1-Konformität brauchen (z.B. Distributed-Time-Sync mit Inaccuracy-Tracking).

## Quick Start

```rust
use zerodds_time_service::{TimeService, Uto, ComparisonType};

let service = TimeService::default();
let now = service.universal_time().unwrap();

// Vergleich mit Inaccuracy-Envelope
let later = Uto::new(now.time() + 1_000_000, 50, 0);
let cmp = now.compare_time(ComparisonType::IntervalC, later);
assert_ne!(cmp, zerodds_time_service::TimeComparison::GreaterThan);
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std`   | ✅       | std-Re-Exports + `current_time()` Wall-Clock; implies `alloc` |
| `alloc` | ✅       | mandatory (Vec/wire-buffer) |

`no_std`-Build ohne `std`: `current_time()` liefert `0`; eine echte Zeitquelle wird vom Embedding via `TimeService::with_source(...)` injiziert.

## Tests

`cargo test -p zerodds-time-service`: 36 Tests (35 unit + 1 doctest).

## Stability

Alle Public-API-Items sind ab `1.0.0-rc.1` semver-stabil.

## Links

- Spec: [OMG Time Service 1.1](https://www.omg.org/spec/TIME/1.1/)
- CHANGELOG: [`CHANGELOG.md`](CHANGELOG.md)
- Coverage-Doc: [`docs/spec-coverage/omg-time-1.1.md`](../../docs/spec-coverage/omg-time-1.1.md)
- Tutorial: [`examples/tutorials/dds-warehouse/stations/02-time-sync/`](../../examples/tutorials/dds-warehouse/stations/02-time-sync/)
