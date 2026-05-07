// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Expression-Evaluator.
//!
//! Die Evaluation ist strikt — Type-Mismatch (z.B. String < Int)
//! liefert `EvalError::TypeMismatch`. Der Caller entscheidet, ob er
//! das als `filter denies` oder als `filter error` behandelt.

use alloc::string::String;

use crate::ast::{CmpOp, Expr, Operand, Value};

/// Row-Abstraktion: ein Zugriff auf die Felder einer Stichprobe.
///
/// Implementierer mappen dotted-Pfade (`a.b.c`) auf die passenden
/// Werte. Für einfache flache Structs reicht ein `HashMap<String,
/// Value>`-Lookup.
pub trait RowAccess {
    /// Liefert den Wert des Feldpfads, wenn vorhanden. `None` = kein
    /// Feld mit dem Namen.
    fn get(&self, path: &str) -> Option<Value>;
}

/// Fehler bei der Evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// Feld nicht gefunden.
    UnknownField(String),
    /// Parameter-Index ausserhalb des uebergebenen Slice.
    MissingParam(u32),
    /// Operator nicht kompatibel mit den Operand-Typen.
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
    /// Werte die Expression gegen einen Row + Parameter-Slice aus.
    ///
    /// # Errors
    /// Siehe [`EvalError`].
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
    // Numerische Promotion fuer Int/Float-Vergleiche.
    if let (Some(l), Some(r)) = (as_f64(lhs), as_f64(rhs)) {
        return Ok(match op {
            CmpOp::Eq => (l - r).abs() < f64::EPSILON,
            CmpOp::Neq => (l - r).abs() >= f64::EPSILON,
            CmpOp::Lt => l < r,
            CmpOp::Le => l <= r,
            CmpOp::Gt => l > r,
            CmpOp::Ge => l >= r,
            CmpOp::Like => {
                return Err(EvalError::TypeMismatch("LIKE nur für String".into()));
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

/// SQL-92 LIKE-Match mit `%` (null-oder-mehr) und `_` (genau ein Zeichen).
/// Backslash-Escape ist nicht implementiert — Spec §B.2.1 verlangt es
/// nicht; %/_ in Daten muss der Caller per Doppel-Encoding einbringen.
fn like_match(s: &str, pat: &str) -> bool {
    // Klassisches DP: m[i][j] = s[..i] matcht pat[..j]
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
        // Int auf einer Seite, Float auf der anderen — promotet zu f64.
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
}
