// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-time-service`. Safety classification: **STANDARD**.
//!
//! OMG Time Service 1.1 — Datatypes + Operations (Spec formal/2002-05-07).
//! Pure-Rust, `forbid(unsafe_code)`.
//!
//! ## Spec
//!
//! - **OMG Time Service 1.1** §1.3.2 (TimeBase: TimeT/InaccuracyT/TdfT/UtcT/IntervalT)
//! - **OMG Time Service 1.1** §1.3.3 (TimeUnavailable Exception; the
//!   CORBA system exceptions `BAD_PARAM`, `DATA_CONVERSION` in
//!   [`exception`])
//! - **OMG Time Service 1.1** §1.3.4 (UTO — Universal Time Object)
//! - **OMG Time Service 1.1** §1.3.5 (TIO — Time Interval Object)
//! - **OMG Time Service 1.1** §2.1 (TimeService Interface)
//! - **OMG Time Service 1.1** §2.2 to §2.4 (CosTimerEvent: TimerEventService,
//!   TimerEventHandler, TimerEventT) in [`timer_event`], feature
//!   `timer-event` (default); events go to a
//!   `zerodds_corba_cos_event::comm::PushConsumer`.
//! - **OMG Time Service 1.1** App. A, App. B (secure time, administration
//!   of time, proxies) in [`secure`], `admin` (feature `std`), the
//!   servant's `RemoteTimePolicy` and `proxy` (feature `serve`); the
//!   statement is in the section "Secure time" below and in the README.
//!
//! ## Layer position
//!
//! Layer 1 — primitives. DDS-DCPS has its own `Time_t` with an 8-byte
//! wire format per DDS-DCPS §2.3.3, byte-distinct from the OMG Time
//! Service `UtcT` with a 16-byte wire format and a 1582 epoch; `zerodds-dcps`
//! uses this crate only for its `TimeServiceClock` adapter and the `Uto` of
//! `TimeSource::now_uto`. Further consumers are applications that need
//! OMG-Time-Service-1.1 conformance (e.g. the tutorial
//! `dds-warehouse/02-time-sync`). With `timer-event` it depends on
//! `zerodds-corba-cos-event` (no dependencies of its own).
//!
//! ## Public API
//!
//! **TimeBase** ([`time_base`]):
//! - [`TimeT`] — 64-bit 100ns tick counter (spec §1.3.2.1).
//! - [`time_base::InaccuracyT`] / [`TdfT`] — 48-bit inaccuracy + 16-bit
//!   time-displacement factor.
//! - [`UtcT`] — 16-byte wire struct with `to_wire`/`from_wire` roundtrip;
//!   an inaccuracy above [`MAX_INACCURACY`] (48 bits) is [`BadParam`].
//! - [`IntervalT`] — lower/upper bound with validation.
//! - [`time_base::current_time`] — wall clock in TimeT format
//!   (the no_std stub returns `0`).
//! - [`time_base::local_tdf`] — the time displacement factor of the local
//!   time zone at a given time (`std`; spec §1.3.2.4).
//! - [`time_base::UTC_EPOCH_TO_UNIX_TICKS`] / [`time_base::TICKS_PER_SECOND`].
//!
//! **UTO** ([`uto`]):
//! - [`Uto`] — Universal Time Object with operations (`absolute_time`,
//!   `compare_time`, `time_to_interval`, `interval`).
//! - [`ComparisonType::{IntervalC, MidC}`] — comparison mode.
//! - [`TimeComparison::{EqualTo, LessThan, GreaterThan, Indeterminate}`].
//!
//! **TIO** ([`tio`]):
//! - [`Tio`] — Time Interval Object with operations (`time_interval`,
//!   `spans`, `overlaps`, `time`).
//! - [`OverlapType`] — overlap classification.
//!
//! **Service** ([`service`]):
//! - [`TimeService`] — `universal_time` / `secure_universal_time` /
//!   `new_universal_time` / `uto_from_utc` / `new_interval` /
//!   `absolute_time` / `with_source` (pluggable clock: the `no_std`
//!   embedding hook, and a `std` override for a disciplined clock,
//!   `CLOCK_TAI` or a deterministic test clock) / `with_tdf` /
//!   `with_inaccuracy`. Absolute UTOs carry the time displacement factor
//!   of the local time zone (`std`) or the one stated with `with_tdf`.
//! - [`TimeUnavailable`] — exception type.
//!
//! **Timer Event Service** ([`timer_event`], feature `timer-event`):
//! - `TimerEventService` — `register` (PushConsumer + data) /
//!   `unregister` / `event_time`; one worker thread pushes the events.
//! - `TimerEventHandler` — `status` / `time_set` / `set_timer` /
//!   `cancel_timer` / `set_data`.
//! - `TimerEventT`, `TimeType`, `EventStatus`, `TimerEventError`.
//!
//! **Administration and secure time** ([`secure`], `admin`):
//! - `TimeService::administered` (feature `std`) — the service and its
//!   `admin::TimeAdmin`: `set_time`, `reset_time`, `designate_source`,
//!   `withdraw_source`, `set_inaccuracy`, each authorized by an
//!   `admin::AdminAuthorizer` and recorded by an `admin::AuditSink`
//!   before it takes effect (App. B.1).
//! - [`secure::SecureTimeSource`], [`secure::SourceReading`],
//!   [`secure::SyncProtection`], [`secure::SourceDesignation`].
//!
//! **CORBA objects** (feature `serve`): `servant` dispatches GIOP
//! requests for `CosTime::TimeService`, `UTO` and `TIO`; `server` serves
//! it over IIOP (`corbaloc::host:port/TimeService`); `proxy` is the
//! latency-correcting client of App. B.2.
//!
//! ## Secure time (App. A, App. B)
//!
//! A service built with `default`, `with_source`, `with_tdf` or
//! `with_inaccuracy` is not secure: `secure_universal_time` raises
//! `TimeUnavailable`. Under `no_std` there is no administration and no
//! secure time. An administered service returns a time from
//! `secure_universal_time` only when, at that call:
//!
//! 1. an authoritative source was designated through
//!    `TimeAdmin::designate_source`, an operation the system security
//!    policy (`AdminAuthorizer`) allowed and the `AuditSink` recorded
//!    before it took effect (App. B, B.1);
//! 2. the administrator stated the synchronization of that source as
//!    protected in transit ([`secure::SyncProtection`], not
//!    `Unprotected`; App. B.1.1);
//! 3. no time set by hand (`TimeAdmin::set_time`) is in effect;
//! 4. the source could be read, reported its clock synchronized and gave
//!    an error bound that fits 48 bits (§1.1.3); the UTO carries that
//!    bound, or the larger administered inaccuracy;
//! 5. the time is not before the last time `secure_universal_time`
//!    returned (§1.1.3, "monotonically increasing").
//!
//! Over the network no secure time is given: the servant raises
//! `TimeUnavailable` for `secure_universal_time` (IIOP does not protect
//! it in transit, B.1.1; the latency to the caller is not known, B.2).
//! It gives no current time either, unless its `RemoteTimePolicy`
//! answers latency-correcting proxies (B.2): `universal_time` and
//! `absolute_time` raise `CORBA::NO_PERMISSION`. ZeroDDS synchronizes no clock and
//! sends no synchronization values; the threats of the CORBA Security
//! Service (§1.1.3 of that specification) and how each is met are in the
//! README.
//!
//! ## Example
//!
//! ```rust
//! use zerodds_time_service::{TimeService, ComparisonType, TimeComparison};
//!
//! let service = TimeService::default();
//! # if let Ok(now) = service.universal_time() {
//! let later = service.universal_time().unwrap();
//! assert_ne!(
//!     now.compare_time(ComparisonType::MidC, later),
//!     TimeComparison::GreaterThan
//! );
//! # }
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
pub mod admin;
pub mod exception;
#[cfg(feature = "serve")]
pub mod proxy;
pub mod secure;
#[cfg(feature = "serve")]
pub mod servant;
#[cfg(feature = "serve")]
pub mod server;
pub mod service;
pub mod time_base;
#[cfg(feature = "timer-event")]
pub mod timer_event;
pub mod tio;
pub mod uto;

pub use exception::{AbsoluteTimeError, BadParam, DataConversion};
pub use service::{TimeService, TimeUnavailable};
pub use time_base::{IntervalT, MAX_INACCURACY, TdfT, TimeT, UtcT};
pub use tio::{OverlapType, Tio};
pub use uto::{ComparisonType, TimeComparison, Uto};
