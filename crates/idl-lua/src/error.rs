// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error family for the Lua code generator.

use std::fmt;

/// Convenience result alias for the Lua backend.
pub type Result<T> = core::result::Result<T, IdlLuaError>;

/// Errors raised while emitting Lua source from an IDL AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlLuaError {
    /// A construct the Lua backend does not (yet) emit.
    Unsupported(String),
}

impl fmt::Display for IdlLuaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported by the Lua backend: {what}"),
        }
    }
}

impl std::error::Error for IdlLuaError {}
