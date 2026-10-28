# `zerodds-cargo-dag` (internal)

> Topologically sorts the ZeroDDS workspace crates so a sequential
> `cargo publish` runs in dependency order.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

Internal release-engineering tool. Runs `cargo metadata
--format-version 1 --no-deps` over the workspace, takes the
workspace-internal path dependencies of every member as edges, and
emits the crate names in topological order. Used by the publish
script to upload crates in an order that satisfies crates.io's
dependency-resolution rule.

The member set comes from `cargo metadata`, not from the
`workspace.members` list in the root `Cargo.toml`: Cargo makes every
path dependency of a listed member a member as well, so the list is
smaller than the workspace. A new path dependency therefore enters the
publish order on its own, with no list to keep in step.

Dev-dependencies are not edges (they are not resolved when a crate is
published); build-dependencies are.

This tool is **not** published on crates.io; it consumes the
in-tree workspace graph and is only meaningful from the repo root.

## Quickstart

```bash
cd /path/to/zero-dds
cargo run -p zerodds-cargo-dag -- . --only-publishable
```

Output: one crate name per line in publish order. Without
`--only-publishable` the crates carrying `publish = false` are listed
too. `--format verbose` adds path and dependency count, `--format
json` emits the same records as an array.

## Stability

`1.0.0-rc.7`, a pre-release. The public API can still change before
1.0.0; the crate's CHANGELOG lists every change.

## Build & test

```bash
cargo build -p zerodds-cargo-dag
cargo test  -p zerodds-cargo-dag
```

`tests/workspace_publish_set.rs` runs against this repository: it
compares the emitted publish order against the publishable members
`cargo metadata` reports, and checks that every crate is ordered after
its dependencies.

## Licence

Apache-2.0. See [`LICENSE`](../../LICENSE).
