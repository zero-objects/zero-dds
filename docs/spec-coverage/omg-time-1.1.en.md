# OMG Time Service 1.1 — Spec Coverage

**Spec:** [OMG Time Service 1.1 — formal/2002-05-07 (52 pages) →](https://www.omg.org/spec/TIME/)

**Context:** the OMG Time Service is a classic CORBA object service.
`crates/time-service/` implements the modules TimeBase, CosTime and
CosTimerEvent as a Rust library; the events of the Timer Event Service go to
the `PushConsumer` of `crates/corba-cos-event/`. With the feature `serve`, CosTime is
reachable as a CORBA object over GIOP (`crates/time-service/src/servant.rs`, `crates/time-service/src/server.rs`, §1.2).

Implementation:

- `crates/time-service/` — TimeBase, CosTime (UTO, TIO, TimeService) and CosTimerEvent (`timer_event.rs`, feature `timer-event`); administration and secure time of App. A and App. B (`admin.rs`, `secure.rs`, `proxy.rs`).
- `crates/dcps/src/time_source.rs::KernelClock` — the Linux kernel clock as a secure source (`adjtimex(2)`).
- `crates/security-logging/src/time_audit.rs` — the audit records of the administration as JSON lines (feature `time-service`).

---

## §1.1 Overview

### §1.1.1 Time Service requirements

**Spec:** §1.1.1, p. 1-1 (PDF) — "obtain current time together with an error
estimate associated with it. Additionally, [...] ascertain the order in which
events occurred. Generate time-based events based on timers and alarms.
Compute the interval between two events."

**Repo:** requirements 1, 2 and 4 satisfied by `crates/time-service/`
(`current_time` / `compare_time` / `time_to_interval` / `interval`).
Requirement 3 (events from timers and alarms) is met by the Timer Event
Service in `crates/time-service/src/timer_event.rs` (§2.2 to §2.4).

**Tests:** `crates/time-service/src/uto.rs::tests::compare_time_*`,
`time_to_interval_uses_midpoints`;
`crates/time-service/src/timer_event.rs::tests::time_2_2_3_2_status_triggered_after_one_shot`,
`time_2_2_3_2_periodic_never_triggered_stays_time_set`.

**Status:** done

### §1.1.2 Representation of time

**Spec:** §1.1.2, p. 1-2 (PDF) — "100 nanoseconds (10^-7 seconds); Base time:
15 October 1582 00:00:00; Approximate range: AD 30,000. [...] UTC time in this
service specification always refers to time in Greenwich Time Zone."

**Repo:** `crates/time-service/src/time_base.rs::{TimeT, TICKS_PER_SECOND,
UTC_EPOCH_TO_UNIX_TICKS}`.

**Tests:** `crates/time-service/src/time_base.rs::tests::current_time_is_recent_century`.

**Status:** done

### §1.1.3 Source of time

**Spec:** §1.1.3, p. 1-2/1-3 (PDF) — requirements on the underlying time
source (current time + error, monotonic, optionally secure).

**Repo:** `crates/time-service/src/time_base.rs::current_time` uses
`std::time::SystemTime` as the source; `TimeService::with_source` takes another
clock, `TimeService::with_inaccuracy` its error (at most 48 bits). A secure
source is a `SecureTimeSource` (`crates/time-service/src/secure.rs`): one
reading gives the time, the error bound and the state of synchronization of
the same instant (`SourceReading`). On Linux
`crates/dcps/src/time_source.rs::KernelClock` reads the kernel clock with
`adjtimex(2)` (`maxerror` as the error bound; not synchronized with
`STA_UNSYNC`, `STA_CLOCKERR`, `TIME_ERROR` or `TIME_OOP`).
`crates/time-service/src/admin.rs::Administration::secure_now` checks at every
call of `secure_universal_time`: synchronized, error bound at most 48 bits,
time not before the last one returned (monotonic); else `TimeUnavailable`. The
source is designated by an authorized, audited administrator (App. B).

**Tests:** `crates/time-service/src/service.rs::tests::universal_time_returns_recent_value`,
`time_unavailable_display_describes_failure_mode`;
`crates/time-service/src/time_base.rs::tests::current_time_is_recent_century`;
`crates/time-service/src/service.rs::tests::time_2_1_1_configured_inaccuracy_is_reported_and_bounded_to_48_bits`;
`crates/time-service/src/admin.rs::tests::time_a_1_secure_time_returns_source_time_and_inaccuracy`,
`time_a_1_secure_time_unavailable_when_source_reports_unsynchronized`,
`time_a_1_secure_time_unavailable_when_reading_goes_backwards`,
`time_a_1_secure_time_unavailable_when_the_bound_exceeds_48_bits`;
`crates/dcps/src/time_source.rs::tests::time_a_1_kernel_reading_follows_the_adjtimex_state`,
`time_a_1_adjtimex_state_decides_secure_time_of_an_administered_service`,
`time_a_1_kernel_clock_reads_the_host_clock`,
`time_a_1_kernel_clock_as_authoritative_source`.

**Status:** done — the tests ran green on the Linux test host. Both cases, a synchronized and an unsynchronized kernel clock, are checked by `time_a_1_adjtimex_state_decides_secure_time_of_an_administered_service` with fixed `adjtimex` values through `kernel_reading` in a designated source of an administered service, without the host clock. `time_a_1_kernel_clock_as_authoritative_source` reads the host clock and checks only the case the host gives at that moment; it prints it. Measured on the Linux test host: a probe with `adjtimex(2)` without privileges gave state `TIME_OK`, `STA_UNSYNC` not set, `maxerror` 108 369 µs; a later run of the test with `--no-capture` printed: synchronized, `maxerror` 173 875 µs, secure time given.

---

## §1.2 General object model

### §1.2 General object model — service-object pattern

**Spec:** §1.2, p. 1-3 (PDF) — the service object manages instance objects
(UTOs/TIOs) via the service interface. The CORBA object-service pattern.

**Repo:** Rust API: `TimeService` with the factory operations
`new_universal_time`, `uto_from_utc`, `new_interval` (`crates/time-service/src/service.rs`),
`TimerEventService::register` for the handlers (§2.2.1). CORBA objects
(feature `serve`): `crates/time-service/src/servant.rs::TimeServant`
dispatches GIOP requests to the service object `CosTime::TimeService` (the
five operations of §2.1), to `UTO` (attributes and operations of §1.3.4) and
`TIO` (§1.3.5), and the `CORBA::Object` operations (`_is_a`,
`_non_existent`, `_repository_id`). Every UTO and TIO it creates is a CORBA
object of its own with an IOR; its object key carries the immutable value
(`CosTime/UTO/` and the 16 octets of the `UtcT`, `CosTime/TIO/` and the 16
octets of the `IntervalT`), the server keeps no object table. A UTO or TIO
of another server passed as a parameter is read over IIOP with
`_get_utc_time` or `_get_time_interval`; a nil or unreadable reference is
`BAD_PARAM`. `TimeUnavailable` goes out as a user exception, `BAD_PARAM` and
`DATA_CONVERSION` as system exceptions, in both byte orders.
`crates/time-service/src/server.rs::TimeServer` serves the servant over
IIOP; the service object is `corbaloc::<host>:<port>/TimeService`. A program
starts `TimeServer`; there is no standalone daemon. Over GIOP
`secure_universal_time` always raises `TimeUnavailable`. The two operations
that read the clock of the service, `universal_time` and `absolute_time` of a
UTO, answer only with `RemoteTimePolicy::AnswerCorrectingProxies`
(`TimeServer::start_with_policy`); under the default `Refuse` they raise the
system exception `NO_PERMISSION` (App. B.2, see §2.1.1 and §1.3.4.5).

**Tests:** `crates/time-service/src/service.rs::tests::new_universal_time_creates_uto_from_components`;
`crates/time-service/src/servant.rs::tests::time_1_2_universal_time_returns_a_uto_reference_with_the_value_in_its_key`,
`time_1_2_time_unavailable_is_the_cos_time_user_exception`,
`time_1_2_new_universal_time_bad_param_and_utc_round_trip`,
`time_1_2_new_interval_bad_param_and_tio_operations`,
`time_1_2_uto_operations_compare_interval_absolute`,
`time_1_2_object_operations_and_unknown_keys`,
`time_1_3_5_2_a_nil_or_unreadable_uto_is_bad_param`;
`crates/time-service/tests/time_svc_e2e.rs::time_1_2_a_giop_client_gets_a_uto_reference_and_reads_it`,
`time_1_2_locate_request_finds_the_service_object`,
`time_1_2_exceptions_reach_the_client`,
`time_1_2_a_uto_of_another_server_is_read_over_iiop`,
`time_1_2_a_giop_1_0_client_gets_its_unsigned_long_long_arguments_aligned`;
`crates/time-service/src/servant.rs::tests::time_1_2_giop_1_0_bodies_off_an_8_octet_boundary_are_aligned_from_the_message`;
`crates/time-service/tests/omniorb_e2e.rs::time_1_2_an_omniorb_client_uses_the_time_service`;
`crates/time-service/src/servant.rs::tests::time_b_2_universal_time_is_refused_by_default`,
`time_b_2_absolute_time_is_refused_by_default`;
`crates/time-service/tests/time_svc_e2e.rs::time_b_2_the_server_gives_no_current_time_by_default`.

**Status:** partial — the CosTime service (TimeService, UTO, TIO) is built as a CORBA object, checked with a GIOP client in the process (`time_svc_e2e`, GIOP 1.0 and 1.2, both byte orders) and against omniORB 4.3.3: `omniorb_e2e` ran on the Linux test host in a Debian trixie container with omniidl and libomniORB4, 3 of 3 green (with both `RemoteTimePolicy` values, `absolute_time` included); registered in the CI job `integration-bridges-vendor-corba`, not yet run there. The second service as a CORBA object is missing: `TimerEventService` and `TimerEventHandler` have no GIOP dispatch, `register` takes no remote `PushConsumer` reference, and a timer event does not go out over GIOP as an `any` with a TypeCode.

### §1.2.1 Conformance points (Basic + Timer Event)

**Spec:** §1.2.1, p. 1-4 (PDF) — two conformance points: "Basic Time Service"
(TimeBase + CosTime) and "Timer Event Service" (CosTimerEvent, optional,
depends on Basic).

**Repo:** Basic Time Service: TimeBase and CosTime in `crates/time-service/`.
Timer Event Service: CosTimerEvent in `crates/time-service/src/timer_event.rs`
(feature `timer-event`, default), on the `TimeService` of the same crate; the
events go to the `PushConsumer` of `crates/corba-cos-event/`. See §2.2.

**Tests:** Basic Time Service:
`crates/time-service/src/service.rs::tests::new_universal_time_creates_uto_from_components`,
`crates/time-service/src/uto.rs::tests::compare_time_midc_equal`,
`crates/time-service/src/tio.rs::tests::overlaps_partial`,
`crates/time-service/src/time_base.rs::tests::utct_size_is_16_octets`.
Timer Event Service: `crates/time-service/src/timer_event.rs::tests::time_2_4_1_register_starts_cleared_and_schedules_nothing`,
`time_2_3_1_2_set_timer_absolute_fires_at_the_time`;
`crates/time-service/tests/contract_corba_cos_event.rs::time_2_2_2_timer_events_flow_through_a_push_event_channel`.

**Status:** partial — CosTimerEvent is complete as a Rust API but not reachable as a CORBA object over GIOP (§1.2); CosTime is reachable as a CORBA object; the statement on the criteria of App. A for the Basic Time Service is in `crates/time-service/README.md` (§2.5, App. A).

---

## §1.3 Basic Time Service

### §1.3.1 Object model — the service manages UTOs + TIOs

**Spec:** §1.3.1, p. 1-4/1-5 (PDF) — the Time Service manages UTOs (Universal
Time Objects) and TIOs (Time Interval Objects) via factory methods.

**Repo:** `crates/time-service/src/service.rs::TimeService`,
`crates/time-service/src/uto.rs::Uto`,
`crates/time-service/src/tio.rs::Tio`.

**Tests:** `crates/time-service/src/service.rs::tests` (8 tests):
`new_universal_time_creates_uto_from_components`, `uto_from_utc_wraps_passed_struct`,
`new_interval_creates_tio_for_valid_bounds`,
`new_interval_rejects_lower_greater_than_upper`;
`crates/time-service/src/uto.rs::tests::attributes_return_constructor_values`,
`crates/time-service/src/tio.rs::tests::time_interval_attribute_returns_constructor_value`.

**Status:** done

### §1.3.2 Data types

**Spec:** §1.3.2, p. 1-5 (PDF) — `module TimeBase { typedef unsigned long long
TimeT; typedef TimeT InaccuracyT; typedef short TdfT; struct UtcT { TimeT
time; unsigned long inacclo; unsigned short inacchi; TdfT tdf; }; struct
IntervalT { TimeT lower_bound; TimeT upper_bound; }; };`.

**Repo:** `crates/time-service/src/time_base.rs::{TimeT, InaccuracyT, TdfT,
UtcT, IntervalT}`.

**Tests:** `crates/time-service/src/time_base.rs::tests::utct_size_is_16_octets`,
`intervalt_size_is_16_octets`.

**Status:** done

### §1.3.2.1 Type TimeT — 64-bit, 100ns ticks since 1582

**Spec:** §1.3.2.1, p. 1-6 (PDF) — "TimeT represents a single time value,
which is 64 bits in size, and holds the number of 100 nanoseconds that have
passed since the base time."

**Repo:** `crates/time-service/src/time_base.rs::TimeT` (alias `u64`).

**Tests:** `crates/time-service/src/time_base.rs::tests::utct_wire_roundtrip_preserves_all_fields`.

**Status:** done

### §1.3.2.2 Type InaccuracyT — 48-bit inaccuracy in 100ns

**Spec:** §1.3.2.2, p. 1-6 (PDF) — "represents the value of inaccuracy in time
in units of 100 nanoseconds. [...] 48 bits is sufficient."

**Repo:** `crates/time-service/src/time_base.rs::{InaccuracyT, MAX_INACCURACY}`;
`UtcT::new` and `UtcT::set_inaccuracy` reject a value above 48 bits with
`BadParam` instead of cutting it.

**Tests:** `crates/time-service/src/time_base.rs::tests::time_1_3_2_2_utct_rejects_inaccuracy_above_48_bits`,
`time_1_3_2_2_set_inaccuracy_rejects_above_48_bits_and_keeps_the_value`.

**Status:** done

### §1.3.2.3 Type TdfT — 16-bit time-zone offset in minutes

**Spec:** §1.3.2.3, p. 1-6 (PDF) — "size 16 bits short type and holds the time
displacement factor in the form of minutes of displacement from the Greenwich
Meridian. [...] East is positive, West is negative."

**Repo:** `crates/time-service/src/time_base.rs::TdfT` (alias `i16`).

**Tests:** `crates/time-service/src/time_base.rs::tests::local_time_negative_tdf_west_of_greenwich`.

**Status:** done

### §1.3.2.4 Type UtcT — 16-octet struct with time + inaccuracy + tdf

**Spec:** §1.3.2.4, p. 1-6/1-7 (PDF) — "UtcT defines the structure of the time
value [...] basic value of time is of type TimeT [...] inacclo and inacchi
fields together hold a 48-bit estimate [...] tdf field holds time zone
information. [...] for any given UtcT value 'utc', the local time can be
computed as utc.time + utc.tdf * 600,000,000."

**Repo:** `crates/time-service/src/time_base.rs::UtcT` with a `UtcT::local_time()`
operation. The tdf of every absolute UTO the service creates
(`TimeService::universal_time`, `TimeService::absolute_time`,
`Uto::absolute_time`) is the one of the local time zone at the time of the
UTO: `time_base::local_tdf` reads the zone of the process (the `TZ` variable
as IANA name, TZif path or POSIX rule, else `/etc/localtime`, on Windows the
system setting) through the `jiff` library, daylight saving time included.
Windows has no system zone database: `jiff` maps the Windows zone to its IANA
name and looks it up in the database the feature `tzdb-bundle-platform` builds
into the program on Windows and wasm only (`crates/time-service/Cargo.toml`).
`TimeService::with_tdf` states the value (without `std` there is no zone
database, the default there is 0). A relative UTO keeps tdf 0.
`crates/dcps/src/time_source.rs` (`TimeSource::now_uto`) carries the local
tdf as well.

**Tests:** `crates/time-service/src/time_base.rs::tests::utct_size_is_16_octets`,
`local_time_applies_tdf`, `local_time_negative_tdf_west_of_greenwich`;
`crates/time-service/src/service.rs::tests::time_1_3_2_4_universal_time_carries_the_local_tdf`
(a child process per zone: `CET-1CEST,M3.5.0,M10.5.0/3` 60 in January, 120 in
July, `EST5EDT,M3.2.0,M11.1.0` -300, `IST-5:30` 330, `UTC0` 0),
`time_1_3_2_4_absolute_time_carries_the_local_tdf`,
`time_1_3_2_4_stated_tdf_is_carried_by_universal_and_absolute_time`;
`crates/dcps/src/time_source.rs::tests::time_1_3_2_4_now_uto_carries_zero_inaccuracy_and_the_local_tdf`
(a child process per zone: CET 60 in January, 120 in July, `EST5EDT` -300 and
-240, `UTC0` 0),
`time_1_3_2_4_now_uto_carries_the_tdf_of_the_windows_zone` (Windows only:
without `TZ` equal to the offset Windows states for the same instants).

**Status:** done — the tests ran green on the Linux test host. Nothing ran on
Windows: `time_1_3_2_4_now_uto_carries_the_tdf_of_the_windows_zone` runs in
the weekly jobs `platform-windows-dcps-1` and `-2`. It tells a zone that was
not found only when the zone of the Windows machine has an offset in January
or July.

### §1.3.2.5 Type IntervalT — lower + upper bound

**Spec:** §1.3.2.5, p. 1-7 (PDF) — "two TimeT values corresponding to the
lower and upper bound of the interval. An IntervalT structure containing a
lower bound greater than the upper bound is invalid."

**Repo:** `crates/time-service/src/time_base.rs::IntervalT` with `IntervalT::new`
rejecting lower > upper (returns `None`).

**Tests:** `crates/time-service/src/time_base.rs::tests::intervalt_rejects_lower_greater_than_upper`,
`intervalt_size_is_16_octets`, `intervalt_wire_roundtrip_preserves_bounds`.

**Status:** done

### §1.3.2.6 Enum ComparisonType — IntervalC vs MidC

**Spec:** §1.3.2.6, p. 1-7 (PDF) — "ComparisonType defines the two types of
time comparison. IntervalC comparison does the comparison taking into account
the error envelope. MidC comparison just compares the base times. A MidC
comparison can never return TCIndeterminate."

**Repo:** `crates/time-service/src/uto.rs::ComparisonType`.

**Tests:** `crates/time-service/src/uto.rs::tests::compare_time_midc_equal`,
`::compare_time_midc_less_than`, `::compare_time_midc_greater_than`,
`::compare_time_intervalc_equal_when_inaccuracy_zero_and_time_match`,
`::compare_time_intervalc_indeterminate_on_envelope_overlap`,
`::compare_time_intervalc_less_than_when_envelopes_disjoint`,
`::compare_time_intervalc_greater_than_when_envelopes_disjoint`.

**Status:** done

### §1.3.2.7 Enum TimeComparison — Equal/Less/Greater/Indeterminate

**Spec:** §1.3.2.7, p. 1-8 (PDF) — "TCEqualTo, TCLessThan, TCGreaterThan,
TCIndeterminate. TCIndeterminate value is returned if the error envelopes
around the two times being compared overlap."

**Repo:** `crates/time-service/src/uto.rs::TimeComparison`.

**Tests:** `crates/time-service/src/uto.rs::tests::compare_time_intervalc_indeterminate_on_envelope_overlap`.

**Status:** done

### §1.3.2.8 Enum OverlapType — Container/Contained/Overlap/NoOverlap

**Spec:** §1.3.2.8, p. 1-8 (PDF) — four cases per Figure 1-3 (OTContainer,
OTContained, OTOverlap, OTNoOverlap).

**Repo:** `crates/time-service/src/tio.rs::OverlapType`.

**Tests:** `crates/time-service/src/tio.rs::tests::overlaps_otcontainer`, `overlaps_otcontained`,
`overlaps_partial`, `overlaps_no_overlap`.

**Status:** done

---

## §1.3.3 Exceptions

### §1.3.3.1 TimeUnavailable

**Spec:** §1.3.3.1, p. 1-8 (PDF) — "raised when the underlying trusted time
service fails, or is unable to provide time that meets the required security
assurance."

**Repo:** `crates/time-service/src/service.rs::TimeUnavailable`; over GIOP
(feature `serve`) the user exception
`IDL:omg.org/CosTime/TimeUnavailable:1.0` (`crates/time-service/src/servant.rs`).

**Tests:** `crates/time-service/src/service.rs::tests::time_a_1_secure_time_unavailable_on_a_service_that_is_not_administered`,
`time_unavailable_display_describes_failure_mode`;
`crates/time-service/src/servant.rs::tests::time_1_2_time_unavailable_is_the_cos_time_user_exception`;
`crates/time-service/tests/time_svc_e2e.rs::time_1_2_exceptions_reach_the_client`.

**Status:** done

---

## §1.3.4 Universal Time Object (UTO)

### §1.3.4.1 Readonly attribute time

**Spec:** §1.3.4.1, p. 1-9 (PDF) — "the time attribute of a UTO represented as
a value of type TimeT."

**Repo:** `crates/time-service/src/uto.rs::Uto::time()`.

**Tests:** `crates/time-service/src/uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.2 Readonly attribute inaccuracy

**Spec:** §1.3.4.2, p. 1-9 (PDF) — "of type InaccuracyT."

**Repo:** `crates/time-service/src/uto.rs::Uto::inaccuracy()`.

**Tests:** `crates/time-service/src/uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.3 Readonly attribute tdf

**Spec:** §1.3.4.3, p. 1-10 (PDF) — "time displacement factor [...] of type
TdfT."

**Repo:** `crates/time-service/src/uto.rs::Uto::tdf()`.

**Tests:** `crates/time-service/src/uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.4 Readonly attribute utc_time

**Spec:** §1.3.4.4, p. 1-10 (PDF) — "returns a properly populated UtcT
structure [...]."

**Repo:** `crates/time-service/src/uto.rs::Uto::utc_time()`.

**Tests:** `crates/time-service/src/uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.5 Operation absolute_time

**Spec:** §1.3.4.5, p. 1-10 (PDF) — "Absolute time = current time + time in the
object. Raises CORBA::DATA_CONVERSION exception if the attempt to obtain
absolute time causes an overflow."

**Repo:** `crates/time-service/src/uto.rs::Uto::absolute_time()` (platform
clock) and `crates/time-service/src/service.rs::TimeService::absolute_time`
(clock and zone of the service); overflow is
`AbsoluteTimeError::DataConversion` (`CORBA::DATA_CONVERSION`), a clock
without time `AbsoluteTimeError::TimeUnavailable`. The result carries the
local tdf (§1.3.2.4). Over GIOP (feature `serve`, `crates/time-service/src/servant.rs`): overflow is
`DATA_CONVERSION`, a clock without time `TRANSIENT`. Under the default policy
`RemoteTimePolicy::Refuse`, `absolute_time` of a served UTO raises
`NO_PERMISSION`: the result contains the current time of the service, which
must not go to another address space with an unknown latency (App. B.2). For
a remote caller this is a deviation from §1.3.4.5. With
`AnswerCorrectingProxies` the servant answers;
`crates/time-service/src/proxy.rs::TimeServiceProxy::absolute_time` measures
the round trip of `absolute_time` and `_get_utc_time` and widens time and
inaccuracy by half of it each.

**Tests:** `crates/time-service/src/uto.rs::tests::absolute_time_adds_current_to_relative`,
`time_1_3_4_5_absolute_time_overflow_is_data_conversion`;
`crates/time-service/src/service.rs::tests::time_1_3_4_5_absolute_time_on_the_service_clock`;
`crates/time-service/src/servant.rs::tests::time_1_2_uto_operations_compare_interval_absolute`,
`time_b_2_absolute_time_is_refused_by_default`;
`crates/time-service/src/proxy.rs::tests::time_b_2_absolute_time_correction_overflow_is_data_conversion`;
`crates/time-service/tests/time_svc_e2e.rs::time_b_2_the_server_gives_no_current_time_by_default`,
`time_b_2_the_proxy_widens_absolute_time_by_the_measured_latency`.

**Status:** done — the tests ran green on the Linux test host. In the address space of the service and over GIOP with `AnswerCorrectingProxies` as §1.3.4.5 says; over GIOP under `Refuse` the deviation stated above, following App. B.2.

### §1.3.4.6 Operation compare_time

**Spec:** §1.3.4.6, p. 1-10 (PDF) — "Compares the time contained in the object
with the time given in the input parameter uto using the comparison type
specified in the in parameter comparison_type."

**Repo:** `crates/time-service/src/uto.rs::Uto::compare_time(ct, uto)`.

**Tests:** `crates/time-service/src/uto.rs::tests::compare_time_midc_equal`,
`::compare_time_midc_less_than`, `::compare_time_midc_greater_than`,
`::compare_time_intervalc_equal_when_inaccuracy_zero_and_time_match`,
`::compare_time_intervalc_indeterminate_on_envelope_overlap`,
`::compare_time_intervalc_less_than_when_envelopes_disjoint`,
`::compare_time_intervalc_greater_than_when_envelopes_disjoint`.

**Status:** done

### §1.3.4.7 Operation time_to_interval

**Spec:** §1.3.4.7, p. 1-10 (PDF) — "Returns a TIO representing the time
interval between the time in the object and the time in the UTO [...]. The
interval returned is the interval between the midpoints of the two UTOs and
the inaccuracies in the UTOs are not taken into consideration."

**Repo:** `crates/time-service/src/uto.rs::Uto::time_to_interval(other)`.

**Tests:** `crates/time-service/src/uto.rs::tests::time_to_interval_uses_midpoints`.

**Status:** done

### §1.3.4.8 Operation interval

**Spec:** §1.3.4.8, p. 1-10 (PDF) — "TIO.upper_bound = UTO.time +
UTO.inaccuracy. TIO.lower_bound = UTO.time - UTO.inaccuracy."

**Repo:** `crates/time-service/src/uto.rs::Uto::interval()`.

**Tests:** `crates/time-service/src/uto.rs::tests::interval_returns_inaccuracy_envelope`.

**Status:** done

---

## §1.3.5 Time Interval Object (TIO)

### §1.3.5.1 Readonly attribute time_interval

**Spec:** §1.3.5.1, p. 1-11 (PDF) — "returns an IntervalT structure with the
values of its fields filled in [...]."

**Repo:** `crates/time-service/src/tio.rs::Tio::time_interval()`.

**Tests:** `crates/time-service/src/tio.rs::tests::time_interval_attribute_returns_constructor_value`.

**Status:** done

### §1.3.5.2 Operation spans

**Spec:** §1.3.5.2, p. 1-11 (PDF) — "Returns a value of type OverlapType
depending on how the interval in the object and the time range represented by
the parameter UTO overlap. [...] If OverlapType is not OTNoOverlap, then the
out parameter overlap contains the overlap interval, otherwise the out
parameter contains the gap between the two intervals."

**Repo:** `crates/time-service/src/tio.rs::Tio::spans(uto)`.

**Tests:** `crates/time-service/src/tio.rs::tests::spans_uses_uto_inaccuracy_envelope`.

**Status:** done

### §1.3.5.3 Operation overlaps

**Spec:** §1.3.5.3, p. 1-11 (PDF) — "Returns a value of type OverlapType
depending on how the interval in the object and interval in the parameter TIO
overlap."

**Repo:** `crates/time-service/src/tio.rs::Tio::overlaps(tio)`.

**Tests:** `crates/time-service/src/tio.rs::tests::overlaps_otcontainer`, `overlaps_otcontained`,
`overlaps_partial`, `overlaps_no_overlap`.

**Status:** done

### §1.3.5.4 Operation time

**Spec:** §1.3.5.4, p. 1-11 (PDF) — "Returns a UTO in which the inaccuracy
interval is equal to the time interval in the TIO and time value is the
midpoint of the interval."

**Repo:** `crates/time-service/src/tio.rs::Tio::time()`; if half the width
exceeds 48 bits, the result is `DataConversion` instead of a cut inaccuracy.

**Tests:** `crates/time-service/src/tio.rs::tests::time_method_returns_uto_with_midpoint_and_half_width`,
`time_1_3_5_4_interval_wider_than_48_bit_inaccuracy_is_data_conversion`.

**Status:** done

---

## §2.1 Time Service interface

### §2.1.1 Operation universal_time

**Spec:** §2.1.1, p. 2-2 (PDF) — "returns the current time and an estimate of
inaccuracy in a UTO. It raises TimeUnavailable exceptions to indicate failure
of an underlying time provider. The time returned in the UTO by this operation
is not guaranteed to be secure or trusted."

**Repo:** `crates/time-service/src/service.rs::TimeService::universal_time`:
the time of the clock with its inaccuracy, `TimeUnavailable` only when the
clock gives no time. Over GIOP (feature `serve`,
`crates/time-service/src/servant.rs`): with
`RemoteTimePolicy::AnswerCorrectingProxies` the same, `TimeUnavailable` as a
user exception. Under the default policy `Refuse` the served object raises
the system exception `NO_PERMISSION`, also when the clock gives a time. For a
remote caller this deviates from §2.1.1 ("If any time is available at all,
that time is returned by this operation"). The reason is App. B.2: a time
that arrives with an unknown latency must not reach the caller; the caller
uses a Time Service in its own address space (the first way of B.2).
`TimeUnavailable` is not used for this refusal; it keeps the meaning §2.1.1
gives it, the failure of the time provider (§1.2.1 rules out a
`universal_time` that always raises `TimeUnavailable`). Stated in
`crates/time-service/README.md`, "Proxies (B.2)".

**Tests:** `crates/time-service/src/service.rs::tests::universal_time_returns_recent_value`;
`crates/time-service/src/servant.rs::tests::time_1_2_universal_time_returns_a_uto_reference_with_the_value_in_its_key`,
`time_1_2_time_unavailable_is_the_cos_time_user_exception`,
`time_b_2_universal_time_is_refused_by_default`;
`crates/time-service/tests/time_svc_e2e.rs::time_b_2_the_server_gives_no_current_time_by_default`.

**Status:** done — the tests ran green on the Linux test host. In the address space of the service and over GIOP with `AnswerCorrectingProxies` as §2.1.1 says; over GIOP under `Refuse` the deviation stated above, following App. B.2.

### §2.1.2 Operation secure_universal_time

**Spec:** §2.1.2, p. 2-2 (PDF) — "returns the current time in a UTO only if the
time can be guaranteed to have been obtained securely. [...] If there is any
uncertainty at all about meeting any aspect of these criteria, then this
operation must return the TimeUnavailable exception."

**Repo:** `crates/time-service/src/service.rs::TimeService::secure_universal_time`
with `crates/time-service/src/admin.rs::Administration::secure_now`: a time only
for an administered service (`TimeService::administered`), from a source
designated through `TimeAdmin::designate_source` whose synchronization is
stated as protected (`SyncProtection`, not `Unprotected`), with no time set by
hand, from a synchronized reading with an error bound of at most 48 bits and
not before the last time returned; in every other case `TimeUnavailable`, also
without `std` and over GIOP (`crates/time-service/src/servant.rs`). The criteria and their handling:
`crates/time-service/README.md`, section "Secure time (Appendix A and B)".

**Tests:** `crates/time-service/src/service.rs::tests::time_a_1_secure_time_unavailable_on_a_service_that_is_not_administered`;
`crates/time-service/src/admin.rs::tests::time_a_1_secure_time_unavailable_without_designated_source`,
`time_a_1_secure_time_returns_source_time_and_inaccuracy`,
`time_a_1_secure_time_unavailable_when_source_reports_unsynchronized`,
`time_a_1_secure_time_unavailable_for_unprotected_synchronization`,
`time_a_1_secure_time_unavailable_when_reading_goes_backwards`,
`time_a_1_secure_time_unavailable_while_a_time_set_by_hand_is_in_effect`,
`time_a_1_secure_time_unavailable_when_the_bound_exceeds_48_bits`;
`crates/time-service/src/servant.rs::tests::time_b_2_secure_universal_time_never_leaves_the_address_space`;
`crates/time-service/tests/time_svc_e2e.rs::time_b_2_secure_time_stays_in_the_address_space_of_the_service`;
`crates/dcps/src/time_source.rs::tests::time_a_1_adjtimex_state_decides_secure_time_of_an_administered_service`,
`time_a_1_kernel_clock_as_authoritative_source`.

**Status:** done — the tests ran green on the Linux test host.

### §2.1.2.1 Operation new_universal_time

**Spec:** §2.1.2.1, p. 2-2 (PDF) — "constructing a new UTO. The parameters
passed in are the time of type TimeT and inaccuracy of type InaccuracyT.
[...] CORBA::BAD_PARAM is raised in the case of an out-of-range parameter
value for inaccuracy."

**Repo:** `crates/time-service/src/service.rs::TimeService::new_universal_time`
returns `Result<Uto, BadParam>`: an inaccuracy above 48 bits
(`MAX_INACCURACY`) is `BadParam` (`CORBA::BAD_PARAM`).

**Tests:** `crates/time-service/src/service.rs::tests::new_universal_time_creates_uto_from_components`,
`time_2_1_2_1_inaccuracy_above_48_bits_is_bad_param`,
`time_2_1_2_1_inaccuracy_at_48_bit_limit_is_accepted`;
`crates/time-service/src/uto.rs::tests::time_2_1_2_1_uto_new_rejects_inaccuracy_above_48_bits`.

**Status:** done

### §2.1.2.2 Operation uto_from_utc

**Spec:** §2.1.2.2, p. 2-2 (PDF) — "create a UTO given a time in the UtcT form.
[...] used to convert a UTC received over the wire into a UTO."

**Repo:** `crates/time-service/src/service.rs::TimeService::uto_from_utc`.

**Tests:** `crates/time-service/src/service.rs::tests::uto_from_utc_wraps_passed_struct`.

**Status:** done

### §2.1.2.3 Operation new_interval

**Spec:** §2.1.2.3, p. 2-3 (PDF) — "If the value of the lower parameter is
greater than the value of the upper parameter, then a CORBA::BAD_PARAM
exception is raised."

**Repo:** `crates/time-service/src/service.rs::TimeService::new_interval`
(`BadParam` on lower > upper).

**Tests:** `crates/time-service/src/service.rs::tests::new_interval_rejects_lower_greater_than_upper`,
`new_interval_creates_tio_for_valid_bounds`.

**Status:** done

---

## §2.2 Timer Event Service

### §2.2.1 Object model — TimerEventService manages TimerEventHandlers

**Spec:** §2.2.1, p. 2-3 (PDF) — "The TimerEventService object manages Timer
Event Handlers. [...] Each Timer Event Handler is immutably associated with a
specific event channel at the time of its creation."

**Repo:** `crates/time-service/src/timer_event.rs::TimerEventService`
manages the `TimerEventHandler`s: `register` binds each handler to the given
`PushConsumer` for good (no operation changes it), `unregister` releases it,
a handler of another or of a finished service is `INV_OBJREF`. One worker
thread per service triggers the events.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_4_1_register_starts_cleared_and_schedules_nothing`,
`time_2_4_2_unregister_then_use_is_inv_objref`,
`time_2_4_2_handler_of_another_service_is_inv_objref`.

**Status:** done

### §2.2.2 Usage — push event channel + timer events

**Spec:** §2.2.2, p. 2-4 (PDF) — workflow: create an event channel, register a
TimerEventHandler, set a timer, push events over the channel.

**Repo:** push model: an event goes as `AnyEvent` with the repository id
`IDL:omg.org/CosTimerEvent/TimerEventT:1.0` to
`zerodds_corba_cos_event::comm::PushConsumer`, also to the
`ProxyPushConsumer` of an `EventChannel`. The expected interval is
documented in the module documentation of
`crates/time-service/src/timer_event.rs` and in the crate's README: never
before the requested time, after it by the wake-up latency of the operating
system and the duration of pushes due before it; the clock is read again at
least once a second. Measured on the Linux test host: at most 68 µs delay
over 20 triggers of 5 ms. Every `TimerEventT` carries the actual time and
the delay as inaccuracy.

**Tests:** `crates/time-service/tests/contract_corba_cos_event.rs::time_2_2_2_timer_events_flow_through_a_push_event_channel`;
`crates/time-service/src/timer_event.rs::tests::time_2_2_2_trigger_delay_is_reported_and_bounded`,
`time_2_3_1_2_set_timer_absolute_fires_at_the_time`.

**Status:** done

### §2.2.3 Data types CosTimerEvent

**Spec:** §2.2.3, p. 2-4 (PDF) — `module CosTimerEvent { enum TimeType {
TTAbsolute, TTRelative, TTPeriodic }; enum EventStatus { ESTimeSet,
ESTimeCleared, ESTriggered, ESFailedTrigger }; struct TimerEventT {
TimeBase::UtcT utc; any event_data; }; };`.

**Repo:** `crates/time-service/src/timer_event.rs::{TimeType, EventStatus,
TimerEventT}`; `TimerEventT` is `{ utc: UtcT, event_data: AnyEvent }`,
`AnyEvent` the `any` of `zerodds-corba-cos-event` (repository id and CDR
encapsulation). `TimerEventT::to_any` writes the struct as a CDR
encapsulation (`UtcT`, then the `any` as `string` and `sequence<octet>`);
`TimerEventT::from_any` reads both byte orders and rejects a wrong
repository id, lengths past the end and bytes left over with `Marshal`.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_2_3_timer_event_t_any_roundtrip_both_byte_orders`,
`time_2_2_3_timer_event_t_malformed_any_is_marshal`,
`time_2_2_3_3_inaccuracy_is_requested_minus_actual`.

**Status:** done

### §2.2.3.1 Enum TimeType

**Spec:** §2.2.3.1, p. 2-4 (PDF) — TTAbsolute / TTRelative / TTPeriodic.

**Repo:** `crates/time-service/src/timer_event.rs::TimeType::{Absolute,
Relative, Periodic}`; `TimerEventHandler::set_timer` evaluates all three
(§2.3.1.2).

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_3_1_2_set_timer_absolute_fires_at_the_time`,
`time_2_3_1_2_set_timer_relative_zero_reuses_last`,
`time_2_2_3_2_periodic_never_triggered_stays_time_set`.

**Status:** done

### §2.2.4 Exceptions

**Spec:** §2.2.4, p. 2-5 (PDF) — exception definitions for the
TimerEventService.

**Repo:** no service-specific exceptions.
`crates/time-service/src/timer_event.rs::TimerEventError` names the CORBA
system exceptions `BAD_PARAM` (§2.3.1.2), `NO_RESOURCES` (§2.4.1),
`INV_OBJREF` (§2.4.2) and the case of a clock without time.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_3_1_2_set_timer_relative_zero_without_previous_is_bad_param`,
`time_2_4_1_register_beyond_capacity_is_no_resources`,
`time_2_4_2_unregister_then_use_is_inv_objref`.

**Status:** done

---

## §2.3 Timer Event Handler

### §2.3.1 Attribute status

**Spec:** §2.3, p. 2-5/2-6 (PDF) — the `TimerEventHandler` interface with the
attribute `status` and the operations `set_timer/cancel_timer/set_data/time_set`.

**Repo:** `crates/time-service/src/timer_event.rs::TimerEventHandler` with
`status`, `time_set`, `set_timer`, `cancel_timer`, `set_data`. `status` has
the four states of §2.2.3.2: `TimeCleared` after `register` and after
`cancel_timer`, `TimeSet` after `set_timer` and after every periodic trigger,
`Triggered` after a delivered one-shot trigger, `FailedTrigger` when the push
fails with `Disconnected` (a periodic handler stops then). `time_set` returns
the flag and the current timer value as a UTO. `set_timer` cancels the
previous trigger; absolute at the given time, relative at the current time
plus the value, periodic at that interval; a relative or periodic value of 0
takes the last relative time, without one it is `BAD_PARAM`, as is a time
beyond `TimeT`. `cancel_timer` returns `true` only for a pending trigger.
`set_data` applies to all later triggers.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_2_3_2_status_triggered_after_one_shot`,
`time_2_2_3_2_status_failed_trigger_when_push_fails`,
`time_2_2_3_2_periodic_never_triggered_stays_time_set`,
`time_2_3_1_1_time_set_returns_flag_and_uto`,
`time_2_3_1_2_set_timer_relative_zero_reuses_last`,
`time_2_3_1_2_set_timer_relative_zero_without_previous_is_bad_param`,
`time_2_3_1_2_set_timer_cancels_the_previous_trigger`,
`time_2_3_1_2_set_timer_overflow_is_bad_param`,
`time_2_3_1_3_cancel_timer_returns_true_only_for_a_pending_trigger`,
`time_2_3_1_4_set_data_reaches_the_event`.

**Status:** done

---

## §2.4 Timer Event Service operations

### §2.4.1 Operation register

**Spec:** §2.4.1, p. 2-7 (PDF) — `TimerEventHandler register(in
CosEventComm::PushConsumer event_interface, in any data);`.

**Repo:** `crates/time-service/src/timer_event.rs::TimerEventService::register`
takes `Arc<dyn PushConsumer>` and `AnyEvent`: the handler's time is the
current time, its state `TimeCleared`, nothing scheduled. Beyond the handler
limit (`DEFAULT_MAX_HANDLERS` or `TimerEventService::with_capacity`) the
result is `NO_RESOURCES`, as it is when the worker thread does not start.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_4_1_register_starts_cleared_and_schedules_nothing`,
`time_2_4_1_register_beyond_capacity_is_no_resources`;
`crates/time-service/tests/contract_corba_cos_event.rs::contract_push_consumer_receives_the_timer_event_as_any`.

**Status:** done

### §2.4.2 Operation unregister

**Spec:** §2.4.2, p. 2-7 (PDF) — `void unregister(in TimerEventHandler);`.

**Repo:** `crates/time-service/src/timer_event.rs::TimerEventService::unregister`
removes the handler and its pending trigger; every later use of the handler
is `INV_OBJREF`.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_4_2_unregister_then_use_is_inv_objref`,
`time_2_4_2_unregister_cancels_the_pending_trigger`,
`time_2_4_2_handler_of_another_service_is_inv_objref`.

**Status:** done

### §2.4.3 Operation event_time

**Spec:** §2.4.3, p. 2-7 (PDF) — `UTO event_time(in CosTimerEvent::TimerEventT
event);`.

**Repo:** `crates/time-service/src/timer_event.rs::TimerEventService::event_time`
returns a UTO with the trigger time and the inaccuracy of the event.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_4_3_event_time_returns_uto`.

**Status:** done

---

## §2.5 Conformance

### §2.5 Conformance statement

**Spec:** §2.5, p. 2-7 (PDF) — two conformance points (Basic Time Service +
Timer Event Service); the Timer Event Service requires the Basic Time Service.

**Repo:** Basic Time Service (§1.3, §2.1) in `crates/time-service/`;
Timer Event Service (§2.2 to §2.4) in
`crates/time-service/src/timer_event.rs`, on the `TimeService` of the same
crate.

**Tests:** Basic Time Service: `crates/time-service/src/service.rs::tests`,
`crates/time-service/src/uto.rs::tests`,
`crates/time-service/src/tio.rs::tests`,
`crates/time-service/src/time_base.rs::tests` — among them
`utct_wire_roundtrip_preserves_all_fields`, `time_1_3_2_2_utct_rejects_inaccuracy_above_48_bits`,
`local_time_applies_tdf`, `spans_uses_uto_inaccuracy_envelope`.
Timer Event Service: `crates/time-service/src/timer_event.rs::tests::time_2_2_3_2_status_triggered_after_one_shot`,
`time_2_3_1_2_set_timer_relative_zero_reuses_last`;
`crates/time-service/tests/contract_corba_cos_event.rs::time_2_2_2_timer_events_flow_through_a_push_event_channel`.
Statement on App. A: `crates/time-service/src/secure.rs::tests::time_a_1_readme_states_every_criterion`.

**Status:** partial — the Timer Event Service is complete as a Rust API, not reachable over GIOP (§1.2). Basic Time Service: met; the statement how the criteria of App. A are handled is in `crates/time-service/README.md` ("Secure time (Appendix A and B)"), and `secure_universal_time` returns a time only under these criteria (§2.1.2, App. A).

---

## Appendix A — Implementation guidelines

### App. A — Secure time-source criteria

**Spec:** App. A, p. A-1 (PDF) — "Appropriate documented handling of the criteria presented here is mandatory for conformance to the Basic Time Service conformance point." A.1: "If these criteria are not satisfactorily addressed in an ORB, then it must return the TimeUnavailable exception upon invocation of the secure_universal_time operation".

**Repo:** Documented handling: `crates/time-service/README.md`, section
"Secure time (Appendix A and B)" (criteria 1 to 8 with the place in the spec,
the handling and a test; secure yes/no), in short in
`crates/time-service/src/lib.rs`. A.1 announces criteria without listing them
(p. A-2 is empty); the table takes those of §1.1.3, §2.1.2 and App. B. Checked
at every call: `crates/time-service/src/admin.rs::Administration::secure_now`;
sources: `crates/time-service/src/secure.rs::{SecureTimeSource, SourceReading, SyncProtection, SourceDesignation}`,
on Linux `crates/dcps/src/time_source.rs::KernelClock`. Without the criteria
met, `secure_universal_time` raises `TimeUnavailable` (A.1).

**Tests:** `crates/time-service/src/secure.rs::tests::time_a_1_readme_states_every_criterion`;
`crates/time-service/src/service.rs::tests::time_a_1_secure_time_unavailable_on_a_service_that_is_not_administered`;
`crates/time-service/src/admin.rs::tests::time_a_1_secure_time_unavailable_without_designated_source`,
`time_a_1_secure_time_returns_source_time_and_inaccuracy`,
`time_a_1_secure_time_unavailable_when_source_reports_unsynchronized`,
`time_a_1_secure_time_unavailable_for_unprotected_synchronization`,
`time_a_1_secure_time_unavailable_when_reading_goes_backwards`,
`time_a_1_secure_time_unavailable_while_a_time_set_by_hand_is_in_effect`,
`time_a_1_secure_time_unavailable_when_the_bound_exceeds_48_bits`;
`crates/dcps/src/time_source.rs::tests::time_a_1_kernel_reading_follows_the_adjtimex_state`,
`time_a_1_adjtimex_state_decides_secure_time_of_an_administered_service`,
`time_a_1_kernel_clock_reads_the_host_clock`,
`time_a_1_kernel_clock_as_authoritative_source`.

**Status:** done — the tests ran green on the Linux test host. A synchronized and an unsynchronized kernel clock are checked by `time_a_1_adjtimex_state_decides_secure_time_of_an_administered_service` with fixed `adjtimex` values; `time_a_1_kernel_clock_as_authoritative_source` checks only the case the host gives and prints it. Measured on the Linux test host: a probe with `adjtimex(2)` gave `TIME_OK`, `maxerror` 108 369 µs; a later run of the test printed: synchronized, `maxerror` 173 875 µs, secure time given. Whether the synchronization of the host is protected in transit is not checked by ZeroDDS: the administrator states it when designating the source (`SyncProtection`), the audit record keeps who did.

---

## Appendix B-F

### App. B — Administration

**Spec:** App. B, p. B-1 (PDF) — "Only administrators authorized by the system security policy may set the time and specify the source of time"; operations that set the time, designate a source as authoritative or change the accuracy "must be protected against unauthorized invocation. They must also be mandatorily audited"; "Time Service implementors must state whether or not their implementation is secure."

**Repo:** B.1: `crates/time-service/src/admin.rs::{TimeAdmin, AdminAuthorizer, DenyAll, AdminSubject, AdminOp, AuditSink, AuditRecord, AuditOutcome, AdminError}`.
`TimeService::administered` returns the service with its `TimeAdmin`;
`set_time`, `reset_time`, `designate_source`, `withdraw_source` and
`set_inaccuracy` run one at a time: authorization by the `AdminAuthorizer`
(the security policy; `DenyAll` refuses everything), check of the
parameters, the audit record (granted, denied, failed; time, subject,
operation, value before and after), only then the change; if the record cannot
be written, nothing changes. The fields are private; `with_tdf` and
`with_inaccuracy` on an administered service give a new service that is not
administered. JSON lines: `crates/security-logging/src/time_audit.rs` (feature
`time-service`; `JsonLinesLoggingPlugin::append_synced` writes past the level
filter and with `sync_data`). B.1.1 and B.2: `crates/time-service/README.md`,
section "Secure time (Appendix A and B)" (secure yes/no, protection of the
synchronization values in transit, the six threats of CORBA Security Service
1.8 §1.1.3, proxies). B.2: `crates/time-service/src/servant.rs::RemoteTimePolicy`.
Default `Refuse`: no current time of the service over GIOP; the two
operations that read its clock, `universal_time` and `absolute_time` of a UTO
("current time + time in the object", §1.3.4.5), raise the system exception
`NO_PERMISSION` (`REFUSED_REMOTE_TIME`); the operations without a clock are
served. `secure_universal_time` over GIOP always `TimeUnavailable`.
`AnswerCorrectingProxies` answers `universal_time` and `absolute_time` for
callers that correct the latency themselves:
`crates/time-service/src/proxy.rs::TimeServiceProxy::{universal_time, absolute_time}`
measures the round trip on the monotonic clock and widens time and
inaccuracy by half of it each; the proxy's `secure_universal_time` never
gives a time. The deviations from §2.1.1 and §1.3.4.5 under `Refuse` are
stated there and in the README.

**Tests:** `crates/time-service/src/admin.rs::tests::time_b_1_default_authorizer_denies_every_operation`,
`time_b_1_set_time_unauthorized_is_refused_and_audited`,
`time_b_1_set_time_authorized_shifts_universal_time_and_is_audited`,
`time_b_1_reset_time_restores_the_source_reading`,
`time_b_1_designate_source_is_audited_before_it_takes_effect`,
`time_b_1_set_inaccuracy_changes_generated_timestamps_and_is_audited`,
`time_b_1_audit_failure_refuses_the_operation`,
`time_b_1_with_methods_on_an_administered_service_leave_the_administration`;
`crates/time-service/src/secure.rs::tests::time_b_1_1_only_unprotected_synchronization_is_not_protected`,
`time_a_1_readme_states_every_criterion`;
`crates/security-logging/src/time_audit.rs::tests::time_b_1_audit_record_reaches_the_jsonl_log`,
`time_b_1_a_full_audit_log_refuses_the_operation`;
`crates/time-service/src/servant.rs::tests::time_b_2_universal_time_is_refused_by_default`,
`time_b_2_absolute_time_is_refused_by_default`,
`time_b_2_secure_universal_time_never_leaves_the_address_space`;
`crates/time-service/src/proxy.rs::tests::time_b_2_correction_widens_by_half_the_round_trip_each_side`,
`time_b_2_correction_beyond_48_bits_is_time_unavailable`,
`time_b_2_absolute_time_correction_overflow_is_data_conversion`;
`crates/time-service/tests/time_svc_e2e.rs::time_b_2_the_server_gives_no_current_time_by_default`,
`time_b_2_secure_time_stays_in_the_address_space_of_the_service`,
`time_b_2_the_proxy_widens_the_interval_by_the_measured_latency`,
`time_b_2_the_proxy_widens_absolute_time_by_the_measured_latency`;
`crates/time-service/tests/omniorb_e2e.rs::time_1_2_an_omniorb_client_uses_the_time_service`.

**Status:** done — the tests ran green on the Linux test host. `omniorb_e2e` ran on the Linux test host in a Debian trixie container (omniidl and libomniorb4-dev 4.3.3, g++ 14.2), 3 of 3 green: under `RemoteTimePolicy::Refuse` the omniORB client gets the system exception `NO_PERMISSION` for `universal_time` and for `absolute_time` of a UTO of time 0, under `AnswerCorrectingProxies` the time, and `TimeUnavailable` for `secure_universal_time` in both cases; registered in the CI job `integration-bridges-vendor-corba`, not run there. Whether the synchronization of the host is protected in transit is not checked by ZeroDDS (B.1.1: the administrator states it).

### App. C — IDL listing

**Spec:** App. C (PDF) — IDL for TimeBase + CosTime + CosTimerEvent
(non-normative as an appendix; normatively equivalent definitions are in §1.3
+ §2.x).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — consolidation of the IDL of §1.3 and §2; the definitions are items there.

### App. D — Notes

**Spec:** App. D (PDF) — implementation notes (non-normative).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — notes for users.

### App. E — Examples

**Spec:** App. E (PDF) — example code (non-normative).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — extension examples.

### App. F — References

**Spec:** App. F (PDF) — external references (non-normative).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reference list.

---

## Audit status

42 done / 3 partial / 0 open / 0 blocked / 4 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document omg-time-1.1.md`):

* `cargo nextest run --profile merge -p zerodds-corba-ccm --all-targets` (job `merge-rust-corba-ccm`) — 17 tests green.
* `cargo nextest run --profile merge -p zerodds-time-service --all-targets` (job `merge-rust-time-service`) — 35 tests green.

52 cited tests green, 0 not green.

Open items: `omg-time-1.1.open.en.md`.
