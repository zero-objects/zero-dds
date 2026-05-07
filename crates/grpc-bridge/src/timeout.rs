// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! gRPC `grpc-timeout` Header — Spec §"Timeout".

use alloc::string::String;
use core::fmt;

/// Spec §"TimeoutUnit".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeoutUnit {
    /// `H` — Hour.
    Hour,
    /// `M` — Minute.
    Minute,
    /// `S` — Second.
    Second,
    /// `m` — Millisecond.
    Millisecond,
    /// `u` — Microsecond.
    Microsecond,
    /// `n` — Nanosecond.
    Nanosecond,
}

impl TimeoutUnit {
    /// Wire-Char.
    #[must_use]
    pub const fn to_char(self) -> char {
        match self {
            Self::Hour => 'H',
            Self::Minute => 'M',
            Self::Second => 'S',
            Self::Millisecond => 'm',
            Self::Microsecond => 'u',
            Self::Nanosecond => 'n',
        }
    }

    /// Konvertiert Wire-Char.
    #[must_use]
    pub const fn from_char(c: char) -> Option<Self> {
        match c {
            'H' => Some(Self::Hour),
            'M' => Some(Self::Minute),
            'S' => Some(Self::Second),
            'm' => Some(Self::Millisecond),
            'u' => Some(Self::Microsecond),
            'n' => Some(Self::Nanosecond),
            _ => None,
        }
    }
}

/// Timeout-Parser-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeoutError {
    /// Empty Header.
    Empty,
    /// Spec — TimeoutValue MUST positive integer with at most 8
    /// digits.
    ValueTooLong,
    /// Non-digit Character vor Unit.
    InvalidValue,
    /// Spec — Unit MUST be one of H/M/S/m/u/n.
    InvalidUnit(char),
}

impl fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty timeout header"),
            Self::ValueTooLong => f.write_str("timeout value > 8 digits"),
            Self::InvalidValue => f.write_str("non-digit in timeout value"),
            Self::InvalidUnit(c) => write!(f, "invalid timeout unit `{c}`"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TimeoutError {}

/// Spec §"Timeout" — encodes timeout value+unit als
/// `grpc-timeout`-Header-Wert.
///
/// Liefert formatted String wie `"100m"` (100 Millisekunden) oder
/// `"30S"` (30 Sekunden).
///
/// # Errors
/// `ValueTooLong` wenn `value > 99_999_999` (9+ digits).
pub fn encode_timeout(value: u32, unit: TimeoutUnit) -> Result<String, TimeoutError> {
    if value > 99_999_999 {
        return Err(TimeoutError::ValueTooLong);
    }
    let mut s = alloc::format!("{value}");
    s.push(unit.to_char());
    Ok(s)
}

/// Spec §"Timeout" — decodes `grpc-timeout`-Header-Wert.
///
/// # Errors
/// Siehe [`TimeoutError`].
pub fn decode_timeout(header: &str) -> Result<(u32, TimeoutUnit), TimeoutError> {
    if header.is_empty() {
        return Err(TimeoutError::Empty);
    }
    let last_char = header.chars().next_back().ok_or(TimeoutError::Empty)?;
    let unit = TimeoutUnit::from_char(last_char).ok_or(TimeoutError::InvalidUnit(last_char))?;
    let value_str = &header[..header.len() - last_char.len_utf8()];
    if value_str.is_empty() || value_str.len() > 8 {
        return Err(TimeoutError::ValueTooLong);
    }
    if !value_str.bytes().all(|b| b.is_ascii_digit()) {
        return Err(TimeoutError::InvalidValue);
    }
    let value: u32 = value_str.parse().map_err(|_| TimeoutError::InvalidValue)?;
    Ok((value, unit))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn timeout_unit_round_trip_for_all() {
        for u in [
            TimeoutUnit::Hour,
            TimeoutUnit::Minute,
            TimeoutUnit::Second,
            TimeoutUnit::Millisecond,
            TimeoutUnit::Microsecond,
            TimeoutUnit::Nanosecond,
        ] {
            assert_eq!(TimeoutUnit::from_char(u.to_char()), Some(u));
        }
    }

    #[test]
    fn well_known_unit_chars_match_spec() {
        // Spec §"TimeoutUnit".
        assert_eq!(TimeoutUnit::Hour.to_char(), 'H');
        assert_eq!(TimeoutUnit::Minute.to_char(), 'M');
        assert_eq!(TimeoutUnit::Second.to_char(), 'S');
        assert_eq!(TimeoutUnit::Millisecond.to_char(), 'm');
        assert_eq!(TimeoutUnit::Microsecond.to_char(), 'u');
        assert_eq!(TimeoutUnit::Nanosecond.to_char(), 'n');
    }

    #[test]
    fn encodes_30_seconds() {
        // Spec §"Timeout" Beispiel.
        assert_eq!(encode_timeout(30, TimeoutUnit::Second).expect("ok"), "30S");
    }

    #[test]
    fn encodes_500_milliseconds() {
        assert_eq!(
            encode_timeout(500, TimeoutUnit::Millisecond).expect("ok"),
            "500m"
        );
    }

    #[test]
    fn rejects_value_above_8_digits_on_encode() {
        // Spec §"TimeoutValue" — at most 8 digits.
        assert_eq!(
            encode_timeout(100_000_000, TimeoutUnit::Second),
            Err(TimeoutError::ValueTooLong)
        );
    }

    #[test]
    fn round_trip_decode_encode() {
        for v in [1u32, 30, 500, 99_999_999] {
            for u in [
                TimeoutUnit::Hour,
                TimeoutUnit::Minute,
                TimeoutUnit::Second,
                TimeoutUnit::Millisecond,
                TimeoutUnit::Microsecond,
                TimeoutUnit::Nanosecond,
            ] {
                let s = encode_timeout(v, u).expect("encode");
                let (dv, du) = decode_timeout(&s).expect("decode");
                assert_eq!(dv, v);
                assert_eq!(du, u);
            }
        }
    }

    #[test]
    fn decode_rejects_empty() {
        assert_eq!(decode_timeout(""), Err(TimeoutError::Empty));
    }

    #[test]
    fn decode_rejects_unknown_unit() {
        assert_eq!(decode_timeout("100x"), Err(TimeoutError::InvalidUnit('x')));
    }

    #[test]
    fn decode_rejects_non_digit_value() {
        assert_eq!(decode_timeout("abcS"), Err(TimeoutError::InvalidValue));
    }

    #[test]
    fn decode_rejects_value_above_8_digits() {
        // Spec — at most 8 digits.
        assert_eq!(
            decode_timeout("123456789S"),
            Err(TimeoutError::ValueTooLong)
        );
    }

    #[test]
    fn decode_rejects_only_unit_no_value() {
        assert_eq!(decode_timeout("S"), Err(TimeoutError::ValueTooLong));
    }
}
