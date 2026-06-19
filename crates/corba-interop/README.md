# `zerodds-corba-interop`

CORBA speed + cross-ORB interop harness for ZeroDDS (hand-marshalled Echo over the real GIOP/IIOP/POA/CDR stack).

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
zerodds-corba-interop = { path = "../path/to/corba-interop" }
# or, when published:
# zerodds-corba-interop = "0.x"
```

## Tests

```bash
cargo test -p zerodds-corba-interop
```

## See also

* [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) —
  layered crate architecture.
* [`documentation/02-architecture/components.md`](../../documentation/02-architecture/components.md) —
  per-crate map (English).
