// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-idl-go`. Safety classification: **STANDARD**.
//!
//! IDL4 → Go code generator for ZeroDDS data types. Reads the IDL AST from
//! `zerodds-idl` and emits a self-contained Go file: a shared XCDR2 wire
//! `Writer` (byte-identical to the hand-written `endpoints/go` core) plus, per
//! IDL `struct`, a Go struct and a `MarshalXCDR(endian)` method.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only.
//!
//! # Layer position
//!
//! Layer 3 (schema) — analogous to `zerodds-idl-cpp` / `-rust` / `-python`,
//! but emits Go whose wire output is byte-identical to the Rust goldens.
//!
//! # Public API
//!
//! - [`generate_go_module`] — AST + options → Go source.
//! - [`GoGenOptions`] — codegen options.
//! - [`error::IdlGoError`] — error family.
//!
//! # What is emitted
//!
//! | IDL | Go |
//! |-----|-----|
//! | `struct` `@final` | struct + compact `MarshalXCDR` (no DHEADER) |
//! | `struct` `@appendable` | struct + DHEADER-framed `MarshalXCDR` |
//! | `octet` / `uint8` | `byte` / `uint8` |
//! | `short`..`long long` (signed/unsigned) | `int16`..`uint64` |
//! | `float` / `double` | `float32` / `float64` |
//! | `boolean` / `char` | `bool` / `byte` |
//! | `string` | `string` |
//! | `sequence<octet>` | `[]byte` |
//! | `sequence<primitive>` | `[]T` (u32 count + per-element) |
//!
//! `@mutable`, unions, nested-struct members, maps, `long double`, `wchar`,
//! and `wstring` currently raise [`error::IdlGoError::Unsupported`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emitter;
pub mod error;
mod keywords;

pub use emitter::{GoGenOptions, generate_go_module};
pub use error::{IdlGoError, Result};
