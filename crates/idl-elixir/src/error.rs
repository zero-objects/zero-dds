// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error family for the Elixir code generator.

use std::fmt;

/// Convenience result alias for the Elixir backend.
pub type Result<T> = core::result::Result<T, IdlElixirError>;

/// Errors raised while emitting Elixir source from an IDL AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlElixirError {
    /// A construct the Elixir backend does not (yet) emit.
    Unsupported(String),
}

impl fmt::Display for IdlElixirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported by the Elixir backend: {what}"),
        }
    }
}

impl std::error::Error for IdlElixirError {}
