// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fehler-Typen fuer den IDL→Java-Codegen.

use core::fmt;

/// Top-Level-Fehler des Java-Code-Generators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaGenError {
    /// IDL-Konstrukt ist im aktuellen Foundation-Scope (C5.4-a/-b)
    /// nicht unterstuetzt. `construct` ist eine kurze Bezeichnung
    /// (z.B. `"interface"`, `"valuetype"`, `"fixed"`, `"any"`,
    /// `"map"`); seit C5.4-b kann es auch eine Bitset/Bitmask-
    /// Constraint-Verletzung sein (z.B. `"bitset width > 64"`).
    UnsupportedConstruct {
        /// Name des nicht-unterstuetzten Konstrukts.
        construct: String,
        /// Optional: Identifier-Name (Type-Name oder Member-Name).
        context: Option<String>,
    },
    /// Identifier kollidiert mit einem reservierten Java-Keyword.
    /// Java erlaubt keine `@`-Escape-Syntax, daher wird der Identifier
    /// vom Emitter mit `_`-Suffix umbenannt; dieser Fehler tritt nur
    /// auf, wenn der bereinigte Name selbst kollidiert oder leer ist.
    InvalidName {
        /// Der unzulaessige Identifier.
        name: String,
        /// Grund der Ablehnung.
        reason: String,
    },
    /// Inheritance-Cycle im Struct-Graphen (Self-Reference oder
    /// indirekte Schleife). Wird vor der Emission erkannt.
    InheritanceCycle {
        /// Beteiligter Type-Name am Cycle.
        type_name: String,
    },
    /// Generierter Output ist intern inkonsistent (sollte nicht
    /// auftreten — Bug-Indikator).
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
