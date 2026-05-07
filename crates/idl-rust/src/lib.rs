// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-idl-rust`. Safety classification: **STANDARD**.
//!
//! IDL4 → Rust-Code-Generator fuer ZeroDDS-DataTypes. Liest den IDL-AST
//! aus `zerodds-idl` und emittiert `pub struct ... { ... }` plus
//! `impl zerodds_dcps::DdsType for ...` plus die noetigen
//! `zerodds_cdr::CdrEncode`/`CdrDecode`-Helper.
//!
//! Build-Zeit-Tool — `forbid(unsafe_code)`, std-only, kein no_std-Use-Case.
//!
//! ## Schichten-Position
//!
//! Layer 3 (Schema) — analog zu `zerodds-idl-cpp`/`-csharp`/`-java`/`-ts`,
//! aber emittiert Rust-Code fuer den ZeroDDS-eigenen Stack.
//!
//! ## Public API
//!
//! - [`generate_rust_module`] — AST + Optionen → Rust-Modul-Code.
//! - [`RustGenOptions`] — Codegen-Optionen.
//! - [`error::RustGenError`] — Fehler-Familie.
//!
//! ## Was wird emittiert
//!
//! Pro IDL-Konstrukt:
//!
//! | IDL | Rust |
//! |-----|------|
//! | `struct` (final) | `pub struct` + `impl DdsType` mit XCDR2-Final-Wire |
//! | `struct` (appendable) | mit `zerodds_cdr::struct_enc::encode_appendable` |
//! | `struct` (mutable) | mit `zerodds_cdr::struct_enc::MutableStructEncoder` |
//! | `enum` | `pub enum #[repr(i32)]` + `from_wire` + `CdrEncode/Decode` |
//! | `union` | `pub enum` mit Variants pro Case |
//! | `typedef` | `pub type X = Y;` |
//! | `module` | `pub mod m { ... }` mit nested Definitions |
//! | `@key` | `encode_key_holder_be` Implementation |
//! | `@id(N)` | Member-ID fuer mutable extensibility |
//!
//! ## Beispiel
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
