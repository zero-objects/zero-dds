// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeScript/ECMAScript reserved words and escaping logic.
//!
//! Spec: ECMA-262 `ReservedWord` (§12.7.2) + strict-mode-only
//! reserved words (always in effect here — generated output is an
//! ES module, which is implicitly strict) + `await` (reserved at
//! module top level). TypeScript-only contextual keywords (`type`,
//! `namespace`, `declare`, `keyof`, `readonly`, `as`, …) are
//! deliberately **excluded** — empirically verified via `tsc
//! --strict` that they remain legal as declaration names, parameter
//! names, and property keys (see Welle C.2 #14 investigation notes).
//!
//! Generated identifiers never affect the wire format (XCDR2 is
//! positional, not name-keyed), so it is safe to escape
//! defensively at every identifier emit site — including object/
//! interface property keys, where a bare keyword would in fact
//! already be legal TS. Uniform escaping keeps the rule simple and
//! avoids relying on per-site TS syntax nuance.

use alloc::format;
use alloc::string::String;

/// ECMAScript/TypeScript reserved words that cannot be used as a
/// declaration name, binding identifier, or parameter name.
pub(crate) const TS_RESERVED: &[&str] = &[
    // ECMA-262 §12.7.2 ReservedWord.
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    // Strict-mode-only reserved words — generated TS is always an ES
    // module, which is implicitly strict.
    "implements",
    "interface",
    "let",
    "package",
    "private",
    "protected",
    "public",
    "static",
    "yield",
    // Reserved at the top level of a module (ES modules only).
    "await",
];

/// Checks whether an identifier is a reserved TypeScript/ECMAScript
/// word (case-sensitive — TS has no stropping).
#[must_use]
pub(crate) fn is_reserved(name: &str) -> bool {
    TS_RESERVED.contains(&name)
}

/// Returns an identifier guaranteed usable anywhere in emitted
/// TypeScript (declaration name, parameter, property key, module
/// path segment): unchanged if collision-free, otherwise suffixed
/// with `_` (`class` -> `class_`). Infallible — an empty name is an
/// upstream IDL-parser invariant, not something this backend needs
/// to re-validate.
#[must_use]
pub(crate) fn escape_ts_ident(name: &str) -> String {
    if is_reserved(name) {
        format!("{name}_")
    } else {
        String::from(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_is_reserved() {
        assert!(is_reserved("class"));
        assert!(is_reserved("await"));
        assert!(is_reserved("yield"));
    }

    #[test]
    fn ts_contextual_keywords_are_not_reserved() {
        // Empirically verified safe via `tsc --strict` (see module docs).
        for kw in [
            "type",
            "namespace",
            "module",
            "declare",
            "abstract",
            "keyof",
            "readonly",
            "infer",
            "as",
            "is",
            "asserts",
            "from",
            "of",
            "satisfies",
            "unique",
            "override",
            "out",
            "accessor",
            "global",
            "async",
            "get",
            "set",
        ] {
            assert!(!is_reserved(kw), "{kw} should not be treated as reserved");
        }
    }

    #[test]
    fn non_keyword_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("my_field"));
    }

    #[test]
    fn escape_class_yields_trailing_underscore() {
        assert_eq!(escape_ts_ident("class"), "class_");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_ts_ident("Foo"), "Foo");
    }

    #[test]
    fn all_reserved_words_escape_to_trailing_underscore() {
        for kw in TS_RESERVED {
            assert_eq!(escape_ts_ident(kw), format!("{kw}_"));
        }
    }
}
