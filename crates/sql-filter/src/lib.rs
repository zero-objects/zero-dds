// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG DDS Content-Filter-Expression-Parser + Evaluator.
//!
//! Safety classification: **SAFE** (reiner Parser + Evaluator, keine
//! Datenflüsse aus externen Netzen ohne Caller-Vermittlung).
//!
//! Spec: OMG DDS 1.4 §B.2.1 "Filter expressions". Die Syntax ist eine
//! SQL-92-Untermenge, erweitert um `%N`-Parameter-Placeholder.
//!
//! # Aktueller Scope
//!
//! * Literale: String (`'...'`), Integer (`i64`), Float (`f64`),
//!   Boolean (`TRUE`/`FALSE`).
//! * Identifier: dotted (`a.b.c`) — für nested Field-Access.
//! * Parameter-Placeholder: `%0`, `%1`, …
//! * Vergleichs-Ops: `=`, `!=`, `<>`, `<`, `<=`, `>`, `>=`, `LIKE`.
//! * Boolean-Ops: `AND`, `OR`, `NOT`.
//! * Klammern.
//! * `LIKE`-Wildcards: `%` (mehrere Zeichen), `_` (ein Zeichen).
//!
//! Nicht im MVP: `BETWEEN ... AND`, `IN (...)`, `IS NULL`. Die folgen
//! in 3.7c.
//!
//! # Architektur
//!
//! 1. `lexer`: Tokenizer. Alle Keywords case-insensitive, String-
//!    Literale `'...'` mit `''`-Escape.
//! 2. `parser`: Recursive-Descent mit Precedence-Klettern —
//!    `OR` < `AND` < `NOT` < vergleich < atom.
//! 3. `ast`: Datentypen fuer Expressions + `Value`.
//! 4. `evaluator`: `Expr::evaluate(row, params)` → `bool`; `row`
//!    implementiert `RowAccess` (Field-Lookup per Name).
//!
//! # Verwendung
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
