// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Time Interval Object (TIO) — OMG Time Service 1.1 §1.3.5.
//!
//! Spec-IDL:
//! ```idl
//! interface TIO {
//!     readonly attribute TimeBase::IntervalT time_interval;
//!     OverlapType spans (in UTO time, out TIO overlap);
//!     OverlapType overlaps (in TIO interval, out TIO overlap);
//!     UTO time();
//! };
//! ```

use crate::time_base::{IntervalT, TimeT, UtcT};
use crate::uto::Uto;

/// `CosTime::OverlapType` (Spec §1.3.2.8) — Vier Faelle der Overlap-
/// Beziehung zwischen zwei Intervallen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlapType {
    /// `OTContainer` — Self enthaelt Other.
    Container,
    /// `OTContained` — Self ist in Other enthalten.
    Contained,
    /// `OTOverlap` — partielle Ueberlappung (kein Containment).
    Overlap,
    /// `OTNoOverlap` — Intervalle disjunkt.
    NoOverlap,
}

/// Time Interval Object — Spec §1.3.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tio {
    inner: IntervalT,
}

impl Tio {
    /// Konstruiert TIO aus IntervalT. Spec §2.1.x (`new_interval`).
    #[must_use]
    pub const fn from_interval(interval: IntervalT) -> Self {
        Self { inner: interval }
    }

    /// Spec §1.3.5.1 — `time_interval` Attribute.
    #[must_use]
    pub const fn time_interval(self) -> IntervalT {
        self.inner
    }

    /// Spec §1.3.5.2 — `spans(uto)`: Beziehung zwischen Self-Intervall
    /// und der durch UTO definierten Inaccuracy-Range.
    #[must_use]
    pub fn spans(self, uto: Uto) -> (OverlapType, Tio) {
        // Self ist Interval A; UTO definiert Interval B als
        // [time - inaccuracy, time + inaccuracy].
        let other_lo = uto.time().saturating_sub(uto.inaccuracy());
        let other_hi = uto.time().saturating_add(uto.inaccuracy());
        let a = self.inner;
        let b = IntervalT {
            lower_bound: other_lo,
            upper_bound: other_hi,
        };
        compute_overlap(a, b)
    }

    /// Spec §1.3.5.3 — `overlaps(tio)`: Beziehung zwischen Self und
    /// Other.
    #[must_use]
    pub fn overlaps(self, other: Tio) -> (OverlapType, Tio) {
        compute_overlap(self.inner, other.inner)
    }

    /// Spec §1.3.5.4 — `time()`: Liefert UTO mit time = Midpoint und
    /// Inaccuracy = halbe Intervall-Breite.
    #[must_use]
    pub fn time(self) -> Uto {
        let lo = self.inner.lower_bound;
        let hi = self.inner.upper_bound;
        let midpoint = lo / 2 + hi / 2 + (lo % 2 + hi % 2) / 2;
        let half_width = (hi - lo) / 2;
        Uto::from_utc(UtcT::new(midpoint, half_width, 0))
    }
}

/// Spec §1.3.2.8 — Overlap-Berechnung zwischen zwei Intervals.
fn compute_overlap(a: IntervalT, b: IntervalT) -> (OverlapType, Tio) {
    // OTNoOverlap: disjunkt.
    if a.upper_bound < b.lower_bound {
        // Gap zwischen den beiden.
        let gap = IntervalT {
            lower_bound: a.upper_bound,
            upper_bound: b.lower_bound,
        };
        return (OverlapType::NoOverlap, Tio::from_interval(gap));
    }
    if b.upper_bound < a.lower_bound {
        let gap = IntervalT {
            lower_bound: b.upper_bound,
            upper_bound: a.lower_bound,
        };
        return (OverlapType::NoOverlap, Tio::from_interval(gap));
    }

    // OTContainer: A enthaelt B vollstaendig.
    if a.lower_bound <= b.lower_bound && a.upper_bound >= b.upper_bound {
        // Spec §1.3.2.8: "the overlap interval is the same as the interval B."
        return (OverlapType::Container, Tio::from_interval(b));
    }
    // OTContained: A ist in B.
    if b.lower_bound <= a.lower_bound && b.upper_bound >= a.upper_bound {
        return (OverlapType::Contained, Tio::from_interval(a));
    }
    // OTOverlap: partielle Ueberlappung.
    let overlap_lo = max_t(a.lower_bound, b.lower_bound);
    let overlap_hi = min_t(a.upper_bound, b.upper_bound);
    let overlap_interval = IntervalT {
        lower_bound: overlap_lo,
        upper_bound: overlap_hi,
    };
    (OverlapType::Overlap, Tio::from_interval(overlap_interval))
}

const fn max_t(a: TimeT, b: TimeT) -> TimeT {
    if a > b { a } else { b }
}

const fn min_t(a: TimeT, b: TimeT) -> TimeT {
    if a < b { a } else { b }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn time_interval_attribute_returns_constructor_value() {
        let i = IntervalT::new(100, 200).expect("ok");
        let tio = Tio::from_interval(i);
        assert_eq!(tio.time_interval(), i);
    }

    #[test]
    fn time_method_returns_uto_with_midpoint_and_half_width() {
        // Spec §1.3.5.4.
        let i = IntervalT::new(100, 200).expect("ok");
        let tio = Tio::from_interval(i);
        let uto = tio.time();
        assert_eq!(uto.time(), 150);
        assert_eq!(uto.inaccuracy(), 50);
    }

    #[test]
    fn overlaps_otcontainer() {
        // A = [100, 300] enthaelt B = [150, 250]. Spec §1.3.2.8.
        let a = Tio::from_interval(IntervalT::new(100, 300).expect("ok"));
        let b = Tio::from_interval(IntervalT::new(150, 250).expect("ok"));
        let (kind, overlap) = a.overlaps(b);
        assert_eq!(kind, OverlapType::Container);
        // Spec §1.3.2.8: overlap-Interval ist B.
        assert_eq!(overlap.time_interval().lower_bound, 150);
        assert_eq!(overlap.time_interval().upper_bound, 250);
    }

    #[test]
    fn overlaps_otcontained() {
        // A = [150, 250] ist in B = [100, 300] enthalten.
        let a = Tio::from_interval(IntervalT::new(150, 250).expect("ok"));
        let b = Tio::from_interval(IntervalT::new(100, 300).expect("ok"));
        let (kind, overlap) = a.overlaps(b);
        assert_eq!(kind, OverlapType::Contained);
        // overlap = A.
        assert_eq!(overlap.time_interval().lower_bound, 150);
        assert_eq!(overlap.time_interval().upper_bound, 250);
    }

    #[test]
    fn overlaps_partial() {
        // A = [100, 200], B = [150, 250]. Overlap = [150, 200].
        let a = Tio::from_interval(IntervalT::new(100, 200).expect("ok"));
        let b = Tio::from_interval(IntervalT::new(150, 250).expect("ok"));
        let (kind, overlap) = a.overlaps(b);
        assert_eq!(kind, OverlapType::Overlap);
        assert_eq!(overlap.time_interval().lower_bound, 150);
        assert_eq!(overlap.time_interval().upper_bound, 200);
    }

    #[test]
    fn overlaps_no_overlap() {
        // A = [100, 200], B = [300, 400]. Gap = [200, 300].
        let a = Tio::from_interval(IntervalT::new(100, 200).expect("ok"));
        let b = Tio::from_interval(IntervalT::new(300, 400).expect("ok"));
        let (kind, gap) = a.overlaps(b);
        assert_eq!(kind, OverlapType::NoOverlap);
        // Spec §1.3.5.2/3: gap zwischen den Intervallen.
        assert_eq!(gap.time_interval().lower_bound, 200);
        assert_eq!(gap.time_interval().upper_bound, 300);
    }

    #[test]
    fn spans_uses_uto_inaccuracy_envelope() {
        // Self = [50, 250], UTO = time=150 ± 50 -> [100, 200].
        // Self enthaelt UTO-Range -> OTContainer.
        let a = Tio::from_interval(IntervalT::new(50, 250).expect("ok"));
        let uto = Uto::new(150, 50, 0);
        let (kind, overlap) = a.spans(uto);
        assert_eq!(kind, OverlapType::Container);
        // overlap = UTO-Envelope.
        assert_eq!(overlap.time_interval().lower_bound, 100);
        assert_eq!(overlap.time_interval().upper_bound, 200);
    }
}
