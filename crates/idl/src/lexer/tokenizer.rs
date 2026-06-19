// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Tokenizer — converts source text into a [`TokenStream`].
//!
//! Algorithm per source position:
//!
//! 1. Skip whitespace (space/tab/newline/CR).
//! 2. If the position is at an identifier start (alpha or `_`):
//!    - Scan the complete identifier (alphanumeric or `_`).
//!    - If the scanned text matches a keyword in [`TokenRules`]:
//!      emit `Keyword(s)`. Otherwise: emit `Ident`.
//!    - This way `structfoo` is recognized as a single identifier, not
//!      as `Keyword("struct")` + `Ident("foo")`.
//! 3. If the position is at a digit: scan IntegerLiteral (optional float — phase 0
//!    only has int support).
//! 4. Otherwise: try all punct rules in length order (longer
//!    first, thanks to [`TokenRules`] sorting). The first match wins.
//! 5. If nothing matches: [`ParseError::LexerError`] with the position.
//!
//! Pattern-based tokens (identifier, IntegerLiteral) are emitted only
//! if the respective rule is contained in the `TokenRules` —
//! otherwise the tokenizer would produce unexpected tokens that the
//! recognizer would not accept.
//!
//! Whitespace and comments are dropped (no trivia tracking in
//! phase 0). Source-preserving output comes with a source-map refactor
//! in a later phase, if formatters are needed.
//!
//! See RFC 0001 §4.1 and §5.x.

use crate::errors::{ParseError, Span};
use crate::grammar::{Grammar, TokenKind};

use super::rules::TokenRules;
use super::token::{Token, TokenStream};

/// Lexer-Frontend.
///
/// Construction via [`Tokenizer::for_grammar`] derives the token rules
/// once from the grammar; subsequent [`Tokenizer::tokenize`]
/// calls work on the fixed rule set.
#[derive(Debug, Clone)]
pub struct Tokenizer {
    rules: TokenRules,
}

impl Tokenizer {
    /// Constructs a tokenizer from an existing rule set.
    #[must_use]
    pub fn new(rules: TokenRules) -> Self {
        Self { rules }
    }

    /// Constructs a tokenizer by auto-extracting the rules from
    /// a grammar.
    #[must_use]
    pub fn for_grammar(grammar: &Grammar) -> Self {
        Self::new(TokenRules::from_grammar(grammar))
    }

    /// Access to the underlying rules.
    #[must_use]
    pub fn rules(&self) -> &TokenRules {
        &self.rules
    }

    /// Tokenizes the source text.
    ///
    /// # Errors
    /// Returns [`ParseError::LexerError`] if no token pattern matches
    /// at a position (unexpected character).
    pub fn tokenize<'src>(&self, source: &'src str) -> Result<TokenStream<'src>, ParseError> {
        let mut tokens = TokenStream::new();
        let bytes = source.as_bytes();
        let mut pos = 0usize;

        while pos < bytes.len() {
            // 1. Skip trivia (whitespace + comments).
            match skip_trivia(bytes, pos) {
                Ok(after) if after > pos => {
                    pos = after;
                    continue;
                }
                Err(e) => return Err(e),
                Ok(_) => {}
            }

            // 2. Wide-literal disambiguator: 'L"' (WideString) or "L'" (WideChar).
            if bytes[pos] == b'L' && pos + 1 < bytes.len() {
                if bytes[pos + 1] == b'"' && self.has_kind(TokenKind::WideStringLiteral) {
                    let end = scan_string_literal(source, bytes, pos + 1)?;
                    push_token(&mut tokens, TokenKind::WideStringLiteral, source, pos, end);
                    pos = end;
                    continue;
                }
                if bytes[pos + 1] == b'\'' && self.has_kind(TokenKind::WideCharLiteral) {
                    let end = scan_char_literal(source, bytes, pos + 1)?;
                    push_token(&mut tokens, TokenKind::WideCharLiteral, source, pos, end);
                    pos = end;
                    continue;
                }
            }

            // 3. Identifier or keyword.
            if is_ident_start(bytes[pos]) {
                let end = scan_ident(bytes, pos);
                let text = &source[pos..end];
                let kind = self.classify_ident(text);
                tokens.push(Token::new(kind, Span::new(pos, end), text));
                pos = end;
                continue;
            }

            // 4. String-Literal.
            if bytes[pos] == b'"' && self.has_kind(TokenKind::StringLiteral) {
                let end = scan_string_literal(source, bytes, pos)?;
                push_token(&mut tokens, TokenKind::StringLiteral, source, pos, end);
                pos = end;
                continue;
            }

            // 5. Char-Literal.
            if bytes[pos] == b'\'' && self.has_kind(TokenKind::CharLiteral) {
                let end = scan_char_literal(source, bytes, pos)?;
                push_token(&mut tokens, TokenKind::CharLiteral, source, pos, end);
                pos = end;
                continue;
            }

            // 6. Number-Literal: Integer / Float / Fixed-Point.
            if bytes[pos].is_ascii_digit()
                || (bytes[pos] == b'.' && pos + 1 < bytes.len() && bytes[pos + 1].is_ascii_digit())
            {
                if let Some((kind, end)) = self.scan_number(bytes, pos) {
                    push_token(&mut tokens, kind, source, pos, end);
                    pos = end;
                    continue;
                }
            }

            // 7. Punct (longest-first dank Rule-Sortierung).
            if let Some((kind, len)) = self.match_punct(source, pos) {
                let text = &source[pos..pos + len];
                tokens.push(Token::new(kind, Span::new(pos, pos + len), text));
                pos += len;
                continue;
            }

            // 8. No match — error.
            return Err(ParseError::LexerError {
                message: format_unknown_char(source, pos),
                span: Span::point(pos),
            });
        }

        Ok(tokens)
    }

    /// Scans a number from position `start`. Distinguishes integer (decimal,
    /// octal `0…`, hex `0x…`), float (with `.` or exponent) and
    /// fixed-point (with suffix `d`/`D`). Returns `None` if no
    /// matching token kind is in the rules.
    fn scan_number(&self, bytes: &[u8], start: usize) -> Option<(TokenKind, usize)> {
        // Hex
        if bytes[start] == b'0'
            && start + 1 < bytes.len()
            && (bytes[start + 1] == b'x' || bytes[start + 1] == b'X')
            && self.has_kind(TokenKind::IntegerLiteral)
        {
            let mut pos = start + 2;
            while pos < bytes.len() && bytes[pos].is_ascii_hexdigit() {
                pos += 1;
            }
            if pos > start + 2 {
                return Some((TokenKind::IntegerLiteral, pos));
            }
        }

        let mut pos = start;
        let mut int_part_present = false;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
            int_part_present = true;
        }
        let int_end = pos;

        // Optional Fraction
        let mut has_dot = false;
        if pos < bytes.len() && bytes[pos] == b'.' {
            // `.` is only a float when either an int part was present or digits follow `.`.
            let next_is_digit = pos + 1 < bytes.len() && bytes[pos + 1].is_ascii_digit();
            if int_part_present || next_is_digit {
                has_dot = true;
                pos += 1;
                while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                    pos += 1;
                }
            }
        }

        // Optional Exponent
        let mut has_exp = false;
        if pos < bytes.len() && (bytes[pos] == b'e' || bytes[pos] == b'E') {
            let mut exp_pos = pos + 1;
            if exp_pos < bytes.len() && (bytes[exp_pos] == b'+' || bytes[exp_pos] == b'-') {
                exp_pos += 1;
            }
            let exp_digits_start = exp_pos;
            while exp_pos < bytes.len() && bytes[exp_pos].is_ascii_digit() {
                exp_pos += 1;
            }
            if exp_pos > exp_digits_start {
                has_exp = true;
                pos = exp_pos;
            }
        }

        // Fixed-Point Suffix?
        let has_fixed = pos < bytes.len()
            && (bytes[pos] == b'd' || bytes[pos] == b'D')
            && self.has_kind(TokenKind::FixedPtLiteral);
        if has_fixed {
            return Some((TokenKind::FixedPtLiteral, pos + 1));
        }

        if (has_dot || has_exp) && self.has_kind(TokenKind::FloatLiteral) {
            return Some((TokenKind::FloatLiteral, pos));
        }

        if int_part_present && self.has_kind(TokenKind::IntegerLiteral) {
            return Some((TokenKind::IntegerLiteral, int_end));
        }

        None
    }

    /// Classifies an identifier text: keyword if it is in the rules,
    /// otherwise Ident.
    ///
    /// §7.2.3.2 escape identifier: an identifier with a `_` prefix
    /// (`_module`, `_struct`, ...) is *never* a keyword. The
    /// underscore turns off the keyword check and thus allows
    /// the use of reserved words as identifiers.
    fn classify_ident(&self, text: &str) -> TokenKind {
        if text.starts_with('_') {
            return TokenKind::Ident;
        }
        for rule in self.rules.iter() {
            if let TokenKind::Keyword(kw) = rule.kind {
                if kw == text {
                    return TokenKind::Keyword(kw);
                }
            }
        }
        TokenKind::Ident
    }

    /// Tries each punct rule in order (longest first thanks to sort).
    fn match_punct(&self, source: &str, pos: usize) -> Option<(TokenKind, usize)> {
        let tail = &source[pos..];
        for rule in self.rules.iter() {
            if let TokenKind::Punct(p) = rule.kind {
                if tail.starts_with(p) {
                    return Some((TokenKind::Punct(p), p.len()));
                }
            }
        }
        None
    }

    /// `true` if the rules contain a pattern-based rule for `kind`.
    fn has_kind(&self, kind: TokenKind) -> bool {
        self.rules.iter().any(|r| r.kind == kind)
    }
}

// ---------------------------------------------------------------------------
// Hilfs-Funktionen (file-private)
// ---------------------------------------------------------------------------

fn skip_whitespace(bytes: &[u8], start: usize) -> usize {
    // §7.2.1: "Blanks, horizontal and vertical tabs, newlines, form feeds,
    // and comments (collective, 'white space') are ignored except as they
    // serve to separate tokens." VT (0x0B) and FF (0x0C) belong here per
    // spec Tab. 7-5.
    let mut i = start;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r' | b'\x0b' | b'\x0c') {
        i += 1;
    }
    i
}

/// Consumes, in a loop, all whitespace and comment sequences
/// (repeated until nothing more is trivializable). Returns the
/// new position, or a [`ParseError`] on an unterminated block
/// comment.
fn skip_trivia(bytes: &[u8], start: usize) -> Result<usize, ParseError> {
    let mut pos = start;
    loop {
        let after_ws = skip_whitespace(bytes, pos);
        if after_ws > pos {
            pos = after_ws;
            continue;
        }
        // Line-Comment: //
        if pos + 1 < bytes.len() && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            pos = skip_line_comment(bytes, pos);
            continue;
        }
        // Block-Comment: /* … */
        if pos + 1 < bytes.len() && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            pos = skip_block_comment(bytes, pos)?;
            continue;
        }
        break;
    }
    Ok(pos)
}

fn skip_line_comment(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 2; // "//" skippen
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    i
}

fn skip_block_comment(bytes: &[u8], start: usize) -> Result<usize, ParseError> {
    // "/* ... */" — IDL allows no nesting; the first "*/"
    // ends the block.
    let mut i = start + 2;
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            return Ok(i + 2);
        }
        i += 1;
    }
    Err(ParseError::LexerError {
        message: format!("unterminated block comment starting at byte offset {start}"),
        span: Span::new(start, bytes.len()),
    })
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn scan_ident(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && is_ident_continue(bytes[i]) {
        i += 1;
    }
    i
}

/// Scans a string literal `"..."` from position `start`. Supports
/// `\` escape sequences (every character after `\` belongs to the string,
/// in particular `\"`).
fn scan_string_literal(source: &str, bytes: &[u8], start: usize) -> Result<usize, ParseError> {
    let mut i = start + 1; // Anfangs-`"` skippen
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => i += 2,
            b'"' => return Ok(i + 1),
            _ => i += 1,
        }
    }
    Err(ParseError::LexerError {
        message: format!("unterminated string literal starting at byte offset {start}"),
        span: Span::new(start, source.len()),
    })
}

/// Scans a char literal `'x'` (or `'\n'`, `'\xFF'`) from position `start`.
fn scan_char_literal(source: &str, bytes: &[u8], start: usize) -> Result<usize, ParseError> {
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => i += 2,
            b'\'' => return Ok(i + 1),
            _ => i += 1,
        }
    }
    Err(ParseError::LexerError {
        message: format!("unterminated character literal starting at byte offset {start}"),
        span: Span::new(start, source.len()),
    })
}

/// Helper: construct a token with span and append it to the stream.
fn push_token<'src>(
    stream: &mut TokenStream<'src>,
    kind: TokenKind,
    source: &'src str,
    start: usize,
    end: usize,
) {
    stream.push(Token::new(kind, Span::new(start, end), &source[start..end]));
}

/// Formats a lexer error message for an unknown character at
/// `pos`. Safe against multi-byte UTF-8 (takes the first Rust `char` from
/// `pos`, uses `'?'` as a fallback).
fn format_unknown_char(source: &str, pos: usize) -> String {
    let ch = source[pos..].chars().next().unwrap_or('?');
    format!("unexpected character {ch:?} at byte offset {pos}")
}

#[cfg(test)]
mod tests {
    // Tests may panic + use .expect (workspace lints are primarily
    // intended for production code; assert_eq!() panics internally too).
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::grammar::{
        Alternative, Grammar, IdlVersion, Production, ProductionId, SpecRef, Symbol,
    };

    const TS: SpecRef = SpecRef {
        doc: "TEST",
        section: "0.0",
    };

    const fn alt(symbols: &'static [Symbol]) -> Alternative {
        Alternative {
            name: None,
            symbols,
            note: None,
        }
    }

    const fn prod(id: u32, name: &'static str, alts: &'static [Alternative]) -> Production {
        Production {
            id: ProductionId(id),
            name,
            spec_ref: TS,
            alternatives: alts,
            ast_hint: None,
        }
    }

    /// Grammar with keyword "struct", Ident, Punct "{", "}", ";".
    const G_BASIC: Grammar = Grammar {
        name: "basic",
        version: IdlVersion::V4_2,
        productions: &[prod(
            0,
            "a",
            &[alt(&[
                Symbol::Terminal(TokenKind::Keyword("struct")),
                Symbol::Terminal(TokenKind::Ident),
                Symbol::Terminal(TokenKind::Punct("{")),
                Symbol::Terminal(TokenKind::Punct("}")),
                Symbol::Terminal(TokenKind::Punct(";")),
            ])],
        )],
        start: ProductionId(0),
        token_rules: &[],
    };

    // -----------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------

    #[test]
    fn for_grammar_extracts_rules() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        assert!(t.rules().len() >= 5);
    }

    #[test]
    fn new_uses_provided_rules() {
        let rules = TokenRules::from_grammar(&G_BASIC);
        let original_len = rules.len();
        let t = Tokenizer::new(rules);
        assert_eq!(t.rules().len(), original_len);
    }

    // -----------------------------------------------------------------
    // Whitespace + empty input
    // -----------------------------------------------------------------

    #[test]
    fn empty_source_yields_empty_stream() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("").expect("must succeed");
        assert!(s.is_empty());
    }

    #[test]
    fn whitespace_only_yields_empty_stream() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize(" \t\n\r ").expect("must succeed");
        assert!(s.is_empty());
    }

    // -----------------------------------------------------------------
    // Keyword vs. Ident
    // -----------------------------------------------------------------

    #[test]
    fn single_keyword_emits_keyword_token() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("struct").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::Keyword("struct"));
        assert_eq!(s.tokens()[0].text, "struct");
        assert_eq!(s.tokens()[0].span, Span::new(0, 6));
    }

    #[test]
    fn single_ident_emits_ident_token() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("Foo").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::Ident);
        assert_eq!(s.tokens()[0].text, "Foo");
    }

    #[test]
    fn ident_starting_with_keyword_prefix_stays_ident() {
        // Regression: "structfoo" must NOT be split into keyword + ident
        // — the lexer first scans the full identifier and
        // then classifies.
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("structfoo").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::Ident);
        assert_eq!(s.tokens()[0].text, "structfoo");
    }

    #[test]
    fn ident_starting_with_underscore() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("_internal").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::Ident);
    }

    #[test]
    fn ident_with_digits_after_letter() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("foo42").expect("must succeed");
        assert_eq!(s.tokens()[0].text, "foo42");
        assert_eq!(s.tokens()[0].kind, TokenKind::Ident);
    }

    // -----------------------------------------------------------------
    // Punctuation
    // -----------------------------------------------------------------

    #[test]
    fn single_punct() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("{").expect("must succeed");
        assert_eq!(s.tokens()[0].kind, TokenKind::Punct("{"));
    }

    #[test]
    fn longest_match_for_multichar_punct() {
        // Grammar with "::" and ":" — the tokenizer must recognize "::" as 1 token,
        // not 2x ":".
        const G: Grammar = Grammar {
            name: "colons",
            version: IdlVersion::V4_2,
            productions: &[prod(
                0,
                "a",
                &[alt(&[
                    Symbol::Terminal(TokenKind::Punct("::")),
                    Symbol::Terminal(TokenKind::Punct(":")),
                ])],
            )],
            start: ProductionId(0),
            token_rules: &[],
        };
        let t = Tokenizer::for_grammar(&G);
        let s = t.tokenize("::").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::Punct("::"));
    }

    #[test]
    fn shorter_punct_matches_when_longer_does_not_apply() {
        const G: Grammar = Grammar {
            name: "colons",
            version: IdlVersion::V4_2,
            productions: &[prod(
                0,
                "a",
                &[alt(&[
                    Symbol::Terminal(TokenKind::Punct("::")),
                    Symbol::Terminal(TokenKind::Punct(":")),
                ])],
            )],
            start: ProductionId(0),
            token_rules: &[],
        };
        let t = Tokenizer::for_grammar(&G);
        let s = t.tokenize(":").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::Punct(":"));
    }

    // -----------------------------------------------------------------
    // Sequence + Spans
    // -----------------------------------------------------------------

    #[test]
    fn sequence_struct_ident_braces() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("struct Foo {}").expect("must succeed");
        let kinds: Vec<TokenKind> = s.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Keyword("struct"),
                TokenKind::Ident,
                TokenKind::Punct("{"),
                TokenKind::Punct("}"),
            ]
        );
    }

    #[test]
    fn spans_are_continuous_and_correct() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let src = "struct Foo;";
        let s = t.tokenize(src).expect("must succeed");
        // "struct" 0..6, "Foo" 7..10, ";" 10..11
        assert_eq!(s.tokens()[0].span, Span::new(0, 6));
        assert_eq!(s.tokens()[1].span, Span::new(7, 10));
        assert_eq!(s.tokens()[2].span, Span::new(10, 11));
    }

    #[test]
    fn newlines_separate_tokens_without_emitting_trivia() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("struct\nFoo").expect("must succeed");
        assert_eq!(s.len(), 2);
    }

    // -----------------------------------------------------------------
    // Integer literal (only if the grammar knows it)
    // -----------------------------------------------------------------

    #[test]
    fn integer_literal_when_grammar_includes_it() {
        const G: Grammar = Grammar {
            name: "ints",
            version: IdlVersion::V4_2,
            productions: &[prod(
                0,
                "a",
                &[alt(&[
                    Symbol::Terminal(TokenKind::IntegerLiteral),
                    Symbol::Terminal(TokenKind::Punct("+")),
                ])],
            )],
            start: ProductionId(0),
            token_rules: &[],
        };
        let t = Tokenizer::for_grammar(&G);
        let s = t.tokenize("42 + 100").expect("must succeed");
        assert_eq!(s.len(), 3);
        assert_eq!(s.tokens()[0].kind, TokenKind::IntegerLiteral);
        assert_eq!(s.tokens()[0].text, "42");
        assert_eq!(s.tokens()[2].kind, TokenKind::IntegerLiteral);
        assert_eq!(s.tokens()[2].text, "100");
    }

    #[test]
    fn integer_literal_skipped_when_grammar_does_not_include_it() {
        // G_BASIC does not know IntegerLiteral — "42" → lexer error.
        let t = Tokenizer::for_grammar(&G_BASIC);
        let result = t.tokenize("42");
        assert!(matches!(result, Err(ParseError::LexerError { .. })));
    }

    // -----------------------------------------------------------------
    // Error paths
    // -----------------------------------------------------------------

    #[test]
    fn unknown_character_yields_lexer_error_at_position() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let result = t.tokenize("struct @");
        assert!(matches!(
            result,
            Err(ParseError::LexerError {
                ref message,
                span: Span { start: 7, end: 7 },
            }) if message.contains('@')
        ));
    }

    #[test]
    fn unknown_character_at_position_zero() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let result = t.tokenize("@struct");
        assert!(matches!(
            result,
            Err(ParseError::LexerError {
                span: Span { start: 0, end: 0 },
                ..
            })
        ));
    }

    // -----------------------------------------------------------------
    // E2E with the toy grammar
    // -----------------------------------------------------------------

    #[test]
    fn tokenize_toy_grammar_addition() {
        use crate::grammar::toy::TOY;
        let t = Tokenizer::for_grammar(&TOY);
        let s = t.tokenize("n + n").expect("must succeed");
        let kinds: Vec<TokenKind> = s.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Keyword("n"),
                TokenKind::Punct("+"),
                TokenKind::Keyword("n"),
            ]
        );
    }

    // -----------------------------------------------------------------
    // IDL-4.2 number-/string-/char literals (T3.1)
    // -----------------------------------------------------------------

    /// Grammar with all important IDL literal classes.
    const G_LITERALS: Grammar = Grammar {
        name: "literals",
        version: IdlVersion::V4_2,
        productions: &[prod(
            0,
            "a",
            &[alt(&[
                Symbol::Terminal(TokenKind::IntegerLiteral),
                Symbol::Terminal(TokenKind::FloatLiteral),
                Symbol::Terminal(TokenKind::FixedPtLiteral),
                Symbol::Terminal(TokenKind::StringLiteral),
                Symbol::Terminal(TokenKind::CharLiteral),
                Symbol::Terminal(TokenKind::WideStringLiteral),
                Symbol::Terminal(TokenKind::WideCharLiteral),
            ])],
        )],
        start: ProductionId(0),
        token_rules: &[],
    };

    #[test]
    fn integer_decimal_octal_hex() {
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("42 0755 0xCAFE 0X1F").expect("must succeed");
        assert_eq!(s.len(), 4);
        assert!(s.iter().all(|t| t.kind == TokenKind::IntegerLiteral));
        assert_eq!(s.tokens()[0].text, "42");
        assert_eq!(s.tokens()[1].text, "0755");
        assert_eq!(s.tokens()[2].text, "0xCAFE");
        assert_eq!(s.tokens()[3].text, "0X1F");
    }

    #[test]
    fn float_with_dot_and_exponent() {
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("3.14 .5 1.0e10 2E-3").expect("must succeed");
        assert_eq!(s.len(), 4);
        assert!(s.iter().all(|t| t.kind == TokenKind::FloatLiteral));
        assert_eq!(s.tokens()[0].text, "3.14");
        assert_eq!(s.tokens()[1].text, ".5");
        assert_eq!(s.tokens()[2].text, "1.0e10");
        assert_eq!(s.tokens()[3].text, "2E-3");
    }

    #[test]
    fn fixed_point_with_d_suffix() {
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("1.5d 100D").expect("must succeed");
        assert_eq!(s.len(), 2);
        assert!(s.iter().all(|t| t.kind == TokenKind::FixedPtLiteral));
    }

    #[test]
    fn string_literal_with_escape() {
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t
            .tokenize(r#""hello" "with \"quote\"""#)
            .expect("must succeed");
        assert_eq!(s.len(), 2);
        assert_eq!(s.tokens()[0].kind, TokenKind::StringLiteral);
        assert_eq!(s.tokens()[0].text, r#""hello""#);
    }

    #[test]
    fn char_literal_with_escape() {
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize(r"'x' '\n' '\\'").expect("must succeed");
        assert_eq!(s.len(), 3);
        assert!(s.iter().all(|t| t.kind == TokenKind::CharLiteral));
    }

    #[test]
    fn wide_string_literal() {
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize(r#"L"wide""#).expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::WideStringLiteral);
        assert_eq!(s.tokens()[0].text, r#"L"wide""#);
    }

    #[test]
    fn wide_char_literal() {
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize(r"L'x'").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::WideCharLiteral);
    }

    #[test]
    fn unterminated_string_literal_is_error() {
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let result = t.tokenize(r#""unterminated"#);
        assert!(matches!(result, Err(ParseError::LexerError { .. })));
    }

    #[test]
    fn unterminated_char_literal_is_error() {
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let result = t.tokenize(r"'");
        assert!(matches!(result, Err(ParseError::LexerError { .. })));
    }

    // -----------------------------------------------------------------
    // T-LIM-1 — Comments
    // -----------------------------------------------------------------

    #[test]
    fn line_comment_skipped() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t
            .tokenize("struct // comment\n Foo {}")
            .expect("must succeed");
        let kinds: Vec<TokenKind> = s.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Keyword("struct"),
                TokenKind::Ident,
                TokenKind::Punct("{"),
                TokenKind::Punct("}"),
            ]
        );
    }

    #[test]
    fn line_comment_at_end_of_input() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("struct Foo // last").expect("must succeed");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn block_comment_skipped() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t
            .tokenize("struct /* a comment */ Foo")
            .expect("must succeed");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn multiline_block_comment_skipped() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t
            .tokenize("struct /* line 1\nline 2\nline 3 */ Foo")
            .expect("must succeed");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn multiple_comments_in_a_row() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t
            .tokenize("struct // c1\n /* c2 */ // c3\n Foo")
            .expect("must succeed");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn comments_inside_struct_definition() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t
            .tokenize("struct Foo { // member follows\n}")
            .expect("must succeed");
        let kinds: Vec<TokenKind> = s.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Keyword("struct"),
                TokenKind::Ident,
                TokenKind::Punct("{"),
                TokenKind::Punct("}"),
            ]
        );
    }

    #[test]
    fn unterminated_block_comment_is_error() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        let result = t.tokenize("struct /* unterminated");
        assert!(matches!(result, Err(ParseError::LexerError { .. })));
    }

    #[test]
    fn slash_in_string_is_not_comment_start() {
        let t = Tokenizer::for_grammar(&G_LITERALS);
        // String with "//" inside — must not be interpreted as a comment.
        let s = t.tokenize(r#""http://example""#).expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::StringLiteral);
    }

    #[test]
    fn identifier_starting_with_l_is_not_wide_literal() {
        // "Lazy" begins with L, but is not L"..." or L'...'.
        // The lexer must recognize it as an identifier.
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("Lazy").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::Ident);
        assert_eq!(s.tokens()[0].text, "Lazy");
    }

    // -----------------------------------------------------------------
    // §7.2.1 — VT/FF in source whitespace (Phase 1.1)
    // -----------------------------------------------------------------

    #[test]
    fn whitespace_includes_vt_and_ff() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        // Source with VT (0x0B) and FF (0x0C) between tokens. Spec §7.2.1
        // counts both as "white space".
        let s = t
            .tokenize("struct\x0bA\x0c{}\x0b;")
            .expect("VT/FF must be whitespace");
        assert_eq!(
            s.kinds(),
            vec![
                TokenKind::Keyword("struct"),
                TokenKind::Ident,
                TokenKind::Punct("{"),
                TokenKind::Punct("}"),
                TokenKind::Punct(";"),
            ]
        );
    }

    // -----------------------------------------------------------------
    // §7.2.2 — comment markers inside comments have no meaning
    // (Phase 1.2)
    // -----------------------------------------------------------------

    #[test]
    fn line_comment_contains_block_comment_start() {
        // `//` starts a line comment, `/*` inside it has no meaning.
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t
            .tokenize("struct A // foo /* bar\n{};")
            .expect("must succeed");
        assert_eq!(
            s.kinds(),
            vec![
                TokenKind::Keyword("struct"),
                TokenKind::Ident,
                TokenKind::Punct("{"),
                TokenKind::Punct("}"),
                TokenKind::Punct(";"),
            ]
        );
    }

    #[test]
    fn block_comment_contains_line_comment_marker() {
        // `//` and `/*` inside `/* … */` have no meaning.
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t
            .tokenize("struct A /* // not a line comment\n /* nested-looking */{};")
            .expect("must succeed");
        assert_eq!(
            s.kinds(),
            vec![
                TokenKind::Keyword("struct"),
                TokenKind::Ident,
                TokenKind::Punct("{"),
                TokenKind::Punct("}"),
                TokenKind::Punct(";"),
            ]
        );
    }

    #[test]
    fn block_comment_does_not_nest() {
        // `/* /* inner */ rest */` — the first `*/` closes the block,
        // ` rest */` remains as source. Spec §7.2.2: comments do not nest.
        let t = Tokenizer::for_grammar(&G_BASIC);
        let result = t.tokenize("struct /* /* inner */ A {} ;");
        // ` A ` becomes an Ident, then `{} ;`. But "inner" was closed by the first
        // `*/` — so after `*/` the next token is
        // `A`, which is an Ident. For the test we only check that nothing
        // crashes and `A` is found as an Ident.
        let s = result.expect("must succeed");
        let kinds: Vec<_> = s.iter().map(|t| t.kind).collect();
        assert!(kinds.contains(&TokenKind::Ident));
    }

    // -----------------------------------------------------------------
    // §7.2.4 — completeness test for all 73 IDL keywords
    // (Phase 1.3)
    // -----------------------------------------------------------------

    #[test]
    fn all_table_7_6_keywords_classified_as_keyword() {
        use crate::grammar::idl42::IDL_42;
        // 73 keywords from spec §7.2.4 Table 7-6.
        const KEYWORDS: &[&str] = &[
            "abstract",
            "any",
            "alias",
            "attribute",
            "bitfield",
            "bitmask",
            "bitset",
            "boolean",
            "case",
            "char",
            "component",
            "connector",
            "const",
            "consumes",
            "context",
            "custom",
            "default",
            "double",
            "exception",
            "emits",
            "enum",
            "eventtype",
            "factory",
            "FALSE",
            "finder",
            "fixed",
            "float",
            "getraises",
            "home",
            "import",
            "in",
            "inout",
            "interface",
            "local",
            "long",
            "manages",
            "map",
            "mirrorport",
            "module",
            "multiple",
            "native",
            "Object",
            "octet",
            "oneway",
            "out",
            "primarykey",
            "private",
            "port",
            "porttype",
            "provides",
            "public",
            "publishes",
            "raises",
            "readonly",
            "setraises",
            "sequence",
            "short",
            "string",
            "struct",
            "supports",
            "switch",
            "TRUE",
            "truncatable",
            "typedef",
            "typeid",
            "typename",
            "typeprefix",
            "unsigned",
            "union",
            "uses",
            "ValueBase",
            "valuetype",
            "void",
            "wchar",
            "wstring",
            "int8",
            "uint8",
            "int16",
            "int32",
            "int64",
            "uint16",
            "uint32",
            "uint64",
        ];
        let t = Tokenizer::for_grammar(&IDL_42);
        for kw in KEYWORDS {
            let s = t
                .tokenize(kw)
                .unwrap_or_else(|e| panic!("keyword {kw} must be lexable: {e:?}"));
            assert_eq!(s.len(), 1, "keyword {kw}: expect exactly 1 token");
            assert_eq!(
                s.tokens()[0].kind,
                TokenKind::Keyword(kw),
                "keyword {kw}: TokenKind must be Keyword({kw}), is {:?}",
                s.tokens()[0].kind
            );
        }
    }

    // -----------------------------------------------------------------
    // §7.2.6.4 — Floating-Point Edge Cases (Phase 1.5)
    // -----------------------------------------------------------------

    #[test]
    fn float_no_int_part() {
        // `.5e10` without an integer part before the dot.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize(".5e10").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FloatLiteral);
        assert_eq!(s.tokens()[0].text, ".5e10");
    }

    #[test]
    fn float_no_fraction_part() {
        // `5.e10` without a fraction part after the dot.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("5.e10").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FloatLiteral);
        assert_eq!(s.tokens()[0].text, "5.e10");
    }

    #[test]
    fn float_no_decimal_point_only_exponent() {
        // `5e10` — no dot, only exponent.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("5e10").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FloatLiteral);
    }

    #[test]
    fn float_no_exponent_only_decimal_point() {
        // `5.5` — dot without exponent.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("5.5").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FloatLiteral);
    }

    #[test]
    fn float_dot_alone_is_punct_not_float() {
        // `.` alone without a digit on either side is Punct, not Float.
        // Spec §7.2.6.4: "Either the integer part or the fraction part
        // (but not both) may be missing."
        let t = Tokenizer::for_grammar(&G_BASIC);
        let result = t.tokenize(".");
        // G_BASIC has no `.` punct, hence an unknown-character error.
        // Main point: no FloatLiteral.
        assert!(
            result.is_err(),
            "`.` alone must not count as a FloatLiteral"
        );
    }

    // -----------------------------------------------------------------
    // §7.2.6.5 — Fixed-Point Edge Cases (Phase 1.6)
    // -----------------------------------------------------------------

    #[test]
    fn fixed_no_int_part() {
        // `.5d` without an integer part.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize(".5d").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FixedPtLiteral);
    }

    #[test]
    fn fixed_no_fraction_part() {
        // `5.d` without a fraction part.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("5.d").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FixedPtLiteral);
    }

    #[test]
    fn fixed_no_decimal_point() {
        // `5d` — no dot. Spec §7.2.6.5: "the decimal point (but not the
        // letter d or D) may be missing."
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("5d").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FixedPtLiteral);
    }

    #[test]
    fn fixed_uppercase_d() {
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("5D").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FixedPtLiteral);
    }

    #[test]
    fn fixed_without_d_is_not_fixed() {
        // `5.5` without a d suffix → FloatLiteral, not FixedPtLiteral.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("5.5").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FloatLiteral);
    }

    #[test]
    fn tokenize_toy_grammar_parenthesized() {
        use crate::grammar::toy::TOY;
        let t = Tokenizer::for_grammar(&TOY);
        let s = t.tokenize("(n*n)").expect("must succeed");
        assert_eq!(s.len(), 5);
        let kinds: Vec<TokenKind> = s.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Punct("("),
                TokenKind::Keyword("n"),
                TokenKind::Punct("*"),
                TokenKind::Keyword("n"),
                TokenKind::Punct(")"),
            ]
        );
    }
}
