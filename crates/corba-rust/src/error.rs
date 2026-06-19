// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Error family of the CORBA Rust codegen.

use core::fmt;

/// Error while generating CORBA Rust service code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorbaRustError {
    /// IDL construct currently not supported by the Rust service codegen.
    Unsupported {
        /// Construct name (e.g. `"abstract interface"`).
        what: &'static str,
        /// Position in the IDL source (byte offset).
        at: usize,
    },
    /// Type reference could not be resolved.
    UnresolvedType {
        /// Fully qualified name.
        scoped: String,
    },
    /// Error from the DataType codegen (idl-rust) passed through.
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

/// Result alias.
pub type Result<T> = core::result::Result<T, CorbaRustError>;
