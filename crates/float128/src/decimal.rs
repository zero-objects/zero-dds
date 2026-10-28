// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Decimal in and decimal out, both exact.
//!
//! A corpus value file is JSON and a JSON number is a `double`, so a
//! `long double` has to travel as text — the same rule this project
//! already applies to `long long`, for the same reason. Text is only a
//! carrier if it round-trips, so both directions here are correctly
//! rounded against the exact value and neither goes through an `f64`.
//!
//! - [`Float128::to_exact_decimal`] writes every digit the value has.
//!   Finite binary values always have finitely many, up to about 11 500
//!   for the smallest subnormal.
//! - [`Float128::to_shortest_decimal`] writes the fewest digits that
//!   read back as the same bit pattern, found by asking the parser
//!   rather than by trusting a table.
//! - `FromStr` reads either back, plus `inf`, `-inf` and `nan`.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use core::str::FromStr;

use crate::bigint::BigUint;
use crate::float::Float128;

/// Why a decimal string is not a binary128 value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseFloat128Error {
    /// The string held no digits at all.
    Empty,
    /// A character that cannot appear where it did.
    Unexpected {
        /// The offending character.
        found: char,
        /// Its byte offset in the input.
        at: usize,
    },
    /// An exponent that is not an integer, or does not fit one.
    BadExponent,
}

impl fmt::Display for ParseFloat128Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "a binary128 value needs at least one digit"),
            Self::Unexpected { found, at } => {
                write!(
                    f,
                    "`{found}` cannot appear at byte {at} of a binary128 value"
                )
            }
            Self::BadExponent => write!(f, "the exponent is not an integer this crate can use"),
        }
    }
}

impl core::error::Error for ParseFloat128Error {}

/// Past this the value is certainly out of range in one direction or the
/// other, and the exact path would build a megabyte of limbs to find
/// that out. binary128 spans about `1e-4966` to `1e4932`.
const DECIMAL_LIMIT: i64 = 6000;

impl Float128 {
    /// Every digit the value has, exactly.
    ///
    /// `1.5` gives `"1.5"`; the smallest subnormal gives its full 11 500
    /// digits. Infinities and NaNs give `"inf"`, `"-inf"` and `"nan"`,
    /// which is what `FromStr` reads back.
    #[must_use]
    pub fn to_exact_decimal(&self) -> String {
        match self.exact_digits() {
            None => non_finite(*self).to_string(),
            Some((negative, digits, point)) => render(negative, &digits, point),
        }
    }

    /// The fewest digits that read back as this exact bit pattern.
    ///
    /// Found by rounding the exact expansion to one digit, then two, and
    /// asking `FromStr` whether it comes back — so the answer is true of
    /// *this* parser rather than true of a rule about digit counts.
    /// Thirty-six always succeeds; [`Float128::DIGITS_ROUND_TRIP`] is
    /// that bound.
    #[must_use]
    pub fn to_shortest_decimal(&self) -> String {
        let Some((negative, digits, point)) = self.exact_digits() else {
            return non_finite(*self).to_string();
        };
        if digits.is_empty() {
            return render(negative, &digits, point);
        }
        for length in 1..=Self::DIGITS_ROUND_TRIP {
            if length >= digits.len() {
                break;
            }
            let (rounded, rounded_point) = round_significant(&digits, point, length);
            let text = render(negative, &rounded, rounded_point);
            if Self::from_str(&text).is_ok_and(|back| back.to_bits() == self.to_bits()) {
                return text;
            }
        }
        render(negative, &digits, point)
    }

    /// The exact expansion as `(negative, digits, point)`, where the
    /// value is `0.<digits> * 10^point` and `digits` has no leading
    /// zero. `None` for an infinity or a NaN; an empty digit list for a
    /// zero, whose sign is still in the first field.
    fn exact_digits(&self) -> Option<(bool, Vec<u8>, i32)> {
        let (negative, significand, exponent) = self.decompose()?;
        if significand == 0 {
            return Some((negative, Vec::new(), 0));
        }
        let mut value = BigUint::from_u128(significand);
        let point;
        if exponent >= 0 {
            value.shl(exponent as usize);
            let mut digits = value.to_decimal_digits();
            point = decimal_point(digits.len(), 0);
            trim_trailing_zeros(&mut digits);
            return Some((negative, digits, point));
        }
        // m * 2^-k is exactly m * 5^k * 10^-k, which is how a binary
        // fraction becomes a terminating decimal one.
        let scale = (-exponent) as usize;
        value.mul_pow5(scale);
        let mut digits = value.to_decimal_digits();
        point = decimal_point(digits.len(), scale);
        trim_trailing_zeros(&mut digits);
        Some((negative, digits, point))
    }
}

/// The decimal point position for `digits` digits scaled by `10^-scale`.
///
/// A separate function because it is the one arithmetic in this file that
/// could overflow an `i32`, and it cannot: the exact expansion of a
/// binary128 is at most 11 563 digits and the scale at most 16 494, both
/// four orders of magnitude below `i32::MAX`. Saturating rather than
/// wrapping so that a future value which somehow exceeded the bound would
/// render as an absurd exponent rather than as a plausible wrong one.
fn decimal_point(digits: usize, scale: usize) -> i32 {
    let digits = i32::try_from(digits).unwrap_or(i32::MAX);
    let scale = i32::try_from(scale).unwrap_or(i32::MAX);
    digits.saturating_sub(scale)
}

/// Drop trailing zeros, which the point already accounts for.
///
/// `0.5` comes out of the exact expansion as a five and 112 zeros —
/// every binary fraction ends in a long tail of them, because `2^-k`
/// scaled to a decimal is `5^k` and that always ends in a five. Keeping
/// them would make the exact form of a half 113 characters long and
/// `to_shortest_decimal` search for a length it already has.
fn trim_trailing_zeros(digits: &mut Vec<u8>) {
    while digits.last() == Some(&0) {
        digits.pop();
    }
}

/// The word for a value with no digits.
fn non_finite(value: Float128) -> &'static str {
    if value.is_nan() {
        "nan"
    } else if value.is_sign_negative() {
        "-inf"
    } else {
        "inf"
    }
}

/// Round `digits` to `length` significant digits, half to even.
///
/// Returns the shortened digits and the adjusted point, which moves when
/// rounding carries `999` up to `1000`.
fn round_significant(digits: &[u8], point: i32, length: usize) -> (Vec<u8>, i32) {
    let mut kept: Vec<u8> = digits[..length].to_vec();
    let next = digits[length];
    let rest_nonzero = digits[length + 1..].iter().any(|&digit| digit != 0);
    let round_up = match next.cmp(&5) {
        core::cmp::Ordering::Greater => true,
        core::cmp::Ordering::Less => false,
        core::cmp::Ordering::Equal => rest_nonzero || kept[length - 1] % 2 == 1,
    };
    if !round_up {
        while kept.last() == Some(&0) {
            kept.pop();
        }
        return (kept, point);
    }
    let mut index = length;
    loop {
        if index == 0 {
            // Every digit carried: 999 -> 1000, one place wider.
            kept.insert(0, 1);
            kept.truncate(length);
            while kept.last() == Some(&0) {
                kept.pop();
            }
            return (kept, point + 1);
        }
        index -= 1;
        if kept[index] == 9 {
            kept[index] = 0;
        } else {
            kept[index] += 1;
            break;
        }
    }
    while kept.last() == Some(&0) {
        kept.pop();
    }
    (kept, point)
}

/// Write `0.<digits> * 10^point` in the most readable form that reads
/// back the same.
///
/// Plain decimal while that stays short, scientific otherwise. The
/// threshold is about readability only: both forms parse, and
/// `to_shortest_decimal` checks the one that comes out.
fn render(negative: bool, digits: &[u8], point: i32) -> String {
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if digits.is_empty() {
        out.push_str("0.0");
        return out;
    }
    let length = decimal_point(digits.len(), 0);
    if point > 0 && point <= 34 {
        // ddd.ddd
        for (index, &digit) in digits.iter().enumerate() {
            if decimal_point(index, 0) == point {
                out.push('.');
            }
            out.push(char::from(b'0' + digit));
        }
        if length <= point {
            for _ in length..point {
                out.push('0');
            }
            out.push_str(".0");
        }
        return out;
    }
    if point <= 0 && point > -6 {
        // 0.000ddd
        out.push_str("0.");
        for _ in 0..-point {
            out.push('0');
        }
        for &digit in digits {
            out.push(char::from(b'0' + digit));
        }
        return out;
    }
    // d.ddde±xx
    out.push(char::from(b'0' + digits[0]));
    out.push('.');
    if digits.len() == 1 {
        out.push('0');
    } else {
        for &digit in &digits[1..] {
            out.push(char::from(b'0' + digit));
        }
    }
    out.push('e');
    let exponent = point - 1;
    if exponent >= 0 {
        out.push('+');
    }
    out.push_str(&exponent.to_string());
    out
}

impl fmt::Display for Float128 {
    /// The shortest round-tripping decimal.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&self.to_shortest_decimal())
    }
}

impl FromStr for Float128 {
    type Err = ParseFloat128Error;

    /// Correctly rounded, round to nearest with ties to even.
    ///
    /// Never goes through an `f64`: a two-step parse rounds twice, and
    /// double rounding gets the last bit wrong for the values that
    /// matter here.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse(text)
    }
}

/// The parser behind `FromStr`.
fn parse(text: &str) -> Result<Float128, ParseFloat128Error> {
    let trimmed = text.trim();
    let (negative, body) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    if body.is_empty() {
        return Err(ParseFloat128Error::Empty);
    }
    if body.eq_ignore_ascii_case("inf") || body.eq_ignore_ascii_case("infinity") {
        return Ok(if negative {
            Float128::NEG_INFINITY
        } else {
            Float128::INFINITY
        });
    }
    if body.eq_ignore_ascii_case("nan") {
        let nan = Float128::NAN;
        return Ok(if negative { nan.neg() } else { nan });
    }

    // Split off the exponent first so the digit walk has one job.
    let (mantissa, exponent_text) = match body.find(['e', 'E']) {
        Some(index) => (&body[..index], Some(&body[index + 1..])),
        None => (body, None),
    };
    let mut exponent: i64 = match exponent_text {
        None => 0,
        Some(digits) => digits
            .parse()
            .map_err(|_| ParseFloat128Error::BadExponent)?,
    };

    let mut significand = BigUint::zero();
    let mut seen_digit = false;
    let mut seen_point = false;
    let mut fraction_digits: i64 = 0;
    for (offset, character) in mantissa.char_indices() {
        match character {
            '0'..='9' => {
                seen_digit = true;
                significand.mul_add_small(10, u32::from(character as u8 - b'0'));
                if seen_point {
                    fraction_digits += 1;
                }
            }
            '.' if !seen_point => seen_point = true,
            // An underscore is a Rust literal separator, not a decimal
            // one, and accepting it here would make `1_5` parse as
            // fifteen in a value file where it was meant as a typo.
            other => {
                return Err(ParseFloat128Error::Unexpected {
                    found: other,
                    at: offset,
                });
            }
        }
    }
    if !seen_digit {
        return Err(ParseFloat128Error::Empty);
    }
    exponent -= fraction_digits;

    if significand.is_zero() {
        return Ok(if negative {
            Float128::NEG_ZERO
        } else {
            Float128::ZERO
        });
    }

    // Reject the magnitudes whose exact expansion would be megabytes of
    // limbs, but only from the safe side of the estimate. A digit count
    // derived from the bit length is off by a digit or two either way,
    // so the overflow test uses the low estimate and the underflow test
    // the high one: a bound that guessed in the convenient direction
    // would answer infinity for a value that has one.
    // A bit length that did not fit an `i64` would need an exabyte of
    // limbs; the cast is total on every target this builds for.
    let bits = significand.bit_length() as i64;
    let digits_low = bits * 30103 / 100_000;
    let digits_high = digits_low + 2;
    if exponent + digits_low > DECIMAL_LIMIT {
        return Ok(if negative {
            Float128::NEG_INFINITY
        } else {
            Float128::INFINITY
        });
    }
    if exponent + digits_high < -DECIMAL_LIMIT {
        return Ok(if negative {
            Float128::NEG_ZERO
        } else {
            Float128::ZERO
        });
    }

    if exponent >= 0 {
        // Non-negative by the branch, and bounded by `DECIMAL_LIMIT`
        // above, so the width is not in question and there is no fallback
        // for a caller to be wrong about.
        let mut scaled = significand;
        scaled.mul_pow10(exponent.unsigned_abs() as usize);
        let (value, shift) = significand_with_sticky(scaled);
        return Ok(Float128::from_parts_rounded(negative, value, shift).0);
    }

    // value = significand / 10^scale. Shift the numerator up far enough
    // that the quotient carries more bits than binary128 needs, divide,
    // and fold every discarded remainder into one sticky bit — which is
    // what makes the single rounding at the end the correct one.
    // Positive by the branch, and bounded by `DECIMAL_LIMIT`. A fallback
    // of zero here would divide by 10^0 and answer with a value a
    // thousand orders of magnitude out.
    let scale = exponent.unsigned_abs() as usize;
    let wanted = 130 + scale * 10 / 3;
    let have = significand.bit_length();
    let shift = wanted.saturating_sub(have);
    let mut numerator = significand;
    numerator.shl(shift);
    let sticky = divide_by_pow10(&mut numerator, scale);
    let (mut value, extra) = significand_with_sticky(numerator);
    if sticky {
        value |= 1;
    }
    let exponent = extra - decimal_point(shift, 0);
    Ok(Float128::from_parts_rounded(negative, value, exponent).0)
}

/// Divide by `10^power` in nine-digit steps, returning whether anything
/// was discarded.
///
/// `a / 10^18 == (a / 10^9) / 10^9`, and the two-step remainder is zero
/// exactly when both single-step remainders are, which is why one
/// boolean carries the whole sticky state.
fn divide_by_pow10(value: &mut BigUint, power: usize) -> bool {
    const CHUNK: u32 = 1_000_000_000;
    const CHUNK_POWER: usize = 9;
    let mut sticky = false;
    let mut left = power;
    while left >= CHUNK_POWER {
        sticky |= value.div_rem_small(CHUNK) != 0;
        left -= CHUNK_POWER;
    }
    if left != 0 {
        let mut divisor = 1u32;
        for _ in 0..left {
            divisor *= 10;
        }
        sticky |= value.div_rem_small(divisor) != 0;
    }
    sticky
}

/// The top bits of a big integer as `(value, shift)` with
/// `original == value * 2^shift`, keeping enough bits that one
/// round-to-nearest at the end is the correct one, and folding what was
/// dropped into the low bit.
fn significand_with_sticky(value: BigUint) -> (u128, i32) {
    let bits = value.bit_length();
    if bits <= 127 {
        return (value.to_u128().0, 0);
    }
    // 118 kept bits leave five past binary128's 113, so the guard,
    // round and sticky positions are all inside the `u128`.
    let drop = bits - 118;
    let mut kept = value;
    let lost = kept.shr(drop);
    let (mut low, _) = kept.to_u128();
    if lost {
        low |= 1;
    }
    (low, decimal_point(drop, 0))
}
