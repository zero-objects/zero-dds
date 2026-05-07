# `chaos-clock-skew`

LD_PRELOAD shim that skews clock_gettime() for chaos-engineering (WP 5.F.2)

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
chaos-clock-skew = { path = "../path/to/chaos-clock-skew" }
# or, when published:
# chaos-clock-skew = "0.x"
```

## Tests

```bash
cargo test -p chaos-clock-skew
```

## See also

* [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) —
  layered crate architecture.
* [`documentation/02-architecture/components.md`](../../documentation/02-architecture/components.md) —
  per-crate map (English).
