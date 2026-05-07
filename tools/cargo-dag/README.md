# `zerodds-cargo-dag` (internal)

> Topologically sorts the ZeroDDS workspace crates so a sequential
> `cargo publish` runs in dependency order.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

Internal release-engineering tool. Parses the workspace
`Cargo.toml`, walks the per-crate `[dependencies]` blocks, and
emits the crate names in topological order. Used by the publish
script to upload crates in an order that satisfies crates.io's
dependency-resolution rule.

This tool is **not** published on crates.io; it consumes the
in-tree workspace graph and is only meaningful from the repo root.

## Quickstart

```bash
cd /path/to/zero-dds
cargo run -p zerodds-cargo-dag -- --workspace .
```

Output: one crate name per line in publish order.

## Stability

`1.0.0-rc.1` — internal CLI is stable for the publish-script
contract. Breaking changes are coordinated with that script.

## Build & test

```bash
cargo build -p zerodds-cargo-dag
cargo test  -p zerodds-cargo-dag
```

## Licence

Apache-2.0. See [`LICENSE`](../../LICENSE).
