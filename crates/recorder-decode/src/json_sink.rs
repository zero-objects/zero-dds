// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DynamicData` → JSON, and an NDJSON sample sink.
//!
//! One JSON object per decoded sample (newline-delimited). Each line carries
//! the topic, receive timestamp, writer GUID, the decoded `value` tree, and
//! the raw CDR bytes (hex) so `zerodds-replay` can re-publish byte-exact from
//! JSON.
//!
//! ## What JSON cannot hold
//!
//! Three of the value tree's forms have no JSON number that is equal to them,
//! and each is written as a **string** rather than as the nearest `f64`:
//!
//! - an integer outside `i64`/`u64` — IDL has `long long` and the value tree
//!   holds an `i128`, so a `uint64` above `2^63` is representable in neither
//!   arm of `serde_json::Number`;
//! - a `fixed<P, S>`, which is a decimal and not a binary float;
//! - a `long double`, which is 113 bits of significand against `f64`'s 53.
//!
//! A JSON number in this output is therefore exact, and a JSON string in a
//! numeric position says the value did not fit rather than that it was
//! rounded. `NaN` and `±inf` are the one remaining gap and become `null`:
//! JSON has no spelling for them at all, and a string there would collide
//! with the three cases above.

use std::io::Write;

use serde_json::{Map, Number, Value};
use zerodds_types::dynamic::DynamicData;

use crate::RawSample;

/// Convert a decoded [`DynamicData`] to JSON.
///
/// An aggregate becomes an object in the order the reader met its members —
/// for a `@mutable` type, the order the *writer* chose.
#[must_use]
/// zerodds-lint: recursion-depth 64 (walks decoded DynamicData; bounded by type nesting).
pub fn data_to_json(d: &DynamicData) -> Value {
    match d {
        DynamicData::Boolean(b) => Value::Bool(*b),
        DynamicData::Integer { value, .. } => integer_json(*value),
        DynamicData::Float { value, .. } => f64_json(*value),
        DynamicData::LongDouble(x) => Value::String(x.to_string()),
        DynamicData::Character { value, .. } | DynamicData::Text(value) => {
            Value::String(value.clone())
        }
        DynamicData::Fixed(text) => Value::String(text.clone()),
        DynamicData::Enumerator(name) => Value::String(name.clone()),
        DynamicData::Bits(items) | DynamicData::Collection(items) => {
            Value::Array(items.iter().map(data_to_json).collect())
        }
        DynamicData::Bitfields(fields) | DynamicData::Aggregate(fields) => Value::Object(
            fields
                .iter()
                .map(|(name, v)| (name.clone(), data_to_json(v)))
                .collect::<Map<String, Value>>(),
        ),
        _ => Value::Null,
    }
}

/// An `i128` as a JSON number where one is equal to it, and as a decimal
/// string where none is.
fn integer_json(value: i128) -> Value {
    i64::try_from(value).map_or_else(
        |_| {
            u64::try_from(value).map_or_else(
                |_| Value::String(value.to_string()),
                |u| Value::Number(Number::from(u)),
            )
        },
        |i| Value::Number(Number::from(i)),
    )
}

/// NaN / ±inf are not representable in JSON → `null`.
fn f64_json(x: f64) -> Value {
    Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// Lowercase hex of `bytes` (raw CDR storage for byte-exact replay; avoids a
/// base64 dependency).
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Decode lowercase/uppercase hex back to bytes (replay reads `raw_cdr`).
///
/// # Errors
/// Returns `None` on odd length or a non-hex digit.
#[must_use]
pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Build one NDJSON record line for a decoded sample. The `type_name` (the DDS
/// type name, e.g. `cuas::Track`) makes the line self-describing so
/// `zerodds-replay` can re-publish from JSON without an out-of-band IDL.
#[must_use]
pub fn sample_line(
    topic: &str,
    type_name: &str,
    recv_ts_ns: i64,
    writer_hex: &str,
    value: Value,
    raw: RawSample<'_>,
) -> String {
    let mut obj = Map::new();
    obj.insert("topic".to_string(), Value::String(topic.to_string()));
    obj.insert("type".to_string(), Value::String(type_name.to_string()));
    obj.insert(
        "recv_ts_ns".to_string(),
        Value::Number(Number::from(recv_ts_ns)),
    );
    obj.insert("writer".to_string(), Value::String(writer_hex.to_string()));
    obj.insert("value".to_string(), value);
    obj.insert("raw_cdr".to_string(), Value::String(to_hex(raw.body)));
    // The representation identifier of the sample's encapsulation header
    // (DDSI-RTPS 2.5 §10.5): `raw_cdr` is the body without it, and a replay
    // sends it under the header it came with.
    if let Some(id) = raw.representation_identifier {
        obj.insert(
            "representation_identifier".to_string(),
            Value::String(to_hex(&id)),
        );
    }
    Value::Object(obj).to_string()
}

/// NDJSON sink: one decoded sample per line.
pub struct JsonSink<W: Write> {
    out: W,
    written: u64,
}

impl<W: Write> JsonSink<W> {
    /// Create a sink over `out`.
    pub fn new(out: W) -> Self {
        Self { out, written: 0 }
    }

    /// Write one decoded sample as an NDJSON line.
    ///
    /// # Errors
    /// I/O error from the underlying writer.
    pub fn write_sample(
        &mut self,
        topic: &str,
        type_name: &str,
        recv_ts_ns: i64,
        writer_hex: &str,
        data: &DynamicData,
        raw: RawSample<'_>,
    ) -> std::io::Result<()> {
        let line = sample_line(
            topic,
            type_name,
            recv_ts_ns,
            writer_hex,
            data_to_json(data),
            raw,
        );
        self.out.write_all(line.as_bytes())?;
        self.out.write_all(b"\n")?;
        self.written += 1;
        Ok(())
    }

    /// Number of samples written.
    #[must_use]
    pub fn count(&self) -> u64 {
        self.written
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// `raw_cdr` is the body in hex, two lower-case digits per octet; text
    /// of an odd length is no body.
    #[test]
    fn the_raw_body_is_two_lower_case_hex_digits_per_octet() {
        assert_eq!(to_hex(&[0xde, 0xad, 0x00, 0x7f]), "dead007f");
        assert_eq!(from_hex("dead007f").unwrap(), [0xde, 0xad, 0x00, 0x7f]);
        assert!(from_hex("abc").is_none());
    }

    /// Every value reaches JSON without losing or inventing digits
    /// (RFC 8259 §6: a number is a decimal of any length; NaN and the
    /// infinities are not numbers). A `uint64` above 2^63 stays a number,
    /// an integer past `u64` its exact decimal text, a `fixed` its decimal
    /// text (as an `f64`, `fixed<28,4>` would be another number), NaN is
    /// `null`; an enumerator is its name, since the number behind it
    /// depends on `@value` and the dialect.
    #[test]
    fn rfc_8259_6_a_value_reaches_json_without_losing_digits() {
        let integer = |value| DynamicData::Integer { kind: None, value };
        assert_eq!(
            data_to_json(&integer(i128::from(u64::MAX))),
            serde_json::json!(u64::MAX)
        );
        let big = i128::from(u64::MAX) + 1;
        assert_eq!(data_to_json(&integer(big)), Value::String(big.to_string()));
        let text = String::from("1234567890123456789012.3456");
        assert_eq!(
            data_to_json(&DynamicData::Fixed(text.clone())),
            Value::String(text)
        );
        assert_eq!(
            data_to_json(&DynamicData::Float {
                kind: None,
                value: f64::NAN
            }),
            Value::Null
        );
        assert_eq!(
            data_to_json(&DynamicData::Enumerator(String::from("RED"))),
            serde_json::json!("RED")
        );
    }
}
