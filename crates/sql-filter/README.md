# `zerodds-sql-filter`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-sql-filter/badge.svg)](https://docs.rs/zerodds-sql-filter)

OMG DDS 1.4 §B.2.1 ContentFilteredTopic-Filter-Expression Parser +
Evaluator fuer den [ZeroDDS](https://zerodds.org)-Stack. Pure-Rust +
`alloc`. Safety classification: **SAFE**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG DDS 1.4 | §B.2.1 (Filter-Expression-Syntax) |

## Was ist drin

- `parse(input)`, `Expr` (`And/Or/Not/Cmp/Between`), `CmpOp`, `Operand`, `Value`.
- `RowAccess`-Trait + `Expr::evaluate(row, params)`.
- `ParseError`, `EvalError`.

## Spec-Coverage (§B.2.1 voll)

- Literale: String/Int/Float/Bool.
- Identifier: dotted (`a.b.c`).
- Parameter `%0`, `%1`, …
- Vergleichs-Ops: `=`, `!=`/`<>`, `<`, `<=`, `>`, `>=`, `LIKE`.
- Boolean: `AND`, `OR`, `NOT`.
- `BETWEEN low AND high` + `NOT BETWEEN low AND high`.
- Klammern.
- LIKE-Wildcards: `%`, `_`.

## Schichten-Position

Layer 4. Pure-Rust + `alloc`, **keine** ZeroDDS-Crate-Deps.

## Quickstart

```rust
use zerodds_sql_filter::{parse, Value, RowAccess};
use std::collections::HashMap;

struct MapRow(HashMap<String, Value>);
impl RowAccess for MapRow {
    fn get(&self, path: &str) -> Option<Value> { self.0.get(path).cloned() }
}

let expr = parse("color = %0 AND x BETWEEN 0 AND 100").unwrap();
let row = MapRow(HashMap::from([
    ("color".into(), Value::String("RED".into())),
    ("x".into(),     Value::Int(42)),
]));
assert_eq!(expr.evaluate(&row, &[Value::String("RED".into())]), Ok(true));
```

## Stabilitaet

`1.0.0-rc.1`. Public-API + Filter-Expression-Grammar RC1-stabil.

## Tests

```bash
cargo test -p zerodds-sql-filter
```

27 + 1 Doc-Test grün.

## Lizenz

Apache-2.0.
