// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RFC-1982 Serial Number Arithmetic fuer XRCE-Sequence-Numbers
//! (Spec §8.3.2.3).
//!
//! XRCE nutzt 16-bit Serial Numbers mit `SERIAL_BITS = 16`. Vergleiche
//! erfolgen modulo `2^16` mit Half-Window-Logik nach RFC 1982:
//!
//! - `a < b` gilt, wenn die signed-Differenz `b - a` modulo `2^16` im
//!   Bereich `(0, 2^15)` liegt.
//! - `a > b` analog im Bereich `(-2^15, 0)`.
//! - Bei exakt `2^15` Abstand ist die Ordnung undefiniert (RFC 1982
//!   §3.2). Wir markieren das hier als `is_undefined_pair`.
//!
//! Maximal 32768 outstanding Messages pro Stream (Spec §8.3.2.3).

use core::cmp::Ordering;

/// 16-bit Serial Number (Wrapping bei `2^16`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SerialNumber16(pub u16);

impl SerialNumber16 {
    /// Halbe Wrap-Distanz; bei diesem Abstand ist Ordnung
    /// laut RFC 1982 §3.2 undefiniert.
    pub const HALF_WINDOW: u16 = 1u16 << 15; // 0x8000

    /// Konstruiere aus rohem `u16`.
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Roher Wert.
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Inkrement (wrappt bei `u16::MAX`).
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// Wrapping-Subtraktion: liefert `(rhs - self) mod 2^16`.
    /// Achtung: diese Differenz ist immer `>= 0`. Fuer "Distanz mit
    /// Vorzeichen" siehe `wrapping_diff`.
    #[must_use]
    pub fn wrapping_sub(self, rhs: Self) -> u16 {
        self.0.wrapping_sub(rhs.0)
    }

    /// Signierte Distanz `self - rhs` interpretiert als Half-Window-
    /// Wert. Resultat-Bereich `[-2^15, +2^15)`. Bei exakt `+2^15` (also
    /// `0x8000`) ist die Ordnung undefiniert; wir liefern dann
    /// `i32::from(HALF_WINDOW)`, was am Rand liegt — Caller muss via
    /// `is_undefined_pair` differenzieren.
    #[must_use]
    pub fn wrapping_diff(self, rhs: Self) -> i32 {
        let raw_diff = self.0.wrapping_sub(rhs.0); // u16, in [0, 2^16)
        match raw_diff.cmp(&Self::HALF_WINDOW) {
            // Undefiniert nach RFC 1982 §3.2; konservativ als HALF_WINDOW
            // melden (Caller kann `is_undefined_pair` pruefen).
            core::cmp::Ordering::Less | core::cmp::Ordering::Equal => i32::from(raw_diff),
            // raw_diff in (HALF_WINDOW, 2^16) → negativ.
            core::cmp::Ordering::Greater => i32::from(raw_diff) - (1i32 << 16),
        }
    }

    /// `true`, wenn die Differenz zwischen `self` und `rhs` exakt
    /// `2^15` betraegt — Ordnung dann undefiniert (RFC 1982 §3.2).
    #[must_use]
    pub fn is_undefined_pair(self, rhs: Self) -> bool {
        self.0.wrapping_sub(rhs.0) == Self::HALF_WINDOW
    }

    /// `self < rhs` nach RFC-1982. Ist die Ordnung undefiniert,
    /// liefert die Methode `false`.
    #[must_use]
    pub fn wrapping_lt(self, rhs: Self) -> bool {
        let d = rhs.0.wrapping_sub(self.0);
        d > 0 && d < Self::HALF_WINDOW
    }

    /// `self > rhs` nach RFC-1982. Undefinierte Ordnung -> `false`.
    #[must_use]
    pub fn wrapping_gt(self, rhs: Self) -> bool {
        rhs.wrapping_lt(self)
    }

    /// Wrapping-Comparison als `Option<Ordering>`. `None` bei
    /// undefiniertem Paar.
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
        // u16::MAX - 5  <  3  weil Distanz <= HALF_WINDOW
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
