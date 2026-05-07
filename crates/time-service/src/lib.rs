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
//! - **OMG Time Service 1.1** §2.2 + §2.4 — TimerEventService: aus
//!   ZeroDDS-Scope ausgenommen (verlangt CORBA-Event-Channel-ORB);
//!   `corba-ccm` adressiert das im CCM-Container-PSM.
//!
//! ## Schichten-Position
//!
//! Layer 1 — Primitives. Standalone-Library; ZeroDDS verbraucht
//! die Crate intern nicht (DDS-DCPS hat sein eigenes `Time_t` mit
//! 8-Byte-Wire-Format gemaess DDS-DCPS §2.3.3, byte-distinkt zu OMG-
//! Time-Service `UtcT` mit 16-Byte-Wire-Format und 1582-Epoch).
//! Konsumenten sind End-User-Applikationen die OMG-Time-Service-1.1-
//! Konformitaet brauchen (z.B. Tutorial `dds-warehouse/02-time-sync`).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! **TimeBase** ([`time_base`]):
//! - [`TimeT`] — 64-bit 100ns-Tick-Counter (Spec §1.3.2.1).
//! - [`time_base::InaccuracyT`] / [`TdfT`] — 48-bit Inaccuracy + 16-bit
//!   Time-Displacement-Factor.
//! - [`UtcT`] — 16-Byte-Wire-Struct mit `to_wire`/`from_wire` Roundtrip.
//! - [`IntervalT`] — Lower/Upper-bound mit Validation.
//! - [`time_base::current_time`] — Wall-Clock in TimeT-Format
//!   (no_std-Stub liefert `0`).
//! - [`time_base::UTC_EPOCH_TO_UNIX_TICKS`] / [`time_base::TICKS_PER_SECOND`].
//!
//! **UTO** ([`uto`]):
//! - [`Uto`] — Universal Time Object mit Operations (`absolute_time`,
//!   `compare_time`, `time_to_interval`, `interval`).
//! - [`ComparisonType::{IntervalC, MidC}`] — Vergleichs-Modus.
//! - [`TimeComparison::{EqualTo, LessThan, GreaterThan, Indeterminate}`].
//!
//! **TIO** ([`tio`]):
//! - [`Tio`] — Time Interval Object mit Operations (`time_interval`,
//!   `overlaps`, `contains`, `spans`).
//! - [`OverlapType`] — Overlap-Klassifizierung.
//!
//! **Service** ([`service`]):
//! - [`TimeService`] — `universal_time` / `secure_universal_time` /
//!   `new_universal_time` / `uto_from_utc` / `new_interval`.
//! - [`TimeUnavailable`] — Exception-Type.
//!
//! ## Beispiel
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
