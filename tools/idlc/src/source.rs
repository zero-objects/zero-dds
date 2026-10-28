// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Reading a unit, and saying where in it something is.
//!
//! A diagnostic carries a byte range. A person reads line and column, so
//! the translation lives here and in one place: every subcommand that
//! prints anything goes through [`Location::of`], and none of them counts
//! newlines itself.

use zerodds_idl_cst::Span;

/// One IDL unit, with its text kept for reporting.
pub struct Unit {
    /// The path as written on the command line, which is what a message
    /// names — not a canonicalised one the caller did not type.
    pub path: String,
    /// The bytes, exactly as read.
    pub text: String,
}

impl Unit {
    /// Read `path`.
    ///
    /// # Errors
    /// The I/O error, with the path in it: a bare "No such file or
    /// directory" from a tool that was given six paths says nothing.
    pub fn read(path: &str) -> Result<Self, String> {
        std::fs::read_to_string(path)
            .map(|text| Self {
                path: path.to_string(),
                text,
            })
            .map_err(|error| format!("{path}: {error}"))
    }
}

/// A one-based line and column in a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// One-based line.
    pub line: usize,
    /// One-based column, counted in characters and not in bytes.
    pub column: usize,
}

impl Location {
    /// Where `offset` is in `text`.
    ///
    /// The column counts **characters**, because a person reading an
    /// editor's ruler counts those: a `//` comment holding a `ä` before
    /// the offending token would otherwise report a column one further
    /// right than the editor shows. An offset past the end lands on the
    /// last line, which is where a truncated file's error is.
    #[must_use]
    pub fn of(text: &str, offset: u32) -> Self {
        let offset = (offset as usize).min(text.len());
        let before = text.get(..offset).unwrap_or(text);
        let line = before.matches('\n').count() + 1;
        let column = before
            .rfind('\n')
            .map_or(before, |at| before.get(at + 1..).unwrap_or(""))
            .chars()
            .count()
            + 1;
        Self { line, column }
    }

    /// Where a span starts.
    #[must_use]
    pub fn start_of(text: &str, span: Span) -> Self {
        Self::of(text, span.start)
    }
}

impl core::fmt::Display for Location {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    #[test]
    fn the_first_byte_is_line_one_column_one() {
        assert_eq!(
            Location::of("struct S;", 0),
            Location { line: 1, column: 1 }
        );
    }

    #[test]
    fn a_newline_starts_the_next_line() {
        let text = "module m {\n  struct S;\n};";
        let at = u32::try_from(text.find("struct").expect("present")).expect("small");
        assert_eq!(Location::of(text, at), Location { line: 2, column: 3 });
    }

    /// The column is characters and not bytes: an editor's ruler counts
    /// what it draws.
    #[test]
    fn a_multibyte_character_advances_the_column_by_one() {
        let text = "// ä\nstruct S;";
        let at = u32::try_from(text.find("struct").expect("present")).expect("small");
        assert_eq!(Location::of(text, at), Location { line: 2, column: 1 });
        let inside = u32::try_from(text.find('\n').expect("present")).expect("small");
        assert_eq!(
            Location::of(text, inside),
            Location { line: 1, column: 5 },
            "`// ä` is four characters, so the newline sits at column five"
        );
    }

    /// A truncated unit's diagnostic can point one past the end. It
    /// reports the last position rather than panicking on a slice.
    #[test]
    fn an_offset_past_the_end_lands_on_the_last_line() {
        let text = "struct S {";
        let got = Location::of(text, 9_999);
        assert_eq!(got.line, 1);
        assert_eq!(got.column, text.len() + 1);
    }

    #[test]
    fn an_empty_unit_is_line_one_column_one() {
        assert_eq!(Location::of("", 0), Location { line: 1, column: 1 });
    }
}
