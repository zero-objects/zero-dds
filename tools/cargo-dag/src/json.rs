// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Minimal JSON reader for the `cargo metadata` document.
//!
//! `zerodds-cargo-dag` is a release-critical tool that must build from the
//! bare workspace with no registry access, so it carries no dependencies.
//! This module is the whole JSON surface it needs: the document produced by
//! `cargo metadata --format-version 1 --no-deps` is read into a [`Value`]
//! tree, from which `lib.rs` picks out `packages[].name`,
//! `packages[].manifest_path`, `packages[].publish` and
//! `packages[].dependencies[]`.
//!
//! The reader is a complete RFC 8259 value parser (objects, arrays, strings
//! with `\uXXXX` escapes and surrogate pairs, numbers, `true`/`false`/`null`)
//! — not a substring heuristic. Trailing content after the top-level value is
//! an error, so a truncated document is rejected rather than half-read.

use std::collections::BTreeMap;

/// A parsed JSON value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    /// `null`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// Any JSON number, as an `f64`.
    Number(f64),
    /// A string, with all escapes resolved.
    String(String),
    /// An array.
    Array(Vec<Value>),
    /// An object. Duplicate keys resolve to the last occurrence.
    Object(BTreeMap<String, Value>),
}

impl Value {
    /// Member of an object, or `None` for a non-object / absent key.
    pub(crate) fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Object(map) => map.get(key),
            _ => None,
        }
    }

    /// The string content, or `None` if this is not a string.
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The elements, or `None` if this is not an array.
    pub(crate) fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(v) => Some(v.as_slice()),
            _ => None,
        }
    }

    /// `true` if this is `null` or the key was absent.
    pub(crate) fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

/// Reads one JSON document.
///
/// # Errors
/// A human-readable message naming the byte offset of the first problem.
pub(crate) fn parse(input: &str) -> Result<Value, String> {
    let mut p = Parser {
        bytes: input.as_bytes(),
        pos: 0,
    };
    p.skip_whitespace();
    let value = p.value()?;
    p.skip_whitespace();
    if p.pos != p.bytes.len() {
        return Err(format!("trailing content at byte {}", p.pos));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "expected `{}` at byte {}",
                char::from(byte),
                self.pos
            ))
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(format!("expected `{word}` at byte {}", self.pos))
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(_) => self.number(),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn object(&mut self) -> Result<Value, String> {
        self.expect(b'{')?;
        let mut map = BTreeMap::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.skip_whitespace();
            let key = self.string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            let value = self.value()?;
            map.insert(key, value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(map));
                }
                _ => return Err(format!("expected `,` or `}}` at byte {}", self.pos)),
            }
        }
    }

    fn array(&mut self) -> Result<Value, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_whitespace();
            items.push(self.value()?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(format!("expected `,` or `]` at byte {}", self.pos)),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        let mut run_start = self.pos;
        loop {
            let Some(b) = self.peek() else {
                return Err("unterminated string".to_string());
            };
            match b {
                b'"' => {
                    out.push_str(self.str_slice(run_start, self.pos)?);
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    out.push_str(self.str_slice(run_start, self.pos)?);
                    self.pos += 1;
                    let ch = self.escape()?;
                    out.push(ch);
                    run_start = self.pos;
                }
                _ => self.pos += 1,
            }
        }
    }

    fn str_slice(&self, from: usize, to: usize) -> Result<&str, String> {
        std::str::from_utf8(&self.bytes[from..to])
            .map_err(|e| format!("invalid utf-8 at byte {from}: {e}"))
    }

    fn escape(&mut self) -> Result<char, String> {
        let Some(b) = self.peek() else {
            return Err("unterminated escape".to_string());
        };
        self.pos += 1;
        let ch = match b {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => return self.unicode_escape(),
            other => {
                return Err(format!(
                    "unknown escape `\\{}` at byte {}",
                    char::from(other),
                    self.pos - 1
                ));
            }
        };
        Ok(ch)
    }

    fn unicode_escape(&mut self) -> Result<char, String> {
        let first = self.hex4()?;
        // A high surrogate must be followed by `\uXXXX` with its low half.
        if (0xD800..0xDC00).contains(&first) {
            if !self.bytes[self.pos..].starts_with(b"\\u") {
                return Err(format!("lone high surrogate at byte {}", self.pos));
            }
            self.pos += 2;
            let second = self.hex4()?;
            if !(0xDC00..0xE000).contains(&second) {
                return Err(format!("invalid low surrogate at byte {}", self.pos));
            }
            let combined = 0x1_0000 + ((first - 0xD800) << 10) + (second - 0xDC00);
            return char::from_u32(combined)
                .ok_or_else(|| format!("invalid code point at byte {}", self.pos));
        }
        char::from_u32(first).ok_or_else(|| format!("invalid code point at byte {}", self.pos))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let end = self.pos + 4;
        if end > self.bytes.len() {
            return Err("truncated `\\u` escape".to_string());
        }
        let digits = self.str_slice(self.pos, end)?;
        let value = u32::from_str_radix(digits, 16)
            .map_err(|e| format!("bad `\\u` escape at byte {}: {e}", self.pos))?;
        self.pos = end;
        Ok(value)
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(
            self.peek(),
            Some(b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
        ) {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(format!("unexpected byte at {start}"));
        }
        let text = self.str_slice(start, self.pos)?;
        text.parse::<f64>()
            .map(Value::Number)
            .map_err(|e| format!("bad number at byte {start}: {e}"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests may use unwrap.
mod tests {
    use super::*;

    #[test]
    fn parses_the_shape_cargo_metadata_emits() {
        let doc = r#"{"packages":[{"name":"a","publish":null,"dependencies":[]},
                       {"name":"b","publish":[],"dependencies":[{"name":"a","kind":null}]}],
                      "version":1}"#;
        let v = parse(doc).unwrap();
        let pkgs = v.get("packages").unwrap().as_array().unwrap();
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].get("name").unwrap().as_str(), Some("a"));
        assert!(pkgs[0].get("publish").unwrap().is_null());
        assert_eq!(pkgs[1].get("publish").unwrap().as_array(), Some(&[][..]));
        let deps = pkgs[1].get("dependencies").unwrap().as_array().unwrap();
        assert_eq!(deps[0].get("name").unwrap().as_str(), Some("a"));
        assert!(deps[0].get("kind").unwrap().is_null());
        assert_eq!(v.get("version"), Some(&Value::Number(1.0)));
    }

    #[test]
    fn parses_escapes_and_surrogate_pairs() {
        let v = parse(r#""a\"b\\c\/d\neäf😀""#).unwrap();
        assert_eq!(v.as_str(), Some("a\"b\\c/d\ne\u{e4}f\u{1f600}"));
    }

    #[test]
    fn parses_empty_containers_and_nesting() {
        assert_eq!(parse("[]").unwrap(), Value::Array(vec![]));
        assert_eq!(parse("{}").unwrap(), Value::Object(BTreeMap::new()));
        let v = parse(r#"{"a":{"b":[1,{"c":true},null]}}"#).unwrap();
        let inner = v.get("a").unwrap().get("b").unwrap().as_array().unwrap();
        assert_eq!(inner.len(), 3);
        assert_eq!(inner[1].get("c"), Some(&Value::Bool(true)));
        assert!(inner[2].is_null());
    }

    #[test]
    fn parses_negative_and_exponent_numbers() {
        assert_eq!(parse("-1.5e3").unwrap(), Value::Number(-1500.0));
    }

    #[test]
    fn rejects_truncated_and_trailing_input() {
        assert!(parse(r#"{"a":1"#).is_err());
        assert!(parse(r#""abc"#).is_err());
        assert!(parse("[1,2] junk").is_err());
        assert!(parse(r#"{"a":1,}"#).is_err());
        assert!(parse(r#""\ud83d""#).is_err());
    }
}
