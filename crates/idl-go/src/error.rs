// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error family for the Go code generator.

use std::fmt;

/// Convenience result alias for the Go backend.
pub type Result<T> = core::result::Result<T, IdlGoError>;

/// Errors raised while emitting Go source from an IDL AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlGoError {
    /// A construct the Go backend does not (yet) emit.
    Unsupported(String),
}

impl fmt::Display for IdlGoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported by the Go backend: {what}"),
        }
    }
}

impl std::error::Error for IdlGoError {}
