// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The constant-expression tests' one step in common: lower a source and
//! evaluate one of its constants.

use zerodds_ast::{Ast, ConstValue, EvalError};

/// What constant `name` of `idl` evaluates to, or the evaluation error.
///
/// # Panics
/// When the source does not parse or declares no constant `name`.
pub fn evaluate(idl: &str, name: &str) -> Result<ConstValue, EvalError> {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    let ast: Ast = lowered.ast;
    let (_, constant) = ast
        .constants()
        .find(|(_, node)| node.name.as_str() == name)
        .unwrap_or_else(|| panic!("`{name}` is not in the model"));
    ast.eval_const(constant.expr)
}
