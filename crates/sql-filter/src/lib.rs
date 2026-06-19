// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG DDS content-filter-expression parser + evaluator.
//!
//! Safety classification: **SAFE** (a pure parser + evaluator, no data
//! flows from external networks without caller mediation).
//!
//! Spec: OMG DDS 1.4 §B.2.1 "Filter expressions". The syntax is a
//! SQL-92 subset, extended with `%N` parameter placeholders.
//!
//! # Current scope
//!
//! * Literals: string (`'...'`), integer (`i64`), float (`f64`),
//!   boolean (`TRUE`/`FALSE`).
//! * Identifiers: dotted (`a.b.c`) — for nested field access.
//! * Parameter placeholders: `%0`, `%1`, …
//! * Comparison ops: `=`, `!=`, `<>`, `<`, `<=`, `>`, `>=`, `LIKE`.
//! * Boolean ops: `AND`, `OR`, `NOT`.
//! * Parentheses.
//! * `LIKE` wildcards: `%` (several characters), `_` (one character).
//!
//! Not in the MVP: `BETWEEN ... AND`, `IN (...)`, `IS NULL`. Those
//! follow in 3.7c.
//!
//! # Architecture
//!
//! 1. `lexer`: tokenizer. All keywords case-insensitive, string
//!    literals `'...'` with `''` escape.
//! 2. `parser`: recursive descent with precedence climbing —
//!    `OR` < `AND` < `NOT` < comparison < atom.
//! 3. `ast`: data types for expressions + `Value`.
//! 4. `evaluator`: `Expr::evaluate(row, params)` → `bool`; `row`
//!    implements `RowAccess` (field lookup by name).
//!
//! # Usage
//!
//! ```
//! use zerodds_sql_filter::{parse, Value, RowAccess};
//! use std::collections::HashMap;
//!
//! struct MapRow(HashMap<String, Value>);
//! impl RowAccess for MapRow {
//!     fn get(&self, path: &str) -> Option<Value> {
//!         self.0.get(path).cloned()
//!     }
//! }
//!
//! let expr = parse("color = %0 AND x > 10").expect("parse");
//! let row = MapRow(HashMap::from([
//!     ("color".into(), Value::String("RED".into())),
//!     ("x".into(),     Value::Int(42)),
//! ]));
//! let params = [Value::String("RED".into())];
//! assert_eq!(expr.evaluate(&row, &params), Ok(true));
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod ast;
mod evaluator;
mod lexer;
mod parser;

pub use ast::{Expr, Value};
pub use evaluator::{EvalError, RowAccess};
pub use parser::{ParseError, parse};
