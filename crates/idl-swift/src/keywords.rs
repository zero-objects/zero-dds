// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Swift reserved words and escaping logic.
//!
//! Swift keywords used in declarations, statements, and expressions
//! (Swift 5/6 reference, "Lexical Structure" / "Keywords and Punctuation").
//! Swift has a native raw-identifier escape — backticks — so an IDL
//! identifier that collides with a keyword is wrapped as `` `name` ``
//! rather than renamed. This is the idiomatic Swift equivalent of
//! Rust's `r#` prefix and keeps the identifier semantically identical
//! to the original IDL name.

/// Swift reserved keywords (declarations, statements, expressions,
/// pattern/type context words). Case-sensitive exact match.
pub(crate) const SWIFT_RESERVED: &[&str] = &[
    // Keywords used in declarations
    "associatedtype",
    "class",
    "deinit",
    "enum",
    "extension",
    "fileprivate",
    "func",
    "import",
    "init",
    "inout",
    "internal",
    "let",
    "open",
    "operator",
    "precedencegroup",
    "private",
    "protocol",
    "public",
    "rethrows",
    "static",
    "struct",
    "subscript",
    "typealias",
    "var",
    // Keywords used in statements
    "break",
    "case",
    "catch",
    "continue",
    "default",
    "defer",
    "do",
    "else",
    "fallthrough",
    "for",
    "guard",
    "if",
    "in",
    "repeat",
    "return",
    "throw",
    "switch",
    "where",
    "while",
    // Keywords used in expressions and types
    "Any",
    "as",
    "catch",
    "false",
    "is",
    "nil",
    "rethrows",
    "self",
    "Self",
    "super",
    "throw",
    "throws",
    "true",
    "try",
];

/// Checks whether an identifier is a Swift keyword (case-sensitive).
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    SWIFT_RESERVED.contains(&name)
}

/// Returns an identifier that is guaranteed usable in Swift.
///
/// - If `name` is collision-free: unchanged.
/// - If `name` is a Swift keyword: wrapped in backticks (`` `name` ``),
///   Swift's native raw-identifier syntax.
#[must_use]
pub fn escape_swift_ident(name: &str) -> String {
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
    fn class_is_reserved() {
        assert!(is_reserved("class"));
    }

    #[test]
    fn self_uppercase_and_lowercase_both_reserved() {
        assert!(is_reserved("self"));
        assert!(is_reserved("Self"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("my_field"));
    }

    #[test]
    fn escape_class_yields_backtick_wrapped() {
        assert_eq!(escape_swift_ident("class"), "`class`");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_swift_ident("Foo"), "Foo");
    }

    #[test]
    fn all_reserved_words_escape_to_backtick_wrapped() {
        for kw in SWIFT_RESERVED {
            assert_eq!(escape_swift_ident(kw), format!("`{kw}`"));
        }
    }
}
