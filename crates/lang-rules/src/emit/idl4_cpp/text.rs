// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C++ source text: literals. The indenting writer is the Java one's.

use core::fmt::Write as _;

pub use crate::emit::idl4_java::text::Writer;

/// One byte of a narrow literal: the printable ASCII characters as they
/// are, the quote and the backslash escaped, every other byte as a
/// three-digit octal escape, which ends after three digits where a hex
/// escape would take the next character in.
fn escape_byte(byte: u8, quote: char, out: &mut String) {
    match byte {
        b'\\' => out.push_str("\\\\"),
        b'"' if quote == '"' => out.push_str("\\\""),
        b'\'' if quote == '\'' => out.push_str("\\'"),
        b'?' => out.push_str("\\?"),
        0x20..=0x7E => out.push(char::from(byte)),
        _ => {
            let _ = write!(out, "\\{byte:03o}");
        }
    }
}

/// One code point of a wide literal: printable ASCII as it is, a control
/// character as an octal escape, everything else as a universal character
/// name.
fn escape_code_point(point: u32, quote: char, out: &mut String) {
    match point {
        0x5C => out.push_str("\\\\"),
        0x22 if quote == '"' => out.push_str("\\\""),
        0x27 if quote == '\'' => out.push_str("\\'"),
        0x3F => out.push_str("\\?"),
        0x20..=0x7E => out.push(char::from_u32(point).unwrap_or('?')),
        0x00..=0x1F | 0x7F..=0x9F => {
            let _ = write!(out, "\\{point:03o}");
        }
        0xA0..=0xFFFF => {
            let _ = write!(out, "\\u{point:04x}");
        }
        _ => {
            let _ = write!(out, "\\U{point:08x}");
        }
    }
}

/// A narrow C++ string literal holding the ISO Latin-1 characters of an
/// IDL `string`; a character above U+00FF is written as its UTF-8 bytes.
#[must_use]
pub fn string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        let point = u32::from(c);
        if point <= 0xFF {
            escape_byte(u8::try_from(point).unwrap_or(b'?'), '"', &mut out);
        } else {
            let mut buffer = [0u8; 4];
            for byte in c.encode_utf8(&mut buffer).bytes() {
                escape_byte(byte, '"', &mut out);
            }
        }
    }
    out.push('"');
    out
}

/// A wide C++ string literal (`L"..."`) holding the code points of an IDL
/// `wstring` (§7.2.3: "The constant value of wide character and wide
/// string constants shall be preceded by L in C++").
#[must_use]
pub fn wstring_literal(points: &[u32]) -> String {
    let mut out = String::from("L\"");
    for point in points {
        escape_code_point(*point, '"', &mut out);
    }
    out.push('"');
    out
}

/// A narrow C++ character literal.
#[must_use]
pub fn char_literal(c: char) -> String {
    let point = u32::from(c);
    let mut out = String::from("'");
    escape_byte(u8::try_from(point).unwrap_or(b'?'), '\'', &mut out);
    out.push('\'');
    out
}

/// A wide C++ character literal (`L'...'`).
#[must_use]
pub fn wchar_literal(point: u32) -> String {
    let mut out = String::from("L'");
    escape_code_point(point, '\'', &mut out);
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_escape_what_cpp_needs_escaped() {
        assert_eq!(string_literal("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(string_literal("ä\n"), "\"\\344\\012\"");
        assert_eq!(string_literal("??="), "\"\\?\\?=\"");
        assert_eq!(
            wstring_literal(&[0x48, 0xE4, 0x1F600]),
            "L\"H\\u00e4\\U0001f600\""
        );
        assert_eq!(char_literal('\''), "'\\''");
        assert_eq!(wchar_literal(0x20AC), "L'\\u20ac'");
    }
}
