// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error family for the Swift code generator.

use std::fmt;

/// Convenience result alias for the Swift backend.
pub type Result<T> = core::result::Result<T, IdlSwiftError>;

/// Errors raised while emitting Swift source from an IDL AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlSwiftError {
    /// A construct the Swift backend does not (yet) emit.
    Unsupported(String),
}

impl fmt::Display for IdlSwiftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported by the Swift backend: {what}"),
        }
    }
}

impl std::error::Error for IdlSwiftError {}
