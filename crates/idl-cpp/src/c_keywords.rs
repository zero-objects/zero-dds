// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C reserved words and escaping logic for the C-mode backend
//! (`crates/idl-cpp/src/c_mode.rs`, vendor spec `zerodds-xcdr2-c-1.0`).
//!
//! C has no raw-identifier / stropping syntax (unlike Rust `r#…`, Swift
//! `` `…` ``, Zig `@"…"`, Nim `` `…` ``) — a reserved word can never be a
//! legal C identifier in any escaped form. The idiomatic C-generator
//! workaround (used by e.g. protobuf-c, flatcc) is a **trailing
//! underscore suffix**: `int` -> `int_`. C permits identifiers ending in
//! an underscore (only a *leading* double-underscore, or leading
//! underscore + uppercase, is reserved to the implementation — a plain
//! trailing underscore is always safe).
//!
//! Covers ISO/IEC 9899 keywords through C23 (a superset is harmless: the
//! `zerodds-idl` frontend already rejects any IDL identifier that
//! collides with an *IDL* keyword, so entries here that happen to also be
//! IDL keywords are simply unreachable dead weight, not incorrect).

/// C keywords (C89/C99/C11/C17/C23), case-sensitive (C has no
/// case-insensitivity or partial-identifier-equality quirks).
pub(crate) const C_RESERVED: &[&str] = &[
    // C89/ISO C90
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "int",
    "long",
    "register",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    // C99
    "inline",
    "restrict",
    "_Bool",
    "_Complex",
    "_Imaginary",
    // C11
    "_Alignas",
    "_Alignof",
    "_Atomic",
    "_Generic",
    "_Noreturn",
    "_Static_assert",
    "_Thread_local",
    // C23 (ISO/IEC 9899:2024)
    "alignas",
    "alignof",
    "bool",
    "constexpr",
    "false",
    "nullptr",
    "static_assert",
    "thread_local",
    "true",
    "typeof",
    "typeof_unqual",
    "_BitInt",
    "_Decimal32",
    "_Decimal64",
    "_Decimal128",
];

/// Checks whether an identifier is a reserved C keyword.
#[must_use]
pub(crate) fn is_reserved(name: &str) -> bool {
    C_RESERVED.contains(&name)
}

/// Returns an identifier that is guaranteed usable as a bare C token.
///
/// - If `name` is collision-free: unchanged.
/// - If `name` is a C keyword: a trailing `_` is appended (`int` ->
///   `int_`).
///
/// This is applied only where a raw, non-prefixed IDL identifier reaches
/// C-syntax position (a plain struct/union/typedef name at empty scope, a
/// struct/union member field name, or a local companion like
/// `<field>_present`). Compound, module/type-prefixed tokens (e.g. the
/// `{c_name}_{enumerator}` form emitted by `emit_enum`/`emit_bitmask`/
/// `emit_bitset`) can never collide with a bare keyword by construction
/// and are left untouched.
#[must_use]
pub(crate) fn escape_c_ident(name: &str) -> String {
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
    fn int_is_reserved() {
        assert!(is_reserved("int"));
        assert!(is_reserved("struct"));
        assert!(is_reserved("static"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("my_field"));
    }

    #[test]
    fn escape_int_yields_trailing_underscore() {
        assert_eq!(escape_c_ident("int"), "int_");
        assert_eq!(escape_c_ident("default"), "default_");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_c_ident("Foo"), "Foo");
    }

    #[test]
    fn all_keywords_escape_to_trailing_underscore() {
        for kw in C_RESERVED {
            assert_eq!(escape_c_ident(kw), format!("{kw}_"));
        }
    }

    #[test]
    fn escaped_form_is_never_itself_reserved() {
        // Guards against a keyword-plus-underscore accidentally being
        // another reserved word (it structurally can't be, since no C
        // keyword ends in `_`, but assert it defensively).
        for kw in C_RESERVED {
            let escaped = escape_c_ident(kw);
            assert!(!is_reserved(&escaped), "{escaped} must not be reserved");
        }
    }
}
