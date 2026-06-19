// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SOAP 1.2 Envelope — W3C SOAP 1.2 Part 1 §5.
//!
//! `<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope">`
//! with `<soap:Header>?` and `<soap:Body>`.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_xml_wire::emitter::{EmitError, XmlEmitter};
use zerodds_xml_wire::parser::{Event, ParseError, XmlParser};

/// SOAP 1.2 Envelope-Namespace (W3C 2003).
pub const SOAP_12_NS: &str = "http://www.w3.org/2003/05/soap-envelope";

/// SOAP-Envelope (Header + Body).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Envelope {
    /// Optional header content as raw XML.
    pub header_xml: Option<String>,
    /// Body content as raw XML (e.g. an Operation-Request element).
    pub body_xml: String,
}

/// Envelope error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeError {
    /// XML parse error.
    Parse(ParseError),
    /// XML emit error.
    Emit(EmitError),
    /// No `<soap:Envelope>` root.
    NoEnvelope,
    /// No `<soap:Body>` in the envelope.
    NoBody,
}

impl core::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "parse: {e}"),
            Self::Emit(e) => write!(f, "emit: {e}"),
            Self::NoEnvelope => f.write_str("no <soap:Envelope> root"),
            Self::NoBody => f.write_str("no <soap:Body>"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EnvelopeError {}

impl From<ParseError> for EnvelopeError {
    fn from(e: ParseError) -> Self {
        Self::Parse(e)
    }
}
impl From<EmitError> for EnvelopeError {
    fn from(e: EmitError) -> Self {
        Self::Emit(e)
    }
}

/// Builds a SOAP-1.2 envelope string. Spec §5.1.
///
/// # Errors
/// `Emit` if the body or header tag name is invalid.
pub fn build_envelope(env: &Envelope) -> Result<String, EnvelopeError> {
    let mut e = XmlEmitter::new();
    e.declaration();
    e.start_element("soap:Envelope", &[("xmlns:soap", SOAP_12_NS)])?;
    if let Some(h) = &env.header_xml {
        // Header is raw XML — caller is responsible for valid form.
        // We emit a wrapping <soap:Header> and inject raw between.
        e.start_element("soap:Header", &[])?;
        // We can't push raw XML through XmlEmitter, so we drain it out
        // to a String and concatenate manually.
        let mut prefix = e.finish();
        prefix.push_str(h);
        prefix.push_str("</soap:Header>");
        let mut e2 = XmlEmitter::new();
        e2.start_element("soap:Body", &[])?;
        prefix.push_str(&e2.finish());
        prefix.push_str(&env.body_xml);
        prefix.push_str("</soap:Body></soap:Envelope>");
        return Ok(prefix);
    }
    // No header: emit Body directly.
    e.start_element("soap:Body", &[])?;
    let mut out = e.finish();
    out.push_str(&env.body_xml);
    out.push_str("</soap:Body></soap:Envelope>");
    Ok(out)
}

/// Parses a SOAP-1.2 envelope string back to [`Envelope`].
///
/// # Errors
/// `NoEnvelope` / `NoBody` / `Parse`.
pub fn parse_envelope(xml: &str) -> Result<Envelope, EnvelopeError> {
    // Find <soap:Envelope> ... </soap:Envelope> bounds.
    let env_open = xml
        .find("<soap:Envelope")
        .ok_or(EnvelopeError::NoEnvelope)?;
    let env_inner_start =
        xml[env_open..].find('>').ok_or(EnvelopeError::NoEnvelope)? + env_open + 1;
    let env_close = xml
        .rfind("</soap:Envelope>")
        .ok_or(EnvelopeError::NoEnvelope)?;
    let inner = &xml[env_inner_start..env_close];

    // Extract header (optional) and body.
    let header_xml = inner_block(inner, "soap:Header");
    let body_xml = inner_block(inner, "soap:Body").ok_or(EnvelopeError::NoBody)?;

    // Sanity-parse the body to make sure it's at least balanced XML
    // (best-effort — we don't reject content that fails our minimal
    // parser, since the spec allows arbitrary user-types).
    let _ = XmlParser::new(&body_xml).collect::<Result<Vec<Event>, _>>();

    Ok(Envelope {
        header_xml,
        body_xml,
    })
}

fn inner_block(input: &str, tag: &str) -> Option<String> {
    let open_marker = format!("<{tag}");
    let close_marker = format!("</{tag}>");
    let open = input.find(&open_marker)?;
    let inner_start = input[open..].find('>')? + open + 1;
    let close = input.find(&close_marker)?;
    if close <= inner_start {
        return None;
    }
    Some(input[inner_start..close].trim().to_string())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_body_only() {
        let env = Envelope {
            header_xml: None,
            body_xml: "<m:Get xmlns:m=\"x\"/>".into(),
        };
        let out = build_envelope(&env).unwrap();
        assert!(out.contains("<soap:Envelope"));
        assert!(out.contains("<soap:Body>"));
        assert!(out.contains("<m:Get"));
        assert!(out.ends_with("</soap:Envelope>"));
    }

    #[test]
    fn build_with_header() {
        let env = Envelope {
            header_xml: Some("<h:Token xmlns:h=\"y\"/>".into()),
            body_xml: "<m:Op/>".into(),
        };
        let out = build_envelope(&env).unwrap();
        assert!(out.contains("<soap:Header>"));
        assert!(out.contains("<h:Token"));
        assert!(out.contains("<soap:Body>"));
    }

    #[test]
    fn parse_round_trip() {
        let env = Envelope {
            header_xml: Some("<h:Token xmlns:h=\"y\"/>".into()),
            body_xml: "<m:Op xmlns:m=\"x\"/>".into(),
        };
        let xml = build_envelope(&env).unwrap();
        let parsed = parse_envelope(&xml).unwrap();
        assert!(parsed.body_xml.contains("m:Op"));
        assert!(parsed.header_xml.unwrap().contains("h:Token"));
    }

    #[test]
    fn parse_missing_envelope_rejected() {
        let xml = "<foo/>";
        assert!(matches!(
            parse_envelope(xml),
            Err(EnvelopeError::NoEnvelope)
        ));
    }

    #[test]
    fn parse_missing_body_rejected() {
        let xml = format!("<soap:Envelope xmlns:soap=\"{SOAP_12_NS}\"></soap:Envelope>");
        assert!(matches!(parse_envelope(&xml), Err(EnvelopeError::NoBody)));
    }

    #[test]
    fn build_includes_namespace_declaration() {
        let env = Envelope {
            header_xml: None,
            body_xml: String::new(),
        };
        let out = build_envelope(&env).unwrap();
        assert!(out.contains(SOAP_12_NS));
    }
}
