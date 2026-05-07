# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-sql-filter`-Crate.

### Spec-Referenzen

- **OMG DDS 1.4** §B.2.1 (Filter-Expression-Syntax).

### Public-API

- `parse(input) -> Result<Expr, ParseError>`.
- `Expr::{And, Or, Not, Cmp, Between}`.
- `CmpOp::{Eq, Neq, Lt, Le, Gt, Ge, Like}`.
- `Operand::{Literal, Field, Param}`.
- `Value::{String, Int, Float, Bool}`.
- `RowAccess`-Trait, `Expr::evaluate`.
- `EvalError::{UnknownField, MissingParam, TypeMismatch}`.
- `ParseError`.

### Implementierung

Recursive-Descent-Parser mit Precedence-Klettern: `or_expr` < `and_expr` < `not_expr` < `cmp` < `atom`. Tokenizer mit case-insensitive Keywords und SQL-92-`''`-Escape fuer String-Literale.

`BETWEEN low AND high` ist als `Expr::Between`-Variante implementiert (Spec §B.2.1 BetweenPredicate). `NOT BETWEEN` wird vor dem BETWEEN-Token erkannt und setzt `negated=true`. Evaluator delegiert an `cmp(field, Ge, low) && cmp(field, Le, high)` und negiert bei `negated=true`.

LIKE-Match via klassisches DP mit `%` (null-oder-mehr) und `_` (genau ein Zeichen). Backslash-Escape ist nicht implementiert — Spec §B.2.1 verlangt es nicht.

`forbid(unsafe_code)`. no_std + alloc kompatibel.

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** keine. Pure-Rust + `alloc`.
- **Dependents (out):** `dcps` (`ContentFilteredTopic`-Filter).
- **Feature-Flags:** `std` (default), `alloc`.

### Stabilitaet

Public-API + Filter-Expression-Grammar RC1-stabil. `IN (...)` und `IS NULL` sind nicht in §B.2.1 — falls Bedarf, additive Erweiterung als Major-2.0-Hook.
