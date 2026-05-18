# RC1 Review — `zerodds-sql-filter`

> **Layer:** 4. **Reviewer:** Claude. **Public-Strategy:** 🌐 public.

## 1 Purpose

OMG DDS 1.4 §B.2.1 ContentFilteredTopic-Filter-Expression Parser + Evaluator. SQL-92-Subset mit `%N`-Parametern.

## 3 Content-Inventur

5 src-Files, **~960 LOC** (post-BETWEEN-add), 27+1 Tests grün.

### 3.4 Coherence-Audit

| Public-Item | Spec | External Refs | Klassifikation |
|---|---|---|---|
| `parse`, `Expr`, `CmpOp`, `Operand`, `Value` | §B.2.1 Syntax | `dcps` (ContentFilteredTopic), end-user | CONNECTED |
| `RowAccess`, `Expr::evaluate` | §B.2.1 Semantik | `dcps`, end-user | CONNECTED |
| `EvalError`, `ParseError` | crate-internal | alle Konsumenten | CONNECTED |

Ergebnis: **0 ❌-Klassen**.

## 6 Cleanup-Findings

- Forbidden-Token-Sweep: 0.
- Sprint-Marker pre: `WP 3.7b`, `MVP`, `3.7c`. Post: 0.
- No-op-Sweep: 0.
- SPDX in 5 src-Files post.
- **Spec-Gap geschlossen**: `BETWEEN`-Predicate (Spec §B.2.1 BetweenPredicate) war pre-RC1 nicht implementiert (Doc sagte "Nicht im MVP"). RC1 ergaenzt:
  - `Expr::Between { field, low, high, negated }`-AST-Variante.
  - `Token::Between` + Lexer-Keyword.
  - Parser: `field [NOT] BETWEEN low AND high` als Predicate-Form.
  - Evaluator: `cmp(field, Ge, low) && cmp(field, Le, high)` mit Negierung.
  - 3 neue Tests (`parse_between_predicate`, `parse_not_between_predicate`, `parse_between_with_params`).

`IN (...)` und `IS NULL` sind nicht in §B.2.1 — keine Spec-Pflicht, deferred als optionale Major-2.0-Erweiterung.

## 7 Cleanup-Actions

1. **F-SQL-FILTER-1** ✅ (Spec-Gap + Sprint-Marker): `BETWEEN`/`NOT BETWEEN` voll implementiert (AST + Lexer + Parser + Evaluator + 3 Tests). Sprint-Marker raus. lib.rs in Guardrails §1.2-Form mit Spec-Coverage-Sektion und expliziter Nicht-Ziele-Beschreibung.
2. SPDX in 5 src-Files.
3. Cargo.toml-Metadata + `publish=true`.
4. README + CHANGELOG.

## 10 Tests + Lints + Doc-Build

```
cargo test           ✅ 27 + 1 doc passed
cargo clippy --tests -- -D warnings  ✅
cargo fmt -- --check ✅
zerodds-lint check   ✅
```

## 11 RC1-DoD

Alle 13 Punkte; **No-op 0 Treffer**; **Spec-Coverage §B.2.1 voll** (post-BETWEEN-add).

## 12 Sign-off

`1.0.0-rc.1`. Reviewer Claude.
