// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Error types for the IDL → C# codegen.

use core::fmt;

/// Top-level error of the C# code generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CsGenError {
    /// The IDL construct is not supported in the current foundation
    /// scope (C5.3-a). `construct` is a short label (e.g.
    /// `"interface"`, `"valuetype"`, `"fixed"`, `"any"`, `"map"`,
    /// `"bitset"`, `"bitmask"`).
    UnsupportedConstruct {
        /// Name of the unsupported construct.
        construct: String,
        /// Optional: identifier name (type name or member name).
        context: Option<String>,
    },
    /// Identifier collides with a reserved C# keyword.
    /// Unlike C++, C# does not reject but escapes with an
    /// `@` prefix (spec behavior, §6 IDL4-CS mapping).
    /// This error only occurs if the name itself would still be
    /// invalid after escaping (empty string, double `@`, ...).
    InvalidName {
        /// The invalid identifier.
        name: String,
        /// Reason for the rejection.
        reason: String,
    },
    /// Inheritance cycle in the struct graph (self-reference or
    /// indirect loop). Detected before emission.
    InheritanceCycle {
        /// Type name involved in the cycle.
        type_name: String,
    },
    /// The generated output is internally inconsistent (should not
    /// happen — bug indicator).
    Internal(String),
}

impl fmt::Display for CsGenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedConstruct { construct, context } => match context {
                Some(ctx) => write!(
                    f,
                    "unsupported IDL construct '{construct}' in '{ctx}' (C5.3-a Foundation scope)",
                ),
                None => write!(
                    f,
                    "unsupported IDL construct '{construct}' (C5.3-a Foundation scope)",
                ),
            },
            Self::InvalidName { name, reason } => {
                write!(f, "invalid identifier '{name}': {reason}")
            }
            Self::InheritanceCycle { type_name } => {
                write!(f, "inheritance cycle detected at type '{type_name}'")
            }
            Self::Internal(msg) => write!(f, "internal codegen error: {msg}"),
        }
    }
}

impl std::error::Error for CsGenError {}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn unsupported_display_has_context() {
        let e = CsGenError::UnsupportedConstruct {
            construct: "interface".into(),
            context: Some("Foo".into()),
        };
        let s = format!("{e}");
        assert!(s.contains("interface"));
        assert!(s.contains("Foo"));
    }

    #[test]
    fn unsupported_display_without_context() {
        let e = CsGenError::UnsupportedConstruct {
            construct: "any".into(),
            context: None,
        };
        let s = format!("{e}");
        assert!(s.contains("any"));
    }

    #[test]
    fn invalid_name_display() {
        let e = CsGenError::InvalidName {
            name: "@@".into(),
            reason: "double-escape not allowed".into(),
        };
        assert!(format!("{e}").contains("double-escape"));
    }

    #[test]
    fn inheritance_cycle_display() {
        let e = CsGenError::InheritanceCycle {
            type_name: "Loop".into(),
        };
        assert!(format!("{e}").contains("Loop"));
    }

    #[test]
    fn internal_display() {
        let e = CsGenError::Internal("oops".into());
        assert!(format!("{e}").contains("oops"));
    }

    #[test]
    fn errors_are_clonable_and_eq() {
        let e1 = CsGenError::Internal("x".into());
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }
}
