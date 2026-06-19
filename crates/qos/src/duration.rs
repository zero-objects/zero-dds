// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS `Duration_t` (DDSI-RTPS §9.3.2) — i32 seconds + u32 fraction.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// DDS Duration (signed seconds + unsigned 2^-32-fractions).
///
/// The spec defines two special values:
/// - `DURATION_INFINITE`: `{ seconds: 0x7FFFFFFF, fraction: 0xFFFFFFFF }`.
/// - `DURATION_ZERO`: `{ seconds: 0, fraction: 0 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duration {
    /// Sekundenanteil (signed 32 bit).
    pub seconds: i32,
    /// Bruchteil-Anteil (2^-32-Sekunden).
    pub fraction: u32,
}

impl Duration {
    /// Spec §9.3.2: `DURATION_INFINITE`.
    pub const INFINITE: Self = Self {
        seconds: i32::MAX,
        fraction: u32::MAX,
    };

    /// Spec §9.3.2: `DURATION_ZERO`.
    pub const ZERO: Self = Self {
        seconds: 0,
        fraction: 0,
    };

    /// Creates a `Duration` from a number of seconds.
    #[must_use]
    pub const fn from_secs(seconds: i32) -> Self {
        Self {
            seconds,
            fraction: 0,
        }
    }

    /// Creates a `Duration` from a number of milliseconds.
    ///
    /// Time is represented as `{ seconds: i32, fraction: u32 (2^-32 s) }`
    /// — `fraction` is **unsigned**. Negative durations describe the time
    /// via negative `seconds` + positive `fraction`, so that
    /// `(seconds + fraction*2^-32)` runs continuously across 0. With
    /// `div_euclid/rem_euclid` the remainder always stays `[0, 1000)`.
    ///
    /// Examples:
    /// - `from_millis(1500)` = `{1, 2^31}` (1.5 s).
    /// - `from_millis(-500)` = `{-1, 2^31}` (= -1 + 0.5 = -0.5 s).
    /// - `from_millis(-1500)` = `{-2, 2^31}` (= -2 + 0.5 = -1.5 s).
    #[must_use]
    pub const fn from_millis(ms: i32) -> Self {
        let seconds = ms.div_euclid(1000);
        let remainder_ms = ms.rem_euclid(1000) as u32; // in [0, 1000)
        let fraction = ((remainder_ms as u64 * (1u64 << 32)) / 1000) as u32;
        Self { seconds, fraction }
    }

    /// `true` if `self == INFINITE`.
    #[must_use]
    pub const fn is_infinite(self) -> bool {
        self.seconds == i32::MAX && self.fraction == u32::MAX
    }

    /// Converts into nanoseconds. INFINITE and negative durations
    /// return `u128::MAX` or saturate to `0` (the caller treats
    /// `u128::MAX` as "never expires").
    #[must_use]
    pub const fn to_nanos(self) -> u128 {
        if self.is_infinite() {
            return u128::MAX;
        }
        if self.seconds < 0 {
            return 0;
        }
        let secs = self.seconds as u128;
        // fraction is in 2^-32 seconds: nanos = fraction * 1e9 / 2^32.
        let frac_nanos = (self.fraction as u128 * 1_000_000_000) >> 32;
        secs * 1_000_000_000 + frac_nanos
    }

    /// `true` if `self == ZERO`.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.seconds == 0 && self.fraction == 0
    }

    /// Wire-Encoding: `{ i32 seconds; u32 fraction }` (8 byte).
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.seconds as u32)?;
        w.write_u32(self.fraction)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let seconds = r.read_u32()? as i32;
        let fraction = r.read_u32()?;
        Ok(Self { seconds, fraction })
    }

    /// 8-byte array (LE) — useful for in-place copies in PL_CDR values.
    #[must_use]
    pub fn to_bytes_le(self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[..4].copy_from_slice(&self.seconds.to_le_bytes());
        out[4..].copy_from_slice(&self.fraction.to_le_bytes());
        out
    }

    /// 8-byte array (BE) — for PL_CDR_BE payloads like the handshake `c.pdata`.
    #[must_use]
    pub fn to_bytes_be(self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[..4].copy_from_slice(&self.seconds.to_be_bytes());
        out[4..].copy_from_slice(&self.fraction.to_be_bytes());
        out
    }

    /// Aus 8-byte-LE-Array.
    #[must_use]
    pub fn from_bytes_le(bytes: [u8; 8]) -> Self {
        let seconds = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let fraction = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        Self { seconds, fraction }
    }
}

impl Default for Duration {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn infinite_constant_matches_spec() {
        assert_eq!(Duration::INFINITE.seconds, i32::MAX);
        assert_eq!(Duration::INFINITE.fraction, u32::MAX);
        assert!(Duration::INFINITE.is_infinite());
    }

    #[test]
    fn zero_is_default_and_zero() {
        assert_eq!(Duration::default(), Duration::ZERO);
        assert!(Duration::ZERO.is_zero());
    }

    #[test]
    fn from_secs_has_zero_fraction() {
        let d = Duration::from_secs(42);
        assert_eq!(d.seconds, 42);
        assert_eq!(d.fraction, 0);
    }

    #[test]
    fn from_millis_splits_correctly() {
        let d = Duration::from_millis(1500);
        assert_eq!(d.seconds, 1);
        // 500ms -> 500/1000 * 2^32 = 2_147_483_648
        assert_eq!(d.fraction, 2_147_483_648);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let d = Duration {
            seconds: 7,
            fraction: 0xCAFE_BABE,
        };
        let mut w = BufferWriter::new(Endianness::Little);
        d.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 8);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let back = Duration::decode_from(&mut r).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn to_from_bytes_le_roundtrip() {
        let d = Duration {
            seconds: -3,
            fraction: 0xDEAD_BEEF,
        };
        let bytes = d.to_bytes_le();
        let back = Duration::from_bytes_le(bytes);
        assert_eq!(back, d);
    }

    #[test]
    fn ord_compares_seconds_then_fraction() {
        let a = Duration {
            seconds: 1,
            fraction: 0,
        };
        let b = Duration {
            seconds: 1,
            fraction: 1,
        };
        let c = Duration {
            seconds: 2,
            fraction: 0,
        };
        assert!(a < b);
        assert!(b < c);
    }
}
