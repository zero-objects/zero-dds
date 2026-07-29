// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-idl-rust`. Safety classification: **STANDARD**.
//!
//! IDL4 → Rust code generator for ZeroDDS data types. Reads the IDL AST
//! from `zerodds-idl` and emits `pub struct ... { ... }` plus
//! `impl zerodds_dcps::DdsType for ...` plus the necessary
//! `zerodds_cdr::CdrEncode`/`CdrDecode` helpers.
//!
//! Build-time tool — `forbid(unsafe_code)`, std-only, no no_std use case.
//!
//! ## Layer position
//!
//! Layer 3 (schema) — analogous to `zerodds-idl-cpp`/`-csharp`/`-java`/`-ts`,
//! but emits Rust code for the ZeroDDS-native stack.
//!
//! ## Public API
//!
//! - [`generate_rust_module`] — AST + options → Rust module code.
//! - [`RustGenOptions`] — codegen options.
//! - [`error::RustGenError`] — error family.
//!
//! ## What is emitted
//!
//! Per IDL construct:
//!
//! | IDL | Rust |
//! |-----|------|
//! | `struct` (final) | `pub struct` + `impl DdsType` with XCDR2-final wire |
//! | `struct` (appendable) | with `zerodds_cdr::struct_enc::encode_appendable` |
//! | `struct` (mutable) | with `zerodds_cdr::struct_enc::MutableStructEncoder` |
//! | `enum` | `pub enum #[repr(i32)]` + `from_wire` + `CdrEncode/Decode` |
//! | `union` | `pub enum` with variants per case |
//! | `typedef` | `pub type X = Y;` |
//! | `module` | `pub mod m { ... }` with nested definitions |
//! | `@key` | `encode_key_holder_be` implementation |
//! | `@id(N)` | member ID for mutable extensibility |
//!
//! ## Runtime dependencies of the generated code
//!
//! In default mode (`RustGenOptions::cdr_only == false`), the emitted Rust
//! module needs exactly three ZeroDDS crates as `[dependencies]`:
//! `zerodds-cdr` (`CdrEncode`/`CdrDecode`, wire types), `zerodds-dcps`
//! (`DdsType`, plus `FilterValue` — the re-exported SQL-filter value type
//! used by `field_value`) and `zerodds-types` (`TypeIdentifier` for
//! `DdsType::TYPE_IDENTIFIER`). It does **not** need a
//! direct dependency on `zerodds-sql-filter` — `field_value` goes through
//! `zerodds_dcps::FilterValue`, exactly like `#[derive(DdsType)]` in
//! `zerodds-cdr-derive` does, so a minimal typed user never has to add the
//! filter crate to their own `Cargo.toml`.
//!
//! In `cdr_only` mode (the CORBA/GIOP path) the `DdsType` impl is omitted
//! entirely, so only `zerodds-cdr` is needed.
//!
//! ## Example
//!
//! ```rust
//! use zerodds_idl::config::ParserConfig;
//! use zerodds_idl_rust::{generate_rust_module, RustGenOptions};
//!
//! let ast = zerodds_idl::parse(
//!     "struct S { long x; long y; };",
//!     &ParserConfig::default(),
//! )
//! .expect("parse");
//! let rust_src = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");
//! assert!(rust_src.contains("pub struct S"));
//! assert!(rust_src.contains("impl zerodds_dcps::DdsType for S"));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod annotations;
pub mod bitset_emit;
pub mod emitter;
pub mod enum_emit;
pub mod error;
pub mod struct_emit;
pub mod type_identifier;
pub mod type_map;
pub mod typedef_emit;
pub mod union_emit;

pub use emitter::{RustGenOptions, generate_rust_module};
pub use error::{Result, RustGenError};
