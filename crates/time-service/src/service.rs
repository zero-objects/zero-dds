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
//! We model this without a CORBA interface — the operations are realized
//! as methods on the [`TimeService`] struct.

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

/// `TimeService` object — Spec §2.1. ZeroDDS implementation as a
/// plain Rust struct (no CORBA object).
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeService {
    /// Tdf set in the UTOs at `universal_time()`.
    /// Default 0 (Greenwich).
    pub default_tdf: TdfT,
    /// Inaccuracy reported in the UTOs at `universal_time()`.
    /// Default 0 (the spec allows implementations to know their own
    /// inaccuracy).
    pub default_inaccuracy: InaccuracyT,
    /// If `true`, the underlying time source is marked
    /// "secure" (spec §2.1.2 + appendix A). Otherwise
    /// `secure_universal_time()` throws `TimeUnavailable`.
    pub secure_source: bool,
}

impl TimeService {
    /// Spec §2.1.1 — `universal_time()`. Returns the current time.
    /// Raises `TimeUnavailable` if the time source is not
    /// available.
    ///
    /// # Errors
    /// `TimeUnavailable` if `current_time()` returns 0
    /// (e.g. no_std without a real clock).
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

    /// Spec §2.1.2 — `secure_universal_time()`. Returns the time only if
    /// the time source is configured as "secure" (spec appendix A).
    ///
    /// # Errors
    /// `TimeUnavailable` if `secure_source = false` or the
    /// time source is not available.
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
    /// Spec says `CORBA::BAD_PARAM` on out-of-range inaccuracy. We
    /// silently clamp to 48 bit instead (see [`UtcT::new`]).
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
    /// `CORBA::BAD_PARAM` if `lower > upper`. We return `None`.
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
        // Spec §2.1.2.3 — BAD_PARAM when lower > upper.
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
        // Spec §2.1.1 — time must be > 0 and in the plausible range
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
