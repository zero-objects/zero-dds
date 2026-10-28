# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `parse_topic_expression` and `TopicExpression`, `Select`, `SubjectField`:
  the `SELECT … FROM … NATURAL JOIN … WHERE …` of DDS 1.4 Annex B.2 (the
  three spellings of the join, parenthesized join items, `AS` and the
  juxtaposed alias, quoted topic names, an optional closing `;`). Anything
  else is a `ParseError`.
- `parse_query_expression` and `QueryExpression`: a condition and `ORDER BY`
  with FIELDNAMEs (Annex B.2).
- Integers take a plus sign and the form `0x1F` (Annex B.2 INTEGERVALUE); a
  malformed hexadecimal is an error.
- Subscripts on a field reference (DDS-XTypes 1.3 §7.6.7.1): `a[0]` for an
  array or sequence element, `a['key']` for a map value or a bitmask flag.
  `RowAccess::get` receives the reference as written; `split_path` and
  `PathStep` take it apart.
- The `null` literal and `Value::Null` (§7.6.7.2, §7.6.7.3): an optional
  member without a value equals `null`, a member with a value does not, and
  no ordering comparison with `null` is true.
- A bitmask flag compares with `TRUE`, `FALSE`, `1` and `0` (§7.6.7.1).

### Fixed

- A string literal is read by characters: a character outside ASCII was
  taken byte by byte, so `'ü'` was two characters and matched no string. A
  new-line in a string is refused (Annex B.2 STRING).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-sql-filter` crate.

### Spec references

- **OMG DDS 1.4** §B.2.1 (filter-expression syntax).

### Public API

- `parse(input) -> Result<Expr, ParseError>`.
- `Expr::{And, Or, Not, Cmp, Between}`.
- `CmpOp::{Eq, Neq, Lt, Le, Gt, Ge, Like}`.
- `Operand::{Literal, Field, Param}`.
- `Value::{String, Int, Float, Bool}`.
- `RowAccess`-Trait, `Expr::evaluate`.
- `EvalError::{UnknownField, MissingParam, TypeMismatch}`.
- `ParseError`.

### Implementation

Recursive-descent parser with precedence climbing: `or_expr` < `and_expr` < `not_expr` < `cmp` < `atom`. Tokenizer with case-insensitive keywords and SQL-92 `''` escaping for string literals.

`BETWEEN low AND high` is implemented as the `Expr::Between` variant (spec §B.2.1 BetweenPredicate). `NOT BETWEEN` is recognized before the BETWEEN token and sets `negated=true`. The evaluator delegates to `cmp(field, Ge, low) && cmp(field, Le, high)` and negates when `negated=true`.

LIKE match via classic DP with `%` (zero or more) and `_` (exactly one character). Backslash escaping is not implemented — spec §B.2.1 does not require it.

`forbid(unsafe_code)`. no_std + alloc compatible.

### Architecture

- **Layer:** 4 (core services).
- **Dependencies (in):** none. Pure Rust + `alloc`.
- **Dependents (out):** `dcps` (`ContentFilteredTopic` filter).
- **Feature flags:** `std` (default), `alloc`.

### Stability

Public API + filter-expression grammar RC1-stable. `IN (...)` and `IS NULL` are not in §B.2.1 — if needed, an additive extension as a major-2.0 hook.
