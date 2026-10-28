// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The 80-bit extended format, because the field sends it.
//!
//! Fast DDS and OpenDDS both map IDL `long double` to the compiler's
//! `long double` and copy its sixteen bytes onto the wire. On x86-64
//! that type is the x87 double-extended format — one sign bit, a
//! fifteen-bit exponent biased by 16383, and a **sixty-four**-bit
//! significand with an *explicit* integer bit — padded to sixteen bytes
//! with six bytes the compiler never writes. Measured on Debian 13 /
//! gcc 14.2, `1.5L` is
//! `00 00 00 00 00 00 00 c0 ff 3f` followed by six bytes of whatever the
//! stack held. On aarch64 the same source emits binary128 instead,
//! because there `long double` *is* binary128.
//!
//! So the same vendor, the same IDL and the same value give two
//! different payloads on two architectures, and a third on a platform
//! where `long double` is `double` — macOS on Apple silicon, measured at
//! `LDBL_MANT_DIG = 53`. `docs/float128-column.md` §2 records the runs.
//!
//! A binary128 type that could not read those bytes would be conformant
//! and useless. These conversions are how the reader meets the field
//! where it is; nothing here writes x87 unless a caller asks for it by
//! name, and [`Float128::to_x87_extended`] reports what a conversion
//! costs rather than performing it silently.

use crate::float::{
    EXPONENT_MAX, EXPONENT_SHIFT, FRACTION_BITS, FRACTION_MASK, Float128, QUIET_BIT,
    round_shift_right,
};

/// Significand bits in x87 double-extended, including the explicit
/// integer bit.
const X87_SIGNIFICAND_BITS: u32 = 64;
/// Fraction bits after the explicit integer bit.
const X87_FRACTION_BITS: u32 = X87_SIGNIFICAND_BITS - 1;
/// The bias, which binary128 happens to share.
const X87_BIAS: i32 = 16383;
/// All-ones exponent.
const X87_EXPONENT_MAX: u32 = 0x7fff;
/// The x87 quiet flag, one below the explicit integer bit.
const X87_QUIET: u64 = 0x4000_0000_0000_0000;
/// The x87 integer bit, explicit where binary128 leaves it implied.
const X87_INTEGER: u64 = 0x8000_0000_0000_0000;
/// NaN payload bits x87 carries: everything below the quiet flag.
const X87_PAYLOAD_MASK: u64 = X87_QUIET - 1;
/// How far an x87 payload moves to sit under binary128's quiet bit.
///
/// x87 carries 62 payload bits topping out at bit 61; binary128 carries
/// 111 topping out at bit 110. Aligning the two tops is a shift of 49.
/// Aligning the two *bottoms* instead would move a NaN's identity into
/// bits a narrowing conversion then discards.
const X87_PAYLOAD_SHIFT: u32 = FRACTION_BITS - 1 - 62;

/// An x87 double-extended value: the ten bytes that carry it.
///
/// Held as `(exponent_and_sign, significand)` rather than as bytes
/// because the padding is not part of the value, and a type that carried
/// the padding would invite a caller to compare two of them and find
/// them different for a reason that is not the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Extended80 {
    /// Sign in bit 15, biased exponent in bits 0..=14.
    pub sign_exponent: u16,
    /// The significand, with the integer bit explicit in bit 63.
    pub significand: u64,
}

impl Extended80 {
    /// From the ten bytes as x86 stores them, least significant first.
    #[must_use]
    pub const fn from_le_bytes(bytes: [u8; 10]) -> Self {
        let significand = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        Self {
            sign_exponent: u16::from_le_bytes([bytes[8], bytes[9]]),
            significand,
        }
    }

    /// The ten bytes, least significant first.
    #[must_use]
    pub const fn to_le_bytes(self) -> [u8; 10] {
        let significand = self.significand.to_le_bytes();
        let sign_exponent = self.sign_exponent.to_le_bytes();
        [
            significand[0],
            significand[1],
            significand[2],
            significand[3],
            significand[4],
            significand[5],
            significand[6],
            significand[7],
            sign_exponent[0],
            sign_exponent[1],
        ]
    }

    /// From the sixteen bytes a Fast DDS or OpenDDS payload holds on
    /// x86-64: the ten that matter, then six the compiler did not write.
    ///
    /// **The trailing six bytes are ignored.** They are padding in the
    /// C object and were measured carrying leftover stack — a reader
    /// that folded them into the value would decode the same number
    /// differently on two runs of the same writer.
    #[must_use]
    pub const fn from_padded_le_bytes(bytes: [u8; 16]) -> Self {
        Self::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            bytes[8], bytes[9],
        ])
    }

    /// The sixteen bytes with the padding **zeroed**.
    ///
    /// Zeroed rather than left uninitialised because XTypes 1.3 §7.4.1.1
    /// requires trailing padding in a serialised sample to be zero, and
    /// because six bytes of process memory on a multicast group is a
    /// disclosure whatever the specification says about it.
    #[must_use]
    pub const fn to_padded_le_bytes(self) -> [u8; 16] {
        let ten = self.to_le_bytes();
        [
            ten[0], ten[1], ten[2], ten[3], ten[4], ten[5], ten[6], ten[7], ten[8], ten[9], 0, 0,
            0, 0, 0, 0,
        ]
    }

    /// The sign bit.
    #[must_use]
    pub const fn is_sign_negative(self) -> bool {
        self.sign_exponent & 0x8000 != 0
    }

    /// The biased exponent.
    #[must_use]
    pub const fn biased_exponent(self) -> u32 {
        (self.sign_exponent & 0x7fff) as u32
    }
}

impl Float128 {
    /// From x87 double-extended. Always exact: 64 significand bits fit
    /// inside 113, and the two formats share an exponent bias and range.
    ///
    /// The unnormal and pseudo-* encodings the 8087 defined and the
    /// 80387 dropped — a cleared integer bit with a non-zero exponent —
    /// have no binary128 counterpart. They are decoded as what the
    /// significand and exponent say, which is what every current x86
    /// treats them as when it does not fault.
    #[must_use]
    pub fn from_x87_extended(value: Extended80) -> Self {
        let negative = value.is_sign_negative();
        let biased = value.biased_exponent();
        let significand = value.significand;

        if biased == X87_EXPONENT_MAX {
            let payload = u128::from(significand & X87_PAYLOAD_MASK);
            if payload == 0 && significand & X87_QUIET == 0 {
                return if negative {
                    Self::NEG_INFINITY
                } else {
                    Self::INFINITY
                };
            }
            // A NaN: the 8087 keeps its integer bit set and its quiet
            // flag one bit below, which is where binary128's quiet bit
            // is relative to the top of the fraction. Aligning the two
            // tops keeps a quiet NaN quiet.
            let quiet = significand & X87_QUIET != 0;
            let fraction = (payload << X87_PAYLOAD_SHIFT) | if quiet { QUIET_BIT } else { 0 };
            // A signalling NaN whose whole payload is zero is not a NaN
            // at all once the flag is dropped; it becomes an infinity.
            // One bit is set instead, which keeps it a NaN and keeps it
            // signalling.
            let fraction = if fraction & FRACTION_MASK == 0 {
                1
            } else {
                fraction
            };
            let bits = (u128::from(negative) << 127)
                | ((EXPONENT_MAX as u128) << EXPONENT_SHIFT)
                | (fraction & FRACTION_MASK);
            return Self::from_bits(bits);
        }
        if significand == 0 {
            return if negative { Self::NEG_ZERO } else { Self::ZERO };
        }
        // Both formats bias by 16383, so the unbiased exponent carries
        // straight across; what differs is where the leading one sits.
        let unbiased = if biased == 0 {
            1 - X87_BIAS
        } else {
            biased as i32 - X87_BIAS
        };
        let (value, lost) = Self::from_parts_rounded(
            negative,
            u128::from(significand),
            unbiased - X87_FRACTION_BITS as i32,
        );
        debug_assert!(!lost, "64 significand bits always fit inside 113");
        value
    }

    /// From the sixteen bytes an x86-64 Fast DDS or OpenDDS writes.
    ///
    /// The padding is dropped; see [`Extended80::from_padded_le_bytes`].
    #[must_use]
    pub fn from_x87_padded_bytes(bytes: [u8; 16]) -> Self {
        Self::from_x87_extended(Extended80::from_padded_le_bytes(bytes))
    }

    /// To x87 double-extended, rounding to nearest with ties to even.
    ///
    /// Returns the value and whether the conversion was exact. It is
    /// inexact whenever the significand needs more than 64 bits, which
    /// is most of binary128 — that is the whole cost of the x86 dialect,
    /// and it is returned rather than swallowed so a bridge can refuse.
    #[must_use]
    pub fn to_x87_extended(self) -> (Extended80, bool) {
        let negative = self.is_sign_negative();
        let sign = if negative { 0x8000u16 } else { 0 };

        if self.is_nan() {
            let payload = ((self.fraction() & !QUIET_BIT) >> X87_PAYLOAD_SHIFT) as u64;
            let quiet = self.to_bits() & QUIET_BIT != 0;
            let significand =
                X87_INTEGER | if quiet { X87_QUIET } else { 0 } | (payload & X87_PAYLOAD_MASK);
            // As in the other direction: never let a NaN collapse into
            // an infinity because its payload fell off the bottom.
            let significand = if significand & !X87_INTEGER == 0 {
                significand | 1
            } else {
                significand
            };
            let lost = self.fraction() & ((1u128 << X87_PAYLOAD_SHIFT) - 1) != 0;
            return (
                Extended80 {
                    sign_exponent: sign | X87_EXPONENT_MAX as u16,
                    significand,
                },
                !lost,
            );
        }
        if self.is_infinite() {
            return (
                Extended80 {
                    sign_exponent: sign | X87_EXPONENT_MAX as u16,
                    significand: X87_INTEGER,
                },
                true,
            );
        }
        let Some((_, significand, exponent)) = self.decompose() else {
            return (
                Extended80 {
                    sign_exponent: sign,
                    significand: 0,
                },
                true,
            );
        };
        if significand == 0 {
            return (
                Extended80 {
                    sign_exponent: sign,
                    significand: 0,
                },
                true,
            );
        }

        let leading = 127 - significand.leading_zeros();
        let unbiased = exponent + leading as i32;
        let biased = unbiased + X87_BIAS;
        if biased >= X87_EXPONENT_MAX as i32 {
            // Cannot happen from a finite binary128, whose exponent
            // range is exactly x87's, but a caller reading this wants to
            // see the case handled rather than assumed away.
            return (
                Extended80 {
                    sign_exponent: sign | X87_EXPONENT_MAX as u16,
                    significand: X87_INTEGER,
                },
                false,
            );
        }

        if biased <= 0 {
            // Positive by construction: `biased` is at most zero here. And
            // `rounded` is a subnormal significand, so it is below 2^63 and
            // the mask below cannot be reached — it is there so the width is
            // a property of the expression and not of a branch above it.
            let drop = (leading as i32 - X87_FRACTION_BITS as i32 + (1 - biased))
                .max(0)
                .unsigned_abs();
            let (rounded, lost) = round_shift_right(significand, drop);
            let extended = Extended80 {
                sign_exponent: sign,
                significand: (rounded & u128::from(u64::MAX)) as u64,
            };
            return (extended, !lost);
        }

        let (fitted, lost) = if leading >= X87_FRACTION_BITS {
            round_shift_right(significand, leading - X87_FRACTION_BITS)
        } else {
            (significand << (X87_FRACTION_BITS - leading), false)
        };
        // Rounding up can carry out of the 64-bit significand.
        let (fitted, biased) = if fitted >> X87_SIGNIFICAND_BITS != 0 {
            (fitted >> 1, biased + 1)
        } else {
            (fitted, biased)
        };
        if biased >= X87_EXPONENT_MAX as i32 {
            return (
                Extended80 {
                    sign_exponent: sign | X87_EXPONENT_MAX as u16,
                    significand: X87_INTEGER,
                },
                false,
            );
        }
        // `biased` was checked into `1..X87_EXPONENT_MAX` above and
        // `fitted` into 64 bits, so both narrowings are exact. Masking
        // makes that a property of the expression rather than of a check
        // three branches away, and it leaves no fallback to be wrong: a
        // conversion that answered zero here would produce a subnormal,
        // which is a different number and not an error.
        let extended = Extended80 {
            sign_exponent: sign | ((biased as u32 & X87_EXPONENT_MAX) as u16),
            significand: (fitted & u128::from(u64::MAX)) as u64,
        };
        (extended, !lost)
    }

    /// To the sixteen bytes an x86-64 `long double` occupies, padding
    /// zeroed.
    ///
    /// The boolean is [`Float128::to_x87_extended`]'s: true when the
    /// value survived.
    #[must_use]
    pub fn to_x87_padded_bytes(self) -> ([u8; 16], bool) {
        let (extended, exact) = self.to_x87_extended();
        (extended.to_padded_le_bytes(), exact)
    }
}

/// Whether a sixteen-byte payload is *plausibly* x87 rather than
/// binary128.
///
/// There is no flag on the wire saying which dialect a peer used, so
/// this is a heuristic and is named one. It answers the question a
/// diagnostic asks — "these bytes decoded to something absurd, is the
/// peer an x86 Fast DDS?" — and nothing decides a decode on it.
///
/// The signal is structural: x87 sets an explicit integer bit at bit 63
/// of the low eight bytes for every normal value, and binary128 puts its
/// exponent there. A binary128 pattern with bit 63 set and bits 64..=79
/// looking like a plausible x87 exponent is possible, so a `true` here
/// is a suggestion, never a verdict.
#[must_use]
pub fn looks_like_x87(bytes: [u8; 16]) -> bool {
    let padding = &bytes[10..];
    if padding.iter().any(|&byte| byte != 0) {
        // Non-zero in the six bytes x87 never writes. Either it is
        // binary128, or it is an x86 writer leaking its stack — which is
        // what Fast DDS was measured doing, so this is not a decision.
        return false;
    }
    let extended = Extended80::from_padded_le_bytes(bytes);
    let biased = extended.biased_exponent();
    let integer_bit = extended.significand & X87_INTEGER != 0;
    if biased == 0 {
        return extended.significand == 0 || !integer_bit;
    }
    integer_bit
}
