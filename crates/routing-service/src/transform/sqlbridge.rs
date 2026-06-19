// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bridges decoded samples to the `zerodds-sql-filter` evaluator.

use zerodds_sql_filter::{Expr, RowAccess, Value, parse};

use super::codec::DynValue;
use super::shape::TypeShape;
use crate::error::RoutingError;

/// Maps a decoded scalar to a SQL filter [`Value`].
pub fn to_sql_value(v: &DynValue) -> Value {
    match v {
        DynValue::Bool(b) => Value::Bool(*b),
        DynValue::U8(x) => Value::Int(i64::from(*x)),
        DynValue::I8(x) => Value::Int(i64::from(*x)),
        DynValue::U16(x) => Value::Int(i64::from(*x)),
        DynValue::I16(x) => Value::Int(i64::from(*x)),
        DynValue::U32(x) => Value::Int(i64::from(*x)),
        DynValue::I32(x) => Value::Int(i64::from(*x)),
        // u64 above i64::MAX saturates — filters comparing such huge counters
        // are vanishingly rare and a saturating map is well-defined.
        DynValue::U64(x) => Value::Int(i64::try_from(*x).unwrap_or(i64::MAX)),
        DynValue::I64(x) => Value::Int(*x),
        DynValue::F32(x) => Value::Float(f64::from(*x)),
        DynValue::F64(x) => Value::Float(*x),
        DynValue::Str(s) => Value::String(s.clone()),
    }
}

/// A decoded row exposing members by name for [`RowAccess`].
pub struct DecodedRow<'a> {
    shape: &'a TypeShape,
    values: &'a [DynValue],
}

impl<'a> DecodedRow<'a> {
    /// Wraps a decoded sample.
    #[must_use]
    pub fn new(shape: &'a TypeShape, values: &'a [DynValue]) -> Self {
        Self { shape, values }
    }
}

impl RowAccess for DecodedRow<'_> {
    fn get(&self, path: &str) -> Option<Value> {
        let i = self.shape.index_of(path)?;
        self.values.get(i).map(to_sql_value)
    }
}

/// A parsed filter expression plus its bound parameters.
pub struct CompiledFilter {
    expr: Expr,
    params: Vec<Value>,
}

impl CompiledFilter {
    /// Parses `expression` and binds `parameters` (each parsed as int, then
    /// float, then bool, else string).
    ///
    /// # Errors
    /// [`RoutingError::Filter`] on a parse error.
    pub fn build(
        route: &str,
        expression: &str,
        parameters: &[String],
    ) -> crate::error::Result<Self> {
        let expr = parse(expression).map_err(|e| RoutingError::Filter {
            route: route.to_string(),
            reason: format!("{e:?}"),
        })?;
        let params = parameters.iter().map(|p| parse_param(p)).collect();
        Ok(Self { expr, params })
    }

    /// Evaluates the filter against a decoded row. On an evaluation error
    /// (e.g. unknown field), the sample is conservatively **dropped**
    /// (`false`).
    #[must_use]
    pub fn matches(&self, row: &DecodedRow) -> bool {
        self.expr.evaluate(row, &self.params).unwrap_or(false)
    }
}

fn parse_param(s: &str) -> Value {
    if let Ok(i) = s.parse::<i64>() {
        return Value::Int(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::Float(f);
    }
    match s {
        "true" | "TRUE" => return Value::Bool(true),
        "false" | "FALSE" => return Value::Bool(false),
        _ => {}
    }
    // Strip surrounding single quotes if present.
    let trimmed = s
        .strip_prefix('\'')
        .and_then(|t| t.strip_suffix('\''))
        .unwrap_or(s);
    Value::String(trimmed.to_string())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::transform::shape::{Member, ScalarKind};

    fn shape() -> TypeShape {
        TypeShape {
            name: "T".into(),
            members: vec![
                Member {
                    name: "temp".into(),
                    kind: ScalarKind::I32,
                },
                Member {
                    name: "zone".into(),
                    kind: ScalarKind::String,
                },
            ],
            appendable: false,
        }
    }

    #[test]
    fn filter_matches() {
        let sh = shape();
        let f = CompiledFilter::build("r", "temp > 50 AND zone = 'A'", &[]).unwrap();
        let pass = vec![DynValue::I32(60), DynValue::Str("A".into())];
        let fail_temp = vec![DynValue::I32(40), DynValue::Str("A".into())];
        let fail_zone = vec![DynValue::I32(60), DynValue::Str("B".into())];
        assert!(f.matches(&DecodedRow::new(&sh, &pass)));
        assert!(!f.matches(&DecodedRow::new(&sh, &fail_temp)));
        assert!(!f.matches(&DecodedRow::new(&sh, &fail_zone)));
    }

    #[test]
    fn filter_with_param() {
        let sh = shape();
        let f = CompiledFilter::build("r", "temp > %0", &["50".into()]).unwrap();
        assert!(f.matches(&DecodedRow::new(
            &sh,
            &[DynValue::I32(60), DynValue::Str("A".into())]
        )));
        assert!(!f.matches(&DecodedRow::new(
            &sh,
            &[DynValue::I32(10), DynValue::Str("A".into())]
        )));
    }
}
