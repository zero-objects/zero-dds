// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fehler-Familie des Rust-Codegens.

use core::fmt;

/// Fehler beim Erzeugen von Rust-Code aus dem IDL-AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RustGenError {
    /// IDL-Konstrukt wird vom Rust-Codegen aktuell nicht unterstuetzt.
    ///
    /// Ist kein Spec-Bug — manche IDL-Features (Fixed, Map, Any, Bitset,
    /// Bitmask, Valuetype, Interface, Component, Home) sind ausserhalb
    /// des DDS-DataType-Pfades. Wer DDS-DataTypes generiert braucht sie
    /// nicht.
    Unsupported {
        /// Konstrukt-Name (z.B. `"fixed"`).
        what: &'static str,
        /// Position im IDL-Source (Byte-Offset vom AST-Span).
        at: usize,
    },
    /// Annotation-Wert konnte nicht ausgewertet werden.
    InvalidAnnotation {
        /// Annotation-Name (z.B. `"@id"`).
        name: String,
        /// Begruendung.
        reason: &'static str,
    },
    /// Type-Reference konnte nicht aufgeloest werden.
    UnresolvedType {
        /// Voll-qualifizierter Name.
        scoped: String,
    },
}

impl fmt::Display for RustGenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { what, at } => {
                write!(
                    f,
                    "rust-codegen: unsupported IDL construct `{what}` at offset {at}"
                )
            }
            Self::InvalidAnnotation { name, reason } => {
                write!(f, "rust-codegen: invalid annotation `{name}`: {reason}")
            }
            Self::UnresolvedType { scoped } => {
                write!(f, "rust-codegen: unresolved type `{scoped}`")
            }
        }
    }
}

impl std::error::Error for RustGenError {}

/// Result-Alias.
pub type Result<T> = core::result::Result<T, RustGenError>;
