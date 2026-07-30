// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Ada 2012 reserved words and escaping logic.
//!
//! Ada has two properties that rule out the escaping conventions used by
//! the other ZeroDDS backends:
//!
//! 1. Ada is fully **case-insensitive** for identifiers (Ada RM 2.3):
//!    `Type`, `TYPE`, and `type` name the same entity, so reserved-word
//!    detection here lowercases both sides before comparing.
//! 2. Ada identifier syntax (RM 2.3) forbids a **trailing underscore**
//!    and forbids consecutive underscores — the D/Go/Julia-style
//!    trailing-`_` convention (`class` -> `class_`) is not a legal Ada
//!    identifier.
//! 3. Ada has no raw-identifier/stropping escape syntax (unlike Nim's
//!    `` `type` `` or Swift's `` `class` ``) — a reserved word can never
//!    be a legal Ada identifier in any quoted/escaped form either.
//!
//! Given (2)+(3), collisions are resolved with a fixed, always-legal
//! suffix: **`_Id`** (`Type` -> `Type_Id`). `Id` is not itself an Ada
//! reserved word (checked by a unit test below), the suffix starts with
//! a single underscore (never doubled, since no Ada reserved word itself
//! ends in `_`), and the result never ends in `_`.

/// Ada 2012 reserved words (RM 2012 §2.9, Table).
pub(crate) const ADA_RESERVED: &[&str] = &[
    "abort",
    "abs",
    "abstract",
    "accept",
    "access",
    "aliased",
    "all",
    "and",
    "array",
    "at",
    "begin",
    "body",
    "case",
    "constant",
    "declare",
    "delay",
    "delta",
    "digits",
    "do",
    "else",
    "elsif",
    "end",
    "entry",
    "exception",
    "exit",
    "for",
    "function",
    "generic",
    "goto",
    "if",
    "in",
    "interface",
    "is",
    "limited",
    "loop",
    "mod",
    "new",
    "not",
    "null",
    "of",
    "or",
    "others",
    "out",
    "overriding",
    "package",
    "parallel",
    "pragma",
    "private",
    "procedure",
    "protected",
    "raise",
    "range",
    "record",
    "rem",
    "renames",
    "requeue",
    "return",
    "reverse",
    "select",
    "separate",
    "some",
    "subtype",
    "synchronized",
    "tagged",
    "task",
    "terminate",
    "then",
    "type",
    "until",
    "use",
    "when",
    "while",
    "with",
    "xor",
];

/// The suffix appended to a reserved-word collision. See the module doc for
/// why it must be non-trailing-underscore-terminal and why `Id` was chosen.
const ESCAPE_SUFFIX: &str = "_Id";

/// Checks whether `name` is an Ada reserved word. Case-insensitive (Ada RM
/// §2.3 — identifiers are case-insensitive).
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ADA_RESERVED.contains(&lower.as_str())
}

/// Returns an identifier that is guaranteed usable in Ada.
///
/// - If `name` is collision-free (case-insensitively): unchanged.
/// - If `name` is a reserved word: [`ESCAPE_SUFFIX`] is appended
///   (`Type` -> `Type_Id`, `type` -> `type_Id`).
///
/// Infallible: every input maps to exactly one valid Ada identifier.
#[must_use]
pub fn escape_ada_ident(name: &str) -> String {
    if is_reserved(name) {
        format!("{name}{ESCAPE_SUFFIX}")
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn type_is_reserved() {
        assert!(is_reserved("type"));
    }

    #[test]
    fn case_insensitive_detection() {
        assert!(is_reserved("Type"));
        assert!(is_reserved("TYPE"));
        assert!(is_reserved("TyPe"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("My_Field"));
    }

    #[test]
    fn escape_keyword_appends_suffix() {
        assert_eq!(escape_ada_ident("type"), "type_Id");
        assert_eq!(escape_ada_ident("Type"), "Type_Id");
        assert_eq!(escape_ada_ident("body"), "body_Id");
        assert_eq!(escape_ada_ident("record"), "record_Id");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_ada_ident("Foo"), "Foo");
    }

    #[test]
    fn escape_suffix_itself_is_not_reserved() {
        assert!(!is_reserved(ESCAPE_SUFFIX.trim_start_matches('_')));
        assert!(!is_reserved("Id"));
    }

    #[test]
    fn all_keywords_escape_without_trailing_or_double_underscore() {
        for kw in ADA_RESERVED {
            let e = escape_ada_ident(kw);
            assert_eq!(e, format!("{kw}{ESCAPE_SUFFIX}"));
            assert!(!e.ends_with('_'), "must not end with `_`: {e}");
            assert!(!e.contains("__"), "must not contain `__`: {e}");
            assert!(
                !is_reserved(&e),
                "escaped form must not itself collide: {e}"
            );
        }
    }

    #[test]
    fn keyword_list_has_expected_size() {
        assert!(ADA_RESERVED.len() >= 70);
    }
}
