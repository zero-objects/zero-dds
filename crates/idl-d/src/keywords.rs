// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! D reserved words and escaping logic.
//!
//! D has no raw-identifier / stropping escape syntax (unlike Rust `r#…`,
//! Swift `` `…` ``, Zig `@"…"`, Nim `` `…` ``). The established D
//! community idiom for a name that collides with a keyword is a
//! **trailing underscore** (`class` -> `class_`) — D identifiers may
//! legally end in `_`, so this is always syntactically valid and
//! collision-free (D does not itself generate trailing-underscore
//! names for its own reserved set).

/// D keywords (D Language Reference, "Lexical" chapter — keyword list).
pub(crate) const D_RESERVED: &[&str] = &[
    "abstract",
    "alias",
    "align",
    "asm",
    "assert",
    "auto",
    "body",
    "bool",
    "break",
    "byte",
    "case",
    "cast",
    "catch",
    "cdouble",
    "cent",
    "cfloat",
    "char",
    "class",
    "const",
    "continue",
    "creal",
    "dchar",
    "debug",
    "default",
    "delegate",
    "delete",
    "deprecated",
    "do",
    "double",
    "else",
    "enum",
    "export",
    "extern",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "foreach",
    "foreach_reverse",
    "function",
    "goto",
    "idouble",
    "if",
    "ifloat",
    "immutable",
    "import",
    "in",
    "inout",
    "int",
    "interface",
    "invariant",
    "ireal",
    "is",
    "lazy",
    "long",
    "macro",
    "mixin",
    "module",
    "new",
    "nothrow",
    "null",
    "out",
    "override",
    "package",
    "pragma",
    "private",
    "protected",
    "public",
    "pure",
    "real",
    "ref",
    "return",
    "scope",
    "shared",
    "short",
    "static",
    "struct",
    "super",
    "switch",
    "synchronized",
    "template",
    "this",
    "throw",
    "true",
    "try",
    "typedef",
    "typeid",
    "typeof",
    "ubyte",
    "ucent",
    "uint",
    "ulong",
    "union",
    "unittest",
    "ushort",
    "version",
    "void",
    "wchar",
    "while",
    "with",
];

/// Checks whether an identifier is a D keyword (case-sensitive — D, unlike
/// Ada/Nim, is case-sensitive).
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    D_RESERVED.contains(&name)
}

/// Returns an identifier that is guaranteed usable in D.
///
/// - If `name` is collision-free: unchanged.
/// - If `name` is a D keyword: a trailing `_` is appended (`class` ->
///   `class_`).
///
/// Infallible: D has no reserved-name syntax restriction beyond the
/// keyword table itself, so every input maps to exactly one valid
/// D identifier.
#[must_use]
pub fn escape_d_ident(name: &str) -> String {
    if is_reserved(name) {
        format!("{name}_")
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn class_is_reserved() {
        assert!(is_reserved("class"));
    }

    #[test]
    fn version_is_reserved() {
        assert!(is_reserved("version"));
    }

    #[test]
    fn body_is_reserved() {
        assert!(is_reserved("body"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("my_field"));
    }

    #[test]
    fn escape_keyword_appends_underscore() {
        assert_eq!(escape_d_ident("class"), "class_");
        assert_eq!(escape_d_ident("in"), "in_");
        assert_eq!(escape_d_ident("out"), "out_");
        assert_eq!(escape_d_ident("version"), "version_");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_d_ident("Foo"), "Foo");
    }

    #[test]
    fn all_keywords_escape_with_trailing_underscore() {
        for kw in D_RESERVED {
            let e = escape_d_ident(kw);
            assert_eq!(e, format!("{kw}_"));
            // The escaped form itself must never collide with the reserved set.
            assert!(!is_reserved(&e));
        }
    }

    #[test]
    fn keyword_list_has_full_size() {
        assert!(D_RESERVED.len() >= 95);
    }
}
