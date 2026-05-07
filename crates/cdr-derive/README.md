# `zerodds-cdr-derive`

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Proc-macro `#[derive(DdsType)]` fuer XCDR2 TypeSupport. Implementiert
[`zerodds-xcdr2-rust-1.0`](../../docs/specs/zerodds-xcdr2-rust-1.0.md)
§11.1.

## Was diese Crate tut

Annotiert einen Plain-Rust-`struct` und emittiert zur Compile-Zeit eine
`impl zerodds_dcps::DdsType`-Implementation, die XCDR2 byte-genau gegen
[OMG XTypes 1.3 §7.4](https://www.omg.org/spec/DDS-XTypes/) encoded und
decoded — gleiche Wire-Form wie der `idl-rust`-Codegen.

## Spec + Layer

- Spec: `zerodds-xcdr2-rust-1.0` §11.1; XTypes 1.3 §7.4 + §7.6.8.4.
- Layer: 1 Primitives (Helper fuer `zerodds-cdr` und `zerodds-dcps`).

## Quickstart

```rust,ignore
use zerodds_cdr_derive::DdsType;

#[derive(DdsType, Clone, PartialEq)]
pub struct Sensor {
    #[dds(key)]
    pub id: i32,
    pub value: f64,
}

// Generiert automatisch:
//   impl zerodds_dcps::DdsType for Sensor {
//       const TYPE_NAME: &'static str = "Sensor";
//       const HAS_KEY: bool = true;
//       fn encode(&self, out: &mut Vec<u8>) -> ... { ... }
//       fn decode(bytes: &[u8]) -> Result<Self, ...> { ... }
//       fn encode_key_holder_be(&self, holder: ...) { ... }
//   }
```

## Feature-Flags

Keine.

## Stability

Alle `pub`-Items sind `1.0.0-rc.1`-stabil. Das exakte Token-Layout des
Macro-Outputs kann zwischen Minor-Versionen aendern; die emittierte
`impl DdsType`-Form bleibt spec-konform.

## Links

- Spec: [`docs/specs/zerodds-xcdr2-rust-1.0.md`](../../docs/specs/zerodds-xcdr2-rust-1.0.md)
- Coverage: [`docs/spec-coverage/zerodds-xcdr2-rust-1.0.md`](../../docs/spec-coverage/zerodds-xcdr2-rust-1.0.md)
- Tests: `crates/cdr-derive/tests/derive_smoke.rs` (6 Tests, byte-genau zu V-2-Wire-Vector).
- CHANGELOG: [`CHANGELOG.md`](CHANGELOG.md).
