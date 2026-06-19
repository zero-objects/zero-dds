// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 parser, AST, and semantic model (OMG IDL 4.2 / ISO/IEC 19516).
//!
//! Crate `zerodds-idl`.
//!
//! Safety classification: **SAFE (std-only)**.
//! See `docs/architecture/02_architecture.md §3` and
//! `docs/architecture/04_safety_by_architecture.md §2`.
//!
//! This library provides the parser, AST types and semantic analysis for
//! OMG IDL 4.2. Backend code generators (C, C++, C#, Java, Python, Rust)
//! live in the binary crate `zerodds-idlc`, which consumes this library.
//!
//! **No no_std support:** IDL parsing is a build-time operation
//! (tool pipeline, code generator). IDL structures are compiled into finished
//! binaries before they are deployed to embedded targets. A no_std IDL
//! parser has no real use case. Safety quality is ensured via
//! `forbid(unsafe_code)` + workspace clippy rules (no panic/unwrap/expect),
//! not via embedded capability. See RFC 0001
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

pub use parser::{Error, parse, parse_source};
