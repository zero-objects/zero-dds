// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Variable-Length-Integer-Coding — RFC 7541 §5.1.
//!
//! Encoded auf einer N-Bit-Praefix-Position mit Continuation-Marker
//! im MSB der Folge-Bytes.

use alloc::vec::Vec;

/// Integer-Coding-Fehler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerError {
    /// Eingabe-Buffer zu kurz.
    Truncated,
    /// Ueber 7 Continuation-Bytes — RFC 7541 erlaubt das nicht
    /// (Spec §5.1, Implementations MAY enforce a limit).
    TooLarge,
}

impl core::fmt::Display for IntegerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => f.write_str("integer truncated"),
            Self::TooLarge => f.write_str("integer too large"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for IntegerError {}

/// Encode `value` mit `prefix_bits` Bits Praefix-Slot. Spec §5.1.
///
/// `prefix_bits` muss in 1..=8 liegen. Das erste Output-Byte enthaelt
/// die `(8 - prefix_bits)` Most-Significant-Bits aus
/// `out_byte_prefix_bits` (Caller liefert die in Bits 7..prefix_bits).
#[must_use]
pub fn encode_integer(value: u64, prefix_bits: u8, out_byte_prefix_bits: u8) -> Vec<u8> {
    let mask = (1u64 << prefix_bits) - 1;
    let mut out = Vec::new();
    if value < mask {
        out.push(out_byte_prefix_bits | (value as u8));
        return out;
    }
    out.push(out_byte_prefix_bits | mask as u8);
    let mut v = value - mask;
    while v >= 128 {
        out.push((v & 0x7f) as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
    out
}

/// Decode einen Integer aus einem Byte-Slice. Spec §5.1.
///
/// Liefert (decoded value, bytes consumed).
///
/// # Errors
/// `Truncated` / `TooLarge`.
pub fn decode_integer(input: &[u8], prefix_bits: u8) -> Result<(u64, usize), IntegerError> {
    if input.is_empty() {
        return Err(IntegerError::Truncated);
    }
    let mask = (1u64 << prefix_bits) - 1;
    let first = u64::from(input[0]) & mask;
    if first < mask {
        return Ok((first, 1));
    }
    let mut value = mask;
    let mut shift: u32 = 0;
    let mut idx = 1;
    while idx < input.len() {
        let b = input[idx];
        idx += 1;
        if shift >= 56 {
            return Err(IntegerError::TooLarge);
        }
        value = value
            .checked_add(u64::from(b & 0x7f) << shift)
            .ok_or(IntegerError::TooLarge)?;
        shift += 7;
        if (b & 0x80) == 0 {
            return Ok((value, idx));
        }
    }
    Err(IntegerError::Truncated)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn rfc7541_c1_1_encoded_10_in_5bit_prefix() {
        // Spec §C.1.1 — value 10 in 5-bit prefix → 0x0a (one byte)
        let buf = encode_integer(10, 5, 0);
        assert_eq!(buf, alloc::vec![0x0a]);
        assert_eq!(decode_integer(&buf, 5).unwrap(), (10, 1));
    }

    #[test]
    fn rfc7541_c1_2_encoded_1337_in_5bit_prefix() {
        // Spec §C.1.2 — value 1337 in 5-bit prefix → 0x1f, 0x9a, 0x0a
        let buf = encode_integer(1337, 5, 0);
        assert_eq!(buf, alloc::vec![0x1f, 0x9a, 0x0a]);
        assert_eq!(decode_integer(&buf, 5).unwrap(), (1337, 3));
    }

    #[test]
    fn rfc7541_c1_3_encoded_42_in_8bit_prefix() {
        // Spec §C.1.3 — value 42 in 8-bit prefix → 0x2a (one byte)
        let buf = encode_integer(42, 8, 0);
        assert_eq!(buf, alloc::vec![0x2a]);
        assert_eq!(decode_integer(&buf, 8).unwrap(), (42, 1));
    }

    #[test]
    fn round_trip_small_values() {
        for v in 0..256u64 {
            let buf = encode_integer(v, 7, 0);
            let (decoded, _) = decode_integer(&buf, 7).unwrap();
            assert_eq!(decoded, v);
        }
    }

    #[test]
    fn round_trip_large_values() {
        for v in [u64::from(u32::MAX), 1_000_000, 1u64 << 32] {
            let buf = encode_integer(v, 5, 0);
            let (decoded, _) = decode_integer(&buf, 5).unwrap();
            assert_eq!(decoded, v);
        }
    }

    #[test]
    fn truncated_continuation_rejected() {
        // 5-bit prefix all-ones triggers continuation, but no 2nd byte.
        let buf = alloc::vec![0x1f];
        assert_eq!(decode_integer(&buf, 5), Err(IntegerError::Truncated));
    }

    #[test]
    fn empty_input_truncated() {
        assert_eq!(decode_integer(&[], 5), Err(IntegerError::Truncated));
    }

    #[test]
    fn prefix_bits_left_other_bits_alone() {
        let buf = encode_integer(5, 5, 0xc0); // top 3 bits set
        assert_eq!(buf[0] & 0xe0, 0xc0);
        assert_eq!(buf[0] & 0x1f, 5);
    }
}
