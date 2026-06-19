# `dds-roundtrip-codegen`

Internal build helper: generates C++17 headers from tests/perf/dds-roundtrip-bench/roundtrip.idl via the zerodds-idl-cpp library API. Used by the cross-vendor roundtrip benchmark.

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
dds-roundtrip-codegen = { path = "../path/to/dds-roundtrip-codegen" }
# or, when published:
# dds-roundtrip-codegen = "0.x"
```

## Tests

```bash
cargo test -p dds-roundtrip-codegen
```

## See also

* [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) —
  layered crate architecture.
* [`documentation/02-architecture/components.md`](../../documentation/02-architecture/components.md) —
  per-crate map (English).
