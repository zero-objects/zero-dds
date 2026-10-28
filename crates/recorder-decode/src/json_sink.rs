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
    if s.len() % 2 != 0 {
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
    raw_cdr: &[u8],
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
    obj.insert("raw_cdr".to_string(), Value::String(to_hex(raw_cdr)));
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
        raw_cdr: &[u8],
    ) -> std::io::Result<()> {
        let line = sample_line(
            topic,
            type_name,
            recv_ts_ns,
            writer_hex,
            data_to_json(data),
            raw_cdr,
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
    use crate::type_source::TypeBook;
    use zerodds_ast_layout::key::{Encoding, Endianness};
    use zerodds_wire_codec::{ProbeRequest, decode, encode};

    const IDL: &str = r"
        module cuas {
            @final struct Track {
                unsigned long id;
                string name;
                sequence<long> xs;
            };
        };
    ";

    #[test]
    fn hex_roundtrip() {
        let b = [0xde, 0xad, 0xbe, 0xef, 0x00, 0x7f];
        assert_eq!(from_hex(&to_hex(&b)).unwrap(), b);
        assert!(from_hex("abc").is_none());
    }

    /// The real recorder flow: encode a value, decode the octets back, and
    /// render the decoded tree. The JSON is produced from what came off the
    /// wire and not from what went onto it.
    #[test]
    fn decoded_sample_to_json() {
        let book = TypeBook::from_idl(IDL).expect("idl");
        let request = ProbeRequest::new("cuas::Track", Encoding::DelimitedCdr, Endianness::Little);
        let value = DynamicData::Aggregate(vec![
            (
                String::from("id"),
                DynamicData::Integer {
                    kind: None,
                    value: 7,
                },
            ),
            (
                String::from("name"),
                DynamicData::Text(String::from("alpha")),
            ),
            (
                String::from("xs"),
                DynamicData::Collection(vec![
                    DynamicData::Integer {
                        kind: None,
                        value: 1,
                    },
                    DynamicData::Integer {
                        kind: None,
                        value: 2,
                    },
                    DynamicData::Integer {
                        kind: None,
                        value: 3,
                    },
                ]),
            ),
        ]);

        let bytes = encode(book.ast(), &request, &value).expect("encode");
        let back: DynamicData = decode(book.ast(), &request, &bytes).expect("decode");
        let j = data_to_json(&back);
        assert_eq!(j["id"], serde_json::json!(7));
        assert_eq!(j["name"], serde_json::json!("alpha"));
        assert_eq!(j["xs"], serde_json::json!([1, 2, 3]));

        let line = sample_line("Track", "cuas::Track", 1_700, "aabbccdd", j, &bytes);
        let parsed: Value = serde_json::from_str(&line).expect("line parses");
        assert_eq!(parsed["topic"], serde_json::json!("Track"));
        assert_eq!(parsed["type"], serde_json::json!("cuas::Track"));
        assert_eq!(parsed["value"]["name"], serde_json::json!("alpha"));
        assert_eq!(
            from_hex(parsed["raw_cdr"].as_str().unwrap()).unwrap(),
            bytes
        );
    }

    /// A `uint64` above `2^63` has no `i64` and the value tree holds it
    /// exactly; it must reach JSON as a number and not as a rounded float.
    #[test]
    fn a_large_unsigned_integer_stays_a_number() {
        let v = DynamicData::Integer {
            kind: None,
            value: i128::from(u64::MAX),
        };
        assert_eq!(data_to_json(&v), serde_json::json!(u64::MAX));
    }

    /// Beyond `u64` there is no JSON number at all, and the decimal string
    /// says so rather than losing digits.
    #[test]
    fn an_integer_past_u64_becomes_its_exact_decimal() {
        let big = i128::from(u64::MAX) + 1;
        assert_eq!(
            data_to_json(&DynamicData::Integer {
                kind: None,
                value: big
            }),
            Value::String(big.to_string())
        );
    }

    /// A `fixed` is a decimal. Writing it as an `f64` would make
    /// `fixed<28,4>` a different number, so it stays the text the canonical
    /// form gives it.
    #[test]
    fn a_fixed_keeps_its_decimal_digits() {
        let text = String::from("1234567890123456789012.3456");
        assert_eq!(
            data_to_json(&DynamicData::Fixed(text.clone())),
            Value::String(text)
        );
    }

    #[test]
    fn not_a_number_has_no_json_spelling() {
        assert_eq!(
            data_to_json(&DynamicData::Float {
                kind: None,
                value: f64::NAN
            }),
            Value::Null
        );
    }

    /// An enumerator is its name. The number behind it depends on `@value`
    /// and on the dialect, and two peers that number theirs differently
    /// still mean the same enumerator.
    #[test]
    fn an_enumerator_is_written_by_name() {
        assert_eq!(
            data_to_json(&DynamicData::Enumerator(String::from("RED"))),
            serde_json::json!("RED")
        );
    }
}
