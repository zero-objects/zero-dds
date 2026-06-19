// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Streaming-XML-Emitter — DDS-XML 1.0 §6.3.
//!
//! Spec §6.3: XML output must be XML 1.0 conformant, with correct
//! entity encoding for `<`, `>`, `&`, `"`, `'`.

use alloc::string::String;
use alloc::vec::Vec;

/// Emitter error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitError {
    /// `end_element` without a matching `start_element`.
    UnbalancedEnd,
    /// `start_element` with an invalid tag name (not an XML NameStartChar).
    InvalidTagName(String),
}

impl core::fmt::Display for EmitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnbalancedEnd => f.write_str("end_element without matching start"),
            Self::InvalidTagName(s) => write!(f, "invalid tag name `{s}`"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EmitError {}

/// XML-Emitter.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct XmlEmitter {
    buf: String,
    stack: Vec<String>,
}

impl XmlEmitter {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `<?xml version="1.0" encoding="UTF-8"?>`.
    pub fn declaration(&mut self) {
        self.buf
            .push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    }

    /// `<name>` or `<name attr="...">`.
    ///
    /// # Errors
    /// `InvalidTagName` if the tag name is empty or starts with a digit.
    pub fn start_element(&mut self, name: &str, attrs: &[(&str, &str)]) -> Result<(), EmitError> {
        if !is_valid_name(name) {
            return Err(EmitError::InvalidTagName(name.into()));
        }
        self.buf.push('<');
        self.buf.push_str(name);
        for (k, v) in attrs {
            self.buf.push(' ');
            self.buf.push_str(k);
            self.buf.push_str("=\"");
            encode_to(&mut self.buf, v);
            self.buf.push('"');
        }
        self.buf.push('>');
        self.stack.push(name.into());
        Ok(())
    }

    /// `</name>`.
    ///
    /// # Errors
    /// `UnbalancedEnd` if there is no open element on the stack.
    pub fn end_element(&mut self) -> Result<(), EmitError> {
        let name = self.stack.pop().ok_or(EmitError::UnbalancedEnd)?;
        self.buf.push_str("</");
        self.buf.push_str(&name);
        self.buf.push('>');
        Ok(())
    }

    /// Selbstschliessendes Element `<name attr="..." />`.
    ///
    /// # Errors
    /// `InvalidTagName` if the tag name is invalid.
    pub fn empty_element(&mut self, name: &str, attrs: &[(&str, &str)]) -> Result<(), EmitError> {
        if !is_valid_name(name) {
            return Err(EmitError::InvalidTagName(name.into()));
        }
        self.buf.push('<');
        self.buf.push_str(name);
        for (k, v) in attrs {
            self.buf.push(' ');
            self.buf.push_str(k);
            self.buf.push_str("=\"");
            encode_to(&mut self.buf, v);
            self.buf.push('"');
        }
        self.buf.push_str("/>");
        Ok(())
    }

    /// Text content with entity encoding.
    pub fn text(&mut self, content: &str) {
        encode_to(&mut self.buf, content);
    }

    /// `<![CDATA[...]]>`. CDATA end sequences `]]>` are split.
    pub fn cdata(&mut self, content: &str) {
        self.buf.push_str("<![CDATA[");
        // Spec XML 1.0 §2.7: `]]>` in CDATA splitten zu `]]]]><![CDATA[>`
        let safe = content.replace("]]>", "]]]]><![CDATA[>");
        self.buf.push_str(&safe);
        self.buf.push_str("]]>");
    }

    /// Consumes the emitter and returns the XML output.
    #[must_use]
    pub fn finish(self) -> String {
        self.buf
    }

    /// Output-Length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// `true` if no bytes were emitted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

fn encode_to(buf: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => buf.push_str("&amp;"),
            '<' => buf.push_str("&lt;"),
            '>' => buf.push_str("&gt;"),
            '"' => buf.push_str("&quot;"),
            '\'' => buf.push_str("&apos;"),
            _ => buf.push(c),
        }
    }
}

fn is_valid_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ':')
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn declaration_emits_xml_pi() {
        let mut e = XmlEmitter::new();
        e.declaration();
        assert!(e.finish().starts_with("<?xml"));
    }

    #[test]
    fn start_text_end_round_trip() {
        let mut e = XmlEmitter::new();
        e.start_element("a", &[]).unwrap();
        e.text("hello");
        e.end_element().unwrap();
        assert_eq!(e.finish(), "<a>hello</a>");
    }

    #[test]
    fn attributes_get_encoded() {
        let mut e = XmlEmitter::new();
        e.empty_element("a", &[("k", "v&\"")]).unwrap();
        let out = e.finish();
        assert!(out.contains("k=\"v&amp;&quot;\""));
    }

    #[test]
    fn text_entities_encoded() {
        let mut e = XmlEmitter::new();
        e.start_element("a", &[]).unwrap();
        e.text("<b&c>");
        e.end_element().unwrap();
        assert_eq!(e.finish(), "<a>&lt;b&amp;c&gt;</a>");
    }

    #[test]
    fn cdata_splits_terminator() {
        let mut e = XmlEmitter::new();
        e.cdata("contains ]]> within");
        let out = e.finish();
        // `]]>` must not appear raw without a CDATA re-open
        assert!(out.contains("]]]]><![CDATA[>"));
    }

    #[test]
    fn unbalanced_end_rejected() {
        let mut e = XmlEmitter::new();
        assert!(e.end_element().is_err());
    }

    #[test]
    fn invalid_tag_name_rejected() {
        let mut e = XmlEmitter::new();
        assert!(matches!(
            e.start_element("123abc", &[]),
            Err(EmitError::InvalidTagName(_))
        ));
        assert!(matches!(
            e.start_element("", &[]),
            Err(EmitError::InvalidTagName(_))
        ));
    }

    #[test]
    fn empty_element_self_closes() {
        let mut e = XmlEmitter::new();
        e.empty_element("br", &[]).unwrap();
        assert_eq!(e.finish(), "<br/>");
    }

    #[test]
    fn nested_elements_emit_correctly() {
        let mut e = XmlEmitter::new();
        e.start_element("a", &[]).unwrap();
        e.start_element("b", &[]).unwrap();
        e.text("x");
        e.end_element().unwrap();
        e.end_element().unwrap();
        assert_eq!(e.finish(), "<a><b>x</b></a>");
    }
}
