// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate-weites Diagnostik-Grundgeruest.
//!
//! Drei Schichten:
//!
//! - [`Span`] — Byte-Offset-Bereich im Source-Text. Klein, `Copy`,
//!   trivial mergebar.
//! - [`Diagnostic`] — strukturierte Fehler-/Warnungs-Meldung mit
//!   primary span, optional secondary spans (rustc-inspiriert).
//! - [`ParseError`] — domaenenspezifische Fehler-Familie, die spaeter
//!   vom Lexer (Task 2.x) und der Engine konsumiert wird; `to_diagnostic`
//!   konvertiert in das uniforme Anzeige-Format.
//!
//! Diese Schicht ist Grundgeruest: die Datentypen stehen, aber Lexer und
//! Engine produzieren noch keine [`ParseError`]-Werte. Die Anbindung
//! erfolgt schrittweise in Woche 2, wenn der Lexer mit Spans arbeitet
//! und der Recognizer auf Token-Wrapper (statt nackten [`crate::grammar::TokenKind`])
//! umgestellt wird.
//!
//! Siehe RFC 0001 §5.5 (Error-Reporting).

use core::fmt;

use crate::grammar::TokenKind;

/// Byte-Offset-Bereich `[start, end)` im Source-Text.
///
/// Halb-offenes Intervall: `start` inklusive, `end` exklusive. Eine
/// Zero-length-Span (`start == end`) markiert eine Position ohne Inhalt
/// — typisch fuer EOF-Caret oder Insertion-Vorschlaege.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Span {
    /// Startposition (inklusive, byte-offset).
    pub start: usize,
    /// Endposition (exklusive, byte-offset).
    pub end: usize,
}

impl Span {
    /// Synthetische Span ohne Quellort. Wird fuer interne, nicht-quellbasierte
    /// Diagnostiken verwendet (z.B. Grammar-Konstruktionsfehler).
    pub const SYNTHETIC: Self = Self { start: 0, end: 0 };

    /// Konstruiert eine neue Span aus Start- und End-Offset.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Span mit Laenge 0 an einer Position (z.B. EOF-Marker).
    #[must_use]
    pub const fn point(at: usize) -> Self {
        Self { start: at, end: at }
    }

    /// Anzahl der ueberdeckten Bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// `true`, wenn Start == End (Zero-length).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Liefert die kleinste Span, die beide ueberdeckt. Symmetrisch.
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

    /// `true`, wenn `byte_offset` innerhalb dieser Span liegt.
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

/// Schweregrad einer [`Diagnostic`]. Entspricht den rustc-Standardstufen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// Hilfreicher Hinweis (z.B. Vorschlag einer Korrektur).
    Help,
    /// Kontextuelle Anmerkung zu einer anderen Diagnostik.
    Note,
    /// Warnung — Code laeuft, koennte aber problematisch sein.
    Warning,
    /// Fehler — Verarbeitung kann nicht fortgesetzt werden.
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

/// Zusaetzliche Span mit erlaeuterndem Text (sekundaerer Anker einer
/// [`Diagnostic`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// Span im Source-Text.
    pub span: Span,
    /// Erlaeuterungs-Text (wird neben dem Span angezeigt).
    pub message: String,
}

/// Strukturierte Diagnose-Meldung.
///
/// Modelliert nach rustc-Diagnostics: ein Schweregrad, eine Hauptbotschaft,
/// ein primary-Span und beliebig viele zusaetzliche Labels mit eigenem Text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Schweregrad.
    pub severity: Severity,
    /// Kompakter Text (eine Zeile, ohne Punkt am Ende).
    pub message: String,
    /// Primary-Anker im Source-Text.
    pub primary_span: Span,
    /// Zusaetzliche Anker mit Erlaeuterungen.
    pub labels: Vec<Label>,
}

impl Diagnostic {
    /// Konstruktor fuer Fehler-Diagnostiken.
    #[must_use]
    pub fn error(message: impl Into<String>, primary_span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
        }
    }

    /// Konstruktor fuer Warnung-Diagnostiken.
    #[must_use]
    pub fn warning(message: impl Into<String>, primary_span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
        }
    }

    /// Builder-Methode: zusaetzliches Label anhaengen.
    #[must_use]
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
        });
        self
    }
}

/// Domaenenspezifische Parser-Fehler.
///
/// Wird vom Lexer (Woche 2), Recognizer (spaetere Anbindung), CST-Builder
/// und AST-Builder konsumiert. Jede Variante laesst sich via
/// [`ParseError::to_diagnostic`] in eine [`Diagnostic`] ueberfuehren.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Erwartet wurde eine bestimmte Token-Menge, gefunden wurde ein
    /// anderes Token.
    UnexpectedToken {
        /// Was tatsaechlich gelesen wurde.
        found: TokenKind,
        /// Welche Tokens erwartet waren (aus dem Earley-Expected-Set).
        expected: Vec<TokenKind>,
        /// Position des unerwarteten Tokens.
        span: Span,
    },
    /// Eingabe endete vor Abschluss der Production. Erwartet wurde noch
    /// mindestens ein Token.
    UnexpectedEof {
        /// Was an dieser Stelle erwartet war.
        expected: Vec<TokenKind>,
        /// Endposition der Eingabe.
        span: Span,
    },
    /// Lexer-spezifischer Fehler (ungueltige Zeichen-Sequenz, unterminated
    /// String, etc.). Detail-Text ist lexer-spezifisch.
    LexerError {
        /// Beschreibung des Lexer-Problems.
        message: String,
        /// Position der fehlerhaften Sequenz.
        span: Span,
    },
}

impl ParseError {
    /// Konvertiert einen Parser-Fehler in eine uniforme [`Diagnostic`].
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

    /// Position des Fehlers im Source-Text.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::UnexpectedToken { span, .. }
            | Self::UnexpectedEof { span, .. }
            | Self::LexerError { span, .. } => *span,
        }
    }
}

/// Formatiert eine Liste erwarteter Tokens fuer die Anzeige in
/// Diagnostik-Texten.
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
        // SYNTHETIC ist 0..0; merge nimmt min(start)=0 — das ist semantisch
        // erwartet (synthetic verbreitert nach 0). Als Doku-Anker.
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
