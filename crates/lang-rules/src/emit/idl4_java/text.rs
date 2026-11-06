// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Java source text: literals and an indenting writer.

use core::fmt::Write as _;

/// Java's escape of one UTF-16 code unit inside a literal (JLS §3.10.6):
/// the printable ASCII characters as they are, the quotes and the
/// backslash escaped, a control character as an octal escape, everything
/// else as `\uXXXX`.
///
/// Not `\uXXXX` below `0x80`: JLS §3.3 translates a Unicode escape before
/// the literal is read, so `\u000a` ends the line inside the literal and
/// `\u0022` closes it.
fn escape_unit(unit: u16, quote: char, out: &mut String) {
    match unit {
        0x5C => out.push_str("\\\\"),
        0x22 if quote == '"' => out.push_str("\\\""),
        0x27 if quote == '\'' => out.push_str("\\'"),
        0x20..=0x7E => out.push(char::from(u8::try_from(unit).unwrap_or(b'?'))),
        0x00..=0x1F | 0x7F => {
            let _ = write!(out, "\\{unit:03o}");
        }
        _ => {
            let _ = write!(out, "\\u{unit:04x}");
        }
    }
}

/// A Java string literal holding `text`.
#[must_use]
pub fn string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for unit in text.encode_utf16() {
        escape_unit(unit, '"', &mut out);
    }
    out.push('"');
    out
}

/// A Java string literal holding the UTF-16 code units `units`, which may
/// be lone surrogates (an IDL `wstring` is a run of code units).
#[must_use]
pub fn string_literal_units(units: &[u16]) -> String {
    let mut out = String::from("\"");
    for unit in units {
        escape_unit(*unit, '"', &mut out);
    }
    out.push('"');
    out
}

/// A Java character literal holding the code unit `unit`.
#[must_use]
pub fn char_literal(unit: u16) -> String {
    let mut out = String::from("'");
    escape_unit(unit, '\'', &mut out);
    out.push('\'');
    out
}

/// Source text built line by line at an indentation of four spaces per
/// level.
#[derive(Debug, Default)]
pub struct Writer {
    out: String,
    level: usize,
}

impl Writer {
    /// A writer starting at level zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// One line at the current level; an empty `text` is an empty line.
    pub fn line(&mut self, text: &str) {
        if !text.is_empty() {
            for _ in 0..self.level {
                self.out.push_str("    ");
            }
            self.out.push_str(text);
        }
        self.out.push('\n');
    }

    /// `text {` and one level in.
    pub fn open(&mut self, text: &str) {
        self.line(&format!("{text} {{"));
        self.level += 1;
    }

    /// One level out and `}` followed by `tail`.
    pub fn close_with(&mut self, tail: &str) {
        self.level = self.level.saturating_sub(1);
        self.line(&format!("}}{tail}"));
    }

    /// One level out and `}`.
    pub fn close(&mut self) {
        self.close_with("");
    }

    /// Lines written verbatim (a `@verbatim` text), each at the current
    /// level.
    pub fn verbatim(&mut self, text: &str) {
        for line in text.lines() {
            self.line(line);
        }
    }

    /// Text built elsewhere, each of its lines at the current level.
    pub fn nested(&mut self, text: &str) {
        for line in text.lines() {
            self.line(line);
        }
    }

    /// The text.
    #[must_use]
    pub fn finish(self) -> String {
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_escape_what_java_needs_escaped() {
        assert_eq!(string_literal("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(string_literal("ä\n"), "\"\\u00e4\\012\"");
        assert_eq!(char_literal(u16::from(b'\'')), "'\\''");
        assert_eq!(char_literal(0xD800), "'\\ud800'");
        assert_eq!(string_literal_units(&[0x61, 0xDC00]), "\"a\\udc00\"");
    }
}
