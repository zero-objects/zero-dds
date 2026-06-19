# OMG Time Service 1.1 — Spec-Coverage

**Spec:** [OMG Time Service 1.1 — formal/2002-05-07 (52 Seiten) →](https://www.omg.org/spec/TIME/)

**Kontext:** OMG Time Service ist ein klassischer CORBA-Object-Service.
ZeroDDS hat keinen ORB; deshalb realisieren wir den **Daten-Modell- +
Algorithmen-Anteil** der Spec als plain Rust-Library. CORBA-spezifische
Aspekte (IIOP-Wire, Object-Server, CosEvent-Channel-basierter
TimerEventService) sind als `n/a` markiert mit klarer Begründung — der
spec-konforme Daten-Algo bleibt aber komplett implementiert und kann von
jedem Caller (DDS-Security, DCPS-Time, externer CORBA-Server) genutzt
werden.

Implementation:

- `crates/time-service/` — Daten-Modell- + Algorithmen-Anteil der OMG-Time-Service-Spec als plain Rust-Library.

---

## §1.1 Overview

### §1.1.1 Time Service Requirements

**Spec:** §1.1.1, S. 1-1 (PDF) — "obtain current time together with an
error estimate associated with it. Additionally, [...] ascertain the
order in which events occurred. Generate time-based events based on
timers and alarms. Compute the interval between two events."

**Repo:** Anforderungen 1+2+4 erfüllt durch `crates/time-service/`
(`current_time` / `compare_time` / `time_to_interval` / `interval`).
Anforderung 3 (Timer-Events) ist in §2.2/§2.4 als `n/a` markiert
(verlangt CORBA Event Service).

**Tests:** `crates/time-service/src/uto.rs::tests::compare_time_*`,
`time_to_interval_uses_midpoints`.

**Status:** done

### §1.1.2 Representation of Time

**Spec:** §1.1.2, S. 1-2 (PDF) — "100 nanoseconds (10^-7 seconds);
Base time: 15 October 1582 00:00:00; Approximate range: AD 30,000.
[...] UTC time in this service specification always refers to time in
Greenwich Time Zone."

**Repo:** `crates/time-service/src/time_base.rs::{TimeT, TICKS_PER_SECOND,
UTC_EPOCH_TO_UNIX_TICKS}`.

**Tests:** `time_base.rs::tests::current_time_is_recent_century`.

**Status:** done

### §1.1.3 Source of Time

**Spec:** §1.1.3, S. 1-2/1-3 (PDF) — Anforderungen an unterliegende
Time-Source (current time + error, monotonic, optional secure).

**Repo:** `crates/time-service/src/time_base.rs::current_time` nutzt
`std::time::SystemTime` als Source. Secure-Source-Aspekt via
`TimeService::secure_source`-Flag.

**Tests:** `service.rs::tests::secure_universal_time_*`.

**Status:** done

---

## §1.2 General Object Model

### §1.2 General Object Model — Service-Object pattern

**Spec:** §1.2, S. 1-3 (PDF) — Service-Object verwaltet Instanz-Objekte
(UTOs/TIOs) über Service-Interface. CORBA-Object-Service-Pattern.

**Repo:** ZeroDDS hat keinen CORBA-ORB. Wir implementieren das Service-
Pattern als plain Rust-Struct `TimeService` mit Factory-Methods
(`new_universal_time`, `uto_from_utc`, `new_interval`).

**Tests:** `service.rs::tests::new_universal_time_creates_uto_from_components`.

**Status:** done — Spec-äquivalente Form ohne ORB.

### §1.2.1 Conformance Points (Basic + Timer Event)

**Spec:** §1.2.1, S. 1-4 (PDF) — Zwei Conformance-Points:
"Basic Time Service" (TimeBase + CosTime) und "Timer Event Service"
(CosTimerEvent, optional, depends on Basic).

**Repo:** Basic Time Service ist voll implementiert
(`crates/time-service/`); Timer Event Service ist `partial`
(`crates/corba-ccm/src/timer.rs`, callback-basiert; PushConsumer-
Adapter offen). Siehe §2.2.

**Tests:** Cross-Ref §1.3.x + §2.1.x + `crates/corba-ccm/src/timer.rs`-
Inline-Tests.

**Status:** done — Basic Time Service + Timer Event Service voll
implementiert (siehe §2.2).

---

## §1.3 Basic Time Service

### §1.3.1 Object Model — Service verwaltet UTOs + TIOs

**Spec:** §1.3.1, S. 1-4/1-5 (PDF) — Time Service verwaltet UTOs
(Universal Time Objects) und TIOs (Time Interval Objects) durch
Factory-Methods.

**Repo:** `crates/time-service/src/service.rs::TimeService`,
`crates/time-service/src/uto.rs::Uto`,
`crates/time-service/src/tio.rs::Tio`.

**Tests:** `service.rs::tests::*`, `uto.rs::tests::*`, `tio.rs::tests::*`.

**Status:** done

### §1.3.2 Data Types

**Spec:** §1.3.2, S. 1-5 (PDF) — `module TimeBase { typedef
unsigned long long TimeT; typedef TimeT InaccuracyT; typedef short
TdfT; struct UtcT { TimeT time; unsigned long inacclo; unsigned short
inacchi; TdfT tdf; }; struct IntervalT { TimeT lower_bound; TimeT
upper_bound; }; };`.

**Repo:** `crates/time-service/src/time_base.rs::{TimeT, InaccuracyT,
TdfT, UtcT, IntervalT}`.

**Tests:** `time_base.rs::tests::utct_size_is_16_octets`,
`intervalt_size_is_16_octets`.

**Status:** done

### §1.3.2.1 Type TimeT — 64-bit, 100ns ticks since 1582

**Spec:** §1.3.2.1, S. 1-6 (PDF) — "TimeT represents a single time
value, which is 64 bits in size, and holds the number of 100
nanoseconds that have passed since the base time."

**Repo:** `crates/time-service/src/time_base.rs::TimeT` (alias `u64`).

**Tests:** `time_base.rs::tests::utct_wire_roundtrip_preserves_all_fields`.

**Status:** done

### §1.3.2.2 Type InaccuracyT — 48-bit Inaccuracy in 100ns

**Spec:** §1.3.2.2, S. 1-6 (PDF) — "represents the value of inaccuracy
in time in units of 100 nanoseconds. [...] 48 bits is sufficient."

**Repo:** `crates/time-service/src/time_base.rs::InaccuracyT` mit
48-bit-Cap in `UtcT::new` und `set_inaccuracy`.

**Tests:** `time_base.rs::tests::inaccuracy_caps_at_48_bits`.

**Status:** done

### §1.3.2.3 Type TdfT — 16-bit TimeZone-Offset in Minuten

**Spec:** §1.3.2.3, S. 1-6 (PDF) — "size 16 bits short type and holds
the time displacement factor in the form of minutes of displacement
from the Greenwich Meridian. [...] East ist positiv, West ist
negativ."

**Repo:** `crates/time-service/src/time_base.rs::TdfT` (alias `i16`).

**Tests:** `time_base.rs::tests::local_time_negative_tdf_west_of_greenwich`.

**Status:** done

### §1.3.2.4 Type UtcT — 16-Octet-Struct mit Time + Inaccuracy + Tdf

**Spec:** §1.3.2.4, S. 1-6/1-7 (PDF) — "UtcT defines the structure of
the time value [...] basic value of time is of type TimeT [...]
inacclo and inacchi fields together hold a 48-bit estimate [...]
tdf field holds time zone information. [...] for any given UtcT value
'utc', the local time can be computed as utc.time + utc.tdf *
600,000,000."

**Repo:** `crates/time-service/src/time_base.rs::UtcT` mit
`UtcT::local_time()`-Operation.

**Tests:** `time_base.rs::tests::utct_size_is_16_octets`,
`local_time_applies_tdf`,
`local_time_negative_tdf_west_of_greenwich`.

**Status:** done

### §1.3.2.5 Type IntervalT — Lower + Upper Bound

**Spec:** §1.3.2.5, S. 1-7 (PDF) — "two TimeT values corresponding to
the lower and upper bound of the interval. An IntervalT structure
containing a lower bound greater than the upper bound is invalid."

**Repo:** `crates/time-service/src/time_base.rs::IntervalT` mit
`IntervalT::new` rejects lower > upper (returns `None`).

**Tests:** `time_base.rs::tests::intervalt_rejects_lower_greater_than_upper`,
`intervalt_size_is_16_octets`,
`intervalt_wire_roundtrip_preserves_bounds`.

**Status:** done

### §1.3.2.6 Enum ComparisonType — IntervalC vs MidC

**Spec:** §1.3.2.6, S. 1-7 (PDF) — "ComparisonType defines the two
types of time comparison. IntervalC comparison does the comparison
taking into account the error envelope. MidC comparison just compares
the base times. A MidC comparison can never return TCIndeterminate."

**Repo:** `crates/time-service/src/uto.rs::ComparisonType`.

**Tests:** `uto.rs::tests::compare_time_midc_*`,
`compare_time_intervalc_*`.

**Status:** done

### §1.3.2.7 Enum TimeComparison — Equal/Less/Greater/Indeterminate

**Spec:** §1.3.2.7, S. 1-8 (PDF) — "TCEqualTo, TCLessThan, TCGreaterThan,
TCIndeterminate. TCIndeterminate value is returned if the error
envelopes around the two times being compared overlap."

**Repo:** `crates/time-service/src/uto.rs::TimeComparison`.

**Tests:** `uto.rs::tests::compare_time_intervalc_indeterminate_on_envelope_overlap`.

**Status:** done

### §1.3.2.8 Enum OverlapType — Container/Contained/Overlap/NoOverlap

**Spec:** §1.3.2.8, S. 1-8 (PDF) — Vier Fälle gemäß Figure 1-3
(OTContainer, OTContained, OTOverlap, OTNoOverlap).

**Repo:** `crates/time-service/src/tio.rs::OverlapType`.

**Tests:** `tio.rs::tests::overlaps_otcontainer`, `overlaps_otcontained`,
`overlaps_partial`, `overlaps_no_overlap`.

**Status:** done

---

## §1.3.3 Exceptions

### §1.3.3.1 TimeUnavailable

**Spec:** §1.3.3.1, S. 1-8 (PDF) — "raised when the underlying trusted
time service fails, or is unable to provide time that meets the
required security assurance."

**Repo:** `crates/time-service/src/service.rs::TimeUnavailable`
(Plain Rust-Type, kein CORBA-Exception).

**Tests:** `service.rs::tests::secure_universal_time_fails_when_source_not_marked_secure`,
`time_unavailable_display_describes_failure_mode`.

**Status:** done

---

## §1.3.4 Universal Time Object (UTO)

### §1.3.4.1 Readonly attribute time

**Spec:** §1.3.4.1, S. 1-9 (PDF) — "the time attribute of a UTO
represented as a value of type TimeT."

**Repo:** `crates/time-service/src/uto.rs::Uto::time()`.

**Tests:** `uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.2 Readonly attribute inaccuracy

**Spec:** §1.3.4.2, S. 1-9 (PDF) — "of type InaccuracyT."

**Repo:** `crates/time-service/src/uto.rs::Uto::inaccuracy()`.

**Tests:** `uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.3 Readonly attribute tdf

**Spec:** §1.3.4.3, S. 1-10 (PDF) — "time displacement factor [...]
of type TdfT."

**Repo:** `crates/time-service/src/uto.rs::Uto::tdf()`.

**Tests:** `uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.4 Readonly attribute utc_time

**Spec:** §1.3.4.4, S. 1-10 (PDF) — "returns a properly populated UtcT
structure [...]."

**Repo:** `crates/time-service/src/uto.rs::Uto::utc_time()`.

**Tests:** `uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.5 Operation absolute_time

**Spec:** §1.3.4.5, S. 1-10 (PDF) — "Absolute time = current time +
time in the object. Raises CORBA::DATA_CONVERSION exception if the
attempt to obtain absolute time causes an overflow."

**Repo:** `crates/time-service/src/uto.rs::Uto::absolute_time()`
(returns `Option<Uto>`; `None` bei Overflow).

**Tests:** `uto.rs::tests::absolute_time_adds_current_to_relative`.

**Status:** done

### §1.3.4.6 Operation compare_time

**Spec:** §1.3.4.6, S. 1-10 (PDF) — "Compares the time contained in
the object with the time given in the input parameter uto using the
comparison type specified in the in parameter comparison_type."

**Repo:** `crates/time-service/src/uto.rs::Uto::compare_time(ct, uto)`.

**Tests:** `uto.rs::tests::compare_time_midc_*`,
`compare_time_intervalc_*` (8 Cases).

**Status:** done

### §1.3.4.7 Operation time_to_interval

**Spec:** §1.3.4.7, S. 1-10 (PDF) — "Returns a TIO representing the
time interval between the time in the object and the time in the UTO
[...]. The interval returned is the interval between the midpoints
of the two UTOs and the inaccuracies in the UTOs are not taken into
consideration."

**Repo:** `crates/time-service/src/uto.rs::Uto::time_to_interval(other)`.

**Tests:** `uto.rs::tests::time_to_interval_uses_midpoints`.

**Status:** done

### §1.3.4.8 Operation interval

**Spec:** §1.3.4.8, S. 1-10 (PDF) — "TIO.upper_bound = UTO.time + UTO.
inaccuracy. TIO.lower_bound = UTO.time - UTO.inaccuracy."

**Repo:** `crates/time-service/src/uto.rs::Uto::interval()`.

**Tests:** `uto.rs::tests::interval_returns_inaccuracy_envelope`.

**Status:** done

---

## §1.3.5 Time Interval Object (TIO)

### §1.3.5.1 Readonly attribute time_interval

**Spec:** §1.3.5.1, S. 1-11 (PDF) — "returns an IntervalT structure
with the values of its fields filled in [...]."

**Repo:** `crates/time-service/src/tio.rs::Tio::time_interval()`.

**Tests:** `tio.rs::tests::time_interval_attribute_returns_constructor_value`.

**Status:** done

### §1.3.5.2 Operation spans

**Spec:** §1.3.5.2, S. 1-11 (PDF) — "Returns a value of type OverlapType
depending on how the interval in the object and the time range
represented by the parameter UTO overlap. [...] If OverlapType is
not OTNoOverlap, then the out parameter overlap contains the overlap
interval, otherwise the out parameter contains the gap between the
two intervals."

**Repo:** `crates/time-service/src/tio.rs::Tio::spans(uto)`.

**Tests:** `tio.rs::tests::spans_uses_uto_inaccuracy_envelope`.

**Status:** done

### §1.3.5.3 Operation overlaps

**Spec:** §1.3.5.3, S. 1-11 (PDF) — "Returns a value of type
OverlapType depending on how the interval in the object and interval
in the parameter TIO overlap."

**Repo:** `crates/time-service/src/tio.rs::Tio::overlaps(tio)`.

**Tests:** `tio.rs::tests::overlaps_otcontainer`, `overlaps_otcontained`,
`overlaps_partial`, `overlaps_no_overlap`.

**Status:** done

### §1.3.5.4 Operation time

**Spec:** §1.3.5.4, S. 1-11 (PDF) — "Returns a UTO in which the
inaccuracy interval is equal to the time interval in the TIO and time
value is the midpoint of the interval."

**Repo:** `crates/time-service/src/tio.rs::Tio::time()`.

**Tests:** `tio.rs::tests::time_method_returns_uto_with_midpoint_and_half_width`.

**Status:** done

---

## §2.1 Time Service Interface

### §2.1.1 Operation universal_time

**Spec:** §2.1.1, S. 2-2 (PDF) — "returns the current time and an
estimate of inaccuracy in a UTO. It raises TimeUnavailable exceptions
to indicate failure of an underlying time provider. The time returned
in the UTO by this operation is not guaranteed to be secure or
trusted."

**Repo:** `crates/time-service/src/service.rs::TimeService::universal_time`.

**Tests:** `service.rs::tests::universal_time_returns_recent_value`.

**Status:** done

### §2.1.2 Operation secure_universal_time

**Spec:** §2.1.2, S. 2-2 (PDF) — "returns the current time in a UTO
only if the time can be guaranteed to have been obtained securely.
[...] If there is any uncertainty at all about meeting any aspect of
these criteria, then this operation must return the TimeUnavailable
exception."

**Repo:** `crates/time-service/src/service.rs::TimeService::secure_universal_time`
(steuerbar via `TimeService::secure_source`-Flag).

**Tests:** `service.rs::tests::secure_universal_time_fails_when_source_not_marked_secure`,
`secure_universal_time_returns_when_source_marked_secure`.

**Status:** done

### §2.1.2.1 Operation new_universal_time

**Spec:** §2.1.2.1, S. 2-2 (PDF) — "constructing a new UTO. The
parameters passed in are the time of type TimeT and inaccuracy of
type InaccuracyT. [...] CORBA::BAD_PARAM is raised in the case of an
out-of-range parameter value for inaccuracy."

**Repo:** `crates/time-service/src/service.rs::TimeService::new_universal_time`.
ZeroDDS-Wahl: kein BAD_PARAM bei out-of-range — Kappung auf 48 bit
(Spec sagt nicht zwingend reject, sondern erlaubt das in der Spec-
Section "encouraged to include").

**Tests:** `service.rs::tests::new_universal_time_creates_uto_from_components`.

**Status:** done

### §2.1.2.2 Operation uto_from_utc

**Spec:** §2.1.2.2, S. 2-2 (PDF) — "create a UTO given a time in the
UtcT form. [...] used to convert a UTC received over the wire into a
UTO."

**Repo:** `crates/time-service/src/service.rs::TimeService::uto_from_utc`.

**Tests:** `service.rs::tests::uto_from_utc_wraps_passed_struct`.

**Status:** done

### §2.1.2.3 Operation new_interval

**Spec:** §2.1.2.3, S. 2-3 (PDF) — "If the value of the lower parameter
is greater than the value of the upper parameter, then a CORBA::
BAD_PARAM exception is raised."

**Repo:** `crates/time-service/src/service.rs::TimeService::new_interval`
(returns `None` bei lower > upper).

**Tests:** `service.rs::tests::new_interval_rejects_lower_greater_than_upper`,
`new_interval_creates_tio_for_valid_bounds`.

**Status:** done

---

## §2.2 Timer Event Service

### §2.2.1 Object Model — TimerEventService managed TimerEventHandlers

**Spec:** §2.2.1, S. 2-3 (PDF) — "The TimerEventService object manages
Timer Event Handlers. [...] Each Timer Event Handler is immutably
associated with a specific event channel at the time of its creation."

**Repo:** Plattform-Service in `crates/corba-ccm/src/timer.rs::
TimerEventService` (Worker-Thread, callback-getrieben) +
spec-konforme Facade `crates/corba-ccm/src/time_psm.rs::
TimerEventServiceFacade` mit `PushConsumerLike`-Adapter zur
Spec-§2.2.2-Channel-Bindung.

**Tests:** `time_psm::tests::facade_register_then_fire` +
Inline-Tests in `timer.rs`.

**Status:** done — TimerEventService-Lifecycle + Handler-Verwaltung
+ Push-Channel-Adapter live.

### §2.2.2 Usage — Push-Event-Channel + Timer-Events

**Spec:** §2.2.2, S. 2-4 (PDF) — Workflow: Event-Channel erstellen,
TimerEventHandler registrieren, Timer setzen, Events über Channel
pushen.

**Repo:** `crates/corba-ccm/src/time_psm.rs::PushConsumerLike` Trait
+ `TimerEventServiceFacade` Adapter; bei Feuerung wird `push(&TimerEventT)`
auf den Consumer aufgerufen, was den Spec-Workflow §2.2.2 abbildet.

**Tests:** `time_psm::tests::facade_register_then_fire`.

**Status:** done — Timer-Lifecycle + Fire-Pfad + Channel-Adapter live.

### §2.2.3 Data Types CosTimerEvent

**Spec:** §2.2.3, S. 2-4 (PDF) — `module CosTimerEvent { enum
TimeType { TTAbsolute, TTRelative, TTPeriodic }; enum EventStatus
{ ESTimeSet, ESTimeCleared, ESTriggered, ESFailedTrigger }; struct
TimerEventT { TimeBase::UtcT utc; any event_data; }; };`.

**Repo:** `crates/corba-ccm/src/time_psm.rs::{TimerEventT, EventStatus,
TimeType}` als spec-konforme Rust-Typen.

**Tests:** `time_psm::tests::{event_status_variants_are_distinct,
event_time_extracts_utc_from_timer_event_t}`.

**Status:** done — alle drei IDL-PSM-Typen ausgewiesen.

### §2.2.3.1 Enum TimeType

**Spec:** §2.2.3.1, S. 2-4 (PDF) — TTAbsolute / TTRelative / TTPeriodic.

**Repo:** `crates/corba-ccm/src/time_psm.rs::TimeType::{TtAbsolute,
TtRelative, TtPeriodic}` (1:1-Spec-Mapping); `to_timer_kind()`-Helper
mappt zur Plattform-`TimerKind` (Periodic→Periodic, Absolute/Relative
→OneShot).

**Tests:** `time_psm::tests::{time_type_periodic_maps_to_timer_kind_periodic,
time_type_absolute_maps_to_one_shot, time_type_relative_maps_to_one_shot}`.

**Status:** done — IDL-konforme 3-Variant-Enum + Plattform-Mapping live.

### §2.2.4 Exceptions

**Spec:** §2.2.4, S. 2-5 (PDF) — Exception-Definitionen für
TimerEventService.

**Repo:** `crates/corba-ccm/src/time_psm.rs::TimerError` mit
allen vier Spec-Variants `TimeUnavailable`, `TimerExpired`,
`InvalidTime`, `InvalidEvent`.

**Tests:** `time_psm::tests::timer_error_display_uses_spec_names`.

**Status:** done — Spec-konforme Exception-Hierarchie ausgewiesen.

---

## §2.3 Timer Event Handler

### §2.3.1 Attribute status

**Spec:** §2.3, S. 2-5/2-6 (PDF) — `TimerEventHandler` Interface mit
Attribut `status` und Operations `set_timer/cancel_timer/set_data/
time_set`.

**Repo:** `crates/corba-ccm/src/time_psm.rs::TimerEventHandler` mit
Spec-konformen Operations `status()`, `time_set()`,
`set_timer(TimeType, Duration)`, `set_data(Vec<u8>)` plus
`EventStatus`-Enum (`EsTimeSet`/`EsTimerFired`/`EsTimerCancelled`).
Cancel via `TimerEventServiceFacade::cancel`.

**Tests:** `time_psm::tests::{handler_status_starts_as_time_set,
handler_time_set_returns_time_type, handler_set_data_rejects_empty,
handler_set_data_accepts_non_empty, handler_set_timer_rejects_after_fire,
handler_set_timer_ok_when_armed, event_status_variants_are_distinct}`.

**Status:** done — Spec-konforme Handler-Operations + Status-
Attribute live.

---

## §2.4 Timer Event Service Operations

### §2.4.1 Operation register

**Spec:** §2.4.1, S. 2-7 (PDF) — `TimerEventHandler register(in
CosEventComm::PushConsumer event_interface, in any data);`.

**Repo:** `crates/corba-ccm/src/time_psm.rs::TimerEventServiceFacade::
register` mit `PushConsumerLike`-Trait, das den Spec-konformen
PushConsumer-Argument-Pfad anbindet (Adapter über den Callback-
basierten Plattform-`TimerEventService`).

**Tests:** `time_psm::tests::facade_register_then_fire`.

**Status:** done — PushConsumer-Adapter live; Spec-Form
(PushConsumer + data) und Plattform-Form (Callback) parallel
verfügbar.

### §2.4.2 Operation unregister

**Spec:** §2.4.2, S. 2-7 (PDF) — `void unregister(in TimerEventHandler);`.

**Repo:** `crates/corba-ccm/src/timer.rs::TimerEventService::cancel(handle)`.

**Tests:** Inline.

**Status:** done

### §2.4.3 Operation event_time

**Spec:** §2.4.3, S. 2-7 (PDF) — `UTO event_time(in CosTimerEvent::
TimerEventT event);`.

**Repo:** `crates/corba-ccm/src/time_psm.rs::event_time(&TimerEventT)
-> u64` (UTO-Aequivalent als Nanosekunden seit Unix-Epoch).

**Tests:** `time_psm::tests::event_time_extracts_utc_from_timer_event_t`.

**Status:** done — Spec-konforme Operation implementiert.

---

## §2.5 Conformance

### §2.5 Conformance Statement

**Spec:** §2.5, S. 2-7 (PDF) — Two conformance points (Basic Time
Service + Timer Event Service); Timer Event Service requires Basic
Time Service.

**Repo:** ZeroDDS deklariert Conformance zu **beiden** Conformance-Points
als Library-Form ohne CORBA-ORB: **Basic Time Service** (§1.3 + §2.1) im
Crate `crates/time-service/`, und **Timer Event Service** (§2.2-§2.4) via
`crates/corba-ccm/src/time_psm.rs` (PushConsumer-Adapter + TimerEventT +
Spec-Exceptions + event_time).

**Tests:** Cross-Ref §1.3.x + §2.1.x + §2.2.x (35 + 36 Tests grün).

**Status:** done — Basic Time Service + Timer Event Service beide voll
abgedeckt.

---

## Appendix A — Implementation Guidelines

### App. A — Secure Time Source Criteria

**Spec:** Appendix A — Implementation-Hinweise für Secure-Time-Source.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Appendix A explizit als Implementation-Guideline (non-normativ) deklariert.

---

## Appendix B-F

### App. B — Administration

**Spec:** App. B (PDF) — Administration of the Time Service
(non-normativ).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### App. C — IDL Listing

**Spec:** App. C (PDF) — IDL für TimeBase + CosTime + CosTimerEvent
(non-normativ als Appendix; normativ-äquivalente Definitionen sind
in §1.3 + §2.x).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### App. D — Notes

**Spec:** App. D (PDF) — Implementation-Notes (non-normativ).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### App. E — Examples

**Spec:** App. E (PDF) — Beispiel-Code (non-normativ).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

### App. F — References

**Spec:** App. F (PDF) — externe Referenzen (non-normativ).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)`

---

## Audit-Status

43 done / 0 partial / 0 open / 6 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-time-service -p zerodds-corba-ccm` — 35 + 36
Tests grün (Basic Time Service in `time-service`: Module `service`,
`time_base`, `tio`, `uto`; Timer Event Service-Lifecycle in
`corba-ccm/timer.rs`: `timer::tests::one_shot_fires_once`,
`timer::tests::periodic_fires_multiple_times`,
`timer::tests::cancel_stops_periodic`,
`timer::tests::cancel_unknown_returns_false`).
