# `zerodds-idl-cpp`

IDL4 → C++ code-generator (OMG IDL4-CPP mapping, formal/2018-07-01) — Foundation (C5.1-a) + Status/QoS/DCPS (C5.1-b) + DDS-PSM-CXX skeleton (C5.2) + DDS-RPC C++ PSM (C6.1.D-cpp)

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
zerodds-idl-cpp = { path = "../path/to/idl-cpp" }
# or, when published:
# zerodds-idl-cpp = "0.x"
```

## Tests

```bash
cargo test -p zerodds-idl-cpp
```

## See also

* [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) —
  layered crate architecture.
* [`documentation/02-architecture/components.md`](../../documentation/02-architecture/components.md) —
  per-crate map (English).
