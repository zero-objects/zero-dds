// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL `fixed<P, S>` Decimal-Type (XCDR2 §7.4.4.5).
//!
//! Wire-Format: Packed Binary-Coded-Decimal (BCD).
//! - Digit-Count = P, Scale = S (Anzahl Stellen nach Komma).
//! - Bytes = `(P + 1) / 2 + 1`. Letztes Halb-Byte ist Sign-Nibble:
//!   `0xC` = positiv, `0xD` = negativ.
//! - Digits werden in Big-Endian-Reihenfolge, je 2 pro Byte gepackt.

#![allow(clippy::manual_div_ceil, clippy::while_let_on_iterator)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::buffer::{BufferReader, BufferWriter};
use crate::encode::{CdrDecode, CdrEncode};
use crate::error::{DecodeError, EncodeError};

/// IDL-`fixed<P, S>`-Decimal mit `P` Gesamt-Stellen und `S` Stellen
/// nach dem Komma.
///
/// Storage als Packed-BCD-Bytes (XCDR2 §7.4.4.5). Pure-Rust ohne
/// externe Crate-Dep. Bewusste Architektur-Wahl: dieser Type bietet
/// **Roundtrip + String-Konversion**, keine Decimal-Arithmetik
/// (Add/Mul). End-User die Decimal-Arithmetik brauchen, kombinieren
/// das mit `rust_decimal` o.ae. via `From`-Impl.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixed<const P: u32, const S: u32> {
    /// Packed-BCD-Storage. Laenge `(P + 1) / 2 + 1` Byte (letztes
    /// Nibble = Sign).
    digits: Vec<u8>,
}

impl<const P: u32, const S: u32> Default for Fixed<P, S> {
    fn default() -> Self {
        let n = ((P + 1) / 2 + 1) as usize;
        let mut digits = alloc::vec![0u8; n];
        // Sign-Nibble auf positiv (0xC) setzen.
        let last = digits.len() - 1;
        digits[last] = 0x0C;
        Self { digits }
    }
}

impl<const P: u32, const S: u32> Fixed<P, S> {
    /// Konstruiert eine `Fixed<P, S>` aus einer Roh-BCD-Bytes-Sequenz.
    ///
    /// # Errors
    /// `Invalid` wenn die Bytes-Laenge nicht `(P + 1) / 2 + 1` ist.
    pub fn from_bcd_bytes(bytes: Vec<u8>) -> Result<Self, DecodeError> {
        let expected = ((P + 1) / 2 + 1) as usize;
        if bytes.len() != expected {
            return Err(DecodeError::LengthExceeded {
                announced: bytes.len(),
                remaining: expected,
                offset: 0,
            });
        }
        Ok(Self { digits: bytes })
    }

    /// Roh-BCD-Bytes.
    #[must_use]
    pub fn as_bcd_bytes(&self) -> &[u8] {
        &self.digits
    }

    /// Erzeugt aus String (z.B. `"123.45"` oder `"-1.5"`).
    ///
    /// # Errors
    /// `Invalid` bei nicht-numerischen Eingaben oder Overflow gegen P/S.
    pub fn from_str_repr(s: &str) -> Result<Self, DecodeError> {
        let (sign, rest) = if let Some(stripped) = s.strip_prefix('-') {
            (false, stripped)
        } else if let Some(stripped) = s.strip_prefix('+') {
            (true, stripped)
        } else {
            (true, s)
        };
        let (int_part, frac_part) = rest.split_once('.').unwrap_or((rest, ""));
        // Trimme zu P/S-Layout.
        let total_p = P as usize;
        let total_s = S as usize;
        let mut digits_buf = String::with_capacity(total_p);
        // Pad int_part links wenn zu kurz.
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
        // Pad frac_part rechts wenn zu kurz, trim wenn zu lang.
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
        // Parse digits + pack BCD
        let mut packed: Vec<u8> = Vec::with_capacity(((P + 1) / 2 + 1) as usize);
        let mut chars = digits_buf.chars().rev().peekable();
        let sign_nibble: u8 = if sign { 0x0C } else { 0x0D };
        let mut current = sign_nibble;
        let mut have_low = true;
        while let Some(c) = chars.next() {
            let d = c.to_digit(10).ok_or(DecodeError::InvalidString {
                offset: 0,
                reason: "fixed: non-digit char",
            })? as u8;
            if have_low {
                current |= (d & 0x0F) << 4;
                packed.push(current);
                current = 0;
                have_low = false;
            } else {
                current |= d & 0x0F;
                have_low = true;
            }
        }
        if !have_low {
            packed.push(current);
        }
        packed.reverse();
        // Sicherstellen dass Laenge stimmt
        let expected = ((P + 1) / 2 + 1) as usize;
        while packed.len() < expected {
            packed.insert(0, 0);
        }
        Ok(Self { digits: packed })
    }

    /// Decimal-String-Repraesentation (z.B. `"123.45"`).
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
        // Trim leading zeros (mind. eine Ziffer behalten)
        while digits_chars.len() > (S as usize + 1) && digits_chars[0] == '0' {
            digits_chars.remove(0);
        }
        // Komma einfuegen falls S > 0
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
        // XCDR2 §7.4.4.5: BCD-Bytes raw, kein length-prefix (weil Bytes-
        // Anzahl statisch via P bekannt ist).
        w.write_bytes(&self.digits)
    }
}

impl<const P: u32, const S: u32> CdrDecode for Fixed<P, S> {
    fn decode(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let n = ((P + 1) / 2 + 1) as usize;
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
}
