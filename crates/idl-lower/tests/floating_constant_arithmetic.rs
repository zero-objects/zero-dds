// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Floating-point constant expressions, IDL 4.2 §7.4.1.4.2.
//!
//! The clause fixes the arithmetic: "If the type of a floating-point
//! constant is double, then each sub-expression of the associated
//! constant expression is treated as a double", "If either argument is
//! long double, it shall use long double. Otherwise it shall use
//! double", only unary `+ -` and binary `* / + -` apply, an infix
//! operator does not combine a floating operand with an integer one, and
//! the final value has to fit the declared type. Every case below is one
//! of those sentences.
#![allow(clippy::expect_used, clippy::panic)]

#[path = "support/constant.rs"]
mod constant;

use zerodds_ast::{ConstValue, EvalError};

/// The value of constant `name`, as text or as the evaluation error.
fn value_of(idl: &str, name: &str) -> Result<String, EvalError> {
    match constant::evaluate(idl, name)? {
        ConstValue::Float {
            mantissa,
            exponent: None,
        } => Ok(mantissa),
        ConstValue::Float {
            mantissa,
            exponent: Some(exponent),
        } => Ok(format!("{mantissa}e{exponent}")),
        other => panic!("`{name}` came back as {other:?} and not as a float"),
    }
}

#[test]
fn a_difference_of_two_literals_evaluates() {
    assert_eq!(
        value_of("const double G = 0.0 - 1.5;", "G").as_deref(),
        Ok("-1.5")
    );
}

/// "each sub-expression … is treated as a double": `0.1 + 0.2` is the
/// double sum, not the decimal one.
#[test]
fn the_arithmetic_is_double() {
    assert_eq!(
        value_of("const double S = 0.1 + 0.2;", "S").as_deref(),
        Ok("0.30000000000000004")
    );
}

#[test]
fn a_constant_is_an_operand() {
    let idl = "const double A = 1.5;\nconst double B = A * 2.0 / 4.0;";
    assert_eq!(value_of(idl, "B").as_deref(), Ok("0.75"));
}

#[test]
fn a_sign_in_front_of_an_expression_applies_to_its_value() {
    assert_eq!(
        value_of("const double N = -(1.0 + 0.5);", "N").as_deref(),
        Ok("-1.5")
    );
}

#[test]
fn a_large_value_keeps_its_exponent() {
    assert_eq!(
        value_of("const double E = 1.0e300 * 10.0;", "E").as_deref(),
        Ok("1e301")
    );
}

/// "An infix operator may combine two integer types, floating point
/// types or fixed point types, but not mixtures of these."
#[test]
fn an_integer_operand_is_refused() {
    assert!(matches!(
        value_of("const double M = 2 * 1.5;", "M"),
        Err(EvalError::MixedOperands { .. })
    ));
}

/// "Unary (+ -) and binary (* / + -) operators shall be applicable in
/// floating-point and fixed-point expressions" — and no others.
#[test]
fn an_operator_floats_do_not_take_is_refused() {
    assert!(matches!(
        value_of("const double R = 5.0 % 2.0;", "R"),
        Err(EvalError::NotFloatingOperator { .. })
    ));
}

#[test]
fn a_division_by_zero_is_refused() {
    assert!(matches!(
        value_of("const double Z = 1.0 / 0.0;", "Z"),
        Err(EvalError::DivisionByZero { .. })
    ));
}

/// "It is an error if any sub-expression value exceeds the precision of
/// double", and the final value "shall fit in the range of the declared
/// type of the constant".
#[test]
fn a_value_beyond_the_declared_range_is_refused() {
    assert!(matches!(
        value_of("const double D = 1.0e200 * 1.0e200;", "D"),
        Err(EvalError::Overflow { .. })
    ));
    assert!(matches!(
        value_of("const float F = 1.0e30 * 1.0e30;", "F"),
        Err(EvalError::Overflow { .. })
    ));
    // In range for `float`, and the double product, not the decimal one:
    // 1.0e30 has no exact binary form.
    assert_eq!(
        value_of("const float F = 1.0e30 * 1.0e3;", "F"),
        Ok(format!("{:?}", 1.0e30_f64 * 1.0e3_f64))
    );
}

/// A `long double` constant's arithmetic runs in `long double`, which
/// this build cannot do; it is refused by name rather than rounded to
/// `double`. A literal alone needs no arithmetic and still evaluates.
#[test]
fn long_double_arithmetic_is_refused_and_a_long_double_literal_is_not() {
    assert!(matches!(
        value_of("const long double L = 1.0 + 2.0;", "L"),
        Err(EvalError::LongDouble { .. })
    ));
    assert!(matches!(
        value_of("typedef long double LD;\nconst LD T = 1.0 * 2.0;", "T"),
        Err(EvalError::LongDouble { .. })
    ));
    assert!(matches!(
        value_of("const long double W = -(1.0 + 2.0);", "W"),
        Err(EvalError::LongDouble { .. })
    ));
    // "If either argument is long double, it shall use long double."
    assert!(matches!(
        value_of("const long double X = 1.5;\nconst double Y = X * 2.0;", "Y"),
        Err(EvalError::LongDouble { .. })
    ));
    assert_eq!(
        value_of("const long double P = -1.5;", "P").as_deref(),
        Ok("-1.5")
    );
}
