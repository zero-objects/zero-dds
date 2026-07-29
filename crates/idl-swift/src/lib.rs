// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-swift`. Safety classification: **STANDARD**.
//!
//! IDL4 → Swift code generator for ZeroDDS data types. Reads the IDL AST from
//! `zerodds-idl` and emits a self-contained Swift source file: a `Writer`
//! (byte-identical to the `endpoints/swift` core) plus, per IDL `struct`, a
//! Swift struct with a `marshalXCDR(_ endian)` method.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only.
//!
//! # Public API
//!
//! - [`generate_swift_module`] — AST + options → Swift source.
//! - [`SwiftGenOptions`] — codegen options.
//! - [`error::IdlSwiftError`] — error family.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emitter;
pub mod error;
mod keywords;

pub use emitter::{SwiftGenOptions, generate_swift_module};
pub use error::{IdlSwiftError, Result};
