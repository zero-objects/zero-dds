// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-idl-parse`. Safety classification: **STANDARD**.
//!
//! Not **SAFE**, and that is the specification's own line:
//! `docs/architecture/04_safety_by_architecture.md` §2 lists the safe
//! subset and says the `zerodds-idl*` tools do not belong to the
//! no_std safety profile. The parser runs at code-generation time; the
//! SAFE marker it used to carry was applied across the board and made
//! `dds_no_panic_in_safe` demand a contract this crate never signed.
//!
//! A hand-written recursive-descent parser for OMG IDL 4.2 that builds
//! the lossless CST in `zerodds-idl-cst`.
//!
//! It never stops at the first error. A syntax error is diagnosed, the
//! offending range becomes an error node, and parsing resynchronises at
//! the next declaration boundary — so a file with three mistakes reports
//! three rather than costing three runs.
//!
//! ## Spec
//!
//! - `docs/specs/zerodds-idl-frontend-1.0.md` §3 (parsing), §7
//!   (diagnostics).
//! - OMG IDL 4.2 §7.4 (grammar).
//!
//! ## Layer position
//!
//! Layer 1 of the frontend — over `zerodds-idl-cst`.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(test)]
extern crate std;

mod decl;
pub mod diagnostic;
pub mod dialect;
pub mod parser;
pub mod preprocess;
mod suffix;
mod type_spec;

pub use diagnostic::{Diagnostic, DiagnosticCode, Severity, refused_by_a_clause};
pub use dialect::{Dialect, VendorFlag};
pub use parser::{Parse, Parser, parse, parse_preprocessed, parse_preprocessed_with, parse_with};
pub use preprocess::{
    FileLoader, MemoryLoader, Origin, Preprocessed, Provenance, ProvenanceMacro, SourceMap,
    preprocess, preprocess_with,
};
