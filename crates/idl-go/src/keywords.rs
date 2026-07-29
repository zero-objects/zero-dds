// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Go reserved words and escaping logic.
//!
//! The Go Programming Language Specification, "Keywords" — exactly 25
//! reserved words, all lowercase, with no contextual/soft-keyword
//! exceptions. Go has no raw-identifier escape syntax (unlike Rust's
//! `r#` or Swift's backticks); the community convention on a
//! collision is a trailing underscore (`type` -> `type_`).
//!
//! Most identifiers this backend emits (struct fields, enum type +
//! values, union case fields) are already capitalized via
//! [`crate::emitter::exported`] before reaching Go source, which on
//! its own avoids every collision with this list (all 25 keywords are
//! strictly lowercase). The exceptions are struct/union **type**
//! names, which this backend emits with their original IDL casing —
//! and any generated identifier that concatenates a type name
//! (`unmarshalFrom<Type>`) must escape consistently with the type's
//! own declaration. `escape_go_ident` is applied at every such site.

/// The 25 Go keywords (Go Language Specification, "Keywords"). Exact,
/// case-sensitive match — Go has no contextual keywords.
pub(crate) const GO_RESERVED: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
];

/// Checks whether an identifier is a Go keyword (case-sensitive).
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    GO_RESERVED.contains(&name)
}

/// Returns an identifier that is guaranteed usable in Go.
///
/// - If `name` is collision-free: unchanged.
/// - If `name` is a Go keyword: given a trailing underscore
///   (`type` -> `type_`) — Go has no raw-identifier syntax, so this
///   is the idiomatic community workaround.
#[must_use]
pub fn escape_go_ident(name: &str) -> String {
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
    fn type_is_reserved() {
        assert!(is_reserved("type"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("my_field"));
    }

    #[test]
    fn capitalized_keyword_form_is_not_reserved() {
        // Go keywords are strictly lowercase; capitalizing the first
        // letter (as `exported` does) already dodges every collision.
        assert!(!is_reserved("Type"));
        assert!(!is_reserved("Func"));
    }

    #[test]
    fn escape_type_yields_trailing_underscore() {
        assert_eq!(escape_go_ident("type"), "type_");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_go_ident("Foo"), "Foo");
    }

    #[test]
    fn all_reserved_words_escape_to_trailing_underscore() {
        for kw in GO_RESERVED {
            assert_eq!(escape_go_ident(kw), format!("{kw}_"));
        }
    }

    #[test]
    fn list_has_exactly_25_keywords() {
        assert_eq!(GO_RESERVED.len(), 25);
    }
}
