# OMG Time Service 1.1 — Spec-Coverage

**Spec:** [OMG Time Service 1.1 — formal/2002-05-07 (52 Seiten) →](https://www.omg.org/spec/TIME/)

**Kontext:** OMG Time Service ist ein klassischer CORBA-Object-Service.
`crates/time-service/` implementiert die Module TimeBase, CosTime und
CosTimerEvent als Rust-Bibliothek; die Ereignisse des Timer Event Service
gehen an den `PushConsumer` von `crates/corba-cos-event/`. CosTime ist mit dem Feature `serve` als
CORBA-Objekt über GIOP erreichbar (`crates/time-service/src/servant.rs`, `crates/time-service/src/server.rs`, §1.2).

Implementation:

- `crates/time-service/` — TimeBase, CosTime (UTO, TIO, TimeService) und CosTimerEvent (`timer_event.rs`, Feature `timer-event`); Verwaltung und sichere Zeit nach App. A und App. B (`admin.rs`, `secure.rs`, `proxy.rs`).
- `crates/dcps/src/time_source.rs::KernelClock` — die Kernel-Uhr von Linux als sichere Quelle (`adjtimex(2)`).
- `crates/security-logging/src/time_audit.rs` — Audit-Datensätze der Verwaltung als JSON-Zeilen (Feature `time-service`).

---

## §1.1 Overview

### §1.1.1 Time Service Requirements

**Spec:** §1.1.1, S. 1-1 (PDF) — "obtain current time together with an
error estimate associated with it. Additionally, [...] ascertain the
order in which events occurred. Generate time-based events based on
timers and alarms. Compute the interval between two events."

**Repo:** Anforderungen 1, 2 und 4 erfüllt durch `crates/time-service/`
(`current_time` / `compare_time` / `time_to_interval` / `interval`).
Anforderung 3 (Ereignisse aus Timern und Alarmen) erfüllt der Timer Event
Service in `crates/time-service/src/timer_event.rs` (§2.2 bis §2.4).

**Tests:** `crates/time-service/src/uto.rs::tests::compare_time_*`,
`time_to_interval_uses_midpoints`;
`crates/time-service/src/timer_event.rs::tests::time_2_2_3_2_status_triggered_after_one_shot`,
`time_2_2_3_2_periodic_never_triggered_stays_time_set`.

**Status:** done

### §1.1.2 Representation of Time

**Spec:** §1.1.2, S. 1-2 (PDF) — "100 nanoseconds (10^-7 seconds);
Base time: 15 October 1582 00:00:00; Approximate range: AD 30,000.
[...] UTC time in this service specification always refers to time in
Greenwich Time Zone."

**Repo:** `crates/time-service/src/time_base.rs::{TimeT, TICKS_PER_SECOND,
UTC_EPOCH_TO_UNIX_TICKS}`.

**Tests:** `crates/time-service/src/time_base.rs::tests::current_time_is_recent_century`.

**Status:** done

### §1.1.3 Source of Time

**Spec:** §1.1.3, S. 1-2/1-3 (PDF) — Anforderungen an unterliegende
Time-Source (current time + error, monotonic, optional secure).

**Repo:** `crates/time-service/src/time_base.rs::current_time` nutzt
`std::time::SystemTime` als Quelle; `TimeService::with_source` nimmt eine
andere Uhr, `TimeService::with_inaccuracy` ihren Fehler (höchstens 48 bit).
Eine sichere Quelle ist ein `SecureTimeSource`
(`crates/time-service/src/secure.rs`): eine Ablesung liefert Zeit,
Fehlerschranke und Synchronisationszustand desselben Augenblicks
(`SourceReading`). Unter Linux liest
`crates/dcps/src/time_source.rs::KernelClock` die Kernel-Uhr mit
`adjtimex(2)` (`maxerror` als Fehlerschranke; nicht synchronisiert bei
`STA_UNSYNC`, `STA_CLOCKERR`, `TIME_ERROR` oder `TIME_OOP`).
`crates/time-service/src/admin.rs::Administration::secure_now` prüft bei
jedem Aufruf von `secure_universal_time`: synchronisiert, Fehlerschranke
höchstens 48 bit, Zeit nicht vor der zuletzt gelieferten (monoton); sonst
`TimeUnavailable`. Die Quelle bestimmt ein berechtigter, protokollierter
Administrator (App. B).

**Tests:** `crates/time-service/src/service.rs::tests::universal_time_returns_recent_value`,
`time_unavailable_display_describes_failure_mode`,
`time_2_1_1_configured_inaccuracy_is_reported_and_bounded_to_48_bits`;
`crates/time-service/src/time_base.rs::tests::current_time_is_recent_century`;
`crates/time-service/src/admin.rs::tests::time_a_1_secure_time_returns_source_time_and_inaccuracy`,
`time_a_1_secure_time_unavailable_when_source_reports_unsynchronized`,
`time_a_1_secure_time_unavailable_when_reading_goes_backwards`,
`time_a_1_secure_time_unavailable_when_the_bound_exceeds_48_bits`;
`crates/dcps/src/time_source.rs::tests::time_a_1_kernel_reading_follows_the_adjtimex_state`,
`time_a_1_adjtimex_state_decides_secure_time_of_an_administered_service`,
`time_a_1_kernel_clock_reads_the_host_clock`,
`time_a_1_kernel_clock_as_authoritative_source`.

**Status:** done — die Tests liefen auf dem Linux-Testrechner grün. Beide Fälle, synchronisierte und nicht synchronisierte Kernel-Uhr, prüft `time_a_1_adjtimex_state_decides_secure_time_of_an_administered_service` mit festen `adjtimex`-Werten über `kernel_reading` in einer bestimmten Quelle eines verwalteten Dienstes, ohne die Uhr des Rechners. `time_a_1_kernel_clock_as_authoritative_source` liest die Uhr des Rechners und prüft nur den Fall, den der Rechner gerade liefert; es gibt ihn aus. Gemessen auf dem Linux-Testrechner: eine Probe mit `adjtimex(2)` ohne Rechte ergab Zustand `TIME_OK`, `STA_UNSYNC` nicht gesetzt, `maxerror` 108 369 µs; ein späterer Lauf des Tests mit `--no-capture` gab aus: synchronisiert, `maxerror` 173 875 µs, sichere Zeit geliefert.

---

## §1.2 General Object Model

### §1.2 General Object Model — Service-Object pattern

**Spec:** §1.2, S. 1-3 (PDF) — Service-Object verwaltet Instanz-Objekte
(UTOs/TIOs) über Service-Interface. CORBA-Object-Service-Pattern.

**Repo:** Rust-API: `TimeService` mit den Fabrikoperationen
`new_universal_time`, `uto_from_utc`, `new_interval` (`crates/time-service/src/service.rs`),
`TimerEventService::register` für die Handler (§2.2.1). CORBA-Objekte
(Feature `serve`): `crates/time-service/src/servant.rs::TimeServant`
dispatcht GIOP-Anfragen an das Dienstobjekt `CosTime::TimeService` (die
fünf Operationen von §2.1), an `UTO` (Attribute und Operationen von
§1.3.4) und `TIO` (§1.3.5) sowie die `CORBA::Object`-Operationen
(`_is_a`, `_non_existent`, `_repository_id`). Jede erzeugte UTO und TIO ist
ein eigenes CORBA-Objekt mit IOR; ihr Objektschlüssel trägt den
unveränderlichen Wert (`CosTime/UTO/` und die 16 Oktette des `UtcT`,
`CosTime/TIO/` und die 16 Oktette des `IntervalT`), der Server führt keine
Objekttabelle. Eine UTO oder TIO eines anderen Servers als Parameter liest
der Servant über IIOP mit `_get_utc_time` bzw. `_get_time_interval`; eine
nil- oder nicht lesbare Referenz ist `BAD_PARAM`. `TimeUnavailable` geht als
Benutzerausnahme, `BAD_PARAM` und `DATA_CONVERSION` als Systemausnahmen
hinaus, in beiden Bytereihenfolgen. `crates/time-service/src/server.rs::TimeServer`
bedient den Servant über IIOP; das Dienstobjekt ist
`corbaloc::<host>:<port>/TimeService`. Ein Programm startet `TimeServer`;
einen eigenständigen Daemon gibt es nicht. Über GIOP hebt
`secure_universal_time` immer `TimeUnavailable`. Die beiden Operationen, die
die Uhr des Dienstes lesen, `universal_time` und `absolute_time` einer UTO,
antworten nur mit `RemoteTimePolicy::AnswerCorrectingProxies`
(`TimeServer::start_with_policy`); unter dem Standard `Refuse` heben sie die
Systemausnahme `NO_PERMISSION` (App. B.2, siehe §2.1.1 und §1.3.4.5).

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

**Status:** partial — der Dienst CosTime (TimeService, UTO, TIO) ist als CORBA-Objekt gebaut, mit einem GIOP-Client im Prozess geprüft (`time_svc_e2e`, GIOP 1.0 und 1.2, beide Bytereihenfolgen) und gegen omniORB 4.3.3: `omniorb_e2e` lief auf dem Linux-Testrechner in einem Debian-trixie-Container mit omniidl und libomniORB4, 3 von 3 grün (mit beiden `RemoteTimePolicy`-Werten, auch `absolute_time`); im Job `integration-bridges-vendor-corba` eingetragen, dort noch nicht gelaufen. Es fehlt der zweite Dienst als CORBA-Objekt: `TimerEventService` und `TimerEventHandler` haben keinen GIOP-Dispatch, `register` nimmt keine entfernte `PushConsumer`-Referenz, und ein Timer-Ereignis geht nicht als `any` mit TypeCode über GIOP hinaus.

### §1.2.1 Conformance Points (Basic + Timer Event)

**Spec:** §1.2.1, S. 1-4 (PDF) — Zwei Conformance-Points:
"Basic Time Service" (TimeBase + CosTime) und "Timer Event Service"
(CosTimerEvent, optional, depends on Basic).

**Repo:** Basic Time Service: TimeBase und CosTime in `crates/time-service/`.
Timer Event Service: CosTimerEvent in `crates/time-service/src/timer_event.rs`
(Feature `timer-event`, Standard), auf dem `TimeService` desselben Crates;
die Ereignisse gehen an den `PushConsumer` von `crates/corba-cos-event/`.
Siehe §2.2.

**Tests:** Basic Time Service:
`crates/time-service/src/service.rs::tests::new_universal_time_creates_uto_from_components`,
`crates/time-service/src/uto.rs::tests::compare_time_midc_equal`,
`crates/time-service/src/tio.rs::tests::overlaps_partial`,
`crates/time-service/src/time_base.rs::tests::utct_size_is_16_octets`.
Timer Event Service: `crates/time-service/src/timer_event.rs::tests::time_2_4_1_register_starts_cleared_and_schedules_nothing`,
`time_2_3_1_2_set_timer_absolute_fires_at_the_time`;
`crates/time-service/tests/contract_corba_cos_event.rs::time_2_2_2_timer_events_flow_through_a_push_event_channel`.

**Status:** partial — CosTimerEvent ist als Rust-API vollständig, aber nicht als CORBA-Objekt über GIOP erreichbar (§1.2); CosTime ist als CORBA-Objekt erreichbar; die Aussage zu den Kriterien von App. A für den Basic Time Service steht in `crates/time-service/README.md` (§2.5, App. A).

---

## §1.3 Basic Time Service

### §1.3.1 Object Model — Service verwaltet UTOs + TIOs

**Spec:** §1.3.1, S. 1-4/1-5 (PDF) — Time Service verwaltet UTOs
(Universal Time Objects) und TIOs (Time Interval Objects) durch
Factory-Methods.

**Repo:** `crates/time-service/src/service.rs::TimeService`,
`crates/time-service/src/uto.rs::Uto`,
`crates/time-service/src/tio.rs::Tio`.

**Tests:** `crates/time-service/src/service.rs::tests` (8 Tests):
`new_universal_time_creates_uto_from_components`, `uto_from_utc_wraps_passed_struct`,
`new_interval_creates_tio_for_valid_bounds`,
`new_interval_rejects_lower_greater_than_upper`;
`crates/time-service/src/uto.rs::tests::attributes_return_constructor_values`,
`crates/time-service/src/tio.rs::tests::time_interval_attribute_returns_constructor_value`.

**Status:** done

### §1.3.2 Data Types

**Spec:** §1.3.2, S. 1-5 (PDF) — `module TimeBase { typedef
unsigned long long TimeT; typedef TimeT InaccuracyT; typedef short
TdfT; struct UtcT { TimeT time; unsigned long inacclo; unsigned short
inacchi; TdfT tdf; }; struct IntervalT { TimeT lower_bound; TimeT
upper_bound; }; };`.

**Repo:** `crates/time-service/src/time_base.rs::{TimeT, InaccuracyT,
TdfT, UtcT, IntervalT}`.

**Tests:** `crates/time-service/src/time_base.rs::tests::utct_size_is_16_octets`,
`intervalt_size_is_16_octets`.

**Status:** done

### §1.3.2.1 Type TimeT — 64-bit, 100ns ticks since 1582

**Spec:** §1.3.2.1, S. 1-6 (PDF) — "TimeT represents a single time
value, which is 64 bits in size, and holds the number of 100
nanoseconds that have passed since the base time."

**Repo:** `crates/time-service/src/time_base.rs::TimeT` (alias `u64`).

**Tests:** `crates/time-service/src/time_base.rs::tests::utct_wire_roundtrip_preserves_all_fields`.

**Status:** done

### §1.3.2.2 Type InaccuracyT — 48-bit Inaccuracy in 100ns

**Spec:** §1.3.2.2, S. 1-6 (PDF) — "represents the value of inaccuracy
in time in units of 100 nanoseconds. [...] 48 bits is sufficient."

**Repo:** `crates/time-service/src/time_base.rs::{InaccuracyT, MAX_INACCURACY}`;
`UtcT::new` und `UtcT::set_inaccuracy` lehnen einen Wert über 48 bit mit
`BadParam` ab, statt ihn zu kürzen.

**Tests:** `crates/time-service/src/time_base.rs::tests::time_1_3_2_2_utct_rejects_inaccuracy_above_48_bits`,
`time_1_3_2_2_set_inaccuracy_rejects_above_48_bits_and_keeps_the_value`.

**Status:** done

### §1.3.2.3 Type TdfT — 16-bit TimeZone-Offset in Minuten

**Spec:** §1.3.2.3, S. 1-6 (PDF) — "size 16 bits short type and holds
the time displacement factor in the form of minutes of displacement
from the Greenwich Meridian. [...] East ist positiv, West ist
negativ."

**Repo:** `crates/time-service/src/time_base.rs::TdfT` (alias `i16`).

**Tests:** `crates/time-service/src/time_base.rs::tests::local_time_negative_tdf_west_of_greenwich`.

**Status:** done

### §1.3.2.4 Type UtcT — 16-Octet-Struct mit Time + Inaccuracy + Tdf

**Spec:** §1.3.2.4, S. 1-6/1-7 (PDF) — "UtcT defines the structure of
the time value [...] basic value of time is of type TimeT [...]
inacclo and inacchi fields together hold a 48-bit estimate [...]
tdf field holds time zone information. [...] for any given UtcT value
'utc', the local time can be computed as utc.time + utc.tdf *
600,000,000."

**Repo:** `crates/time-service/src/time_base.rs::UtcT` mit
`UtcT::local_time()`-Operation. Der tdf jeder absoluten UTO, die der Dienst
erzeugt (`TimeService::universal_time`, `TimeService::absolute_time`,
`Uto::absolute_time`), ist der der lokalen Zeitzone zum Zeitpunkt der UTO:
`time_base::local_tdf` liest die Zone des Prozesses (Variable `TZ` als
IANA-Name, TZif-Pfad oder POSIX-Regel, sonst `/etc/localtime`, unter
Windows die Systemeinstellung) über die Bibliothek `jiff`, Sommerzeit
eingeschlossen. Windows hat keine Zonendatenbank des Systems: `jiff` bildet
die Windows-Zone auf ihren IANA-Namen ab und sucht ihn in der Datenbank, die
das Feature `tzdb-bundle-platform` nur unter Windows und wasm in das Programm
einbindet (`crates/time-service/Cargo.toml`). `TimeService::with_tdf` legt den
Wert fest (ohne `std` gibt es keine Zonendatenbank, dort ist der Standard 0).
Eine relative UTO behält tdf 0. `crates/dcps/src/time_source.rs` (`TimeSource::now_uto`) trägt
ebenfalls den lokalen tdf.

**Tests:** `crates/time-service/src/time_base.rs::tests::utct_size_is_16_octets`,
`local_time_applies_tdf`,
`local_time_negative_tdf_west_of_greenwich`;
`crates/time-service/src/service.rs::tests::time_1_3_2_4_universal_time_carries_the_local_tdf`
(Kindprozess je Zone: `CET-1CEST,M3.5.0,M10.5.0/3` im Januar 60, im Juli 120,
`EST5EDT,M3.2.0,M11.1.0` −300, `IST-5:30` 330, `UTC0` 0),
`time_1_3_2_4_absolute_time_carries_the_local_tdf`,
`time_1_3_2_4_stated_tdf_is_carried_by_universal_and_absolute_time`;
`crates/dcps/src/time_source.rs::tests::time_1_3_2_4_now_uto_carries_zero_inaccuracy_and_the_local_tdf`
(Kindprozess je Zone: CET im Januar 60, im Juli 120, `EST5EDT` −300 und
−240, `UTC0` 0),
`time_1_3_2_4_now_uto_carries_the_tdf_of_the_windows_zone` (nur unter
Windows: ohne `TZ` gleich dem Versatz, den Windows für dieselben Zeitpunkte
angibt).

**Status:** done — die Tests liefen auf dem Linux-Testrechner grün. Unter
Windows ist nichts gelaufen: `time_1_3_2_4_now_uto_carries_the_tdf_of_the_windows_zone`
läuft wöchentlich in den Jobs `platform-windows-dcps-1` und `-2`. Eine nicht
gefundene Zone erkennt er nur, wenn die Zone des Windows-Rechners im Januar
oder Juli einen Versatz hat.

### §1.3.2.5 Type IntervalT — Lower + Upper Bound

**Spec:** §1.3.2.5, S. 1-7 (PDF) — "two TimeT values corresponding to
the lower and upper bound of the interval. An IntervalT structure
containing a lower bound greater than the upper bound is invalid."

**Repo:** `crates/time-service/src/time_base.rs::IntervalT` mit
`IntervalT::new` rejects lower > upper (returns `None`).

**Tests:** `crates/time-service/src/time_base.rs::tests::intervalt_rejects_lower_greater_than_upper`,
`intervalt_size_is_16_octets`,
`intervalt_wire_roundtrip_preserves_bounds`.

**Status:** done

### §1.3.2.6 Enum ComparisonType — IntervalC vs MidC

**Spec:** §1.3.2.6, S. 1-7 (PDF) — "ComparisonType defines the two
types of time comparison. IntervalC comparison does the comparison
taking into account the error envelope. MidC comparison just compares
the base times. A MidC comparison can never return TCIndeterminate."

**Repo:** `crates/time-service/src/uto.rs::ComparisonType`.

**Tests:** `crates/time-service/src/uto.rs::tests::compare_time_midc_equal`,
`::compare_time_midc_less_than`, `::compare_time_midc_greater_than`,
`::compare_time_intervalc_equal_when_inaccuracy_zero_and_time_match`,
`::compare_time_intervalc_indeterminate_on_envelope_overlap`,
`::compare_time_intervalc_less_than_when_envelopes_disjoint`,
`::compare_time_intervalc_greater_than_when_envelopes_disjoint`.

**Status:** done

### §1.3.2.7 Enum TimeComparison — Equal/Less/Greater/Indeterminate

**Spec:** §1.3.2.7, S. 1-8 (PDF) — "TCEqualTo, TCLessThan, TCGreaterThan,
TCIndeterminate. TCIndeterminate value is returned if the error
envelopes around the two times being compared overlap."

**Repo:** `crates/time-service/src/uto.rs::TimeComparison`.

**Tests:** `crates/time-service/src/uto.rs::tests::compare_time_intervalc_indeterminate_on_envelope_overlap`.

**Status:** done

### §1.3.2.8 Enum OverlapType — Container/Contained/Overlap/NoOverlap

**Spec:** §1.3.2.8, S. 1-8 (PDF) — Vier Fälle gemäß Figure 1-3
(OTContainer, OTContained, OTOverlap, OTNoOverlap).

**Repo:** `crates/time-service/src/tio.rs::OverlapType`.

**Tests:** `crates/time-service/src/tio.rs::tests::overlaps_otcontainer`, `overlaps_otcontained`,
`overlaps_partial`, `overlaps_no_overlap`.

**Status:** done

---

## §1.3.3 Exceptions

### §1.3.3.1 TimeUnavailable

**Spec:** §1.3.3.1, S. 1-8 (PDF) — "raised when the underlying trusted
time service fails, or is unable to provide time that meets the
required security assurance."

**Repo:** `crates/time-service/src/service.rs::TimeUnavailable`; über GIOP
(Feature `serve`) die Benutzerausnahme
`IDL:omg.org/CosTime/TimeUnavailable:1.0` (`crates/time-service/src/servant.rs`).

**Tests:** `crates/time-service/src/service.rs::tests::time_a_1_secure_time_unavailable_on_a_service_that_is_not_administered`,
`time_unavailable_display_describes_failure_mode`;
`crates/time-service/src/servant.rs::tests::time_1_2_time_unavailable_is_the_cos_time_user_exception`;
`crates/time-service/tests/time_svc_e2e.rs::time_1_2_exceptions_reach_the_client`.

**Status:** done

---

## §1.3.4 Universal Time Object (UTO)

### §1.3.4.1 Readonly attribute time

**Spec:** §1.3.4.1, S. 1-9 (PDF) — "the time attribute of a UTO
represented as a value of type TimeT."

**Repo:** `crates/time-service/src/uto.rs::Uto::time()`.

**Tests:** `crates/time-service/src/uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.2 Readonly attribute inaccuracy

**Spec:** §1.3.4.2, S. 1-9 (PDF) — "of type InaccuracyT."

**Repo:** `crates/time-service/src/uto.rs::Uto::inaccuracy()`.

**Tests:** `crates/time-service/src/uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.3 Readonly attribute tdf

**Spec:** §1.3.4.3, S. 1-10 (PDF) — "time displacement factor [...]
of type TdfT."

**Repo:** `crates/time-service/src/uto.rs::Uto::tdf()`.

**Tests:** `crates/time-service/src/uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.4 Readonly attribute utc_time

**Spec:** §1.3.4.4, S. 1-10 (PDF) — "returns a properly populated UtcT
structure [...]."

**Repo:** `crates/time-service/src/uto.rs::Uto::utc_time()`.

**Tests:** `crates/time-service/src/uto.rs::tests::attributes_return_constructor_values`.

**Status:** done

### §1.3.4.5 Operation absolute_time

**Spec:** §1.3.4.5, S. 1-10 (PDF) — "Absolute time = current time +
time in the object. Raises CORBA::DATA_CONVERSION exception if the
attempt to obtain absolute time causes an overflow."

**Repo:** `crates/time-service/src/uto.rs::Uto::absolute_time()` (Plattformuhr)
und `crates/time-service/src/service.rs::TimeService::absolute_time` (Uhr und
Zone des Dienstes); Überlauf ist `AbsoluteTimeError::DataConversion`
(`CORBA::DATA_CONVERSION`), eine Uhr ohne Zeit
`AbsoluteTimeError::TimeUnavailable`. Das Ergebnis trägt den lokalen tdf
(§1.3.2.4). Über GIOP (Feature `serve`, `crates/time-service/src/servant.rs`): Überlauf
`DATA_CONVERSION`, eine Uhr ohne Zeit `TRANSIENT`. Unter der Standard-Politik
`RemoteTimePolicy::Refuse` hebt `absolute_time` einer bedienten UTO
`NO_PERMISSION`: das Ergebnis enthält die aktuelle Zeit des Dienstes, die
nicht mit unbekannter Latenz zu einem anderen Adressraum gehen darf
(App. B.2). Das ist für einen entfernten Aufrufer eine Abweichung von
§1.3.4.5. Mit `AnswerCorrectingProxies` antwortet der Servant;
`crates/time-service/src/proxy.rs::TimeServiceProxy::absolute_time` misst die
Umlaufzeit von `absolute_time` und `_get_utc_time` und verbreitert Zeit und
Ungenauigkeit um je die Hälfte.

**Tests:** `crates/time-service/src/uto.rs::tests::absolute_time_adds_current_to_relative`,
`time_1_3_4_5_absolute_time_overflow_is_data_conversion`;
`crates/time-service/src/service.rs::tests::time_1_3_4_5_absolute_time_on_the_service_clock`;
`crates/time-service/src/servant.rs::tests::time_1_2_uto_operations_compare_interval_absolute`,
`time_b_2_absolute_time_is_refused_by_default`;
`crates/time-service/src/proxy.rs::tests::time_b_2_absolute_time_correction_overflow_is_data_conversion`;
`crates/time-service/tests/time_svc_e2e.rs::time_b_2_the_server_gives_no_current_time_by_default`,
`time_b_2_the_proxy_widens_absolute_time_by_the_measured_latency`.

**Status:** done — die Tests liefen auf dem Linux-Testrechner grün. Im Adressraum des Dienstes und über GIOP mit `AnswerCorrectingProxies` wie §1.3.4.5; über GIOP unter `Refuse` die oben genannte Abweichung nach App. B.2.

### §1.3.4.6 Operation compare_time

**Spec:** §1.3.4.6, S. 1-10 (PDF) — "Compares the time contained in
the object with the time given in the input parameter uto using the
comparison type specified in the in parameter comparison_type."

**Repo:** `crates/time-service/src/uto.rs::Uto::compare_time(ct, uto)`.

**Tests:** `crates/time-service/src/uto.rs::tests::compare_time_midc_equal`,
`::compare_time_midc_less_than`, `::compare_time_midc_greater_than`,
`::compare_time_intervalc_equal_when_inaccuracy_zero_and_time_match`,
`::compare_time_intervalc_indeterminate_on_envelope_overlap`,
`::compare_time_intervalc_less_than_when_envelopes_disjoint`,
`::compare_time_intervalc_greater_than_when_envelopes_disjoint`.

**Status:** done

### §1.3.4.7 Operation time_to_interval

**Spec:** §1.3.4.7, S. 1-10 (PDF) — "Returns a TIO representing the
time interval between the time in the object and the time in the UTO
[...]. The interval returned is the interval between the midpoints
of the two UTOs and the inaccuracies in the UTOs are not taken into
consideration."

**Repo:** `crates/time-service/src/uto.rs::Uto::time_to_interval(other)`.

**Tests:** `crates/time-service/src/uto.rs::tests::time_to_interval_uses_midpoints`.

**Status:** done

### §1.3.4.8 Operation interval

**Spec:** §1.3.4.8, S. 1-10 (PDF) — "TIO.upper_bound = UTO.time + UTO.
inaccuracy. TIO.lower_bound = UTO.time - UTO.inaccuracy."

**Repo:** `crates/time-service/src/uto.rs::Uto::interval()`.

**Tests:** `crates/time-service/src/uto.rs::tests::interval_returns_inaccuracy_envelope`.

**Status:** done

---

## §1.3.5 Time Interval Object (TIO)

### §1.3.5.1 Readonly attribute time_interval

**Spec:** §1.3.5.1, S. 1-11 (PDF) — "returns an IntervalT structure
with the values of its fields filled in [...]."

**Repo:** `crates/time-service/src/tio.rs::Tio::time_interval()`.

**Tests:** `crates/time-service/src/tio.rs::tests::time_interval_attribute_returns_constructor_value`.

**Status:** done

### §1.3.5.2 Operation spans

**Spec:** §1.3.5.2, S. 1-11 (PDF) — "Returns a value of type OverlapType
depending on how the interval in the object and the time range
represented by the parameter UTO overlap. [...] If OverlapType is
not OTNoOverlap, then the out parameter overlap contains the overlap
interval, otherwise the out parameter contains the gap between the
two intervals."

**Repo:** `crates/time-service/src/tio.rs::Tio::spans(uto)`.

**Tests:** `crates/time-service/src/tio.rs::tests::spans_uses_uto_inaccuracy_envelope`.

**Status:** done

### §1.3.5.3 Operation overlaps

**Spec:** §1.3.5.3, S. 1-11 (PDF) — "Returns a value of type
OverlapType depending on how the interval in the object and interval
in the parameter TIO overlap."

**Repo:** `crates/time-service/src/tio.rs::Tio::overlaps(tio)`.

**Tests:** `crates/time-service/src/tio.rs::tests::overlaps_otcontainer`, `overlaps_otcontained`,
`overlaps_partial`, `overlaps_no_overlap`.

**Status:** done

### §1.3.5.4 Operation time

**Spec:** §1.3.5.4, S. 1-11 (PDF) — "Returns a UTO in which the
inaccuracy interval is equal to the time interval in the TIO and time
value is the midpoint of the interval."

**Repo:** `crates/time-service/src/tio.rs::Tio::time()`; ist die halbe Breite
größer als 48 bit, ist das Ergebnis `DataConversion` statt einer gekürzten
Ungenauigkeit.

**Tests:** `crates/time-service/src/tio.rs::tests::time_method_returns_uto_with_midpoint_and_half_width`,
`time_1_3_5_4_interval_wider_than_48_bit_inaccuracy_is_data_conversion`.

**Status:** done

---

## §2.1 Time Service Interface

### §2.1.1 Operation universal_time

**Spec:** §2.1.1, S. 2-2 (PDF) — "returns the current time and an
estimate of inaccuracy in a UTO. It raises TimeUnavailable exceptions
to indicate failure of an underlying time provider. The time returned
in the UTO by this operation is not guaranteed to be secure or
trusted."

**Repo:** `crates/time-service/src/service.rs::TimeService::universal_time`:
die Zeit der Uhr mit ihrer Ungenauigkeit, `TimeUnavailable` nur, wenn die
Uhr keine Zeit liefert. Über GIOP (Feature `serve`,
`crates/time-service/src/servant.rs`): mit
`RemoteTimePolicy::AnswerCorrectingProxies` ebenso, `TimeUnavailable` als
Benutzerausnahme. Unter der Standard-Politik `Refuse` hebt das bediente Objekt
die Systemausnahme `NO_PERMISSION`, auch wenn die Uhr eine Zeit liefert. Das
weicht für einen entfernten Aufrufer von §2.1.1 ab („If any time is available
at all, that time is returned by this operation"). Grund ist App. B.2: eine
Zeit, die mit unbekannter Latenz ankommt, darf den Aufrufer nicht erreichen;
der Aufrufer nimmt einen Time Service im eigenen Adressraum (erster Weg von
B.2). `TimeUnavailable` wird für diese Verweigerung nicht verwendet; es behält
die Bedeutung aus §2.1.1, den Ausfall der Zeitquelle (§1.2.1 schließt eine
`universal_time` aus, die immer `TimeUnavailable` hebt). Beschrieben in
`crates/time-service/README.md`, „Proxies (B.2)".

**Tests:** `crates/time-service/src/service.rs::tests::universal_time_returns_recent_value`;
`crates/time-service/src/servant.rs::tests::time_1_2_universal_time_returns_a_uto_reference_with_the_value_in_its_key`,
`time_1_2_time_unavailable_is_the_cos_time_user_exception`,
`time_b_2_universal_time_is_refused_by_default`;
`crates/time-service/tests/time_svc_e2e.rs::time_b_2_the_server_gives_no_current_time_by_default`.

**Status:** done — die Tests liefen auf dem Linux-Testrechner grün. Im Adressraum des Dienstes und über GIOP mit `AnswerCorrectingProxies` wie §2.1.1; über GIOP unter `Refuse` die oben genannte Abweichung nach App. B.2.

### §2.1.2 Operation secure_universal_time

**Spec:** §2.1.2, S. 2-2 (PDF) — "returns the current time in a UTO
only if the time can be guaranteed to have been obtained securely.
[...] If there is any uncertainty at all about meeting any aspect of
these criteria, then this operation must return the TimeUnavailable
exception."

**Repo:** `crates/time-service/src/service.rs::TimeService::secure_universal_time`
mit `crates/time-service/src/admin.rs::Administration::secure_now`: eine Zeit
nur bei einem verwalteten Dienst (`TimeService::administered`), einer über
`TimeAdmin::designate_source` bestimmten Quelle, deren Synchronisation als
geschützt angegeben ist (`SyncProtection`, nicht `Unprotected`), ohne von
Hand gesetzte Zeit, aus einer synchronisierten Ablesung mit Fehlerschranke
bis 48 bit und nicht vor der zuletzt gelieferten Zeit; in jedem anderen Fall
`TimeUnavailable`, ebenso ohne `std` und über GIOP (`crates/time-service/src/servant.rs`). Die
Kriterien und ihre Behandlung: `crates/time-service/README.md`, Abschnitt
„Secure time (Appendix A and B)".

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

**Status:** done — die Tests liefen auf dem Linux-Testrechner grün.

### §2.1.2.1 Operation new_universal_time

**Spec:** §2.1.2.1, S. 2-2 (PDF) — "constructing a new UTO. The
parameters passed in are the time of type TimeT and inaccuracy of
type InaccuracyT. [...] CORBA::BAD_PARAM is raised in the case of an
out-of-range parameter value for inaccuracy."

**Repo:** `crates/time-service/src/service.rs::TimeService::new_universal_time`
liefert `Result<Uto, BadParam>`: eine Ungenauigkeit über 48 bit
(`MAX_INACCURACY`) ist `BadParam` (`CORBA::BAD_PARAM`).

**Tests:** `crates/time-service/src/service.rs::tests::new_universal_time_creates_uto_from_components`,
`time_2_1_2_1_inaccuracy_above_48_bits_is_bad_param`,
`time_2_1_2_1_inaccuracy_at_48_bit_limit_is_accepted`;
`crates/time-service/src/uto.rs::tests::time_2_1_2_1_uto_new_rejects_inaccuracy_above_48_bits`.

**Status:** done

### §2.1.2.2 Operation uto_from_utc

**Spec:** §2.1.2.2, S. 2-2 (PDF) — "create a UTO given a time in the
UtcT form. [...] used to convert a UTC received over the wire into a
UTO."

**Repo:** `crates/time-service/src/service.rs::TimeService::uto_from_utc`.

**Tests:** `crates/time-service/src/service.rs::tests::uto_from_utc_wraps_passed_struct`.

**Status:** done

### §2.1.2.3 Operation new_interval

**Spec:** §2.1.2.3, S. 2-3 (PDF) — "If the value of the lower parameter
is greater than the value of the upper parameter, then a CORBA::
BAD_PARAM exception is raised."

**Repo:** `crates/time-service/src/service.rs::TimeService::new_interval`
(`BadParam` bei lower > upper).

**Tests:** `crates/time-service/src/service.rs::tests::new_interval_rejects_lower_greater_than_upper`,
`new_interval_creates_tio_for_valid_bounds`.

**Status:** done

---

## §2.2 Timer Event Service

### §2.2.1 Object Model — TimerEventService managed TimerEventHandlers

**Spec:** §2.2.1, S. 2-3 (PDF) — "The TimerEventService object manages
Timer Event Handlers. [...] Each Timer Event Handler is immutably
associated with a specific event channel at the time of its creation."

**Repo:** `crates/time-service/src/timer_event.rs::TimerEventService`
verwaltet die `TimerEventHandler`: `register` bindet jeden Handler fest an
den übergebenen `PushConsumer` (keine Operation ändert ihn), `unregister`
gibt ihn frei, ein Handler eines anderen oder eines beendeten Dienstes ist
`INV_OBJREF`. Ein Worker-Thread je Dienst löst die Ereignisse aus.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_4_1_register_starts_cleared_and_schedules_nothing`,
`time_2_4_2_unregister_then_use_is_inv_objref`,
`time_2_4_2_handler_of_another_service_is_inv_objref`.

**Status:** done

### §2.2.2 Usage — Push-Event-Channel + Timer-Events

**Spec:** §2.2.2, S. 2-4 (PDF) — Workflow: Event-Channel erstellen,
TimerEventHandler registrieren, Timer setzen, Events über Channel
pushen.

**Repo:** Push-Modell: ein Ereignis geht als `AnyEvent` mit der
Repository-ID `IDL:omg.org/CosTimerEvent/TimerEventT:1.0` an
`zerodds_corba_cos_event::comm::PushConsumer`, auch an den
`ProxyPushConsumer` eines `EventChannel`. Das erwartete Intervall steht in
der Moduldokumentation von `crates/time-service/src/timer_event.rs` und im
README des Crates: nie vor der angeforderten Zeit, danach um die
Aufweckzeit des Betriebssystems und die Dauer vorher fälliger Pushes
verspätet; die Uhr wird mindestens einmal je Sekunde neu gelesen. Gemessen
auf dem Linux-Testrechner: höchstens 68 µs Verspätung bei 20 Auslösungen
von 5 ms. Jedes `TimerEventT` trägt die tatsächliche Zeit und die
Verspätung als Ungenauigkeit.

**Tests:** `crates/time-service/tests/contract_corba_cos_event.rs::time_2_2_2_timer_events_flow_through_a_push_event_channel`;
`crates/time-service/src/timer_event.rs::tests::time_2_2_2_trigger_delay_is_reported_and_bounded`,
`time_2_3_1_2_set_timer_absolute_fires_at_the_time`.

**Status:** done

### §2.2.3 Data Types CosTimerEvent

**Spec:** §2.2.3, S. 2-4 (PDF) — `module CosTimerEvent { enum
TimeType { TTAbsolute, TTRelative, TTPeriodic }; enum EventStatus
{ ESTimeSet, ESTimeCleared, ESTriggered, ESFailedTrigger }; struct
TimerEventT { TimeBase::UtcT utc; any event_data; }; };`.

**Repo:** `crates/time-service/src/timer_event.rs::{TimeType, EventStatus,
TimerEventT}`; `TimerEventT` ist `{ utc: UtcT, event_data: AnyEvent }`,
`AnyEvent` das `any` von `zerodds-corba-cos-event` (Repository-ID und
CDR-Kapselung). `TimerEventT::to_any` schreibt die Struktur als
CDR-Kapselung (`UtcT`, dann das `any` als `string` und `sequence<octet>`);
`TimerEventT::from_any` liest beide Bytereihenfolgen und lehnt eine falsche
Repository-ID, Längen über das Ende hinaus und übrige Bytes mit `Marshal`
ab.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_2_3_timer_event_t_any_roundtrip_both_byte_orders`,
`time_2_2_3_timer_event_t_malformed_any_is_marshal`,
`time_2_2_3_3_inaccuracy_is_requested_minus_actual`.

**Status:** done

### §2.2.3.1 Enum TimeType

**Spec:** §2.2.3.1, S. 2-4 (PDF) — TTAbsolute / TTRelative / TTPeriodic.

**Repo:** `crates/time-service/src/timer_event.rs::TimeType::{Absolute,
Relative, Periodic}`; `TimerEventHandler::set_timer` wertet alle drei aus
(§2.3.1.2).

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_3_1_2_set_timer_absolute_fires_at_the_time`,
`time_2_3_1_2_set_timer_relative_zero_reuses_last`,
`time_2_2_3_2_periodic_never_triggered_stays_time_set`.

**Status:** done

### §2.2.4 Exceptions

**Spec:** §2.2.4, S. 2-5 (PDF) — Exception-Definitionen für
TimerEventService.

**Repo:** Keine dienstspezifischen Ausnahmen.
`crates/time-service/src/timer_event.rs::TimerEventError` benennt die
CORBA-Systemausnahmen `BAD_PARAM` (§2.3.1.2), `NO_RESOURCES` (§2.4.1),
`INV_OBJREF` (§2.4.2) und den Fall einer Uhr ohne Zeit.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_3_1_2_set_timer_relative_zero_without_previous_is_bad_param`,
`time_2_4_1_register_beyond_capacity_is_no_resources`,
`time_2_4_2_unregister_then_use_is_inv_objref`.

**Status:** done

---

## §2.3 Timer Event Handler

### §2.3.1 Attribute status

**Spec:** §2.3, S. 2-5/2-6 (PDF) — `TimerEventHandler` Interface mit
Attribut `status` und Operations `set_timer/cancel_timer/set_data/
time_set`.

**Repo:** `crates/time-service/src/timer_event.rs::TimerEventHandler` mit
`status`, `time_set`, `set_timer`, `cancel_timer`, `set_data`. `status` hat
die vier Zustände von §2.2.3.2: `TimeCleared` nach `register` und nach
`cancel_timer`, `TimeSet` nach `set_timer` und nach jeder periodischen
Auslösung, `Triggered` nach einer zugestellten einmaligen Auslösung,
`FailedTrigger`, wenn der Push mit `Disconnected` scheitert (ein
periodischer Handler hält dann an). `time_set` liefert das Flag und den
aktuellen Timerwert als UTO. `set_timer` hebt den vorigen Auslöser auf;
absolut zur angegebenen Zeit, relativ zur aktuellen Zeit plus Wert,
periodisch in diesem Abstand; ein relativer oder periodischer Wert 0 nimmt
die letzte relative Zeit, ohne eine ist er `BAD_PARAM`, ebenso eine Zeit
über `TimeT` hinaus. `cancel_timer` liefert `true` nur für einen
ausstehenden Auslöser. `set_data` gilt für alle späteren Auslösungen.

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

## §2.4 Timer Event Service Operations

### §2.4.1 Operation register

**Spec:** §2.4.1, S. 2-7 (PDF) — `TimerEventHandler register(in
CosEventComm::PushConsumer event_interface, in any data);`.

**Repo:** `crates/time-service/src/timer_event.rs::TimerEventService::register`
nimmt `Arc<dyn PushConsumer>` und `AnyEvent`: Zeit des Handlers = aktuelle
Zeit, Zustand `TimeCleared`, nichts geplant. Über der Handler-Grenze
(`DEFAULT_MAX_HANDLERS` oder `TimerEventService::with_capacity`) ist das
Ergebnis `NO_RESOURCES`, ebenso wenn der Worker-Thread nicht startet.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_4_1_register_starts_cleared_and_schedules_nothing`,
`time_2_4_1_register_beyond_capacity_is_no_resources`;
`crates/time-service/tests/contract_corba_cos_event.rs::contract_push_consumer_receives_the_timer_event_as_any`.

**Status:** done

### §2.4.2 Operation unregister

**Spec:** §2.4.2, S. 2-7 (PDF) — `void unregister(in TimerEventHandler);`.

**Repo:** `crates/time-service/src/timer_event.rs::TimerEventService::unregister`
entfernt den Handler und seinen ausstehenden Auslöser; jede spätere Nutzung
des Handlers ist `INV_OBJREF`.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_4_2_unregister_then_use_is_inv_objref`,
`time_2_4_2_unregister_cancels_the_pending_trigger`,
`time_2_4_2_handler_of_another_service_is_inv_objref`.

**Status:** done

### §2.4.3 Operation event_time

**Spec:** §2.4.3, S. 2-7 (PDF) — `UTO event_time(in CosTimerEvent::
TimerEventT event);`.

**Repo:** `crates/time-service/src/timer_event.rs::TimerEventService::event_time`
liefert eine UTO mit der Auslösezeit und der Ungenauigkeit des Ereignisses.

**Tests:** `crates/time-service/src/timer_event.rs::tests::time_2_4_3_event_time_returns_uto`.

**Status:** done

---

## §2.5 Conformance

### §2.5 Conformance Statement

**Spec:** §2.5, S. 2-7 (PDF) — Two conformance points (Basic Time
Service + Timer Event Service); Timer Event Service requires Basic
Time Service.

**Repo:** Basic Time Service (§1.3, §2.1) in `crates/time-service/`;
Timer Event Service (§2.2 bis §2.4) in
`crates/time-service/src/timer_event.rs`, auf dem `TimeService` desselben
Crates.

**Tests:** Basic Time Service: `crates/time-service/src/service.rs::tests`,
`crates/time-service/src/uto.rs::tests`,
`crates/time-service/src/tio.rs::tests`,
`crates/time-service/src/time_base.rs::tests` — darunter
`utct_wire_roundtrip_preserves_all_fields`, `time_1_3_2_2_utct_rejects_inaccuracy_above_48_bits`,
`local_time_applies_tdf`, `spans_uses_uto_inaccuracy_envelope`.
Timer Event Service: `crates/time-service/src/timer_event.rs::tests::time_2_2_3_2_status_triggered_after_one_shot`,
`time_2_3_1_2_set_timer_relative_zero_reuses_last`;
`crates/time-service/tests/contract_corba_cos_event.rs::time_2_2_2_timer_events_flow_through_a_push_event_channel`.
Aussage zu App. A: `crates/time-service/src/secure.rs::tests::time_a_1_readme_states_every_criterion`.

**Status:** partial — der Timer Event Service ist als Rust-API vollständig, über GIOP nicht erreichbar (§1.2). Basic Time Service: erfüllt; die Aussage, wie die Kriterien von App. A behandelt werden, steht in `crates/time-service/README.md` („Secure time (Appendix A and B)"), und `secure_universal_time` liefert nur unter diesen Kriterien eine Zeit (§2.1.2, App. A).

---

## Appendix A — Implementation Guidelines

### App. A — Secure Time Source Criteria

**Spec:** App. A, S. A-1 (PDF) — "Appropriate documented handling of the criteria presented here is mandatory for conformance to the Basic Time Service conformance point." A.1: "If these criteria are not satisfactorily addressed in an ORB, then it must return the TimeUnavailable exception upon invocation of the secure_universal_time operation".

**Repo:** Dokumentierte Behandlung: `crates/time-service/README.md`, Abschnitt
„Secure time (Appendix A and B)" (Kriterien 1 bis 8 mit Spec-Stelle,
Behandlung und Test; sicher ja/nein), kurz in `crates/time-service/src/lib.rs`.
A.1 kündigt Kriterien an, ohne sie aufzuführen (S. A-2 ist leer); die Tabelle
nimmt die von §1.1.3, §2.1.2 und App. B. Prüfung bei jedem Aufruf:
`crates/time-service/src/admin.rs::Administration::secure_now`; Quellen:
`crates/time-service/src/secure.rs::{SecureTimeSource, SourceReading, SyncProtection, SourceDesignation}`,
unter Linux `crates/dcps/src/time_source.rs::KernelClock`. Ohne erfüllte
Kriterien hebt `secure_universal_time` `TimeUnavailable` (A.1).

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

**Status:** done — die Tests liefen auf dem Linux-Testrechner grün. Synchronisierte und nicht synchronisierte Kernel-Uhr prüft `time_a_1_adjtimex_state_decides_secure_time_of_an_administered_service` mit festen `adjtimex`-Werten; `time_a_1_kernel_clock_as_authoritative_source` prüft nur den Fall, den der Rechner liefert, und gibt ihn aus. Gemessen auf dem Linux-Testrechner: eine Probe mit `adjtimex(2)` ergab `TIME_OK`, `maxerror` 108 369 µs; ein späterer Lauf des Tests gab aus: synchronisiert, `maxerror` 173 875 µs, sichere Zeit geliefert. Ob die Synchronisation des Rechners im Transport geschützt ist, prüft ZeroDDS nicht: der Administrator gibt es bei der Bestimmung der Quelle an (`SyncProtection`), der Audit-Datensatz hält fest, wer.

---

## Appendix B-F

### App. B — Administration

**Spec:** App. B, S. B-1 (PDF) — "Only administrators authorized by the system security policy may set the time and specify the source of time"; Operationen, die die Zeit setzen, eine Quelle als maßgeblich bestimmen oder die Genauigkeit ändern, "must be protected against unauthorized invocation. They must also be mandatorily audited"; "Time Service implementors must state whether or not their implementation is secure."

**Repo:** B.1: `crates/time-service/src/admin.rs::{TimeAdmin, AdminAuthorizer, DenyAll, AdminSubject, AdminOp, AuditSink, AuditRecord, AuditOutcome, AdminError}`.
`TimeService::administered` gibt den Dienst mit seinem `TimeAdmin` zurück;
`set_time`, `reset_time`, `designate_source`, `withdraw_source` und
`set_inaccuracy` laufen nacheinander: Berechtigung durch den
`AdminAuthorizer` (die Sicherheitsrichtlinie; `DenyAll` lehnt alles ab),
Prüfung der Parameter, Audit-Datensatz (gewährt, abgelehnt, fehlgeschlagen;
Zeit, Subjekt, Operation, Wert vorher und nachher), erst dann die Änderung;
kann der Datensatz nicht geschrieben werden, ändert sich nichts. Die Felder
sind privat; `with_tdf` und `with_inaccuracy` an einem verwalteten Dienst
ergeben einen neuen, nicht verwalteten Dienst. JSON-Zeilen:
`crates/security-logging/src/time_audit.rs` (Feature `time-service`,
`JsonLinesLoggingPlugin::append_synced` schreibt ohne Pegelfilter und mit
`sync_data`). B.1.1 und B.2: `crates/time-service/README.md`, Abschnitt
„Secure time (Appendix A and B)" (sicher ja/nein, Schutz der
Synchronisationswerte im Transport, die sechs Bedrohungen aus CORBA Security
Service 1.8 §1.1.3, Proxies). B.2:
`crates/time-service/src/servant.rs::RemoteTimePolicy`. Standard `Refuse`:
keine aktuelle Zeit des Dienstes über GIOP; die beiden Operationen, die seine
Uhr lesen, `universal_time` und `absolute_time` einer UTO („current time +
time in the object", §1.3.4.5), heben die Systemausnahme `NO_PERMISSION`
(`REFUSED_REMOTE_TIME`); die Operationen ohne Uhr werden bedient.
`secure_universal_time` über GIOP immer `TimeUnavailable`.
`AnswerCorrectingProxies` beantwortet `universal_time` und `absolute_time`
für Aufrufer, die die Latenz selbst ausgleichen:
`crates/time-service/src/proxy.rs::TimeServiceProxy::{universal_time, absolute_time}`
misst die Umlaufzeit auf der monotonen Uhr und verbreitert Zeit und
Ungenauigkeit um je die Hälfte; `secure_universal_time` des Proxys liefert
nie eine Zeit. Die Abweichungen von §2.1.1 und §1.3.4.5 unter `Refuse` stehen
dort und in der README.

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

**Status:** done — die Tests liefen auf dem Linux-Testrechner grün. `omniorb_e2e` lief auf dem Linux-Testrechner in einem Debian-trixie-Container (omniidl und libomniorb4-dev 4.3.3, g++ 14.2), 3 von 3 grün: der omniORB-Client erhält unter `RemoteTimePolicy::Refuse` für `universal_time` und für `absolute_time` einer UTO der Zeit 0 die Systemausnahme `NO_PERMISSION`, unter `AnswerCorrectingProxies` die Zeit, `secure_universal_time` in beiden Fällen `TimeUnavailable`; im Job `integration-bridges-vendor-corba` eingetragen, dort nicht gelaufen. Ob die Synchronisation des Rechners im Transport geschützt ist, prüft ZeroDDS nicht (B.1.1: der Administrator gibt es an).

### App. C — IDL Listing

**Spec:** App. C (PDF) — IDL für TimeBase + CosTime + CosTimerEvent
(non-normativ als Appendix; normativ-äquivalente Definitionen sind
in §1.3 + §2.x).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Zusammenfassung des IDL aus §1.3 und §2; die Definitionen sind dort einzeln geführt.

### App. D — Notes

**Spec:** App. D (PDF) — Implementation-Notes (non-normativ).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Hinweise für Anwender.

### App. E — Examples

**Spec:** App. E (PDF) — Beispiel-Code (non-normativ).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Beispiele für Erweiterungen.

### App. F — References

**Spec:** App. F (PDF) — externe Referenzen (non-normativ).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

---

## Audit-Status

42 done / 3 partial / 0 open / 0 blocked / 4 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document omg-time-1.1.md`):

* `cargo nextest run --profile merge -p zerodds-corba-ccm --all-targets` (Job `merge-rust-corba-ccm`) — 17 Tests grün.
* `cargo nextest run --profile merge -p zerodds-time-service --all-targets` (Job `merge-rust-time-service`) — 35 Tests grün.

52 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `omg-time-1.1.open.md`.
