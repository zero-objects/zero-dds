// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fehler-Typen fuer den IDL → C#-Codegen.

use core::fmt;

/// Top-Level-Fehler des C#-Code-Generators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CsGenError {
    /// IDL-Konstrukt ist im aktuellen Foundation-Scope (C5.3-a) nicht
    /// unterstuetzt. `construct` ist eine kurze Bezeichnung (z.B.
    /// `"interface"`, `"valuetype"`, `"fixed"`, `"any"`, `"map"`,
    /// `"bitset"`, `"bitmask"`).
    UnsupportedConstruct {
        /// Name des nicht-unterstuetzten Konstrukts.
        construct: String,
        /// Optional: Identifier-Name (Type-Name oder Member-Name).
        context: Option<String>,
    },
    /// Identifier kollidiert mit einem reservierten C#-Keyword.
    /// Anders als bei C++ wird in C# nicht abgelehnt, sondern mit
    /// `@`-Prefix escaped (Spec-Verhalten, §6 IDL4-CS-Mapping).
    /// Dieser Fehler tritt nur auf, wenn der Name selbst nach Escape
    /// noch ungueltig waere (leerer String, doppelter `@`, ...).
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
