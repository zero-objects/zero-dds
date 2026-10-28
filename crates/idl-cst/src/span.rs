// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Byte ranges into source text. Spec §1, §7.

use core::fmt;

/// A half-open byte range `[start, end)` into one source file.
///
/// Bytes, not characters: a diagnostic has to point into the file a
/// reader opens, and an editor counts bytes. Line and column are derived
/// on demand rather than carried, because most spans are never printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Span {
    /// First byte.
    pub start: u32,
    /// One past the last byte.
    pub end: u32,
}

impl Span {
    /// A range from `start` to `end`.
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// An empty range at `at`, for pointing at a place rather than at
    /// text — a missing semicolon has no bytes of its own.
    #[must_use]
    pub const fn empty_at(at: u32) -> Self {
        Self { start: at, end: at }
    }

    /// How many bytes the range covers.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Whether the range covers no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.end <= self.start
    }

    /// The smallest range covering both.
    #[must_use]
    pub const fn cover(self, other: Self) -> Self {
        Self {
            start: if self.start < other.start {
                self.start
            } else {
                other.start
            },
            end: if self.end > other.end {
                self.end
            } else {
                other.end
            },
        }
    }

    /// Whether `at` falls inside the range.
    #[must_use]
    pub const fn contains(self, at: u32) -> bool {
        self.start <= at && at < self.end
    }

    /// The text this range covers, if it is in bounds.
    #[must_use]
    pub fn slice(self, source: &str) -> Option<&str> {
        source.get(self.start as usize..self.end as usize)
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// A one-based line and column, derived from a span on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineColumn {
    /// One-based line.
    pub line: u32,
    /// One-based column, counted in characters rather than bytes so it
    /// matches what an editor shows.
    pub column: u32,
}

impl LineColumn {
    /// Where `offset` falls in `source`.
    ///
    /// Linear in the file; callers that print many diagnostics for one
    /// file should build a line table instead.
    #[must_use]
    pub fn of(source: &str, offset: u32) -> Self {
        let mut line = 1u32;
        let mut column = 1u32;
        for (index, ch) in source.char_indices() {
            if index as u32 >= offset {
                break;
            }
            if ch == '\n' {
                line = line.saturating_add(1);
                column = 1;
            } else {
                column = column.saturating_add(1);
            }
        }
        Self { line, column }
    }
}

impl fmt::Display for LineColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}
