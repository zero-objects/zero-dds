// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Typed property values (CosTrading `PropertyValue` as an `any` subset).

use alloc::string::String;
use core::cmp::Ordering;

/// A typed property value (or constraint literal).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Integer (`long`/`long long`).
    Int(i64),
    /// Floating point (`float`/`double`).
    Float(f64),
    /// String.
    Str(String),
    /// Boolean.
    Bool(bool),
}

impl Value {
    /// Ordering comparison of two values, where comparable:
    /// * Int/Float are compared numerically (mixed → float promotion),
    /// * Str lexicographically, Bool as `false < true`,
    /// * incompatible types → `None`.
    #[must_use]
    pub fn compare(&self, other: &Value) -> Option<Ordering> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            #[allow(clippy::cast_precision_loss)]
            (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
            #[allow(clippy::cast_precision_loss)]
            (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
            (Value::Str(a), Value::Str(b)) => Some(a.cmp(b)),
            (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }

    /// Numeric value (for `min`/`max` preferences), if Int/Float.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            #[allow(clippy::cast_precision_loss)]
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }
}
