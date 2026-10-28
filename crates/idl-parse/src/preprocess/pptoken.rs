// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Preprocessing tokens, ISO/IEC 14882:2003 §2.4.
//!
//! A macro's replacement list and an `#if` line are read the way the
//! preprocessor reads them and not the way the IDL lexer does: `#` and
//! `##` are operators there, a condition holds `!`, `&&` and `==`, and a
//! pp-number such as `1e` need not be an IDL literal at all. What an
//! expansion produces is lexed as IDL again when it is emitted, so this
//! tokenizer decides nothing about the IDL grammar.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// One preprocessing token and whether white space stood before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Piece {
    /// The token's spelling.
    pub text: String,
    /// White space or a comment came before it. `#` (§16.3.2) turns
    /// that into one space, and nothing else asks.
    pub space_before: bool,
}

/// The punctuators longer than one character, longest first, so a
/// longest match is the first match. §2.12.
const PUNCTUATORS: [&str; 23] = [
    "...", "<<=", ">>=", "##", "::", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "+=", "-=",
    "*=", "/=", "%=", "&=", "|=", "^=", "->", "++",
];

/// The preprocessing tokens of `text`, comments read as white space.
///
/// # Errors
/// The literal or comment that does not end, by what it is.
pub(crate) fn tokenize(text: &str) -> Result<Vec<Piece>, String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    let mut space = false;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte.is_ascii_whitespace() {
            space = true;
            at += 1;
            continue;
        }
        if byte == b'/' && bytes.get(at + 1) == Some(&b'/') {
            while at < bytes.len() && bytes[at] != b'\n' {
                at += 1;
            }
            space = true;
            continue;
        }
        if byte == b'/' && bytes.get(at + 1) == Some(&b'*') {
            let Some(end) = text[at + 2..].find("*/") else {
                return Err("a comment that does not end".to_string());
            };
            at += 2 + end + 2;
            space = true;
            continue;
        }
        let start = at;
        if byte == b'L' && matches!(bytes.get(at + 1), Some(b'\'' | b'"')) {
            at = quoted(bytes, at + 1)?;
        } else if is_identifier_start(byte) {
            while at < bytes.len() && is_identifier_continue(bytes[at]) {
                at += 1;
            }
        } else if byte.is_ascii_digit()
            || (byte == b'.' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit))
        {
            // §2.9: a pp-number is a digit or `.digit` followed by
            // digits, letters, underscores, periods and the four signed
            // exponents. It is wider than any literal on purpose.
            at += 1;
            while at < bytes.len() {
                let next = bytes[at];
                let signed_exponent =
                    matches!(next, b'+' | b'-') && matches!(bytes[at - 1], b'e' | b'E');
                if signed_exponent || next.is_ascii_alphanumeric() || matches!(next, b'_' | b'.') {
                    at += 1;
                } else {
                    break;
                }
            }
        } else if matches!(byte, b'\'' | b'"') {
            at = quoted(bytes, at)?;
        } else if let Some(punctuator) = PUNCTUATORS
            .iter()
            .find(|candidate| text[at..].starts_with(*candidate))
        {
            at += punctuator.len();
        } else {
            // One character, whole: `text` is UTF-8 and a multi-byte
            // character is not split.
            at += text[at..].chars().next().map_or(1, char::len_utf8);
        }
        out.push(Piece {
            text: text[start..at].to_string(),
            space_before: space,
        });
        space = false;
    }
    Ok(out)
}

/// The end of the character or string literal opening at `at`.
fn quoted(bytes: &[u8], at: usize) -> Result<usize, String> {
    let quote = bytes[at];
    let mut index = at + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'\n' => break,
            found if found == quote => return Ok(index + 1),
            _ => index += 1,
        }
    }
    Err(if quote == b'"' {
        "a string literal that does not end".to_string()
    } else {
        "a character literal that does not end".to_string()
    })
}

/// Whether `byte` starts an identifier. A byte past ASCII is taken as a
/// letter: the IDL lexer judges the identifier when it is emitted.
const fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte >= 0x80
}

const fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

/// Whether `text` is an identifier, and so a name a macro can have.
pub(crate) fn is_identifier(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes.next().is_some_and(is_identifier_start) && bytes.all(is_identifier_continue)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn texts(text: &str) -> Vec<String> {
        tokenize(text)
            .unwrap()
            .into_iter()
            .map(|p| p.text)
            .collect()
    }

    #[test]
    fn operators_are_taken_longest_first() {
        assert_eq!(texts("a##b"), ["a", "##", "b"]);
        assert_eq!(texts("#x"), ["#", "x"]);
        assert_eq!(
            texts("A&&!B||C==1"),
            ["A", "&&", "!", "B", "||", "C", "==", "1"]
        );
        assert_eq!(texts("x<<=2"), ["x", "<<=", "2"]);
    }

    #[test]
    fn a_pp_number_is_wider_than_a_literal() {
        assert_eq!(texts("1e+5 0x1F 1.5d .5"), ["1e+5", "0x1F", "1.5d", ".5"]);
        assert_eq!(texts("1-2"), ["1", "-", "2"]);
    }

    #[test]
    fn literals_and_comments() {
        assert_eq!(
            texts(r#""a\"b" L"w" 'c' L'd'"#),
            [r#""a\"b""#, "L\"w\"", "'c'", "L'd'"]
        );
        let pieces = tokenize("a /* c */ b // tail").unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(pieces[1].space_before);
        assert!(tokenize("\"open").is_err());
        assert!(tokenize("/* open").is_err());
    }

    #[test]
    fn identifiers() {
        assert!(is_identifier("_A1"));
        assert!(!is_identifier("1A"));
        assert!(!is_identifier(""));
    }
}
