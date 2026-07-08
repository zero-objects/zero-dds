// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DynamicData` → JSON, and an NDJSON sample sink.
//!
//! One JSON object per decoded sample (newline-delimited). Each line carries the
//! topic, receive timestamp, writer GUID, the decoded `value` tree, and the raw
//! CDR bytes (hex) so `zerodds-replay` can re-publish byte-exact from JSON.

use std::io::Write;

use serde_json::{Map, Number, Value};
use zerodds_types::dynamic::data::{DynamicData, DynamicValue};
use zerodds_types::dynamic::descriptor::TypeKind;

/// Convert a decoded aggregate [`DynamicData`] to a JSON object
/// (`{ member_name: value, … }`, type-declared order). Unset members are omitted.
#[must_use]
/// zerodds-lint: recursion-depth 64 (walks decoded DynamicData; bounded by type nesting).
pub fn data_to_json(d: &DynamicData) -> Value {
    let ty = d.dynamic_type();
    let mut obj = Map::new();
    for m in ty.members() {
        if let Some(v) = d.get_value(m.id()) {
            obj.insert(m.name().to_string(), value_to_json(v));
        }
    }
    Value::Object(obj)
}

/// Convert a [`DynamicValue`] to JSON.
#[must_use]
/// zerodds-lint: recursion-depth 64 (walks decoded DynamicData; bounded by type nesting).
pub fn value_to_json(v: &DynamicValue) -> Value {
    match v {
        DynamicValue::Bool(b) => Value::Bool(*b),
        DynamicValue::Byte(x) | DynamicValue::UInt8(x) | DynamicValue::Char8(x) => {
            Value::Number(Number::from(*x))
        }
        DynamicValue::Int8(x) => Value::Number(Number::from(*x)),
        DynamicValue::Int16(x) => Value::Number(Number::from(*x)),
        DynamicValue::UInt16(x) | DynamicValue::Char16(x) => Value::Number(Number::from(*x)),
        DynamicValue::Int32(x) => Value::Number(Number::from(*x)),
        DynamicValue::UInt32(x) => Value::Number(Number::from(*x)),
        DynamicValue::Int64(x) => Value::Number(Number::from(*x)),
        DynamicValue::UInt64(x) => Value::Number(Number::from(*x)),
        DynamicValue::Float32(x) => f64_json(f64::from(*x)),
        DynamicValue::Float64(x) => f64_json(*x),
        DynamicValue::String(s) => Value::String(s.clone()),
        DynamicValue::WString(u) => Value::String(String::from_utf16_lossy(u)),
        DynamicValue::Complex(d) => data_to_json(d),
        DynamicValue::Sequence(items) => Value::Array(items.iter().map(element_to_json).collect()),
        DynamicValue::Map(entries) => Value::Array(
            entries
                .iter()
                .map(|(k, v)| {
                    let mut o = Map::new();
                    o.insert("key".to_string(), element_to_json(k));
                    o.insert("value".to_string(), element_to_json(v));
                    Value::Object(o)
                })
                .collect(),
        ),
        DynamicValue::None => Value::Null,
    }
}

/// A collection element is either a composite (struct/union → object) or a
/// scalar wrapper holding its value under member id 0.
fn element_to_json(d: &DynamicData) -> Value {
    match d.dynamic_type().kind() {
        TypeKind::Structure | TypeKind::Union => data_to_json(d),
        _ => d.get_value(0).map_or(Value::Null, value_to_json),
    }
}

/// NaN / ±inf are not representable in JSON → `null`.
fn f64_json(x: f64) -> Value {
    Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// Lowercase hex of `bytes` (raw CDR storage for byte-exact replay; avoids a
/// base64 dependency).
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
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
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
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
    use crate::type_source::dynamic_type_from_idl;
    use zerodds_types::dynamic::codec::{decode_dynamic, encode_dynamic};
    use zerodds_types::dynamic::type_::DynamicType;
    use zerodds_types::dynamic::{DynamicDataFactory, DynamicValue as DV};

    #[test]
    fn hex_roundtrip() {
        let b = [0xde, 0xad, 0xbe, 0xef, 0x00, 0x7f];
        assert_eq!(from_hex(&to_hex(&b)).unwrap(), b);
        assert!(from_hex("abc").is_none());
    }

    #[test]
    fn decoded_sample_to_json() {
        const IDL: &str = r#"
            module cuas {
                @final struct Track {
                    unsigned long id;
                    string name;
                    sequence<long> xs;
                };
            };
        "#;
        let ty = dynamic_type_from_idl(IDL, "Track").unwrap();
        let mut d = DynamicDataFactory::create_data(&ty).unwrap();
        d.set_uint32_value(0, 7).unwrap();
        d.set_string_value(1, "alpha").unwrap();
        let mut xs = Vec::new();
        for v in [1_i32, 2, 3] {
            let mut e = DynamicDataFactory::create_data(&DynamicType::new_primitive(
                zerodds_types::dynamic::descriptor::TypeKind::Int32,
            ))
            .unwrap();
            e.set_value_raw(0, DV::Int32(v));
            xs.push(e);
        }
        d.set_sequence_value(2, xs).unwrap();

        // Decode-from-wire path → JSON (the real recorder flow).
        let bytes = encode_dynamic(&d, true, false).unwrap();
        let back = decode_dynamic(&ty, &bytes, true, false).unwrap();
        let j = data_to_json(&back);
        assert_eq!(j["id"], serde_json::json!(7));
        assert_eq!(j["name"], serde_json::json!("alpha"));
        assert_eq!(j["xs"], serde_json::json!([1, 2, 3]));

        let line = sample_line("Track", "cuas::Track", 1_700, "aabbccdd", j, &bytes);
        let parsed: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["topic"], serde_json::json!("Track"));
        assert_eq!(parsed["type"], serde_json::json!("cuas::Track"));
        assert_eq!(parsed["value"]["name"], serde_json::json!("alpha"));
        // raw_cdr round-trips back to the exact wire bytes (replay fidelity).
        assert_eq!(
            from_hex(parsed["raw_cdr"].as_str().unwrap()).unwrap(),
            bytes
        );
    }
}
