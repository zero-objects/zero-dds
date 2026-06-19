# `zerodds-time-service`

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/zerodds-time-service.svg)](https://crates.io/crates/zerodds-time-service)
[![docs.rs](https://img.shields.io/docsrs/zerodds-time-service)](https://docs.rs/zerodds-time-service)

OMG Time Service 1.1 (formal/2002-05-07) — Datatypes + Operations + TimeService API.

Pure-Rust no_std + alloc, `forbid(unsafe_code)`. Part of [**ZeroDDS**](../../README.md). Safety classification: **STANDARD**.

## Scope

Implements the normative data types and operations of the OMG Time Service spec 1.1 as a standalone library:

- **TimeBase** (§1.3.2): `TimeT`, `InaccuracyT`, `TdfT`, `UtcT` (16-byte wire), `IntervalT`.
- **UTO** (§1.3.4): Universal Time Object with `absolute_time`, `compare_time`, `time_to_interval`, `interval`.
- **TIO** (§1.3.5): Time Interval Object with `overlaps`, `contains`, `spans`.
- **TimeService** (§2.1): `universal_time`, `secure_universal_time`, `new_universal_time`, `uto_from_utc`, `new_interval`.

**Out of scope:** CORBA `CosTimerEvent::TimerEventService` (§2.2/§2.4) — requires a CORBA event-channel ORB; addressed in the `corba-ccm` CCM PSM.

## Relationship to DDS-DCPS Time_t

OMG Time Service 1.1 and DDS-DCPS 1.4 §2.3.3 are **spec-distinct**:

| Aspect | OMG Time Service 1.1 | DDS-DCPS 1.4 |
|---|---|---|
| Wire format | `UtcT` 16 byte | `Time_t` 8 byte |
| Epoch | 15 October 1582 | 1 January 1970 (UNIX) |
| Resolution | 100ns ticks | 1ns (sec + nanosec) |
| Inaccuracy / TDF | yes | no |

ZeroDDS-DDS-DCPS uses exclusively the spec-mandated `Time_t` format. `zerodds-time-service` is **NOT** the backing layer for it — it is a separate public API for applications that need OMG Time Service 1.1 conformance (e.g. distributed time sync with inaccuracy tracking).

## Quick Start

```rust
use zerodds_time_service::{TimeService, Uto, ComparisonType};

let service = TimeService::default();
let now = service.universal_time().unwrap();

// Comparison with inaccuracy envelope
let later = Uto::new(now.time() + 1_000_000, 50, 0);
let cmp = now.compare_time(ComparisonType::IntervalC, later);
assert_ne!(cmp, zerodds_time_service::TimeComparison::GreaterThan);
```

## Feature-Flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std`   | ✅       | std-Re-Exports + `current_time()` Wall-Clock; implies `alloc` |
| `alloc` | ✅       | mandatory (Vec/wire-buffer) |

`no_std` build without `std`: `current_time()` returns `0`; a real time source is injected by the embedding via `TimeService::with_source(...)`.

## Tests

`cargo test -p zerodds-time-service`: 36 Tests (35 unit + 1 doctest).

## Stability

All public-API items are semver-stable as of `1.0.0-rc.1`.

## Links

- Spec: [OMG Time Service 1.1](https://www.omg.org/spec/TIME/1.1/)
- CHANGELOG: [`CHANGELOG.md`](CHANGELOG.md)
- Coverage-Doc: [`docs/spec-coverage/omg-time-1.1.md`](../../docs/spec-coverage/omg-time-1.1.md)
- Tutorial: [`examples/tutorials/dds-warehouse/stations/02-time-sync/`](../../examples/tutorials/dds-warehouse/stations/02-time-sync/)
