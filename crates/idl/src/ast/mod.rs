// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Typed abstract syntax tree for OMG IDL 4.2 (T5.1).
//!
//! In contrast to [`crate::cst`], the AST loses grammar-specific
//! boilerplate (list helpers, disambiguation alternatives) and keeps only
//! the semantically relevant structure. Each node carries a [`Span`] into
//! the source text for diagnostics.
//!
//! # Layout
//! - [`types`] — all AST structures and enums.
//! - `builder` (T5.2) — CST → AST conversion.
//! - `print` (T5.3) — Display/pretty-print.
//!
//! The module root re-exports the public types for ergonomic access.

pub mod builder;
pub mod print;
pub mod types;

pub use builder::{BuildResult, BuilderError, build};
pub use types::*;
