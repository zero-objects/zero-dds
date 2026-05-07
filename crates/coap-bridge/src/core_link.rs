// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoRE-Link-Format — RFC 6690.
//!
//! Spec §2: Resource-Discovery via `link-format`-String:
//! ```text
//!   </path>;rt="resource-type";if="interface";ct=40,
//!   </other>;title="name"
//! ```

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Ein einzelner Link (RFC 6690 §2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoreLink {
    /// URI-Reference (zwischen `<` und `>`).
    pub uri: String,
    /// Attributes als (key, value) Tupel.
    pub attrs: Vec<(String, String)>,
}

impl CoreLink {
    /// Konstruktor.
    #[must_use]
    pub fn new(uri: &str) -> Self {
        Self {
            uri: uri.into(),
            attrs: Vec::new(),
        }
    }

    /// Fuegt ein Attribut hinzu.
    #[must_use]
    pub fn attr(mut self, key: &str, value: &str) -> Self {
        self.attrs.push((key.into(), value.into()));
        self
    }

    /// Lookup eines Attribut-Werts.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Encode eine Link-Liste zu einem `link-format`-String. Spec §2.
#[must_use]
pub fn encode_links(links: &[CoreLink]) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(links.len());
    for l in links {
        let mut s = alloc::format!("<{}>", l.uri);
        for (k, v) in &l.attrs {
            // Spec §2: numerische Werte ohne Quotes, sonst quoted.
            if v.chars().all(|c| c.is_ascii_digit()) {
                s.push_str(&alloc::format!(";{k}={v}"));
            } else {
                s.push_str(&alloc::format!(";{k}=\"{}\"", v.replace('"', "\\\"")));
            }
        }
        parts.push(s);
    }
    parts.join(",")
}

/// Decode einen `link-format`-String zu einer Link-Liste. Spec §2.
///
/// # Errors
/// Static-String wenn das Format nicht parsbar ist.
pub fn decode_links(input: &str) -> Result<Vec<CoreLink>, &'static str> {
    let mut out = Vec::new();
    for chunk in split_top_level_commas(input) {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        let mut parts = chunk.splitn(2, ';');
        let uri_part = parts.next().ok_or("missing uri")?;
        let uri = uri_part
            .trim()
            .strip_prefix('<')
            .and_then(|s| s.strip_suffix('>'))
            .ok_or("uri not enclosed in <>")?
            .to_string();
        let mut link = CoreLink::new(&uri);
        if let Some(rest) = parts.next() {
            for attr in split_top_level_semicolons(rest) {
                let attr = attr.trim();
                if attr.is_empty() {
                    continue;
                }
                let mut kv = attr.splitn(2, '=');
                let key = kv.next().ok_or("attr missing key")?.trim().to_string();
                let value = match kv.next() {
                    Some(v) => unquote(v.trim()),
                    None => String::new(),
                };
                link.attrs.push((key, value));
            }
        }
        out.push(link);
    }
    Ok(out)
}

fn split_top_level_commas(input: &str) -> Vec<&str> {
    split_top_level(input, ',')
}

fn split_top_level_semicolons(input: &str) -> Vec<&str> {
    split_top_level(input, ';')
}

fn split_top_level(input: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut in_quotes = false;
    let mut start = 0;
    for (i, c) in input.char_indices() {
        match c {
            '"' if !in_quotes => in_quotes = true,
            '"' if in_quotes => in_quotes = false,
            '<' if !in_quotes => depth += 1,
            '>' if !in_quotes => depth -= 1,
            c if c == sep && !in_quotes && depth == 0 => {
                out.push(&input[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&input[start..]);
    out
}

fn unquote(s: &str) -> String {
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        return s[1..s.len() - 1].replace("\\\"", "\"");
    }
    s.to_string()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn encode_simple_link_format() {
        let s = encode_links(&[CoreLink::new("/sensor")
            .attr("rt", "temperature")
            .attr("ct", "40")]);
        assert_eq!(s, r#"</sensor>;rt="temperature";ct=40"#);
    }

    #[test]
    fn encode_multiple_links_comma_separated() {
        let s = encode_links(&[
            CoreLink::new("/a").attr("rt", "x"),
            CoreLink::new("/b").attr("rt", "y"),
        ]);
        assert_eq!(s, r#"</a>;rt="x",</b>;rt="y""#);
    }

    #[test]
    fn decode_round_trip_with_attrs() {
        let links = vec![
            CoreLink::new("/sensor/0")
                .attr("rt", "temperature")
                .attr("if", "core.s"),
        ];
        let s = encode_links(&links);
        let back = decode_links(&s).unwrap();
        assert_eq!(back, links);
    }

    #[test]
    fn decode_handles_numeric_attr() {
        let s = "</p>;ct=40";
        let links = decode_links(s).unwrap();
        assert_eq!(links[0].uri, "/p");
        assert_eq!(links[0].get("ct"), Some("40"));
    }

    #[test]
    fn decode_handles_quoted_string_with_comma() {
        let s = r#"</p>;title="hello, world""#;
        let links = decode_links(s).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].get("title"), Some("hello, world"));
    }

    #[test]
    fn decode_rejects_uri_without_brackets() {
        let s = "/path;rt=x";
        assert!(decode_links(s).is_err());
    }

    #[test]
    fn decode_attr_without_value_yields_empty() {
        let s = "</p>;hidden";
        let links = decode_links(s).unwrap();
        assert_eq!(links[0].get("hidden"), Some(""));
    }

    #[test]
    fn encode_escapes_inner_quotes() {
        let s = encode_links(&[CoreLink::new("/p").attr("title", r#"a"b"#)]);
        assert!(s.contains(r#"\""#));
    }
}
