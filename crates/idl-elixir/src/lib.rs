// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-elixir`. Safety classification: **STANDARD**.
//!
//! IDL4 → Elixir code generator for ZeroDDS data types. Reads the IDL AST from
//! `zerodds-idl` and emits a self-contained Elixir source file: a `Wire` module
//! (byte-identical to the `endpoints/elixir` core) plus, per IDL `struct`, a
//! module with `defstruct` and a `marshal_xcdr(v, endian)` function.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only.
//!
//! # Public API
//!
//! - [`generate_elixir_module`] — AST + options → Elixir source.
//! - [`ElixirGenOptions`] — codegen options.
//! - [`error::IdlElixirError`] — error family.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emitter;
pub mod error;
pub mod keywords;

pub use emitter::{ElixirGenOptions, generate_elixir_module};
pub use error::{IdlElixirError, Result};
