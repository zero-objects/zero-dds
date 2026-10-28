// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Error family of the CORBA Rust codegen.

use core::fmt;

/// Error while generating CORBA Rust service code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorbaRustError {
    /// IDL construct this generator has no Rust form for.
    Unsupported {
        /// Construct name, e.g. `"abstract interface"`.
        what: &'static str,
        /// Position in the IDL source, as a byte offset. Zero when the
        /// construct was reached through the model rather than through
        /// a span.
        at: usize,
    },
    /// A type reference the frontend left unresolved.
    ///
    /// Reported rather than skipped. `zerodds-idl-lower` leaves a name
    /// it could not bind on the spot `Pending`, and the resolve pass is
    /// the caller's; a generator that silently emitted the unit type
    /// for one would produce a signature no call site matches.
    UnresolvedType {
        /// The name, as far as it is known.
        scoped: String,
    },
    /// The model does not hold a node the generator asked for.
    ///
    /// A stale handle, which is a defect in the caller and not in the
    /// IDL.
    StaleHandle {
        /// What was being generated when the handle was used.
        what: &'static str,
    },
}

impl fmt::Display for CorbaRustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { what, at } => {
                write!(
                    f,
                    "corba-rust-codegen: unsupported IDL construct `{what}` at offset {at}"
                )
            }
            Self::UnresolvedType { scoped } => {
                write!(f, "corba-rust-codegen: unresolved type `{scoped}`")
            }
            Self::StaleHandle { what } => {
                write!(
                    f,
                    "corba-rust-codegen: stale model handle while emitting {what}"
                )
            }
        }
    }
}

impl std::error::Error for CorbaRustError {}

/// Result alias.
pub type Result<T> = core::result::Result<T, CorbaRustError>;
