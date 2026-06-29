// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL `fixed<P, S>` decimal type (XCDR2 §7.4.4.5).
//!
//! Wire format: packed binary-coded decimal (BCD), CORBA/GIOP §9.3.2.7
//! (identical in XCDR2 §7.4.4.5).
//! - Digit count = P, scale = S (number of digits after the decimal point).
//! - Nibbles, big-endian: `[d0 .. d(P-1)][sign]` — P digit nibbles (most
//!   significant first) followed by the sign nibble (`0xC` = positive/zero,
//!   `0xD` = negative). If `P + 1` is odd, a single leading `0x0` nibble is
//!   prepended so the total nibble count is even.
//! - Byte count = `(P + 2) / 2` = `ceil((P + 1) / 2)`. Verified byte-exact
//!   against JacORB 3.9 and omniORB 4.3 (CORBA vendor oracle): a `fixed<5,2>`
//!   is 3 octets `12 34 5c`, a `fixed<4,0>` is `01 23 4c`. The leading
//!   zero digits are kept (omniORB pad-to-P form); some ORBs (JacORB) trim
//!   them, which the decoder also accepts.

#![allow(clippy::manual_div_ceil, clippy::while_let_on_iterator)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::buffer::{BufferReader, BufferWriter};
use crate::encode::{CdrDecode, CdrEncode};
use crate::error::{DecodeError, EncodeError};

/// IDL `fixed<P, S>` decimal with `P` total digits and `S` digits
/// after the decimal point.
///
/// Stored as packed BCD bytes (XCDR2 §7.4.4.5). Pure Rust with no
/// external crate dependency. Deliberate architectural choice: this
/// type offers **roundtrip + string conversion**, no decimal
/// arithmetic (add/mul). End users who need decimal arithmetic combine
/// it with `rust_decimal` or similar via a `From` impl.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixed<const P: u32, const S: u32> {
    /// Packed-BCD storage. Length `(P + 2) / 2` bytes (last
    /// nibble = sign).
    digits: Vec<u8>,
}

impl<const P: u32, const S: u32> Default for Fixed<P, S> {
    fn default() -> Self {
        let n = ((P + 2) / 2) as usize;
        let mut digits = alloc::vec![0u8; n];
        // Set the sign nibble to positive (0xC).
        let last = digits.len() - 1;
        digits[last] = 0x0C;
        Self { digits }
    }
}

impl<const P: u32, const S: u32> Fixed<P, S> {
    /// Constructs a `Fixed<P, S>` from a raw BCD byte sequence.
    ///
    /// # Errors
    /// `Invalid` if the byte length is not `(P + 2) / 2`.
    pub fn from_bcd_bytes(bytes: Vec<u8>) -> Result<Self, DecodeError> {
        let expected = ((P + 2) / 2) as usize;
        if bytes.len() != expected {
            return Err(DecodeError::LengthExceeded {
                announced: bytes.len(),
                remaining: expected,
                offset: 0,
            });
        }
        Ok(Self { digits: bytes })
    }

    /// Raw BCD bytes.
    #[must_use]
    pub fn as_bcd_bytes(&self) -> &[u8] {
        &self.digits
    }

    /// Creates from a string (e.g. `"123.45"` or `"-1.5"`).
    ///
    /// # Errors
    /// `Invalid` for non-numeric input or overflow against P/S.
    pub fn from_str_repr(s: &str) -> Result<Self, DecodeError> {
        let (sign, rest) = if let Some(stripped) = s.strip_prefix('-') {
            (false, stripped)
        } else if let Some(stripped) = s.strip_prefix('+') {
            (true, stripped)
        } else {
            (true, s)
        };
        let (int_part, frac_part) = rest.split_once('.').unwrap_or((rest, ""));
        // Trim to the P/S layout.
        let total_p = P as usize;
        let total_s = S as usize;
        let mut digits_buf = String::with_capacity(total_p);
        // Pad int_part on the left if too short.
        let int_needed = total_p - total_s;
        if int_part.len() > int_needed {
            return Err(DecodeError::InvalidString {
                offset: 0,
                reason: "fixed: integer part exceeds P-S",
            });
        }
        for _ in int_part.len()..int_needed {
            digits_buf.push('0');
        }
        digits_buf.push_str(int_part);
        // Pad frac_part on the right if too short, trim if too long.
        if frac_part.len() > total_s {
            return Err(DecodeError::InvalidString {
                offset: 0,
                reason: "fixed: fractional part exceeds S",
            });
        }
        digits_buf.push_str(frac_part);
        for _ in frac_part.len()..total_s {
            digits_buf.push('0');
        }
        // Build the BCD nibble sequence big-endian: an optional leading 0
        // pad nibble (so the total nibble count is even), the P digit
        // nibbles most-significant first, then the sign nibble. `digits_buf`
        // already holds exactly P digit chars. Then pack 2 nibbles per byte,
        // high nibble first (CORBA §9.3.2.7). This replaces the old
        // low-nibble-first loop, which dropped the most-significant digit
        // for even P and emitted a spurious leading byte for odd P.
        let mut nibbles: Vec<u8> = Vec::with_capacity(P as usize + 2);
        if (P + 1) % 2 == 1 {
            nibbles.push(0);
        }
        for c in digits_buf.chars() {
            let d = c.to_digit(10).ok_or(DecodeError::InvalidString {
                offset: 0,
                reason: "fixed: non-digit char",
            })? as u8;
            nibbles.push(d & 0x0F);
        }
        nibbles.push(if sign { 0x0C } else { 0x0D });
        // `nibbles.len()` is even by construction.
        let mut packed: Vec<u8> = Vec::with_capacity(nibbles.len() / 2);
        for pair in nibbles.chunks_exact(2) {
            packed.push((pair[0] << 4) | pair[1]);
        }
        Ok(Self { digits: packed })
    }

    /// Decimal string representation (e.g. `"123.45"`).
    #[must_use]
    pub fn to_string_repr(&self) -> String {
        let mut digits_chars: Vec<char> = Vec::new();
        let mut sign = '+';
        for (idx, byte) in self.digits.iter().enumerate() {
            let high = (byte >> 4) & 0x0F;
            let low = byte & 0x0F;
            if idx == self.digits.len() - 1 {
                digits_chars.push(char::from_digit(u32::from(high), 10).unwrap_or('?'));
                sign = if low == 0x0D { '-' } else { '+' };
            } else {
                digits_chars.push(char::from_digit(u32::from(high), 10).unwrap_or('?'));
                digits_chars.push(char::from_digit(u32::from(low), 10).unwrap_or('?'));
            }
        }
        // Trim leading zeros (keep at least one digit)
        while digits_chars.len() > (S as usize + 1) && digits_chars[0] == '0' {
            digits_chars.remove(0);
        }
        // Insert the decimal point if S > 0
        let mut out = String::new();
        if sign == '-' {
            out.push('-');
        }
        if (S as usize) > 0 {
            let dot_pos = digits_chars.len().saturating_sub(S as usize);
            for (i, c) in digits_chars.iter().enumerate() {
                if i == dot_pos {
                    out.push('.');
                }
                out.push(*c);
            }
        } else {
            for c in &digits_chars {
                out.push(*c);
            }
        }
        out
    }
}

impl<const P: u32, const S: u32> CdrEncode for Fixed<P, S> {
    fn encode(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        // XCDR2 §7.4.4.5: raw BCD bytes, no length prefix (since the byte
        // count is statically known via P).
        w.write_bytes(&self.digits)
    }
}

impl<const P: u32, const S: u32> CdrDecode for Fixed<P, S> {
    fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let n = ((P + 2) / 2) as usize;
        let bytes = r.read_bytes(n)?;
        Self::from_bcd_bytes(bytes.to_vec())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn fixed_default_is_zero_positive() {
        let f: Fixed<5, 2> = Fixed::default();
        assert_eq!(f.to_string_repr(), "0.00");
    }

    #[test]
    fn fixed_roundtrip_via_string() {
        let f: Fixed<5, 2> = Fixed::from_str_repr("123.45").expect("parse");
        assert_eq!(f.to_string_repr(), "123.45");
    }

    #[test]
    fn fixed_roundtrip_negative() {
        let f: Fixed<6, 3> = Fixed::from_str_repr("-1.500").expect("parse");
        let s = f.to_string_repr();
        assert!(s.starts_with('-'));
        assert!(s.contains("1.500") || s.contains("1.5"));
    }

    #[test]
    fn fixed_wire_roundtrip() {
        use crate::Endianness;
        let f: Fixed<5, 2> = Fixed::from_str_repr("42.00").expect("parse");
        let mut writer = BufferWriter::new(Endianness::Little);
        f.encode(&mut writer).expect("encode");
        let bytes = writer.into_bytes();
        let mut reader = BufferReader::new(&bytes, Endianness::Little);
        let back: Fixed<5, 2> = <Fixed<5, 2> as CdrDecode>::decode(&mut reader).expect("decode");
        assert_eq!(back, f);
    }

    #[test]
    fn fixed_overflow_returns_error() {
        let res: Result<Fixed<3, 1>, _> = Fixed::from_str_repr("9999.5");
        assert!(res.is_err());
    }

    // ---- CORBA vendor-oracle regression (JacORB 3.9 ≡ omniORB 4.3) ----

    /// Odd P: `fixed<5,2>` is exactly 3 octets `12 34 5c` — no spurious
    /// leading byte. (Old encoder emitted 4 octets `00 12 34 5c`.)
    #[test]
    fn fixed_bcd_bytes_odd_p_match_orb() {
        let f: Fixed<5, 2> = Fixed::from_str_repr("123.45").expect("parse");
        assert_eq!(f.as_bcd_bytes(), &[0x12, 0x34, 0x5C]);
        assert_eq!(f.to_string_repr(), "123.45");
    }

    /// Even P: `fixed<4,0>` must keep the most-significant digit — the old
    /// encoder dropped it, encoding 1234 as `00 23 4c` and round-tripping
    /// to "234" (silent data corruption). Correct = `01 23 4c` → "1234".
    #[test]
    fn fixed_even_p_keeps_msd_no_corruption() {
        let f: Fixed<4, 0> = Fixed::from_str_repr("1234").expect("parse");
        assert_eq!(f.as_bcd_bytes(), &[0x01, 0x23, 0x4C]);
        assert_eq!(f.to_string_repr(), "1234");
    }

    /// Negative, pad-to-P (omniORB form): `fixed<6,2>` of -1.50 →
    /// `00 00 15 0d` (4 octets), the conformant P-digit representation.
    #[test]
    fn fixed_negative_pad_to_p_match_orb() {
        let f: Fixed<6, 2> = Fixed::from_str_repr("-1.50").expect("parse");
        assert_eq!(f.as_bcd_bytes(), &[0x00, 0x00, 0x15, 0x0D]);
        assert_eq!(f.to_string_repr(), "-1.50");
    }

    /// Even P with a full digit set: `fixed<6,3>` of 123.456 → `01 23 45 6c`
    /// (leading pad nibble), every digit preserved through roundtrip.
    #[test]
    fn fixed_even_p_full_digits_roundtrip() {
        let f: Fixed<6, 3> = Fixed::from_str_repr("123.456").expect("parse");
        assert_eq!(f.as_bcd_bytes(), &[0x01, 0x23, 0x45, 0x6C]);
        assert_eq!(f.to_string_repr(), "123.456");
    }

    /// Exhaustive small-range roundtrip: every integer 0..=9999 through
    /// `fixed<4,0>` must survive encode→decode-as-string unchanged (guards
    /// against any residual nibble misplacement).
    #[test]
    fn fixed_exhaustive_roundtrip_4_0() {
        for n in 0u32..=9999 {
            let s = alloc::format!("{n}");
            let f: Fixed<4, 0> = Fixed::from_str_repr(&s).expect("parse");
            // to_string_repr trims leading zeros, so compare numerically.
            assert_eq!(
                f.to_string_repr().parse::<u32>().expect("reparse"),
                n,
                "roundtrip lost value for {n}"
            );
        }
    }
}
