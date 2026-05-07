// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C#-Reserved-Words und Escaping-Logik.
//!
//! Spec: ECMA-334 §6.4.4 (C# 12.0 — `formal/2024-12-01` Anhang).
//! IDL4-CS-Mapping §6.6 verlangt, dass Identifier, die mit C#-Keywords
//! kollidieren, mit dem `@`-Prefix escaped werden — sie bleiben dadurch
//! semantisch der ursprueng­liche IDL-Name.
//!
//! Hier listen wir:
//! 1. **Reserved Keywords** — duerfen nicht ohne `@` als Identifier
//!    verwendet werden.
//! 2. **Contextual Keywords** — sind nur in bestimmten Positionen
//!    reserviert; wir escapen sie defensiv ebenfalls (sicher und
//!    spec-konform).
//!
//! Erweiterbar in C5.3-b/c. Liste deckt die haeufigen Kollisionen
//! mit IDL-Identifiern ab.

use crate::error::CsGenError;

/// C#-Reserved-Keywords (ECMA-334 §6.4.4 Tabelle 8).
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

/// Contextual Keywords — wir escapen sie defensiv (ECMA-334 §6.4.4
/// Tabelle 9). Sie sind nur in bestimmten Kontexten reserviert,
/// `@`-Prefix ist aber immer zulaessig und konflictfrei.
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

/// Prueft, ob ein Identifier ein C#-Keyword (reserved oder contextual) ist.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    CS_RESERVED.contains(&name) || CS_CONTEXTUAL.contains(&name)
}

/// Prueft, ob ein Identifier ein **strict-reserved** C#-Keyword ist
/// (Tabelle 8). Contextual-Keywords werden als nicht-reserved gemeldet.
#[must_use]
pub fn is_strict_reserved(name: &str) -> bool {
    CS_RESERVED.contains(&name)
}

/// Liefert einen Identifier, der in C# garantiert verwendbar ist.
///
/// - Wenn `name` kollisionsfrei ist: unveraendert.
/// - Wenn `name` ein C#-Keyword ist: wird mit `@`-Prefix versehen.
///
/// # Errors
/// - [`CsGenError::InvalidName`] wenn `name` leer ist oder bereits mit
///   `@` beginnt (Doppel-Escape ist verboten).
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
