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
    /// Kombinierter `OR`-Ausdruck.
    Or(Box<Expr>, Box<Expr>),
    /// `NOT`-Negation.
    Not(Box<Expr>),
    /// Comparison between two operands.
    Cmp {
        /// Linker Operand.
        lhs: Operand,
        /// Operator.
        op: CmpOp,
        /// Rechter Operand.
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
