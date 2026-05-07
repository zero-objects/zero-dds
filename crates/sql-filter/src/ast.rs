// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AST-Datentypen für Content-Filter-Expressions.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// Ein evaluierbarer skalarer Wert. OMG erlaubt mehr Typen (char, etc.)
/// — wir mappen hier bewusst eng auf die häufigsten Fälle.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// String-Literal oder String-Feld.
    String(String),
    /// Signed 64-bit Integer (mappt Rust `i8..i64` + `u8..u32`).
    Int(i64),
    /// Doppelte Genauigkeit (für `f32` + `f64`-Felder).
    Float(f64),
    /// Boolean.
    Bool(bool),
}

/// Vergleichs-Operatoren.
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
    /// `LIKE` — Wildcard-Vergleich (nur für Strings).
    Like,
}

/// Ein Filter-Ausdruck. Rekursiver Baum, erzeugt vom [`crate::parse`].
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Kombinierter `AND`-Ausdruck.
    And(Box<Expr>, Box<Expr>),
    /// Kombinierter `OR`-Ausdruck.
    Or(Box<Expr>, Box<Expr>),
    /// `NOT`-Negation.
    Not(Box<Expr>),
    /// Vergleich zwischen zwei Operanden.
    Cmp {
        /// Linker Operand.
        lhs: Operand,
        /// Operator.
        op: CmpOp,
        /// Rechter Operand.
        rhs: Operand,
    },
    /// `field BETWEEN low AND high` (Spec §B.2.1 BetweenPredicate).
    /// Equivalent zu `field >= low AND field <= high`. `negated = true`
    /// fuer `NOT BETWEEN`.
    Between {
        /// Feld-Operand (links vom BETWEEN).
        field: Operand,
        /// Untere Grenze (inklusive).
        low: Operand,
        /// Obere Grenze (inklusive).
        high: Operand,
        /// `NOT BETWEEN`?
        negated: bool,
    },
}

/// Ein Vergleichs-Operand: Literal, Feld-Referenz oder Parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    /// Konstantes Literal.
    Literal(Value),
    /// Feldzugriff, ggf. dotted (`a.b.c`).
    Field(String),
    /// Positional Parameter `%N`.
    Param(u32),
}

impl Expr {
    /// Rekursive Sub-Ausdruck-Anzahl (nützlich für Tests / Metriken).
    #[must_use]
    pub fn node_count(&self) -> usize {
        match self {
            Self::And(a, b) | Self::Or(a, b) => 1 + a.node_count() + b.node_count(),
            Self::Not(inner) => 1 + inner.node_count(),
            Self::Cmp { .. } => 1,
            Self::Between { .. } => 1,
        }
    }

    /// Sammelt alle Parameter-Indices, die in der Expression vorkommen.
    #[must_use]
    pub fn collect_param_indices(&self) -> Vec<u32> {
        /// zerodds-lint: recursion-depth = parse-tree-depth (≤ 64 durch
        /// Parser-Input-Caps; keine unbounded Rekursion moeglich).
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
