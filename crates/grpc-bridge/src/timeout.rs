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

    /// Converts a wire char.
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

/// Timeout parser error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeoutError {
    /// Empty Header.
    Empty,
    /// Spec — TimeoutValue MUST positive integer with at most 8
    /// digits.
    ValueTooLong,
    /// Non-digit character before the unit.
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

/// Spec §"Timeout" — encodes the timeout value+unit as the
/// `grpc-timeout` header value.
///
/// Returns a formatted string like `"100m"` (100 milliseconds) or
/// `"30S"` (30 seconds).
///
/// # Errors
/// `ValueTooLong` if `value > 99_999_999` (9+ digits).
pub fn encode_timeout(value: u32, unit: TimeoutUnit) -> Result<String, TimeoutError> {
    if value > 99_999_999 {
        return Err(TimeoutError::ValueTooLong);
    }
    let mut s = alloc::format!("{value}");
    s.push(unit.to_char());
    Ok(s)
}

/// Spec §"Timeout" — decodes the `grpc-timeout` header value.
///
/// # Errors
/// See [`TimeoutError`].
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

    /// §"Requests": TimeoutUnit → Hour "H" / Minute "M" / Second "S" /
    /// Millisecond "m" / Microsecond "u" / Nanosecond "n", both ways.
    #[test]
    fn grpc_requests_the_timeout_units_are_h_m_s_m_u_n() {
        for (u, c) in [
            (TimeoutUnit::Hour, 'H'),
            (TimeoutUnit::Minute, 'M'),
            (TimeoutUnit::Second, 'S'),
            (TimeoutUnit::Millisecond, 'm'),
            (TimeoutUnit::Microsecond, 'u'),
            (TimeoutUnit::Nanosecond, 'n'),
        ] {
            assert_eq!(u.to_char(), c);
            assert_eq!(TimeoutUnit::from_char(c), Some(u));
        }
    }

    #[test]
    fn grpc_requests_encodes_30_seconds() {
        // Spec §"Timeout" example.
        assert_eq!(encode_timeout(30, TimeoutUnit::Second).expect("ok"), "30S");
    }

    #[test]
    fn grpc_requests_encodes_500_milliseconds() {
        assert_eq!(
            encode_timeout(500, TimeoutUnit::Millisecond).expect("ok"),
            "500m"
        );
    }

    #[test]
    fn grpc_requests_rejects_value_above_8_digits_on_encode() {
        // Spec §"TimeoutValue" — at most 8 digits.
        assert_eq!(
            encode_timeout(100_000_000, TimeoutUnit::Second),
            Err(TimeoutError::ValueTooLong)
        );
    }

    /// §"Requests": Timeout → TimeoutValue TimeoutUnit, the value a
    /// positive integer of at most 8 digits; the header values are read.
    #[test]
    fn grpc_requests_a_timeout_header_is_read_as_value_and_unit() {
        for (text, value, unit) in [
            ("1H", 1, TimeoutUnit::Hour),
            ("30S", 30, TimeoutUnit::Second),
            ("500m", 500, TimeoutUnit::Millisecond),
            ("99999999n", 99_999_999, TimeoutUnit::Nanosecond),
            ("7u", 7, TimeoutUnit::Microsecond),
            ("2M", 2, TimeoutUnit::Minute),
        ] {
            assert_eq!(decode_timeout(text).expect("decode"), (value, unit));
            assert_eq!(encode_timeout(value, unit).expect("encode"), text);
        }
    }

    #[test]
    fn grpc_requests_decode_rejects_empty() {
        assert_eq!(decode_timeout(""), Err(TimeoutError::Empty));
    }

    #[test]
    fn grpc_requests_decode_rejects_unknown_unit() {
        assert_eq!(decode_timeout("100x"), Err(TimeoutError::InvalidUnit('x')));
    }

    #[test]
    fn grpc_requests_decode_rejects_non_digit_value() {
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
    fn grpc_requests_decode_rejects_only_unit_no_value() {
        assert_eq!(decode_timeout("S"), Err(TimeoutError::ValueTooLong));
    }
}
