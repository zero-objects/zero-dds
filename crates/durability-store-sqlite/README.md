# `zerodds-durability-store-sqlite`

sqlite-WAL cold adapter for the ZeroDDS Durability-Service: ACID PERSISTENT storage, the default backend. Implements zerodds-durability-store::DurabilityStore.

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
zerodds-durability-store-sqlite = { path = "../path/to/durability-store-sqlite" }
# or, when published:
# zerodds-durability-store-sqlite = "0.x"
```

## Tests

```bash
cargo test -p zerodds-durability-store-sqlite
```

## See also

* [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) —
  layered crate architecture.
* [`documentation/02-architecture/components.md`](../../documentation/02-architecture/components.md) —
  per-crate map (English).
