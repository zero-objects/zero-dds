// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-d`. Safety classification: **STANDARD**.
//!
//! IDL4 → D code generator for ZeroDDS data types. Reads the IDL AST from
//! `zerodds-idl` and emits a self-contained D source file: a shared XCDR2
//! `Writer` (byte-identical to the `endpoints/d` core) plus, per IDL `struct`,
//! a D struct with a `marshalXCDR(endian)` method.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only.
//!
//! # Public API
//!
//! - [`generate_d_module`] — AST + options → D source.
//! - [`DGenOptions`] — codegen options.
//! - [`error::IdlDError`] — error family.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emitter;
pub mod error;
pub mod keywords;

pub use emitter::{DGenOptions, generate_d_module};
pub use error::{IdlDError, Result};
