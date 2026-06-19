// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C# reserved words and escaping logic.
//!
//! Spec: ECMA-334 §6.4.4 (C# 12.0 — `formal/2024-12-01` appendix).
//! IDL4-CS mapping §6.6 requires that identifiers colliding with C#
//! keywords be escaped with the `@` prefix — this keeps them
//! semantically the original IDL name.
//!
//! Here we list:
//! 1. **Reserved keywords** — must not be used as identifiers without
//!    `@`.
//! 2. **Contextual keywords** — are reserved only in certain positions;
//!    we escape them defensively as well (safe and spec-compliant).
//!
//! Extensible in C5.3-b/c. The list covers the common collisions with
//! IDL identifiers.

use crate::error::CsGenError;

/// C# reserved keywords (ECMA-334 §6.4.4 table 8).
pub(crate) const CS_RESERVED: &[&str] = &[
    "abstract",
    "as",
    "base",
    "bool",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "checked",
    "class",
    "const",
    "continue",
    "decimal",
    "default",
    "delegate",
    "do",
    "double",
    "else",
    "enum",
    "event",
    "explicit",
    "extern",
    "false",
    "finally",
    "fixed",
    "float",
    "for",
    "foreach",
    "goto",
    "if",
    "implicit",
    "in",
    "int",
    "interface",
    "internal",
    "is",
    "lock",
    "long",
    "namespace",
    "new",
    "null",
    "object",
    "operator",
    "out",
    "override",
    "params",
    "private",
    "protected",
    "public",
    "readonly",
    "ref",
    "return",
    "sbyte",
    "sealed",
    "short",
    "sizeof",
    "stackalloc",
    "static",
    "string",
    "struct",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "uint",
    "ulong",
    "unchecked",
    "unsafe",
    "ushort",
    "using",
    "virtual",
    "void",
    "volatile",
    "while",
];

/// Contextual keywords — we escape them defensively (ECMA-334 §6.4.4
/// table 9). They are reserved only in certain contexts, but the `@`
/// prefix is always permitted and conflict-free.
pub(crate) const CS_CONTEXTUAL: &[&str] = &[
    "add",
    "alias",
    "ascending",
    "async",
    "await",
    "by",
    "descending",
    "dynamic",
    "equals",
    "from",
    "get",
    "global",
    "group",
    "init",
    "into",
    "join",
    "let",
    "managed",
    "nameof",
    "nint",
    "notnull",
    "nuint",
    "on",
    "orderby",
    "partial",
    "record",
    "remove",
    "select",
    "set",
    "unmanaged",
    "value",
    "var",
    "when",
    "where",
    "with",
    "yield",
];

/// Checks whether an identifier is a C# keyword (reserved or contextual).
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    CS_RESERVED.contains(&name) || CS_CONTEXTUAL.contains(&name)
}

/// Checks whether an identifier is a **strict-reserved** C# keyword
/// (table 8). Contextual keywords are reported as not reserved.
#[must_use]
pub fn is_strict_reserved(name: &str) -> bool {
    CS_RESERVED.contains(&name)
}

/// Returns an identifier that is guaranteed usable in C#.
///
/// - If `name` is collision-free: unchanged.
/// - If `name` is a C# keyword: prefixed with `@`.
///
/// # Errors
/// - [`CsGenError::InvalidName`] if `name` is empty or already starts
///   with `@` (double-escaping is forbidden).
pub fn escape_identifier(name: &str) -> Result<String, CsGenError> {
    if name.is_empty() {
        return Err(CsGenError::InvalidName {
            name: name.to_string(),
            reason: "empty identifier".into(),
        });
    }
    if name.starts_with('@') {
        return Err(CsGenError::InvalidName {
            name: name.to_string(),
            reason: "identifier already starts with '@' (double-escape forbidden)".into(),
        });
    }
    if is_reserved(name) {
        Ok(format!("@{name}"))
    } else {
        Ok(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn class_is_strict_reserved() {
        assert!(is_strict_reserved("class"));
        assert!(is_reserved("class"));
    }

    #[test]
    fn async_is_contextual_only() {
        assert!(!is_strict_reserved("async"));
        assert!(is_reserved("async"));
    }

    #[test]
    fn record_is_contextual() {
        assert!(!is_strict_reserved("record"));
        assert!(is_reserved("record"));
    }

    #[test]
    fn non_keyword_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("my_field"));
    }

    #[test]
    fn escape_class_yields_at_class() {
        let escaped = escape_identifier("class").expect("must escape");
        assert_eq!(escaped, "@class");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        let escaped = escape_identifier("Foo").expect("must pass");
        assert_eq!(escaped, "Foo");
    }

    #[test]
    fn escape_empty_rejected() {
        let err = escape_identifier("").expect_err("empty must reject");
        match err {
            CsGenError::InvalidName { reason, .. } => {
                assert!(reason.contains("empty"));
            }
            other => panic!("unexpected variant {other:?}"),
        }
    }

    #[test]
    fn escape_already_at_prefixed_rejected() {
        let err = escape_identifier("@class").expect_err("must reject double-escape");
        match err {
            CsGenError::InvalidName { reason, .. } => {
                assert!(reason.contains("double-escape") || reason.contains("@"));
            }
            other => panic!("unexpected variant {other:?}"),
        }
    }

    #[test]
    fn all_strict_keywords_escape_to_at_prefix() {
        for kw in CS_RESERVED {
            let e = escape_identifier(kw).expect("strict kw must escape");
            assert_eq!(e, format!("@{kw}"));
        }
    }

    #[test]
    fn contextual_keywords_also_escape() {
        for kw in CS_CONTEXTUAL {
            let e = escape_identifier(kw).expect("contextual kw must escape");
            assert_eq!(e, format!("@{kw}"));
        }
    }
}
