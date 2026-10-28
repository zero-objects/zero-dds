// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Tokenizer for content-filter expressions.
//!
//! All keywords case-insensitive. String literals: `'...'` with
//! `''` escaping (SQL-92).

use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Ident(String),
    StrLit(String),
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    /// `NULLVALUE` (XTypes 1.3 §7.6.7.3).
    Null,
    Param(u32),
    Eq,
    Neq,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    Like,
    Between,
    LParen,
    RParen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LexError(pub String);

pub(crate) fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];

        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Parentheses / single-char ops.
        match c {
            b'(' => {
                out.push(Token::LParen);
                i += 1;
                continue;
            }
            b')' => {
                out.push(Token::RParen);
                i += 1;
                continue;
            }
            b'=' => {
                out.push(Token::Eq);
                i += 1;
                continue;
            }
            _ => {}
        }

        // Multi-char operators.
        if c == b'!' && bytes.get(i + 1) == Some(&b'=') {
            out.push(Token::Neq);
            i += 2;
            continue;
        }
        if c == b'<' {
            if bytes.get(i + 1) == Some(&b'>') {
                out.push(Token::Neq);
                i += 2;
                continue;
            }
            if bytes.get(i + 1) == Some(&b'=') {
                out.push(Token::Le);
                i += 2;
                continue;
            }
            out.push(Token::Lt);
            i += 1;
            continue;
        }
        if c == b'>' {
            if bytes.get(i + 1) == Some(&b'=') {
                out.push(Token::Ge);
                i += 2;
                continue;
            }
            out.push(Token::Gt);
            i += 1;
            continue;
        }

        // Parameter: %0, %123.
        if c == b'%' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j == i + 1 {
                return Err(LexError(alloc::format!(
                    "empty parameter index at position {i}"
                )));
            }
            let digits = &input[i + 1..j];
            let idx: u32 = digits
                .parse()
                .map_err(|_| LexError(alloc::format!("Parameter-Index '{digits}' > u32::MAX")))?;
            out.push(Token::Param(idx));
            i = j;
            continue;
        }

        // String literal: '...' with '' -> '. Annex B.2 STRING: "Any series
        // of characters encapsulated in single quotes, except a new-line
        // character". The characters are copied whole, not byte by byte.
        if c == b'\'' {
            let mut s = String::new();
            let mut chars = input[i + 1..].char_indices().peekable();
            let mut end = None;
            while let Some((offset, ch)) = chars.next() {
                match ch {
                    '\'' if matches!(chars.peek(), Some((_, '\''))) => {
                        s.push('\'');
                        chars.next();
                    }
                    '\'' => {
                        end = Some(i + 1 + offset + 1);
                        break;
                    }
                    '\n' | '\r' => {
                        return Err(LexError(alloc::format!(
                            "a string literal may not contain a new-line, at position {}",
                            i + 1 + offset
                        )));
                    }
                    other => s.push(other),
                }
            }
            let Some(j) = end else {
                return Err(LexError("unterminated string literal".into()));
            };
            out.push(Token::StrLit(s));
            i = j;
            continue;
        }

        // Numeric literal. Annex B.2 INTEGERVALUE: "Any series of digits,
        // optionally preceded by a plus or minus sign … A hexadecimal number
        // is preceded by 0x and must be a valid hexadecimal expression."
        // FLOATVALUE: digits, optionally signed, optionally with a '.', and
        // a power of ten "en" with an optionally signed n.
        let signed = c == b'-' || c == b'+';
        if c.is_ascii_digit() || (signed && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            let mut j = i;
            if signed {
                j += 1;
            }
            // 0x…
            if bytes.get(j) == Some(&b'0') && matches!(bytes.get(j + 1), Some(b'x' | b'X')) {
                let digits_start = j + 2;
                let mut k = digits_start;
                while bytes.get(k).is_some_and(u8::is_ascii_hexdigit) {
                    k += 1;
                }
                if k == digits_start {
                    return Err(LexError(alloc::format!(
                        "a hexadecimal number needs digits after 0x, at position {i}"
                    )));
                }
                if bytes
                    .get(k)
                    .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
                {
                    return Err(LexError(alloc::format!(
                        "not a hexadecimal number, at position {i}"
                    )));
                }
                let magnitude = i128::from_str_radix(&input[digits_start..k], 16)
                    .map_err(|_| LexError(alloc::format!("not an integer: '{}'", &input[i..k])))?;
                let value = if c == b'-' { -magnitude } else { magnitude };
                let n = i64::try_from(value)
                    .map_err(|_| LexError(alloc::format!("not an integer: '{}'", &input[i..k])))?;
                out.push(Token::IntLit(n));
                i = k;
                continue;
            }
            let mut saw_dot = false;
            let mut saw_exp = false;
            while j < bytes.len() {
                let d = bytes[j];
                if d.is_ascii_digit() {
                    j += 1;
                } else if d == b'.' && !saw_dot && !saw_exp {
                    saw_dot = true;
                    j += 1;
                } else if (d == b'e' || d == b'E') && !saw_exp {
                    saw_exp = true;
                    j += 1;
                    if let Some(&nx) = bytes.get(j) {
                        if nx == b'+' || nx == b'-' {
                            j += 1;
                        }
                    }
                } else {
                    break;
                }
            }
            let slice = &input[i..j];
            if saw_dot || saw_exp {
                let f: f64 = slice
                    .parse()
                    .map_err(|_| LexError(alloc::format!("not a float: '{slice}'")))?;
                out.push(Token::FloatLit(f));
            } else {
                let n: i64 = slice
                    .parse()
                    .map_err(|_| LexError(alloc::format!("not an integer: '{slice}'")))?;
                out.push(Token::IntLit(n));
            }
            i = j;
            continue;
        }

        // Identifier or keyword. Allowed: [A-Za-z_][A-Za-z0-9_.]*, each
        // name optionally followed by subscripts: `[0]` (array or
        // sequence element), `['key']` (map value or bitmask flag),
        // XTypes 1.3 §7.6.7.1.
        if c.is_ascii_alphabetic() || c == b'_' {
            let (word, j, subscripted) = scan_field(input, i)?;
            if subscripted {
                out.push(Token::Ident(word));
                i = j;
                continue;
            }
            let upper = {
                let mut s = String::with_capacity(word.len());
                for b in word.bytes() {
                    s.push(b.to_ascii_uppercase() as char);
                }
                s
            };
            let tok = match upper.as_str() {
                "AND" => Token::And,
                "OR" => Token::Or,
                "NOT" => Token::Not,
                "LIKE" => Token::Like,
                "BETWEEN" => Token::Between,
                "TRUE" => Token::BoolLit(true),
                "FALSE" => Token::BoolLit(false),
                // §7.6.7.3: "The NULLVALUE token shall always be null."
                _ if word == "null" => Token::Null,
                _ => Token::Ident(word),
            };
            out.push(tok);
            i = j;
            continue;
        }

        return Err(LexError(alloc::format!(
            "unexpected character '{}' at position {i}",
            c as char
        )));
    }
    Ok(out)
}

/// Scans a field reference from `start`: names joined by `.`, each one
/// optionally followed by subscripts. Returns the canonical text (no
/// blanks inside the brackets, `'` doubled inside a key), the position
/// after it and whether it carries a subscript.
///
/// A subscript is a non-negative integer (§7.6.7.1: "zero-based
/// subscript") or a single-quoted string (a map key or a bitmask flag
/// name, "according to the syntax of STRING").
fn scan_field(input: &str, start: usize) -> Result<(String, usize, bool), LexError> {
    let bytes = input.as_bytes();
    let mut text = String::new();
    let mut j = start;
    let mut subscripted = false;
    loop {
        let name_start = j;
        while j < bytes.len()
            && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b'.')
        {
            j += 1;
        }
        if subscripted && j > name_start && bytes[name_start] != b'.' {
            return Err(LexError(alloc::format!(
                "expected '.' after ']' at position {name_start}"
            )));
        }
        text.push_str(&input[name_start..j]);
        if bytes.get(j) != Some(&b'[') {
            return Ok((text, j, subscripted));
        }
        if j == name_start {
            return Err(LexError(alloc::format!(
                "a subscript without a name before it at position {j}"
            )));
        }
        subscripted = true;
        j += 1;
        while bytes.get(j).is_some_and(u8::is_ascii_whitespace) {
            j += 1;
        }
        text.push('[');
        match bytes.get(j) {
            Some(d) if d.is_ascii_digit() => {
                let digits_start = j;
                while bytes.get(j).is_some_and(u8::is_ascii_digit) {
                    j += 1;
                }
                let digits = &input[digits_start..j];
                let index: u32 = digits
                    .parse()
                    .map_err(|_| LexError(alloc::format!("subscript '{digits}' > u32::MAX")))?;
                text.push_str(&alloc::format!("{index}"));
            }
            Some(b'\'') => {
                j += 1;
                text.push('\'');
                loop {
                    match bytes.get(j) {
                        None => return Err(LexError("unterminated subscript key".into())),
                        Some(b'\'') if bytes.get(j + 1) == Some(&b'\'') => {
                            text.push_str("''");
                            j += 2;
                        }
                        Some(b'\'') => {
                            text.push('\'');
                            j += 1;
                            break;
                        }
                        Some(_) => {
                            // Whole characters: copy up to the next quote.
                            let chunk_start = j;
                            while j < bytes.len() && bytes[j] != b'\'' {
                                j += 1;
                            }
                            text.push_str(&input[chunk_start..j]);
                        }
                    }
                }
            }
            _ => {
                return Err(LexError(alloc::format!(
                    "a subscript is a non-negative integer or a quoted key, at position {j}"
                )));
            }
        }
        while bytes.get(j).is_some_and(u8::is_ascii_whitespace) {
            j += 1;
        }
        if bytes.get(j) != Some(&b']') {
            return Err(LexError(alloc::format!("expected ']' at position {j}")));
        }
        j += 1;
        text.push(']');
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_boolean_literal_is_true_or_false_in_any_case() {
        // XTypes 1.3 §7.6.7.3: "The BOOLEANVALUE token shall be either true
        // or false (case-insensitive)."
        for word in ["true", "TRUE", "True", "tRuE"] {
            assert_eq!(
                tokenize(&alloc::format!("flag = {word}")).expect("lex"),
                vec![Token::Ident("flag".into()), Token::Eq, Token::BoolLit(true)],
                "{word}"
            );
        }
        for word in ["false", "FALSE", "False", "fAlSe"] {
            assert_eq!(
                tokenize(&alloc::format!("flag = {word}")).expect("lex"),
                vec![
                    Token::Ident("flag".into()),
                    Token::Eq,
                    Token::BoolLit(false)
                ],
                "{word}"
            );
        }
    }

    #[test]
    fn lex_basic_comparison() {
        let t = tokenize("color = 'RED'").expect("lex");
        assert_eq!(
            t,
            vec![
                Token::Ident("color".into()),
                Token::Eq,
                Token::StrLit("RED".into()),
            ],
        );
    }

    #[test]
    fn lex_mixed_operators() {
        let t = tokenize("x <= 10 AND y <> 5").expect("lex");
        assert_eq!(
            t,
            vec![
                Token::Ident("x".into()),
                Token::Le,
                Token::IntLit(10),
                Token::And,
                Token::Ident("y".into()),
                Token::Neq,
                Token::IntLit(5),
            ],
        );
    }

    #[test]
    fn lex_parameter_and_float() {
        let t = tokenize("temp > %0 AND temp < 3.14e2").expect("lex");
        assert!(matches!(t[2], Token::Param(0)));
        assert!(matches!(t[6], Token::FloatLit(_)));
    }

    #[test]
    fn lex_string_with_escape_quote() {
        let t = tokenize("msg = 'O''Brien'").expect("lex");
        assert_eq!(t[2], Token::StrLit("O'Brien".into()));
    }

    #[test]
    fn lex_like_and_negation() {
        let t = tokenize("NOT name LIKE 'foo%'").expect("lex");
        assert_eq!(t[0], Token::Not);
        assert_eq!(t[2], Token::Like);
    }

    #[test]
    fn lex_rejects_bad_parameter() {
        assert!(tokenize("x = % 3").is_err());
    }

    #[test]
    fn a_subscripted_field_is_one_identifier() {
        assert_eq!(
            tokenize("a.b[ 3 ].c['k''q']").expect("lex"),
            [Token::Ident("a.b[3].c['k''q']".into())]
        );
    }

    #[test]
    fn null_is_a_token_of_its_own() {
        assert_eq!(
            tokenize("x = null").expect("lex"),
            [Token::Ident("x".into()), Token::Eq, Token::Null]
        );
    }

    /// Annex B.2 INTEGERVALUE: a plus sign, and a number "preceded by 0x".
    #[test]
    fn integers_take_a_plus_sign_and_hexadecimal() {
        assert_eq!(tokenize("+5").expect("lex"), [Token::IntLit(5)]);
        assert_eq!(tokenize("-5").expect("lex"), [Token::IntLit(-5)]);
        assert_eq!(tokenize("0x1F").expect("lex"), [Token::IntLit(31)]);
        assert_eq!(tokenize("-0X10").expect("lex"), [Token::IntLit(-16)]);
        assert_eq!(tokenize("+0xff").expect("lex"), [Token::IntLit(255)]);
        assert_eq!(tokenize("+2.5").expect("lex"), [Token::FloatLit(2.5)]);
        assert_eq!(tokenize("+1e2").expect("lex"), [Token::FloatLit(100.0)]);
    }

    /// "A hexadecimal number … must be a valid hexadecimal expression."
    #[test]
    fn a_malformed_hexadecimal_is_refused() {
        for bad in ["0x", "0xG1", "0x1Fz", "0x8000000000000000"] {
            assert!(tokenize(bad).is_err(), "{bad}");
        }
    }

    /// Annex B.2 STRING: any characters but a new-line; a character outside
    /// ASCII is one character, not its bytes.
    #[test]
    fn a_string_keeps_its_characters_and_refuses_a_new_line() {
        assert_eq!(tokenize("'ü€'").expect("lex"), [Token::StrLit("ü€".into())]);
        assert!(tokenize("'a\nb'").is_err());
        assert!(tokenize("'a\rb'").is_err());
    }

    #[test]
    fn a_malformed_subscript_is_refused() {
        for bad in ["a[", "a[]", "a[-1]", "a[x]", "a['k]", "a[1]b", "[1]"] {
            assert!(tokenize(bad).is_err(), "{bad}");
        }
    }
}
