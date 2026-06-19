// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Java-17 keywords (incl. restricted identifiers like `record`, `sealed`,
//! `var`, `yield`).
//!
//! Source: JLS 17 §3.9 + §3.8 (restricted identifiers).
//!
//! When an IDL identifier hits a Java keyword, the generator cannot
//! avoid it via `@`-escape (Java has no such syntax).
//! Instead the name is given an underscore suffix
//! (`class` → `class_`).

use crate::error::JavaGenError;

/// Reserved Java 17 keywords (incl. restricted identifiers).
///
/// Source: JLS 17 §3.9 ("Keywords"), §3.8 ("Identifiers"; restricted)
/// and §3.10.3 (Boolean / null literals).
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
    // Boolean / null literals (§3.10.3) — cannot be used as
    // identifiers.
    "true",
    "false",
    "null",
    // Restricted identifiers (§3.8) — reserved within certain
    // contexts; we conservatively treat them as reserved.
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

/// Checks whether an identifier is a Java keyword.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    JAVA_RESERVED.contains(&name)
}

/// Sanitizes an IDL identifier for Java.
///
/// If the name is reserved, a `_` suffix is appended
/// (`class` → `class_`). Otherwise it stays unchanged.
///
/// # Errors
/// Returns [`JavaGenError::InvalidName`] if the name is empty.
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
        // JLS §3.9 has 50 real keywords; in total with restricted IDs
        // we expect > 60.
        assert!(JAVA_RESERVED.len() >= 50);
    }
}
