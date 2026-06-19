// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate-wide diagnostics scaffolding.
//!
//! Three layers:
//!
//! - [`Span`] — byte-offset range in the source text. Small, `Copy`,
//!   trivially mergeable.
//! - [`Diagnostic`] — structured error/warning message with a
//!   primary span, optional secondary spans (rustc-inspired).
//! - [`ParseError`] — domain-specific error family that is later
//!   consumed by the lexer (Task 2.x) and the engine; `to_diagnostic`
//!   converts into the uniform display format.
//!
//! This layer is scaffolding: the data types are in place, but the lexer and
//! engine do not yet produce [`ParseError`] values. The wiring
//! happens incrementally in week 2, when the lexer works with spans
//! and the recognizer is switched to token wrappers (instead of bare [`crate::grammar::TokenKind`]).
//!
//! See RFC 0001 §5.5 (error reporting).

use core::fmt;

use crate::grammar::TokenKind;

/// Byte-offset range `[start, end)` in the source text.
///
/// Half-open interval: `start` inclusive, `end` exclusive. A
/// zero-length span (`start == end`) marks a position without content
/// — typical for an EOF caret or insertion suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Span {
    /// Start position (inclusive, byte offset).
    pub start: usize,
    /// End position (exclusive, byte offset).
    pub end: usize,
}

impl Span {
    /// Synthetic span without a source location. Used for internal, non-source-based
    /// diagnostics (e.g. grammar construction errors).
    pub const SYNTHETIC: Self = Self { start: 0, end: 0 };

    /// Constructs a new span from a start and end offset.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Span with length 0 at a position (e.g. EOF marker).
    #[must_use]
    pub const fn point(at: usize) -> Self {
        Self { start: at, end: at }
    }

    /// Number of bytes covered.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// `true` if start == end (zero-length).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the smallest span covering both. Symmetric.
    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
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

    /// `true` if `byte_offset` lies within this span.
    #[must_use]
    pub const fn contains_offset(&self, byte_offset: usize) -> bool {
        byte_offset >= self.start && byte_offset < self.end
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// Severity of a [`Diagnostic`]. Corresponds to the rustc standard levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// Helpful hint (e.g. a suggested correction).
    Help,
    /// Contextual note relating to another diagnostic.
    Note,
    /// Warning — the code runs, but could be problematic.
    Warning,
    /// Error — processing cannot continue.
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Help => "help",
            Self::Note => "note",
            Self::Warning => "warning",
            Self::Error => "error",
        };
        f.write_str(label)
    }
}

/// Additional span with explanatory text (secondary anchor of a
/// [`Diagnostic`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// Span in the source text.
    pub span: Span,
    /// Explanatory text (shown next to the span).
    pub message: String,
}

/// Structured diagnostic message.
///
/// Modeled after rustc diagnostics: a severity, a main message,
/// a primary span and any number of additional labels with their own text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Severity.
    pub severity: Severity,
    /// Compact text (one line, no trailing period).
    pub message: String,
    /// Primary anchor in the source text.
    pub primary_span: Span,
    /// Additional anchors with explanations.
    pub labels: Vec<Label>,
}

impl Diagnostic {
    /// Constructor for error diagnostics.
    #[must_use]
    pub fn error(message: impl Into<String>, primary_span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
        }
    }

    /// Constructor for warning diagnostics.
    #[must_use]
    pub fn warning(message: impl Into<String>, primary_span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
        }
    }

    /// Builder method: append an additional label.
    #[must_use]
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
        });
        self
    }
}

/// Domain-specific parser errors.
///
/// Consumed by the lexer (week 2), recognizer (later wiring), CST builder
/// and AST builder. Each variant can be converted via
/// [`ParseError::to_diagnostic`] into a [`Diagnostic`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// A specific set of tokens was expected, but a different token
    /// was found.
    UnexpectedToken {
        /// What was actually read.
        found: TokenKind,
        /// Which tokens were expected (from the Earley expected-set).
        expected: Vec<TokenKind>,
        /// Position of the unexpected token.
        span: Span,
    },
    /// Input ended before the production was complete. At least one more
    /// token was expected.
    UnexpectedEof {
        /// What was expected at this position.
        expected: Vec<TokenKind>,
        /// End position of the input.
        span: Span,
    },
    /// Lexer-specific error (invalid character sequence, unterminated
    /// string, etc.). The detail text is lexer-specific.
    LexerError {
        /// Description of the lexer problem.
        message: String,
        /// Position of the faulty sequence.
        span: Span,
    },
}

impl ParseError {
    /// Converts a parser error into a uniform [`Diagnostic`].
    #[must_use]
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            Self::UnexpectedToken {
                found,
                expected,
                span,
            } => {
                let msg = format!(
                    "unexpected token {:?}, expected {}",
                    found,
                    format_expected(expected),
                );
                Diagnostic::error(msg, *span)
            }
            Self::UnexpectedEof { expected, span } => {
                let msg = format!(
                    "unexpected end of input, expected {}",
                    format_expected(expected),
                );
                Diagnostic::error(msg, *span)
            }
            Self::LexerError { message, span } => Diagnostic::error(message.clone(), *span),
        }
    }

    /// Position of the error in the source text.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::UnexpectedToken { span, .. }
            | Self::UnexpectedEof { span, .. }
            | Self::LexerError { span, .. } => *span,
        }
    }
}

/// Formats a list of expected tokens for display in
/// diagnostic texts.
fn format_expected(expected: &[TokenKind]) -> String {
    match expected {
        [] => "(nothing)".to_string(),
        [single] => format!("{single:?}"),
        many => {
            let formatted: Vec<String> = many.iter().map(|t| format!("{t:?}")).collect();
            format!("one of [{}]", formatted.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------
    // Span
    // -----------------------------------------------------------------

    #[test]
    fn span_new_stores_start_and_end() {
        let s = Span::new(3, 7);
        assert_eq!(s.start, 3);
        assert_eq!(s.end, 7);
    }

    #[test]
    fn span_point_has_zero_length() {
        let s = Span::point(42);
        assert_eq!(s.start, 42);
        assert_eq!(s.end, 42);
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn span_len_counts_bytes() {
        assert_eq!(Span::new(0, 5).len(), 5);
        assert_eq!(Span::new(10, 14).len(), 4);
    }

    #[test]
    fn span_merge_takes_outer_bounds() {
        let a = Span::new(2, 5);
        let b = Span::new(4, 8);
        assert_eq!(a.merge(b), Span::new(2, 8));
        // Symmetrisch.
        assert_eq!(b.merge(a), Span::new(2, 8));
    }

    #[test]
    fn span_merge_with_synthetic_keeps_real_bounds() {
        let real = Span::new(10, 20);
        // SYNTHETIC is 0..0; merge takes min(start)=0 — that is semantically
        // expected (synthetic widens to 0). As a documentation anchor.
        let merged = real.merge(Span::SYNTHETIC);
        assert_eq!(merged, Span::new(0, 20));
    }

    #[test]
    fn span_contains_offset_is_half_open() {
        let s = Span::new(5, 10);
        assert!(s.contains_offset(5));
        assert!(s.contains_offset(7));
        assert!(s.contains_offset(9));
        assert!(!s.contains_offset(10), "end ist exklusiv");
        assert!(!s.contains_offset(4));
    }

    #[test]
    fn span_displays_as_start_dot_dot_end() {
        assert_eq!(format!("{}", Span::new(3, 7)), "3..7");
    }

    // -----------------------------------------------------------------
    // Severity
    // -----------------------------------------------------------------

    #[test]
    fn severity_orders_help_lower_than_error() {
        assert!(Severity::Help < Severity::Note);
        assert!(Severity::Note < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn severity_displays_lowercase_label() {
        assert_eq!(format!("{}", Severity::Error), "error");
        assert_eq!(format!("{}", Severity::Warning), "warning");
        assert_eq!(format!("{}", Severity::Note), "note");
        assert_eq!(format!("{}", Severity::Help), "help");
    }

    // -----------------------------------------------------------------
    // Diagnostic
    // -----------------------------------------------------------------

    #[test]
    fn diagnostic_error_constructor_sets_severity() {
        let d = Diagnostic::error("oops", Span::new(0, 3));
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.message, "oops");
        assert_eq!(d.primary_span, Span::new(0, 3));
        assert!(d.labels.is_empty());
    }

    #[test]
    fn diagnostic_warning_constructor_sets_severity() {
        let d = Diagnostic::warning("hmm", Span::new(2, 4));
        assert_eq!(d.severity, Severity::Warning);
    }

    #[test]
    fn diagnostic_with_label_appends_in_order() {
        let d = Diagnostic::error("bad", Span::new(0, 3))
            .with_label(Span::new(5, 10), "see here")
            .with_label(Span::new(20, 22), "and here");
        assert_eq!(d.labels.len(), 2);
        assert_eq!(d.labels[0].message, "see here");
        assert_eq!(d.labels[0].span, Span::new(5, 10));
        assert_eq!(d.labels[1].message, "and here");
    }

    // -----------------------------------------------------------------
    // ParseError
    // -----------------------------------------------------------------

    #[test]
    fn parse_error_unexpected_token_to_diagnostic() {
        let err = ParseError::UnexpectedToken {
            found: TokenKind::Keyword("struct"),
            expected: vec![TokenKind::Punct("{")],
            span: Span::new(7, 13),
        };
        let d = err.to_diagnostic();
        assert_eq!(d.severity, Severity::Error);
        assert!(d.message.contains("unexpected token"));
        assert!(d.message.contains("struct"));
        assert!(d.message.contains("{"));
        assert_eq!(d.primary_span, Span::new(7, 13));
    }

    #[test]
    fn parse_error_unexpected_eof_to_diagnostic() {
        let err = ParseError::UnexpectedEof {
            expected: vec![TokenKind::Punct(";")],
            span: Span::point(50),
        };
        let d = err.to_diagnostic();
        assert!(d.message.contains("end of input"));
        assert_eq!(d.primary_span, Span::point(50));
    }

    #[test]
    fn parse_error_lexer_to_diagnostic_uses_message() {
        let err = ParseError::LexerError {
            message: "unterminated string literal".to_string(),
            span: Span::new(10, 12),
        };
        let d = err.to_diagnostic();
        assert_eq!(d.message, "unterminated string literal");
    }

    #[test]
    fn parse_error_span_accessor_returns_inner_span() {
        let err = ParseError::UnexpectedToken {
            found: TokenKind::Ident,
            expected: vec![],
            span: Span::new(3, 6),
        };
        assert_eq!(err.span(), Span::new(3, 6));
    }

    #[test]
    fn format_expected_handles_empty_single_and_many() {
        assert_eq!(format_expected(&[]), "(nothing)");
        assert_eq!(format_expected(&[TokenKind::Ident]), "Ident");
        let many = format_expected(&[TokenKind::Punct(";"), TokenKind::Punct(",")]);
        assert!(many.contains("one of"));
        assert!(many.contains(';'));
        assert!(many.contains(','));
    }
}
