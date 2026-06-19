// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XML↔CDR-Codec — DDS-XML 1.0 §6.4.
//!
//! Spec §6.4: topic-sample mapping to XML. The sample structure is
//! encoded as `<Sample><field-name>value</field-name>...</Sample>`,
//! with a type discriminator as the root tag.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::emitter::{EmitError, XmlEmitter};
use crate::parser::{Event, ParseError, XmlParser};

/// Field-Kind (DDS-Builtin-Types Map zu DDS-XML §6.4 Tab.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// `int8`/`int16`/`int32`/`int64` — as XML integers.
    Integer,
    /// `float32`/`float64` — as XML doubles.
    Float,
    /// `bool` — as `true`/`false`.
    Bool,
    /// `string`/`wstring` — as XML text with entity encoding.
    String,
    /// `octet[]` — as hex-encoded XML text.
    Bytes,
}

/// Field value with kind info.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// 64-bit signed integer (covers all DDS integer builtins).
    Integer(i64),
    /// 64-bit float.
    Float(f64),
    /// Boolean.
    Bool(bool),
    /// String.
    String(String),
    /// Bytes (hex-encoded on emit).
    Bytes(Vec<u8>),
}

impl FieldValue {
    /// Field-Kind dieses Values.
    #[must_use]
    pub fn kind(&self) -> FieldKind {
        match self {
            Self::Integer(_) => FieldKind::Integer,
            Self::Float(_) => FieldKind::Float,
            Self::Bool(_) => FieldKind::Bool,
            Self::String(_) => FieldKind::String,
            Self::Bytes(_) => FieldKind::Bytes,
        }
    }
}

/// Codec error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// XML parse error.
    Parse(ParseError),
    /// XML emit error.
    Emit(EmitError),
    /// The field has text content invalid for the `FieldKind`.
    InvalidValue {
        /// Field-Name.
        field: String,
        /// Expected kind.
        kind: FieldKind,
        /// Read value.
        got: String,
    },
    /// Kein Sample-Root-Element.
    NoSampleRoot,
    /// Hex decoding error.
    InvalidHex,
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "parse: {e}"),
            Self::Emit(e) => write!(f, "emit: {e}"),
            Self::InvalidValue { field, kind, got } => {
                write!(f, "field `{field}`: expected {kind:?}, got `{got}`")
            }
            Self::NoSampleRoot => f.write_str("no sample root element"),
            Self::InvalidHex => f.write_str("invalid hex string"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CodecError {}

impl From<ParseError> for CodecError {
    fn from(e: ParseError) -> Self {
        Self::Parse(e)
    }
}

impl From<EmitError> for CodecError {
    fn from(e: EmitError) -> Self {
        Self::Emit(e)
    }
}

/// Encode a sample map into XML. Spec §6.4.
///
/// # Errors
/// `Emit` on tag-name validation.
pub fn encode_to_xml(
    type_name: &str,
    fields: &BTreeMap<String, FieldValue>,
) -> Result<String, CodecError> {
    let mut e = XmlEmitter::new();
    e.start_element(type_name, &[])?;
    for (k, v) in fields {
        e.start_element(k, &[("xsi:type", kind_xsi_name(v.kind()))])?;
        match v {
            FieldValue::Integer(i) => e.text(&format!("{i}")),
            FieldValue::Float(x) => e.text(&format!("{x}")),
            FieldValue::Bool(b) => e.text(if *b { "true" } else { "false" }),
            FieldValue::String(s) => e.text(s),
            FieldValue::Bytes(b) => e.text(&hex_encode(b)),
        }
        e.end_element()?;
    }
    e.end_element()?;
    Ok(e.finish())
}

/// Decode XML back into a sample map. Spec §6.4.
///
/// # Errors
/// `Parse`/`InvalidValue`/`NoSampleRoot`/`InvalidHex`.
pub fn decode_xml(xml: &str) -> Result<(String, BTreeMap<String, FieldValue>), CodecError> {
    let parser = XmlParser::new(xml);
    let mut events: Vec<Event> = Vec::new();
    for ev in parser {
        events.push(ev?);
    }
    let mut iter = events.into_iter().peekable();
    // Skip declaration.
    while let Some(Event::Declaration(_)) = iter.peek() {
        iter.next();
    }
    let type_name = match iter.next() {
        Some(Event::StartElement { name, .. }) => name,
        _ => return Err(CodecError::NoSampleRoot),
    };

    let mut fields = BTreeMap::new();
    while let Some(ev) = iter.next() {
        match ev {
            Event::StartElement { name, attrs } => {
                let kind_attr = attrs
                    .iter()
                    .find(|(k, _)| k == "xsi:type")
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("xs:string");
                let text = match iter.next() {
                    Some(Event::Text(t)) => t,
                    Some(Event::EndElement(_)) => {
                        // Empty body
                        fields.insert(name.clone(), FieldValue::String(String::new()));
                        continue;
                    }
                    _ => String::new(),
                };
                // Consume EndElement for this field
                while let Some(next) = iter.peek() {
                    if matches!(next, Event::EndElement(n) if n == &name) {
                        iter.next();
                        break;
                    }
                    iter.next();
                }
                let value = parse_value(&name, kind_attr, &text)?;
                fields.insert(name, value);
            }
            Event::EndElement(n) if n == type_name => break,
            _ => {}
        }
    }
    Ok((type_name, fields))
}

fn kind_xsi_name(k: FieldKind) -> &'static str {
    match k {
        FieldKind::Integer => "xs:long",
        FieldKind::Float => "xs:double",
        FieldKind::Bool => "xs:boolean",
        FieldKind::String => "xs:string",
        FieldKind::Bytes => "xs:hexBinary",
    }
}

fn parse_value(field: &str, xsi_type: &str, text: &str) -> Result<FieldValue, CodecError> {
    match xsi_type {
        "xs:long" | "xs:int" | "xs:short" | "xs:byte" => text
            .parse::<i64>()
            .map(FieldValue::Integer)
            .map_err(|_| CodecError::InvalidValue {
                field: field.into(),
                kind: FieldKind::Integer,
                got: text.into(),
            }),
        "xs:double" | "xs:float" => {
            text.parse::<f64>()
                .map(FieldValue::Float)
                .map_err(|_| CodecError::InvalidValue {
                    field: field.into(),
                    kind: FieldKind::Float,
                    got: text.into(),
                })
        }
        "xs:boolean" => match text {
            "true" | "1" => Ok(FieldValue::Bool(true)),
            "false" | "0" => Ok(FieldValue::Bool(false)),
            _ => Err(CodecError::InvalidValue {
                field: field.into(),
                kind: FieldKind::Bool,
                got: text.into(),
            }),
        },
        "xs:hexBinary" => hex_decode(text).map(FieldValue::Bytes),
        _ => Ok(FieldValue::String(text.to_string())),
    }
}

fn hex_encode(b: &[u8]) -> String {
    let mut out = String::with_capacity(b.len() * 2);
    for byte in b {
        let hi = (byte >> 4) & 0x0f;
        let lo = byte & 0x0f;
        out.push(hex_digit(hi));
        out.push(hex_digit(lo));
    }
    out
}

fn hex_digit(n: u8) -> char {
    match n {
        0..=9 => char::from(b'0' + n),
        10..=15 => char::from(b'A' + n - 10),
        _ => '?',
    }
}

fn hex_decode(s: &str) -> Result<Vec<u8>, CodecError> {
    if s.len() % 2 != 0 {
        return Err(CodecError::InvalidHex);
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_value(bytes[i])?;
        let lo = hex_value(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_value(c: u8) -> Result<u8, CodecError> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(CodecError::InvalidHex),
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp
)]
mod tests {
    use super::*;

    fn sample() -> BTreeMap<String, FieldValue> {
        let mut m = BTreeMap::new();
        m.insert("id".into(), FieldValue::Integer(42));
        m.insert("price".into(), FieldValue::Float(99.5));
        m.insert("active".into(), FieldValue::Bool(true));
        m.insert("symbol".into(), FieldValue::String("AAPL".into()));
        m.insert("blob".into(), FieldValue::Bytes(alloc::vec![0xCA, 0xFE]));
        m
    }

    #[test]
    fn round_trip_preserves_all_fields() {
        let s = sample();
        let xml = encode_to_xml("Trade", &s).unwrap();
        let (type_name, decoded) = decode_xml(&xml).unwrap();
        assert_eq!(type_name, "Trade");
        assert_eq!(decoded.get("id"), Some(&FieldValue::Integer(42)));
        assert_eq!(decoded.get("price"), Some(&FieldValue::Float(99.5)));
        assert_eq!(decoded.get("active"), Some(&FieldValue::Bool(true)));
        assert_eq!(
            decoded.get("symbol"),
            Some(&FieldValue::String("AAPL".into()))
        );
        assert_eq!(
            decoded.get("blob"),
            Some(&FieldValue::Bytes(alloc::vec![0xCA, 0xFE]))
        );
    }

    #[test]
    fn encode_uses_type_name_as_root() {
        let xml = encode_to_xml("MyTopic", &sample()).unwrap();
        assert!(xml.starts_with("<MyTopic"));
        assert!(xml.ends_with("</MyTopic>"));
    }

    #[test]
    fn invalid_integer_rejected() {
        let xml = r#"<T><id xsi:type="xs:long">notanumber</id></T>"#;
        let err = decode_xml(xml).unwrap_err();
        assert!(matches!(err, CodecError::InvalidValue { .. }));
    }

    #[test]
    fn invalid_bool_rejected() {
        let xml = r#"<T><b xsi:type="xs:boolean">maybe</b></T>"#;
        let err = decode_xml(xml).unwrap_err();
        assert!(matches!(
            err,
            CodecError::InvalidValue {
                kind: FieldKind::Bool,
                ..
            }
        ));
    }

    #[test]
    fn missing_xsi_type_defaults_to_string() {
        let xml = "<T><name>Alice</name></T>";
        let (_, decoded) = decode_xml(xml).unwrap();
        assert_eq!(
            decoded.get("name"),
            Some(&FieldValue::String("Alice".into()))
        );
    }

    #[test]
    fn empty_body_yields_empty_string() {
        let xml = "<T><name></name></T>";
        let (_, decoded) = decode_xml(xml).unwrap();
        assert_eq!(
            decoded.get("name"),
            Some(&FieldValue::String(String::new()))
        );
    }

    #[test]
    fn hex_round_trip() {
        let bytes = alloc::vec![0xDE, 0xAD, 0xBE, 0xEF];
        let s = hex_encode(&bytes);
        assert_eq!(s, "DEADBEEF");
        let back = hex_decode(&s).unwrap();
        assert_eq!(back, bytes);
    }

    #[test]
    fn hex_decode_rejects_odd_length() {
        assert!(hex_decode("ABC").is_err());
    }

    #[test]
    fn hex_decode_rejects_non_hex_char() {
        assert!(hex_decode("ZZ").is_err());
    }

    #[test]
    fn boolean_accepts_zero_and_one() {
        let xml = r#"<T><b xsi:type="xs:boolean">1</b></T>"#;
        let (_, decoded) = decode_xml(xml).unwrap();
        assert_eq!(decoded.get("b"), Some(&FieldValue::Bool(true)));
    }
}
