// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fehler-Typen fuer den IDL→C++-Codegen.

use core::fmt;

/// Top-Level-Fehler des C++-Code-Generators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CppGenError {
    /// IDL-Konstrukt ist im aktuellen Foundation-Scope (C5.1-a) nicht
    /// unterstuetzt. `construct` ist eine kurze Bezeichnung (z.B.
    /// `"interface"`, `"valuetype"`, `"fixed"`, `"any"`, `"map"`).
    UnsupportedConstruct {
        /// Name des nicht-unterstuetzten Konstrukts.
        construct: String,
        /// Optional: Identifier-Name (Type-Name oder Member-Name).
        context: Option<String>,
    },
    /// Identifier kollidiert mit einem reservierten C++-Keyword
    /// (§7.4.5 Implementation-Mapping verlangt Kollisionsvermeidung).
    InvalidName {
        /// Der unzulaessige Identifier.
        name: String,
        /// Grund der Ablehnung (z.B. `"reserved C++ keyword"`).
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

impl fmt::Display for CppGenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedConstruct { construct, context } => match context {
                Some(ctx) => write!(
                    f,
                    "unsupported IDL construct '{construct}' in '{ctx}' (C5.1-a Foundation scope)",
                ),
                None => write!(
                    f,
                    "unsupported IDL construct '{construct}' (C5.1-a Foundation scope)",
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

impl std::error::Error for CppGenError {}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn unsupported_display_has_context() {
        let e = CppGenError::UnsupportedConstruct {
            construct: "interface".into(),
            context: Some("Foo".into()),
        };
        let s = format!("{e}");
        assert!(s.contains("interface"));
        assert!(s.contains("Foo"));
    }

    #[test]
    fn unsupported_display_without_context() {
        let e = CppGenError::UnsupportedConstruct {
            construct: "any".into(),
            context: None,
        };
        let s = format!("{e}");
        assert!(s.contains("any"));
    }

    #[test]
    fn invalid_name_display() {
        let e = CppGenError::InvalidName {
            name: "class".into(),
            reason: "reserved C++ keyword".into(),
        };
        assert!(format!("{e}").contains("reserved"));
    }

    #[test]
    fn inheritance_cycle_display() {
        let e = CppGenError::InheritanceCycle {
            type_name: "Loop".into(),
        };
        assert!(format!("{e}").contains("Loop"));
    }

    #[test]
    fn internal_display() {
        let e = CppGenError::Internal("oops".into());
        assert!(format!("{e}").contains("oops"));
    }
}
