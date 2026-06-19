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
                    "leerer Parameter-Index an Position {i}"
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

        // String literal: '...' with '' -> '.
        if c == b'\'' {
            let mut s = String::new();
            let mut j = i + 1;
            loop {
                if j >= bytes.len() {
                    return Err(LexError("unterminiertes String-Literal".into()));
                }
                if bytes[j] == b'\'' {
                    if bytes.get(j + 1) == Some(&b'\'') {
                        s.push('\'');
                        j += 2;
                        continue;
                    }
                    j += 1;
                    break;
                }
                s.push(bytes[j] as char);
                j += 1;
            }
            out.push(Token::StrLit(s));
            i = j;
            continue;
        }

        // Numerisches Literal.
        if c.is_ascii_digit() || (c == b'-' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            let mut j = i;
            if c == b'-' {
                j += 1;
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

        // Identifier or keyword. Allowed: [A-Za-z_][A-Za-z0-9_.]*
        if c.is_ascii_alphabetic() || c == b'_' {
            let mut j = i + 1;
            while j < bytes.len() {
                let d = bytes[j];
                if d.is_ascii_alphanumeric() || d == b'_' || d == b'.' {
                    j += 1;
                } else {
                    break;
                }
            }
            let word = &input[i..j];
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
                _ => Token::Ident(word.into()),
            };
            out.push(tok);
            i = j;
            continue;
        }

        return Err(LexError(alloc::format!(
            "unerwartetes Zeichen '{}' an Position {i}",
            c as char
        )));
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

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
}
