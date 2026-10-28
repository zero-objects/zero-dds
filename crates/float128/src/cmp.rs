// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Comparison: the IEEE one, and the total one a key needs.

use core::cmp::Ordering;

use crate::float::{Float128, SIGN_MASK};

impl PartialEq for Float128 {
    /// IEEE 754-2008 clause 5.11 `compareQuietEqual`.
    ///
    /// `-0.0 == 0.0`, and a NaN is equal to nothing, itself included.
    /// The same contract `f64` has, so a generated structure holding a
    /// `Float128` compares the way the same structure holding a `double`
    /// does — a binding whose equality changed with the member's width
    /// would be a binding that fails a round trip test for a reason that
    /// has nothing to do with the wire.
    fn eq(&self, other: &Self) -> bool {
        if self.is_nan() || other.is_nan() {
            return false;
        }
        if self.is_zero() && other.is_zero() {
            return true;
        }
        self.to_bits() == other.to_bits()
    }
}

impl PartialOrd for Float128 {
    /// IEEE 754-2008 clause 5.11. `None` when either side is a NaN, the
    /// unordered case.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.is_nan() || other.is_nan() {
            return None;
        }
        if self.is_zero() && other.is_zero() {
            return Some(Ordering::Equal);
        }
        Some(ordered_key(*self).cmp(&ordered_key(*other)))
    }
}

impl Float128 {
    /// IEEE 754-2008 clause 5.10 `totalOrder`.
    ///
    /// A total order over every bit pattern: `-NaN < -inf < … < -0.0 <
    /// +0.0 < … < +inf < +NaN`, with NaNs ordered by payload. This is
    /// what a `@key` member and a sorted expectation need — `partial_cmp`
    /// returning `None` for a NaN key would leave the sort order to
    /// whatever the caller's comparator does with `None`, which is how
    /// two peers disagree about instance order for a sample neither
    /// considers unusual.
    ///
    /// The same construction `f64::total_cmp` uses: read the pattern as
    /// a sign-magnitude integer and map it onto a two's-complement one.
    #[must_use]
    pub fn total_cmp(self, other: Self) -> Ordering {
        ordered_key(self).cmp(&ordered_key(other))
    }

    /// The larger of two values under [`Self::total_cmp`].
    #[must_use]
    pub fn total_max(self, other: Self) -> Self {
        if self.total_cmp(other) == Ordering::Less {
            other
        } else {
            self
        }
    }

    /// The smaller of two values under [`Self::total_cmp`].
    #[must_use]
    pub fn total_min(self, other: Self) -> Self {
        if self.total_cmp(other) == Ordering::Greater {
            other
        } else {
            self
        }
    }
}

/// The bit pattern mapped so that unsigned integer order *is* numeric
/// order.
///
/// A negative value's pattern counts the wrong way round — a larger
/// magnitude is a larger unsigned pattern but a smaller number — so the
/// non-sign bits are complemented for negatives and the sign bit is set
/// for positives.
fn ordered_key(value: Float128) -> u128 {
    let bits = value.to_bits();
    if bits & SIGN_MASK == 0 {
        bits | SIGN_MASK
    } else {
        !bits
    }
}
