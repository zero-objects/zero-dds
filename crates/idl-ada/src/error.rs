// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error family for the Ada code generator.

use std::fmt;

/// Convenience result alias for the Ada backend.
pub type Result<T> = core::result::Result<T, IdlAdaError>;

/// Errors raised while emitting Ada source from an IDL AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlAdaError {
    /// A construct the Ada backend does not (yet) emit.
    Unsupported(String),
}

impl fmt::Display for IdlAdaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported by the Ada backend: {what}"),
        }
    }
}

impl std::error::Error for IdlAdaError {}
