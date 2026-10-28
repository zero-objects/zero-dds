// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared materialise pipeline of the corpus gates.
//!
//! Separate from `corpus.rs` so a gate that only shares the case list
//! does not compile a generator it never calls: every integration test
//! is its own crate, and an unused function in one of them is a
//! warning the lint job treats as an error.

use zerodds_ast::Ast;
use zerodds_lang_cst::Language;

/// IDL in, the declarations text of `language` out — naming the layer
/// that refused instead of panicking.
pub fn generate_declarations(
    idl: &str,
    origin: &str,
    language: Language,
) -> Result<String, String> {
    let parsed = zerodds_idl_parse::parse(idl);
    if !parsed.diagnostics().is_empty() {
        return Err(format!("parse: {:?}", parsed.diagnostics()));
    }
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), origin);
    if !lowered.diagnostics.is_empty() {
        return Err(format!("lower: {:?}", lowered.diagnostics));
    }
    let mut ast: Ast = lowered.ast;
    let report = ast
        .resolve_all_by_name()
        .map_err(|error| format!("resolve: {error:?}"))?;
    if !report.is_clean() {
        return Err(format!("resolve: broken references {:?}", report.broken));
    }
    zerodds_lang_rules::generate::declarations(&ast, language)
        .map_err(|error| format!("materialise: {error}"))
}
