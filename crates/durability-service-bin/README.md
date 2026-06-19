# `zerodds-durability-service-bin`

Standalone ZeroDDS Durability-Service daemon binary (zerodds-durability-svc): one process per domain, pluggable store adapter (sqlite/file/lakehouse), auto-discovery of TRANSIENT/PERSISTENT topics.

Part of [**ZeroDDS**](../../README.md). Safety classification: **TBD**.

## Status

This README is auto-generated from `Cargo.toml` metadata. For
hand-written documentation see the rustdoc on the crate's public
items, or the relevant station in the
[Documentation Trail](../../documentation/README.md).

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
zerodds-durability-service-bin = { path = "../path/to/durability-service-bin" }
# or, when published:
# zerodds-durability-service-bin = "0.x"
```

## Tests

```bash
cargo test -p zerodds-durability-service-bin
```

## See also

* [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) —
  layered crate architecture.
* [`documentation/02-architecture/components.md`](../../documentation/02-architecture/components.md) —
  per-crate map (English).
