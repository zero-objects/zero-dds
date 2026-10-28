// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-lang-rules`. Safety classification: **SAFE**.
//!
//! The common AST onto the language CSTs, through rules that are data.
//!
//! A rule is a value: comparable, serialisable, and checkable for
//! completeness. Two languages' treatment of `@optional` is a diff, not
//! a reading of two emitters three thousand lines apart.
//!
//! ## Spec
//!
//! - `docs/specs/zerodds-lang-cst-1.0.md` §2, §3, §4.
//!
//! ## The engine
//!
//! `seesaw-tgg` 2.0 — the positive-only, NAC-free rule layer, released.
//! A version dependency rather than a path: a path builds only on the
//! machine that wrote it.
//!
//! ## Layer position
//!
//! Layer 2 of the backend arc — over `zerodds-ast` and
//! `zerodds-lang-cst`.
#![forbid(unsafe_code)]

pub mod adapter;
pub mod apply;
pub mod coverage;
pub mod decode_c;
pub mod decode_cpp;
pub mod decode_java;
pub mod default_ada;
pub mod default_ada83;
pub mod default_c;
mod default_common;
pub mod default_cpp;
pub mod default_csharp;
pub mod default_d;
pub mod default_elixir;
pub mod default_fsharp;
pub mod default_go;
pub mod default_java;
pub mod default_julia;
pub mod default_kotlin;
pub mod default_lua;
pub mod default_nim;
pub mod default_node;
pub mod default_ocaml;
pub mod default_python;
pub mod default_rust;
pub mod default_swift;
pub mod default_typescript;
pub mod default_zig;
pub mod emit;
pub mod encode;
pub mod encode_c;
mod fixed_literal;
pub mod generate;
pub mod rule;
mod rust_union;
pub mod verbatim;

pub use adapter::ast::{ModelGraph, NodeRef};
pub use apply::{Applied, apply};
pub use coverage::{ConstructKind, Coverage, coverage_of};
