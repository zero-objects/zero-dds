# zerodds-corba-cos-trading

OMG **Trading Object Service** (`CosTrading`) — pure-Rust `no_std + alloc`,
`forbid(unsafe_code)`.

Service discovery via a **constraint language**: providers export typed
offers, consumers find them with boolean expressions over the properties.

```rust
use zerodds_corba_cos_trading::{Offer, Preference, Trader, Value};

let mut trader = Trader::new();
trader.export(
    Offer::new("Printer", ior_bytes)
        .with("Speed", Value::Int(60))
        .with("Color", Value::Bool(true)),
);

// "fastest color printer":
let hits = trader.query(
    "Printer",
    "Speed > 30 and Color == TRUE",
    &Preference::Max("Speed".into()),
    1,
).unwrap();
```

Constraint language (OMG subset): comparisons `== != < <= > >=`, boolean
`and/or/not`, `exist <prop>`, parentheses, literals (int/float/`'string'`/TRUE/FALSE).

Spec: OMG Trading Object Service. Part of the "optional profiles as
differentiation" strategy (sequenced after the OTS).
