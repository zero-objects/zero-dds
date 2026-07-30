// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-ocaml`. Safety classification: **STANDARD**.
//!
//! IDL4 → OCaml code generator for ZeroDDS data types. Reads the IDL AST from
//! `zerodds-idl` and emits a self-contained OCaml source file: a `Wire` module
//! (byte-identical to the `endpoints/ocaml` core) plus, per IDL `struct`, a
//! module with a record type `t` and a `marshal(v, endian)` function.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only.
//!
//! # Public API
//!
//! - [`generate_ocaml_module`] — AST + options → OCaml source.
//! - [`OcamlGenOptions`] — codegen options.
//! - [`error::IdlOcamlError`] — error family.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emitter;
pub mod error;
mod keywords;

pub use emitter::{OcamlGenOptions, generate_ocaml_module};
pub use error::{IdlOcamlError, Result};
