// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The lexer. Spec §1.
//!
//! Total by construction: every byte of the input belongs to exactly one
//! token or one piece of trivia, and the lexer never fails. An
//! unterminated string or an illegal character becomes an `Error` token
//! covering its range, so the stream stays complete and the tree still
//! round-trips.

use crate::span::Span;
use crate::token::{KEYWORDS, Radix, Token, TokenKind, TokenValue, Trivia, TriviaKind};
use alloc::string::String;
use alloc::vec::Vec;

/// Lex a whole file.
///
/// The returned stream always ends in one [`TokenKind::Eof`] token
/// carrying the file's trailing trivia.
#[must_use]
pub fn lex(source: &str) -> Vec<Token> {
    Lexer::new(source).collect_all()
}

/// The value of a single literal token, from its own text.
///
/// The green tree carries text rather than values, so a consumer that
/// needs the number behind `0x1F` or the content behind `"a\nb"` re-lexes
/// exactly that token. One implementation of radix and escape handling
/// rather than a second one in every consumer.
///
/// Returns [`TokenValue::None`] when the text is not one whole token of
/// the given kind — a caller cannot mistake a partial read for a value.
#[must_use]
pub fn literal_value(kind: TokenKind, text: &str) -> TokenValue {
    let mut lexer = Lexer::new(text);
    let token = lexer.next_token();
    if token.kind != kind || token.span.end as usize != text.len() || token.span.start != 0 {
        return TokenValue::None;
    }
    token.value
}

/// The lexer.
pub struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Lexer<'a> {
    /// A lexer over `source`.
    #[must_use]
    pub const fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            offset: 0,
        }
    }

    /// Lex the whole input.
    #[must_use]
    pub fn collect_all(mut self) -> Vec<Token> {
        let mut out = Vec::new();
        loop {
            let token = self.next_token();
            let is_eof = token.kind == TokenKind::Eof;
            out.push(token);
            if is_eof {
                return out;
            }
        }
    }

    /// The next token, with the trivia that precedes it.
    pub fn next_token(&mut self) -> Token {
        let leading = self.scan_trivia();
        let start = self.offset;

        let Some(byte) = self.peek() else {
            return Token {
                kind: TokenKind::Eof,
                span: Span::new(self.pos(), self.pos()),
                leading,
                value: TokenValue::None,
            };
        };

        // `L'x'` and `L"x"` are wide literals. This has to be decided
        // before the identifier rule, because `L` is alphabetic and
        // would otherwise be consumed as a name.
        if byte == b'L' {
            match self.peek_at(1) {
                Some(b'\'') => {
                    self.bump();
                    return self.lex_char(start, leading, true);
                }
                Some(b'"') => {
                    self.bump();
                    return self.lex_string(start, leading, true);
                }
                _ => {}
            }
        }

        // Identifiers and keywords, including the `_name` escape.
        if byte == b'_' || byte.is_ascii_alphabetic() {
            return self.lex_word(start, leading);
        }

        if byte.is_ascii_digit()
            || (byte == b'.' && self.peek_at(1).is_some_and(|b| b.is_ascii_digit()))
        {
            return self.lex_number(start, leading);
        }

        match byte {
            b'\'' => self.lex_char(start, leading, false),
            b'"' => self.lex_string(start, leading, false),
            _ => self.lex_punct(start, leading),
        }
    }

    // -- trivia ----------------------------------------------------------

    fn scan_trivia(&mut self) -> Vec<Trivia> {
        let mut out = Vec::new();
        loop {
            let start = self.offset;
            let Some(byte) = self.peek() else {
                return out;
            };
            let kind = match byte {
                b'\n' => {
                    self.bump();
                    TriviaKind::Newline
                }
                b' ' | b'\t' | b'\r' => {
                    while self
                        .peek()
                        .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r'))
                    {
                        self.bump();
                    }
                    TriviaKind::Whitespace
                }
                b'/' if self.peek_at(1) == Some(b'/') => {
                    while self.peek().is_some_and(|b| b != b'\n') {
                        self.bump();
                    }
                    TriviaKind::LineComment
                }
                b'/' if self.peek_at(1) == Some(b'*') => {
                    self.bump();
                    self.bump();
                    // An unterminated block comment runs to end of file
                    // rather than failing; the bytes still belong to it.
                    while self.peek().is_some() {
                        if self.peek() == Some(b'*') && self.peek_at(1) == Some(b'/') {
                            self.bump();
                            self.bump();
                            break;
                        }
                        self.bump();
                    }
                    TriviaKind::BlockComment
                }
                b'#' if self.at_line_start(start) => {
                    // A preprocessor line is trivia here; the
                    // preprocessor interprets it. Line continuations
                    // keep it going.
                    while let Some(b) = self.peek() {
                        if b == b'\\' && self.peek_at(1) == Some(b'\n') {
                            self.bump();
                            self.bump();
                            continue;
                        }
                        if b == b'\n' {
                            break;
                        }
                        self.bump();
                    }
                    TriviaKind::Directive
                }
                _ => return out,
            };
            out.push(Trivia {
                kind,
                span: Span::new(start as u32, self.pos()),
            });
        }
    }

    /// Whether `at` is the first non-blank byte of its line.
    fn at_line_start(&self, at: usize) -> bool {
        let mut index = at;
        while index > 0 {
            index -= 1;
            match self.bytes.get(index) {
                Some(b'\n') => return true,
                Some(b' ' | b'\t' | b'\r') => {}
                _ => return false,
            }
        }
        true
    }

    // -- words -----------------------------------------------------------

    fn lex_word(&mut self, start: usize, leading: Vec<Trivia>) -> Token {
        // IDL escapes an identifier that collides with a keyword by
        // prefixing an underscore. The underscore is not part of the
        // name. OMG IDL 4.2 §7.2.3.
        let escaped = self.peek() == Some(b'_');
        if escaped {
            self.bump();
        }
        while self
            .peek()
            .is_some_and(|b| b == b'_' || b.is_ascii_alphanumeric())
        {
            self.bump();
        }
        let span = Span::new(start as u32, self.pos());
        let text = span.slice(self.source).unwrap_or("");
        let word = if escaped { &text[1..] } else { text };

        if escaped {
            return Token {
                kind: TokenKind::Ident,
                span,
                leading,
                value: TokenValue::Ident { escaped: true },
            };
        }

        // Keyword recognition is exact. OMG IDL 4.2 §7.2.4, p. 20:
        // "Keywords must be written exactly as shown in the above list.
        // Identifiers that collide with keywords (see 7.2.3,
        // Identifiers) are illegal."
        //
        // Two rules, and this used to be one match with
        // `eq_ignore_ascii_case`, which made the identifier `CUSTOM`
        // *become* the keyword `custom` — a silently wrong parse of a
        // file the author meant as an enumerator. Recognition belongs
        // here; the collision belongs to §7.2.3 and is reported by name
        // in the lowering, where the keyword it collides with can be
        // said out loud. `keyword_collision` is that question.
        let kind = KEYWORDS
            .iter()
            .find(|(name, _)| *name == word)
            .map_or(TokenKind::Ident, |(_, kind)| *kind);

        Token {
            kind,
            span,
            leading,
            value: if kind == TokenKind::Ident {
                TokenValue::Ident { escaped: false }
            } else {
                TokenValue::None
            },
        }
    }

    // -- numbers ---------------------------------------------------------

    #[allow(clippy::too_many_lines)]
    fn lex_number(&mut self, start: usize, leading: Vec<Trivia>) -> Token {
        let mut radix = Radix::Decimal;
        let mut is_float = false;

        if self.peek() == Some(b'0') {
            match self.peek_at(1) {
                Some(b'x' | b'X') => {
                    radix = Radix::Hex;
                    self.bump();
                    self.bump();
                    while self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
                        self.bump();
                    }
                }
                // A leading zero makes it octal only if the digit run
                // ends there. `0123.450d` is §7.4.1.4.3's own worked
                // example of a fixed-point literal — "considered to be
                // fixed<7,3>" — and reading it as octal `0123` followed
                // by `.450d` produced two tokens and a parse error on
                // the second. §7.2.6.1 gives an octal literal digits and
                // nothing after them; a point, an exponent or a `d`
                // suffix means the run was the integer part of a
                // decimal number all along.
                Some(b'0'..=b'7') if !self.decimal_ahead() => {
                    radix = Radix::Octal;
                    self.bump();
                    while self.peek().is_some_and(|b| (b'0'..=b'7').contains(&b)) {
                        self.bump();
                    }
                }
                _ => {
                    self.bump();
                }
            }
        }

        if radix == Radix::Decimal {
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.bump();
            }
            // A point followed by a digit, or a trailing point: `1.` is
            // a float and stays distinguishable from `1.0`.
            if self.peek() == Some(b'.') {
                is_float = true;
                self.bump();
                while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                    self.bump();
                }
            }
            if self.peek().is_some_and(|b| matches!(b, b'e' | b'E')) {
                is_float = true;
                self.bump();
                if self.peek().is_some_and(|b| matches!(b, b'+' | b'-')) {
                    self.bump();
                }
                while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                    self.bump();
                }
            }
        }

        // A `d` or `D` suffix makes it fixed-point. IDL 4.2 §7.2.6.4.
        if self.peek().is_some_and(|b| matches!(b, b'd' | b'D')) {
            self.bump();
            return Token {
                kind: TokenKind::FixedLiteral,
                span: Span::new(start as u32, self.pos()),
                leading,
                value: TokenValue::None,
            };
        }

        let span = Span::new(start as u32, self.pos());
        if is_float {
            return Token {
                kind: TokenKind::FloatLiteral,
                span,
                leading,
                value: TokenValue::None,
            };
        }

        let text = span.slice(self.source).unwrap_or("0");
        let value = match radix {
            Radix::Hex => i128::from_str_radix(text.get(2..).unwrap_or("0"), 16),
            Radix::Octal => i128::from_str_radix(text.get(1..).unwrap_or("0"), 8),
            Radix::Decimal => text.parse::<i128>(),
        };

        match value {
            Ok(value) => Token {
                kind: TokenKind::IntLiteral,
                span,
                leading,
                value: TokenValue::Int { value, radix },
            },
            // A literal too large for i128 is an error token rather than
            // a truncated value: a silently wrapped bound would be worse
            // than a rejected file.
            Err(_) => Token {
                kind: TokenKind::Error,
                span,
                leading,
                value: TokenValue::None,
            },
        }
    }

    /// Whether the digit run starting here is the integer part of a
    /// decimal number rather than an octal literal.
    ///
    /// Looks past the digits — every digit, not only the octal ones, so
    /// `0189.5d` is answered as well — and reads what follows: a point,
    /// an exponent or a fixed-point suffix. Nothing is consumed.
    fn decimal_ahead(&self) -> bool {
        let mut at = 1;
        while self.peek_at(at).is_some_and(|b| b.is_ascii_digit()) {
            at += 1;
        }
        matches!(self.peek_at(at), Some(b'.' | b'e' | b'E' | b'd' | b'D'))
    }

    // -- character and string literals -----------------------------------

    /// A `'c'` or `L'c'` literal.
    ///
    /// # A raw non-ASCII character is one character and not its last
    /// octet
    ///
    /// This read the source a byte at a time and kept the last one, so
    /// `case 'ä':` — C3 A4 in a UTF-8 file — became U+00A4 rather than
    /// U+00E4, silently. It was found by
    /// `docs/corba-dialect-followup.md` item 4, printing the model back
    /// as CORBA IDL, and the escaped form `'\xe4'` was right all along —
    /// only a raw character was affected, and no corpus case has one,
    /// because every `char` case label in the corpus is ASCII where the
    /// last octet and the code point coincide.
    ///
    /// The reading is the broadest of the three the clause admits.
    /// §7.4.1.1.2 says a Char8 "shall not be interpreted to have a
    /// specific encoding", which leaves open whether `'ä'` in a UTF-8
    /// source means U+00E4, means the two octets C3 A4, or is an error.
    /// It means the character the author typed: an IDL file is text, the
    /// two-octet reading has nowhere to put a second octet in a
    /// `TokenValue::Char`, and the third reading throws away a
    /// declaration this project can represent exactly.
    ///
    /// What is refused rather than narrowed is a character the *type*
    /// cannot hold — §7.2.2.2.1.2.2 makes a `CHAR_8_TYPE` "representable
    /// using 8 bits", and §7.2.2.2.1.2.5 restricts a `CHAR_16_TYPE` to
    /// the Basic Multilingual Plane. `zerodds_wire_probe::contract`
    /// already refuses both for a *value*; a literal is the same
    /// question asked in the source file.
    ///
    /// A literal holding more than one character is refused for the same
    /// reason: keeping the last one is how `'ä'` became U+00A4 in the
    /// first place.
    fn lex_char(&mut self, start: usize, leading: Vec<Trivia>, wide: bool) -> Token {
        self.bump(); // opening quote
        let mut value = 0u32;
        let mut characters = 0usize;
        let mut terminated = false;
        while let Some(byte) = self.peek() {
            if byte == b'\'' {
                self.bump();
                terminated = true;
                break;
            }
            if byte == b'\n' {
                break;
            }
            if byte == b'\\' {
                self.bump();
                value = self.scan_escape();
            } else {
                value = self.scan_utf8_scalar(byte);
            }
            characters += 1;
        }
        let representable = if wide { value <= 0xFFFF } else { value <= 0xFF };
        let span = Span::new(start as u32, self.pos());
        Token {
            kind: if !terminated || characters != 1 || !representable {
                TokenKind::Error
            } else if wide {
                TokenKind::WCharLiteral
            } else {
                TokenKind::CharLiteral
            },
            span,
            leading,
            value: TokenValue::Char { value },
        }
    }

    /// One Unicode scalar value out of the UTF-8 source, consumed whole.
    ///
    /// A malformed sequence yields the leading octet's value and
    /// consumes only that octet, which makes the literal a
    /// [`TokenKind::Error`] through the representability check above for
    /// every continuation octet — `0x80..=0xBF` is above `0xFF`? No: it
    /// is not, so a lone continuation octet is instead caught by the
    /// character count, because the octets around it are characters of
    /// their own. Either way the literal does not become a code point
    /// nobody wrote.
    fn scan_utf8_scalar(&mut self, first: u8) -> u32 {
        // The leading octet is the caller's, which is how this avoids a
        // fallback: `self.peek()` has already answered `Some` in the
        // loop above, and an `unwrap_or(0)` here would turn a
        // hypothetical end of input into a NUL character nobody wrote.
        self.bump();
        let (mut value, extra) = match first {
            0x00..=0x7F => return u32::from(first),
            0xC2..=0xDF => (u32::from(first & 0x1F), 1),
            0xE0..=0xEF => (u32::from(first & 0x0F), 2),
            0xF0..=0xF4 => (u32::from(first & 0x07), 3),
            // A continuation octet or an over-long lead: not the start
            // of a scalar. Returned as itself so the literal is one
            // character of a value the caller can still refuse.
            _ => return u32::from(first),
        };
        for _ in 0..extra {
            match self.peek() {
                Some(byte @ 0x80..=0xBF) => {
                    value = (value << 6) | u32::from(byte & 0x3F);
                    self.bump();
                }
                // Truncated. What has been decoded so far is returned;
                // it is not a scalar the source contained, and the
                // literal is refused by the checks above unless it
                // happens to be representable, in which case the file is
                // not UTF-8 and the parser's own diagnostics apply.
                _ => return value,
            }
        }
        value
    }

    fn lex_string(&mut self, start: usize, leading: Vec<Trivia>, wide: bool) -> Token {
        self.bump(); // opening quote
        let mut text = String::new();
        let mut terminated = false;
        while let Some(byte) = self.peek() {
            if byte == b'"' {
                self.bump();
                terminated = true;
                break;
            }
            if byte == b'\n' {
                break;
            }
            if byte == b'\\' {
                self.bump();
                if let Some(ch) = char::from_u32(self.scan_escape()) {
                    text.push(ch);
                }
            } else {
                // Multi-byte UTF-8 passes through intact.
                let from = self.offset;
                self.bump_char();
                if let Some(slice) = self.source.get(from..self.offset) {
                    text.push_str(slice);
                }
            }
        }
        let span = Span::new(start as u32, self.pos());
        Token {
            kind: if !terminated {
                TokenKind::Error
            } else if wide {
                TokenKind::WStringLiteral
            } else {
                TokenKind::StringLiteral
            },
            span,
            leading,
            value: TokenValue::Text { value: text },
        }
    }

    /// Consume one escape sequence after the backslash, returning its
    /// code point. IDL 4.2 §7.2.6.2.
    fn scan_escape(&mut self) -> u32 {
        let Some(byte) = self.peek() else {
            return 0;
        };
        self.bump();
        match byte {
            b'n' => u32::from(b'\n'),
            b't' => u32::from(b'\t'),
            b'v' => 0x0b,
            b'b' => 0x08,
            b'r' => u32::from(b'\r'),
            b'f' => 0x0c,
            b'a' => 0x07,
            b'\\' => u32::from(b'\\'),
            b'?' => u32::from(b'?'),
            b'\'' => u32::from(b'\''),
            b'"' => u32::from(b'"'),
            b'0'..=b'7' => {
                let mut value = u32::from(byte - b'0');
                let mut digits = 1;
                while digits < 3 && self.peek().is_some_and(|b| (b'0'..=b'7').contains(&b)) {
                    if let Some(next) = self.peek() {
                        value = value * 8 + u32::from(next - b'0');
                        self.bump();
                        digits += 1;
                    }
                }
                value
            }
            b'x' | b'X' => {
                let mut value = 0u32;
                let mut digits = 0;
                while digits < 2 && self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
                    if let Some(next) = self.peek() {
                        value = value * 16 + u32::from(hex_value(next));
                        self.bump();
                        digits += 1;
                    }
                }
                value
            }
            b'u' | b'U' => {
                let mut value = 0u32;
                let mut digits = 0;
                while digits < 4 && self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
                    if let Some(next) = self.peek() {
                        value = value * 16 + u32::from(hex_value(next));
                        self.bump();
                        digits += 1;
                    }
                }
                value
            }
            other => u32::from(other),
        }
    }

    // -- punctuation -----------------------------------------------------

    fn lex_punct(&mut self, start: usize, leading: Vec<Trivia>) -> Token {
        let Some(byte) = self.peek() else {
            return Token {
                kind: TokenKind::Eof,
                span: Span::new(start as u32, self.pos()),
                leading,
                value: TokenValue::None,
            };
        };
        self.bump();

        let kind = match byte {
            b';' => TokenKind::Semicolon,
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b',' => TokenKind::Comma,
            b'=' => TokenKind::Equals,
            b'+' => TokenKind::Plus,
            b'-' => TokenKind::Minus,
            b'*' => TokenKind::Star,
            b'/' => TokenKind::Slash,
            b'%' => TokenKind::Percent,
            b'~' => TokenKind::Tilde,
            b'|' => TokenKind::Pipe,
            b'^' => TokenKind::Caret,
            b'&' => TokenKind::Ampersand,
            b'@' => TokenKind::At,
            b'#' => TokenKind::Hash,
            b':' => {
                if self.peek() == Some(b':') {
                    self.bump();
                    TokenKind::ColonColon
                } else {
                    TokenKind::Colon
                }
            }
            b'<' => {
                if self.peek() == Some(b'<') {
                    self.bump();
                    TokenKind::LessLess
                } else {
                    TokenKind::Less
                }
            }
            // One token. Whether it closes two type arguments or shifts
            // is the parser's call. Spec §1.2, §2.3.
            b'>' => {
                if self.peek() == Some(b'>') {
                    self.bump();
                    TokenKind::GreaterGreater
                } else {
                    TokenKind::Greater
                }
            }
            _ => {
                // An illegal byte: consume the whole run of them so one
                // stray character does not produce one error per byte.
                while self.peek().is_some_and(|b| !is_recognised_start(b)) {
                    self.bump();
                }
                TokenKind::Error
            }
        };

        Token {
            kind,
            span: Span::new(start as u32, self.pos()),
            leading,
            value: TokenValue::None,
        }
    }

    // -- cursor ----------------------------------------------------------

    const fn peek(&self) -> Option<u8> {
        if self.offset < self.bytes.len() {
            Some(self.bytes[self.offset])
        } else {
            None
        }
    }

    const fn peek_at(&self, ahead: usize) -> Option<u8> {
        let at = self.offset + ahead;
        if at < self.bytes.len() {
            Some(self.bytes[at])
        } else {
            None
        }
    }

    const fn bump(&mut self) {
        if self.offset < self.bytes.len() {
            self.offset += 1;
        }
    }

    /// Advance one character, so multi-byte UTF-8 is not split.
    fn bump_char(&mut self) {
        let rest = self.source.get(self.offset..).unwrap_or("");
        let width = rest.chars().next().map_or(1, char::len_utf8);
        self.offset = (self.offset + width).min(self.bytes.len());
    }

    #[allow(clippy::cast_possible_truncation)]
    const fn pos(&self) -> u32 {
        self.offset as u32
    }
}

const fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

/// Whether a byte can begin a token or trivia.
///
/// Used to decide how far an error run reaches: everything up to the
/// next byte that could start something real.
const fn is_recognised_start(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'_' | b'\''
                | b'"'
                | b';'
                | b'{'
                | b'}'
                | b'('
                | b')'
                | b'['
                | b']'
                | b'<'
                | b'>'
                | b','
                | b':'
                | b'='
                | b'+'
                | b'-'
                | b'*'
                | b'/'
                | b'%'
                | b'~'
                | b'|'
                | b'^'
                | b'&'
                | b'@'
                | b'#'
                | b'.'
                | b' '
                | b'\t'
                | b'\r'
                | b'\n'
        )
}
