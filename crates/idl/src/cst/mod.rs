// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Concrete Syntax Tree (CST) — untyped Baum-Repraesentation des Parse-
//! Ergebnisses.
//!
//! Datentypen leben in [`node`]. Die Konstruktion eines CST aus einem
//! Recognizer-Output (ParseForest → CST) folgt in Task 2.6.
//!
//! Siehe RFC 0001 §4.1 und §5.4.

pub mod build;
pub mod node;
pub mod walk;

pub use build::build_cst;
pub use node::{CstKind, CstNode};
