# `zerodds-durability-service`

Standalone DDS Durability-Service daemon (ADR 0009): ingests TRANSIENT/PERSISTENT topics over RTPS and replays them to late-joiners, surviving writer death and service restart.

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
zerodds-durability-service = { path = "../path/to/durability-service" }
# or, when published:
# zerodds-durability-service = "0.x"
```

## Tests

```bash
cargo test -p zerodds-durability-service
```

## See also

* [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) —
  layered crate architecture.
* [`documentation/02-architecture/components.md`](../../documentation/02-architecture/components.md) —
  per-crate map (English).
