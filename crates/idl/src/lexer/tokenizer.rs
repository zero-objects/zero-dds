// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Tokenizer — wandelt Source-Text in einen [`TokenStream`].
//!
//! Algorithmus pro Source-Position:
//!
//! 1. Skip whitespace (Space/Tab/Newline/CR).
//! 2. Wenn Position auf Identifier-Start (alpha oder `_`):
//!    - Scan vollstaendigen Identifier (alphanumeric oder `_`).
//!    - Wenn der gescannte Text einem Keyword in [`TokenRules`] entspricht:
//!      emit `Keyword(s)`. Sonst: emit `Ident`.
//!    - Damit wird `structfoo` als ein einziger Identifier erkannt, nicht
//!      als `Keyword("struct")` + `Ident("foo")`.
//! 3. Wenn Position auf Digit: scan IntegerLiteral (optional Float — Phase 0
//!    hat nur Int-Support).
//! 4. Sonst: probiere alle Punct-Regeln in Laengen-Reihenfolge (laenger
//!    zuerst, dank [`TokenRules`]-Sortierung). Erstes Match gewinnt.
//! 5. Wenn nichts matcht: [`ParseError::LexerError`] mit Position.
//!
//! Pattern-basierte Tokens (Identifier, IntegerLiteral) werden nur
//! emittiert, wenn die jeweilige Regel in den `TokenRules` enthalten ist —
//! sonst wuerde der Tokenizer unerwartete Tokens produzieren, die der
//! Recognizer nicht akzeptieren wuerde.
//!
//! Whitespace und Kommentare werden gedroppt (kein Trivia-Tracking in
//! Phase 0). Source-Preserving Output kommt mit einem Source-Map-Refactor
//! in einer spaeteren Phase, falls Formatter benoetigt werden.
//!
//! Siehe RFC 0001 §4.1 und §5.x.

use crate::errors::{ParseError, Span};
use crate::grammar::{Grammar, TokenKind};

use super::rules::TokenRules;
use super::token::{Token, TokenStream};

/// Lexer-Frontend.
///
/// Konstruktion ueber [`Tokenizer::for_grammar`] leitet die Token-Regeln
/// einmalig aus der Grammar ab; anschliessende [`Tokenizer::tokenize`]-
/// Aufrufe arbeiten auf dem festen Regel-Satz.
#[derive(Debug, Clone)]
pub struct Tokenizer {
    rules: TokenRules,
}

impl Tokenizer {
    /// Konstruiert einen Tokenizer aus einer existierenden Regel-Menge.
    #[must_use]
    pub fn new(rules: TokenRules) -> Self {
        Self { rules }
    }

    /// Konstruiert einen Tokenizer durch Auto-Extraktion der Regeln aus
    /// einer Grammar.
    #[must_use]
    pub fn for_grammar(grammar: &Grammar) -> Self {
        Self::new(TokenRules::from_grammar(grammar))
    }

    /// Zugriff auf die zugrundeliegenden Regeln.
    #[must_use]
    pub fn rules(&self) -> &TokenRules {
        &self.rules
    }

    /// Tokenisiert den Source-Text.
    ///
    /// # Errors
    /// Liefert [`ParseError::LexerError`], wenn an einer Position kein
    /// Token-Pattern matcht (unerwartetes Zeichen).
    pub fn tokenize<'src>(&self, source: &'src str) -> Result<TokenStream<'src>, ParseError> {
        let mut tokens = TokenStream::new();
        let bytes = source.as_bytes();
        let mut pos = 0usize;

        while pos < bytes.len() {
            // 1. Trivia (Whitespace + Comments) ueberspringen.
            match skip_trivia(bytes, pos) {
                Ok(after) if after > pos => {
                    pos = after;
                    continue;
                }
                Err(e) => return Err(e),
                Ok(_) => {}
            }

            // 2. Wide-Literal-Disambiguator: 'L"' (WideString) oder "L'" (WideChar).
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

            // 3. Identifier oder Keyword.
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

            // 8. Kein Match — Error.
            return Err(ParseError::LexerError {
                message: format_unknown_char(source, pos),
                span: Span::point(pos),
            });
        }

        Ok(tokens)
    }

    /// Scannt eine Zahl ab Position `start`. Unterscheidet Integer (decimal,
    /// octal `0…`, hex `0x…`), Float (mit `.` oder Exponent) und
    /// Fixed-Point (mit Suffix `d`/`D`). Liefert `None`, wenn keine
    /// passende Token-Kind in den Regeln ist.
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
            // `.` ist nur Float, wenn entweder Int-Part da war oder nach `.` Digits.
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

    /// Klassifiziert einen Identifier-Text: Keyword wenn in den Regeln,
    /// sonst Ident.
    ///
    /// §7.2.3.2 Escape-Identifier: ein Identifier mit `_`-Prefix
    /// (`_module`, `_struct`, ...) ist *nie* ein Keyword. Der
    /// Underscore schaltet die Keyword-Pruefung ab und erlaubt damit
    /// die Verwendung reservierter Worte als Identifier.
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

    /// Versucht jede Punct-Regel in Reihenfolge (longest first dank Sort).
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

    /// `true`, wenn die Regeln eine Pattern-basierte Regel fuer `kind`
    /// enthalten.
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
    // serve to separate tokens." VT (0x0B) und FF (0x0C) gehoeren laut
    // Spec-Tab. 7-5 dazu.
    let mut i = start;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r' | b'\x0b' | b'\x0c') {
        i += 1;
    }
    i
}

/// Konsumiert in einer Schleife alle Whitespace- und Comment-Sequenzen
/// (Mehrfach-Wiederholung bis nichts mehr triviabar ist). Liefert die
/// neue Position, oder einen [`ParseError`] bei unterminiertem Block-
/// Comment.
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
    // "/* ... */" — IDL erlaubt keine Verschachtelung; das erste "*/"
    // beendet den Block.
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

/// Scannt ein String-Literal `"..."` ab Position `start`. Unterstuetzt
/// `\`-Escape-Sequenzen (jedes Zeichen nach `\` gehoert zum String,
/// insbesondere `\"`).
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

/// Scannt ein Char-Literal `'x'` (oder `'\n'`, `'\xFF'`) ab Position `start`.
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

/// Hilfsfunktion: Token mit Span konstruieren und an Stream anhaengen.
fn push_token<'src>(
    stream: &mut TokenStream<'src>,
    kind: TokenKind,
    source: &'src str,
    start: usize,
    end: usize,
) {
    stream.push(Token::new(kind, Span::new(start, end), &source[start..end]));
}

/// Formatiert eine Lexer-Fehlermeldung fuer ein unbekanntes Zeichen an
/// `pos`. Sicher gegen multi-byte UTF-8 (nimmt das erste Rust-`char` ab
/// `pos`, nimmt `'?'` als Fallback).
fn format_unknown_char(source: &str, pos: usize) -> String {
    let ch = source[pos..].chars().next().unwrap_or('?');
    format!("unexpected character {ch:?} at byte offset {pos}")
}

#[cfg(test)]
mod tests {
    // Tests duerfen panicen + .expect verwenden (Workspace-Lints sind primaer
    // fuer Produktions-Code gedacht; assert_eq!() panickt intern auch).
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

    /// Grammar mit Keyword "struct", Ident, Punct "{", "}", ";".
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
    // Konstruktion
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
    // Whitespace + leerer Input
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
        // Regression: "structfoo" darf NICHT als Keyword + Ident zerlegt
        // werden — der Lexer scannt zuerst den vollen Identifier und
        // klassifiziert dann.
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
        // Grammar mit "::" und ":" — Tokenizer muss "::" als 1 Token
        // erkennen, nicht 2x ":".
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
    // Integer-Literal (nur wenn Grammar es kennt)
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
        // G_BASIC kennt IntegerLiteral nicht — "42" → Lexer-Error.
        let t = Tokenizer::for_grammar(&G_BASIC);
        let result = t.tokenize("42");
        assert!(matches!(result, Err(ParseError::LexerError { .. })));
    }

    // -----------------------------------------------------------------
    // Error-Pfade
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
    // E2E mit Toy-Grammar
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
    // IDL-4.2 Number-/String-/Char-Literale (T3.1)
    // -----------------------------------------------------------------

    /// Grammar mit allen wichtigen IDL-Literal-Klassen.
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
        // String mit "//" innen — darf nicht als Comment interpretiert werden.
        let s = t.tokenize(r#""http://example""#).expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::StringLiteral);
    }

    #[test]
    fn identifier_starting_with_l_is_not_wide_literal() {
        // "Lazy" beginnt mit L, ist aber kein L"..." oder L'...'.
        // Lexer muss als Identifier erkennen.
        let t = Tokenizer::for_grammar(&G_BASIC);
        let s = t.tokenize("Lazy").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::Ident);
        assert_eq!(s.tokens()[0].text, "Lazy");
    }

    // -----------------------------------------------------------------
    // §7.2.1 — VT/FF im Source-Whitespace (Phase 1.1)
    // -----------------------------------------------------------------

    #[test]
    fn whitespace_includes_vt_and_ff() {
        let t = Tokenizer::for_grammar(&G_BASIC);
        // Source mit VT (0x0B) und FF (0x0C) zwischen Tokens. Spec §7.2.1
        // zaehlt beide zu "white space".
        let s = t
            .tokenize("struct\x0bA\x0c{}\x0b;")
            .expect("VT/FF muessen Whitespace sein");
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
    // §7.2.2 — Comment-Marker innerhalb Comments haben keine Bedeutung
    // (Phase 1.2)
    // -----------------------------------------------------------------

    #[test]
    fn line_comment_contains_block_comment_start() {
        // `//` startet Line-Comment, `/*` darin hat keine Bedeutung.
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
        // `//` und `/*` innerhalb `/* … */` haben keine Bedeutung.
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
        // `/* /* inner */ rest */` — der erste `*/` schliesst den Block,
        // ` rest */` bleibt als Source. Spec §7.2.2: comments do not nest.
        let t = Tokenizer::for_grammar(&G_BASIC);
        let result = t.tokenize("struct /* /* inner */ A {} ;");
        // ` A ` wird Ident, dann `{} ;`. Aber "inner" wurde durch das erste
        // `*/` abgeschlossen — daher kommt nach `*/` zunaechst der Token
        // `A`, was Ident ist. Vor dem Test pruefen wir nur, dass nichts
        // crasht und `A` als Ident gefunden wird.
        let s = result.expect("must succeed");
        let kinds: Vec<_> = s.iter().map(|t| t.kind).collect();
        assert!(kinds.contains(&TokenKind::Ident));
    }

    // -----------------------------------------------------------------
    // §7.2.4 — Vollstaendigkeitstest fuer alle 73 IDL-Keywords
    // (Phase 1.3)
    // -----------------------------------------------------------------

    #[test]
    fn all_table_7_6_keywords_classified_as_keyword() {
        use crate::grammar::idl42::IDL_42;
        // 73 Keywords aus Spec §7.2.4 Table 7-6.
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
                .unwrap_or_else(|e| panic!("Keyword {kw} muss lexbar sein: {e:?}"));
            assert_eq!(s.len(), 1, "Keyword {kw}: erwarte genau 1 Token");
            assert_eq!(
                s.tokens()[0].kind,
                TokenKind::Keyword(kw),
                "Keyword {kw}: TokenKind muss Keyword({kw}) sein, ist {:?}",
                s.tokens()[0].kind
            );
        }
    }

    // -----------------------------------------------------------------
    // §7.2.6.4 — Floating-Point Edge Cases (Phase 1.5)
    // -----------------------------------------------------------------

    #[test]
    fn float_no_int_part() {
        // `.5e10` ohne integer-part vor dem Dot.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize(".5e10").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FloatLiteral);
        assert_eq!(s.tokens()[0].text, ".5e10");
    }

    #[test]
    fn float_no_fraction_part() {
        // `5.e10` ohne fraction-part nach dem Dot.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("5.e10").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FloatLiteral);
        assert_eq!(s.tokens()[0].text, "5.e10");
    }

    #[test]
    fn float_no_decimal_point_only_exponent() {
        // `5e10` — kein Dot, nur Exponent.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("5e10").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FloatLiteral);
    }

    #[test]
    fn float_no_exponent_only_decimal_point() {
        // `5.5` — Dot ohne Exponent.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("5.5").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FloatLiteral);
    }

    #[test]
    fn float_dot_alone_is_punct_not_float() {
        // `.` allein ohne Digit auf einer Seite ist Punct, kein Float.
        // Spec §7.2.6.4: "Either the integer part or the fraction part
        // (but not both) may be missing."
        let t = Tokenizer::for_grammar(&G_BASIC);
        let result = t.tokenize(".");
        // G_BASIC kennt keinen `.`-Punct, daher unknown-character-Error.
        // Hauptpunkt: kein FloatLiteral.
        assert!(
            result.is_err(),
            "`.` allein darf nicht als FloatLiteral gelten"
        );
    }

    // -----------------------------------------------------------------
    // §7.2.6.5 — Fixed-Point Edge Cases (Phase 1.6)
    // -----------------------------------------------------------------

    #[test]
    fn fixed_no_int_part() {
        // `.5d` ohne integer-part.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize(".5d").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FixedPtLiteral);
    }

    #[test]
    fn fixed_no_fraction_part() {
        // `5.d` ohne fraction-part.
        let t = Tokenizer::for_grammar(&G_LITERALS);
        let s = t.tokenize("5.d").expect("must succeed");
        assert_eq!(s.len(), 1);
        assert_eq!(s.tokens()[0].kind, TokenKind::FixedPtLiteral);
    }

    #[test]
    fn fixed_no_decimal_point() {
        // `5d` — kein Dot. Spec §7.2.6.5: "the decimal point (but not the
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
        // `5.5` ohne d-Suffix → FloatLiteral, nicht FixedPtLiteral.
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
