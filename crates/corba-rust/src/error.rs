// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fehler-Familie des CORBA-Rust-Codegens.

use core::fmt;

/// Fehler beim Erzeugen von CORBA-Rust-Service-Code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorbaRustError {
    /// IDL-Konstrukt wird vom Rust-Service-Codegen aktuell nicht
    /// unterstuetzt.
    Unsupported {
        /// Konstrukt-Name (z.B. `"abstract interface"`).
        what: &'static str,
        /// Position im IDL-Source (Byte-Offset).
        at: usize,
    },
    /// Type-Reference konnte nicht aufgeloest werden.
    UnresolvedType {
        /// Voll-qualifizierter Name.
        scoped: String,
    },
    /// Fehler im DataType-Codegen (idl-rust) durchgereicht.
    DataType(zerodds_idl_rust::RustGenError),
}

impl fmt::Display for CorbaRustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { what, at } => {
                write!(
                    f,
                    "corba-rust-codegen: unsupported IDL construct `{what}` at offset {at}"
                )
            }
            Self::UnresolvedType { scoped } => {
                write!(f, "corba-rust-codegen: unresolved type `{scoped}`")
            }
            Self::DataType(e) => write!(f, "corba-rust-codegen: data-type emit failed: {e}"),
        }
    }
}

impl std::error::Error for CorbaRustError {}

impl From<zerodds_idl_rust::RustGenError> for CorbaRustError {
    fn from(e: zerodds_idl_rust::RustGenError) -> Self {
        Self::DataType(e)
    }
}

/// Result-Alias.
pub type Result<T> = core::result::Result<T, CorbaRustError>;
