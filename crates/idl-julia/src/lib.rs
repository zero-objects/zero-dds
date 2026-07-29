// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-julia`. Safety classification: **STANDARD**.
//!
//! IDL4 → Julia code generator for ZeroDDS data types. Reads the IDL AST from
//! `zerodds-idl` and emits a self-contained Julia source file: a `Writer`
//! (byte-identical to the `endpoints/julia` core) plus, per IDL `struct`, a
//! Julia struct with a `marshal_xcdr(v, endian)` function.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only.
//!
//! # Public API
//!
//! - [`generate_julia_module`] — AST + options → Julia source.
//! - [`JuliaGenOptions`] — codegen options.
//! - [`error::IdlJuliaError`] — error family.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emitter;
pub mod error;
pub mod keywords;

pub use emitter::{JuliaGenOptions, generate_julia_module};
pub use error::{IdlJuliaError, Result};
