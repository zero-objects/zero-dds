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

/// OMG IDL 4.2 §7.4.1.4.2: "If the type of a floating-point constant is
/// long double, then each sub-expression of the associated constant
/// expression is treated as a long double", and "If either argument is long
/// double, it shall use long double." The arithmetic runs in binary128
/// (§7.4.1.4.4.2.2: an exponent of at least 15 bits and a fraction of at
/// least 64), so `1.0 + 1e-20` keeps what a `double` loses, a value too
/// large for binary128 is an error, and a `double` constant computed from a
/// `long double` holds the `double` the result rounds to.
#[test]
fn idl_7_4_1_4_2_long_double_arithmetic_runs_in_binary128() {
    assert_eq!(
        value_of("const long double L = 1.0 + 2.0;", "L").as_deref(),
        Ok("3.0")
    );
    assert_eq!(
        value_of("typedef long double LD;\nconst LD T = 1.0 * 2.0;", "T").as_deref(),
        Ok("2.0")
    );
    assert_eq!(
        value_of("const long double W = -(1.0 + 2.0);", "W").as_deref(),
        Ok("-3.0")
    );
    assert_eq!(
        value_of("const long double E = 1.0 + 1e-20;", "E").as_deref(),
        Ok("1.00000000000000000001")
    );
    assert_eq!(
        value_of("const double E = 1.0 + 1e-20;", "E").as_deref(),
        Ok("1.0"),
        "the same sum in double"
    );
    // "If either argument is long double, it shall use long double."
    assert_eq!(
        value_of("const long double X = 1.5;\nconst double Y = X * 2.0;", "Y").as_deref(),
        Ok("3.0")
    );
    // A literal too large for a double is a long double, and the result
    // has to fit the declared type.
    assert!(matches!(
        value_of("const double D = 1e400;", "D"),
        Err(EvalError::Overflow { .. })
    ));
    assert!(value_of("const long double B = 1e300 * 1e300;", "B").is_ok());
    assert!(matches!(
        value_of("const long double O = 1e4000 * 1e4000;", "O"),
        Err(EvalError::Overflow { .. })
    ));
    assert!(matches!(
        value_of("const long double Z = 1.0 / 0.0;", "Z"),
        Err(EvalError::DivisionByZero { .. })
    ));
    assert_eq!(
        value_of("const long double P = -1.5;", "P").as_deref(),
        Ok("-1.5")
    );
}
