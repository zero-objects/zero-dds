// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Lexer-output data types: [`Token`] and [`TokenStream`].
//!
//! A **token** is the smallest lexical unit the lexer produces from
//! the source text. It consists of three components:
//!
//! - [`crate::grammar::TokenKind`] — classification, reused from the
//!   grammar module (see `grammar/mod.rs`). This ensures that the
//!   lexer-output classes are exactly the terminal symbols the
//!   recognizer matches against.
//! - [`crate::errors::Span`] — position in the source text (byte offsets).
//!   Passed through into later AST nodes and diagnostics.
//! - `text: &'src str` — slice into the original source. No allocation
//!   per token; lifetime `'src` coupled to the source-string binding.
//!
//! A **TokenStream** is the sequence of all tokens of a source file,
//! plus helpers for iteration and cursor operations. Whitespace and
//! comments are usually dropped by the lexer as trivia — the
//! stream contains only significant tokens.
//!
//! See RFC 0001 §4.1 (pipeline) and §5.x (lexer concept).

use core::fmt;

use crate::errors::Span;
use crate::grammar::TokenKind;

/// A single lexer token.
///
/// `'src` is the lifetime of the underlying source string. `text` is
/// a slice into exactly that string — valid for the duration of the lexer
/// output and all downstream processing steps (recognition,
/// CST build, AST build), as long as the source string lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token<'src> {
    /// Classification of the token (keyword, identifier, literal, etc.).
    pub kind: TokenKind,
    /// Position in the source text.
    pub span: Span,
    /// Text slice in the original source. For keywords/punctuation
    /// congruent with the `kind` content; for identifiers and literals the
    /// actual value read.
    pub text: &'src str,
}

impl<'src> Token<'src> {
    /// Constructs a new token.
    #[must_use]
    pub const fn new(kind: TokenKind, span: Span, text: &'src str) -> Self {
        Self { kind, span, text }
    }

    /// Constructs a token without a source location — `span = SYNTHETIC`, `text = ""`.
    ///
    /// Useful for programmatic recognition calls (tests, snippets,
    /// fixture builders) where only the token classification matters and
    /// no real source is present.
    #[must_use]
    pub const fn synthetic(kind: TokenKind) -> Token<'static> {
        Token {
            kind,
            span: Span::SYNTHETIC,
            text: "",
        }
    }

    /// Length of the token text in bytes (equals the span length).
    #[must_use]
    pub const fn len(&self) -> usize {
        self.span.len()
    }

    /// `true` if the token text is empty (the span has length 0).
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

/// Sequence of [`Token`]s the lexer produces from a source file.
///
/// Trivia (whitespace, comments) is not included — the lexer drops
/// it. If source-preserving rewrites are needed (formatter, CST
/// with trivia anchors), that is modeled in an extension of the lexer
/// output (Task 2.x or phase 1).
#[derive(Debug, Clone, Default)]
pub struct TokenStream<'src> {
    tokens: Vec<Token<'src>>,
}

impl<'src> TokenStream<'src> {
    /// New empty stream.
    #[must_use]
    pub fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    /// Constructs a stream from an existing token sequence.
    #[must_use]
    pub fn from_vec(tokens: Vec<Token<'src>>) -> Self {
        Self { tokens }
    }

    /// Appends a token to the end.
    pub fn push(&mut self, token: Token<'src>) {
        self.tokens.push(token);
    }

    /// All tokens as a slice.
    #[must_use]
    pub fn tokens(&self) -> &[Token<'src>] {
        &self.tokens
    }

    /// Iterates over all tokens in order.
    pub fn iter(&self) -> impl Iterator<Item = &Token<'src>> {
        self.tokens.iter()
    }

    /// Number of tokens.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// `true` if the stream contains no tokens.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Accesses the token at position `index`.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Token<'src>> {
        self.tokens.get(index)
    }

    /// Collects only the [`TokenKind`]s — convenience for recognizer calls
    /// that still work on `&[TokenKind]` (Task 2.4 switches them over).
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
        // Lifetime regression: Token::text must point into `src`.
        let src = String::from("hello world");
        let t = Token::new(TokenKind::Ident, Span::new(0, 5), &src[0..5]);
        assert_eq!(t.text, "hello");
        // Pointer identity: Token::text must lie within src.
        assert!(std::ptr::eq(t.text.as_ptr(), src.as_ptr()));
    }
}
