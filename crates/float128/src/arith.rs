// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Addition, subtraction, multiplication and division, correctly rounded.
//!
//! IEEE 754-2008 clause 5.4.1 defines the four as computing the exact
//! result and rounding it once; the default rounding direction is to
//! nearest with ties to even (clause 4.3.3). Each operation here builds
//! the exact result as an integral significand and a binary exponent —
//! wide enough that nothing that decides the rounding is lost, with
//! whatever lies below folded into one sticky bit — and hands it to
//! [`Float128::from_parts_rounded`], the one rounding step the
//! conversions use too. Overflow gives an infinity, as clause 7.4
//! requires of the default rounding direction.
//!
//! The special values follow clause 6: a NaN operand gives a quiet NaN,
//! `∞ − ∞`, `0 × ∞`, `0 / 0` and `∞ / ∞` give a quiet NaN (invalid
//! operation, clause 7.2), `x / 0` gives a signed infinity (clause 7.3),
//! and an exact zero sum is `+0` unless both operands are `−0` (clause
//! 6.3).
//!
//! The results are measured against GCC's `__float128` (libgcc's soft-fp)
//! in `tests/gcc_binary128_ops.rs`.

use crate::float::{FRACTION_BITS, Float128};

/// Headroom an addend is shifted left by before the two are aligned: with
/// it the smaller operand's bits that decide the rounding survive the
/// alignment, and a cancellation that normalises by one bit still leaves
/// guard bits below the 113 that are kept.
const ADD_HEADROOM: u32 = 14;

/// Quotient bits produced beyond the 113 of the significand: guard,
/// round, and the bit the remainder is jammed into.
const DIV_EXTRA: u32 = 3;

impl core::ops::Add for Float128 {
    type Output = Self;
    /// `self + other`, correctly rounded.
    fn add(self, other: Self) -> Self {
        add(self, other)
    }
}

impl core::ops::Sub for Float128 {
    type Output = Self;
    /// `self - other`, correctly rounded.
    fn sub(self, other: Self) -> Self {
        add(self, other.neg())
    }
}

impl core::ops::Mul for Float128 {
    type Output = Self;
    /// `self * other`, correctly rounded.
    fn mul(self, other: Self) -> Self {
        mul(self, other)
    }
}

impl core::ops::Div for Float128 {
    type Output = Self;
    /// `self / other`, correctly rounded.
    fn div(self, other: Self) -> Self {
        div(self, other)
    }
}

/// The sign of a product or quotient.
const fn signed(negative: bool, magnitude: Float128) -> Float128 {
    if negative { magnitude.neg() } else { magnitude }
}

fn add(a: Float128, b: Float128) -> Float128 {
    if a.is_nan() || b.is_nan() {
        return Float128::NAN;
    }
    match (a.is_infinite(), b.is_infinite()) {
        (true, true) => {
            return if a.is_sign_negative() == b.is_sign_negative() {
                a
            } else {
                Float128::NAN
            };
        }
        (true, false) => return a,
        (false, true) => return b,
        (false, false) => {}
    }
    let (Some((na, sa, ea)), Some((nb, sb, eb))) = (a.decompose(), b.decompose()) else {
        return Float128::NAN;
    };
    if sa == 0 && sb == 0 {
        // Clause 6.3: x + x = x for a zero x, and +0 for opposite signs.
        return if na && nb {
            Float128::NEG_ZERO
        } else {
            Float128::ZERO
        };
    }
    if sa == 0 {
        return b;
    }
    if sb == 0 {
        return a;
    }
    // The operand with the larger exponent first.
    let ((n_big, s_big, e_big), (n_small, s_small, e_small)) = if ea >= eb {
        ((na, sa, ea), (nb, sb, eb))
    } else {
        ((nb, sb, eb), (na, sa, ea))
    };
    let big = s_big << ADD_HEADROOM;
    let distance = (e_big - e_small).unsigned_abs();
    let small = jam_shift_right(s_small << ADD_HEADROOM, distance);
    let exponent = e_big - ADD_HEADROOM as i32;
    if n_big == n_small {
        // Both significands are below 2^113 and shifted by 14: the sum
        // stays below 2^128.
        let (value, _) = Float128::from_parts_rounded(n_big, big + small, exponent);
        return value;
    }
    let (negative, magnitude) = if big >= small {
        (n_big, big - small)
    } else {
        (n_small, small - big)
    };
    if magnitude == 0 {
        return Float128::ZERO;
    }
    let (value, _) = Float128::from_parts_rounded(negative, magnitude, exponent);
    value
}

fn mul(a: Float128, b: Float128) -> Float128 {
    if a.is_nan() || b.is_nan() {
        return Float128::NAN;
    }
    let negative = a.is_sign_negative() != b.is_sign_negative();
    if a.is_infinite() || b.is_infinite() {
        // ∞ × 0 is invalid; ∞ × anything else is an infinity.
        if a.is_zero() || b.is_zero() {
            return Float128::NAN;
        }
        return signed(negative, Float128::INFINITY);
    }
    let (Some((_, sa, ea)), Some((_, sb, eb))) = (a.decompose(), b.decompose()) else {
        return Float128::NAN;
    };
    if sa == 0 || sb == 0 {
        return signed(negative, Float128::ZERO);
    }
    let (high, low) = wide_mul(sa, sb);
    let (significand, exponent) = if high == 0 {
        (low, ea + eb)
    } else {
        // Keep 127 bits of the 256-bit product, the rest in one sticky
        // bit: far more than the 113 + 2 the rounding needs.
        let width = 256 - high.leading_zeros();
        let shift = width - 127;
        let kept = (high << (128 - shift)) | (low >> shift);
        let lost = low & ((1u128 << shift) - 1) != 0;
        (kept | u128::from(lost), ea + eb + shift as i32)
    };
    let (value, _) = Float128::from_parts_rounded(negative, significand, exponent);
    value
}

fn div(a: Float128, b: Float128) -> Float128 {
    if a.is_nan() || b.is_nan() {
        return Float128::NAN;
    }
    let negative = a.is_sign_negative() != b.is_sign_negative();
    match (a.is_infinite(), b.is_infinite()) {
        (true, true) => return Float128::NAN,
        (true, false) => return signed(negative, Float128::INFINITY),
        (false, true) => return signed(negative, Float128::ZERO),
        (false, false) => {}
    }
    let (Some((_, sa, ea)), Some((_, sb, eb))) = (a.decompose(), b.decompose()) else {
        return Float128::NAN;
    };
    match (sa == 0, sb == 0) {
        (true, true) => return Float128::NAN,
        (false, true) => return signed(negative, Float128::INFINITY),
        (true, false) => return signed(negative, Float128::ZERO),
        (false, false) => {}
    }
    // Both significands with their leading one at bit 112, so the
    // quotient of the two lies in (1/2, 2).
    let (sa, ea) = normalise(sa, ea);
    let (sb, eb) = normalise(sb, eb);
    // Long division, one quotient bit per step: the integral bit, then
    // the 112 + 3 that follow it. The remainder stays below 2·sb < 2^114.
    let steps = FRACTION_BITS + 1 + DIV_EXTRA;
    let mut remainder = sa;
    let mut quotient: u128 = 0;
    for _ in 0..steps {
        quotient <<= 1;
        if remainder >= sb {
            remainder -= sb;
            quotient |= 1;
        }
        remainder <<= 1;
    }
    let sticky = u128::from(remainder != 0);
    let exponent = ea - eb - (FRACTION_BITS + DIV_EXTRA) as i32;
    let (value, _) = Float128::from_parts_rounded(negative, quotient | sticky, exponent);
    value
}

/// Shift right by `shift`, folding every bit shifted out into the lowest
/// bit that remains — the sticky bit of the IEEE rounding.
const fn jam_shift_right(value: u128, shift: u32) -> u128 {
    if shift == 0 {
        return value;
    }
    if shift >= 128 {
        return (value != 0) as u128;
    }
    let lost = value & ((1u128 << shift) - 1);
    (value >> shift) | (lost != 0) as u128
}

/// The leading one of a non-zero significand moved to bit 112, and the
/// exponent moved with it.
fn normalise(significand: u128, exponent: i32) -> (u128, i32) {
    let leading = 127 - significand.leading_zeros();
    let shift = FRACTION_BITS - leading;
    (significand << shift, exponent - shift as i32)
}

/// The full 256-bit product of two values below 2^113, as (high, low).
fn wide_mul(a: u128, b: u128) -> (u128, u128) {
    let mask = u128::from(u64::MAX);
    let (a1, a0) = (a >> 64, a & mask);
    let (b1, b0) = (b >> 64, b & mask);
    let low_low = a0 * b0;
    let cross_a = a1 * b0;
    let cross_b = a0 * b1;
    let high_high = a1 * b1;
    // The cross terms are below 2^113 each, their sum below 2^114.
    let cross = cross_a + cross_b;
    let (low, carry) = low_low.overflowing_add(cross << 64);
    let high = high_high + (cross >> 64) + u128::from(carry);
    (high, low)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(value: f64) -> Float128 {
        Float128::from_f64(value)
    }

    #[test]
    fn small_exact_results_agree_with_f64() {
        for (a, b) in [(1.5, 2.25), (-3.0, 0.5), (7.0, 7.0)] {
            assert_eq!((f(a) + f(b)).to_bits(), f(a + b).to_bits(), "{a} + {b}");
            assert_eq!((f(a) * f(b)).to_bits(), f(a * b).to_bits(), "{a} * {b}");
        }
        assert_eq!((f(1.0) / f(4.0)).to_bits(), f(0.25).to_bits());
        assert_eq!((f(3.0) - f(3.0)).to_bits(), Float128::ZERO.to_bits());
    }
}
