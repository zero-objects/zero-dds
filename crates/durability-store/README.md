# `zerodds-durability-store`

DDS Durability-Service storage abstraction: DurabilityStore trait, contract-bounded retention, memory hot-tier cache over any cold adapter. TRANSIENT/PERSISTENT (DDS 1.4 §2.2.3.4/§2.2.3.5).

Part of [**ZeroDDS**](../../README.md). Safety classification: **STANDARD**.

## Status

This README is auto-generated from `Cargo.toml` metadata. For
hand-written documentation see the rustdoc on the crate's public
items, or the relevant station in the
[Documentation Trail](../../documentation/README.md).

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
zerodds-durability-store = { path = "../path/to/durability-store" }
# or, when published:
# zerodds-durability-store = "0.x"
```

## Tests

```bash
cargo test -p zerodds-durability-store
```

## See also

* [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) —
  layered crate architecture.
* [`documentation/02-architecture/components.md`](../../documentation/02-architecture/components.md) —
  per-crate map (English).
