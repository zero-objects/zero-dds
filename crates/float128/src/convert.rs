// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Conversions in and out, with the inexact cases reported.
//!
//! Every narrowing conversion here comes in two forms: one that refuses
//! a value it cannot carry ([`Float128::to_f64`], [`Float128::to_i64`])
//! and one that rounds and says so ([`Float128::to_f64_rounded`]). There
//! is no third form that rounds in silence, because that is the
//! behaviour this crate replaced.

use core::fmt;

use crate::float::{
    EXPONENT_BIAS, EXPONENT_MAX, EXPONENT_SHIFT, FRACTION_BITS, Float128, QUIET_BIT, SIGN_MASK,
};

/// A conversion that could not be exact.
///
/// Carries the rounded result, so a caller that wants it after all takes
/// it from the error rather than repeating the conversion with a
/// different function and hoping the two agree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Inexact<T> {
    /// What the value rounds to in the destination type.
    pub rounded: T,
    /// What the source was, for a message that names it.
    pub source: Float128,
}

impl<T> fmt::Display for Inexact<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the binary128 value 0x{:032x} does not fit the destination type exactly",
            self.source.to_bits()
        )
    }
}

impl<T: fmt::Debug> core::error::Error for Inexact<T> {}

// -- from the narrower floats, which is always exact ---------------------

impl Float128 {
    /// From an `f64`. Always exact: binary128 covers every binary64
    /// value, subnormals included, with 60 fraction bits to spare.
    #[must_use]
    pub fn from_f64(value: f64) -> Self {
        let bits = value.to_bits();
        let negative = bits >> 63 != 0;
        let biased = ((bits >> 52) & 0x7ff) as u32;
        let fraction = u128::from(bits & 0x000f_ffff_ffff_ffff);
        widen(negative, biased, fraction, 11, 52)
    }

    /// From an `f32`. Always exact, for the same reason.
    #[must_use]
    pub fn from_f32(value: f32) -> Self {
        let bits = value.to_bits();
        let negative = bits >> 31 != 0;
        let biased = (bits >> 23) & 0xff;
        let fraction = u128::from(bits & 0x007f_ffff);
        widen(negative, biased, fraction, 8, 23)
    }

    /// From any signed integer up to 128 bits, rounding to nearest with
    /// ties to even. Exact for everything up to 2^113.
    #[must_use]
    pub fn from_i128_rounded(value: i128) -> Self {
        let negative = value < 0;
        let magnitude = value.unsigned_abs();
        Self::from_parts_rounded(negative, magnitude, 0).0
    }

    /// From any unsigned integer up to 128 bits, rounding to nearest
    /// with ties to even. Exact for everything up to 2^113.
    #[must_use]
    pub fn from_u128_rounded(value: u128) -> Self {
        Self::from_parts_rounded(false, value, 0).0
    }

    /// From a signed integer, refusing one that does not fit exactly.
    ///
    /// # Errors
    /// [`Inexact`] when the integer needs more than 113 significant
    /// bits, carrying what it rounds to.
    pub fn try_from_i128(value: i128) -> Result<Self, Inexact<Self>> {
        let negative = value < 0;
        let (rounded, lost) = Self::from_parts_rounded(negative, value.unsigned_abs(), 0);
        if lost {
            Err(Inexact {
                rounded,
                source: rounded,
            })
        } else {
            Ok(rounded)
        }
    }

    /// From an unsigned integer, refusing one that does not fit exactly.
    ///
    /// # Errors
    /// [`Inexact`] when the integer needs more than 113 significant
    /// bits, carrying what it rounds to.
    pub fn try_from_u128(value: u128) -> Result<Self, Inexact<Self>> {
        let (rounded, lost) = Self::from_parts_rounded(false, value, 0);
        if lost {
            Err(Inexact {
                rounded,
                source: rounded,
            })
        } else {
            Ok(rounded)
        }
    }
}

/// Widen a narrower IEEE binary format into binary128.
///
/// `biased` and `fraction` are the source's fields; `exponent_bits` and
/// `fraction_bits` its widths. Every case is exact, including the source
/// subnormals, which become binary128 *normals* — binary128's exponent
/// range swallows binary64's whole subnormal range.
fn widen(
    negative: bool,
    biased: u32,
    fraction: u128,
    exponent_bits: u32,
    fraction_bits: u32,
) -> Float128 {
    let sign = u128::from(negative) << (FRACTION_BITS + 15);
    let source_max = (1u32 << exponent_bits) - 1;
    let source_bias = (1i32 << (exponent_bits - 1)) - 1;
    let shift = FRACTION_BITS - fraction_bits;

    if biased == source_max {
        // Infinity keeps its emptiness; a NaN keeps its payload in the
        // high fraction bits, which is what puts the source's quiet bit
        // on binary128's quiet bit and keeps a signalling NaN
        // signalling. IEEE 754-2008 clause 6.2.3.
        let bits = sign | ((EXPONENT_MAX as u128) << EXPONENT_SHIFT) | (fraction << shift);
        return Float128::from_bits(bits);
    }
    if biased == 0 {
        if fraction == 0 {
            return Float128::from_bits(sign);
        }
        // A source subnormal: value = fraction * 2^(1 - bias - fbits).
        let exponent = 1 - source_bias - fraction_bits as i32;
        let (value, lost) = Float128::from_parts_rounded(negative, fraction, exponent);
        debug_assert!(!lost, "widening a narrower binary format is always exact");
        return value;
    }
    let biased128 = biased as i32 - source_bias + EXPONENT_BIAS;
    let bits = sign | ((biased128 as u128) << EXPONENT_SHIFT) | (fraction << shift);
    Float128::from_bits(bits)
}

// -- out to the narrower floats ------------------------------------------

impl Float128 {
    /// To an `f64`, rounding to nearest with ties to even.
    ///
    /// Overflow gives an infinity and underflow a signed zero, which is
    /// what IEEE 754-2008 clause 7.4 requires; both are inexact and
    /// [`Self::to_f64`] reports them.
    #[must_use]
    pub fn to_f64_rounded(self) -> f64 {
        self.narrow_f64().0
    }

    /// To an `f64` when the value fits one exactly.
    ///
    /// # Errors
    /// [`Inexact`] carrying the rounded result. This is the refusal the
    /// `long double` column needed: the old writer took an `f64` and put
    /// it on the wire as sixteen bytes, so a value with 113 significant
    /// bits was narrowed on the way in and nobody was told.
    pub fn to_f64(self) -> Result<f64, Inexact<f64>> {
        let (rounded, lost) = self.narrow_f64();
        if lost {
            Err(Inexact {
                rounded,
                source: self,
            })
        } else {
            Ok(rounded)
        }
    }

    /// To an `f32`, rounding to nearest with ties to even.
    #[must_use]
    pub fn to_f32_rounded(self) -> f32 {
        self.narrow_f32().0
    }

    /// To an `f32` when the value fits one exactly.
    ///
    /// # Errors
    /// [`Inexact`] carrying the rounded result.
    pub fn to_f32(self) -> Result<f32, Inexact<f32>> {
        let (rounded, lost) = self.narrow_f32();
        if lost {
            Err(Inexact {
                rounded,
                source: self,
            })
        } else {
            Ok(rounded)
        }
    }

    /// The `f64` narrowing, and whether anything was lost.
    fn narrow_f64(self) -> (f64, bool) {
        let (bits, lost) = self.narrow(11, 52);
        (f64::from_bits(bits as u64), lost)
    }

    /// The `f32` narrowing, and whether anything was lost.
    fn narrow_f32(self) -> (f32, bool) {
        let (bits, lost) = self.narrow(8, 23);
        (f32::from_bits(bits as u32), lost)
    }

    /// Narrow to a smaller IEEE binary format, returning its bit pattern
    /// right-aligned in a `u128` and whether the value survived.
    fn narrow(self, exponent_bits: u32, fraction_bits: u32) -> (u128, bool) {
        let target_max = (1u32 << exponent_bits) - 1;
        let target_bias = (1i32 << (exponent_bits - 1)) - 1;
        let sign = u128::from(self.is_sign_negative()) << (exponent_bits + fraction_bits);
        let shift = FRACTION_BITS - fraction_bits;

        if self.is_nan() {
            // A NaN stays a NaN and keeps as much payload as fits. The
            // quiet bit rides along in the payload rather than being
            // forced on: binary128 puts it at the top of the fraction
            // and so does every narrower format, so the same shift that
            // aligns the payload aligns the flag, and a signalling NaN
            // that arrived signalling leaves that way.
            //
            // The one thing that cannot be allowed is a payload whose
            // set bits all fall off the bottom, because an all-zero
            // fraction is not a NaN but an infinity.
            let mut payload = self.fraction() >> shift;
            if payload == 0 {
                payload = 1;
            }
            let bits = sign | (u128::from(target_max) << fraction_bits) | payload;
            return (bits, self.fraction() & ((1u128 << shift) - 1) != 0);
        }
        if self.is_infinite() {
            return (sign | (u128::from(target_max) << fraction_bits), false);
        }
        let Some((_, significand, exponent)) = self.decompose() else {
            return (sign, false);
        };
        if significand == 0 {
            return (sign, false);
        }

        // Re-normalise into the target's scale: leading one at
        // `fraction_bits`, exponent adjusted to match.
        let leading = 127 - significand.leading_zeros();
        let mut exponent = exponent + leading as i32;
        let target_exponent = exponent + target_bias;
        if target_exponent >= target_max as i32 {
            let bits = sign | (u128::from(target_max) << fraction_bits);
            return (bits, true);
        }

        let lost;
        let mut fraction;
        if target_exponent <= 0 {
            // Subnormal in the target: the exponent is pinned and the
            // significand absorbs the difference.
            // Positive by construction: `target_exponent` is at most zero
            // here and `leading` is at least `fraction_bits` minus that, so
            // the shift is total and needs no fallback.
            let drop = leading as i32 - fraction_bits as i32 + (1 - target_exponent);
            let drop = drop.max(0).unsigned_abs();
            let (rounded, dropped) = crate::float::round_shift_right(significand, drop);
            // A subnormal that rounds up to the smallest normal carries
            // out of the fraction into the exponent field by itself.
            let bits = sign | rounded;
            return (bits, dropped);
        } else if leading > fraction_bits {
            let drop = leading - fraction_bits;
            let (rounded, dropped) = crate::float::round_shift_right(significand, drop);
            lost = dropped;
            fraction = rounded;
            if fraction >> fraction_bits > 1 {
                fraction >>= 1;
                exponent += 1;
            }
        } else {
            fraction = significand << (fraction_bits - leading);
            lost = false;
        }

        let target_exponent = exponent + target_bias;
        if target_exponent >= target_max as i32 {
            let bits = sign | (u128::from(target_max) << fraction_bits);
            return (bits, true);
        }
        let bits = sign
            | ((target_exponent as u128) << fraction_bits)
            | (fraction & ((1u128 << fraction_bits) - 1));
        (bits, lost)
    }
}

// -- out to the integers -------------------------------------------------

impl Float128 {
    /// The value truncated toward zero, when it is an exact integer that
    /// fits an `i128`.
    ///
    /// # Errors
    /// [`Inexact`] carrying the truncated result, for a value with a
    /// fractional part, an infinity, a NaN, or a magnitude past the
    /// destination.
    pub fn to_i128(self) -> Result<i128, Inexact<i128>> {
        let (value, exact) = self.trunc_i128();
        if exact {
            Ok(value)
        } else {
            Err(Inexact {
                rounded: value,
                source: self,
            })
        }
    }

    /// As [`Self::to_i128`], into an `i64`.
    ///
    /// # Errors
    /// [`Inexact`] carrying the saturated or truncated result.
    pub fn to_i64(self) -> Result<i64, Inexact<i64>> {
        match self.to_i128() {
            Ok(wide) => i64::try_from(wide).map_err(|_| Inexact {
                rounded: if wide < 0 { i64::MIN } else { i64::MAX },
                source: self,
            }),
            Err(inexact) => Err(Inexact {
                rounded: i64::try_from(inexact.rounded).unwrap_or(if inexact.rounded < 0 {
                    i64::MIN
                } else {
                    i64::MAX
                }),
                source: self,
            }),
        }
    }

    /// The truncation and whether it was exact.
    fn trunc_i128(self) -> (i128, bool) {
        if self.is_nan() {
            return (0, false);
        }
        if self.is_infinite() {
            return (
                if self.is_sign_negative() {
                    i128::MIN
                } else {
                    i128::MAX
                },
                false,
            );
        }
        let Some((negative, significand, exponent)) = self.decompose() else {
            return (0, false);
        };
        if significand == 0 {
            return (0, true);
        }
        let leading = 127 - significand.leading_zeros();
        let magnitude_bits = leading as i32 + exponent + 1;
        // `i128::MIN` is the one magnitude that needs all 128 bits.
        let limit = if negative { 128 } else { 127 };
        if magnitude_bits > limit {
            return (if negative { i128::MIN } else { i128::MAX }, false);
        }
        let (magnitude, fractional) = if exponent >= 0 {
            // Non-negative by the branch; the `>= 128` test below is the
            // one that matters and it is on the value, not on the cast.
            let shift = exponent.unsigned_abs();
            if shift >= 128 {
                return (if negative { i128::MIN } else { i128::MAX }, false);
            }
            (significand << shift, false)
        } else {
            // Positive by the branch, likewise.
            let shift = exponent.unsigned_abs();
            if shift >= 128 {
                (0, true)
            } else {
                (
                    significand >> shift,
                    significand & ((1u128 << shift) - 1) != 0,
                )
            }
        };
        let signed = if negative {
            magnitude.wrapping_neg() as i128
        } else {
            magnitude as i128
        };
        (signed, !fractional)
    }
}

impl From<f64> for Float128 {
    fn from(value: f64) -> Self {
        Self::from_f64(value)
    }
}

impl From<f32> for Float128 {
    fn from(value: f32) -> Self {
        Self::from_f32(value)
    }
}

impl TryFrom<Float128> for f64 {
    type Error = Inexact<f64>;

    fn try_from(value: Float128) -> Result<Self, Self::Error> {
        value.to_f64()
    }
}

impl TryFrom<Float128> for f32 {
    type Error = Inexact<f32>;

    fn try_from(value: Float128) -> Result<Self, Self::Error> {
        value.to_f32()
    }
}

impl Float128 {
    /// A quiet NaN carrying `payload` in its low fraction bits.
    ///
    /// A wire test needs a NaN it can recognise on the other side, and
    /// `Float128::NAN` is one pattern out of 2^113. The payload is
    /// masked to the bits below the quiet flag, so no argument can
    /// produce an infinity by accident.
    #[must_use]
    pub const fn quiet_nan(payload: u128) -> Self {
        let fraction = (payload & (QUIET_BIT - 1)) | QUIET_BIT;
        Self::from_bits(((EXPONENT_MAX as u128) << EXPONENT_SHIFT) | fraction)
    }

    /// The same, negative.
    #[must_use]
    pub const fn neg_quiet_nan(payload: u128) -> Self {
        Self::from_bits(Self::quiet_nan(payload).to_bits() | SIGN_MASK)
    }
}
