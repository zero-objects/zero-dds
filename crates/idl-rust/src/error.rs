// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Error family of the Rust codegen.

use core::fmt;

/// Error while generating Rust code from the IDL AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RustGenError {
    /// The IDL construct is currently not supported by the Rust codegen.
    ///
    /// Not a spec bug — some IDL features (Fixed, Map, Any, Bitset,
    /// Bitmask, Valuetype, Interface, Component, Home) are outside
    /// the DDS-DataType path. Whoever generates DDS DataTypes does not
    /// need them.
    Unsupported {
        /// Construct name (e.g. `"fixed"`).
        what: &'static str,
        /// Position in the IDL source (byte offset from the AST span).
        at: usize,
    },
    /// The annotation value could not be evaluated.
    InvalidAnnotation {
        /// Annotation name (e.g. `"@id"`).
        name: String,
        /// Reason.
        reason: &'static str,
    },
    /// The type reference could not be resolved.
    UnresolvedType {
        /// Fully qualified name.
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
