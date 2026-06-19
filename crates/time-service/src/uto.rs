// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Universal Time Object (UTO) — OMG Time Service 1.1 §1.3.4.
//!
//! Spec-IDL:
//! ```idl
//! interface UTO {
//!     readonly attribute TimeBase::TimeT       time;
//!     readonly attribute TimeBase::InaccuracyT inaccuracy;
//!     readonly attribute TimeBase::TdfT        tdf;
//!     readonly attribute TimeBase::UtcT        utc_time;
//!     UTO absolute_time();
//!     TimeComparison compare_time(in ComparisonType, in UTO);
//!     TIO time_to_interval(in UTO);
//!     TIO interval();
//! };
//! ```
//!
//! We model this without a CORBA interface — the "operations" are realized
//! as Rust methods on [`Uto`].

#[cfg(feature = "std")]
use crate::time_base::current_time;
use crate::time_base::{InaccuracyT, IntervalT, TdfT, TimeT, UtcT};
use crate::tio::Tio;

/// `CosTime::ComparisonType` (Spec §1.3.2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonType {
    /// IntervalC — compares taking the inaccuracy range into account.
    IntervalC,
    /// MidC — vergleicht nur die Base-Times. Spec §1.3.2.6: "MidC
    /// comparison can never return TCIndeterminate".
    MidC,
}

/// `CosTime::TimeComparison` (Spec §1.3.2.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeComparison {
    /// `TCEqualTo`.
    EqualTo,
    /// `TCLessThan`.
    LessThan,
    /// `TCGreaterThan`.
    GreaterThan,
    /// `TCIndeterminate` — when the inaccuracy envelopes overlap.
    Indeterminate,
}

/// Universal Time Object — Spec §1.3.4. Immutable per Spec
/// ("It is intended that UTOs are immutable.")
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uto {
    inner: UtcT,
}

impl Uto {
    /// Constructs a UTO from `UtcT`. Spec §2.1.2.2 (`uto_from_utc`).
    #[must_use]
    pub const fn from_utc(utc: UtcT) -> Self {
        Self { inner: utc }
    }

    /// Constructor from the individual fields (Spec §2.1.2.1
    /// `new_universal_time`).
    #[must_use]
    pub const fn new(time: TimeT, inaccuracy: InaccuracyT, tdf: TdfT) -> Self {
        Self {
            inner: UtcT::new(time, inaccuracy, tdf),
        }
    }

    /// Spec §1.3.4.1 — `time` Attribute.
    #[must_use]
    pub const fn time(self) -> TimeT {
        self.inner.time
    }

    /// Spec §1.3.4.2 — `inaccuracy` Attribute.
    #[must_use]
    pub const fn inaccuracy(self) -> InaccuracyT {
        self.inner.inaccuracy()
    }

    /// Spec §1.3.4.3 — `tdf` Attribute.
    #[must_use]
    pub const fn tdf(self) -> TdfT {
        self.inner.tdf
    }

    /// Spec §1.3.4.4 — `utc_time` Attribute.
    #[must_use]
    pub const fn utc_time(self) -> UtcT {
        self.inner
    }

    /// Spec §1.3.4.5 — `absolute_time()`. Converts a relative UTO
    /// into an absolute one: `absolute = current + relative.time`.
    /// Raises `DATA_CONVERSION` on overflow — we return `None`.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn absolute_time(self) -> Option<Self> {
        let now = current_time();
        let absolute = now.checked_add(self.inner.time)?;
        Some(Self::from_utc(UtcT::new(
            absolute,
            self.inner.inaccuracy(),
            self.inner.tdf,
        )))
    }

    /// Spec §1.3.4.6 — `compare_time(comparison_type, uto)`.
    /// Self is the first parameter, `other` the second.
    #[must_use]
    pub fn compare_time(self, comparison_type: ComparisonType, other: Self) -> TimeComparison {
        match comparison_type {
            ComparisonType::MidC => {
                // Spec §1.3.2.6: MidC compares only base times.
                match self.inner.time.cmp(&other.inner.time) {
                    core::cmp::Ordering::Less => TimeComparison::LessThan,
                    core::cmp::Ordering::Greater => TimeComparison::GreaterThan,
                    core::cmp::Ordering::Equal => TimeComparison::EqualTo,
                }
            }
            ComparisonType::IntervalC => {
                // Spec §1.3.2.7: IntervalC with an inaccuracy envelope.
                // Equal only when Time + both inaccuracies = 0
                // (exact match) — otherwise Indeterminate on overlap.
                let self_lo = self.inner.time.saturating_sub(self.inaccuracy());
                let self_hi = self.inner.time.saturating_add(self.inaccuracy());
                let other_lo = other.inner.time.saturating_sub(other.inaccuracy());
                let other_hi = other.inner.time.saturating_add(other.inaccuracy());

                if self.inner.time == other.inner.time
                    && self.inaccuracy() == 0
                    && other.inaccuracy() == 0
                {
                    TimeComparison::EqualTo
                } else if self_hi < other_lo {
                    TimeComparison::LessThan
                } else if self_lo > other_hi {
                    TimeComparison::GreaterThan
                } else {
                    // Envelopes overlap -> Indeterminate.
                    TimeComparison::Indeterminate
                }
            }
        }
    }

    /// Spec §1.3.4.7 — `time_to_interval(uto)`. Returns a TIO with the
    /// midpoints of the two UTOs as bounds. Inaccuracies are
    /// not taken into account.
    #[must_use]
    pub fn time_to_interval(self, other: Self) -> Option<Tio> {
        let (lo, hi) = if self.inner.time <= other.inner.time {
            (self.inner.time, other.inner.time)
        } else {
            (other.inner.time, self.inner.time)
        };
        IntervalT::new(lo, hi).map(Tio::from_interval)
    }

    /// Spec §1.3.4.8 — `interval()`. Returns the inaccuracy envelope
    /// as a TIO: `[time-inaccuracy, time+inaccuracy]`.
    #[must_use]
    pub fn interval(self) -> Tio {
        let lo = self.inner.time.saturating_sub(self.inaccuracy());
        let hi = self.inner.time.saturating_add(self.inaccuracy());
        Tio::from_interval(IntervalT::new(lo, hi).unwrap_or(IntervalT {
            lower_bound: lo,
            upper_bound: lo,
        }))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn attributes_return_constructor_values() {
        // Spec §1.3.4.1-4.
        let uto = Uto::new(1_000, 50, 60);
        assert_eq!(uto.time(), 1_000);
        assert_eq!(uto.inaccuracy(), 50);
        assert_eq!(uto.tdf(), 60);
        let utc = uto.utc_time();
        assert_eq!(utc.time, 1_000);
        assert_eq!(utc.tdf, 60);
    }

    #[test]
    fn compare_time_midc_equal() {
        let a = Uto::new(100, 50, 0);
        let b = Uto::new(100, 200, 0);
        assert_eq!(
            a.compare_time(ComparisonType::MidC, b),
            TimeComparison::EqualTo
        );
    }

    #[test]
    fn compare_time_midc_less_than() {
        let a = Uto::new(100, 0, 0);
        let b = Uto::new(200, 0, 0);
        assert_eq!(
            a.compare_time(ComparisonType::MidC, b),
            TimeComparison::LessThan
        );
    }

    #[test]
    fn compare_time_midc_greater_than() {
        let a = Uto::new(300, 0, 0);
        let b = Uto::new(200, 0, 0);
        assert_eq!(
            a.compare_time(ComparisonType::MidC, b),
            TimeComparison::GreaterThan
        );
    }

    #[test]
    fn compare_time_intervalc_equal_when_inaccuracy_zero_and_time_match() {
        let a = Uto::new(100, 0, 0);
        let b = Uto::new(100, 0, 0);
        assert_eq!(
            a.compare_time(ComparisonType::IntervalC, b),
            TimeComparison::EqualTo
        );
    }

    #[test]
    fn compare_time_intervalc_indeterminate_on_envelope_overlap() {
        // Spec §1.3.2.7: TCIndeterminate when envelopes overlap.
        let a = Uto::new(100, 50, 0); // [50, 150]
        let b = Uto::new(120, 50, 0); // [70, 170]
        assert_eq!(
            a.compare_time(ComparisonType::IntervalC, b),
            TimeComparison::Indeterminate
        );
    }

    #[test]
    fn compare_time_intervalc_less_than_when_envelopes_disjoint() {
        let a = Uto::new(100, 10, 0); // [90, 110]
        let b = Uto::new(200, 10, 0); // [190, 210]
        assert_eq!(
            a.compare_time(ComparisonType::IntervalC, b),
            TimeComparison::LessThan
        );
    }

    #[test]
    fn compare_time_intervalc_greater_than_when_envelopes_disjoint() {
        let a = Uto::new(300, 10, 0);
        let b = Uto::new(200, 10, 0);
        assert_eq!(
            a.compare_time(ComparisonType::IntervalC, b),
            TimeComparison::GreaterThan
        );
    }

    #[test]
    fn time_to_interval_uses_midpoints() {
        // Spec §1.3.4.7: interval between the midpoints, inaccuracies
        // are not taken into account.
        let a = Uto::new(100, 9999, 0);
        let b = Uto::new(200, 9999, 0);
        let tio = a.time_to_interval(b).expect("ok");
        assert_eq!(tio.time_interval().lower_bound, 100);
        assert_eq!(tio.time_interval().upper_bound, 200);
    }

    #[test]
    fn interval_returns_inaccuracy_envelope() {
        // Spec §1.3.4.8: TIO.upper = time + inaccuracy, TIO.lower = time - inaccuracy.
        let uto = Uto::new(1_000, 50, 0);
        let tio = uto.interval();
        assert_eq!(tio.time_interval().lower_bound, 950);
        assert_eq!(tio.time_interval().upper_bound, 1_050);
    }

    #[cfg(feature = "std")]
    #[test]
    fn absolute_time_adds_current_to_relative() {
        let relative = Uto::new(1_000, 0, 0);
        let absolute = relative.absolute_time().expect("ok");
        // Absolute > current_time (should at least be > base time).
        assert!(absolute.time() > 1_000);
    }
}
