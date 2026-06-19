// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-time-service`. Safety classification: **STANDARD**.
//!
//! OMG Time Service 1.1 — Datatypes + Operations (Spec formal/2002-05-07).
//! Pure-Rust no_std + alloc, `forbid(unsafe_code)`.
//!
//! ## Spec
//!
//! - **OMG Time Service 1.1** §1.3.2 (TimeBase: TimeT/InaccuracyT/TdfT/UtcT/IntervalT)
//! - **OMG Time Service 1.1** §1.3.3 (TimeUnavailable Exception)
//! - **OMG Time Service 1.1** §1.3.4 (UTO — Universal Time Object)
//! - **OMG Time Service 1.1** §1.3.5 (TIO — Time Interval Object)
//! - **OMG Time Service 1.1** §2.1 (TimeService Interface)
//! - **OMG Time Service 1.1** §2.2 + §2.4 — TimerEventService: excluded
//!   from the ZeroDDS scope (requires a CORBA event-channel ORB);
//!   `corba-ccm` addresses it in the CCM container PSM.
//!
//! ## Layer position
//!
//! Layer 1 — primitives. Standalone library; ZeroDDS does not
//! consume the crate internally (DDS-DCPS has its own `Time_t` with
//! an 8-byte wire format per DDS-DCPS §2.3.3, byte-distinct from the OMG
//! Time Service `UtcT` with a 16-byte wire format and a 1582 epoch).
//! Consumers are end-user applications that need OMG-Time-Service-1.1
//! conformance (e.g. the tutorial `dds-warehouse/02-time-sync`).
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! **TimeBase** ([`time_base`]):
//! - [`TimeT`] — 64-bit 100ns tick counter (spec §1.3.2.1).
//! - [`time_base::InaccuracyT`] / [`TdfT`] — 48-bit inaccuracy + 16-bit
//!   time-displacement factor.
//! - [`UtcT`] — 16-byte wire struct with `to_wire`/`from_wire` roundtrip.
//! - [`IntervalT`] — lower/upper bound with validation.
//! - [`time_base::current_time`] — wall clock in TimeT format
//!   (the no_std stub returns `0`).
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
//!   `overlaps`, `contains`, `spans`).
//! - [`OverlapType`] — overlap classification.
//!
//! **Service** ([`service`]):
//! - [`TimeService`] — `universal_time` / `secure_universal_time` /
//!   `new_universal_time` / `uto_from_utc` / `new_interval`.
//! - [`TimeUnavailable`] — exception type.
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

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod service;
pub mod time_base;
pub mod tio;
pub mod uto;

pub use service::{TimeService, TimeUnavailable};
pub use time_base::{IntervalT, TdfT, TimeT, UtcT};
pub use tio::{OverlapType, Tio};
pub use uto::{ComparisonType, TimeComparison, Uto};
