// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS `Duration_t` (DDSI-RTPS §9.3.2) — i32 seconds + u32 fraction.

use zerodds_wire::{Reader, WireError, Writer};

/// DDS Duration (signed seconds + unsigned 2^-32-fractions).
///
/// The spec defines two special values:
/// - `DURATION_INFINITE`: `{ seconds: 0x7FFFFFFF, fraction: 0xFFFFFFFF }`.
/// - `DURATION_ZERO`: `{ seconds: 0, fraction: 0 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duration {
    /// Seconds component (signed 32 bit).
    pub seconds: i32,
    /// Fractional component (2^-32 seconds).
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
        // Rounded to the nearest 2^-32 s, so that the way back to
        // nanoseconds ([`Self::to_nanos`]) lands on the whole value.
        let fraction = ((remainder_ms as u64 * (1u64 << 32) + 500) / 1000) as u32;
        Self { seconds, fraction }
    }

    /// Creates a `Duration` from a number of microseconds. Sub-millisecond
    /// precision is preserved via the RTPS `fraction` field (latency budgets,
    /// deadlines, time-based filters routinely use µs). Same negative-value
    /// convention as [`Self::from_millis`].
    #[must_use]
    pub const fn from_micros(us: i32) -> Self {
        let seconds = us.div_euclid(1_000_000);
        let remainder_us = us.rem_euclid(1_000_000) as u32; // in [0, 1_000_000)
        let fraction = ((remainder_us as u64 * (1u64 << 32) + 500_000) / 1_000_000) as u32;
        Self { seconds, fraction }
    }

    /// `nanosec` (below one second) as 2^-32 s, rounded to the nearest.
    ///
    /// Rounded both ways, every whole nanosecond survives the way there
    /// and back ([`Self::to_sec_nanosec`]): the resolution is 0.23 ns.
    /// Truncated both ways, 300 ms came back as 299 999 999 ns.
    #[must_use]
    pub const fn fraction_of_nanosec(nanosec: u32) -> u32 {
        // nanosec < 1e9 keeps the quotient below 2^32.
        (((nanosec % 1_000_000_000) as u64 * (1u64 << 32) + 500_000_000) / 1_000_000_000) as u32
    }

    /// From seconds and nanoseconds; a `nanosec` of a second or more
    /// carries into the seconds.
    #[must_use]
    pub const fn from_sec_nanosec(seconds: i32, nanosec: u32) -> Self {
        Self {
            seconds: seconds.saturating_add((nanosec / 1_000_000_000) as i32),
            fraction: Self::fraction_of_nanosec(nanosec),
        }
    }

    /// Seconds and nanoseconds, the fraction rounded to the nearest
    /// nanosecond; a fraction that rounds up to a whole second carries.
    /// INFINITE is the caller's to recognise first.
    #[must_use]
    pub const fn to_sec_nanosec(self) -> (i32, u32) {
        let nanos = ((self.fraction as u64) * 1_000_000_000 + (1u64 << 31)) >> 32;
        if nanos >= 1_000_000_000 {
            (
                self.seconds.saturating_add(1),
                (nanos - 1_000_000_000) as u32,
            )
        } else {
            (self.seconds, nanos as u32)
        }
    }

    /// Whole milliseconds, rounded to the nearest; `u128::MAX` for
    /// INFINITE, `0` for a negative duration (as [`Self::to_nanos`]).
    #[must_use]
    pub const fn to_millis(self) -> u128 {
        match self.to_nanos() {
            u128::MAX => u128::MAX,
            nanos => (nanos + 500_000) / 1_000_000,
        }
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
        // fraction is in 2^-32 seconds: nanos = fraction * 1e9 / 2^32,
        // rounded to the nearest (see [`Self::fraction_of_nanosec`]).
        let frac_nanos = (self.fraction as u128 * 1_000_000_000 + (1u128 << 31)) >> 32;
        secs * 1_000_000_000 + frac_nanos
    }

    /// `true` if `self == ZERO`.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.seconds == 0 && self.fraction == 0
    }

    /// Wire-Encoding: `{ i32 seconds; u32 fraction }` (8 byte).
    pub fn encode_into(self, w: &mut Writer) {
        w.write_u32(self.seconds as u32);
        w.write_u32(self.fraction);
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
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

    /// From 8-byte-LE array.
    #[must_use]
    pub fn from_bytes_le(bytes: [u8; 8]) -> Self {
        let seconds = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let fraction = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        Self { seconds, fraction }
    }

    /// From 8-byte-BE array — the value of a PL_CDR_BE parameter.
    #[must_use]
    pub fn from_bytes_be(bytes: [u8; 8]) -> Self {
        let seconds = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let fraction = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        Self { seconds, fraction }
    }

    /// From the 8 octets of a parameter value in the list's byte order.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 8], little_endian: bool) -> Self {
        if little_endian {
            Self::from_bytes_le(bytes)
        } else {
            Self::from_bytes_be(bytes)
        }
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
    use zerodds_wire::Endianness;

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
    fn from_micros_keeps_sub_millisecond_precision() {
        // 1.5 s expressed in µs.
        let d = Duration::from_micros(1_500_000);
        assert_eq!(d.seconds, 1);
        assert_eq!(d.fraction, 2_147_483_648);
        // 500 µs — below millisecond granularity, must not collapse to zero.
        let half_ms = Duration::from_micros(500);
        assert_eq!(half_ms.seconds, 0);
        assert!(half_ms.fraction > 0);
        // 500 µs == 0.5 ms: 2^32 / 2000, rounded to the nearest.
        assert_eq!(
            half_ms.fraction,
            ((500u64 * (1u64 << 32) + 500_000) / 1_000_000) as u32
        );
    }

    /// Every whole nanosecond survives the way to 2^-32 s and back; it
    /// lost one before, 300 ms came back as 299 999 999 ns.
    #[test]
    fn a_whole_nanosecond_survives_the_way_there_and_back() {
        for nanosec in [
            0,
            1,
            2,
            3,
            299_999_999,
            300_000_000,
            999_999_998,
            999_999_999,
        ] {
            let d = Duration::from_sec_nanosec(5, nanosec);
            assert_eq!(d.to_sec_nanosec(), (5, nanosec), "{nanosec}");
        }
        for nanosec in (0..1_000_000_000u32).step_by(7_919) {
            assert_eq!(
                Duration::from_sec_nanosec(0, nanosec).to_sec_nanosec(),
                (0, nanosec)
            );
        }
        assert_eq!(Duration::from_millis(300).to_nanos(), 300_000_000);
        assert_eq!(Duration::from_micros(1).to_nanos(), 1_000);
        assert_eq!(Duration::from_millis(300).to_millis(), 300);
        assert_eq!(
            Duration::from_sec_nanosec(1, 1_500_000_000),
            Duration::from_millis(2500)
        );
    }

    #[test]
    fn encode_decode_roundtrip() {
        let d = Duration {
            seconds: 7,
            fraction: 0xCAFE_BABE,
        };
        let mut w = Writer::new(Endianness::Little);
        d.encode_into(&mut w);
        let bytes = w.finish();
        assert_eq!(bytes.len(), 8);
        let mut r = Reader::new(&bytes, Endianness::Little);
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
