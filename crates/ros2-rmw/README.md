# `zerodds-ros2-rmw`

ROS 2 RMW middleware-interface mapping (REP-2003/2004 + topic-name-mangling) for ZeroDDS bridge

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
zerodds-ros2-rmw = { path = "../path/to/ros2-rmw" }
# or, when published:
# zerodds-ros2-rmw = "0.x"
```

## Tests

```bash
cargo test -p zerodds-ros2-rmw
```

## See also

* [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) —
  layered crate architecture.
* [`documentation/02-architecture/components.md`](../../documentation/02-architecture/components.md) —
  per-crate map (English).
