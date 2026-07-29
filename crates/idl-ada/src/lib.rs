// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-ada`. Safety classification: **STANDARD**.
//!
//! IDL4 → Ada code generator for ZeroDDS data types. Reads the IDL AST from
//! `zerodds-idl` and emits a self-contained Ada package (spec + body): a
//! bounded XCDR2 wire buffer (byte-identical to the `endpoints/ada-native`
//! core) plus, per IDL `struct`, an Ada record and a `Marshal` function.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only. Target: Ada 2012.
//!
//! # Public API
//!
//! - [`generate_ada_module`] — AST + options → [`AdaModule`] (`.ads` + `.adb`).
//! - [`AdaGenOptions`] — codegen options.
//! - [`error::IdlAdaError`] — error family.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emitter;
pub mod error;
pub mod keywords;

pub use emitter::{AdaGenOptions, AdaModule, generate_ada_module};
pub use error::{IdlAdaError, Result};
