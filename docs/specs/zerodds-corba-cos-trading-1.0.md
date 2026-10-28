# `zerodds-corba-cos-trading` v1.0 — CosTrading: service discovery via a constraint language

ZeroDDS vendor spec. Implemented in `crates/corba-cos-trading`. Authored in the style
of the OMG CORBA spec (numbered clauses, RFC-2119 keywords, conformance profile).
The **service semantics** are OMG-normative (OMG Trading Object Service); this spec
standardizes the **ZeroDDS-specific Rust PSM** and the supported subset of the
**constraint language** — the OMG has not standardized a Rust language mapping.

## Motivation

The **Trading Object Service** is CORBA's service discovery by properties:
providers export typed *offers*, consumers search for them with boolean
**constraint expressions** over the properties (`"Speed > 100 and Color == 'red'"`).
This is a self-contained OMG profile and fits the "optional profiles as
differentiation" strategy. `zerodds-corba-cos-trading` provides register + lookup +
a complete constraint engine in `no_std + alloc`, `forbid(unsafe_code)`.

## Goals

- **Typed offers**: service type + `Value` properties + object reference.
- **Constraint language**: tokenizer + recursive-descent parser + evaluator over
  a well-defined grammar subset (§2).
- **Register + lookup**: `export`/`withdraw` + `query` with constraint + preferences
  + `how_many` limit.
- **Spec-faithful error semantics**: missing property → the comparison does not match.

## Non-goals

- **Federated traders** (trader-to-trader links, OMG `Link` interface) — v1.0 is
  a single in-memory trader.
- **Dynamic properties** (property values supplied by a callback) — v1.0 carries
  static `Value` properties.
- **Service type hierarchies** (subtype matching) — v1.0 matches the service type
  exactly.

## §1 Data model

### §1.1 `Value`

A typed property value (CosTrading `PropertyValue` as an `any` subset):

```rust
pub enum Value { Int(i64), Float(f64), Str(String), Bool(bool) }
```

`Value::compare` MUST compare Int/Float numerically (mixed → float
promotion), Str lexicographically, Bool as `false < true`; incompatible types →
no comparison.

### §1.2 `Offer`

```rust
pub struct Offer {
    pub service_type: String,
    pub properties: BTreeMap<String, Value>,
    pub ior: Vec<u8>,   // object reference (opaque IOR bytes)
}
```

## §2 Constraint language

### §2.1 Grammar

The supported subset of the OMG Trading Constraint Language (RFC-2119:
a conformant evaluator MUST accept exactly these productions):

```ebnf
expr       ::= or_expr
or_expr    ::= and_expr ('or' and_expr)*
and_expr   ::= not_expr ('and' not_expr)*
not_expr   ::= 'not' not_expr | comparison
comparison ::= primary (('==' | '!=' | '<' | '<=' | '>' | '>=') primary)?
primary    ::= '(' expr ')' | 'exist' ident | ident | literal
literal    ::= integer | float | "'" string "'" | TRUE | FALSE
```

`and` binds more tightly than `or`. `not` is prefix. A bare property as an
expression (`Available`) matches if it is `Bool(true)`. An empty constraint
always matches (`TRUE`).

### §2.2 Evaluation

- Comparison: both operands must be resolvable; if a property is missing, the
  comparison MUST return `false` (OMG semantics). Incompatible types → `false`.
- `exist <prop>` → whether the property is present in the map.
- Keyword comparisons are case-insensitive (`and`/`AND`, `true`/`TRUE`).

### §2.3 Errors

`Constraint::parse` MUST return `ConstraintError`
(`UnexpectedToken`/`UnexpectedEnd`/`BadNumber`/`UnterminatedString`/`BadChar`)
on lexer/parser errors — e.g. `"Speed >"` (missing operand), `"Speed = 5"` (`=` instead of `==`),
`"Color == 'red"` (unterminated), `"(Speed > 1"` (missing `)`).

## §3 Trader

### §3.1 Register + lookup

```rust
impl Trader {
    pub fn export(&mut self, offer: Offer) -> OfferId;   // Register::export
    pub fn withdraw(&mut self, id: OfferId) -> bool;     // Register::withdraw
    pub fn query(&self, service_type: &str, constraint: &str,
                 preference: &Preference, how_many: usize)
        -> Result<Vec<(OfferId, &Offer)>, ConstraintError>; // Lookup::query
}
```

`query` MUST filter offers by `service_type` (exact) AND `constraint` match,
sort by `preference` and limit to `how_many` (`0` = unlimited).

### §3.2 Preferences

`Preference` is `First` (export order), `Max(prop)` (descending by
numeric property) or `Min(prop)` (ascending). Non-numeric/missing
properties sort to the end.

## §4 Conformance

A **Trading-conformant** ZeroDDS module:

1. accepts the constraint grammar from §2.1 and evaluates it according to §2.2,
2. returns `false` for a missing property in a comparison,
3. filters/sorts/limits `query` according to §3.1/§3.2,
4. reports constraint syntax errors as `ConstraintError` (§2.3).

## §5 Implementation mapping

| Spec | Code |
|---|---|
| §1.1 `Value` | `corba-cos-trading/src/property.rs` |
| §2 Constraint language | `corba-cos-trading/src/constraint.rs` — tokenizer, parser, evaluator |
| §1.2/§3 Trader | `corba-cos-trading/src/trader.rs` — `Offer`, `Trader`, `Preference` |

## §6 Tests

- Unit (17): constraint eval (numeric/float/string/bool/logic/precedence/
  parentheses/`exist`/negative numbers/incompatible types/parser errors) + trader query
  (filter by type + constraint, max/min preferences, `how_many` limit,
  export/withdraw).
