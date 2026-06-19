# `zerodds-corba-cos-trading` v1.0 — CosTrading: Service-Discovery per Constraint-Sprache

ZeroDDS Vendor-Spec. In `crates/corba-cos-trading` implementiert. Authored im Stil
der OMG-CORBA-Spec (nummerierte Klauseln, RFC-2119-Keywords, Konformitätsprofil).
Die **Service-Semantik** ist OMG-normativ (OMG Trading Object Service); diese Spec
normiert das **ZeroDDS-eigene Rust-PSM** und die unterstützte Teilmenge der
**Constraint-Sprache** — die OMG hat kein Rust-Language-Mapping standardisiert.

## Motivation

Der **Trading Object Service** ist CORBAs Service-Discovery per Eigenschaften:
Anbieter exportieren typisierte *Offers*, Konsumenten suchen sie mit booleschen
**Constraint-Ausdrücken** über die Properties (`"Speed > 100 and Color == 'red'"`).
Das ist ein abgeschlossenes OMG-Profil und passt zur „optionale Profile als
Differenzierung"-Strategie. `zerodds-corba-cos-trading` liefert Register + Lookup +
eine vollständige Constraint-Engine in `no_std + alloc`, `forbid(unsafe_code)`.

## Ziele

- **Typisierte Offers**: Service-Type + `Value`-Properties + Object-Reference.
- **Constraint-Sprache**: Tokenizer + Recursive-Descent-Parser + Evaluator über
  einer wohldefinierten Grammatik-Teilmenge (§2).
- **Register + Lookup**: `export`/`withdraw` + `query` mit Constraint + Preferences
  + `how_many`-Limit.
- **Spec-treue Fehlersemantik**: fehlende Property → Vergleich matcht nicht.

## Nicht-Ziele

- **Föderierte Trader** (Trader-zu-Trader-Links, OMG `Link`-Interface) — v1.0 ist
  ein einzelner In-Memory-Trader.
- **Dynamic Properties** (Property-Werte, die ein Callback liefert) — v1.0 trägt
  statische `Value`-Properties.
- **Service-Type-Hierarchien** (Subtyp-Matching) — v1.0 matcht den Service-Type
  exakt.

## §1 Datenmodell

### §1.1 `Value`

Ein typisierter Property-Wert (CosTrading `PropertyValue` als `any`-Teilmenge):

```rust
pub enum Value { Int(i64), Float(f64), Str(String), Bool(bool) }
```

`Value::compare` MUSS Int/Float numerisch vergleichen (gemischt → Float-
Promotion), Str lexikographisch, Bool als `false < true`; inkompatible Typen →
kein Vergleich.

### §1.2 `Offer`

```rust
pub struct Offer {
    pub service_type: String,
    pub properties: BTreeMap<String, Value>,
    pub ior: Vec<u8>,   // Object-Reference (opake IOR-Bytes)
}
```

## §2 Constraint-Sprache

### §2.1 Grammatik

Die unterstützte Teilmenge der OMG-Trading-Constraint-Language (RFC-2119:
ein konformer Evaluator MUSS genau diese Produktionen akzeptieren):

```ebnf
expr       ::= or_expr
or_expr    ::= and_expr ('or' and_expr)*
and_expr   ::= not_expr ('and' not_expr)*
not_expr   ::= 'not' not_expr | comparison
comparison ::= primary (('==' | '!=' | '<' | '<=' | '>' | '>=') primary)?
primary    ::= '(' expr ')' | 'exist' ident | ident | literal
literal    ::= integer | float | "'" string "'" | TRUE | FALSE
```

`and` bindet stärker als `or`. `not` ist präfix. Eine nackte Property als
Ausdruck (`Available`) matcht, wenn sie `Bool(true)` ist. Ein leerer Constraint
matcht immer (`TRUE`).

### §2.2 Evaluation

- Vergleich: beide Operanden müssen auflösbar sein; fehlt eine Property, MUSS der
  Vergleich `false` liefern (OMG-Semantik). Inkompatible Typen → `false`.
- `exist <prop>` → ob die Property in der Map vorhanden ist.
- Keyword-Vergleiche sind case-insensitiv (`and`/`AND`, `true`/`TRUE`).

### §2.3 Fehler

`Constraint::parse` MUSS bei Lexer-/Parser-Fehlern `ConstraintError`
(`UnexpectedToken`/`UnexpectedEnd`/`BadNumber`/`UnterminatedString`/`BadChar`)
liefern — z.B. `"Speed >"` (fehlender Operand), `"Speed = 5"` (`=` statt `==`),
`"Color == 'red"` (unterminiert), `"(Speed > 1"` (fehlende `)`).

## §3 Trader

### §3.1 Register + Lookup

```rust
impl Trader {
    pub fn export(&mut self, offer: Offer) -> OfferId;   // Register::export
    pub fn withdraw(&mut self, id: OfferId) -> bool;     // Register::withdraw
    pub fn query(&self, service_type: &str, constraint: &str,
                 preference: &Preference, how_many: usize)
        -> Result<Vec<(OfferId, &Offer)>, ConstraintError>; // Lookup::query
}
```

`query` MUSS Offers nach `service_type` (exakt) UND `constraint`-Match filtern,
nach `preference` sortieren und auf `how_many` begrenzen (`0` = unbegrenzt).

### §3.2 Preferences

`Preference` ist `First` (Export-Reihenfolge), `Max(prop)` (absteigend nach
numerischer Property) oder `Min(prop)` (aufsteigend). Nicht-numerische/fehlende
Properties sortieren ans Ende.

## §4 Konformität

Ein **Trading-konformes** ZeroDDS-Modul:

1. akzeptiert die Constraint-Grammatik aus §2.1 und wertet sie gemäß §2.2 aus,
2. liefert für eine fehlende Property im Vergleich `false`,
3. filtert/sortiert/limitiert `query` gemäß §3.1/§3.2,
4. meldet Constraint-Syntaxfehler als `ConstraintError` (§2.3).

## §5 Implementierungs-Mapping

| Spec | Code |
|---|---|
| §1.1 `Value` | `corba-cos-trading/src/property.rs` |
| §2 Constraint-Sprache | `corba-cos-trading/src/constraint.rs` — Tokenizer, Parser, Evaluator |
| §1.2/§3 Trader | `corba-cos-trading/src/trader.rs` — `Offer`, `Trader`, `Preference` |

## §6 Tests

- Unit (17): Constraint-Eval (numerisch/float/string/bool/Logik/Präzedenz/
  Klammern/`exist`/Negativzahlen/inkompatible-Typen/Parser-Fehler) + Trader-Query
  (Filter nach Typ + Constraint, Max-/Min-Preferences, `how_many`-Limit,
  export/withdraw).
