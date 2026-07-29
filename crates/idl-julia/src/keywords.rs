// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Julia reserved words and escaping logic.
//!
//! Julia has no raw-identifier/backtick escape syntax for keywords.
//! The established community convention on collision is a trailing
//! underscore, e.g. `struct` -> `struct_`. Safe for the wire format:
//! XCDR encodes members positionally, so renaming an identifier for
//! keyword-collision has no effect on wire bytes.

/// Julia's reserved keywords (fixed, case-sensitive).
pub(crate) const JULIA_RESERVED: &[&str] = &[
    "baremodule",
    "begin",
    "break",
    "catch",
    "const",
    "continue",
    "do",
    "else",
    "elseif",
    "end",
    "export",
    "false",
    "finally",
    "for",
    "function",
    "global",
    "if",
    "import",
    "let",
    "local",
    "macro",
    "module",
    "quote",
    "return",
    "struct",
    "true",
    "try",
    "using",
    "while",
];

/// Checks whether an identifier is a Julia reserved keyword.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    JULIA_RESERVED.contains(&name)
}

/// Returns an identifier that is guaranteed usable in Julia source:
/// unchanged if collision-free, otherwise suffixed with `_`.
#[must_use]
pub fn escape_julia_ident(name: &str) -> String {
    if is_reserved(name) {
        format!("{name}_")
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_is_reserved() {
        assert!(is_reserved("struct"));
        assert!(is_reserved("function"));
        assert!(is_reserved("end"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("my_field"));
    }

    #[test]
    fn escape_struct_yields_trailing_underscore() {
        assert_eq!(escape_julia_ident("struct"), "struct_");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_julia_ident("Foo"), "Foo");
    }

    #[test]
    fn all_reserved_escape_with_trailing_underscore() {
        for kw in JULIA_RESERVED {
            assert_eq!(escape_julia_ident(kw), format!("{kw}_"));
        }
    }
}
