// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error family for the OCaml code generator.

use std::fmt;

/// Convenience result alias for the OCaml backend.
pub type Result<T> = core::result::Result<T, IdlOcamlError>;

/// Errors raised while emitting OCaml source from an IDL AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlOcamlError {
    /// A construct the OCaml backend does not (yet) emit.
    Unsupported(String),
}

impl fmt::Display for IdlOcamlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported by the OCaml backend: {what}"),
        }
    }
}

impl std::error::Error for IdlOcamlError {}
