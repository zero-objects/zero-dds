// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Variable Byte Integer — MQTT v5.0 §1.5.5.

use alloc::vec::Vec;

/// Maximum VBI value: 268 435 455 (Spec §1.5.5).
pub const MAX_VBI: u32 = 0x0FFF_FFFF;

/// Spec §1.5.5 — encodes a VBI with 1..=4 bytes (7-bit groups +
/// continuation flag in the MSB).
///
/// # Errors
/// Returns `None` if `value > MAX_VBI`.
#[must_use]
pub fn encode_vbi(value: u32) -> Option<Vec<u8>> {
    if value > MAX_VBI {
        return None;
    }
    let mut out = Vec::with_capacity(4);
    let mut x = value;
    loop {
        #[allow(clippy::cast_possible_truncation)]
        let mut byte = (x & 0x7F) as u8;
        x >>= 7;
        if x > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if x == 0 {
            break;
        }
    }
    Some(out)
}

/// Spec §1.5.5 — decodes a VBI from `bytes[offset]`. Returns
/// `Ok((value, consumed_bytes))`.
///
/// # Errors
/// * `Err(VbiError::Truncated)` if fewer than the required bytes.
/// * `Err(VbiError::Malformed)` if 5+ continuation bytes (the spec
///   requires max 4).
pub fn decode_vbi(bytes: &[u8]) -> Result<(u32, usize), VbiError> {
    let mut value: u32 = 0;
    let mut multiplier: u32 = 1;
    for i in 0..4 {
        if i >= bytes.len() {
            return Err(VbiError::Truncated);
        }
        let byte = bytes[i];
        value = value
            .checked_add(u32::from(byte & 0x7F) * multiplier)
            .ok_or(VbiError::Malformed)?;
        if (byte & 0x80) == 0 {
            return Ok((value, i + 1));
        }
        multiplier = multiplier.checked_mul(128).ok_or(VbiError::Malformed)?;
    }
    // 5th byte: continuation bit set → spec violation.
    Err(VbiError::Malformed)
}

/// Returns the number of bytes `encode_vbi(value)` would produce.
#[must_use]
pub const fn vbi_size(value: u32) -> Option<usize> {
    if value > MAX_VBI {
        return None;
    }
    let n = if value < 128 {
        1
    } else if value < 16_384 {
        2
    } else if value < 2_097_152 {
        3
    } else {
        4
    };
    Some(n)
}

/// VBI codec error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VbiError {
    /// Input bytes not large enough.
    Truncated,
    /// VBI has 5+ continuation bytes or overflow (Spec §1.5.5).
    Malformed,
}

impl core::fmt::Display for VbiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Truncated => "VBI truncated",
            Self::Malformed => "VBI malformed (>4 bytes or overflow)",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for VbiError {}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn encode_zero_yields_single_zero_byte() {
        // Spec §1.5.5 Table 1.5-3 — 0 = 0x00.
        assert_eq!(encode_vbi(0).expect("ok"), alloc::vec![0]);
    }

    #[test]
    fn encode_127_yields_single_7f_byte() {
        // Spec — boundary 1-byte / 2-byte.
        assert_eq!(encode_vbi(127).expect("ok"), alloc::vec![0x7F]);
    }

    #[test]
    fn encode_128_yields_two_bytes_with_continuation() {
        // Spec §1.5.5 Table — 128 = 0x80 0x01.
        assert_eq!(encode_vbi(128).expect("ok"), alloc::vec![0x80, 0x01]);
    }

    #[test]
    fn encode_16383_is_two_byte_max() {
        // Spec — 16,383 = 0xFF 0x7F.
        assert_eq!(encode_vbi(16_383).expect("ok"), alloc::vec![0xFF, 0x7F]);
    }

    #[test]
    fn encode_16384_is_three_byte_min() {
        // Spec — 16,384 = 0x80 0x80 0x01.
        assert_eq!(
            encode_vbi(16_384).expect("ok"),
            alloc::vec![0x80, 0x80, 0x01]
        );
    }

    #[test]
    fn encode_max_vbi_is_four_byte_max() {
        // Spec §1.5.5 Table — 268,435,455 = 0xFF 0xFF 0xFF 0x7F.
        assert_eq!(
            encode_vbi(MAX_VBI).expect("ok"),
            alloc::vec![0xFF, 0xFF, 0xFF, 0x7F]
        );
    }

    #[test]
    fn encode_above_max_returns_none() {
        // Spec §1.5.5 — values > 268,435,455 not allowed.
        assert!(encode_vbi(MAX_VBI + 1).is_none());
    }

    #[test]
    fn decode_round_trips_all_boundary_values() {
        for v in [
            0u32, 1, 127, 128, 16_383, 16_384, 2_097_151, 2_097_152, MAX_VBI,
        ] {
            let bytes = encode_vbi(v).expect("encode");
            let (decoded, consumed) = decode_vbi(&bytes).expect("decode");
            assert_eq!(decoded, v);
            assert_eq!(consumed, bytes.len());
        }
    }

    #[test]
    fn decode_rejects_truncated_input() {
        // 0x80 = continuation set but no following byte.
        assert_eq!(decode_vbi(&[0x80]), Err(VbiError::Truncated));
        assert_eq!(decode_vbi(&[]), Err(VbiError::Truncated));
    }

    #[test]
    fn decode_rejects_5_byte_vbi() {
        // Spec §1.5.5 — max 4 bytes; 5-byte form malformed.
        assert_eq!(
            decode_vbi(&[0x80, 0x80, 0x80, 0x80, 0x01]),
            Err(VbiError::Malformed)
        );
    }

    #[test]
    fn vbi_size_matches_encoded_byte_count() {
        for v in [
            0u32, 1, 127, 128, 16_383, 16_384, 2_097_151, 2_097_152, MAX_VBI,
        ] {
            let bytes = encode_vbi(v).expect("encode");
            assert_eq!(vbi_size(v), Some(bytes.len()));
        }
    }
}
