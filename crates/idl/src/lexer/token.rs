// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Lexer-Output-Datentypen: [`Token`] und [`TokenStream`].
//!
//! Ein **Token** ist die kleinste lexikalische Einheit, die der Lexer aus
//! dem Source-Text produziert. Er besteht aus drei Komponenten:
//!
//! - [`crate::grammar::TokenKind`] — Klassifikation, wiederverwendet aus dem
//!   Grammar-Modul (siehe `grammar/mod.rs`). Das stellt sicher, dass die
//!   Lexer-Output-Klassen exakt die Terminal-Symbole sind, gegen die der
//!   Recognizer matcht.
//! - [`crate::errors::Span`] — Position im Source-Text (Byte-Offsets).
//!   Wird durchgereicht in spaetere AST-Nodes und Diagnostiken.
//! - `text: &'src str` — Slice in den Original-Source. Keine Allokation
//!   pro Token; Lifetime `'src` an die Source-String-Bindung gekoppelt.
//!
//! Ein **TokenStream** ist die Sequenz aller Tokens einer Source-Datei,
//! plus Helper fuer Iteration und Cursor-Operationen. Whitespace und
//! Kommentare werden ueblicherweise vom Lexer als Trivia gedroppt — der
//! Stream enthaelt nur signifikante Tokens.
//!
//! Siehe RFC 0001 §4.1 (Pipeline) und §5.x (Lexer-Konzept).

use core::fmt;

use crate::errors::Span;
use crate::grammar::TokenKind;

/// Ein einzelner Lexer-Token.
///
/// `'src` ist die Lifetime des zugrundeliegenden Source-Strings. `text` ist
/// ein Slice in genau diesen String — gueltig fuer die Dauer des Lexer-
/// Outputs und aller nachgelagerten Verarbeitungs-Schritte (Recognition,
/// CST-Bau, AST-Bau), solange der Source-String lebt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token<'src> {
    /// Klassifikation des Tokens (Keyword, Identifier, Literal, etc.).
    pub kind: TokenKind,
    /// Position im Source-Text.
    pub span: Span,
    /// Text-Slice im Original-Source. Bei Keywords/Punctuation deckungs-
    /// gleich mit `kind`-Inhalt; bei Identifiern und Literalen der echte
    /// gelesene Wert.
    pub text: &'src str,
}

impl<'src> Token<'src> {
    /// Konstruiert einen neuen Token.
    #[must_use]
    pub const fn new(kind: TokenKind, span: Span, text: &'src str) -> Self {
        Self { kind, span, text }
    }

    /// Konstruiert einen Token ohne Quellort — `span = SYNTHETIC`, `text = ""`.
    ///
    /// Nuetzlich fuer programmatische Recognition-Aufrufe (Tests, Snippets,
    /// Fixture-Builder), wo nur die Token-Klassifikation interessiert und
    /// keine reale Source vorliegt.
    #[must_use]
    pub const fn synthetic(kind: TokenKind) -> Token<'static> {
        Token {
            kind,
            span: Span::SYNTHETIC,
            text: "",
        }
    }

    /// Laenge des Token-Texts in Bytes (entspricht Span-Laenge).
    #[must_use]
    pub const fn len(&self) -> usize {
        self.span.len()
    }

    /// `true`, wenn der Token-Text leer ist (Span hat Laenge 0).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.span.is_empty()
    }
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}@{} {:?}", self.kind, self.span, self.text)
    }
}

/// Sequenz von [`Token`]s, die der Lexer aus einer Source-Datei erzeugt.
///
/// Trivia (Whitespace, Kommentare) ist nicht enthalten — der Lexer dropt
/// sie. Wenn Source-Preserving Rewrites benoetigt werden (Formatter, CST
/// mit Trivia-Anker), wird das in einer Erweiterung des Lexer-Outputs
/// modelliert (Task 2.x oder Phase 1).
#[derive(Debug, Clone, Default)]
pub struct TokenStream<'src> {
    tokens: Vec<Token<'src>>,
}

impl<'src> TokenStream<'src> {
    /// Neuer leerer Stream.
    #[must_use]
    pub fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    /// Konstruiert einen Stream aus einer existierenden Token-Sequenz.
    #[must_use]
    pub fn from_vec(tokens: Vec<Token<'src>>) -> Self {
        Self { tokens }
    }

    /// Haengt einen Token an das Ende.
    pub fn push(&mut self, token: Token<'src>) {
        self.tokens.push(token);
    }

    /// Alle Tokens als Slice.
    #[must_use]
    pub fn tokens(&self) -> &[Token<'src>] {
        &self.tokens
    }

    /// Iteriert ueber alle Tokens in Reihenfolge.
    pub fn iter(&self) -> impl Iterator<Item = &Token<'src>> {
        self.tokens.iter()
    }

    /// Anzahl der Tokens.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// `true`, wenn der Stream keine Tokens enthaelt.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Greift auf den Token an Position `index` zu.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Token<'src>> {
        self.tokens.get(index)
    }

    /// Sammelt nur die [`TokenKind`]s — Convenience fuer Recognizer-Aufrufe,
    /// die noch auf `&[TokenKind]` arbeiten (Task 2.4 stellt sie um).
    #[must_use]
    pub fn kinds(&self) -> Vec<TokenKind> {
        self.tokens.iter().map(|t| t.kind).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "struct Foo { int32 x; };";

    #[test]
    fn token_new_stores_components() {
        let t = Token::new(TokenKind::Keyword("struct"), Span::new(0, 6), &SRC[0..6]);
        assert_eq!(t.kind, TokenKind::Keyword("struct"));
        assert_eq!(t.span, Span::new(0, 6));
        assert_eq!(t.text, "struct");
    }

    #[test]
    fn token_len_matches_span() {
        let t = Token::new(TokenKind::Ident, Span::new(7, 10), &SRC[7..10]);
        assert_eq!(t.len(), 3);
        assert!(!t.is_empty());
    }

    #[test]
    fn token_is_empty_for_zero_length_span() {
        let t = Token::new(TokenKind::EndOfInput, Span::point(24), "");
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn token_displays_kind_span_and_text() {
        let t = Token::new(TokenKind::Keyword("struct"), Span::new(0, 6), "struct");
        let display = format!("{t}");
        assert!(display.contains("struct"));
        assert!(display.contains("0..6"));
    }

    #[test]
    fn token_stream_starts_empty() {
        let s: TokenStream<'_> = TokenStream::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert!(s.tokens().is_empty());
    }

    #[test]
    fn token_stream_default_equals_new() {
        let a: TokenStream<'_> = TokenStream::default();
        let b: TokenStream<'_> = TokenStream::new();
        assert_eq!(a.len(), b.len());
        assert_eq!(a.is_empty(), b.is_empty());
    }

    #[test]
    fn token_stream_push_appends_in_order() {
        let mut s = TokenStream::new();
        s.push(Token::new(
            TokenKind::Keyword("struct"),
            Span::new(0, 6),
            "struct",
        ));
        s.push(Token::new(TokenKind::Ident, Span::new(7, 10), "Foo"));
        assert_eq!(s.len(), 2);
        assert_eq!(s.tokens()[0].kind, TokenKind::Keyword("struct"));
        assert_eq!(s.tokens()[1].kind, TokenKind::Ident);
    }

    #[test]
    fn token_stream_from_vec_preserves_order() {
        let v = vec![
            Token::new(TokenKind::Keyword("struct"), Span::new(0, 6), "struct"),
            Token::new(TokenKind::Ident, Span::new(7, 10), "Foo"),
            Token::new(TokenKind::Punct("{"), Span::new(11, 12), "{"),
        ];
        let s = TokenStream::from_vec(v);
        assert_eq!(s.len(), 3);
        let kinds: Vec<_> = s.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Keyword("struct"),
                TokenKind::Ident,
                TokenKind::Punct("{"),
            ]
        );
    }

    #[test]
    fn token_stream_get_returns_token_at_index() {
        let s = TokenStream::from_vec(vec![
            Token::new(TokenKind::Ident, Span::new(0, 3), "abc"),
            Token::new(TokenKind::Punct(";"), Span::new(3, 4), ";"),
        ]);
        assert_eq!(s.get(0).map(|t| t.kind), Some(TokenKind::Ident));
        assert_eq!(s.get(1).map(|t| t.kind), Some(TokenKind::Punct(";")));
        assert!(s.get(2).is_none());
    }

    #[test]
    fn token_stream_kinds_returns_kind_only_vec() {
        let s = TokenStream::from_vec(vec![
            Token::new(TokenKind::Keyword("struct"), Span::new(0, 6), "struct"),
            Token::new(TokenKind::Ident, Span::new(7, 10), "Foo"),
        ]);
        assert_eq!(
            s.kinds(),
            vec![TokenKind::Keyword("struct"), TokenKind::Ident],
        );
    }

    #[test]
    fn tokens_borrow_source_string() {
        // Lifetime-Regression: Token::text muss in `src` zeigen.
        let src = String::from("hello world");
        let t = Token::new(TokenKind::Ident, Span::new(0, 5), &src[0..5]);
        assert_eq!(t.text, "hello");
        // Pointer-Identitaet: Token::text muss in src liegen.
        assert!(std::ptr::eq(t.text.as_ptr(), src.as_ptr()));
    }
}
