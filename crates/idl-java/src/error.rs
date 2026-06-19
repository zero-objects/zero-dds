// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Error types for the IDL→Java codegen.

use core::fmt;

/// Top-level error of the Java code generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaGenError {
    /// The IDL construct is not supported in the current foundation
    /// scope (C5.4-a/-b). `construct` is a short label
    /// (e.g. `"interface"`, `"valuetype"`, `"fixed"`, `"any"`,
    /// `"map"`); since C5.4-b it can also be a bitset/bitmask
    /// constraint violation (e.g. `"bitset width > 64"`).
    UnsupportedConstruct {
        /// Name of the unsupported construct.
        construct: String,
        /// Optional: identifier name (type name or member name).
        context: Option<String>,
    },
    /// Identifier collides with a reserved Java keyword.
    /// Java has no `@`-escape syntax, so the identifier is
    /// renamed with a `_` suffix by the emitter; this error only
    /// occurs if the sanitized name itself collides or is empty.
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

impl fmt::Display for JavaGenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedConstruct { construct, context } => match context {
                Some(ctx) => write!(
                    f,
                    "unsupported IDL construct '{construct}' in '{ctx}' (idl-java foundation)",
                ),
                None => write!(
                    f,
                    "unsupported IDL construct '{construct}' (idl-java foundation)",
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

impl std::error::Error for JavaGenError {}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn unsupported_display_has_context() {
        let e = JavaGenError::UnsupportedConstruct {
            construct: "interface".into(),
            context: Some("Foo".into()),
        };
        let s = format!("{e}");
        assert!(s.contains("interface"));
        assert!(s.contains("Foo"));
    }

    #[test]
    fn unsupported_display_without_context() {
        let e = JavaGenError::UnsupportedConstruct {
            construct: "any".into(),
            context: None,
        };
        let s = format!("{e}");
        assert!(s.contains("any"));
    }

    #[test]
    fn invalid_name_display() {
        let e = JavaGenError::InvalidName {
            name: "class".into(),
            reason: "reserved Java keyword".into(),
        };
        assert!(format!("{e}").contains("reserved"));
    }

    #[test]
    fn inheritance_cycle_display() {
        let e = JavaGenError::InheritanceCycle {
            type_name: "Loop".into(),
        };
        assert!(format!("{e}").contains("Loop"));
    }

    #[test]
    fn internal_display() {
        let e = JavaGenError::Internal("oops".into());
        assert!(format!("{e}").contains("oops"));
    }
}
