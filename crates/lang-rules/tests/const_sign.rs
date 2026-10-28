// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A sign in front of a floating constant evaluates.
//!
//! # Why this is a test
//!
//! Because it did not, and the failure said something else. Every
//! `ConstExpr::Unary` and `ConstExpr::Binary` node went to
//! `eval_integral_at` whatever its operands were, so
//! `const double SLOPE = -1.5;` asked for the *integer* value of a
//! `Float` literal and the caller was told "the constant's value does
//! not evaluate" — a sentence about the expression, for what was really
//! a missing arm.
//!
//! Measured through `wireprobe-zerodds-c-gen` before the fix:
//! `const long A = -7;` and `const double B = 1.5;` both carried, and
//! `-1.5`, `-2.5` and `0.0 - 1.5` all came back with that message. So
//! integer arithmetic worked, floating literals worked, and every
//! floating *expression* failed.
//!
//! IDL 4.2 §7.4.1.4.2 has `<const_exp>` over the floating types as much
//! as over the integer ones. What this file pins is the part that needs
//! no arithmetic: `ConstValue::Float` keeps the mantissa as the text the
//! source wrote, so a sign is one character in front of it, nothing is
//! parsed and nothing is rounded.
//!
//! The rest of floating-point constant arithmetic, which does round, is
//! `crates/idl-lower/tests/floating_constant_arithmetic.rs`.
#![allow(clippy::expect_used, clippy::panic)]

use zerodds_ast::{Ast, ConstValue};

/// The value of the one constant an IDL source declares.
fn value_of(idl: &str, name: &str) -> Result<ConstValue, String> {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let ast: Ast = lowered.ast;
    let (_, constant) = ast
        .constants()
        .find(|(_, node)| node.name.as_str() == name)
        .unwrap_or_else(|| panic!("`{name}` is not in the model"));
    ast.eval_const(constant.expr)
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn a_negative_floating_constant_evaluates_to_its_own_text() {
    let value = value_of("const double SLOPE = -1.5;", "SLOPE")
        .unwrap_or_else(|error| panic!("`-1.5` does not evaluate: {error}"));
    let ConstValue::Float { mantissa, .. } = value else {
        panic!("`-1.5` came back as {value:?} and not as a float");
    };
    assert!(
        mantissa.starts_with('-'),
        "the sign is gone: the mantissa reads `{mantissa}`"
    );
    assert!(
        mantissa.contains("1.5"),
        "the digits moved: the mantissa reads `{mantissa}`"
    );
}

#[test]
fn a_leading_plus_leaves_the_value_alone() {
    let value = value_of("const float RISE = +2.5;", "RISE")
        .unwrap_or_else(|error| panic!("`+2.5` does not evaluate: {error}"));
    let ConstValue::Float { mantissa, .. } = value else {
        panic!("`+2.5` came back as {value:?} and not as a float");
    };
    assert!(
        !mantissa.starts_with('-') && mantissa.contains("2.5"),
        "a leading plus changed the value: `{mantissa}`"
    );
}

/// Two signs cancel rather than stacking into text no reader would take
/// for a number.
#[test]
fn two_signs_cancel() {
    let value = value_of("const double UP = - -1.5;", "UP")
        .unwrap_or_else(|error| panic!("`- -1.5` does not evaluate: {error}"));
    let ConstValue::Float { mantissa, .. } = value else {
        panic!("`- -1.5` came back as {value:?}");
    };
    assert!(
        !mantissa.starts_with('-'),
        "`- -1.5` is positive and the mantissa reads `{mantissa}`"
    );
    assert!(
        !mantissa.contains("--"),
        "the signs stacked instead of cancelling: `{mantissa}`"
    );
}

/// The integer side, which always worked, and which the fix must not
/// have taken away.
#[test]
fn a_negative_integer_constant_still_evaluates() {
    let value = value_of("const long OFFSET = -7;", "OFFSET")
        .unwrap_or_else(|error| panic!("`-7` does not evaluate: {error}"));
    assert_eq!(
        value,
        ConstValue::Integer(-7),
        "the integer path moved when the floating one was added"
    );
}
