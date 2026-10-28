// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fixed-point constant expressions, IDL 4.2 §7.4.1.4.2 and Table 7-11.
//!
//! The lowering already gave a `const fixed` the `fixed<d,s>` its
//! expression has; the value was not computed, and every expression with
//! an operator came back "not integral". Each pairwise computation is
//! exact here, a result of more than 31 significant digits keeps 31 with
//! "the omitted digits … discarded; rounding shall not be performed", and
//! only `+ - * /` apply.
#![allow(clippy::expect_used, clippy::panic)]

#[path = "support/constant.rs"]
mod constant;

use zerodds_ast::{ConstValue, EvalError};

/// The value of constant `name` as decimal text, or the evaluation error.
fn value_of(idl: &str, name: &str) -> Result<String, EvalError> {
    match constant::evaluate(idl, name)? {
        ConstValue::Fixed { digits, scale } => Ok(decimal(&digits, scale)),
        other => panic!("`{name}` came back as {other:?} and not as a fixed-point value"),
    }
}

/// `digits` with the point put back `scale` places from the right.
fn decimal(digits: &str, scale: u8) -> String {
    let (sign, magnitude) = digits
        .strip_prefix('-')
        .map_or(("", digits), |rest| ("-", rest));
    let scale = usize::from(scale);
    if scale == 0 {
        return format!("{sign}{magnitude}");
    }
    let padded = format!("{magnitude:0>width$}", width = scale + 1);
    let split = padded.len() - scale;
    format!("{sign}{}.{}", &padded[..split], &padded[split..])
}

#[test]
fn a_sum_lines_the_points_up_and_drops_the_insignificant_zero() {
    assert_eq!(
        value_of("const fixed A = 1.50d + 2.5d;", "A").as_deref(),
        Ok("4")
    );
    assert_eq!(
        value_of("const fixed S = 1.0d - 1.5d;", "S").as_deref(),
        Ok("-0.5")
    );
}

#[test]
fn a_product_carries_both_scales() {
    assert_eq!(
        value_of("const fixed P = 1.1d * 1.1d;", "P").as_deref(),
        Ok("1.21")
    );
}

#[test]
fn a_sign_in_front_of_an_expression_applies_to_its_value() {
    assert_eq!(
        value_of("const fixed N = -(1.25d * 4.0d);", "N").as_deref(),
        Ok("-5")
    );
}

#[test]
fn a_constant_is_an_operand() {
    let idl = "const fixed A = 0.5d;\nconst fixed B = A + A;";
    assert_eq!(value_of(idl, "B").as_deref(), Ok("1"));
}

/// A quotient that comes out even is exact; one that does not is cut on
/// the right, never rounded up.
#[test]
fn a_quotient_is_exact_or_cut_without_rounding() {
    assert_eq!(
        value_of("const fixed Q = 1.0d / 8.0d;", "Q").as_deref(),
        Ok("0.125")
    );
    let third = value_of("const fixed T = 2.0d / 3.0d;", "T").expect("2/3");
    assert!(third.starts_with("0.666"), "{third}");
    assert!(third.ends_with('6'), "rounded: {third}");
    assert!(third.len() <= 33, "more than 31 digits: {third}");
}

#[test]
fn a_division_by_zero_is_refused() {
    assert!(matches!(
        value_of("const fixed Z = 1.0d / 0.0d;", "Z"),
        Err(EvalError::DivisionByZero { .. })
    ));
}

/// "Unary (+ -) and binary (* / + -) operators shall be applicable in
/// floating-point and fixed-point expressions" — and no others.
#[test]
fn an_operator_fixed_values_do_not_take_is_refused() {
    assert!(matches!(
        value_of("const fixed R = 5.0d % 2.0d;", "R"),
        Err(EvalError::NotFixedOperator { .. })
    ));
}

/// More than 31 digits before the point leaves nothing on the right to
/// discard.
#[test]
fn a_result_past_31_whole_digits_is_refused() {
    let nines = "9".repeat(31);
    assert!(matches!(
        value_of(&format!("const fixed L = {nines}.0d * 10.0d;"), "L"),
        Err(EvalError::Overflow { .. })
    ));
}
