// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C# source text: literals. The indenting writer is the Java one's.

use core::fmt::Write as _;

pub use crate::emit::idl4_java::text::Writer;

/// C#'s escape of one UTF-16 code unit inside a literal (ECMA-334
/// §6.4.5.5): the printable ASCII characters as they are, the quote and
/// the backslash escaped, everything else as `\uXXXX`, which C# reads as
/// one code unit inside a literal, a lone surrogate included.
fn escape_unit(unit: u16, quote: char, out: &mut String) {
    match unit {
        0x5C => out.push_str("\\\\"),
        0x22 if quote == '"' => out.push_str("\\\""),
        0x27 if quote == '\'' => out.push_str("\\'"),
        0x20..=0x7E => out.push(char::from(u8::try_from(unit).unwrap_or(b'?'))),
        _ => {
            let _ = write!(out, "\\u{unit:04x}");
        }
    }
}

/// A C# string literal holding the code units `units`.
#[must_use]
pub fn string_literal_units(units: &[u16]) -> String {
    let mut out = String::from("\"");
    for unit in units {
        escape_unit(*unit, '"', &mut out);
    }
    out.push('"');
    out
}

/// A C# string literal holding `text`.
#[must_use]
pub fn string_literal(text: &str) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    string_literal_units(&units)
}

/// A C# character literal holding the code unit `unit`.
#[must_use]
pub fn char_literal(unit: u16) -> String {
    let mut out = String::from("'");
    escape_unit(unit, '\'', &mut out);
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_escape_what_csharp_needs_escaped() {
        assert_eq!(string_literal("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(string_literal("ä\n"), "\"\\u00e4\\u000a\"");
        assert_eq!(char_literal(u16::from(b'\'')), "'\\''");
        assert_eq!(char_literal(0xD800), "'\\ud800'");
    }
}
