// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL in, the resolved AST out: the front half a generator test runs before
//! its own materialiser, naming the layer that refused.

/// Parses, lowers and resolves `idl`, `origin` naming it in diagnostics.
///
/// The resolution is the step the C, C++ and TypeScript columns take: a member
/// whose type is written as a scoped name arrives as an unresolved reference,
/// and a materialiser that asked `resolved()` without it would report "the
/// member type does not evaluate" for every cross-module reference in the
/// corpus.
pub fn ast(idl: &str, origin: &str) -> Result<zerodds_ast::Ast, String> {
    let parsed = zerodds_idl_parse::parse(idl);
    if !parsed.diagnostics().is_empty() {
        return Err(format!("parse: {:?}", parsed.diagnostics()));
    }
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), origin);
    if !lowered.diagnostics.is_empty() {
        return Err(format!("lower: {:?}", lowered.diagnostics));
    }
    let mut ast = lowered.ast;
    ast.resolve_all_by_name()
        .map_err(|error| format!("resolve: {error:?}"))?;
    Ok(ast)
}
