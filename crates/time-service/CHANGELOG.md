# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1]

Initiale Release-Materialisierung der `zerodds-time-service`-Crate.

### Spec-Referenzen

- **OMG Time Service 1.1** (formal/2002-05-07) §1.3.2 (TimeBase: TimeT, InaccuracyT, TdfT, UtcT, IntervalT).
- **OMG Time Service 1.1** §1.3.3 (TimeUnavailable Exception).
- **OMG Time Service 1.1** §1.3.4 (UTO — Universal Time Object).
- **OMG Time Service 1.1** §1.3.5 (TIO — Time Interval Object).
- **OMG Time Service 1.1** §2.1 (TimeService Interface).

### Public-API

**TimeBase** (`time_base`-Modul):

- `TimeT` (Type-Alias `u64`) — 64-bit 100ns-Tick-Counter.
- `InaccuracyT` (Type-Alias `u64`) — 48-bit Inaccuracy-Wert.
- `TdfT` (Type-Alias `i16`) — Time-Displacement-Factor in Minuten.
- `UtcT { time, inacclo, inacchi, tdf }` — 16-byte Wire-Struct mit `to_wire`/`from_wire` Roundtrip + `local_time`.
- `IntervalT { lower_bound, upper_bound }` — mit `IntervalT::new` Validation.
- `current_time() -> TimeT` — Wall-Clock (`#[cfg(feature = "std")]`); no_std-Stub liefert `0`.
- `UTC_EPOCH_TO_UNIX_TICKS`, `TICKS_PER_SECOND` — Konstanten.

**UTO** (`uto`-Modul):

- `Uto` — Immutable Universal-Time-Object.
- `Uto::new` / `Uto::from_utc` — Konstruktoren.
- `Uto::time` / `inaccuracy` / `tdf` / `utc_time` — Getter (Spec §1.3.4.1-4).
- `Uto::absolute_time` — Spec §1.3.4.5 (`#[cfg(feature = "std")]`).
- `Uto::compare_time(ComparisonType, Uto) -> TimeComparison` — Spec §1.3.4.6 mit IntervalC/MidC.
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
- `TimeUnavailable` — Exception-Type mit `Display` + `std::error::Error` Impl.

### Implementierung

- `forbid(unsafe_code)`.
- `#![cfg_attr(not(feature = "std"), no_std)]` mit mandatory `alloc`.
- 35 Unit-Tests + 1 doc-test grün.
- Konversionsfaktor `UTC_EPOCH_TO_UNIX_TICKS = 122_192_928_000_000_000` (141_427 Tage × 86_400 sec × 10_000_000 ticks).
- 16-byte UtcT Wire-Format byte-genau gemäß Spec §1.3.2.4.

### Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std`   | ✅      | `current_time()` via `SystemTime`; std-Re-Exports |
| `alloc` | ✅      | mandatory (Vec/wire-buffer) |

### Verhältnis zu DDS-DCPS Time_t

Spec-distinkt: OMG Time Service 1.1 verwendet 16-byte `UtcT` mit 1582-Epoch und 100ns-Ticks; DDS-DCPS 1.4 §2.3.3 verwendet 8-byte `Time_t` mit 1970-Unix-Epoch und 1ns-Auflösung. ZeroDDS-DDS-DCPS verbraucht `zerodds-time-service` daher nicht intern. Konsumenten sind End-User-Applikationen mit OMG-Time-Service-Konformitätsbedarf (z.B. Tutorial `dds-warehouse/02-time-sync`).
