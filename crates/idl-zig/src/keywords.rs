// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Zig reserved words and identifier escaping.
//!
//! Zig has a native raw-identifier syntax — `@"..."` names any byte
//! sequence as an identifier, including a keyword (`@"error"`). We use
//! that as the escape mechanism: exact case-sensitive collision with a
//! reserved word gets wrapped in `@"..."`; everything else passes
//! through unchanged.

/// Zig reserved keywords (primitive type names like `u8`/`i32`/`f64`
/// are not reserved words — they are ordinary identifiers shadowable
/// by user code — so they are intentionally excluded).
pub(crate) const ZIG_RESERVED: &[&str] = &[
    "align",
    "allowzero",
    "and",
    "anyframe",
    "anytype",
    "asm",
    "async",
    "await",
    "break",
    "callconv",
    "catch",
    "comptime",
    "const",
    "continue",
    "defer",
    "else",
    "enum",
    "errdefer",
    "error",
    "export",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "inline",
    "linksection",
    "noalias",
    "noinline",
    "nosuspend",
    "null",
    "opaque",
    "or",
    "orelse",
    "packed",
    "pub",
    "resume",
    "return",
    "struct",
    "suspend",
    "switch",
    "test",
    "threadlocal",
    "true",
    "try",
    "undefined",
    "union",
    "unreachable",
    "usingnamespace",
    "var",
    "volatile",
    "while",
];

/// Checks whether an identifier is a Zig reserved keyword (exact,
/// case-sensitive — Zig has no style-insensitivity).
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    ZIG_RESERVED.contains(&name)
}

/// Escapes `name` for use as a Zig identifier.
///
/// If `name` collides with a Zig keyword, wraps it in the native
/// `@"..."` raw-identifier syntax. Otherwise returns it unchanged.
#[must_use]
pub fn escape_zig_ident(name: &str) -> String {
    if is_reserved(name) {
        format!("@\"{name}\"")
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_is_reserved() {
        assert!(is_reserved("error"));
        assert!(is_reserved("struct"));
        assert!(is_reserved("try"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("u8"));
        assert!(!is_reserved("i32"));
    }

    #[test]
    fn escape_reserved_wraps_in_at_quote() {
        assert_eq!(escape_zig_ident("error"), "@\"error\"");
        assert_eq!(escape_zig_ident("switch"), "@\"switch\"");
    }

    #[test]
    fn escape_non_reserved_unchanged() {
        assert_eq!(escape_zig_ident("Foo"), "Foo");
        assert_eq!(escape_zig_ident("my_field"), "my_field");
    }

    #[test]
    fn all_reserved_escape_to_at_quote_form() {
        for kw in ZIG_RESERVED {
            assert_eq!(escape_zig_ident(kw), format!("@\"{kw}\""));
        }
    }
}
