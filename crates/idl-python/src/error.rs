// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error family for the `zerodds-idl-python` codegen.

use core::fmt;

/// Codegen error while emitting Python code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlPythonError {
    /// IDL construct outside the supported Python mapping scope
    /// (e.g. bitmask, map, fixed, any, valuetype, interface). Phase-2
    /// material.
    Unsupported(String),
    /// Language-map conflict: an IDL identifier collides with a
    /// Python reserved word and the configured escape mode would
    /// cause two different IDL names to yield the same Python
    /// name.
    NameConflict(String),
}

impl fmt::Display for IdlPythonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(msg) => write!(f, "unsupported IDL construct: {msg}"),
            Self::NameConflict(msg) => write!(f, "python name conflict: {msg}"),
        }
    }
}

impl std::error::Error for IdlPythonError {}

/// Specialized `Result` for codegen calls.
pub type Result<T> = core::result::Result<T, IdlPythonError>;
