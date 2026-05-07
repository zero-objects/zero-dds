// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TimeService Interface — OMG Time Service 1.1 §2.1.
//!
//! Spec-IDL:
//! ```idl
//! interface TimeService {
//!     UTO universal_time() raises(TimeUnavailable);
//!     UTO secure_universal_time() raises(TimeUnavailable);
//!     UTO new_universal_time(in TimeT, in InaccuracyT, in TdfT);
//!     UTO uto_from_utc(in UtcT);
//!     TIO new_interval(in TimeT lower, in TimeT upper);
//! };
//! ```
//!
//! Wir bilden das ohne CORBA-Interface ab — die Operations werden als
//! Methoden auf der [`TimeService`]-Struct realisiert.

use core::fmt;

#[cfg(feature = "std")]
use crate::time_base::current_time;
use crate::time_base::{InaccuracyT, IntervalT, TdfT, TimeT, UtcT};
use crate::tio::Tio;
use crate::uto::Uto;

/// Spec §1.3.3.1 — `TimeUnavailable`-Exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeUnavailable;

impl fmt::Display for TimeUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "underlying time service unavailable")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TimeUnavailable {}

/// `TimeService`-Object — Spec §2.1. ZeroDDS-Implementation als
/// plain Rust-Struct (kein CORBA-Object).
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeService {
    /// Tdf, der bei `universal_time()` in den UTOs gesetzt wird.
    /// Default 0 (Greenwich).
    pub default_tdf: TdfT,
    /// Inaccuracy, die bei `universal_time()` in den UTOs angegeben
    /// wird. Default 0 (Spec erlaubt Implementations, ihre eigene
    /// Inaccuracy zu kennen).
    pub default_inaccuracy: InaccuracyT,
    /// Wenn `true`, dann ist die zugrundeliegende Zeitquelle als
    /// "secure" markiert (Spec §2.1.2 + Appendix A). Sonst wirft
    /// `secure_universal_time()` `TimeUnavailable`.
    pub secure_source: bool,
}

impl TimeService {
    /// Spec §2.1.1 — `universal_time()`. Liefert die aktuelle Zeit.
    /// Raises `TimeUnavailable`, wenn die Time-Source nicht
    /// verfuegbar ist.
    ///
    /// # Errors
    /// `TimeUnavailable` wenn `current_time()` 0 zurueckliefert
    /// (z.B. no_std ohne Real-Clock).
    #[cfg(feature = "std")]
    pub fn universal_time(&self) -> Result<Uto, TimeUnavailable> {
        let now = current_time();
        if now == 0 {
            return Err(TimeUnavailable);
        }
        Ok(Uto::from_utc(UtcT::new(
            now,
            self.default_inaccuracy,
            self.default_tdf,
        )))
    }

    /// Spec §2.1.2 — `secure_universal_time()`. Liefert Zeit nur, wenn
    /// die Time-Source als "secure" konfiguriert ist (Spec Appendix A).
    ///
    /// # Errors
    /// `TimeUnavailable` wenn `secure_source = false` oder die
    /// Time-Source nicht verfuegbar ist.
    #[cfg(feature = "std")]
    pub fn secure_universal_time(&self) -> Result<Uto, TimeUnavailable> {
        if !self.secure_source {
            return Err(TimeUnavailable);
        }
        self.universal_time()
    }

    /// Spec §2.1.2.1 — `new_universal_time(time, inaccuracy, tdf)`.
    ///
    /// # Errors
    /// Spec sagt `CORBA::BAD_PARAM` bei out-of-range Inaccuracy. Wir
    /// kappen statt dessen still auf 48 bit (siehe [`UtcT::new`]).
    #[must_use]
    pub fn new_universal_time(time: TimeT, inaccuracy: InaccuracyT, tdf: TdfT) -> Uto {
        Uto::new(time, inaccuracy, tdf)
    }

    /// Spec §2.1.2.2 — `uto_from_utc(utc)`.
    #[must_use]
    pub fn uto_from_utc(utc: UtcT) -> Uto {
        Uto::from_utc(utc)
    }

    /// Spec §2.1.2.3 — `new_interval(lower, upper)`. Raises
    /// `CORBA::BAD_PARAM` wenn `lower > upper`. Wir liefern `None`.
    #[must_use]
    pub fn new_interval(lower: TimeT, upper: TimeT) -> Option<Tio> {
        IntervalT::new(lower, upper).map(Tio::from_interval)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn new_universal_time_creates_uto_from_components() {
        // Spec §2.1.2.1.
        let uto = TimeService::new_universal_time(1_000, 50, 60);
        assert_eq!(uto.time(), 1_000);
        assert_eq!(uto.inaccuracy(), 50);
        assert_eq!(uto.tdf(), 60);
    }

    #[test]
    fn uto_from_utc_wraps_passed_struct() {
        // Spec §2.1.2.2.
        let utc = UtcT::new(100, 0, 0);
        let uto = TimeService::uto_from_utc(utc);
        assert_eq!(uto.utc_time(), utc);
    }

    #[test]
    fn new_interval_rejects_lower_greater_than_upper() {
        // Spec §2.1.2.3 — BAD_PARAM bei lower > upper.
        assert!(TimeService::new_interval(200, 100).is_none());
    }

    #[test]
    fn new_interval_creates_tio_for_valid_bounds() {
        let tio = TimeService::new_interval(100, 200).expect("ok");
        assert_eq!(tio.time_interval().lower_bound, 100);
        assert_eq!(tio.time_interval().upper_bound, 200);
    }

    #[cfg(feature = "std")]
    #[test]
    fn universal_time_returns_recent_value() {
        let service = TimeService::default();
        let uto = service.universal_time().expect("ok");
        // Spec §2.1.1 — Time muss > 0 sein und im plausiblen Bereich
        // (post-2020, pre-2200).
        assert!(uto.time() > 130_000_000_000_000_000);
    }

    #[cfg(feature = "std")]
    #[test]
    fn secure_universal_time_fails_when_source_not_marked_secure() {
        // Spec §2.1.2 — secure source = false -> TimeUnavailable.
        let service = TimeService::default();
        assert_eq!(service.secure_universal_time(), Err(TimeUnavailable));
    }

    #[cfg(feature = "std")]
    #[test]
    fn secure_universal_time_returns_when_source_marked_secure() {
        let service = TimeService {
            secure_source: true,
            ..TimeService::default()
        };
        assert!(service.secure_universal_time().is_ok());
    }

    #[test]
    fn time_unavailable_display_describes_failure_mode() {
        let s = alloc::format!("{TimeUnavailable}");
        assert!(s.contains("time service unavailable"));
    }
}
