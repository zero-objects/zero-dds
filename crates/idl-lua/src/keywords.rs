// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Lua 5.4 reserved words and escaping logic.
//!
//! Lua has no raw-identifier / stropping syntax (unlike Rust `r#…`, Swift
//! `` `…` ``, Nim `` `…` ``). A reserved word cannot appear as a `Name`
//! token anywhere a Lua identifier is expected — not as a `local`/global
//! declaration, not as a bare `t.name` field access, not as a table
//! constructor `{ name = v }` key. The emitter builds field access and
//! declaration text via `format!`, so collisions must be resolved before
//! the name is spliced into source.
//!
//! Convention: **trailing underscore suffix** (`end` -> `end_`). Lua
//! places no restriction on leading or trailing underscores (unlike C,
//! which reserves leading-underscore names to the implementation), but a
//! trailing suffix keeps this backend consistent with the sibling
//! backends in this batch that use the same convention.

/// Lua 5.4 reserved keywords (all 22 — the full and final list; Lua does
/// not add new keywords across faithfully-compatible versions).
pub(crate) const LUA_RESERVED: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// Checks whether an identifier is a Lua reserved word.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    LUA_RESERVED.contains(&name)
}

/// Returns an identifier that is guaranteed usable as a Lua `Name` token
/// (declaration, `t.name` field access, or table-constructor key).
///
/// If `name` is collision-free: unchanged. If `name` is a Lua reserved
/// word: a trailing `_` is appended (`end` -> `end_`).
#[must_use]
pub(crate) fn escape_lua_ident(name: &str) -> String {
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
    fn end_is_reserved() {
        assert!(is_reserved("end"));
        assert!(is_reserved("function"));
        assert!(is_reserved("local"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("my_field"));
    }

    #[test]
    fn escape_end_yields_trailing_underscore() {
        assert_eq!(escape_lua_ident("end"), "end_");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_lua_ident("Foo"), "Foo");
    }

    #[test]
    fn all_keywords_escape_to_trailing_underscore() {
        for kw in LUA_RESERVED {
            assert_eq!(escape_lua_ident(kw), format!("{kw}_"));
        }
    }

    #[test]
    fn double_escape_is_idempotent_safe() {
        // "end_" is not itself reserved, so re-escaping is a no-op (no
        // double-suffixing even if called twice by mistake).
        let once = escape_lua_ident("end");
        let twice = escape_lua_ident(&once);
        assert_eq!(once, twice);
    }
}
