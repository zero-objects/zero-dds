// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value itself: the bit pattern, the fields, and the constants.

/// Bits in the stored fraction. IEEE 754-2008 clause 3.6, binary128.
pub(crate) const FRACTION_BITS: u32 = 112;
/// Bits in the biased exponent field.
pub(crate) const EXPONENT_BITS: u32 = 15;
/// What is added to the unbiased exponent to get the stored one.
pub(crate) const EXPONENT_BIAS: i32 = 16383;
/// The stored exponent of an infinity or a NaN — all ones.
pub(crate) const EXPONENT_MAX: u32 = (1 << EXPONENT_BITS) - 1;
/// The stored fraction, masked off.
pub(crate) const FRACTION_MASK: u128 = (1u128 << FRACTION_BITS) - 1;
/// The sign bit.
pub(crate) const SIGN_MASK: u128 = 1u128 << (FRACTION_BITS + EXPONENT_BITS);
/// Where the exponent field starts.
pub(crate) const EXPONENT_SHIFT: u32 = FRACTION_BITS;
/// Significand bits including the implicit one: what "precision" means.
pub(crate) const SIGNIFICAND_BITS: u32 = FRACTION_BITS + 1;
/// The bit that makes a NaN quiet, per IEEE 754-2008 clause 6.2.1.
pub(crate) const QUIET_BIT: u128 = 1u128 << (FRACTION_BITS - 1);

/// What a value is, in the sense of IEEE 754-2008 clause 5.7.2.
///
/// The same five classes `core::num::FpCategory` names, spelled here so
/// that a `no_std` caller matching on them does not have to reach into
/// `core::num` for a type this crate could have owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// A quiet or signalling NaN.
    Nan,
    /// Positive or negative infinity.
    Infinite,
    /// A finite value with the implicit leading one.
    Normal,
    /// A finite value below `MIN_POSITIVE`, with no leading one.
    Subnormal,
    /// Positive or negative zero.
    Zero,
}

/// An IEEE 754-2008 binary128 value.
///
/// Held as the 128-bit pattern, because that is what goes on the wire
/// and every operation here is defined on it. `to_be_bytes` and
/// `from_be_bytes` are the wire form: CORBA Figure 9.2 shows the whole
/// sixteen bytes reversed between the two orders, not two eight-byte
/// halves swapped independently, and RTI Connext 7.7 was measured doing
/// exactly that (`docs/float128-column.md` §2).
///
/// # Equality
///
/// `PartialEq` is IEEE equality, the same as `f64`: `-0.0 == 0.0`, and a
/// NaN equals nothing including itself. So there is no `Eq`, no `Ord`
/// and no `Hash` — a key holder wants the *bit pattern*, which
/// [`Float128::to_bits`] and [`Float128::to_be_bytes`] give, and a total
/// order is [`Float128::total_cmp`]. Deriving `Eq` on IEEE equality
/// would make `-0.0` and `0.0` interchangeable in a hash map, which is
/// how a key holder loses an instance.
#[derive(Debug, Clone, Copy, Default)]
pub struct Float128 {
    bits: u128,
}

impl Float128 {
    /// Positive zero.
    pub const ZERO: Self = Self { bits: 0 };
    /// Negative zero. Distinct from [`Self::ZERO`] on the wire, equal to
    /// it under `==`, ordered below it by [`Self::total_cmp`].
    pub const NEG_ZERO: Self = Self { bits: SIGN_MASK };
    /// One.
    pub const ONE: Self = Self {
        bits: (EXPONENT_BIAS as u128) << EXPONENT_SHIFT,
    };
    /// Minus one.
    pub const NEG_ONE: Self = Self {
        bits: SIGN_MASK | ((EXPONENT_BIAS as u128) << EXPONENT_SHIFT),
    };
    /// Positive infinity.
    pub const INFINITY: Self = Self {
        bits: (EXPONENT_MAX as u128) << EXPONENT_SHIFT,
    };
    /// Negative infinity.
    pub const NEG_INFINITY: Self = Self {
        bits: SIGN_MASK | ((EXPONENT_MAX as u128) << EXPONENT_SHIFT),
    };
    /// A quiet NaN with no payload.
    pub const NAN: Self = Self {
        bits: ((EXPONENT_MAX as u128) << EXPONENT_SHIFT) | QUIET_BIT,
    };
    /// The largest finite value, `(2 - 2^-112) * 2^16383`.
    pub const MAX: Self = Self {
        bits: (((EXPONENT_MAX - 1) as u128) << EXPONENT_SHIFT) | FRACTION_MASK,
    };
    /// The most negative finite value.
    pub const MIN: Self = Self {
        bits: SIGN_MASK | (((EXPONENT_MAX - 1) as u128) << EXPONENT_SHIFT) | FRACTION_MASK,
    };
    /// The smallest positive *normal* value, `2^-16382`.
    pub const MIN_POSITIVE: Self = Self {
        bits: 1u128 << EXPONENT_SHIFT,
    };
    /// The smallest positive value of any kind, `2^-16494`.
    pub const MIN_SUBNORMAL: Self = Self { bits: 1 };
    /// The step from `1.0` to the next larger value, `2^-112`.
    pub const EPSILON: Self = Self {
        bits: ((EXPONENT_BIAS - FRACTION_BITS as i32) as u128) << EXPONENT_SHIFT,
    };

    /// Significand bits including the implicit leading one: 113.
    pub const MANTISSA_DIGITS: u32 = SIGNIFICAND_BITS;
    /// Decimal digits that always round-trip: `ceil(113 * log10 2) + 1`.
    ///
    /// Thirty-six is the number [`Float128::to_shortest_decimal`] never
    /// exceeds and the number a peer needs to read one back exactly.
    pub const DIGITS_ROUND_TRIP: usize = 36;

    /// From the raw 128-bit pattern.
    #[must_use]
    pub const fn from_bits(bits: u128) -> Self {
        Self { bits }
    }

    /// The raw 128-bit pattern.
    #[must_use]
    pub const fn to_bits(self) -> u128 {
        self.bits
    }

    /// From sixteen big-endian bytes — the CDR big-endian wire form.
    #[must_use]
    pub const fn from_be_bytes(bytes: [u8; 16]) -> Self {
        Self {
            bits: u128::from_be_bytes(bytes),
        }
    }

    /// Sixteen big-endian bytes.
    #[must_use]
    pub const fn to_be_bytes(self) -> [u8; 16] {
        self.bits.to_be_bytes()
    }

    /// From sixteen little-endian bytes — the CDR little-endian wire
    /// form, which is the big-endian one reversed as a whole.
    #[must_use]
    pub const fn from_le_bytes(bytes: [u8; 16]) -> Self {
        Self {
            bits: u128::from_le_bytes(bytes),
        }
    }

    /// Sixteen little-endian bytes.
    #[must_use]
    pub const fn to_le_bytes(self) -> [u8; 16] {
        self.bits.to_le_bytes()
    }

    // -- fields ----------------------------------------------------------

    /// The sign bit, as it is stored. True for negative, including
    /// `-0.0` and a negative NaN.
    #[must_use]
    pub const fn is_sign_negative(self) -> bool {
        self.bits & SIGN_MASK != 0
    }

    /// The complement of [`Self::is_sign_negative`].
    #[must_use]
    pub const fn is_sign_positive(self) -> bool {
        !self.is_sign_negative()
    }

    /// The biased exponent field, `0..=32767`.
    #[must_use]
    pub(crate) const fn biased_exponent(self) -> u32 {
        ((self.bits >> EXPONENT_SHIFT) & (EXPONENT_MAX as u128)) as u32
    }

    /// The stored fraction, without the implicit leading one.
    #[must_use]
    pub(crate) const fn fraction(self) -> u128 {
        self.bits & FRACTION_MASK
    }

    /// What kind of value this is.
    #[must_use]
    pub const fn classify(self) -> Class {
        let exponent = self.biased_exponent();
        let fraction = self.fraction();
        if exponent == EXPONENT_MAX {
            if fraction == 0 {
                Class::Infinite
            } else {
                Class::Nan
            }
        } else if exponent == 0 {
            if fraction == 0 {
                Class::Zero
            } else {
                Class::Subnormal
            }
        } else {
            Class::Normal
        }
    }

    /// Whether this is a NaN.
    #[must_use]
    pub const fn is_nan(self) -> bool {
        matches!(self.classify(), Class::Nan)
    }

    /// Whether this is an infinity.
    #[must_use]
    pub const fn is_infinite(self) -> bool {
        matches!(self.classify(), Class::Infinite)
    }

    /// Whether this is neither infinite nor a NaN.
    #[must_use]
    pub const fn is_finite(self) -> bool {
        self.biased_exponent() != EXPONENT_MAX
    }

    /// Whether this is `+0.0` or `-0.0`.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.bits & !SIGN_MASK == 0
    }

    /// Whether this is finite, non-zero and below [`Self::MIN_POSITIVE`].
    #[must_use]
    pub const fn is_subnormal(self) -> bool {
        matches!(self.classify(), Class::Subnormal)
    }

    /// Whether this is finite and normal — not zero, not subnormal.
    #[must_use]
    pub const fn is_normal(self) -> bool {
        matches!(self.classify(), Class::Normal)
    }

    /// Whether this NaN is quiet. False for anything that is not a NaN.
    #[must_use]
    pub const fn is_quiet_nan(self) -> bool {
        self.is_nan() && (self.bits & QUIET_BIT) != 0
    }

    // -- sign operations, which need no arithmetic -----------------------

    /// The same magnitude with the sign bit cleared. NaN payloads and
    /// signalling-ness survive, per IEEE 754-2008 clause 5.5.1.
    #[must_use]
    pub const fn abs(self) -> Self {
        Self {
            bits: self.bits & !SIGN_MASK,
        }
    }

    /// The same magnitude with the sign bit flipped.
    #[must_use]
    pub const fn neg(self) -> Self {
        Self {
            bits: self.bits ^ SIGN_MASK,
        }
    }

    /// This magnitude with `sign`'s sign bit.
    #[must_use]
    pub const fn copysign(self, sign: Self) -> Self {
        Self {
            bits: (self.bits & !SIGN_MASK) | (sign.bits & SIGN_MASK),
        }
    }

    // -- decomposition ---------------------------------------------------

    /// The value as `(negative, significand, exponent)` with
    /// `value = (-1)^negative * significand * 2^exponent`.
    ///
    /// `None` for an infinity or a NaN, which no such triple describes.
    /// The significand is the *integral* one — the implicit leading bit
    /// is materialised for a normal value — so the triple is exact and
    /// needs no separate "is subnormal" flag to interpret.
    #[must_use]
    pub const fn decompose(self) -> Option<(bool, u128, i32)> {
        if !self.is_finite() {
            return None;
        }
        let biased = self.biased_exponent();
        let fraction = self.fraction();
        let (significand, exponent) = if biased == 0 {
            // Subnormal and zero share the exponent of the smallest
            // normal, which is what makes the two encodings continuous.
            (fraction, 1 - EXPONENT_BIAS - FRACTION_BITS as i32)
        } else {
            (
                fraction | (1u128 << FRACTION_BITS),
                biased as i32 - EXPONENT_BIAS - FRACTION_BITS as i32,
            )
        };
        Some((self.is_sign_negative(), significand, exponent))
    }

    /// The inverse of [`Self::decompose`], rounding to nearest with ties
    /// to even and reporting whether that rounding lost anything.
    ///
    /// This is the one place where a value that does not fit is
    /// *constructed* rather than refused, because every conversion into
    /// binary128 funnels through it and each caller decides for itself
    /// what an inexact result means. Overflow gives an infinity, which
    /// is what IEEE 754-2008 clause 7.4 requires of the default rounding
    /// direction, and it is reported inexact so no caller mistakes it
    /// for a value that was sent.
    pub(crate) fn from_parts_rounded(
        negative: bool,
        significand: u128,
        exponent: i32,
    ) -> (Self, bool) {
        if significand == 0 {
            return (if negative { Self::NEG_ZERO } else { Self::ZERO }, false);
        }
        // Clamped before anything is added to it. A decimal parser can
        // hand this an exponent of a hundred thousand, and the answer
        // there is a saturated infinity or zero, not a wrapped `i32`
        // that lands back inside the normal range.
        const OVERFLOW: i32 = EXPONENT_MAX as i32;
        const UNDERFLOW: i32 = -(SIGNIFICAND_BITS as i32) - EXPONENT_BIAS - 8;
        if exponent > OVERFLOW {
            let value = if negative {
                Self::NEG_INFINITY
            } else {
                Self::INFINITY
            };
            return (value, true);
        }
        if exponent < UNDERFLOW - 128 {
            let value = if negative { Self::NEG_ZERO } else { Self::ZERO };
            return (value, true);
        }

        // Normalise so the leading one sits at bit `FRACTION_BITS`.
        let leading = 127 - significand.leading_zeros();
        let mut exponent = exponent;
        let mut significand = significand;
        let mut sticky = false;
        if leading > FRACTION_BITS {
            let drop = leading - FRACTION_BITS;
            let (rounded, lost) = round_shift_right(significand, drop);
            significand = rounded;
            sticky = lost;
            exponent += drop as i32;
            // Rounding up can carry into the next binade.
            if significand > (FRACTION_MASK | (1u128 << FRACTION_BITS)) {
                significand >>= 1;
                exponent += 1;
            }
        } else {
            let shift = FRACTION_BITS - leading;
            significand <<= shift;
            exponent -= shift as i32;
        }

        // `exponent` now scales an integral significand whose leading
        // one is at bit 112, so the stored exponent is offset by that.
        let biased = exponent + EXPONENT_BIAS + FRACTION_BITS as i32;
        if biased >= EXPONENT_MAX as i32 {
            let value = if negative {
                Self::NEG_INFINITY
            } else {
                Self::INFINITY
            };
            return (value, true);
        }
        if biased <= 0 {
            // Subnormal: shift the significand down into the fixed
            // exponent-zero scale, rounding again. The shift can exceed
            // the significand's width, and `round_shift_right` still
            // answers correctly there — a value just over half the
            // smallest subnormal rounds up to it rather than to zero.
            // `biased` is at most zero here, so `1 - biased` is `1 + |biased|`
            // and the conversion is total by construction — no fallback, and
            // so no value a fallback could be wrong about.
            let drop = 1 + biased.unsigned_abs();
            let (rounded, lost) = round_shift_right(significand, drop);
            sticky |= lost;
            // Rounding a subnormal up can reach the smallest normal,
            // whose encoding is exactly `1 << FRACTION_BITS` — the carry
            // out of the fraction lands in the exponent field on its
            // own, which is the point of the biased-exponent layout, so
            // the fraction is deliberately not masked here.
            let bits = (u128::from(negative) << (FRACTION_BITS + EXPONENT_BITS)) | rounded;
            return (Self { bits }, sticky);
        }

        let bits = (u128::from(negative) << (FRACTION_BITS + EXPONENT_BITS))
            | ((biased as u128) << EXPONENT_SHIFT)
            | (significand & FRACTION_MASK);
        (Self { bits }, sticky)
    }
}

/// Shift right by `shift`, rounding to nearest with ties to even.
///
/// Returns the rounded value and whether anything was discarded. A
/// caller that only looked at the value would report a truncation as
/// exact, which is the class of defect this crate exists to remove.
pub(crate) fn round_shift_right(value: u128, shift: u32) -> (u128, bool) {
    if shift == 0 {
        return (value, false);
    }
    if shift >= 128 {
        return (0, value != 0);
    }
    let kept = value >> shift;
    let dropped = value & ((1u128 << shift) - 1);
    if dropped == 0 {
        return (kept, false);
    }
    let half = 1u128 << (shift - 1);
    let rounded = if dropped > half || (dropped == half && kept & 1 == 1) {
        kept + 1
    } else {
        kept
    };
    (rounded, true)
}

impl core::ops::Neg for Float128 {
    type Output = Self;

    fn neg(self) -> Self {
        Self::neg(self)
    }
}
