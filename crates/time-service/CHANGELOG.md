# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1]

Initial release materialization der `zerodds-time-service`-Crate.

### Spec-Referenzen

- **OMG Time Service 1.1** (formal/2002-05-07) §1.3.2 (TimeBase: TimeT, InaccuracyT, TdfT, UtcT, IntervalT).
- **OMG Time Service 1.1** §1.3.3 (TimeUnavailable Exception).
- **OMG Time Service 1.1** §1.3.4 (UTO — Universal Time Object).
- **OMG Time Service 1.1** §1.3.5 (TIO — Time Interval Object).
- **OMG Time Service 1.1** §2.1 (TimeService Interface).

### Public-API

**TimeBase** (`time_base`-Modul):

- `TimeT` (Type-Alias `u64`) — 64-bit 100ns-Tick-Counter.
- `InaccuracyT` (type alias `u64`) — 48-bit inaccuracy value.
- `TdfT` (Type-Alias `i16`) — Time-Displacement-Factor in Minuten.
- `UtcT { time, inacclo, inacchi, tdf }` — 16-byte wire struct with `to_wire`/`from_wire` roundtrip + `local_time`.
- `IntervalT { lower_bound, upper_bound }` — with `IntervalT::new` validation.
- `current_time() -> TimeT` — wall clock (`#[cfg(feature = "std")]`); no_std stub returns `0`.
- `UTC_EPOCH_TO_UNIX_TICKS`, `TICKS_PER_SECOND` — Konstanten.

**UTO** (`uto`-Modul):

- `Uto` — Immutable Universal-Time-Object.
- `Uto::new` / `Uto::from_utc` — Konstruktoren.
- `Uto::time` / `inaccuracy` / `tdf` / `utc_time` — Getter (Spec §1.3.4.1-4).
- `Uto::absolute_time` — Spec §1.3.4.5 (`#[cfg(feature = "std")]`).
- `Uto::compare_time(ComparisonType, Uto) -> TimeComparison` — spec §1.3.4.6 with IntervalC/MidC.
- `Uto::time_to_interval` / `interval` — Spec §1.3.4.7-8.
- `ComparisonType::{IntervalC, MidC}`.
- `TimeComparison::{EqualTo, LessThan, GreaterThan, Indeterminate}`.

**TIO** (`tio`-Modul):

- `Tio` — Time Interval Object.
- `Tio::time_interval` / `overlaps` / `contains` / `spans` — Operations (Spec §1.3.5).
- `OverlapType` — Overlap-Klassifizierung.

**Service** (`service`-Modul):

- `TimeService { default_tdf, default_inaccuracy, secure_source }`.
- `TimeService::universal_time` / `secure_universal_time` — Spec §2.1.1-2.
- `TimeService::new_universal_time` / `uto_from_utc` / `new_interval` — Spec §2.1.2.
- `TimeUnavailable` — exception type with `Display` + `std::error::Error` impl.

### Implementation

- `forbid(unsafe_code)`.
- `#![cfg_attr(not(feature = "std"), no_std)]` with mandatory `alloc`.
- 35 unit tests + 1 doc-test green.
- Conversion factor `UTC_EPOCH_TO_UNIX_TICKS = 122_192_928_000_000_000` (141_427 days × 86_400 sec × 10_000_000 ticks).
- 16-byte UtcT wire format byte-exact per spec §1.3.2.4.

### Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std`   | ✅      | `current_time()` via `SystemTime`; std re-exports |
| `alloc` | ✅      | mandatory (Vec/wire buffer) |

### Relationship to DDS-DCPS Time_t

Spec-distinct: OMG Time Service 1.1 uses a 16-byte `UtcT` with a 1582 epoch and 100ns ticks; DDS-DCPS 1.4 §2.3.3 uses an 8-byte `Time_t` with a 1970 Unix epoch and 1ns resolution. ZeroDDS-DDS-DCPS therefore does not consume `zerodds-time-service` internally. Consumers are end-user applications with an OMG Time Service conformance need (e.g. the tutorial `dds-warehouse/02-time-sync`).
