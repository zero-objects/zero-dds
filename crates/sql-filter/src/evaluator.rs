// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Expression evaluator.
//!
//! Evaluation is strict — a type mismatch (e.g. String < Int) returns
//! `EvalError::TypeMismatch`. The caller decides whether to treat that
//! as `filter denies` or as `filter error`.

use alloc::string::String;

use crate::ast::{CmpOp, Expr, Operand, Value};

/// Row abstraction: access to the fields of a sample.
///
/// Implementers map dotted paths (`a.b.c`) to the matching values. For
/// simple flat structs a `HashMap<String, Value>` lookup is enough.
pub trait RowAccess {
    /// Returns the value of the field path if present. `None` = no field
    /// with that name.
    fn get(&self, path: &str) -> Option<Value>;
}

/// Error during evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// Field not found.
    UnknownField(String),
    /// Parameter index outside the passed slice.
    MissingParam(u32),
    /// Operator not compatible with the operand types.
    TypeMismatch(String),
}

impl core::fmt::Display for EvalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownField(n) => write!(f, "unknown field: {n}"),
            Self::MissingParam(i) => write!(f, "missing parameter %{i}"),
            Self::TypeMismatch(m) => write!(f, "type mismatch: {m}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EvalError {}

impl Expr {
    /// Evaluates the expression against a row + parameter slice.
    ///
    /// # Errors
    /// See [`EvalError`].
    pub fn evaluate<R: RowAccess>(&self, row: &R, params: &[Value]) -> Result<bool, EvalError> {
        match self {
            Self::And(a, b) => Ok(a.evaluate(row, params)? && b.evaluate(row, params)?),
            Self::Or(a, b) => Ok(a.evaluate(row, params)? || b.evaluate(row, params)?),
            Self::Not(inner) => Ok(!inner.evaluate(row, params)?),
            Self::Cmp { lhs, op, rhs } => {
                let l = resolve_operand(lhs, row, params)?;
                let r = resolve_operand(rhs, row, params)?;
                cmp(&l, *op, &r)
            }
            Self::Between {
                field,
                low,
                high,
                negated,
            } => {
                let f = resolve_operand(field, row, params)?;
                let lo = resolve_operand(low, row, params)?;
                let hi = resolve_operand(high, row, params)?;
                // No value lies between, or outside, bounds that are null.
                if [&f, &lo, &hi].iter().any(|v| matches!(v, Value::Null)) {
                    return Ok(false);
                }
                let in_range = cmp(&f, CmpOp::Ge, &lo)? && cmp(&f, CmpOp::Le, &hi)?;
                Ok(if *negated { !in_range } else { in_range })
            }
        }
    }
}

fn resolve_operand<R: RowAccess>(
    op: &Operand,
    row: &R,
    params: &[Value],
) -> Result<Value, EvalError> {
    match op {
        Operand::Literal(v) => Ok(v.clone()),
        Operand::Field(name) => row
            .get(name)
            .ok_or_else(|| EvalError::UnknownField(name.clone())),
        Operand::Param(i) => params
            .get(*i as usize)
            .cloned()
            .ok_or(EvalError::MissingParam(*i)),
    }
}

fn cmp(lhs: &Value, op: CmpOp, rhs: &Value) -> Result<bool, EvalError> {
    // XTypes 1.3 §7.6.7.2: a member that takes no value equals `null` and
    // nothing else does; "Inequalities expressed relative to null shall
    // never evaluate to true — no value is greater than or less than null."
    if matches!(lhs, Value::Null) || matches!(rhs, Value::Null) {
        let both = matches!(lhs, Value::Null) && matches!(rhs, Value::Null);
        return Ok(match op {
            CmpOp::Eq => both,
            CmpOp::Neq => !both,
            CmpOp::Lt | CmpOp::Le | CmpOp::Gt | CmpOp::Ge | CmpOp::Like => false,
        });
    }
    // §7.6.7.1: a bitmask flag has a Boolean type, and "comparisons with
    // the integer literals 1 and 0 shall also be allowed".
    if let (Value::Bool(b), Value::Int(n)) | (Value::Int(n), Value::Bool(b)) = (lhs, rhs) {
        if matches!(op, CmpOp::Eq | CmpOp::Neq) && (*n == 0 || *n == 1) {
            let equal = (*n == 1) == *b;
            return Ok(if matches!(op, CmpOp::Eq) {
                equal
            } else {
                !equal
            });
        }
    }
    // Two integers compare exactly: through f64 every pair above 2^53 that
    // differs only in the low bits would compare equal.
    if let (Value::Int(l), Value::Int(r)) = (lhs, rhs) {
        return match op {
            CmpOp::Eq => Ok(l == r),
            CmpOp::Neq => Ok(l != r),
            CmpOp::Lt => Ok(l < r),
            CmpOp::Le => Ok(l <= r),
            CmpOp::Gt => Ok(l > r),
            CmpOp::Ge => Ok(l >= r),
            CmpOp::Like => Err(EvalError::TypeMismatch("LIKE only for String".into())),
        };
    }
    // Numeric promotion for int/float comparisons.
    if let (Some(l), Some(r)) = (as_f64(lhs), as_f64(rhs)) {
        return Ok(match op {
            CmpOp::Eq => (l - r).abs() < f64::EPSILON,
            CmpOp::Neq => (l - r).abs() >= f64::EPSILON,
            CmpOp::Lt => l < r,
            CmpOp::Le => l <= r,
            CmpOp::Gt => l > r,
            CmpOp::Ge => l >= r,
            CmpOp::Like => {
                return Err(EvalError::TypeMismatch("LIKE only for String".into()));
            }
        });
    }

    match (lhs, rhs, op) {
        (Value::String(a), Value::String(b), CmpOp::Eq) => Ok(a == b),
        (Value::String(a), Value::String(b), CmpOp::Neq) => Ok(a != b),
        (Value::String(a), Value::String(b), CmpOp::Lt) => Ok(a < b),
        (Value::String(a), Value::String(b), CmpOp::Le) => Ok(a <= b),
        (Value::String(a), Value::String(b), CmpOp::Gt) => Ok(a > b),
        (Value::String(a), Value::String(b), CmpOp::Ge) => Ok(a >= b),
        (Value::String(a), Value::String(b), CmpOp::Like) => Ok(like_match(a, b)),
        (Value::Bool(a), Value::Bool(b), CmpOp::Eq) => Ok(a == b),
        (Value::Bool(a), Value::Bool(b), CmpOp::Neq) => Ok(a != b),
        (a, b, op) => Err(EvalError::TypeMismatch(alloc::format!(
            "{a:?} {op:?} {b:?}"
        ))),
    }
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        #[allow(clippy::cast_precision_loss)]
        Value::Int(n) => Some(*n as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

/// SQL-92 LIKE match with `%` (zero-or-more) and `_` (exactly one character).
/// Backslash escape is not implemented — spec §B.2.1 does not require it;
/// %/_ in data must be introduced by the caller via double-encoding.
fn like_match(s: &str, pat: &str) -> bool {
    // Classic DP: m[i][j] = s[..i] matches pat[..j]
    let s_chars: alloc::vec::Vec<char> = s.chars().collect();
    let p_chars: alloc::vec::Vec<char> = pat.chars().collect();
    let (m, n) = (s_chars.len(), p_chars.len());
    let mut dp = alloc::vec![alloc::vec![false; n + 1]; m + 1];
    dp[0][0] = true;
    for j in 1..=n {
        if p_chars[j - 1] == '%' {
            dp[0][j] = dp[0][j - 1];
        }
    }
    for i in 1..=m {
        for j in 1..=n {
            let pc = p_chars[j - 1];
            dp[i][j] = if pc == '%' {
                dp[i - 1][j] || dp[i][j - 1]
            } else if pc == '_' || pc == s_chars[i - 1] {
                dp[i - 1][j - 1]
            } else {
                false
            };
        }
    }
    dp[m][n]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use alloc::collections::BTreeMap;

    struct MapRow(BTreeMap<String, Value>);
    impl RowAccess for MapRow {
        fn get(&self, path: &str) -> Option<Value> {
            self.0.get(path).cloned()
        }
    }

    fn row(pairs: &[(&str, Value)]) -> MapRow {
        let mut m = BTreeMap::new();
        for (k, v) in pairs {
            m.insert((*k).into(), v.clone());
        }
        MapRow(m)
    }

    #[test]
    fn a_boolean_literal_compares_with_a_boolean_member_in_any_case() {
        // XTypes 1.3 §7.6.7.3: BOOLEANVALUE is true or false, case-insensitive.
        let on = row(&[("flag", Value::Bool(true))]);
        let off = row(&[("flag", Value::Bool(false))]);
        for (text, on_matches, off_matches) in [
            ("flag = true", true, false),
            ("flag = TRUE", true, false),
            ("flag = True", true, false),
            ("flag = false", false, true),
            ("flag = FaLsE", false, true),
            ("flag <> TRUE", false, true),
        ] {
            let e = parse(text).unwrap();
            assert_eq!(e.evaluate(&on, &[]), Ok(on_matches), "{text} on true");
            assert_eq!(e.evaluate(&off, &[]), Ok(off_matches), "{text} on false");
        }
    }

    #[test]
    fn evaluates_string_eq() {
        let e = parse("color = 'RED'").unwrap();
        let r = row(&[("color", Value::String("RED".into()))]);
        assert_eq!(e.evaluate(&r, &[]), Ok(true));
    }

    #[test]
    fn evaluates_int_compare() {
        let e = parse("x > 10 AND x <= 100").unwrap();
        let r = row(&[("x", Value::Int(42))]);
        assert_eq!(e.evaluate(&r, &[]), Ok(true));
    }

    #[test]
    fn evaluates_float_int_cross() {
        // Int on one side, float on the other — promoted to f64.
        let e = parse("x < 3.5").unwrap();
        let r = row(&[("x", Value::Int(3))]);
        assert_eq!(e.evaluate(&r, &[]), Ok(true));
    }

    #[test]
    fn evaluates_boolean_not_or() {
        let e = parse("NOT (x = 0 OR y = 0)").unwrap();
        let r = row(&[("x", Value::Int(1)), ("y", Value::Int(2))]);
        assert_eq!(e.evaluate(&r, &[]), Ok(true));
    }

    #[test]
    fn evaluates_param() {
        let e = parse("color = %0").unwrap();
        let r = row(&[("color", Value::String("BLUE".into()))]);
        assert_eq!(e.evaluate(&r, &[Value::String("BLUE".into())]), Ok(true),);
    }

    #[test]
    fn missing_param_is_error() {
        let e = parse("color = %0").unwrap();
        let r = row(&[("color", Value::String("BLUE".into()))]);
        assert_eq!(e.evaluate(&r, &[]), Err(EvalError::MissingParam(0)),);
    }

    #[test]
    fn unknown_field_is_error() {
        let e = parse("missing = 1").unwrap();
        let r = row(&[("x", Value::Int(1))]);
        assert!(matches!(
            e.evaluate(&r, &[]),
            Err(EvalError::UnknownField(_))
        ));
    }

    #[test]
    fn like_wildcards() {
        let e = parse("name LIKE 'foo%'").unwrap();
        let r_yes = row(&[("name", Value::String("foobar".into()))]);
        let r_no = row(&[("name", Value::String("barfoo".into()))]);
        assert_eq!(e.evaluate(&r_yes, &[]), Ok(true));
        assert_eq!(e.evaluate(&r_no, &[]), Ok(false));

        let single = parse("name LIKE 'a_c'").unwrap();
        let r_yes = row(&[("name", Value::String("abc".into()))]);
        let r_no = row(&[("name", Value::String("abbc".into()))]);
        assert_eq!(single.evaluate(&r_yes, &[]), Ok(true));
        assert_eq!(single.evaluate(&r_no, &[]), Ok(false));
    }

    #[test]
    fn like_on_non_string_rejected() {
        let e = parse("x LIKE 5").unwrap();
        let r = row(&[("x", Value::Int(5))]);
        assert!(matches!(
            e.evaluate(&r, &[]),
            Err(EvalError::TypeMismatch(_))
        ));
    }

    #[test]
    fn integers_above_two_to_the_53_compare_exactly() {
        let e = parse("x = %0").unwrap();
        let r = row(&[("x", Value::Int(9_007_199_254_740_993))]);
        let near = [Value::Int(9_007_199_254_740_992)];
        assert_eq!(e.evaluate(&r, &near), Ok(false));
        let same = [Value::Int(9_007_199_254_740_993)];
        assert_eq!(e.evaluate(&r, &same), Ok(true));
    }

    #[test]
    fn a_parameter_is_read_as_the_literal_it_holds() {
        assert_eq!(Value::from_parameter("42"), Value::Int(42));
        assert_eq!(Value::from_parameter("2.5"), Value::Float(2.5));
        assert_eq!(Value::from_parameter("TRUE"), Value::Bool(true));
        assert_eq!(Value::from_parameter("'RED'"), Value::String("RED".into()));
        assert_eq!(
            Value::from_parameter("plain"),
            Value::String("plain".into())
        );
        let e = parse("x > %0").unwrap();
        let r = row(&[("x", Value::Int(5))]);
        assert_eq!(e.evaluate(&r, &[Value::from_parameter("4")]), Ok(true));
    }

    #[test]
    fn an_expression_says_whether_it_reads_a_field() {
        assert!(parse("x > 1").unwrap().references_fields());
        assert!(
            parse("1 = 1 AND NOT (a.b BETWEEN 1 AND %0)")
                .unwrap()
                .references_fields()
        );
        assert!(!parse("%0 = 'on'").unwrap().references_fields());
    }

    // -- XTypes 1.3 §7.6.7 -------------------------------------------------

    /// §7.6.7.1: an element of a sequence by zero-based subscript, a map
    /// value by quoted key, a bitmask flag by quoted name.
    #[test]
    fn subscripts_reach_the_row_as_written() {
        struct Echo(&'static str, Value);
        impl RowAccess for Echo {
            fn get(&self, path: &str) -> Option<Value> {
                (path == self.0).then(|| self.1.clone())
            }
        }
        let by_index = parse("samples[0] > 10").unwrap();
        assert_eq!(
            by_index.evaluate(&Echo("samples[0]", Value::Int(11)), &[]),
            Ok(true)
        );
        let by_key = parse("limits['max'] = 5").unwrap();
        assert_eq!(
            by_key.evaluate(&Echo("limits['max']", Value::Int(5)), &[]),
            Ok(true)
        );
        let nested = parse("a.b[2].c['k'] = 'x'").unwrap();
        assert_eq!(
            nested.evaluate(&Echo("a.b[2].c['k']", Value::String("x".into())), &[]),
            Ok(true)
        );
    }

    /// §7.6.7.1: a bitmask flag is Boolean; comparisons with 1 and 0 are
    /// allowed.
    #[test]
    fn a_bitmask_flag_compares_with_true_and_with_one_and_zero() {
        let set = row(&[("m['READ']", Value::Bool(true))]);
        let clear = row(&[("m['READ']", Value::Bool(false))]);
        for (filter, on_set, on_clear) in [
            ("m['READ']  = TRUE", true, false),
            ("m['READ'] = 1", true, false),
            ("m['READ'] = 0", false, true),
            ("m['READ'] <> 0", true, false),
        ] {
            let expr = parse(filter).unwrap();
            assert_eq!(
                expr.evaluate(&set, &[]),
                Ok(on_set),
                "{filter} on a set flag"
            );
            assert_eq!(
                expr.evaluate(&clear, &[]),
                Ok(on_clear),
                "{filter} on a clear flag"
            );
        }
    }

    /// §7.6.7.2: an optional member with no value equals `null`; one with a
    /// value does not; no inequality against `null` is true.
    #[test]
    fn null_follows_the_rules_for_optional_members() {
        let absent = row(&[("opt", Value::Null)]);
        let present = row(&[("opt", Value::Int(3))]);
        for (filter, on_absent, on_present) in [
            ("opt = null", true, false),
            ("opt <> null", false, true),
            ("opt > null", false, false),
            ("opt < null", false, false),
            ("opt >= null", false, false),
            ("opt <= null", false, false),
            ("opt BETWEEN null AND 5", false, false),
            ("opt = 3", false, true),
        ] {
            let expr = parse(filter).unwrap();
            assert_eq!(
                expr.evaluate(&absent, &[]),
                Ok(on_absent),
                "{filter} absent"
            );
            assert_eq!(
                expr.evaluate(&present, &[]),
                Ok(on_present),
                "{filter} present"
            );
        }
        // The NULLVALUE token is `null`; a parameter spelled that way too.
        let by_param = parse("opt = %0").unwrap();
        assert_eq!(
            by_param.evaluate(&absent, &[Value::from_parameter("null")]),
            Ok(true)
        );
    }
}
