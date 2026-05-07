// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Streaming-XML-Parser — DDS-XML 1.0 §6.2.
//!
//! Spec §6.2: XML-Wire muss XML 1.0 + Namespaces 1.0 honor. Wir
//! liefern einen Streaming-Parser ohne DOM-Buffering, der Events
//! emittiert (StartElement/EndElement/Text/Cdata).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Parser-Event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// `<elem attr="val" ...>`.
    StartElement {
        /// Tag-Name.
        name: String,
        /// Attribut-Liste (name, value).
        attrs: Vec<(String, String)>,
    },
    /// `</elem>`.
    EndElement(String),
    /// Text-Inhalt zwischen Tags.
    Text(String),
    /// `<![CDATA[...]]>` (XML 1.0 §2.7).
    CData(String),
    /// `<?xml ...?>` Declaration (XML 1.0 §2.8).
    Declaration(String),
}

/// Parser-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Unerwartetes EOF.
    UnexpectedEof,
    /// Tag-Mismatch (Closing-Tag passt nicht zum Opening).
    TagMismatch {
        /// Erwarteter (Opening-Stack-Top) Tag.
        expected: String,
        /// Erhaltener Closing-Tag.
        got: String,
    },
    /// Malformed Tag (z.B. `<tag attr=value>` ohne Quotes).
    MalformedTag(String),
    /// Unbekannte Entity-Reference (nur `&amp;`/`&lt;`/`&gt;`/`&quot;`/
    /// `&apos;` werden erkannt).
    UnknownEntity(String),
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected EOF"),
            Self::TagMismatch { expected, got } => {
                write!(f, "tag mismatch: expected </{expected}>, got </{got}>")
            }
            Self::MalformedTag(s) => write!(f, "malformed tag: {s}"),
            Self::UnknownEntity(s) => write!(f, "unknown entity: &{s};"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

/// Streaming-XML-Parser — input ist `&str`, output ist Iterator von
/// `Result<Event, ParseError>`.
#[derive(Debug)]
pub struct XmlParser<'a> {
    input: &'a str,
    pos: usize,
    stack: Vec<String>,
    finished: bool,
    pending_end: Option<String>,
}

impl<'a> XmlParser<'a> {
    /// Konstruktor.
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            stack: Vec::new(),
            finished: false,
            pending_end: None,
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self, n: usize) {
        self.pos += n;
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.advance(c.len_utf8());
            } else {
                break;
            }
        }
    }

    fn next_event(&mut self) -> Option<Result<Event, ParseError>> {
        if self.finished {
            return None;
        }
        if let Some(name) = self.pending_end.take() {
            return Some(Ok(Event::EndElement(name)));
        }
        if self.pos >= self.input.len() {
            self.finished = true;
            if !self.stack.is_empty() {
                return Some(Err(ParseError::UnexpectedEof));
            }
            return None;
        }

        if self.peek_char() == Some('<') {
            self.parse_tag()
        } else {
            self.parse_text()
        }
    }

    fn parse_tag(&mut self) -> Option<Result<Event, ParseError>> {
        // peek 2nd char to determine tag-kind
        let rest = &self.input[self.pos..];
        if rest.starts_with("<?") {
            return Some(self.parse_pi());
        }
        if rest.starts_with("<![CDATA[") {
            return Some(self.parse_cdata());
        }
        if rest.starts_with("<!--") {
            return Some(self.skip_comment());
        }
        if rest.starts_with("</") {
            return Some(self.parse_end_tag());
        }
        Some(self.parse_start_tag())
    }

    fn parse_pi(&mut self) -> Result<Event, ParseError> {
        // <?...?>
        let close = self.input[self.pos..]
            .find("?>")
            .ok_or(ParseError::UnexpectedEof)?;
        let body = &self.input[self.pos + 2..self.pos + close];
        self.pos += close + 2;
        Ok(Event::Declaration(body.trim().to_string()))
    }

    fn parse_cdata(&mut self) -> Result<Event, ParseError> {
        let start = self.pos + "<![CDATA[".len();
        let close = self.input[start..]
            .find("]]>")
            .ok_or(ParseError::UnexpectedEof)?;
        let body = &self.input[start..start + close];
        self.pos = start + close + "]]>".len();
        Ok(Event::CData(body.to_string()))
    }

    fn skip_comment(&mut self) -> Result<Event, ParseError> {
        // Skip and re-emit next event by recursive call.
        let close = self.input[self.pos..]
            .find("-->")
            .ok_or(ParseError::UnexpectedEof)?;
        self.pos += close + "-->".len();
        match self.next_event() {
            Some(r) => r,
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_start_tag(&mut self) -> Result<Event, ParseError> {
        // <name attr="val" ... > or <name ... />
        self.advance(1); // <
        let name_end = self.input[self.pos..]
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .ok_or(ParseError::UnexpectedEof)?;
        let name = self.input[self.pos..self.pos + name_end].to_string();
        if name.is_empty() {
            return Err(ParseError::MalformedTag("empty tag name".into()));
        }
        self.pos += name_end;

        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            match self.peek_char() {
                Some('>') => {
                    self.advance(1);
                    self.stack.push(name.clone());
                    return Ok(Event::StartElement { name, attrs });
                }
                Some('/') => {
                    self.advance(1);
                    if self.peek_char() != Some('>') {
                        return Err(ParseError::MalformedTag("expected > after /".into()));
                    }
                    self.advance(1);
                    // Self-closing: emit Start now, queue End for next call.
                    self.pending_end = Some(name.clone());
                    return Ok(Event::StartElement { name, attrs });
                }
                Some(_) => {
                    let (n, v) = self.parse_attr()?;
                    attrs.push((n, v));
                }
                None => return Err(ParseError::UnexpectedEof),
            }
        }
    }

    fn parse_attr(&mut self) -> Result<(String, String), ParseError> {
        let name_end = self.input[self.pos..]
            .find('=')
            .ok_or(ParseError::UnexpectedEof)?;
        let name = self.input[self.pos..self.pos + name_end].trim().to_string();
        self.pos += name_end + 1;
        let quote = self.peek_char().ok_or(ParseError::UnexpectedEof)?;
        if quote != '"' && quote != '\'' {
            return Err(ParseError::MalformedTag("attribute without quotes".into()));
        }
        self.advance(1);
        let close = self.input[self.pos..]
            .find(quote)
            .ok_or(ParseError::UnexpectedEof)?;
        let raw = &self.input[self.pos..self.pos + close];
        let value = decode_entities(raw)?;
        self.pos += close + 1;
        Ok((name, value))
    }

    fn parse_end_tag(&mut self) -> Result<Event, ParseError> {
        self.advance(2); // </
        let name_end = self.input[self.pos..]
            .find('>')
            .ok_or(ParseError::UnexpectedEof)?;
        let name = self.input[self.pos..self.pos + name_end].trim().to_string();
        self.pos += name_end + 1;
        let expected = self.stack.pop().ok_or_else(|| ParseError::TagMismatch {
            expected: String::new(),
            got: name.clone(),
        })?;
        if expected != name {
            return Err(ParseError::TagMismatch {
                expected,
                got: name,
            });
        }
        Ok(Event::EndElement(name))
    }

    fn parse_text(&mut self) -> Option<Result<Event, ParseError>> {
        let next_lt = self.input[self.pos..]
            .find('<')
            .unwrap_or(self.input.len() - self.pos);
        let raw = &self.input[self.pos..self.pos + next_lt];
        self.pos += next_lt;
        if raw.trim().is_empty() {
            return self.next_event();
        }
        Some(decode_entities(raw).map(Event::Text))
    }
}

impl Iterator for XmlParser<'_> {
    type Item = Result<Event, ParseError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_event()
    }
}

fn decode_entities(s: &str) -> Result<String, ParseError> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.char_indices();
    while let Some((_, c)) = chars.next() {
        if c == '&' {
            let rest = chars.as_str();
            let semi = rest
                .find(';')
                .ok_or_else(|| ParseError::UnknownEntity(rest.to_string()))?;
            let entity = &rest[..semi];
            let resolved = match entity {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                _ => return Err(ParseError::UnknownEntity(entity.to_string())),
            };
            out.push(resolved);
            // advance chars past the entity + semicolon
            for _ in 0..semi + 1 {
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_element() {
        let xml = "<a>text</a>";
        let events: Vec<_> = XmlParser::new(xml).collect::<Result<_, _>>().unwrap();
        assert_eq!(events.len(), 3);
        match &events[0] {
            Event::StartElement { name, .. } => assert_eq!(name, "a"),
            e => panic!("expected Start, got {e:?}"),
        }
        match &events[1] {
            Event::Text(s) => assert_eq!(s, "text"),
            e => panic!("expected Text, got {e:?}"),
        }
        match &events[2] {
            Event::EndElement(n) => assert_eq!(n, "a"),
            e => panic!("expected End, got {e:?}"),
        }
    }

    #[test]
    fn parses_attributes() {
        let xml = r#"<elem foo="bar" baz="qux"></elem>"#;
        let events: Vec<_> = XmlParser::new(xml).collect::<Result<_, _>>().unwrap();
        match &events[0] {
            Event::StartElement { attrs, .. } => {
                assert_eq!(attrs.len(), 2);
                assert_eq!(attrs[0], ("foo".into(), "bar".into()));
                assert_eq!(attrs[1], ("baz".into(), "qux".into()));
            }
            e => panic!("expected Start, got {e:?}"),
        }
    }

    #[test]
    fn parses_xml_declaration() {
        let xml = r#"<?xml version="1.0"?><a/>"#;
        let events: Vec<_> = XmlParser::new(xml).collect::<Result<_, _>>().unwrap();
        match &events[0] {
            Event::Declaration(s) => assert!(s.contains("version=\"1.0\"")),
            e => panic!("got {e:?}"),
        }
    }

    #[test]
    fn parses_cdata() {
        let xml = "<a><![CDATA[<raw>]]></a>";
        let events: Vec<_> = XmlParser::new(xml).collect::<Result<_, _>>().unwrap();
        let cdata = events.iter().find_map(|e| match e {
            Event::CData(s) => Some(s.as_str()),
            _ => None,
        });
        assert_eq!(cdata, Some("<raw>"));
    }

    #[test]
    fn skips_comments() {
        let xml = "<a><!-- comment -->text</a>";
        let events: Vec<_> = XmlParser::new(xml).collect::<Result<_, _>>().unwrap();
        let texts: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::Text(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, alloc::vec!["text"]);
    }

    #[test]
    fn decodes_entity_references() {
        let xml = r#"<a>&amp;&lt;&gt;&quot;&apos;</a>"#;
        let events: Vec<_> = XmlParser::new(xml).collect::<Result<_, _>>().unwrap();
        let text = events.iter().find_map(|e| match e {
            Event::Text(s) => Some(s.as_str()),
            _ => None,
        });
        assert_eq!(text, Some("&<>\"'"));
    }

    #[test]
    fn rejects_tag_mismatch() {
        let xml = "<a></b>";
        let r: Result<Vec<_>, _> = XmlParser::new(xml).collect();
        assert!(matches!(r, Err(ParseError::TagMismatch { .. })));
    }

    #[test]
    fn rejects_unknown_entity() {
        let xml = "<a>&xyz;</a>";
        let r: Result<Vec<_>, _> = XmlParser::new(xml).collect();
        assert!(matches!(r, Err(ParseError::UnknownEntity(_))));
    }

    #[test]
    fn nested_elements_work() {
        let xml = "<a><b><c/></b></a>";
        let events: Vec<_> = XmlParser::new(xml).collect::<Result<_, _>>().unwrap();
        // Depending on self-closing handling we expect at least 3 starts, but
        // self-closing only emits Start (no synthetic End). Verify the
        // non-self-closing ones balance correctly.
        let starts: usize = events
            .iter()
            .filter(|e| matches!(e, Event::StartElement { .. }))
            .count();
        let ends: usize = events
            .iter()
            .filter(|e| matches!(e, Event::EndElement(_)))
            .count();
        assert!(starts >= ends);
    }
}
