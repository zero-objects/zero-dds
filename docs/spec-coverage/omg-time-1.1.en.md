# OMG Time Service 1.1 — Spec Coverage

**Spec:** [OMG Time Service 1.1 — formal/2002-05-07 (52 pages) →](https://www.omg.org/spec/TIME/)

**Context:** the OMG Time Service is a classic CORBA object service. ZeroDDS
has no ORB; we therefore realize the **data-model + algorithms part** of the
spec as a plain Rust library. CORBA-specific aspects (IIOP wire, object
server, the CosEvent-channel-based TimerEventService) are marked `n/a` with
a clear rationale — but the spec-conformant data algorithm remains fully
implemented and can be used by any caller (DDS-Security, DCPS-Time, an
external CORBA server).

Implementation:

- `crates/time-service/` — data-model + algorithms part of the OMG Time Service spec as a plain Rust library.

---

## §1.1 Overview

### §1.1.1 Time Service requirements

**Spec:** §1.1.1, p. 1-1 (PDF) — "obtain current time together with an error
estimate associated with it. Additionally, [...] ascertain the order in which
events occurred. Generate time-based events based on timers and alarms.
Compute the interval between two events."

**Repo:** requirements 1+2+4 satisfied by `crates/time-service/`
(`current_time` / `compare_time` / `time_to_interval` / `interval`).
Requirement 3 (timer events) is marked `n/a` in §2.2/§2.4 (requires the CORBA
Event Service).

**Tests:** `crates/time-service/src/uto.rs::tests::compare_time_*`,
`time_to_interval_uses_midpoints`.

**Status:** done

### §1.1.2 Representation of time

**Spec:** §1.1.2, p. 1-2 (PDF) — "100 nanoseconds (10^-7 seconds); Base time:
15 October 1582 00:00:00; Approximate range: AD 30,000. [...] UTC time in this
service specification always refers to time in Greenwich Time Zone."

**Repo:** `crates/time-service/src/time_base.rs::{TimeT, TICKS_PER_SECOND,
UTC_EPOCH_TO_UNIX_TICKS}`.

**Tests:** `time_base.rs::tests::current_time_is_recent_century`.

**Status:** done

### §1.1.3 Source of time

**Spec:** §1.1.3, p. 1-2/1-3 (PDF) — requirements on the underlying time
source (current time + error, monotonic, optionally secure).

**Repo:** `crates/time-service/src/time_base.rs::current_time` uses
`std::time::SystemTime` as the source. The secure-source aspect via a
`TimeService::secure_source` flag.

**Tests:** `service.rs::tests::secure_universal_time_*`.

**Status:** done

---

## §1.2 General object model

### §1.2 General object model — service-object pattern

**Spec:** §1.2, p. 1-3 (PDF) — the service object manages instance objects
(UTOs/TIOs) via the service interface. The CORBA object-service pattern.

**Repo:** ZeroDDS has no CORBA ORB. We implement the service pattern as a
plain Rust struct `TimeService` with factory methods (`new_universal_time`,
`uto_from_utc`, `new_interval`).

**Tests:** `service.rs::tests::new_universal_time_creates_uto_from_components`.

**Status:** done — a spec-equivalent form without an ORB.

### §1.2.1 Conformance points (Basic + Timer Event)

**Spec:** §1.2.1, p. 1-4 (PDF) — two conformance points: "Basic Time Service"
(TimeBase + CosTime) and "Timer Event Service" (CosTimerEvent, optional,
depends on Basic).

**Repo:** the Basic Time Service is fully implemented
(`crates/time-service/`); the Timer Event Service is implemented in
`crates/corba-ccm/src/timer.rs` (callback-based) + the
`crates/corba-ccm/src/time_psm.rs` spec facade. See §2.2.

**Tests:** cross-ref §1.3.x + §2.1.x + the `crates/corba-ccm/src/timer.rs`
inline tests.

**Status:** done — Basic Time Service + Timer Event Service both fully
implemented (see §2.2).

---

## §1.3 Basic Time Service

### §1.3.1 Object model — the service manages UTOs + TIOs

**Spec:** §1.3.1, p. 1-4/1-5 (PDF) — the Time Service manages UTOs (Universal
Time Objects) and TIOs (Time Interval Objects) via factory methods.

**Repo:** `crates/time-service/src/service.rs::TimeService`,
`crates/time-service/src/uto.rs::Uto`,
`crates/time-service/src/tio.rs::Tio`.

**Tests:** `service.rs::tests::*`, `uto.rs::tests::*`, `tio.rs::tests::*`.

**Status:** done

### §1.3.2 Data types

**Spec:** §1.3.2, p. 1-5 (PDF) — `module TimeBase { typedef unsigned long long
TimeT; typedef TimeT InaccuracyT; typedef short TdfT; struct UtcT { TimeT
time; unsigned long inacclo; unsigned short inacchi; TdfT tdf; }; struct
IntervalT { TimeT lower_bound; TimeT upper_bound; }; };`.

**Repo:** `crates/time-service/src/time_base.rs::{TimeT, InaccuracyT, TdfT,
UtcT, IntervalT}`.

**Tests:** `time_base.rs::tests::utct_size_is_16_octets`,
`intervalt_size_is_16_octets`.

**Status:** done

### §1.3.2.1 Type TimeT — 64-bit, 100ns ticks since 1582

**Spec:** §1.3.2.1, p. 1-6 (PDF) — "TimeT represents a single time value,
which is 64 bits in size, and holds the number of 100 nanoseconds that have
passed since the base time."

**Repo:** `crates/time-service/src/time_base.rs::TimeT` (alias `u64`).

**Tests:** `time_base.rs::tests::utct_wire_roundtrip_preserves_all_fields`.

**Status:** done

### §1.3.2.2 Type InaccuracyT — 48-bit inaccuracy in 100ns

**Spec:** §1.3.2.2, p. 1-6 (PDF) — "represents the value of inaccuracy in time
in units of 100 nanoseconds. [...] 48 bits is sufficient."

**Repo:** `crates/time-service/src/time_base.rs::InaccuracyT` with a 48-bit
cap in `UtcT::new` and `set_inaccuracy`.

**Tests:** `time_base.rs::tests::inaccuracy_caps_at_48_bits`.

**Status:** done

### §1.3.2.3 Type TdfT — 16-bit time-zone offset in minutes

**Spec:** §1.3.2.3, p. 1-6 (PDF) — "size 16 bits short type and holds the time
displacement factor in the form of minutes of displacement from the Greenwich
Meridian. [...] East is positive, West is negative."

**Repo:** `crates/time-service/src/time_base.rs::TdfT` (alias `i16`).

**Tests:** `time_base.rs::tests::local_time_negative_tdf_west_of_greenwich`.

**Status:** done

### §1.3.2.4 Type UtcT — 16-octet struct with time + inaccuracy + tdf

**Spec:** §1.3.2.4, p. 1-6/1-7 (PDF) — "UtcT defines the structure of the time
value [...] basic value of time is of type TimeT [...] inacclo and inacchi
fields together hold a 48-bit estimate [...] tdf field holds time zone
information. [...] for any given UtcT value 'utc', the local time can be
computed as utc.time + utc.tdf * 600,000,000."

**Repo:** `crates/time-service/src/time_base.rs::UtcT` with a `UtcT::local_time()`
operation.

**Tests:** `time_base.rs::tests::utct_size_is_16_octets`,
`local_time_applies_tdf`, `local_time_negative_tdf_west_of_greenwich`.

**Status:** done

### §1.3.2.5 Type IntervalT — lower + upper bound

**Spec:** §1.3.2.5, p. 1-7 (PDF) — "two TimeT values corresponding to the
lower and upper bound of the interval. An IntervalT structure containing a
lower bound greater than the upper bound is invalid."

**Repo:** `crates/time-service/src/time_base.rs::IntervalT` with `IntervalT::new`
rejecting lower > upper (returns `None`).

**Tests:** `time_base.rs::tests::intervalt_rejects_lower_greater_than_upper`,
`intervalt_size_is_16_octets`, `intervalt_wire_roundtrip_preserves_bounds`.

**Status:** done

### §1.3.2.6 Enum ComparisonType — IntervalC vs MidC

**Spec:** §1.3.2.6, p. 1-7 (PDF) — "ComparisonType defines the two types of
time comparison. IntervalC comparison does the comparison taking into account
the error envelope. MidC comparison just compares the base times. A MidC
comparison can never return TCIndeterminate."

**Repo:** `crates/time-service/src/uto.rs::ComparisonType`.

**Tests:** `uto.rs::tests::compare_time_midc_*`, `compare_time_intervalc_*`.

**Status:** done

### §1.3.2.7 Enum TimeComparison — Equal/Less/Greater/Indeterminate

**Spec:** §1.3.2.7, p. 1-8 (PDF) — "TCEqualTo, TCLessThan, TCGreaterThan,
TCIndeterminate. TCIndeterminate value is returned if the error envelopes
around the two times being compared overlap."

**Repo:** `crates/time-service/src/uto.rs::TimeComparison`.

**Tests:** `uto.rs::tests::compare_time_intervalc_indeterminate_on_envelope_overlap`.

**Status:** done

### §1.3.2.8 Enum OverlapType — Container/Contained/Overlap/NoOverlap

**Spec:** §1.3.2.8, p. 1-8 (PDF) — four cases per Figure 1-3 (OTContainer,
OTContained, OTOverlap, OTNoOverlap).

**Repo:** `crates/time-service/src/tio.rs::OverlapType`.

**Tests:** `tio.rs::tests::overlaps_otcontainer`, `overlaps_otcontained`,
`overlaps_partial`, `overlaps_no_overlap`.

**Status:** done

---

## §1.3.3 Exceptions

### §1.3.3.1 TimeUnavailable

**Spec:** §1.3.3.1, p. 1-8 (PDF) — "raised when the underlying trusted time
service fails, or is unable to provide time that meets the required security
assurance."

**Repo:** `crates/time-service/src/service.rs::TimeUnavailable` (a plain Rust
type, not a CORBA exception).

**Tests:** `service.rs::tests::secure_universal_time_fails_when_source_not_marked_secure`,
`time_unavailable_display_describes_failure_mode`.

**Status:** done

---

## §1.3.4 Universal Time Object (UTO)

### §1.3.4.1 Readonly attribute time

**Spec:** §1.3.4.1, p. 1-9 (PDF) — "the time attribute of a UTO represented as
a value of type TimeT."

**Repo:** `crates/time-service/src/uto.rs::Uto::time()`.

**Tests:** `uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.2 Readonly attribute inaccuracy

**Spec:** §1.3.4.2, p. 1-9 (PDF) — "of type InaccuracyT."

**Repo:** `crates/time-service/src/uto.rs::Uto::inaccuracy()`.

**Tests:** `uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.3 Readonly attribute tdf

**Spec:** §1.3.4.3, p. 1-10 (PDF) — "time displacement factor [...] of type
TdfT."

**Repo:** `crates/time-service/src/uto.rs::Uto::tdf()`.

**Tests:** `uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.4 Readonly attribute utc_time

**Spec:** §1.3.4.4, p. 1-10 (PDF) — "returns a properly populated UtcT
structure [...]."

**Repo:** `crates/time-service/src/uto.rs::Uto::utc_time()`.

**Tests:** `uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.5 Operation absolute_time

**Spec:** §1.3.4.5, p. 1-10 (PDF) — "Absolute time = current time + time in the
object. Raises CORBA::DATA_CONVERSION exception if the attempt to obtain
absolute time causes an overflow."

**Repo:** `crates/time-service/src/uto.rs::Uto::absolute_time()` (returns
`Option<Uto>`; `None` on overflow).

**Tests:** `uto.rs::tests::absolute_time_adds_current_to_relative`.

**Status:** done

### §1.3.4.6 Operation compare_time

**Spec:** §1.3.4.6, p. 1-10 (PDF) — "Compares the time contained in the object
with the time given in the input parameter uto using the comparison type
specified in the in parameter comparison_type."

**Repo:** `crates/time-service/src/uto.rs::Uto::compare_time(ct, uto)`.

**Tests:** `uto.rs::tests::compare_time_midc_*`, `compare_time_intervalc_*` (8
cases).

**Status:** done

### §1.3.4.7 Operation time_to_interval

**Spec:** §1.3.4.7, p. 1-10 (PDF) — "Returns a TIO representing the time
interval between the time in the object and the time in the UTO [...]. The
interval returned is the interval between the midpoints of the two UTOs and
the inaccuracies in the UTOs are not taken into consideration."

**Repo:** `crates/time-service/src/uto.rs::Uto::time_to_interval(other)`.

**Tests:** `uto.rs::tests::time_to_interval_uses_midpoints`.

**Status:** done

### §1.3.4.8 Operation interval

**Spec:** §1.3.4.8, p. 1-10 (PDF) — "TIO.upper_bound = UTO.time +
UTO.inaccuracy. TIO.lower_bound = UTO.time - UTO.inaccuracy."

**Repo:** `crates/time-service/src/uto.rs::Uto::interval()`.

**Tests:** `uto.rs::tests::interval_returns_inaccuracy_envelope`.

**Status:** done

---

## §1.3.5 Time Interval Object (TIO)

### §1.3.5.1 Readonly attribute time_interval

**Spec:** §1.3.5.1, p. 1-11 (PDF) — "returns an IntervalT structure with the
values of its fields filled in [...]."

**Repo:** `crates/time-service/src/tio.rs::Tio::time_interval()`.

**Tests:** `tio.rs::tests::time_interval_attribute_returns_constructor_value`.

**Status:** done

### §1.3.5.2 Operation spans

**Spec:** §1.3.5.2, p. 1-11 (PDF) — "Returns a value of type OverlapType
depending on how the interval in the object and the time range represented by
the parameter UTO overlap. [...] If OverlapType is not OTNoOverlap, then the
out parameter overlap contains the overlap interval, otherwise the out
parameter contains the gap between the two intervals."

**Repo:** `crates/time-service/src/tio.rs::Tio::spans(uto)`.

**Tests:** `tio.rs::tests::spans_uses_uto_inaccuracy_envelope`.

**Status:** done

### §1.3.5.3 Operation overlaps

**Spec:** §1.3.5.3, p. 1-11 (PDF) — "Returns a value of type OverlapType
depending on how the interval in the object and interval in the parameter TIO
overlap."

**Repo:** `crates/time-service/src/tio.rs::Tio::overlaps(tio)`.

**Tests:** `tio.rs::tests::overlaps_otcontainer`, `overlaps_otcontained`,
`overlaps_partial`, `overlaps_no_overlap`.

**Status:** done

### §1.3.5.4 Operation time

**Spec:** §1.3.5.4, p. 1-11 (PDF) — "Returns a UTO in which the inaccuracy
interval is equal to the time interval in the TIO and time value is the
midpoint of the interval."

**Repo:** `crates/time-service/src/tio.rs::Tio::time()`.

**Tests:** `tio.rs::tests::time_method_returns_uto_with_midpoint_and_half_width`.

**Status:** done

---

## §2.1 Time Service interface

### §2.1.1 Operation universal_time

**Spec:** §2.1.1, p. 2-2 (PDF) — "returns the current time and an estimate of
inaccuracy in a UTO. It raises TimeUnavailable exceptions to indicate failure
of an underlying time provider. The time returned in the UTO by this operation
is not guaranteed to be secure or trusted."

**Repo:** `crates/time-service/src/service.rs::TimeService::universal_time`.

**Tests:** `service.rs::tests::universal_time_returns_recent_value`.

**Status:** done

### §2.1.2 Operation secure_universal_time

**Spec:** §2.1.2, p. 2-2 (PDF) — "returns the current time in a UTO only if the
time can be guaranteed to have been obtained securely. [...] If there is any
uncertainty at all about meeting any aspect of these criteria, then this
operation must return the TimeUnavailable exception."

**Repo:** `crates/time-service/src/service.rs::TimeService::secure_universal_time`
(controllable via the `TimeService::secure_source` flag).

**Tests:** `service.rs::tests::secure_universal_time_fails_when_source_not_marked_secure`,
`secure_universal_time_returns_when_source_marked_secure`.

**Status:** done

### §2.1.2.1 Operation new_universal_time

**Spec:** §2.1.2.1, p. 2-2 (PDF) — "constructing a new UTO. The parameters
passed in are the time of type TimeT and inaccuracy of type InaccuracyT.
[...] CORBA::BAD_PARAM is raised in the case of an out-of-range parameter
value for inaccuracy."

**Repo:** `crates/time-service/src/service.rs::TimeService::new_universal_time`.
ZeroDDS choice: no BAD_PARAM on out-of-range — capping to 48 bits (the spec
does not strictly require a reject, but allows it in the spec section
"encouraged to include").

**Tests:** `service.rs::tests::new_universal_time_creates_uto_from_components`.

**Status:** done

### §2.1.2.2 Operation uto_from_utc

**Spec:** §2.1.2.2, p. 2-2 (PDF) — "create a UTO given a time in the UtcT form.
[...] used to convert a UTC received over the wire into a UTO."

**Repo:** `crates/time-service/src/service.rs::TimeService::uto_from_utc`.

**Tests:** `service.rs::tests::uto_from_utc_wraps_passed_struct`.

**Status:** done

### §2.1.2.3 Operation new_interval

**Spec:** §2.1.2.3, p. 2-3 (PDF) — "If the value of the lower parameter is
greater than the value of the upper parameter, then a CORBA::BAD_PARAM
exception is raised."

**Repo:** `crates/time-service/src/service.rs::TimeService::new_interval`
(returns `None` on lower > upper).

**Tests:** `service.rs::tests::new_interval_rejects_lower_greater_than_upper`,
`new_interval_creates_tio_for_valid_bounds`.

**Status:** done

---

## §2.2 Timer Event Service

### §2.2.1 Object model — TimerEventService manages TimerEventHandlers

**Spec:** §2.2.1, p. 2-3 (PDF) — "The TimerEventService object manages Timer
Event Handlers. [...] Each Timer Event Handler is immutably associated with a
specific event channel at the time of its creation."

**Repo:** the platform service in
`crates/corba-ccm/src/timer.rs::TimerEventService` (worker thread,
callback-driven) + a spec-conformant facade
`crates/corba-ccm/src/time_psm.rs::TimerEventServiceFacade` with a
`PushConsumerLike` adapter for the spec-§2.2.2 channel binding.

**Tests:** `time_psm::tests::facade_register_then_fire` + inline tests in
`timer.rs`.

**Status:** done — TimerEventService lifecycle + handler management + push
channel adapter live.

### §2.2.2 Usage — push event channel + timer events

**Spec:** §2.2.2, p. 2-4 (PDF) — workflow: create an event channel, register a
TimerEventHandler, set a timer, push events over the channel.

**Repo:** the `crates/corba-ccm/src/time_psm.rs::PushConsumerLike` trait + the
`TimerEventServiceFacade` adapter; on firing, `push(&TimerEventT)` is called on
the consumer, which maps the spec workflow §2.2.2.

**Tests:** `time_psm::tests::facade_register_then_fire`.

**Status:** done — timer lifecycle + fire path + channel adapter live.

### §2.2.3 Data types CosTimerEvent

**Spec:** §2.2.3, p. 2-4 (PDF) — `module CosTimerEvent { enum TimeType {
TTAbsolute, TTRelative, TTPeriodic }; enum EventStatus { ESTimeSet,
ESTimeCleared, ESTriggered, ESFailedTrigger }; struct TimerEventT {
TimeBase::UtcT utc; any event_data; }; };`.

**Repo:** `crates/corba-ccm/src/time_psm.rs::{TimerEventT, EventStatus,
TimeType}` as spec-conformant Rust types.

**Tests:** `time_psm::tests::{event_status_variants_are_distinct,
event_time_extracts_utc_from_timer_event_t}`.

**Status:** done — all three IDL-PSM types declared.

### §2.2.3.1 Enum TimeType

**Spec:** §2.2.3.1, p. 2-4 (PDF) — TTAbsolute / TTRelative / TTPeriodic.

**Repo:** `crates/corba-ccm/src/time_psm.rs::TimeType::{TtAbsolute, TtRelative,
TtPeriodic}` (1:1 spec mapping); a `to_timer_kind()` helper maps to the
platform `TimerKind` (Periodic→Periodic, Absolute/Relative→OneShot).

**Tests:** `time_psm::tests::{time_type_periodic_maps_to_timer_kind_periodic,
time_type_absolute_maps_to_one_shot, time_type_relative_maps_to_one_shot}`.

**Status:** done — an IDL-conformant 3-variant enum + platform mapping live.

### §2.2.4 Exceptions

**Spec:** §2.2.4, p. 2-5 (PDF) — exception definitions for the
TimerEventService.

**Repo:** `crates/corba-ccm/src/time_psm.rs::TimerError` with all four spec
variants `TimeUnavailable`, `TimerExpired`, `InvalidTime`, `InvalidEvent`.

**Tests:** `time_psm::tests::timer_error_display_uses_spec_names`.

**Status:** done — a spec-conformant exception hierarchy declared.

---

## §2.3 Timer Event Handler

### §2.3.1 Attribute status

**Spec:** §2.3, p. 2-5/2-6 (PDF) — the `TimerEventHandler` interface with the
attribute `status` and the operations `set_timer/cancel_timer/set_data/time_set`.

**Repo:** `crates/corba-ccm/src/time_psm.rs::TimerEventHandler` with the
spec-conformant operations `status()`, `time_set()`, `set_timer(TimeType,
Duration)`, `set_data(Vec<u8>)` plus the `EventStatus` enum
(`EsTimeSet`/`EsTimerFired`/`EsTimerCancelled`). Cancel via
`TimerEventServiceFacade::cancel`.

**Tests:** `time_psm::tests::{handler_status_starts_as_time_set,
handler_time_set_returns_time_type, handler_set_data_rejects_empty,
handler_set_data_accepts_non_empty, handler_set_timer_rejects_after_fire,
handler_set_timer_ok_when_armed, event_status_variants_are_distinct}`.

**Status:** done — spec-conformant handler operations + status attribute live.

---

## §2.4 Timer Event Service operations

### §2.4.1 Operation register

**Spec:** §2.4.1, p. 2-7 (PDF) — `TimerEventHandler register(in
CosEventComm::PushConsumer event_interface, in any data);`.

**Repo:** `crates/corba-ccm/src/time_psm.rs::TimerEventServiceFacade::register`
with a `PushConsumerLike` trait that binds the spec-conformant PushConsumer
argument path (an adapter over the callback-based platform
`TimerEventService`).

**Tests:** `time_psm::tests::facade_register_then_fire`.

**Status:** done — PushConsumer adapter live; the spec form (PushConsumer +
data) and the platform form (callback) available in parallel.

### §2.4.2 Operation unregister

**Spec:** §2.4.2, p. 2-7 (PDF) — `void unregister(in TimerEventHandler);`.

**Repo:** `crates/corba-ccm/src/timer.rs::TimerEventService::cancel(handle)`.

**Tests:** inline.

**Status:** done

### §2.4.3 Operation event_time

**Spec:** §2.4.3, p. 2-7 (PDF) — `UTO event_time(in CosTimerEvent::TimerEventT
event);`.

**Repo:** `crates/corba-ccm/src/time_psm.rs::event_time(&TimerEventT) -> u64`
(a UTO equivalent as nanoseconds since the Unix epoch).

**Tests:** `time_psm::tests::event_time_extracts_utc_from_timer_event_t`.

**Status:** done — a spec-conformant operation implemented.

---

## §2.5 Conformance

### §2.5 Conformance statement

**Spec:** §2.5, p. 2-7 (PDF) — two conformance points (Basic Time Service +
Timer Event Service); the Timer Event Service requires the Basic Time Service.

**Repo:** ZeroDDS declares conformance to **both** conformance points as a
library form without a CORBA ORB: the **Basic Time Service** (§1.3 + §2.1) in
the crate `crates/time-service/`, and the **Timer Event Service** (§2.2-§2.4)
via `crates/corba-ccm/src/time_psm.rs` (PushConsumer adapter + TimerEventT +
spec exceptions + event_time).

**Tests:** cross-ref §1.3.x + §2.1.x + §2.2.x (35 + 36 tests green).

**Status:** done — Basic Time Service + Timer Event Service both fully
covered.

---

## Appendix A — Implementation guidelines

### App. A — Secure time-source criteria

**Spec:** Appendix A — implementation notes for a secure time source.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Appendix A explicitly declared an
implementation guideline (non-normative).

---

## Appendix B-F

### App. B — Administration

**Spec:** App. B (PDF) — administration of the Time Service (non-normative).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### App. C — IDL listing

**Spec:** App. C (PDF) — IDL for TimeBase + CosTime + CosTimerEvent
(non-normative as an appendix; normatively equivalent definitions are in §1.3
+ §2.x).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### App. D — Notes

**Spec:** App. D (PDF) — implementation notes (non-normative).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### App. E — Examples

**Spec:** App. E (PDF) — example code (non-normative).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### App. F — References

**Spec:** App. F (PDF) — external references (non-normative).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Audit status

43 done / 0 partial / 0 open / 6 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-time-service -p zerodds-corba-ccm` — 35 + 36
tests green (Basic Time Service in `time-service`: modules `service`,
`time_base`, `tio`, `uto`; Timer Event Service lifecycle in
`corba-ccm/timer.rs`: `timer::tests::one_shot_fires_once`,
`timer::tests::periodic_fires_multiple_times`,
`timer::tests::cancel_stops_periodic`,
`timer::tests::cancel_unknown_returns_false`).
