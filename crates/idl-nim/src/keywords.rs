// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Nim reserved words and identifier escaping.
//!
//! Nim identifier *equality* — including how the parser recognizes a
//! keyword token — is "partial case- and underscore-insensitive": two
//! identifiers are the same if their first character matches exactly
//! (case-sensitive) and the remainder, lowercased with underscores
//! stripped, matches too (Nim manual, "Identifiers & Keywords").
//! `tYpe` and `t_y_p_e` therefore denote the *same* identifier as
//! `type` (the keyword) to Nim's lexer — because they share the exact
//! first character — while `Type`/`TYPE` do not. A naive exact-string
//! reserved-word check would miss the former and still emit invalid
//! code. [`is_reserved`] uses the style-insensitive
//! comparison; anything colliding is escaped with Nim's native
//! backtick stropping (`` `type` ``), which names the identifier
//! without invoking the keyword.

/// Nim reserved keywords (Nim manual, "Identifiers & Keywords" — the
/// full keyword list).
pub(crate) const NIM_RESERVED: &[&str] = &[
    "addr",
    "and",
    "as",
    "asm",
    "bind",
    "block",
    "break",
    "case",
    "cast",
    "concept",
    "const",
    "continue",
    "converter",
    "defer",
    "discard",
    "distinct",
    "div",
    "do",
    "elif",
    "else",
    "end",
    "enum",
    "except",
    "export",
    "finally",
    "for",
    "from",
    "func",
    "if",
    "import",
    "in",
    "include",
    "interface",
    "is",
    "isnot",
    "iterator",
    "let",
    "macro",
    "method",
    "mixin",
    "mod",
    "nil",
    "not",
    "notin",
    "object",
    "of",
    "or",
    "out",
    "proc",
    "ptr",
    "raise",
    "ref",
    "return",
    "shl",
    "shr",
    "static",
    "template",
    "try",
    "tuple",
    "type",
    "using",
    "var",
    "when",
    "while",
    "xor",
    "yield",
];

/// Normalizes an identifier per Nim's style-insensitivity rule: keeps
/// the first byte as-is, lowercases and strips underscores from the
/// rest. Two identifiers are the same Nim token iff their normalized
/// forms are equal.
fn normalize(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => {
            let mut out = String::with_capacity(name.len());
            out.push(first);
            for c in chars {
                if c != '_' {
                    out.extend(c.to_lowercase());
                }
            }
            out
        }
        None => String::new(),
    }
}

/// Checks whether two identifiers are the same token under Nim's
/// style-insensitive identifier equality.
#[must_use]
pub fn nim_identifiers_equal(a: &str, b: &str) -> bool {
    !a.is_empty() && !b.is_empty() && a.as_bytes()[0] == b.as_bytes()[0] && {
        normalize(a) == normalize(b)
    }
}

/// Checks whether `name` collides with a Nim reserved keyword under
/// Nim's style-insensitive identifier equality (not a naive exact
/// match — see module docs).
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    NIM_RESERVED
        .iter()
        .any(|kw| nim_identifiers_equal(name, kw))
}

/// Escapes `name` for use as a Nim identifier.
///
/// If `name` collides with a Nim keyword (style-insensitively), wraps
/// it in Nim's native backtick stropping. Otherwise returns it
/// unchanged.
#[must_use]
pub fn escape_nim_ident(name: &str) -> String {
    if is_reserved(name) {
        format!("`{name}`")
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_is_reserved_exact() {
        assert!(is_reserved("type"));
        assert!(is_reserved("object"));
        assert!(is_reserved("proc"));
    }

    #[test]
    fn style_insensitive_variants_are_also_reserved() {
        // Same first char (case-sensitive), rest case/underscore-insensitive.
        assert!(is_reserved("tYpe"));
        assert!(is_reserved("t_y_p_e"));
        assert!(is_reserved("t_Y_p_E"));
    }

    #[test]
    fn different_first_char_case_is_not_reserved() {
        // First char is compared case-sensitively — "Type"/"TYPE" != "type".
        assert!(!is_reserved("Type"));
        assert!(!is_reserved("TYPE"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("myField"));
    }

    #[test]
    fn nim_identifiers_equal_matches_manual_examples() {
        assert!(nim_identifiers_equal("myVar", "my_var"));
        assert!(nim_identifiers_equal("MyVar", "My_Var"));
        assert!(!nim_identifiers_equal("myVar", "MyVar"));
    }

    #[test]
    fn escape_reserved_wraps_in_backticks() {
        assert_eq!(escape_nim_ident("type"), "`type`");
        assert_eq!(escape_nim_ident("t_y_p_e"), "`t_y_p_e`");
    }

    #[test]
    fn escape_non_reserved_unchanged() {
        assert_eq!(escape_nim_ident("Foo"), "Foo");
        assert_eq!(escape_nim_ident("my_field"), "my_field");
    }

    #[test]
    fn all_reserved_escape_to_backtick_form() {
        for kw in NIM_RESERVED {
            assert_eq!(escape_nim_ident(kw), format!("`{kw}`"));
        }
    }
}
