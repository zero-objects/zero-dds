// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-XML 1.0 Cross-Vendor Conformance — Spec §6 + W3C-XSD-Validation.

use crate::{CaseResult, TestCase};

use alloc::collections::BTreeMap;
use zerodds_xml_wire::{
    Event, FieldKind, FieldValue, ValidationError, XmlEmitter, XmlParser, XsdGenerator, XsdType,
    decode_xml, encode_to_xml, validate,
};

// ============================================================================
// Spec §6.2 — XML 1.0 Streaming-Parser
// ============================================================================

fn case_1_1_xml_decl_then_root_then_text() -> CaseResult {
    let xml = r#"<?xml version="1.0"?><a>hello</a>"#;
    let events: alloc::vec::Vec<Event> = match XmlParser::new(xml).collect::<Result<_, _>>() {
        Ok(e) => e,
        Err(e) => return CaseResult::Fail(alloc::format!("parse: {e}")),
    };
    let has_decl = events.iter().any(|e| matches!(e, Event::Declaration(_)));
    let has_text = events
        .iter()
        .any(|e| matches!(e, Event::Text(t) if t == "hello"));
    if has_decl && has_text {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§6.2 declaration + text".into())
    }
}

fn case_1_2_entity_decoding() -> CaseResult {
    let xml = r#"<a>&amp;&lt;&gt;&quot;&apos;</a>"#;
    let events: alloc::vec::Vec<Event> = match XmlParser::new(xml).collect::<Result<_, _>>() {
        Ok(e) => e,
        Err(e) => return CaseResult::Fail(alloc::format!("parse: {e}")),
    };
    let text = events
        .iter()
        .find_map(|e| match e {
            Event::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .unwrap_or("");
    if text == "&<>\"'" {
        CaseResult::Pass
    } else {
        CaseResult::Fail("XML 1.0 §4.1 entity decoding".into())
    }
}

fn case_1_3_tag_mismatch_rejected() -> CaseResult {
    let xml = "<a></b>";
    let r: Result<alloc::vec::Vec<Event>, _> = XmlParser::new(xml).collect();
    if r.is_err() {
        CaseResult::Pass
    } else {
        CaseResult::Fail("XML 1.0: closing tag must match opening".into())
    }
}

// ============================================================================
// Spec §6.3 — XML-Emitter
// ============================================================================

fn case_2_1_emitter_entity_encoding() -> CaseResult {
    let mut e = XmlEmitter::new();
    if e.start_element("a", &[]).is_err() {
        return CaseResult::Fail("start element".into());
    }
    e.text("<b&c>");
    let _ = e.end_element();
    if e.finish() == "<a>&lt;b&amp;c&gt;</a>" {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§6.3 entity encoding on output".into())
    }
}

fn case_2_2_cdata_terminator_split() -> CaseResult {
    let mut e = XmlEmitter::new();
    e.cdata("contains ]]> within");
    let out = e.finish();
    if out.contains("]]]]><![CDATA[>") {
        CaseResult::Pass
    } else {
        CaseResult::Fail("XML 1.0 §2.7 `]]>` in CDATA must split".into())
    }
}

// ============================================================================
// Spec §6.4 — XML↔CDR Codec
// ============================================================================

fn case_3_1_codec_round_trip_all_types() -> CaseResult {
    let mut sample = BTreeMap::new();
    sample.insert("id".into(), FieldValue::Integer(42));
    sample.insert("price".into(), FieldValue::Float(99.5));
    sample.insert("active".into(), FieldValue::Bool(true));
    sample.insert("symbol".into(), FieldValue::String("AAPL".into()));
    sample.insert("blob".into(), FieldValue::Bytes(alloc::vec![0xCA, 0xFE]));
    let xml = match encode_to_xml("Trade", &sample) {
        Ok(s) => s,
        Err(e) => return CaseResult::Fail(alloc::format!("encode: {e}")),
    };
    match decode_xml(&xml) {
        Ok((name, decoded)) if name == "Trade" && decoded == sample => CaseResult::Pass,
        _ => CaseResult::Fail("§6.4 type round-trip".into()),
    }
}

fn case_3_2_invalid_integer_rejected() -> CaseResult {
    let xml = r#"<T><id xsi:type="xs:long">notanumber</id></T>"#;
    if decode_xml(xml).is_err() {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§6.4 invalid integer must reject".into())
    }
}

// ============================================================================
// Spec §6.5 — XSD-Generator
// ============================================================================

fn case_4_1_xsd_emits_root_and_fields() -> CaseResult {
    let xsd = XsdGenerator::new("Trade")
        .field("id", XsdType::Long, false)
        .field("symbol", XsdType::String, false)
        .render();
    if xsd.contains(r#"<xs:element name="Trade">"#)
        && xsd.contains(r#"<xs:element name="id" type="xs:long"/>"#)
        && xsd.contains(r#"<xs:element name="symbol" type="xs:string"/>"#)
    {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§6.5 XSD root + fields".into())
    }
}

fn case_4_2_xsd_optional_min_occurs_zero() -> CaseResult {
    let xsd = XsdGenerator::new("T")
        .field("opt", XsdType::Long, true)
        .render();
    if xsd.contains(r#"minOccurs="0""#) {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§6.5 optional → minOccurs=0".into())
    }
}

// ============================================================================
// Spec §6.6 — Validator
// ============================================================================

fn case_5_1_validator_accepts_valid_sample() -> CaseResult {
    let schema = XsdGenerator::new("T")
        .field("id", XsdType::Long, false)
        .field("name", XsdType::String, true);
    let mut sample = BTreeMap::new();
    sample.insert("id".into(), FieldValue::Integer(1));
    sample.insert("name".into(), FieldValue::String("X".into()));
    if validate(&schema, &sample).is_ok() {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§6.6 valid sample".into())
    }
}

fn case_5_2_validator_rejects_missing_required() -> CaseResult {
    let schema = XsdGenerator::new("T")
        .field("id", XsdType::Long, false)
        .field("name", XsdType::String, true);
    let sample = BTreeMap::new(); // missing required "id"
    match validate(&schema, &sample) {
        Err(ValidationError::MissingField(_)) => CaseResult::Pass,
        _ => CaseResult::Fail("§6.6 missing required must reject".into()),
    }
}

fn case_5_3_validator_rejects_type_mismatch() -> CaseResult {
    let schema = XsdGenerator::new("T").field("id", XsdType::Long, false);
    let mut sample = BTreeMap::new();
    sample.insert("id".into(), FieldValue::String("x".into()));
    match validate(&schema, &sample) {
        Err(ValidationError::TypeMismatch { .. }) => CaseResult::Pass,
        _ => CaseResult::Fail("§6.6 type mismatch must reject".into()),
    }
}

fn case_5_4_field_kind_xsd_mapping() -> CaseResult {
    if XsdType::from_field_kind(FieldKind::Integer) == XsdType::Long
        && XsdType::from_field_kind(FieldKind::Float) == XsdType::Double
        && XsdType::from_field_kind(FieldKind::Bytes) == XsdType::HexBinary
    {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§6.5 field-kind → xsd mapping".into())
    }
}

/// Komplette DDS-XML Test-Suite.
pub const SUITE: &[TestCase] = &[
    TestCase {
        name: "ddsxml-6.2-decl-text",
        run: case_1_1_xml_decl_then_root_then_text,
    },
    TestCase {
        name: "ddsxml-6.2-entity-decoding",
        run: case_1_2_entity_decoding,
    },
    TestCase {
        name: "ddsxml-6.2-tag-mismatch",
        run: case_1_3_tag_mismatch_rejected,
    },
    TestCase {
        name: "ddsxml-6.3-entity-encoding",
        run: case_2_1_emitter_entity_encoding,
    },
    TestCase {
        name: "ddsxml-6.3-cdata-split",
        run: case_2_2_cdata_terminator_split,
    },
    TestCase {
        name: "ddsxml-6.4-codec-roundtrip",
        run: case_3_1_codec_round_trip_all_types,
    },
    TestCase {
        name: "ddsxml-6.4-invalid-integer-rejected",
        run: case_3_2_invalid_integer_rejected,
    },
    TestCase {
        name: "ddsxml-6.5-xsd-root-fields",
        run: case_4_1_xsd_emits_root_and_fields,
    },
    TestCase {
        name: "ddsxml-6.5-xsd-optional",
        run: case_4_2_xsd_optional_min_occurs_zero,
    },
    TestCase {
        name: "ddsxml-6.6-validator-accepts",
        run: case_5_1_validator_accepts_valid_sample,
    },
    TestCase {
        name: "ddsxml-6.6-validator-missing",
        run: case_5_2_validator_rejects_missing_required,
    },
    TestCase {
        name: "ddsxml-6.6-validator-type",
        run: case_5_3_validator_rejects_type_mismatch,
    },
    TestCase {
        name: "ddsxml-6.5-fieldkind-mapping",
        run: case_5_4_field_kind_xsd_mapping,
    },
];

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn full_suite_passes() {
        let (p, s, f) = crate::run_suite(SUITE);
        assert_eq!(f, 0, "no DDS-XML cases must fail");
        assert_eq!(p + s, SUITE.len());
        assert!(p >= 12);
    }
}
