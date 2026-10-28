# `zerodds-sys`

Marker crate without symbols. The C ABI of ZeroDDS is
[`zerodds-c-api`](../zerodds-c-api/README.md) (`libzerodds`, header
`include/zerodds.h`, spec `docs/specs/zerodds-c-api-1.0.md`).

Part of [**ZeroDDS**](../../README.md).

The crate holds the constant `REFERENCE_C_API_CRATE` and nothing else; it is
`no_std` and `#![forbid(unsafe_code)]`. `crates/safe-crates-only` lists it
among the crates without unsafe code.

## Tests

```bash
cargo test -p zerodds-sys
```
