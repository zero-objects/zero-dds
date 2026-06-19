// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Concrete syntax tree (CST) — untyped tree representation of the parse
//! result.
//!
//! Data types live in [`node`]. The construction of a CST from a
//! recognizer output (ParseForest → CST) follows in Task 2.6.
//!
//! See RFC 0001 §4.1 and §5.4.

pub mod build;
pub mod node;
pub mod walk;

pub use build::build_cst;
pub use node::{CstKind, CstNode};
