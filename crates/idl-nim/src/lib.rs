// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-nim`. Safety classification: **STANDARD**.
//!
//! IDL4 → Nim code generator for ZeroDDS data types. Reads the IDL AST from
//! `zerodds-idl` and emits a self-contained Nim source file: a shared XCDR2
//! `Writer` (byte-identical to the `endpoints/nim` core) plus, per IDL
//! `struct`, a Nim `object` with a `marshalXCDR(endian)` proc.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only.
//!
//! # Public API
//!
//! - [`generate_nim_module`] — AST + options → Nim source.
//! - [`NimGenOptions`] — codegen options.
//! - [`error::IdlNimError`] — error family.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emitter;
pub mod error;
pub mod keywords;

pub use emitter::{NimGenOptions, generate_nim_module};
pub use error::{IdlNimError, Result};
