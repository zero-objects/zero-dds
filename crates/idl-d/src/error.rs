// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error family for the D code generator.

use std::fmt;

/// Convenience result alias for the D backend.
pub type Result<T> = core::result::Result<T, IdlDError>;

/// Errors raised while emitting D source from an IDL AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlDError {
    /// A construct the D backend does not (yet) emit.
    Unsupported(String),
}

impl fmt::Display for IdlDError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported by the D backend: {what}"),
        }
    }
}

impl std::error::Error for IdlDError {}
