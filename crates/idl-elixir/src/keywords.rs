// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Elixir reserved words and escaping logic.
//!
//! Elixir's genuine reserved words (cannot be used as a bare
//! variable/function/module-local identifier at all) are a short,
//! fixed list — everything else (including things like `def`,
//! `defmodule`, `import`) is an ordinary macro/function name, not a
//! language keyword.
//!
//! Elixir has no raw-identifier/backtick escape syntax. The
//! generator's own identifiers (struct/enum/union module names,
//! struct field atoms used as `defstruct` keys, dot-access field
//! names, and per-enumerator function names) are escaped defensively
//! with a trailing underscore on collision, e.g. `do` -> `do_`. This
//! is safe for the wire format: XCDR encodes members positionally,
//! so renaming an identifier for keyword-collision has no effect on
//! wire bytes.

/// Elixir's true reserved words.
pub(crate) const ELIXIR_RESERVED: &[&str] = &[
    "true", "false", "nil", "when", "and", "or", "not", "in", "fn", "do", "end", "catch", "rescue",
    "after", "else",
];

/// Checks whether an identifier is an Elixir reserved word.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    ELIXIR_RESERVED.contains(&name)
}

/// Returns an identifier that is guaranteed usable in Elixir source:
/// unchanged if collision-free, otherwise suffixed with `_`.
#[must_use]
pub fn escape_elixir_ident(name: &str) -> String {
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
    fn do_is_reserved() {
        assert!(is_reserved("do"));
        assert!(is_reserved("end"));
        assert!(is_reserved("fn"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("my_field"));
    }

    #[test]
    fn escape_do_yields_trailing_underscore() {
        assert_eq!(escape_elixir_ident("do"), "do_");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_elixir_ident("Foo"), "Foo");
    }

    #[test]
    fn all_reserved_escape_with_trailing_underscore() {
        for kw in ELIXIR_RESERVED {
            assert_eq!(escape_elixir_ident(kw), format!("{kw}_"));
        }
    }
}
