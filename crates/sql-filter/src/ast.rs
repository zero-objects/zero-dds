// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AST data types for content-filter expressions.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// An evaluable scalar value. OMG allows more types (char, etc.) — we
/// deliberately map narrowly to the most common cases here.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// String literal or string field.
    String(String),
    /// Signed 64-bit integer (maps Rust `i8..i64` + `u8..u32`).
    Int(i64),
    /// Double precision (for `f32` + `f64` fields).
    Float(f64),
    /// Boolean.
    Bool(bool),
    /// `null` (XTypes 1.3 §7.6.7.2): the value of an optional member that
    /// takes no value, and the `NULLVALUE` literal.
    Null,
}

impl Value {
    /// A filter parameter (`%N`) as DDS 1.4 Annex B passes it: a string
    /// holding a literal. `TRUE`/`FALSE` (any case) is a boolean, an integer
    /// or a float is numeric, `'text'` is the text without its quotes, and
    /// anything else is taken as text as written.
    #[must_use]
    pub fn from_parameter(s: &str) -> Self {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("TRUE") {
            return Self::Bool(true);
        }
        if trimmed.eq_ignore_ascii_case("FALSE") {
            return Self::Bool(false);
        }
        // §7.6.7.3: the NULLVALUE token is always `null`.
        if trimmed == "null" {
            return Self::Null;
        }
        if let Ok(i) = trimmed.parse::<i64>() {
            return Self::Int(i);
        }
        if let Ok(f) = trimmed.parse::<f64>() {
            return Self::Float(f);
        }
        if trimmed.len() >= 2 && trimmed.starts_with('\'') && trimmed.ends_with('\'') {
            return Self::String(trimmed[1..trimmed.len() - 1].into());
        }
        Self::String(trimmed.into())
    }
}

impl Value {
    /// A total order for `ORDER BY` (Annex B.2 `QueryExpression`): `null`
    /// first, then booleans (`FALSE` before `TRUE`), then numbers (an integer
    /// and a float by their value), then strings by their characters. Values
    /// of one field are of one kind, so the order between kinds only keeps
    /// the order total.
    #[must_use]
    pub fn order(&self, other: &Self) -> core::cmp::Ordering {
        fn rank(v: &Value) -> u8 {
            match v {
                Value::Null => 0,
                Value::Bool(_) => 1,
                Value::Int(_) | Value::Float(_) => 2,
                Value::String(_) => 3,
            }
        }
        match (self, other) {
            (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
            (Self::Int(a), Self::Int(b)) => a.cmp(b),
            (Self::Float(a), Self::Float(b)) => a.total_cmp(b),
            #[allow(clippy::cast_precision_loss)]
            (Self::Int(a), Self::Float(b)) => (*a as f64).total_cmp(b),
            #[allow(clippy::cast_precision_loss)]
            (Self::Float(a), Self::Int(b)) => a.total_cmp(&(*b as f64)),
            (Self::String(a), Self::String(b)) => a.cmp(b),
            _ => rank(self).cmp(&rank(other)),
        }
    }
}

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    /// `=`
    Eq,
    /// `!=` / `<>`
    Neq,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `LIKE` — wildcard comparison (only for strings).
    Like,
}

/// A filter expression. A recursive tree, produced by [`crate::parse`].
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Combined `AND` expression.
    And(Box<Expr>, Box<Expr>),
    /// Combined `OR` expression.
    Or(Box<Expr>, Box<Expr>),
    /// `NOT`-Negation.
    Not(Box<Expr>),
    /// Comparison between two operands.
    Cmp {
        /// Left operand.
        lhs: Operand,
        /// Operator.
        op: CmpOp,
        /// Right operand.
        rhs: Operand,
    },
    /// `field BETWEEN low AND high` (spec §B.2.1 BetweenPredicate).
    /// Equivalent to `field >= low AND field <= high`. `negated = true`
    /// for `NOT BETWEEN`.
    Between {
        /// Field operand (left of BETWEEN).
        field: Operand,
        /// Lower bound (inclusive).
        low: Operand,
        /// Upper bound (inclusive).
        high: Operand,
        /// `NOT BETWEEN`?
        negated: bool,
    },
}

/// A comparison operand: literal, field reference or parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    /// Constant literal.
    Literal(Value),
    /// Field access, possibly dotted (`a.b.c`).
    Field(String),
    /// Positional parameter `%N`.
    Param(u32),
}

impl Expr {
    /// Recursive sub-expression count (useful for tests / metrics).
    #[must_use]
    pub fn node_count(&self) -> usize {
        match self {
            Self::And(a, b) | Self::Or(a, b) => 1 + a.node_count() + b.node_count(),
            Self::Not(inner) => 1 + inner.node_count(),
            Self::Cmp { .. } => 1,
            Self::Between { .. } => 1,
        }
    }

    /// Whether the expression reads a field of the sample; one that does
    /// not compares only literals and parameters.
    #[must_use]
    pub fn references_fields(&self) -> bool {
        /// zerodds-lint: recursion-depth = parse-tree-depth (≤ 64 via
        /// parser input caps; no unbounded recursion possible).
        fn walk(e: &Expr) -> bool {
            match e {
                Expr::And(a, b) | Expr::Or(a, b) => walk(a) || walk(b),
                Expr::Not(inner) => walk(inner),
                Expr::Cmp { lhs, rhs, .. } => {
                    matches!(lhs, Operand::Field(_)) || matches!(rhs, Operand::Field(_))
                }
                Expr::Between {
                    field, low, high, ..
                } => [field, low, high]
                    .iter()
                    .any(|op| matches!(op, Operand::Field(_))),
            }
        }
        walk(self)
    }

    /// The fields the expression reads, as written, in the order they occur
    /// and each time they occur.
    #[must_use]
    pub fn collect_fields(&self) -> Vec<String> {
        /// zerodds-lint: recursion-depth = parse-tree-depth (≤ 64 via
        /// parser input caps; no unbounded recursion possible).
        fn walk(e: &Expr, out: &mut Vec<String>) {
            let mut operand = |op: &Operand| {
                if let Operand::Field(name) = op {
                    out.push(name.clone());
                }
            };
            match e {
                Expr::And(a, b) | Expr::Or(a, b) => {
                    walk(a, out);
                    walk(b, out);
                }
                Expr::Not(inner) => walk(inner, out),
                Expr::Cmp { lhs, rhs, .. } => {
                    operand(lhs);
                    operand(rhs);
                }
                Expr::Between {
                    field, low, high, ..
                } => {
                    operand(field);
                    operand(low);
                    operand(high);
                }
            }
        }
        let mut out = Vec::new();
        walk(self, &mut out);
        out
    }

    /// Collects all parameter indices that occur in the expression.
    #[must_use]
    pub fn collect_param_indices(&self) -> Vec<u32> {
        /// zerodds-lint: recursion-depth = parse-tree-depth (≤ 64 via
        /// parser input caps; no unbounded recursion possible).
        fn walk(e: &Expr, out: &mut Vec<u32>) {
            match e {
                Expr::And(a, b) | Expr::Or(a, b) => {
                    walk(a, out);
                    walk(b, out);
                }
                Expr::Not(inner) => walk(inner, out),
                Expr::Cmp { lhs, rhs, .. } => {
                    if let Operand::Param(i) = lhs {
                        out.push(*i);
                    }
                    if let Operand::Param(i) = rhs {
                        out.push(*i);
                    }
                }
                Expr::Between {
                    field, low, high, ..
                } => {
                    for op in [field, low, high] {
                        if let Operand::Param(i) = op {
                            out.push(*i);
                        }
                    }
                }
            }
        }
        let mut out = Vec::new();
        walk(self, &mut out);
        out
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;
    use core::cmp::Ordering;

    #[test]
    fn values_order_by_kind_and_value() {
        assert_eq!(Value::Int(1).order(&Value::Int(2)), Ordering::Less);
        assert_eq!(Value::Int(2).order(&Value::Float(1.5)), Ordering::Greater);
        assert_eq!(Value::Float(1.0).order(&Value::Int(1)), Ordering::Equal);
        assert_eq!(
            Value::String("a".into()).order(&Value::String("b".into())),
            Ordering::Less
        );
        assert_eq!(Value::Bool(false).order(&Value::Bool(true)), Ordering::Less);
        assert_eq!(Value::Null.order(&Value::Int(0)), Ordering::Less);
        assert_eq!(
            Value::Int(0).order(&Value::String("".into())),
            Ordering::Less
        );
    }
}
