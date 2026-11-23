# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Added
- Timer Event Service, module `timer_event` (feature `timer-event`, default; OMG Time Service 1.1 §2.2 to §2.4): `TimerEventService` (`register` with a `CosEventComm::PushConsumer` of `zerodds-corba-cos-event`, `unregister`, `event_time`), `TimerEventHandler` (`status`, `time_set`, `set_timer`, `cancel_timer`, `set_data`), `TimerEventT`, `TimeType`, `EventStatus` with the four states of §2.2.3.2, `TimerEventError` (`BAD_PARAM`, `NO_RESOURCES`, `INV_OBJREF`). One worker thread per service; each event carries its delay as inaccuracy.
- CosTime as CORBA objects (feature `serve`, §1.2): `servant::TimeServant` (GIOP dispatch for `TimeService`, `UTO`, `TIO`, GIOP 1.0 to 1.2, both byte orders) and `server::TimeServer` (IIOP, `corbaloc::host:port/TimeService`).
- Administration and secure time (App. A, App. B): `TimeService::administered` and `admin::TimeAdmin` (`set_time`, `reset_time`, `designate_source`, `withdraw_source`, `set_inaccuracy`), each authorized by an `admin::AdminAuthorizer` (`DenyAll` refuses all) and recorded by an `admin::AuditSink` before it takes effect; `secure::{SecureTimeSource, SourceReading, SyncProtection, SourceDesignation}`; `TimeService::is_administered`.
- `servant::RemoteTimePolicy`, `servant::REFUSED_REMOTE_TIME` and `server::TimeServer::start_with_policy` (App. B.2); `proxy::TimeServiceProxy` (`universal_time`, `absolute_time`, `secure_universal_time`) and `proxy::correct_for_latency`, a client that widens time and inaccuracy by half the measured round trip (feature `serve`).
- README section "Secure time (Appendix A and B)": the statement of App. A, B.1.1 and B.2.
- `time_base::local_tdf`, `TimeService::with_tdf`, `TimeService::with_inaccuracy`, `TimeService::inaccuracy`, `TimeService::tdf_at`, `TimeService::absolute_time`, `MAX_INACCURACY`, `exception::{BadParam, DataConversion, AbsoluteTimeError}`.

### Changed
- Absolute UTOs (`universal_time`, `absolute_time`) carry the time displacement factor of the local time zone at their time (§1.3.2.4), read with `jiff`; `no_std` uses 0 unless `with_tdf` states it.
- `UtcT::new`, `UtcT::set_inaccuracy`, `Uto::new` and `TimeService::new_universal_time` return `BadParam` for an inaccuracy above 48 bits instead of cutting it (§2.1.2.1); `TimeService::new_interval` returns `Result<Tio, BadParam>`; `Tio::time` returns `DataConversion` when half the width exceeds 48 bits; `Uto::absolute_time` returns `Result<Uto, AbsoluteTimeError>`.
- The public fields `TimeService::default_tdf` and `default_inaccuracy` are gone (`with_tdf`, `with_inaccuracy`).
- The public field `TimeService::secure_source` is gone: `secure_universal_time` returns a time only from an administered service under the criteria of the README (App. A); otherwise `TimeUnavailable`.
- Over GIOP `secure_universal_time` always raises `TimeUnavailable`. `TimeServer::start` gives no current time of the service to a remote caller: `universal_time` and `absolute_time` of a UTO raise `CORBA::NO_PERMISSION` (`RemoteTimePolicy::Refuse`, App. B.2), not `TimeUnavailable`, which keeps the meaning of §2.1.1; `start_with_policy(.., AnswerCorrectingProxies)` answers both.
- `with_tdf` and `with_inaccuracy` on an administered service return a service that is not administered.

### Fixed
- `#![cfg_attr(not(feature = "std"), no_std)]` is in `src/lib.rs` again: without it the crate always linked `std`, and a build for a target without `std` failed although the manifest states `no_std` + `alloc`. `--no-default-features --features alloc` builds for `thumbv7em-none-eabihf`; `tools/no-std-claims-gate.sh` now measures the crate.

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
