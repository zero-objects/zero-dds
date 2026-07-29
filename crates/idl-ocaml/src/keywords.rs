// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OCaml reserved words and escaping logic.
//!
//! OCaml has no raw-identifier / stropping syntax (unlike Rust `r#…`,
//! Swift `` `…` ``, Nim `` `…` ``). All OCaml keywords are lowercase, so
//! they can only collide with this backend's *lowercase-initial*
//! identifier positions: record field labels (`c.field.rs`'s union case
//! fields included) and the lower-cased enum type name built by
//! `type_ident`. Uppercase-initial positions (module names via
//! `module_name`, enum constructor names) cannot collide with a
//! lowercase-only keyword by construction and are left untouched.
//!
//! Convention: **trailing underscore suffix** (`type` -> `type_`). OCaml
//! permits identifiers ending in an underscore; this is the standard
//! OCaml/ppx-generated-code idiom for keyword collisions.

/// OCaml reserved keywords (the full keyword set — no version skew to
/// track; OCaml's keyword list has been stable across the codegen's
/// supported range).
pub(crate) const OCAML_RESERVED: &[&str] = &[
    "and",
    "as",
    "assert",
    "asr",
    "begin",
    "class",
    "constraint",
    "do",
    "done",
    "downto",
    "else",
    "end",
    "exception",
    "external",
    "false",
    "for",
    "fun",
    "function",
    "functor",
    "if",
    "in",
    "include",
    "inherit",
    "initializer",
    "land",
    "lazy",
    "let",
    "lor",
    "lsl",
    "lsr",
    "lxor",
    "match",
    "method",
    "mod",
    "module",
    "mutable",
    "new",
    "nonrec",
    "object",
    "of",
    "open",
    "or",
    "private",
    "rec",
    "sig",
    "struct",
    "then",
    "to",
    "true",
    "try",
    "type",
    "val",
    "virtual",
    "when",
    "while",
    "with",
];

/// Checks whether an identifier is an OCaml reserved word.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    OCAML_RESERVED.contains(&name)
}

/// Returns an identifier that is guaranteed usable as an OCaml lowercase
/// identifier (`LIDENT`) — a record field label, a value name, or the
/// lower-cased enum type name.
///
/// If `name` is collision-free: unchanged. If `name` is an OCaml reserved
/// word: a trailing `_` is appended (`type` -> `type_`).
#[must_use]
pub(crate) fn escape_ocaml_ident(name: &str) -> String {
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
        assert!(is_reserved("and"));
        assert!(is_reserved("to"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("my_field"));
    }

    #[test]
    fn escape_type_yields_trailing_underscore() {
        assert_eq!(escape_ocaml_ident("type"), "type_");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_ocaml_ident("foo"), "foo");
    }

    #[test]
    fn all_keywords_escape_to_trailing_underscore() {
        for kw in OCAML_RESERVED {
            assert_eq!(escape_ocaml_ident(kw), format!("{kw}_"));
        }
    }

    #[test]
    fn double_escape_is_idempotent_safe() {
        let once = escape_ocaml_ident("type");
        let twice = escape_ocaml_ident(&once);
        assert_eq!(once, twice);
    }
}
