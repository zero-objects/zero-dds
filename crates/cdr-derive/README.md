# `zerodds-cdr-derive`

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Proc-macro `#[derive(DdsType)]` for XCDR2 TypeSupport. Implements
[`zerodds-xcdr2-rust-1.0`](../../docs/specs/zerodds-xcdr2-rust-1.0.md)
§11.1.

## What this crate does

Annotates a plain Rust `struct` and emits at compile time an
`impl zerodds_dcps::DdsType` implementation that encodes and decodes
XCDR2 byte-exact against
[OMG XTypes 1.3 §7.4](https://www.omg.org/spec/DDS-XTypes/) — the same
wire form as the `idl-rust` codegen.

## Spec + layer

- Spec: `zerodds-xcdr2-rust-1.0` §11.1; XTypes 1.3 §7.4 + §7.6.8.4.
- Layer: 1 Primitives (helper for `zerodds-cdr` and `zerodds-dcps`).

## Quickstart

```rust,ignore
use zerodds_cdr_derive::DdsType;

#[derive(DdsType, Clone, PartialEq)]
pub struct Sensor {
    #[dds(key)]
    pub id: i32,
    pub value: f64,
}

// Auto-generates:
//   impl zerodds_dcps::DdsType for Sensor {
//       const TYPE_NAME: &'static str = "Sensor";
//       const HAS_KEY: bool = true;
//       fn encode(&self, out: &mut Vec<u8>) -> ... { ... }
//       fn decode(bytes: &[u8]) -> Result<Self, ...> { ... }
//       fn encode_key_holder_be(&self, holder: ...) { ... }
//   }
```

## Feature flags

None.

## Stability

All `pub` items are `1.0.0-rc.1`-stable. The exact token layout of the
macro output may change between minor versions; the emitted
`impl DdsType` form stays spec-conformant.

## Links

- Spec: [`docs/specs/zerodds-xcdr2-rust-1.0.md`](../../docs/specs/zerodds-xcdr2-rust-1.0.md)
- Coverage: [`docs/spec-coverage/zerodds-xcdr2-rust-1.0.md`](../../docs/spec-coverage/zerodds-xcdr2-rust-1.0.md)
- Tests: `crates/cdr-derive/tests/derive_smoke.rs` (6 tests, byte-exact to the V-2 wire vector).
- CHANGELOG: [`CHANGELOG.md`](CHANGELOG.md).
