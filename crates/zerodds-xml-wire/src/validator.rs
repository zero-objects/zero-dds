// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Schema validation — DDS-XML 1.0 §6.6.
//!
//! Spec §6.6: the XSD must validate all received samples. We
//! implement minimal XSD: element names + type coercion to
//! `xs:long`/`xs:double`/`xs:boolean`/`xs:string`/`xs:hexBinary`.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};

use crate::codec::{FieldKind, FieldValue};
use crate::xsd::XsdGenerator;

/// Validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// A required field is missing from the sample.
    MissingField(String),
    /// A field has the wrong type.
    TypeMismatch {
        /// Field name.
        field: String,
        /// Expected XSD type name.
        expected: String,
        /// Actual FieldKind.
        actual: FieldKind,
    },
    /// The sample contains an unknown field name.
    UnknownField(String),
}

impl core::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingField(s) => write!(f, "missing required field `{s}`"),
            Self::TypeMismatch {
                field,
                expected,
                actual,
            } => write!(f, "field `{field}`: expected {expected}, got {actual:?}"),
            Self::UnknownField(s) => write!(f, "unknown field `{s}`"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ValidationError {}

/// Validates a sample map against an XSD generator (= schema builder).
///
/// # Errors
/// See [`ValidationError`].
pub fn validate(
    schema: &XsdGenerator,
    sample: &BTreeMap<String, FieldValue>,
) -> Result<(), ValidationError> {
    let xsd = schema.render();
    // We parse the fields directly from the XSD source text; alternatively
    // XsdGenerator could expose the fields via a getter — here
    // we keep the XSD-source-text view as the single source.
    let fields = parse_xsd_fields(&xsd);
    // 1) UnknownField: every sample field must exist in the XSD.
    for sample_name in sample.keys() {
        if !fields.iter().any(|(n, _, _)| n == sample_name) {
            return Err(ValidationError::UnknownField(sample_name.clone()));
        }
    }
    // 2) MissingField + TypeMismatch.
    for (name, expected_xsd, optional) in &fields {
        match sample.get(name) {
            None => {
                if !optional {
                    return Err(ValidationError::MissingField(name.clone()));
                }
            }
            Some(v) => {
                let actual_xsd = field_kind_to_xsd(v.kind());
                if actual_xsd != expected_xsd.as_str() {
                    return Err(ValidationError::TypeMismatch {
                        field: name.clone(),
                        expected: expected_xsd.clone(),
                        actual: v.kind(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn field_kind_to_xsd(k: FieldKind) -> &'static str {
    match k {
        FieldKind::Integer => "xs:long",
        FieldKind::Float => "xs:double",
        FieldKind::Bool => "xs:boolean",
        FieldKind::String => "xs:string",
        FieldKind::Bytes => "xs:hexBinary",
    }
}

fn parse_xsd_fields(xsd: &str) -> alloc::vec::Vec<(String, String, bool)> {
    let mut out = alloc::vec::Vec::new();
    let mut cursor = 0;
    while let Some(s) = xsd[cursor..].find("<xs:element name=\"") {
        let start = cursor + s + "<xs:element name=\"".len();
        let name_end = match xsd[start..].find('"') {
            Some(e) => e,
            None => break,
        };
        let name = xsd[start..start + name_end].to_string();
        let after_name = start + name_end + 1;
        // Skip the schema's root element (it has type only via complexType).
        if !xsd[after_name..]
            .lines()
            .next()
            .unwrap_or("")
            .contains("type=\"")
        {
            cursor = after_name;
            continue;
        }
        let type_marker = "type=\"";
        let type_start = match xsd[after_name..].find(type_marker) {
            Some(t) => after_name + t + type_marker.len(),
            None => break,
        };
        let type_end = match xsd[type_start..].find('"') {
            Some(e) => e,
            None => break,
        };
        let xsd_type = xsd[type_start..type_start + type_end].to_string();
        // Look for minOccurs="0" on same line up to "/>".
        let line_end = xsd[after_name..]
            .find("/>")
            .map(|p| after_name + p)
            .unwrap_or(xsd.len());
        let optional = xsd[after_name..line_end].contains(r#"minOccurs="0""#);
        out.push((name, xsd_type, optional));
        cursor = line_end;
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::xsd::XsdType;

    fn schema() -> XsdGenerator {
        XsdGenerator::new("Trade")
            .field("id", XsdType::Long, false)
            .field("price", XsdType::Double, false)
            .field("symbol", XsdType::String, true)
    }

    #[test]
    fn valid_sample_passes() {
        let mut s = BTreeMap::new();
        s.insert("id".into(), FieldValue::Integer(1));
        s.insert("price".into(), FieldValue::Float(2.0));
        s.insert("symbol".into(), FieldValue::String("X".into()));
        validate(&schema(), &s).unwrap();
    }

    #[test]
    fn optional_field_can_be_absent() {
        let mut s = BTreeMap::new();
        s.insert("id".into(), FieldValue::Integer(1));
        s.insert("price".into(), FieldValue::Float(2.0));
        validate(&schema(), &s).unwrap();
    }

    #[test]
    fn missing_required_field_rejected() {
        let mut s = BTreeMap::new();
        s.insert("id".into(), FieldValue::Integer(1));
        let err = validate(&schema(), &s).unwrap_err();
        assert!(matches!(err, ValidationError::MissingField(ref f) if f == "price"));
    }

    #[test]
    fn unknown_field_rejected() {
        let mut s = BTreeMap::new();
        s.insert("id".into(), FieldValue::Integer(1));
        s.insert("price".into(), FieldValue::Float(2.0));
        s.insert("ghost".into(), FieldValue::Integer(0));
        let err = validate(&schema(), &s).unwrap_err();
        assert!(matches!(err, ValidationError::UnknownField(_)));
    }

    #[test]
    fn type_mismatch_rejected() {
        let mut s = BTreeMap::new();
        s.insert("id".into(), FieldValue::String("not-a-number".into()));
        s.insert("price".into(), FieldValue::Float(2.0));
        let err = validate(&schema(), &s).unwrap_err();
        assert!(matches!(err, ValidationError::TypeMismatch { .. }));
    }
}
