// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 parser, AST, and semantic model (OMG IDL 4.2 / ISO/IEC 19516).
//!
//! Crate `zerodds-idl`.
//!
//! Safety classification: **SAFE (std-only)**.
//! Siehe `docs/architecture/02_architecture.md §3` und
//! `docs/architecture/04_safety_by_architecture.md §2`.
//!
//! Diese Bibliothek liefert Parser, AST-Typen und Semantik-Analyse fuer
//! OMG IDL 4.2. Backend-Code-Generatoren (C, C++, C#, Java, Python, Rust)
//! leben im Binary-Crate `zerodds-idlc`, der diese Bibliothek konsumiert.
//!
//! **Keine no_std-Unterstuetzung:** IDL-Parsing ist eine Build-Zeit-Operation
//! (Tool-Pipeline, Code-Generator). IDL-Strukturen werden zu fertigen Binaries
//! kompiliert, bevor sie auf embedded-Targets deployed werden. Ein no_std-IDL-
//! Parser hat keinen realen Use-Case. Safety-Qualitaet wird ueber
//! `forbid(unsafe_code)` + Workspace-Clippy-Regeln (no panic/unwrap/expect)
//! gesichert, nicht ueber embedded-Faehigkeit. Siehe RFC 0001
//! (`docs/rfcs/0001-idl-parser-architecture.md`).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod ast;
pub mod config;
pub mod cst;
pub mod engine;
pub mod errors;
pub mod features;
pub mod grammar;
pub mod lexer;
pub mod parser;
pub mod preprocessor;
pub mod semantics;

pub use parser::{Error, parse};
