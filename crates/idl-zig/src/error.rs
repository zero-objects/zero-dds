// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error family for the Zig code generator.

use std::fmt;

/// Convenience result alias for the Zig backend.
pub type Result<T> = core::result::Result<T, IdlZigError>;

/// Errors raised while emitting Zig source from an IDL AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlZigError {
    /// A construct the Zig backend does not (yet) emit.
    Unsupported(String),
}

impl fmt::Display for IdlZigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported by the Zig backend: {what}"),
        }
    }
}

impl std::error::Error for IdlZigError {}
