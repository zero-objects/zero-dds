// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL-PSM data type mapping per DDS-XML 1.0 §7.2.
//!
//! Element value parsers for boolean, hex/decimal long, Duration_t,
//! and the symbol constants `LENGTH_UNLIMITED`, `DURATION_INFINITE_SEC`
//! and `DURATION_INFINITE_NSEC` from §7.2.2.
//!
//! All helpers are pure string -> typed-value conversions without
//! allocation, where possible.

use crate::errors::XmlError;
use alloc::format;
use alloc::string::ToString;

/// Spec §7.2.2: `LENGTH_UNLIMITED` as a signed long, value `-1`.
///
/// Used in QoS policies (e.g. `<resource_limits><max_samples>`) as a
/// "no limit" sentinel.
pub const LENGTH_UNLIMITED: i32 = -1;

/// Spec §7.2.2: `DURATION_INFINITE_SEC` = `0x7FFFFFFF` (max signed long).
pub const DURATION_INFINITE_SEC: i32 = 0x7FFF_FFFF;

/// Spec §7.2.2: `DURATION_INFINITE_NSEC` = `0x7FFFFFFF`.
pub const DURATION_INFINITE_NSEC: u32 = 0x7FFF_FFFF;

/// Spec §7.2.2: `DURATION_ZERO_SEC` = `0`.
pub const DURATION_ZERO_SEC: i32 = 0;

/// Spec §7.2.2: `DURATION_ZERO_NSEC` = `0`.
pub const DURATION_ZERO_NSEC: u32 = 0;

/// Spec §7.2.2.6: `TIME_INVALID_SEC = -1`.
///
/// Sentinel value for an "invalid" `Time_t.sec`. Used only for
/// XML sample encoding (DDS timestamps in XML form).
pub const TIME_INVALID_SEC: i32 = -1;

/// Spec §7.2.2.7: `TIME_INVALID_NSEC = 0xFFFFFFFF`.
pub const TIME_INVALID_NSEC: u32 = 0xFFFF_FFFF;

/// `Duration_t` per DDS 1.4 §2.2.1.2 + DDS-XML 1.0 §7.2.6.
///
/// XML-Mapping:
///
/// ```xml
/// <duration>
///   <sec>5</sec>
///   <nanosec>0</nanosec>
/// </duration>
/// ```
///
/// Sentinel values: `<sec>DURATION_INFINITE_SEC</sec>` and
/// `<nanosec>DURATION_INFINITE_NSEC</nanosec>` are mapped via
/// [`Self::INFINITE`]/[`Self::ZERO`] to the spec constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration {
    /// Seconds part (signed, since the spec `nonNegativeInteger_Duration_SEC`
    /// allows the symbol `DURATION_INFINITE_SEC`, which maps to a
    /// signed-long sentinel).
    pub sec: i32,
    /// Nanoseconds part (`0..=999_999_999`, or the sentinel
    /// [`DURATION_INFINITE_NSEC`]).
    pub nanosec: u32,
}

impl Duration {
    /// Sentinel "infinity" — both fields set to the spec sentinel.
    pub const INFINITE: Self = Self {
        sec: DURATION_INFINITE_SEC,
        nanosec: DURATION_INFINITE_NSEC,
    };

    /// Zero duration (spec default for many QoS policies).
    pub const ZERO: Self = Self {
        sec: DURATION_ZERO_SEC,
        nanosec: DURATION_ZERO_NSEC,
    };

    /// `true` if both fields carry the infinite sentinel.
    #[must_use]
    pub fn is_infinite(&self) -> bool {
        self.sec == DURATION_INFINITE_SEC && self.nanosec == DURATION_INFINITE_NSEC
    }
}

/// Boolean parser per Spec §7.1.4 Tab.7.1.
///
/// Accepted values (all case-sensitive in the spec, we additionally
/// accept the common uppercase spelling — Cyclone
/// and FastDDS are tolerant here too):
///
/// * `true`, `TRUE`, `1` -> `true`
/// * `false`, `FALSE`, `0` -> `false`
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] for any other string.
pub fn parse_bool(s: &str) -> Result<bool, XmlError> {
    let t = s.trim();
    match t {
        "1" | "true" | "TRUE" => Ok(true),
        "0" | "false" | "FALSE" => Ok(false),
        _ => Err(XmlError::ValueOutOfRange(format!(
            "boolean expected (true/false/1/0), got `{t}`"
        ))),
    }
}

/// Long parser (signed 32-bit) per Spec §7.1.4 Tab.7.1.
///
/// Accepts:
/// * Decimal: `-2147483648..=2147483647`
/// * Hex (prefix `0x` / `0X`): `0..=0xFFFFFFFF` (bit pattern,
///   reinterpreted as `i32`).
/// * Symbol `LENGTH_UNLIMITED` -> `-1`.
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] on a value range violation or a
/// parser error.
pub fn parse_long(s: &str) -> Result<i32, XmlError> {
    let t = s.trim();
    if t == "LENGTH_UNLIMITED" {
        return Ok(LENGTH_UNLIMITED);
    }
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        let v = u32::from_str_radix(hex, 16)
            .map_err(|e| XmlError::ValueOutOfRange(format!("hex long `{t}`: {e}")))?;
        // Bit-pattern reinterpret: §7.1.4 Tab.7.1 allows
        // 0x80000000..0xFFFFFFFF as a negative i32 (signed).
        return Ok(i32::from_ne_bytes(v.to_ne_bytes()));
    }
    t.parse::<i32>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("long `{t}`: {e}")))
}

/// Unsigned long parser (32-bit) per Spec §7.1.4 Tab.7.1.
///
/// Accepts decimal `0..=4294967295` and hex `0x0..=0xFFFFFFFF`.
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] on a value range violation.
pub fn parse_ulong(s: &str) -> Result<u32, XmlError> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16)
            .map_err(|e| XmlError::ValueOutOfRange(format!("hex ulong `{t}`: {e}")));
    }
    t.parse::<u32>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("ulong `{t}`: {e}")))
}

/// Seconds value for `Duration_t.sec` per Spec §7.2.2 +
/// the `nonNegativeInteger_Duration_SEC` pattern.
///
/// Accepts:
/// * Decimal `0..=0x7FFFFFFF`.
/// * Symbols `DURATION_INFINITY` and `DURATION_INFINITE_SEC` -> sentinel
///   [`DURATION_INFINITE_SEC`].
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] on negative values or overflow.
pub fn parse_duration_sec(s: &str) -> Result<i32, XmlError> {
    let t = s.trim();
    if t == "DURATION_INFINITY" || t == "DURATION_INFINITE_SEC" {
        return Ok(DURATION_INFINITE_SEC);
    }
    let v = t
        .parse::<i64>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("duration_sec `{t}`: {e}")))?;
    if !(0..=i64::from(DURATION_INFINITE_SEC)).contains(&v) {
        return Err(XmlError::ValueOutOfRange(format!(
            "duration_sec `{t}` outside 0..=0x7FFFFFFF"
        )));
    }
    // Safe: due to the range check above.
    Ok(i32::try_from(v).unwrap_or(0))
}

/// Nanoseconds value for `Duration_t.nanosec` per Spec §7.2.2 +
/// the `nonNegativeInteger_Duration_NSEC` pattern.
///
/// Accepts:
/// * Decimal `0..=999_999_999` (regular sub-second range).
/// * Symbols `DURATION_INFINITY` and `DURATION_INFINITE_NSEC` -> sentinel
///   [`DURATION_INFINITE_NSEC`].
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] on a value > 999_999_999 (unless it is a
/// sentinel).
pub fn parse_duration_nsec(s: &str) -> Result<u32, XmlError> {
    let t = s.trim();
    if t == "DURATION_INFINITY" || t == "DURATION_INFINITE_NSEC" {
        return Ok(DURATION_INFINITE_NSEC);
    }
    let v = t
        .parse::<u32>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("duration_nsec `{t}`: {e}")))?;
    // Spec range for regular values: 0..=999_999_999.
    // Values above that are only allowed via the sentinel.
    if v > 999_999_999 {
        return Err(XmlError::ValueOutOfRange(format!(
            "duration_nsec `{t}` exceeds 999_999_999"
        )));
    }
    Ok(v)
}

/// `positiveInteger_UNLIMITED` parser per Spec §7.2.2.9.
///
/// Pattern from the OMG spec: `(LENGTH_UNLIMITED|[1-9]([0-9])*)?`.
/// Unlike [`parse_long`] / [`parse_ulong`], `0` is
/// **not** an allowed value — the spec requires `[1-9]` as the first
/// digit char.
///
/// Accepts:
/// * `LENGTH_UNLIMITED` -> `-1` (sentinel; spec-conformant "unlimited").
/// * Decimal `1..=2147483647` (positive i32 values without a leading `0`).
///
/// Rejected:
/// * `0` (the spec forbids it via the `[1-9]` prefix).
/// * Negative values (except the sentinel).
/// * Hex values (the spec pattern allows only decimal).
/// * Leading zeros (e.g. `01`, `001`).
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] on a violation of the constraints
/// named above.
pub fn parse_positive_long_unlimited(s: &str) -> Result<i32, XmlError> {
    let t = s.trim();
    if t == "LENGTH_UNLIMITED" {
        return Ok(LENGTH_UNLIMITED);
    }
    // Spec pattern `[1-9]([0-9])*` — first digit 1..=9, no hex,
    // no leading zeros.
    let mut chars = t.chars();
    let first = chars.next().ok_or_else(|| {
        XmlError::ValueOutOfRange("positiveInteger_UNLIMITED: empty input".to_string())
    })?;
    if !('1'..='9').contains(&first) {
        return Err(XmlError::ValueOutOfRange(format!(
            "positiveInteger_UNLIMITED `{t}`: first digit must be 1..9 (spec pattern)"
        )));
    }
    if !chars.all(|c| c.is_ascii_digit()) {
        return Err(XmlError::ValueOutOfRange(format!(
            "positiveInteger_UNLIMITED `{t}`: only ASCII decimal digits allowed"
        )));
    }
    t.parse::<i32>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("positiveInteger_UNLIMITED `{t}`: {e}")))
}

/// Octet sequence parser per Spec §7.2.4.2 (comma-separated
/// decimal/hex). Each element is an octet (`u8`).
///
/// Accepts:
/// * Comma-separated decimal: `0,1,2,255`.
/// * Comma-separated hex (prefix `0x` / `0X`): `0x00,0xFF`.
/// * Mixed allowed: `1,0x02,3` (each element is parsed individually).
/// * Whitespace around commas is trimmed.
/// * Empty string -> empty sequence.
///
/// Rejected:
/// * Values outside `0..=255`.
/// * Trailing comma (e.g. `1,2,`).
/// * Non-numeric tokens.
///
/// For Base64-encoded octet sequences see `qos_parser::base64_decode`
/// — the spec allows **either** a comma list **or** Base64,
/// distinguished by the element name (`<value>` vs. `<valueB64>`).
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] on range/format errors.
pub fn parse_octet_sequence(s: &str) -> Result<alloc::vec::Vec<u8>, XmlError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(alloc::vec::Vec::new());
    }
    // DoS cap: 1 MiB raw ≈ 256k values.
    if trimmed.len() > MAX_STRING_BYTES * 16 {
        return Err(XmlError::LimitExceeded(format!(
            "octet sequence ({} bytes) exceeds cap",
            trimmed.len()
        )));
    }
    let mut out = alloc::vec::Vec::new();
    for token in trimmed.split(',') {
        let tok = token.trim();
        if tok.is_empty() {
            return Err(XmlError::ValueOutOfRange(
                "octet sequence: empty element (e.g. `1,,2` or trailing comma)".to_string(),
            ));
        }
        let v = if let Some(hex) = tok.strip_prefix("0x").or_else(|| tok.strip_prefix("0X")) {
            u16::from_str_radix(hex, 16)
                .map_err(|e| XmlError::ValueOutOfRange(format!("octet hex `{tok}`: {e}")))?
        } else {
            tok.parse::<u16>()
                .map_err(|e| XmlError::ValueOutOfRange(format!("octet decimal `{tok}`: {e}")))?
        };
        let byte = u8::try_from(v)
            .map_err(|_| XmlError::ValueOutOfRange(format!("octet `{tok}` outside 0..=255")))?;
        out.push(byte);
    }
    Ok(out)
}

/// Enum whitelist check per Spec §7.1.4 Tab.7.1 (enum values are
/// string literals from DCPS-IDL, *not* numeric).
///
/// # Errors
/// [`XmlError::BadEnum`] if the value is not contained in the whitelist.
pub fn parse_enum<'a>(s: &str, whitelist: &[&'a str]) -> Result<&'a str, XmlError> {
    let t = s.trim();
    whitelist
        .iter()
        .find(|allowed| **allowed == t)
        .copied()
        .ok_or_else(|| XmlError::BadEnum(t.to_string()))
}

/// String-DoS-Cap: 64 KiB.
pub const MAX_STRING_BYTES: usize = 64 * 1024;

/// String parser with a DoS cap per the ZeroDDS security posture.
///
/// XML escaping (`&lt;`, `&amp;` etc.) is already decoded by roxmltree.
///
/// # Errors
/// [`XmlError::LimitExceeded`] on strings over [`MAX_STRING_BYTES`].
pub fn parse_string(s: &str) -> Result<&str, XmlError> {
    if s.len() > MAX_STRING_BYTES {
        return Err(XmlError::LimitExceeded(format!(
            "string ({} bytes) exceeds {MAX_STRING_BYTES}",
            s.len()
        )));
    }
    Ok(s)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // ---- Boolean ------------------------------------------------------

    #[test]
    fn bool_true_variants() {
        assert!(parse_bool("true").expect("true"));
        assert!(parse_bool("TRUE").expect("TRUE"));
        assert!(parse_bool("1").expect("1"));
        assert!(parse_bool("  true  ").expect("trim"));
    }

    #[test]
    fn bool_false_variants() {
        assert!(!parse_bool("false").expect("false"));
        assert!(!parse_bool("FALSE").expect("FALSE"));
        assert!(!parse_bool("0").expect("0"));
    }

    #[test]
    fn bool_invalid() {
        let err = parse_bool("yes").expect_err("invalid");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    // ---- Long --------------------------------------------------------

    #[test]
    fn long_decimal() {
        assert_eq!(parse_long("42").expect("dec"), 42);
        assert_eq!(parse_long("-1").expect("neg"), -1);
        assert_eq!(parse_long("2147483647").expect("max"), i32::MAX);
        assert_eq!(parse_long("-2147483648").expect("min"), i32::MIN);
    }

    #[test]
    fn long_hex() {
        assert_eq!(parse_long("0xFF").expect("hex"), 0xFF);
        assert_eq!(parse_long("0X80").expect("upper-X"), 0x80);
        // 0x80000000 -> reinterpreted as i32 = i32::MIN
        assert_eq!(parse_long("0x80000000").expect("hex-msb"), i32::MIN);
        assert_eq!(parse_long("0x7FFFFFFF").expect("hex-max"), i32::MAX);
    }

    #[test]
    fn long_length_unlimited_symbol() {
        assert_eq!(parse_long("LENGTH_UNLIMITED").expect("symbol"), -1);
    }

    #[test]
    fn long_invalid() {
        assert!(parse_long("not-a-number").is_err());
        assert!(parse_long("0xZZ").is_err());
    }

    // ---- ULong -------------------------------------------------------

    #[test]
    fn ulong_decimal_and_hex() {
        assert_eq!(parse_ulong("0").expect("0"), 0);
        assert_eq!(parse_ulong("4294967295").expect("max"), u32::MAX);
        assert_eq!(parse_ulong("0xFFFFFFFF").expect("hex-max"), u32::MAX);
    }

    #[test]
    fn ulong_invalid() {
        assert!(parse_ulong("-1").is_err());
        assert!(parse_ulong("4294967296").is_err());
    }

    // ---- Duration ----------------------------------------------------

    #[test]
    fn duration_sec_normal() {
        assert_eq!(parse_duration_sec("0").expect("zero"), 0);
        assert_eq!(parse_duration_sec("123").expect("normal"), 123);
    }

    #[test]
    fn duration_sec_infinite_symbols() {
        assert_eq!(
            parse_duration_sec("DURATION_INFINITY").expect("infinity"),
            DURATION_INFINITE_SEC
        );
        assert_eq!(
            parse_duration_sec("DURATION_INFINITE_SEC").expect("infinite_sec"),
            DURATION_INFINITE_SEC
        );
    }

    #[test]
    fn duration_sec_overflow() {
        // 0x80000000 ueberschreitet 0x7FFFFFFF.
        let err = parse_duration_sec("2147483648").expect_err("overflow");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn duration_nsec_normal() {
        assert_eq!(
            parse_duration_nsec("999999999").expect("max-nsec"),
            999_999_999
        );
        assert_eq!(parse_duration_nsec("0").expect("zero"), 0);
    }

    #[test]
    fn duration_nsec_infinite() {
        assert_eq!(
            parse_duration_nsec("DURATION_INFINITE_NSEC").expect("infinite"),
            DURATION_INFINITE_NSEC
        );
        assert_eq!(
            parse_duration_nsec("DURATION_INFINITY").expect("infinity"),
            DURATION_INFINITE_NSEC
        );
    }

    #[test]
    fn duration_nsec_out_of_range() {
        let err = parse_duration_nsec("1000000000").expect_err("oor");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn duration_constants() {
        assert!(Duration::INFINITE.is_infinite());
        assert!(!Duration::ZERO.is_infinite());
        assert_eq!(Duration::ZERO.sec, 0);
        assert_eq!(Duration::ZERO.nanosec, 0);
    }

    // ---- Enum --------------------------------------------------------

    #[test]
    fn enum_match() {
        let wl = ["KEEP_LAST_HISTORY_QOS", "KEEP_ALL_HISTORY_QOS"];
        assert_eq!(
            parse_enum("KEEP_LAST_HISTORY_QOS", &wl).expect("match"),
            "KEEP_LAST_HISTORY_QOS"
        );
    }

    #[test]
    fn enum_no_match() {
        let wl = ["A", "B"];
        let err = parse_enum("C", &wl).expect_err("no-match");
        assert!(matches!(err, XmlError::BadEnum(_)));
    }

    // ---- String ------------------------------------------------------

    #[test]
    fn string_short_passes() {
        assert_eq!(parse_string("hello").expect("ok"), "hello");
    }

    #[test]
    fn string_too_long_rejected() {
        let big = "x".repeat(MAX_STRING_BYTES + 1);
        let err = parse_string(&big).expect_err("too-big");
        assert!(matches!(err, XmlError::LimitExceeded(_)));
    }

    // ---- Spec-Konstanten ---------------------------------------------

    #[test]
    fn spec_constants() {
        assert_eq!(LENGTH_UNLIMITED, -1);
        assert_eq!(DURATION_INFINITE_SEC, 0x7FFF_FFFF);
        assert_eq!(DURATION_INFINITE_NSEC, 0x7FFF_FFFF);
    }

    #[test]
    fn time_invalid_constants() {
        // Spec §7.2.2.6 + §7.2.2.7.
        assert_eq!(TIME_INVALID_SEC, -1);
        assert_eq!(TIME_INVALID_NSEC, 0xFFFF_FFFF);
        // TIME_INVALID must be distinguishable from DURATION_INFINITE and
        // DURATION_ZERO (different sentinel values).
        assert_ne!(TIME_INVALID_SEC, DURATION_INFINITE_SEC);
        assert_ne!(TIME_INVALID_NSEC, DURATION_INFINITE_NSEC);
        assert_ne!(TIME_INVALID_SEC, DURATION_ZERO_SEC);
    }

    // ---- §7.2.2.9 positiveInteger_UNLIMITED --------------------------

    #[test]
    fn positive_unlimited_symbol_passes() {
        assert_eq!(
            parse_positive_long_unlimited("LENGTH_UNLIMITED").expect("symbol"),
            LENGTH_UNLIMITED
        );
    }

    #[test]
    fn positive_unlimited_one_to_max_passes() {
        assert_eq!(parse_positive_long_unlimited("1").expect("1"), 1);
        assert_eq!(parse_positive_long_unlimited("42").expect("42"), 42);
        assert_eq!(
            parse_positive_long_unlimited("2147483647").expect("max"),
            i32::MAX
        );
    }

    #[test]
    fn positive_unlimited_zero_rejected() {
        // Spec pattern `[1-9]([0-9])*` forbids 0 explicitly.
        let err = parse_positive_long_unlimited("0").expect_err("zero");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn positive_unlimited_negative_rejected() {
        let err = parse_positive_long_unlimited("-1").expect_err("negative");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn positive_unlimited_leading_zero_rejected() {
        // `01` does not match the spec pattern.
        let err = parse_positive_long_unlimited("01").expect_err("leading-zero");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn positive_unlimited_hex_rejected() {
        // Pattern allows only decimal.
        let err = parse_positive_long_unlimited("0x10").expect_err("hex");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn positive_unlimited_overflow_rejected() {
        let err = parse_positive_long_unlimited("2147483648").expect_err("overflow");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn positive_unlimited_empty_rejected() {
        let err = parse_positive_long_unlimited("").expect_err("empty");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    // ---- §7.2.4.2 octet sequences ------------------------------------

    #[test]
    fn octet_sequence_decimal_basic() {
        let bytes = parse_octet_sequence("0,1,2,255").expect("ok");
        assert_eq!(bytes, alloc::vec![0, 1, 2, 255]);
    }

    #[test]
    fn octet_sequence_hex_basic() {
        let bytes = parse_octet_sequence("0x00,0xFF,0x42").expect("ok");
        assert_eq!(bytes, alloc::vec![0x00, 0xFF, 0x42]);
    }

    #[test]
    fn octet_sequence_mixed_decimal_and_hex() {
        // Spec: "decimal or hexadecimal" — pro Element entscheidbar.
        let bytes = parse_octet_sequence("1,0x02,3,0x04").expect("mixed");
        assert_eq!(bytes, alloc::vec![1, 2, 3, 4]);
    }

    #[test]
    fn octet_sequence_whitespace_around_commas() {
        let bytes = parse_octet_sequence(" 1 , 2 , 3 ").expect("trim");
        assert_eq!(bytes, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn octet_sequence_empty_string_returns_empty_vec() {
        let bytes = parse_octet_sequence("").expect("empty");
        assert!(bytes.is_empty());
    }

    #[test]
    fn octet_sequence_value_above_255_rejected() {
        let err = parse_octet_sequence("0,256").expect_err("over");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn octet_sequence_negative_rejected() {
        let err = parse_octet_sequence("0,-1").expect_err("negative");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn octet_sequence_trailing_comma_rejected() {
        let err = parse_octet_sequence("1,2,").expect_err("trailing");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn octet_sequence_double_comma_rejected() {
        let err = parse_octet_sequence("1,,2").expect_err("double");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn octet_sequence_non_numeric_token_rejected() {
        let err = parse_octet_sequence("1,abc,3").expect_err("non-numeric");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn octet_sequence_hex_above_255_rejected() {
        let err = parse_octet_sequence("0x100").expect_err("hex over");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }
}
