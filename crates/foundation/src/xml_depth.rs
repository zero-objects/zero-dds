// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Element nesting of an XML text, counted without parsing it.
//!
//! `roxmltree` 0.20, the one XML parser of the workspace, recurses once
//! per open element (`tokenizer::parse_element` calls `parse_content`,
//! which calls `parse_element`), with no limit of its own. A text with
//! enough unclosed start tags overflows the stack before the parser
//! returns; measured with 100 000 `<a>` on an 8 MiB main thread in a
//! release build. Every crate that hands outside text to the parser —
//! a permissions or governance document, a configuration file — runs
//! [`check_nesting`] on it first.
//!
//! The scan is iterative and skips what is not markup: comments, CDATA
//! sections, processing instructions, the DOCTYPE with its internal
//! subset, and quoted attribute values (a `>` inside a quote does not end
//! a tag). An empty-element tag (`<a/>`) opens nothing. A malformed text
//! may be miscounted; the parser refuses it afterwards in any case.

use core::fmt;

/// The nesting limit the workspace's XML readers use, the same as
/// `zerodds_xml::parser::MAX_TREE_DEPTH`. No DDS-XML, permissions,
/// governance or configuration document comes near it.
pub const XML_NESTING_LIMIT: usize = 64;

/// The text opens more than `max_depth` elements at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NestingTooDeep {
    /// The limit the text exceeded.
    pub max_depth: usize,
}

impl fmt::Display for NestingTooDeep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the XML nests more than {} elements; refused before parsing, \
             since the parser recurses once per open element",
            self.max_depth
        )
    }
}

/// Refuses a text that opens more than `max_depth` elements at once.
///
/// # Errors
/// [`NestingTooDeep`] at the first start tag past the limit.
pub fn check_nesting(xml: &str, max_depth: usize) -> Result<(), NestingTooDeep> {
    let b = xml.as_bytes();
    let mut i = 0usize;
    let mut depth = 0usize;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &b[i..];
        if rest.starts_with(b"<!--") {
            i = skip_past(b, i + 4, b"-->");
        } else if rest.starts_with(b"<![CDATA[") {
            i = skip_past(b, i + 9, b"]]>");
        } else if rest.starts_with(b"<?") {
            i = skip_past(b, i + 2, b"?>");
        } else if rest.starts_with(b"<!") {
            i = skip_declaration(b, i + 2);
        } else if rest.starts_with(b"</") {
            depth = depth.saturating_sub(1);
            i = skip_past(b, i + 2, b">");
        } else {
            let (end, empty_element) = skip_start_tag(b, i + 1);
            if !empty_element {
                depth += 1;
                if depth > max_depth {
                    return Err(NestingTooDeep { max_depth });
                }
            }
            i = end;
        }
    }
    Ok(())
}

/// Index just past the first `needle` at or after `from`, or the end.
fn skip_past(b: &[u8], from: usize, needle: &[u8]) -> usize {
    let mut i = from;
    while i + needle.len() <= b.len() {
        if &b[i..i + needle.len()] == needle {
            return i + needle.len();
        }
        i += 1;
    }
    b.len()
}

/// End of a start tag whose name begins at `from`: the index past its `>`,
/// and whether it ends in `/>`. Quoted attribute values are skipped.
fn skip_start_tag(b: &[u8], from: usize) -> (usize, bool) {
    let mut i = from;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                b'"' | b'\'' => quote = Some(c),
                b'>' => return (i + 1, i > from && b[i - 1] == b'/'),
                _ => {}
            },
        }
        i += 1;
    }
    (b.len(), false)
}

/// End of a `<!…>` declaration (the DOCTYPE) whose body begins at `from`:
/// the index past its `>`. The internal subset in `[ … ]` may hold
/// comments, processing instructions, declarations and quoted values with
/// `>` in them; the scan skips each.
fn skip_declaration(b: &[u8], from: usize) -> usize {
    let mut i = from;
    let mut bracket = false;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        let rest = &b[i..];
        if bracket && rest.starts_with(b"<!--") {
            i = skip_past(b, i + 4, b"-->");
            continue;
        }
        if bracket && rest.starts_with(b"<?") {
            i = skip_past(b, i + 2, b"?>");
            continue;
        }
        match c {
            b'"' | b'\'' => quote = Some(c),
            b'[' => bracket = true,
            b']' => bracket = false,
            b'>' if !bracket => return i + 1,
            _ => {}
        }
        i += 1;
    }
    b.len()
}

#[cfg(all(test, feature = "alloc"))]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::string::String;

    fn nested(depth: usize, closed: bool) -> String {
        let mut s = String::new();
        for _ in 0..depth {
            s.push_str("<a>");
        }
        if closed {
            for _ in 0..depth {
                s.push_str("</a>");
            }
        }
        s
    }

    #[test]
    fn the_limit_itself_passes_and_one_more_is_refused() {
        assert_eq!(check_nesting(&nested(64, true), 64), Ok(()));
        assert_eq!(
            check_nesting(&nested(65, true), 64),
            Err(NestingTooDeep { max_depth: 64 })
        );
    }

    #[test]
    fn a_long_run_of_unclosed_start_tags_is_refused_without_recursing() {
        assert_eq!(
            check_nesting(&nested(1_000_000, false), 64),
            Err(NestingTooDeep { max_depth: 64 })
        );
    }

    #[test]
    fn siblings_and_empty_elements_open_nothing_lasting() {
        let mut s = String::from("<r>");
        for _ in 0..10_000 {
            s.push_str("<a></a><b/><c x=\"1\" />");
        }
        s.push_str("</r>");
        assert_eq!(check_nesting(&s, 2), Ok(()));
    }

    #[test]
    fn markup_inside_comments_cdata_pis_and_quotes_is_not_counted() {
        let s = "<?xml version=\"1.0\"?>\
                 <r a=\"<b><c>\" b='>'>\
                 <!-- <x><y><z> -->\
                 <![CDATA[<x><y><z>]]>\
                 <?pi <x><y> ?>\
                 <s/></r>";
        assert_eq!(check_nesting(s, 1), Ok(()));
    }

    #[test]
    fn the_doctype_and_its_internal_subset_are_skipped() {
        let s = "<!DOCTYPE r [ <!ENTITY e \"<x><y>\"> <!-- <z> --> <?p <q> ?> ]>\
                 <r><s/></r>";
        assert_eq!(check_nesting(s, 1), Ok(()));
    }

    #[test]
    fn the_message_names_the_limit() {
        let e = check_nesting(&nested(3, false), 2).unwrap_err();
        assert!(alloc::format!("{e}").contains("more than 2 elements"));
    }
}
