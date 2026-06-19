// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XSD schema generation from DDS topic type definitions.
//!
//! Spec DDS-XML 1.0 §6.5: from an IDL type an XSD schema is
//! generated that all callers can validate against.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::codec::FieldKind;

/// XSD type reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XsdType {
    /// `xs:long`.
    Long,
    /// `xs:double`.
    Double,
    /// `xs:boolean`.
    Boolean,
    /// `xs:string`.
    String,
    /// `xs:hexBinary`.
    HexBinary,
}

impl XsdType {
    /// Returns the spec-conformant XSD name.
    #[must_use]
    pub fn xsd_name(self) -> &'static str {
        match self {
            Self::Long => "xs:long",
            Self::Double => "xs:double",
            Self::Boolean => "xs:boolean",
            Self::String => "xs:string",
            Self::HexBinary => "xs:hexBinary",
        }
    }

    /// Mapping `FieldKind` → `XsdType`.
    #[must_use]
    pub fn from_field_kind(k: FieldKind) -> Self {
        match k {
            FieldKind::Integer => Self::Long,
            FieldKind::Float => Self::Double,
            FieldKind::Bool => Self::Boolean,
            FieldKind::String => Self::String,
            FieldKind::Bytes => Self::HexBinary,
        }
    }
}

/// XSD generator builder. Spec §6.5.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct XsdGenerator {
    type_name: String,
    fields: Vec<(String, XsdType, bool)>,
}

impl XsdGenerator {
    /// Constructor with a type name (becomes the root element name).
    #[must_use]
    pub fn new(type_name: &str) -> Self {
        Self {
            type_name: type_name.into(),
            fields: Vec::new(),
        }
    }

    /// Adds a field. `optional=true` maps to
    /// `minOccurs="0"`.
    #[must_use]
    pub fn field(mut self, name: &str, kind: XsdType, optional: bool) -> Self {
        self.fields.push((name.into(), kind, optional));
        self
    }

    /// Render to an XSD XML string. Spec §6.5 Appendix A.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        out.push('\n');
        out.push_str(
            r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" elementFormDefault="qualified">"#,
        );
        out.push('\n');
        out.push_str(&format!(
            "  <xs:element name=\"{}\">\n    <xs:complexType>\n      <xs:sequence>\n",
            self.type_name
        ));
        for (name, kind, optional) in &self.fields {
            let occurs = if *optional { r#" minOccurs="0""# } else { "" };
            out.push_str(&format!(
                "        <xs:element name=\"{name}\" type=\"{}\"{occurs}/>\n",
                kind.xsd_name()
            ));
        }
        out.push_str(
            "      </xs:sequence>\n    </xs:complexType>\n  </xs:element>\n</xs:schema>\n",
        );
        out
    }

    /// Number of fields.
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn builder_renders_root_element() {
        let xsd = XsdGenerator::new("Trade")
            .field("id", XsdType::Long, false)
            .field("symbol", XsdType::String, false)
            .render();
        assert!(xsd.contains(r#"<xs:element name="Trade">"#));
        assert!(xsd.contains(r#"<xs:element name="id" type="xs:long"/>"#));
        assert!(xsd.contains(r#"<xs:element name="symbol" type="xs:string"/>"#));
    }

    #[test]
    fn optional_field_gets_min_occurs_zero() {
        let xsd = XsdGenerator::new("T")
            .field("opt", XsdType::Long, true)
            .render();
        assert!(xsd.contains(r#"minOccurs="0""#));
    }

    #[test]
    fn from_field_kind_round_trip() {
        assert_eq!(XsdType::from_field_kind(FieldKind::Integer), XsdType::Long);
        assert_eq!(XsdType::from_field_kind(FieldKind::Float), XsdType::Double);
        assert_eq!(XsdType::from_field_kind(FieldKind::Bool), XsdType::Boolean);
        assert_eq!(XsdType::from_field_kind(FieldKind::String), XsdType::String);
        assert_eq!(
            XsdType::from_field_kind(FieldKind::Bytes),
            XsdType::HexBinary
        );
    }

    #[test]
    fn declaration_starts_xsd() {
        let xsd = XsdGenerator::new("X").render();
        assert!(xsd.starts_with("<?xml"));
        assert!(xsd.contains("xs:schema"));
    }

    #[test]
    fn field_count_tracks_additions() {
        let g = XsdGenerator::new("T")
            .field("a", XsdType::Long, false)
            .field("b", XsdType::String, false);
        assert_eq!(g.field_count(), 2);
    }

    #[test]
    fn empty_type_renders_empty_sequence() {
        let xsd = XsdGenerator::new("Empty").render();
        assert!(xsd.contains("<xs:sequence>"));
        assert!(xsd.contains("</xs:sequence>"));
    }

    #[test]
    fn xsd_names_are_spec_conform() {
        assert_eq!(XsdType::Long.xsd_name(), "xs:long");
        assert_eq!(XsdType::HexBinary.xsd_name(), "xs:hexBinary");
        assert_eq!(XsdType::Boolean.xsd_name(), "xs:boolean");
    }
}
