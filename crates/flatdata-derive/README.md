# `zerodds-flatdata-derive`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-flatdata-derive/badge.svg)](https://docs.rs/zerodds-flatdata-derive)

Proc-macro `#[derive(FlatStruct)]` for the
[`zerodds-flatdata`](https://docs.rs/zerodds-flatdata) trait. Generates
`unsafe impl FlatStruct for T` with a deterministic
SHA-256-based `TYPE_HASH` derived from the type name and field layout. Safety
classification: **STANDARD** (proc-macro).

## Spec mapping

| Spec | Section |
|------|-----------|
| ZeroDDS-flatdata 1.0 | §1.2 (derive macro) |

## What's inside

- **`#[derive(FlatStruct)]`** — generates `unsafe impl ::zerodds_flatdata::FlatStruct`
  with `TYPE_HASH = sha256(layout_signature(T))[..16]`.
- **Compile-time checks**:
  - `enum`/`union` are rejected with `compile_error!` (no stable
    layout).
  - A missing `#[repr(C)]` / `#[repr(transparent)]` is rejected with
    `compile_error!` (the default repr is not guaranteed to be
    layout-stable).
- **Layout signature** `<TypeName>{<field-name>:<field-ty>,...}` detects
  type rename, field add/remove, field reorder, field type change.

## Layer position

Layer 4 — Core Services. Build-time companion to `zerodds-flatdata`,
no runtime code surface. No ZeroDDS crate deps.

## Quickstart

```rust,ignore
use zerodds_flatdata_derive::FlatStruct;

#[derive(Copy, Clone, FlatStruct)]
#[repr(C)]
struct Pose { x: f64, y: f64, z: f64 }

// expandiert zu:
//
//   unsafe impl ::zerodds_flatdata::FlatStruct for Pose {
//       const TYPE_HASH: [u8; 16] = [/* sha256("Pose{x:f64,y:f64,z:f64}")[..16] */];
//   }
```

`WIRE_SIZE` and the `as_bytes`/`from_bytes_unchecked` methods come as a
default from the trait and need no macro override.

## Feature flags

None. The crate is a pure `proc-macro` lib.

## Stability

`1.0.0-rc.1` is the initial release materialization. All public macro
paths are RC1-stable; the layout signature format is considered
wire-stable — a change would invalidate all generated `TYPE_HASH`
values and is therefore major-breaking.

## Build & Test

```bash
cargo build -p zerodds-flatdata-derive
# Tests live in zerodds-flatdata because proc-macro crates cannot test
# their own output:
cargo test -p zerodds-flatdata --test derive
```

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`zerodds-flatdata`](../flatdata) — the `FlatStruct` trait itself.
- [`docs/specs/zerodds-flatdata-1.0.md`](../../docs/specs/zerodds-flatdata-1.0.md) §1.2.
