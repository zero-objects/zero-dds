// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-lua`. Safety classification: **STANDARD**.
//!
//! IDL4 → Lua code generator for ZeroDDS data types. Reads the IDL AST from
//! `zerodds-idl` and emits a self-contained Lua source file: a `Writer` built
//! on `string.pack` (byte-identical to the `endpoints/lua` core, no FFI) plus,
//! per IDL `struct`, a `marshal_<name>(v, endian)` function.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only.
//!
//! # Public API
//!
//! - [`generate_lua_module`] — AST + options → Lua source.
//! - [`LuaGenOptions`] — codegen options.
//! - [`error::IdlLuaError`] — error family.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emitter;
pub mod error;
mod keywords;

pub use emitter::{LuaGenOptions, generate_lua_module};
pub use error::{IdlLuaError, Result};
