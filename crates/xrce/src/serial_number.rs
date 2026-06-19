// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RFC-1982 serial number arithmetic for XRCE sequence numbers
//! (Spec §8.3.2.3).
//!
//! XRCE uses 16-bit serial numbers with `SERIAL_BITS = 16`. Comparisons
//! are done modulo `2^16` with half-window logic per RFC 1982:
//!
//! - `a < b` holds when the signed difference `b - a` modulo `2^16` is in
//!   the range `(0, 2^15)`.
//! - `a > b` analogously in the range `(-2^15, 0)`.
//! - At exactly `2^15` distance the order is undefined (RFC 1982
//!   §3.2). We mark that here as `is_undefined_pair`.
//!
//! At most 32768 outstanding messages per stream (Spec §8.3.2.3).

use core::cmp::Ordering;

/// 16-bit serial number (wrapping at `2^16`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SerialNumber16(pub u16);

impl SerialNumber16 {
    /// Half wrap distance; at this distance the order is
    /// undefined per RFC 1982 §3.2.
    pub const HALF_WINDOW: u16 = 1u16 << 15; // 0x8000

    /// Constructs from a raw `u16`.
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Raw value.
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Increment (wraps at `u16::MAX`).
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// Wrapping subtraction: returns `(rhs - self) mod 2^16`.
    /// Note: this difference is always `>= 0`. For a "signed
    /// distance" see `wrapping_diff`.
    #[must_use]
    pub fn wrapping_sub(self, rhs: Self) -> u16 {
        self.0.wrapping_sub(rhs.0)
    }

    /// Signed distance `self - rhs` interpreted as a half-window
    /// value. Result range `[-2^15, +2^15)`. At exactly `+2^15` (i.e.
    /// `0x8000`) the order is undefined; we then return
    /// `i32::from(HALF_WINDOW)`, which lies at the edge — the caller must
    /// distinguish via `is_undefined_pair`.
    #[must_use]
    pub fn wrapping_diff(self, rhs: Self) -> i32 {
        let raw_diff = self.0.wrapping_sub(rhs.0); // u16, in [0, 2^16)
        match raw_diff.cmp(&Self::HALF_WINDOW) {
            // Undefined per RFC 1982 §3.2; conservatively reported as
            // HALF_WINDOW (the caller can check `is_undefined_pair`).
            core::cmp::Ordering::Less | core::cmp::Ordering::Equal => i32::from(raw_diff),
            // raw_diff in (HALF_WINDOW, 2^16) → negative.
            core::cmp::Ordering::Greater => i32::from(raw_diff) - (1i32 << 16),
        }
    }

    /// `true` when the difference between `self` and `rhs` is exactly
    /// `2^15` — order then undefined (RFC 1982 §3.2).
    #[must_use]
    pub fn is_undefined_pair(self, rhs: Self) -> bool {
        self.0.wrapping_sub(rhs.0) == Self::HALF_WINDOW
    }

    /// `self < rhs` per RFC-1982. If the order is undefined,
    /// the method returns `false`.
    #[must_use]
    pub fn wrapping_lt(self, rhs: Self) -> bool {
        let d = rhs.0.wrapping_sub(self.0);
        d > 0 && d < Self::HALF_WINDOW
    }

    /// `self > rhs` per RFC-1982. Undefined order -> `false`.
    #[must_use]
    pub fn wrapping_gt(self, rhs: Self) -> bool {
        rhs.wrapping_lt(self)
    }

    /// Wrapping comparison as `Option<Ordering>`. `None` for an
    /// undefined pair.
    #[must_use]
    pub fn wrapping_cmp(self, rhs: Self) -> Option<Ordering> {
        if self.0 == rhs.0 {
            Some(Ordering::Equal)
        } else if self.is_undefined_pair(rhs) {
            None
        } else if self.wrapping_lt(rhs) {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Greater)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn next_increments_by_one() {
        let s = SerialNumber16::new(5);
        assert_eq!(s.next().raw(), 6);
    }

    #[test]
    fn next_wraps_at_u16_max() {
        let s = SerialNumber16::new(u16::MAX);
        assert_eq!(s.next().raw(), 0);
    }

    #[test]
    fn lt_simple_case() {
        let a = SerialNumber16::new(10);
        let b = SerialNumber16::new(20);
        assert!(a.wrapping_lt(b));
        assert!(!b.wrapping_lt(a));
    }

    #[test]
    fn lt_across_wrap_boundary() {
        // u16::MAX - 5  <  3  because distance <= HALF_WINDOW
        let a = SerialNumber16::new(u16::MAX - 5);
        let b = SerialNumber16::new(3);
        assert!(a.wrapping_lt(b), "wrap-around: u16::MAX-5 < 3");
        assert!(!b.wrapping_lt(a));
    }

    #[test]
    fn gt_is_inverse_of_lt() {
        let a = SerialNumber16::new(100);
        let b = SerialNumber16::new(200);
        assert!(b.wrapping_gt(a));
        assert!(!a.wrapping_gt(b));
    }

    #[test]
    fn equal_serial_numbers_neither_lt_nor_gt() {
        let a = SerialNumber16::new(42);
        let b = SerialNumber16::new(42);
        assert!(!a.wrapping_lt(b));
        assert!(!a.wrapping_gt(b));
        assert_eq!(a.wrapping_cmp(b), Some(Ordering::Equal));
    }

    #[test]
    fn undefined_pair_at_exactly_half_window() {
        let a = SerialNumber16::new(0);
        let b = SerialNumber16::new(SerialNumber16::HALF_WINDOW);
        assert!(a.is_undefined_pair(b));
        assert!(b.is_undefined_pair(a));
        assert_eq!(a.wrapping_cmp(b), None);
    }

    #[test]
    fn diff_signed_within_window() {
        let a = SerialNumber16::new(10);
        let b = SerialNumber16::new(3);
        assert_eq!(a.wrapping_diff(b), 7);
        assert_eq!(b.wrapping_diff(a), -7);
    }

    #[test]
    fn diff_across_wrap_yields_signed_value() {
        // a = u16::MAX, b = 0  →  a - b mod 2^16 = u16::MAX = 65535
        // 65535 > HALF_WINDOW → signed_value = 65535 - 65536 = -1
        let a = SerialNumber16::new(u16::MAX);
        let b = SerialNumber16::new(0);
        assert_eq!(a.wrapping_diff(b), -1);
        assert_eq!(b.wrapping_diff(a), 1);
    }

    #[test]
    fn wrap_around_does_not_break_lt_for_consecutive_numbers() {
        // Konstruktion: a = u16::MAX, b = a.next() = 0
        let a = SerialNumber16::new(u16::MAX);
        let b = a.next();
        assert!(a.wrapping_lt(b));
        assert!(b.wrapping_gt(a));
    }
}
