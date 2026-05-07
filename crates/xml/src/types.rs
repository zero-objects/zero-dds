// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL-PSM-Datentyp-Mapping nach DDS-XML 1.0 §7.2.
//!
//! Element-Werte-Parser fuer Boolean, Hex-/Dezimal-Long, Duration_t,
//! sowie die Symbol-Konstanten `LENGTH_UNLIMITED`, `DURATION_INFINITE_SEC`
//! und `DURATION_INFINITE_NSEC` aus §7.2.2.
//!
//! Alle Helper sind reine String -> typed-Value-Konvertierungen ohne
//! Allokation, soweit moeglich.

use crate::errors::XmlError;
use alloc::format;
use alloc::string::ToString;

/// Spec §7.2.2: `LENGTH_UNLIMITED` als signed long, Wert `-1`.
///
/// Wird in QoS-Policies (z.B. `<resource_limits><max_samples>`) als
/// "no limit"-Sentinel benutzt.
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
/// Sentinel-Wert fuer ein "ungueltiges" `Time_t.sec`. Wird nur fuer
/// XML-Sample-Encoding genutzt (DDS Time-Stamps in XML-Form).
pub const TIME_INVALID_SEC: i32 = -1;

/// Spec §7.2.2.7: `TIME_INVALID_NSEC = 0xFFFFFFFF`.
pub const TIME_INVALID_NSEC: u32 = 0xFFFF_FFFF;

/// `Duration_t` gemaess DDS 1.4 §2.2.1.2 + DDS-XML 1.0 §7.2.6.
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
/// Sentinel-Werte: `<sec>DURATION_INFINITE_SEC</sec>` und
/// `<nanosec>DURATION_INFINITE_NSEC</nanosec>` werden via
/// [`Self::INFINITE`]/[`Self::ZERO`] mit den Spec-Konstanten gemappt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration {
    /// Sekunden-Anteil (signed, da Spec `nonNegativeInteger_Duration_SEC`
    /// das Symbol `DURATION_INFINITE_SEC` zulaesst, das auf einen
    /// signed-long-Sentinel mappt).
    pub sec: i32,
    /// Nanosekunden-Anteil (`0..=999_999_999`, oder Sentinel
    /// [`DURATION_INFINITE_NSEC`]).
    pub nanosec: u32,
}

impl Duration {
    /// Sentinel "infinity" — beide Felder auf den Spec-Sentinel.
    pub const INFINITE: Self = Self {
        sec: DURATION_INFINITE_SEC,
        nanosec: DURATION_INFINITE_NSEC,
    };

    /// Null-Duration (Spec-Default fuer viele QoS-Policies).
    pub const ZERO: Self = Self {
        sec: DURATION_ZERO_SEC,
        nanosec: DURATION_ZERO_NSEC,
    };

    /// `true` wenn beide Felder den Infinite-Sentinel tragen.
    #[must_use]
    pub fn is_infinite(&self) -> bool {
        self.sec == DURATION_INFINITE_SEC && self.nanosec == DURATION_INFINITE_NSEC
    }
}

/// Boolean-Parser gemaess Spec §7.1.4 Tab.7.1.
///
/// Akzeptierte Werte (alle case-sensitive in der Spec, wir akzeptieren
/// zusaetzlich die haeufige Schreibweise mit Grossbuchstaben — Cyclone
/// und FastDDS sind hier ebenfalls tolerant):
///
/// * `true`, `TRUE`, `1` -> `true`
/// * `false`, `FALSE`, `0` -> `false`
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] bei jedem anderen String.
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

/// Long-Parser (signed 32-bit) gemaess Spec §7.1.4 Tab.7.1.
///
/// Akzeptiert:
/// * Dezimal: `-2147483648..=2147483647`
/// * Hex (Prefix `0x` / `0X`): `0..=0xFFFFFFFF` (Bit-Pattern, wird in
///   `i32` reinterpretiert).
/// * Symbol `LENGTH_UNLIMITED` -> `-1`.
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] bei Wertbereich-Verletzung oder
/// Parser-Fehler.
pub fn parse_long(s: &str) -> Result<i32, XmlError> {
    let t = s.trim();
    if t == "LENGTH_UNLIMITED" {
        return Ok(LENGTH_UNLIMITED);
    }
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        let v = u32::from_str_radix(hex, 16)
            .map_err(|e| XmlError::ValueOutOfRange(format!("hex long `{t}`: {e}")))?;
        // Bit-Pattern-Reinterpret: §7.1.4 Tab.7.1 erlaubt
        // 0x80000000..0xFFFFFFFF als negative i32 (signed).
        return Ok(i32::from_ne_bytes(v.to_ne_bytes()));
    }
    t.parse::<i32>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("long `{t}`: {e}")))
}

/// Unsigned-Long-Parser (32-bit) gemaess Spec §7.1.4 Tab.7.1.
///
/// Akzeptiert Dezimal `0..=4294967295` und Hex `0x0..=0xFFFFFFFF`.
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] bei Wertbereich-Verletzung.
pub fn parse_ulong(s: &str) -> Result<u32, XmlError> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16)
            .map_err(|e| XmlError::ValueOutOfRange(format!("hex ulong `{t}`: {e}")));
    }
    t.parse::<u32>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("ulong `{t}`: {e}")))
}

/// Sekunden-Wert fuer `Duration_t.sec` gemaess Spec §7.2.2 +
/// `nonNegativeInteger_Duration_SEC` Pattern.
///
/// Akzeptiert:
/// * Dezimal `0..=0x7FFFFFFF`.
/// * Symbol `DURATION_INFINITY` und `DURATION_INFINITE_SEC` -> Sentinel
///   [`DURATION_INFINITE_SEC`].
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] bei negativen Werten oder Overflow.
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
    // Safe: durch Range-Check oben.
    Ok(i32::try_from(v).unwrap_or(0))
}

/// Nanosekunden-Wert fuer `Duration_t.nanosec` gemaess Spec §7.2.2 +
/// `nonNegativeInteger_Duration_NSEC` Pattern.
///
/// Akzeptiert:
/// * Dezimal `0..=999_999_999` (regulaerer Sub-Sekunden-Bereich).
/// * Symbol `DURATION_INFINITY` und `DURATION_INFINITE_NSEC` -> Sentinel
///   [`DURATION_INFINITE_NSEC`].
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] bei Wert > 999_999_999 (sofern kein
/// Sentinel).
pub fn parse_duration_nsec(s: &str) -> Result<u32, XmlError> {
    let t = s.trim();
    if t == "DURATION_INFINITY" || t == "DURATION_INFINITE_NSEC" {
        return Ok(DURATION_INFINITE_NSEC);
    }
    let v = t
        .parse::<u32>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("duration_nsec `{t}`: {e}")))?;
    // Spec-Range fuer regulaere Werte: 0..=999_999_999.
    // Werte daueber sind nur via Sentinel zulaessig.
    if v > 999_999_999 {
        return Err(XmlError::ValueOutOfRange(format!(
            "duration_nsec `{t}` exceeds 999_999_999"
        )));
    }
    Ok(v)
}

/// `positiveInteger_UNLIMITED`-Parser gemaess Spec §7.2.2.9.
///
/// Pattern aus der OMG-Spec: `(LENGTH_UNLIMITED|[1-9]([0-9])*)?`.
/// Im Unterschied zu [`parse_long`] / [`parse_ulong`] ist `0`
/// **kein** zulaessiger Wert — die Spec verlangt `[1-9]` als ersten
/// Ziffer-Char.
///
/// Akzeptiert:
/// * `LENGTH_UNLIMITED` -> `-1` (Sentinel; spec-konform "unlimited").
/// * Dezimal `1..=2147483647` (positive i32-Werte ohne fuehrende `0`).
///
/// Rejected:
/// * `0` (Spec verbietet via `[1-9]`-Praefix).
/// * Negative Werte (ausser dem Sentinel).
/// * Hex-Werte (Spec-Pattern erlaubt nur Dezimal).
/// * Fuehrende Nullen (z.B. `01`, `001`).
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] bei Verletzung der oben genannten
/// Constraints.
pub fn parse_positive_long_unlimited(s: &str) -> Result<i32, XmlError> {
    let t = s.trim();
    if t == "LENGTH_UNLIMITED" {
        return Ok(LENGTH_UNLIMITED);
    }
    // Spec-Pattern `[1-9]([0-9])*` — erste Ziffer 1..=9, keine Hex,
    // keine fuehrenden Nullen.
    let mut chars = t.chars();
    let first = chars.next().ok_or_else(|| {
        XmlError::ValueOutOfRange("positiveInteger_UNLIMITED: empty input".to_string())
    })?;
    if !('1'..='9').contains(&first) {
        return Err(XmlError::ValueOutOfRange(format!(
            "positiveInteger_UNLIMITED `{t}`: erste Ziffer muss 1..9 sein (Spec-Pattern)"
        )));
    }
    if !chars.all(|c| c.is_ascii_digit()) {
        return Err(XmlError::ValueOutOfRange(format!(
            "positiveInteger_UNLIMITED `{t}`: nur ASCII-Dezimalziffern erlaubt"
        )));
    }
    t.parse::<i32>()
        .map_err(|e| XmlError::ValueOutOfRange(format!("positiveInteger_UNLIMITED `{t}`: {e}")))
}

/// Octet-Sequenz-Parser gemaess Spec §7.2.4.2 (Comma-separated
/// decimal/hex). Jedes Element ist ein Octet (`u8`).
///
/// Akzeptiert:
/// * Comma-separated Dezimal: `0,1,2,255`.
/// * Comma-separated Hex (Prefix `0x` / `0X`): `0x00,0xFF`.
/// * Gemischt erlaubt: `1,0x02,3` (jedes Element wird einzeln geparst).
/// * Whitespace um Kommas wird getrimmt.
/// * Leerer String -> leere Sequenz.
///
/// Rejected:
/// * Werte ausserhalb `0..=255`.
/// * Trailing-Komma (z.B. `1,2,`).
/// * Nicht-numerische Tokens.
///
/// Fuer Base64-encoded Octet-Sequenzen siehe `qos_parser::base64_decode`
/// — die Spec erlaubt **entweder** Comma-Liste **oder** Base64,
/// unterschieden durch Element-Namen (`<value>` vs. `<valueB64>`).
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] bei Range-/Format-Fehlern.
pub fn parse_octet_sequence(s: &str) -> Result<alloc::vec::Vec<u8>, XmlError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(alloc::vec::Vec::new());
    }
    // DoS-Cap: 1 MiB roh ≈ 256k Werte.
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
                "octet sequence: leeres Element (z.B. `1,,2` oder trailing comma)".to_string(),
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

/// Enum-Whitelist-Pruefung gemaess Spec §7.1.4 Tab.7.1 (enum-Werte sind
/// String-Literale aus DCPS-IDL, *nicht* numerisch).
///
/// # Errors
/// [`XmlError::BadEnum`] wenn der Wert nicht in der Whitelist enthalten
/// ist.
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

/// String-Parser mit DoS-Cap gemaess ZeroDDS-Security-Posture.
///
/// XML-Escaping (`&lt;`, `&amp;` etc.) ist bereits durch roxmltree
/// dekodiert.
///
/// # Errors
/// [`XmlError::LimitExceeded`] bei Strings ueber [`MAX_STRING_BYTES`].
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
        // TIME_INVALID muss von DURATION_INFINITE und DURATION_ZERO
        // unterscheidbar sein (verschiedene Sentinel-Werte).
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
        // Spec-Pattern `[1-9]([0-9])*` verbietet 0 explizit.
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
        // `01` matcht nicht das Spec-Pattern.
        let err = parse_positive_long_unlimited("01").expect_err("leading-zero");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn positive_unlimited_hex_rejected() {
        // Pattern erlaubt nur Dezimal.
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

    // ---- §7.2.4.2 Octet-Sequenzen ------------------------------------

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
