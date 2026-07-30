// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-zig`. Safety classification: **STANDARD**.
//!
//! IDL4 → Zig code generator for ZeroDDS data types. Reads the IDL AST from
//! `zerodds-idl` and emits a self-contained Zig source file: a shared XCDR2
//! `Writer` (byte-identical to the `endpoints/zig` core) plus, per IDL
//! `struct`, a Zig struct with a `marshalXCDR(endian, allocator)` method.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only.
//!
//! # Public API
//!
//! - [`generate_zig_module`] — AST + options → Zig source.
//! - [`ZigGenOptions`] — codegen options.
//! - [`error::IdlZigError`] — error family.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emitter;
pub mod error;
pub mod keywords;

pub use emitter::{ZigGenOptions, generate_zig_module};
pub use error::{IdlZigError, Result};
