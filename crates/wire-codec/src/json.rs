// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a value file is written in.
//!
//! The probe's value representation is JSON, because a probe has to be
//! callable from a shell script and diffable by a human. This is the
//! whole of what that costs: two trait implementations, and the four
//! rules that are JSON's rather than IDL's.
//!
//! ## The four rules
//!
//! - **A 64-bit integer is a string.** A JSON number is a double, so
//!   `9007199254740993` read back through one is `9007199254740992`. A
//!   value file that spells a 64-bit integer as a number therefore says
//!   a different number than its author typed and every reader of the
//!   file agrees on the wrong one. Refused on the way in
//!   ([`EXACT_JSON_INTEGER`]) and never produced on the way out — for
//!   every value of those two widths and not only the large ones,
//!   because a form that changed shape at 2^53 would compare unequal
//!   across the boundary.
//! - **A `long double` is a string.** Sixty of a binary128 significand's
//!   113 bits do not fit a double. A number is still accepted on the way
//!   in, because `1.5` in a value file means 1.5 and every double widens
//!   into binary128 exactly; the way out is always the exact decimal.
//! - **A `fixed<P, S>` is a string**, for the same reason: a
//!   `fixed<31, 2>` is exactly the value a double cannot carry.
//! - **NaN and infinity have no spelling.** JSON has neither, so a
//!   sample carrying one has no canonical form and is refused out loud
//!   rather than written as `null`, which would compare equal to an
//!   absent member.
//!
//! Everything else about what a value *means* is in
//! [`crate::contract`] and is the same in every representation.
//!
//! ## Why this is here and not in `zerodds-wire-probe`
//!
//! It belongs with the probe, which is the crate that reads value files;
//! Rust's orphan rule puts it here instead. Both traits and
//! `serde_json::Value` are foreign to the probe, so the probe cannot
//! implement one for the other. Behind the default `serde-json` feature,
//! so a caller that brings its own representation does not take the
//! dependency.

use crate::encode::ProbeError;
use crate::sink::{Aggregate, Sink};
use crate::source::Source;
use serde_json::{Map, Number, Value};
use std::string::String;
use std::vec::Vec;
use zerodds_ast::PrimitiveKind;
use zerodds_float128::Float128;

/// The largest integer a JSON number carries without loss.
///
/// A JSON number is a double, so `9007199254740993` does not survive the
/// round trip through one. `tests/interop-corpus/README.md` states the
/// rule the corpus is written to — "a 64-bit integer is a **string**"
/// — and this is where it is enforced rather than only documented.
pub const EXACT_JSON_INTEGER: i128 = 1 << 53;

impl Source for Value {
    fn kind(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "a boolean",
            Self::Number(_) => "a number",
            Self::String(_) => "a string",
            Self::Array(_) => "an array",
            Self::Object(_) => "an object",
        }
    }

    // Spelled with the inherent method named outright rather than as
    // `self.is_null()`: the trait method has the same name, inherent
    // resolution is what keeps the two apart, and a rename upstream
    // would turn silent precedence into silent recursion.
    fn is_null(&self) -> bool {
        Self::is_null(self)
    }

    fn is_object(&self) -> bool {
        Self::is_object(self)
    }

    fn as_bool(&self) -> Option<bool> {
        Self::as_bool(self)
    }

    fn as_text(&self) -> Option<&str> {
        self.as_str()
    }

    fn as_float(&self) -> Option<f64> {
        self.as_f64()
    }

    fn as_items(&self) -> Option<&[Self]> {
        self.as_array().map(Vec::as_slice)
    }

    fn field(&self, name: &str) -> Option<&Self> {
        self.as_object().and_then(|map| map.get(name))
    }

    fn entries(&self) -> Vec<(&str, &Self)> {
        self.as_object().map_or_else(Vec::new, |map| {
            map.iter()
                .map(|(name, value)| (name.as_str(), value))
                .collect()
        })
    }

    fn as_integer(&self, path: &str) -> Result<i128, ProbeError> {
        if let Some(text) = self.as_str() {
            return text.parse::<i128>().map_err(|_| ProbeError::Mismatch {
                path: path.to_string(),
                expected: "an integer in a string".to_string(),
                found: format!("`{text}`"),
            });
        }
        let number = self
            .as_i64()
            .map(i128::from)
            .or_else(|| self.as_u64().map(i128::from))
            .ok_or_else(|| ProbeError::Mismatch {
                path: path.to_string(),
                expected: "an integer".to_string(),
                found: Source::kind(self).to_string(),
            })?;
        if number.abs() > EXACT_JSON_INTEGER {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "an integer past 2^53 written as a string, which a JSON number \
                           cannot carry exactly"
                    .to_string(),
                found: format!("the number {number}"),
            });
        }
        Ok(number)
    }

    fn as_float128(&self, path: &str) -> Result<Float128, ProbeError> {
        match self {
            Self::String(text) => match float128_from_hex_bits(text) {
                Some(Ok(value)) => Ok(value),
                Some(Err(expected)) => Err(ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: expected.to_string(),
                    found: format!("`{text}`"),
                }),
                None => text
                    .parse::<Float128>()
                    .map_err(|error| ProbeError::Mismatch {
                        path: path.to_string(),
                        expected: "a decimal a long double can hold, or 0x and 32 hex digits"
                            .to_string(),
                        found: format!("`{text}` — {error}"),
                    }),
            },
            // Total for a JSON number — every one of the three widths
            // `serde_json` holds converts — and refused rather than
            // defaulted anyway: substituting a zero for a number that
            // would not convert is a value nobody wrote.
            Self::Number(_) => {
                self.as_f64()
                    .map(Float128::from_f64)
                    .ok_or_else(|| ProbeError::Mismatch {
                        path: path.to_string(),
                        expected: "a number".to_string(),
                        found: Source::kind(self).to_string(),
                    })
            }
            other => Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "a long double, as a decimal string or a number".to_string(),
                found: Source::kind(other).to_string(),
            }),
        }
    }

    fn render(&self) -> String {
        let mut out = String::new();
        write_text(self, &mut out);
        out
    }
}

/// The canonical text of one JSON value.
///
/// Keys sorted, no whitespace, numbers as `serde_json` writes them —
/// which is the shortest decimal that reads back as the same `f64`. This
/// is what a fixture records and what a comparison hashes; a form that
/// depended on the map implementation would change under a Cargo feature
/// somewhere else in the workspace, which is why the keys are sorted
/// here rather than taken in the order the map holds them.
///
/// zerodds-lint: recursion-depth 128 — the value's own nesting. This walks
/// a `serde_json::Value` and nothing else: every arm descends into a
/// strictly smaller part of a tree that cannot hold a cycle, so it ends
/// because the value is finite. `serde_json`'s parser caps nesting at its
/// `RECURSION_LIMIT` of 128, and a value out of `crate::decode` at
/// `crate::MAX_VALUE_DEPTH`.
fn write_text(value: &Value, out: &mut String) {
    use std::fmt::Write as _;
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_text(&Value::String(key.clone()), out);
                out.push(':');
                match map.get(key) {
                    Some(entry) => write_text(entry, out),
                    // Unreachable through `keys()`, but a probe that
                    // printed nothing here would emit invalid JSON and
                    // the reader downstream would say only "parse
                    // error". Naming it keeps the cause visible.
                    None => out.push_str("\"<missing>\""),
                }
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_text(item, out);
            }
            out.push(']');
        }
        Value::String(text) => {
            out.push('"');
            for ch in text.chars() {
                match ch {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    control if (control as u32) < 0x20 => {
                        let _ = write!(out, "\\u{:04x}", control as u32);
                    }
                    other => out.push(other),
                }
            }
            out.push('"');
        }
        Value::Number(number) => out.push_str(&number.to_string()),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Null => out.push_str("null"),
    }
}

impl Sink for Value {
    fn boolean(value: bool) -> Self {
        Self::Bool(value)
    }

    fn integer(kind: PrimitiveKind, value: i128, path: &str) -> Result<Self, ProbeError> {
        // The two 64-bit widths are decimal strings and every narrower
        // one is a number. Decided on the **width** and not on the
        // magnitude: a form that changed shape at 2^53 would compare
        // unequal across the boundary, so a `long long` holding 3 is
        // `"3"` and a `long` holding 3 is `3`.
        if matches!(kind, PrimitiveKind::Int64 | PrimitiveKind::UInt64) {
            return Ok(Self::String(format!("{value}")));
        }
        match i64::try_from(value) {
            Ok(fits) => Ok(Self::Number(fits.into())),
            // Only reachable for a width this arm is not called for, and
            // an error rather than a truncation: a canonical form that
            // silently narrowed would compare equal to a wrong decode.
            Err(_) => Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "an integer a JSON number can hold".to_string(),
                found: format!("{value}"),
            }),
        }
    }

    fn float(kind: PrimitiveKind, value: f64, path: &str) -> Result<Self, ProbeError> {
        let _ = kind;
        Self::number(value, path)
    }

    fn float128(value: Float128) -> Self {
        // The *exact* expansion and not the shortest one. Every binary
        // value has finitely many decimal digits and every column can
        // print them with nothing but long multiplication; the shortest
        // form needs a parser and a search, and four columns that each
        // implement that search will disagree about which of two equally
        // short spellings to emit long before they disagree about the
        // value. `crates/float128` still has `to_shortest_decimal` for
        // callers that want it — this is the canonical form, not the
        // readable one.
        Self::String(value.to_exact_decimal())
    }

    fn character(kind: PrimitiveKind, value: char) -> Self {
        // One-character string for both widths. The width has already
        // decided what may be in it — §7.2.2.2.1.2.2 gives Char8 eight
        // bits and §7.2.2.2.1.2.5 holds Char16 to the Basic Multilingual
        // Plane — and JSON writes both the same way.
        let _ = kind;
        Self::String(value.to_string())
    }

    fn text(value: String) -> Self {
        Self::String(value)
    }

    fn fixed(decimal: String) -> Self {
        Self::String(decimal)
    }

    fn enumerator(name: String) -> Self {
        Self::String(name)
    }

    fn bit_names(names: Vec<String>) -> Self {
        Self::Array(names.into_iter().map(Self::String).collect())
    }

    fn bitfields(fields: Vec<(String, i128)>, path: &str) -> Result<Self, ProbeError> {
        let mut out = Map::new();
        for (name, raw) in fields {
            match i64::try_from(raw) {
                Ok(number) => {
                    out.insert(name, Self::Number(number.into()));
                }
                Err(_) => {
                    return Err(ProbeError::Mismatch {
                        path: format!("{path}.{name}"),
                        expected: "a bitfield a JSON number can hold".to_string(),
                        found: format!("{raw}"),
                    });
                }
            }
        }
        Ok(Self::Object(out))
    }

    fn collection(items: Vec<Self>) -> Self {
        Self::Array(items)
    }

    fn map_entry(key: Self, value: Self) -> Self {
        Self::Array(vec![key, value])
    }

    fn aggregate(members: Aggregate<Self>) -> Self {
        Self::Object(members.into_entries().into_iter().collect())
    }

    fn number(value: f64, path: &str) -> Result<Self, ProbeError> {
        // JSON has no NaN and no infinity, so a sample carrying one has
        // no canonical form and cannot be compared. Said out loud rather
        // than written as `null`, which would compare equal to an absent
        // member.
        match Number::from_f64(value) {
            Some(number) => Ok(Self::Number(number)),
            None => Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "a finite number, which is all JSON can write".to_string(),
                found: format!("{value}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::contract::{Member, character, integer, integer_for, member_value, object};
    use serde_json::json;
    use zerodds_ast_core::Id;

    #[test]
    fn an_integer_past_two_to_the_fifty_third_has_to_be_a_string() {
        assert_eq!(
            integer(&json!("9007199254740993"), "p"),
            Ok(9_007_199_254_740_993)
        );
        let refused = integer(&json!(9_007_199_254_740_993i64), "p").expect_err("a refusal");
        assert!(matches!(refused, ProbeError::Mismatch { ref path, .. } if path == "p"));
        // The boundary itself still round-trips through a double.
        assert_eq!(
            integer(&json!(9_007_199_254_740_992i64), "p"),
            Ok(EXACT_JSON_INTEGER)
        );
    }

    #[test]
    fn a_value_outside_its_primitive_is_refused_and_not_wrapped() {
        assert_eq!(integer_for(PrimitiveKind::Octet, &json!(255), "p"), Ok(255));
        assert!(integer_for(PrimitiveKind::Octet, &json!(300), "p").is_err());
        assert!(integer_for(PrimitiveKind::Int8, &json!(-129), "p").is_err());
        assert!(integer_for(PrimitiveKind::UInt32, &json!(-1), "p").is_err());
    }

    #[test]
    fn a_char_is_one_character_and_never_the_first_of_several() {
        assert_eq!(character(&json!("a"), "p"), Ok('a'));
        assert!(character(&json!("ab"), "p").is_err());
        assert!(character(&json!(""), "p").is_err());
    }

    #[test]
    fn null_is_neither_a_value_nor_an_absence() {
        let value = json!({ "a": null });
        let fields = object(&value, "S").expect("it is an object");
        let required = Member {
            node: zerodds_ast_core::NodeId::from_parts(zerodds_ast_core::IdSpace::MODEL, 1),
            name: "a".to_string(),
            type_: Id::first_at(0),
            optional: false,
        };
        let optional = Member {
            optional: true,
            ..required.clone()
        };
        for member in [&required, &optional] {
            assert!(matches!(
                member_value(fields, member, "S"),
                Err(ProbeError::Mismatch { ref path, .. }) if path == "S.a"
            ));
        }
    }

    #[test]
    fn a_sixty_four_bit_width_is_a_string_at_every_magnitude() {
        // The width decides and not the value: a form that changed shape
        // at 2^53 would compare unequal across the boundary.
        assert_eq!(
            <Value as Sink>::integer(PrimitiveKind::Int64, 3, "p"),
            Ok(Value::String("3".to_string()))
        );
        assert_eq!(
            <Value as Sink>::integer(PrimitiveKind::Int32, 3, "p"),
            Ok(Value::Number(3.into()))
        );
    }
}

/// A `long double` written as its raw binary128 bits: `0x` and exactly
/// 32 hexadecimal digits, least significant octet first.
///
/// The dds-xtypes test suite spells every `float128` this way — its
/// `struct_float128_x1.json` carries `0x0000000000000000000000000000fe3f`,
/// which is 0.5, and `struct_primitives.json` carries
/// `0x9A999999999999999999999999890240`, which is 12.3. A decimal parse
/// cannot read either: it stops at the `x` in byte 1. Both are the
/// octets a little-endian host would write, so they are read back the
/// same way.
///
/// Returns `None` when the text is not this form at all, so the caller
/// can fall through to the decimal spelling. A text that *is* this form
/// but malformed — wrong digit count, a non-hex digit — is an error at
/// the caller, never a silently substituted value.
fn float128_from_hex_bits(text: &str) -> Option<Result<Float128, &'static str>> {
    let digits = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))?;
    if digits.len() != 32 {
        return Some(Err(
            "exactly 32 hexadecimal digits, one per binary128 nibble",
        ));
    }
    // Decoded rather than validated-then-parsed: a second pass would need a
    // fallback for the digit the first pass already accepted, and a fallback
    // on an unreachable branch is exactly the shape that hides a real one
    // later. `nibble` answers `None`, so there is no value to substitute.
    let nibble = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    };
    let mut octets = [0u8; 16];
    for (slot, pair) in octets.iter_mut().zip(digits.as_bytes().chunks_exact(2)) {
        match (nibble(pair[0]), nibble(pair[1])) {
            (Some(high), Some(low)) => *slot = (high << 4) | low,
            _ => return Some(Err("a hexadecimal digit")),
        }
    }
    Some(Ok(Float128::from_le_bytes(octets)))
}

#[cfg(test)]
mod float128_hex_tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    // The two values the dds-xtypes suite actually ships, with the
    // decimals they stand for. Before this path existed, both reached
    // `parse::<Float128>()` and failed at the `x` in byte 1 — which is
    // why 22 of the suite's 26 remaining failures were filed as "waiting
    // for a stable Rust f128" rather than as a missing spelling.
    #[test]
    fn the_suites_own_float128_literals_are_read_as_bits() {
        let half = float128_from_hex_bits("0x0000000000000000000000000000fe3f")
            .expect("recognised as hex")
            .expect("well formed");
        assert_eq!(half.to_le_bytes()[15], 0x3f, "0.5 keeps its exponent octet");
        assert_eq!(half, Float128::from_f64(0.5), "struct_float128_x1 is 0.5");

        let twelve_three = float128_from_hex_bits("0x9A999999999999999999999999890240")
            .expect("recognised as hex")
            .expect("well formed");
        let want: [u8; 16] = [
            0x9A, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x89,
            0x02, 0x40,
        ];
        assert_eq!(
            twelve_three.to_le_bytes(),
            want,
            "struct_primitives x12 round-trips its octets"
        );
    }

    #[test]
    fn a_decimal_still_reaches_the_decimal_parser() {
        assert!(
            float128_from_hex_bits("12.3").is_none(),
            "a decimal is not this spelling and must fall through"
        );
    }

    #[test]
    fn a_malformed_hex_literal_is_refused_rather_than_substituted() {
        assert!(
            matches!(float128_from_hex_bits("0xfe3f"), Some(Err(_))),
            "too few digits is an error, not a zero"
        );
        assert!(
            matches!(float128_from_hex_bits("0xZZ"), Some(Err(_))),
            "a non-hex digit is an error, not a zero"
        );
    }
}
