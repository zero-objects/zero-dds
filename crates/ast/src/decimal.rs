// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fixed-point constant arithmetic, OMG IDL 4.2 §7.4.1.4.2.
//!
//! A value is a sign, a run of decimal digits and the count of them that
//! stand after the point — the shape `ConstValue::Fixed` keeps — and every
//! operation is exact: the clause asks for "double precision (i.e., 62
//! digits) arithmetic" for each pairwise computation, which is the length
//! of the product of two 31-digit operands, and for a result of more than
//! 31 significant digits to keep 31 of them, "the omitted digits shall be
//! discarded; rounding shall not be performed". Digits are held one per
//! byte, most significant first; sixty-two of them are a short loop.

use alloc::string::String;
use alloc::vec::Vec;

/// The most significant digits a fixed-point value keeps (IDL 4.2
/// §7.4.1.4.2: `fixed<d,s> => fixed<31, 31-d+s>`).
pub(crate) const MAX_DIGITS: usize = 31;

/// A decimal: `digits` with `scale` of them after the point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decimal {
    negative: bool,
    /// Most significant first, no leading zeros except a lone `0`.
    digits: Vec<u8>,
    scale: usize,
}

/// Why a fixed-point computation has no result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecimalError {
    /// A digit string that is not one.
    NotDecimal,
    /// Division by zero.
    DivisionByZero,
    /// More than 31 digits before the point: there is nothing on the
    /// right to discard.
    TooLarge,
}

impl Decimal {
    /// The value `ConstValue::Fixed { digits, scale }` stands for. A
    /// leading `-` is the sign.
    pub(crate) fn parse(text: &str, scale: u8) -> Result<Self, DecimalError> {
        let (negative, magnitude) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        if magnitude.is_empty() || !magnitude.bytes().all(|b| b.is_ascii_digit()) {
            return Err(DecimalError::NotDecimal);
        }
        let digits = magnitude.bytes().map(|b| b - b'0').collect();
        Ok(Self {
            negative,
            digits,
            scale: usize::from(scale),
        }
        .normalized())
    }

    /// The digits as text, with `-` in front of a negative value, and the
    /// scale — the two halves `ConstValue::Fixed` keeps.
    ///
    /// # Errors
    /// [`DecimalError::TooLarge`] when the scale does not fit `u8`, which
    /// a value cut to 31 digits never reaches.
    pub(crate) fn to_parts(&self) -> Result<(String, u8), DecimalError> {
        let scale = u8::try_from(self.scale).map_err(|_| DecimalError::TooLarge)?;
        let mut text = String::with_capacity(self.digits.len() + 1);
        if self.negative && !self.is_zero() {
            text.push('-');
        }
        // Pad so there is a digit for every place after the point; the
        // reader of the two halves puts the point back by the scale.
        for _ in self.digits.len()..self.scale {
            text.push('0');
        }
        text.extend(self.digits.iter().map(|d| char::from(b'0' + d)));
        Ok((text, scale))
    }

    fn is_zero(&self) -> bool {
        self.digits.iter().all(|d| *d == 0)
    }

    /// Leading zeros go; a lone zero stays.
    fn normalized(mut self) -> Self {
        let first = self
            .digits
            .iter()
            .position(|d| *d != 0)
            .unwrap_or(self.digits.len().saturating_sub(1));
        self.digits.drain(..first);
        if self.digits.is_empty() {
            self.digits.push(0);
        }
        if self.is_zero() {
            self.negative = false;
        }
        self
    }

    /// The same value at `scale` places, `scale >= self.scale`.
    fn rescaled(&self, scale: usize) -> Vec<u8> {
        let mut digits = self.digits.clone();
        digits.resize(digits.len() + (scale - self.scale), 0);
        digits
    }

    /// Unary minus.
    pub(crate) fn negated(mut self) -> Self {
        if !self.is_zero() {
            self.negative = !self.negative;
        }
        self
    }

    pub(crate) fn add(&self, other: &Self) -> Result<Self, DecimalError> {
        let scale = self.scale.max(other.scale);
        let a = self.rescaled(scale);
        let b = other.rescaled(scale);
        let (negative, digits) = if self.negative == other.negative {
            (self.negative, add_magnitudes(&a, &b))
        } else {
            match compare(&a, &b) {
                core::cmp::Ordering::Less => (other.negative, subtract_magnitudes(&b, &a)),
                _ => (self.negative, subtract_magnitudes(&a, &b)),
            }
        };
        Self {
            negative,
            digits,
            scale,
        }
        .normalized()
        .kept()
    }

    pub(crate) fn subtract(&self, other: &Self) -> Result<Self, DecimalError> {
        self.add(&other.clone().negated())
    }

    pub(crate) fn multiply(&self, other: &Self) -> Result<Self, DecimalError> {
        let mut product = alloc::vec![0u32; self.digits.len() + other.digits.len()];
        for (i, a) in self.digits.iter().enumerate().rev() {
            for (j, b) in other.digits.iter().enumerate().rev() {
                product[i + j + 1] += u32::from(*a) * u32::from(*b);
            }
        }
        for at in (1..product.len()).rev() {
            let carry = product[at] / 10;
            product[at] %= 10;
            product[at - 1] += carry;
        }
        Self {
            negative: self.negative != other.negative,
            digits: product.into_iter().map(|d| d as u8).collect(),
            scale: self.scale + other.scale,
        }
        .normalized()
        .kept()
    }

    /// `fixed<(d1-s1+s2) + sinf, sinf>`: as many places after the point
    /// as it takes to reach 31 significant digits, or until the division
    /// comes out even.
    pub(crate) fn divide(&self, other: &Self) -> Result<Self, DecimalError> {
        if other.is_zero() {
            return Err(DecimalError::DivisionByZero);
        }
        // a/10^sa ÷ b/10^sb = (a·10^sb ÷ b) / 10^sa: the dividend at the
        // divisor's scale divides as two integers.
        let mut dividend = self.digits.clone();
        dividend.resize(dividend.len() + other.scale, 0);
        let divisor = &other.digits;
        let mut quotient = Vec::new();
        let mut remainder: Vec<u8> = alloc::vec![0];
        let mut places = 0usize;
        let mut next = dividend.into_iter();
        loop {
            let digit = match next.next() {
                Some(d) => d,
                None => {
                    let significant = quotient.iter().skip_while(|d| **d == 0).count();
                    if remainder.iter().all(|d| *d == 0) || significant > MAX_DIGITS {
                        break;
                    }
                    places += 1;
                    0
                }
            };
            remainder.push(digit);
            remainder = trimmed(remainder);
            let mut count = 0u8;
            while compare(&remainder, divisor) != core::cmp::Ordering::Less {
                remainder = trimmed(subtract_magnitudes(&remainder, divisor));
                count += 1;
            }
            quotient.push(count);
        }
        Self {
            negative: self.negative != other.negative,
            digits: quotient,
            scale: self.scale + places,
        }
        .normalized()
        .kept()
    }

    /// The clause's cut: past 31 significant digits, the ones on the right
    /// are discarded, without rounding. Trailing zeros after the point are
    /// not significant and go first.
    fn kept(mut self) -> Result<Self, DecimalError> {
        while self.scale > 0 && self.digits.len() > 1 && self.digits.last() == Some(&0) {
            self.digits.pop();
            self.scale -= 1;
        }
        if self.is_zero() {
            self.scale = 0;
        }
        if self.digits.len() > MAX_DIGITS {
            let excess = self.digits.len() - MAX_DIGITS;
            if excess > self.scale {
                return Err(DecimalError::TooLarge);
            }
            self.digits.truncate(MAX_DIGITS);
            self.scale -= excess;
        }
        Ok(self.normalized())
    }

    /// Whether the value fits `fixed<digits, scale>` once the places past
    /// `scale` are discarded, and the value so cut. IDL 4.2 §7.4.1.4.2:
    /// "too large to fit … shall be treated as an error. Truncation on the
    /// right for fixed point types is OK."
    pub(crate) fn fitted(&self, digits: u8, scale: u8) -> Option<Self> {
        let scale = usize::from(scale);
        let mut cut = self.clone();
        if cut.scale > scale {
            let drop = cut.scale - scale;
            let keep = cut.digits.len().saturating_sub(drop);
            cut.digits.truncate(keep);
            if cut.digits.is_empty() {
                cut.digits.push(0);
            }
            cut.scale = scale;
            cut = cut.normalized();
        }
        let whole = cut.digits.len().saturating_sub(cut.scale);
        let whole = if cut.is_zero() { 0 } else { whole };
        (whole <= usize::from(digits).saturating_sub(scale)).then_some(cut)
    }
}

fn compare(a: &[u8], b: &[u8]) -> core::cmp::Ordering {
    let a = &a[a.iter().position(|d| *d != 0).unwrap_or(a.len())..];
    let b = &b[b.iter().position(|d| *d != 0).unwrap_or(b.len())..];
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

fn trimmed(mut digits: Vec<u8>) -> Vec<u8> {
    let first = digits
        .iter()
        .position(|d| *d != 0)
        .unwrap_or(digits.len().saturating_sub(1));
    digits.drain(..first);
    if digits.is_empty() {
        digits.push(0);
    }
    digits
}

fn add_magnitudes(a: &[u8], b: &[u8]) -> Vec<u8> {
    let width = a.len().max(b.len()) + 1;
    let mut out = alloc::vec![0u8; width];
    let mut carry = 0u8;
    for at in 0..width {
        let x = a.len().checked_sub(at + 1).map_or(0, |i| a[i]);
        let y = b.len().checked_sub(at + 1).map_or(0, |i| b[i]);
        let sum = x + y + carry;
        out[width - 1 - at] = sum % 10;
        carry = sum / 10;
    }
    out
}

/// `a - b` for `a >= b`.
fn subtract_magnitudes(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = alloc::vec![0u8; a.len()];
    let mut borrow = 0i8;
    for at in 0..a.len() {
        let x = i8::try_from(a[a.len() - 1 - at]).unwrap_or(0);
        let y = b
            .len()
            .checked_sub(at + 1)
            .map_or(0, |i| i8::try_from(b[i]).unwrap_or(0));
        let mut difference = x - y - borrow;
        borrow = 0;
        if difference < 0 {
            difference += 10;
            borrow = 1;
        }
        out[a.len() - 1 - at] = u8::try_from(difference).unwrap_or(0);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn d(text: &str, scale: u8) -> Decimal {
        Decimal::parse(text, scale).unwrap()
    }

    fn parts(value: &Decimal) -> (String, u8) {
        value.to_parts().unwrap()
    }

    #[test]
    fn sums_and_differences_line_the_points_up() {
        // 1.50 + 2.5 = 4.0, the trailing zero not significant.
        assert_eq!(
            parts(&d("150", 2).add(&d("25", 1)).unwrap()),
            ("4".into(), 0)
        );
        assert_eq!(
            parts(&d("1", 0).subtract(&d("15", 1)).unwrap()),
            ("-5".into(), 1)
        );
        assert_eq!(
            parts(&d("-25", 1).add(&d("25", 1)).unwrap()),
            ("0".into(), 0)
        );
    }

    #[test]
    fn a_product_carries_both_scales() {
        assert_eq!(
            parts(&d("125", 2).multiply(&d("-4", 0)).unwrap()),
            ("-5".into(), 0)
        );
        assert_eq!(
            parts(&d("11", 1).multiply(&d("11", 1)).unwrap()),
            ("121".into(), 2)
        );
    }

    #[test]
    fn a_quotient_runs_to_31_digits_and_is_cut_not_rounded() {
        // 2 / 3: thirty-one sixes after `0.`, the last one not rounded up.
        let (digits, scale) = parts(&d("2", 0).divide(&d("3", 0)).unwrap());
        assert_eq!(digits, "6".repeat(31));
        assert_eq!(scale, 31);
        assert_eq!(
            parts(&d("1", 0).divide(&d("8", 0)).unwrap()),
            ("125".into(), 3)
        );
        assert_eq!(
            parts(&d("75", 1).divide(&d("25", 1)).unwrap()),
            ("3".into(), 0)
        );
        assert_eq!(
            d("1", 0).divide(&d("0", 0)),
            Err(DecimalError::DivisionByZero)
        );
    }

    #[test]
    fn more_than_31_whole_digits_is_too_large() {
        let big = d(&"9".repeat(31), 0);
        assert_eq!(big.add(&d("1", 0)), Err(DecimalError::TooLarge));
        assert!(big.add(&d("0", 0)).is_ok());
    }

    #[test]
    fn a_declared_type_cuts_on_the_right_and_refuses_on_the_left() {
        let value = d("123456", 3); // 123.456
        assert_eq!(parts(&value.fitted(5, 2).unwrap()), ("12345".into(), 2));
        assert_eq!(value.fitted(4, 2), None);
    }
}
