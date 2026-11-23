# `zerodds-time-service`

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)
[![Crates.io](https://img.shields.io/crates/v/zerodds-time-service.svg)](https://crates.io/crates/zerodds-time-service)
[![docs.rs](https://img.shields.io/docsrs/zerodds-time-service)](https://docs.rs/zerodds-time-service)

OMG Time Service 1.1 (formal/2002-05-07) — Datatypes + Operations + TimeService API.

Pure-Rust no_std + alloc, `forbid(unsafe_code)`. Part of [**ZeroDDS**](../../README.md). Safety classification: **STANDARD**.

## Scope

Implements the normative data types and operations of the OMG Time Service spec 1.1 as a standalone library:

- **TimeBase** (§1.3.2): `TimeT`, `InaccuracyT` (48 bits; a larger value is `BadParam`), `TdfT`, `UtcT` (16-byte wire), `IntervalT`.
- **UTO** (§1.3.4): Universal Time Object with `absolute_time`, `compare_time`, `time_to_interval`, `interval`.
- **TIO** (§1.3.5): Time Interval Object with `time_interval`, `spans`, `overlaps`, `time`.
- **TimeService** (§2.1): `universal_time`, `secure_universal_time`, `new_universal_time`, `uto_from_utc`, `new_interval`, `absolute_time`. The CORBA system exceptions the spec names are `BadParam` and `DataConversion`.
- **Administration and secure time** (App. A, App. B; modules `admin` (feature `std`) and `secure`): `TimeService::administered` returns the service with its `TimeAdmin`; `set_time`, `reset_time`, `designate_source`, `withdraw_source` and `set_inaccuracy` are authorized by an `AdminAuthorizer` and recorded by an `AuditSink` before they take effect. `secure_universal_time` returns a time only under the criteria of the section "Secure time" below.
- **Time zone** (§1.3.2.4): every absolute UTO carries the time displacement factor of the local time zone at its time (`TZ`, `/etc/localtime` or the Windows setting, read with `jiff`; on Windows the zone is looked up in the time zone database built into the binary); `TimeService::with_tdf` states a fixed value, the `no_std` default is 0.

- **Timer Event Service** (§2.2 to §2.4, module `timer_event`, feature `timer-event`): `TimerEventService` (`register`, `unregister`, `event_time`), `TimerEventHandler` (`status`, `time_set`, `set_timer`, `cancel_timer`, `set_data`), `TimerEventT`, `TimeType`, `EventStatus`. Events go to a `zerodds_corba_cos_event::comm::PushConsumer`, for example the `ProxyPushConsumer` of an `EventChannel`. The service works in the process; it is not served over GIOP (`serve` covers CosTime only).

- **CORBA objects** (§1.2, feature `serve`): `servant::TimeServant` dispatches GIOP requests to `CosTime::TimeService`, `UTO` and `TIO`; `server::TimeServer` serves it over IIOP at `corbaloc::<host>:<port>/TimeService`. `secure_universal_time` raises `TimeUnavailable` over the network. The two operations that read the clock of the service, `universal_time` and `absolute_time` of a UTO, raise `CORBA::NO_PERMISSION` unless the server is started with `RemoteTimePolicy::AnswerCorrectingProxies` (App. B.2, see "Proxies (B.2)" below). `proxy::TimeServiceProxy` is a client that corrects the latency of both. A UTO or TIO reference carries its immutable value in its object key, so the server keeps no object table. `TimeUnavailable` is the CosTime user exception, `BAD_PARAM` and `DATA_CONVERSION` are system exceptions; GIOP 1.0 to 1.2, both byte orders. Tested against an omniORB 4.3 client (`tests/omniorb_e2e.rs`).

### Expected trigger interval (§2.2.2)

One worker thread per `TimerEventService` waits on a condition variable until the earliest trigger time and pushes when it wakes. An event is never pushed before its requested time. It is pushed after it by the wake-up latency of the operating system plus the time the pushes take that are due before it; the worker reads the clock again at least once a second, so a step of the system clock delays a trigger by at most one second. Each `TimerEventT` carries its actual delay as the inaccuracy of `utc`. Measured on the Linux test host (test `time_2_2_2_trigger_delay_is_reported_and_bounded`, 20 triggers of 5 ms, no load): at most 68 µs.

## Secure time (Appendix A and B)

This section is the statement Time Service 1.1 asks of the implementor: how the criteria for secure time are handled (§2.5, App. A), how synchronization values are protected in transit and whether the implementation is secure (App. B.1.1), and how proxies are handled (App. B.2). App. A.1 announces its criteria without listing them; the criteria below are those of §1.1.3, §2.1.2 and App. B.

**Is the implementation secure?** A `TimeService` built with `default`, `with_source`, `with_tdf` or `with_inaccuracy` is not secure: `secure_universal_time` raises `TimeUnavailable`. An administered service (`TimeService::administered`, feature `std`) is secure at a call of `secure_universal_time` where all criteria 1 to 7 hold; at any other call it raises `TimeUnavailable`. Under `no_std` there is no administration and no secure time. Over the network no secure time is given.

| # | Criterion | Spec | Handling | Test |
|---|---|---|---|---|
| 1 | Only administrators authorized by the system security policy set the time and specify the source of time | App. B | `TimeAdmin` asks the `AdminAuthorizer` (the system security policy) for the `AdminSubject` and the `AdminOp` of each operation; `DenyAll` refuses every operation. The subject is the identity the embedding application authenticated. | `admin::tests::time_b_1_default_authorizer_denies_every_operation`, `time_b_1_set_time_unauthorized_is_refused_and_audited` |
| 2 | Setting and resetting the time, designating the authoritative source and changing the inaccuracy are audited | App. B.1 | Every invocation, granted, denied or failed, goes to the `AuditSink` with time, subject, operation, value before and after and outcome, before the change takes effect; a record that cannot be written refuses the operation. `zerodds-security-logging` (feature `time-service`) writes the records as JSON lines. | `admin::tests::time_b_1_audit_failure_refuses_the_operation`, `time_b_1_designate_source_is_audited_before_it_takes_effect` |
| 3 | An authoritative source | App. A, App. B.1 | Secure time comes only from a source designated through `TimeAdmin::designate_source`. A service that is not administered has none. | `admin::tests::time_a_1_secure_time_unavailable_without_designated_source` |
| 4 | Current time with an error parameter | §1.1.3 | The source returns the time and the bound of its error from the same reading (`SourceReading`); on Linux `zerodds_dcps::time_source::KernelClock` reads both with `adjtimex(2)` (`maxerror`). The UTO carries the bound, or the larger administered inaccuracy; a bound above 48 bits is `TimeUnavailable`. | `admin::tests::time_a_1_secure_time_returns_source_time_and_inaccuracy`, `time_a_1_secure_time_unavailable_when_the_bound_exceeds_48_bits` |
| 5 | Trusted within the inaccuracy window | §1.1.3 | The reading must report a synchronized clock; `KernelClock` reports it when the kernel clock has no `STA_UNSYNC`, the state is not `TIME_ERROR` and no leap second is in progress. A time set by hand has no known bound: no secure time until `reset_time`. | `admin::tests::time_a_1_secure_time_unavailable_when_source_reports_unsynchronized`, `time_a_1_secure_time_unavailable_while_a_time_set_by_hand_is_in_effect` |
| 6 | Monotonically increasing | §1.1.3 | A reading before the last time `secure_universal_time` returned is `TimeUnavailable`, also after a new source is designated. | `admin::tests::time_a_1_secure_time_unavailable_when_reading_goes_backwards` |
| 7 | Synchronization values protected in transit | App. B.1.1 | See below; a source designated as `SyncProtection::Unprotected` gives no secure time. | `admin::tests::time_a_1_secure_time_unavailable_for_unprotected_synchronization` |
| 8 | Proxies | App. B.2 | See below. | `servant::tests::time_b_2_universal_time_is_refused_by_default`, `time_b_2_absolute_time_is_refused_by_default`, `time_b_2_secure_universal_time_never_leaves_the_address_space`; `tests/time_svc_e2e.rs::time_b_2_the_server_gives_no_current_time_by_default`, `time_b_2_secure_time_stays_in_the_address_space_of_the_service`, `time_b_2_the_proxy_widens_the_interval_by_the_measured_latency`, `time_b_2_the_proxy_widens_absolute_time_by_the_measured_latency` |

**Synchronization in transit (B.1.1).** ZeroDDS synchronizes no clock and transmits no time values for synchronization. The clock a source reads is synchronized by the host: chrony or ntpd with NTS (RFC 8915) or a symmetric key (RFC 5905 §7.3, RFC 8573), ptp4l with the PTP authentication TLV (IEEE 1588-2019 §16.14), or a local reference clock with no network in between. The administrator states which applies when designating the source (`SyncProtection::{LocalReference, Nts, NtpSymmetricKey, PtpAuthenticated}`); the statement is part of the audit record of the designation. ZeroDDS cannot check it. `Unprotected` is the statement for plain NTP or PTP and gives no secure time. The time `TimeServiceProxy` reads over IIOP is not protected in transit; it is never secure time.

**Threats of the Security Service.** The threats listed in the CORBA Security Service specification (version 1.8, §1.1.3), as they apply to an administered service:

| Threat | Handling |
|---|---|
| An authorized user gaining access to information hidden from him | The service holds no confidential data. The audit records go where the `AuditSink` puts them; access to that file is the operating system's. |
| A user masquerading as someone else; rights delegated too widely | The service authenticates nobody: the `AdminSubject` is the identity the embedding application authenticated, and the `AdminAuthorizer` decides on it. There is no delegation. |
| Security controls being bypassed | The administered values change only through `TimeAdmin`; the fields are private, and `with_tdf`, `with_inaccuracy` on an administered service return a new service that is not administered and never secure. The clock of the operating system is protected by the operating system (`CAP_SYS_TIME` on Linux); ZeroDDS does not set it. |
| Eavesdropping on a communication line | Time values are not confidential; they are not encrypted. |
| Tampering with communication between objects | Secure time never crosses the network: the servant raises `TimeUnavailable` for `secure_universal_time`, the proxy never gives secure time. The synchronization of the host clock is protected as the administrator states (B.1.1). |
| Lack of accountability | Every administrative operation, also a refused one, is recorded with its subject before it takes effect; without the record it does not take effect. |

**Proxies (B.2).** A Time Service in every address space is the default: `TimeService` is a library in the process of its caller, and a `TimeServer` (feature `serve`) gives a remote caller no current time (`RemoteTimePolicy::Refuse`). The two operations that read the clock of the service, `universal_time` and `absolute_time` of a UTO ("current time + time in the object", §1.3.4.5), raise the system exception `CORBA::NO_PERMISSION`. The operations that read no clock are served: `new_universal_time`, `uto_from_utc`, `new_interval`, the attributes and the other operations of UTO and TIO. This deviates from §2.1.1 ("If any time is available at all, that time is returned by this operation") and from §1.3.4.5 for a remote caller: the served objects give no time although the clock of the service gives one. B.2 requires that a time made stale by an unknown latency does not reach the caller ("Implementors of the Time Service must prevent this problem from occurring"); the refusal follows the first way B.2 names, a caller that needs the current time uses a Time Service in its own address space. The refusal is not `TimeUnavailable`: that exception keeps the meaning §2.1.1 gives it, the failure of the underlying time provider. `secure_universal_time` raises `TimeUnavailable` over the network under every policy. With `RemoteTimePolicy::AnswerCorrectingProxies` the server answers `universal_time` and `absolute_time`; this policy is for callers that correct the latency themselves, which the server cannot check. `proxy::TimeServiceProxy` is such a caller: it measures on its monotonic clock the round trip of `universal_time` (or `absolute_time`) and of `_get_utc_time` on the returned UTO, and returns the time and inaccuracy each increased by half of it, so that the interval contains the time at which it returns (Cristian, App. F).

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
let later = Uto::new(now.time() + 1_000_000, 50, 0).unwrap(); // BAD_PARAM above 48 bits
let cmp = now.compare_time(ComparisonType::IntervalC, later);
assert_ne!(cmp, zerodds_time_service::TimeComparison::GreaterThan);
```

## Feature-Flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std`   | ✅       | std-Re-Exports + `current_time()` Wall-Clock + `local_tdf()` (dependency `jiff`); implies `alloc` |
| `alloc` | ✅       | mandatory (Vec/wire-buffer) |
| `timer-event` | ✅  | Timer Event Service (`timer_event`); implies `std`, dependency `zerodds-corba-cos-event` |
| `serve` | — | CosTime as CORBA objects (`servant`, `server`) and the latency-correcting `proxy`; implies `std`, dependencies `zerodds-wire`, `zerodds-corba-giop`, `zerodds-corba-iiop`, `zerodds-corba-ior` |

`no_std` build without `std`: `current_time()` returns `0`; a real time source is injected by the embedding via `TimeService::with_source(...)`. There is no administration and no secure time without `std`.

## Tests

`cargo nextest run -p zerodds-time-service --all-targets`: 95 tests with the default features, 123 with `--features serve`. The omniORB test (`tests/omniorb_e2e.rs`) is ignored there; it needs omniidl, g++ and libomniORB4 (`-- --ignored`).

## Stability

`1.0.0-rc.7`, a pre-release. The public API can still change before
1.0.0; the crate's CHANGELOG lists every change.

## Links

- Spec: [OMG Time Service 1.1](https://www.omg.org/spec/TIME/1.1/)
- CHANGELOG: [`CHANGELOG.md`](CHANGELOG.md)
- Coverage-Doc: [`docs/spec-coverage/omg-time-1.1.md`](../../docs/spec-coverage/omg-time-1.1.md)
- Tutorial: [`examples/tutorials/dds-warehouse/stations/02-time-sync/`](../../examples/tutorials/dds-warehouse/stations/02-time-sync/)
