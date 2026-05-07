// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Java-17-Keywords (inkl. Restricted Identifiers wie `record`, `sealed`,
//! `var`, `yield`).
//!
//! Quelle: JLS 17 §3.9 + §3.8 (restricted identifiers).
//!
//! Wenn ein IDL-Identifier einen Java-Keyword trifft, kann der Generator
//! ihn nicht ueber `@`-Escape ausweichen (Java hat keine solche Syntax).
//! Stattdessen wird der Name mit Unterstrich-Suffix versehen
//! (`class` → `class_`).

use crate::error::JavaGenError;

/// Reservierte Java-17-Schluesselwoerter (inkl. Restricted Identifiers).
///
/// Quelle: JLS 17 §3.9 ("Keywords"), §3.8 ("Identifiers"; restricted)
/// und §3.10.3 (Boolean / null Literals).
pub(crate) const JAVA_RESERVED: &[&str] = &[
    // 50 echte Keywords (JLS §3.9)
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    // Boolean / null-Literale (§3.10.3) — koennen nicht als Identifier
    // verwendet werden.
    "true",
    "false",
    "null",
    // Restricted Identifiers (§3.8) — innerhalb gewisser Kontexte
    // reserviert; wir behandeln sie konservativ als reserviert.
    "record",
    "sealed",
    "var",
    "yield",
    "non-sealed",
    "permits",
    "exports",
    "module",
    "open",
    "opens",
    "provides",
    "requires",
    "to",
    "transitive",
    "uses",
    "with",
];

/// Prueft, ob ein Identifier ein Java-Keyword ist.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    JAVA_RESERVED.contains(&name)
}

/// Bereinigt einen IDL-Identifier fuer Java.
///
/// Wenn der Name reserviert ist, wird ein `_`-Suffix angehaengt
/// (`class` → `class_`). Andernfalls bleibt er unveraendert.
///
/// # Errors
/// Liefert [`JavaGenError::InvalidName`], wenn der Name leer ist.
pub fn sanitize_identifier(name: &str) -> Result<String, JavaGenError> {
    if name.is_empty() {
        return Err(JavaGenError::InvalidName {
            name: name.to_string(),
            reason: "empty identifier".to_string(),
        });
    }
    if is_reserved(name) {
        Ok(format!("{name}_"))
    } else {
        Ok(name.to_string())
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
    fn record_is_reserved() {
        assert!(is_reserved("record"));
    }

    #[test]
    fn sealed_is_reserved() {
        assert!(is_reserved("sealed"));
    }

    #[test]
    fn var_yield_reserved() {
        assert!(is_reserved("var"));
        assert!(is_reserved("yield"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
    }

    #[test]
    fn sanitize_keyword_appends_underscore() {
        assert_eq!(sanitize_identifier("class").expect("ok"), "class_");
        assert_eq!(sanitize_identifier("int").expect("ok"), "int_");
    }

    #[test]
    fn sanitize_normal_passthrough() {
        assert_eq!(sanitize_identifier("Foo").expect("ok"), "Foo");
    }

    #[test]
    fn sanitize_empty_errors() {
        assert!(matches!(
            sanitize_identifier(""),
            Err(JavaGenError::InvalidName { .. })
        ));
    }

    #[test]
    fn list_contains_at_least_50_keywords() {
        // JLS-§3.9 hat 50 echte Keywords; insgesamt mit Restricted-IDs
        // erwarten wir > 60.
        assert!(JAVA_RESERVED.len() >= 50);
    }
}
