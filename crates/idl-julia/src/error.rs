// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error family for the Julia code generator.

use std::fmt;

/// Convenience result alias for the Julia backend.
pub type Result<T> = core::result::Result<T, IdlJuliaError>;

/// Errors raised while emitting Julia source from an IDL AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlJuliaError {
    /// A construct the Julia backend does not (yet) emit.
    Unsupported(String),
}

impl fmt::Display for IdlJuliaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported by the Julia backend: {what}"),
        }
    }
}

impl std::error::Error for IdlJuliaError {}
