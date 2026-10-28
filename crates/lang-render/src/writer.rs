// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Indentation and line breaking. Spec §5.
//!
//! The writer knows how deep it is and what a line ends with. It knows
//! nothing about any language: a writer that knew would be a second
//! place where output is decided.

use alloc::string::{String, ToString};

/// Accumulates rendered text with an indentation level.
#[derive(Debug, Clone)]
pub struct Writer {
    out: String,
    depth: usize,
    unit: &'static str,
    /// Whether the current line has had anything written to it.
    line_started: bool,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new("    ")
    }
}

impl Writer {
    /// A writer indenting by `unit` per level.
    #[must_use]
    pub fn new(unit: &'static str) -> Self {
        Self {
            out: String::new(),
            depth: 0,
            unit,
            line_started: false,
        }
    }

    /// A writer indenting by four spaces, which is what Rust, C++ and
    /// Java conventionally use and what the previous emitters wrote.
    #[must_use]
    pub fn spaces4() -> Self {
        Self::new("    ")
    }

    /// Go one level deeper.
    pub fn indent(&mut self) {
        self.depth = self.depth.saturating_add(1);
    }

    /// Come back one level.
    pub fn dedent(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// How deep it currently is, so a caller can assert on balance.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Write text, indenting first when the line is still empty.
    pub fn push(&mut self, text: &str) {
        if !self.line_started && !text.is_empty() {
            for _ in 0..self.depth {
                self.out.push_str(self.unit);
            }
            self.line_started = true;
        }
        self.out.push_str(text);
    }

    /// Write text and end the line.
    pub fn line(&mut self, text: &str) {
        self.push(text);
        self.newline();
    }

    /// End the line.
    ///
    /// A line with nothing on it gets no indentation: trailing
    /// whitespace on a blank line is what a formatter strips and what a
    /// byte-comparison then reports as a difference.
    pub fn newline(&mut self) {
        self.out.push('\n');
        self.line_started = false;
    }

    /// End the line only if something has been written to it.
    ///
    /// For a renderer that does not know whether the previous node left
    /// the line open.
    pub fn finish_line(&mut self) {
        if self.line_started {
            self.newline();
        }
    }

    /// An empty line, unless the output is empty or already ends in one.
    ///
    /// Idempotent so that a renderer separating two optional sections
    /// does not have to know whether the first was there.
    pub fn blank_line(&mut self) {
        self.finish_line();
        if self.out.is_empty() || self.out.ends_with("\n\n") {
            return;
        }
        self.out.push('\n');
    }

    /// Whether nothing has been written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.out.is_empty()
    }

    /// The text written so far.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.out
    }

    /// The text, with the writer consumed.
    #[must_use]
    pub fn finish(mut self) -> String {
        self.finish_line();
        self.out
    }
}

/// Write `@verbatim` text, one output line per text line.
///
/// OMG IDL 4.2 §8.3.5.1: the annotation's `text` is copied into the
/// generated artifact unchanged — no comment marker, no escaping. The
/// one transformation is the writer's own indentation, so text placed
/// inside a class body lines up with that body; at file level the
/// indentation is zero and the copy is byte-for-byte.
pub fn verbatim_lines(writer: &mut Writer, text: &str) {
    for line in text.lines() {
        writer.line(line);
    }
}

/// Write a comment block, one line per entry, each with `marker`.
///
/// A line of its own per entry rather than one string: a comment with an
/// embedded newline would put the indentation of its second line beyond
/// the writer's reach.
pub fn comment_lines(writer: &mut Writer, marker: &str, lines: &[String]) {
    for line in lines {
        if line.is_empty() {
            writer.line(marker.trim_end());
        } else {
            let mut text = marker.to_string();
            text.push_str(line);
            writer.line(&text);
        }
    }
}

/// Escape a string for a C-family string literal.
///
/// The four languages agree on these escapes; where they do not — a
/// Rust raw string, a Java unicode escape — the renderer that needs the
/// difference does it itself.
#[must_use]
pub fn escape_string(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            other => out.push(other),
        }
    }
    out
}

/// Escape a character for a C-family character literal.
#[must_use]
pub fn escape_char(ch: char) -> String {
    match ch {
        '\'' => "\\'".to_string(),
        '\\' => "\\\\".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        '\0' => "\\0".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn indentation_applies_once_per_line_not_per_write() {
        let mut w = Writer::spaces4();
        w.indent();
        w.push("a");
        w.push("b");
        w.newline();
        assert_eq!(w.finish(), "    ab\n");
    }

    #[test]
    fn a_blank_line_carries_no_trailing_whitespace() {
        // Trailing whitespace on a blank line is what a formatter strips
        // and what a byte comparison then reports as a difference.
        let mut w = Writer::spaces4();
        w.indent();
        w.line("a");
        w.newline();
        w.line("b");
        assert_eq!(w.finish(), "    a\n\n    b\n");
    }

    #[test]
    fn a_blank_line_does_not_double_up() {
        let mut w = Writer::spaces4();
        w.line("a");
        w.blank_line();
        w.blank_line();
        w.line("b");
        assert_eq!(w.finish(), "a\n\nb\n");
    }

    #[test]
    fn a_blank_line_at_the_start_writes_nothing() {
        let mut w = Writer::spaces4();
        w.blank_line();
        w.line("a");
        assert_eq!(w.finish(), "a\n");
    }

    #[test]
    fn dedent_below_zero_stays_at_zero() {
        // A renderer with an unbalanced dedent is a defect; underflowing
        // the depth would turn it into a panic somewhere else entirely.
        let mut w = Writer::spaces4();
        w.dedent();
        w.dedent();
        assert_eq!(w.depth(), 0);
        w.line("a");
        assert_eq!(w.finish(), "a\n");
    }

    #[test]
    fn string_escapes_cover_the_ones_a_generated_literal_meets() {
        assert_eq!(escape_string("a\"b"), "a\\\"b");
        assert_eq!(escape_string("a\\b"), "a\\\\b");
        assert_eq!(escape_string("a\nb"), "a\\nb");
        assert_eq!(escape_string("plain"), "plain");
    }

    #[test]
    fn a_comment_block_marks_every_line() {
        let mut w = Writer::spaces4();
        comment_lines(
            &mut w,
            "/// ",
            &["first".to_string(), String::new(), "third".to_string()],
        );
        assert_eq!(w.finish(), "/// first\n///\n/// third\n");
    }
}
