#!/usr/bin/env bash
# Builds, once, what every crate `crates/zerodds-build/tests/dds_type.rs`
# writes around a generated module links against: `zerodds-codec`, `zerodds-dcps`,
# `zerodds-types` and `zerodds-wire` as dependencies, `zerodds-build` and
# `zerodds-ast-layout` as build dependencies, under the profile such a crate
# builds them with. The `#[ignore]`d tests of that target then compile only
# their own module and run within the 25 s a test has
# (internal/release/rc8-test-suite.md, "Crate tier"); without this the first
# of them compiles the graph inside its budget.
#
#     bash tests/integration/warm_build_consumer.sh
#
# The consumer is the one `run_generated_with` writes, with one `@final`
# struct, in a scratch directory; the target directory is the job's
# (CARGO_TARGET_DIR), which the tests reuse.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
dir="$(mktemp -d)"
trap 'rm -rf "$dir"' EXIT
mkdir -p "$dir/src"
cat > "$dir/Cargo.toml" <<TOML
[package]
name = "zerodds_build_dds_type_warm"
version = "0.0.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[dependencies]
zerodds-codec = { path = "$root/crates/codec" }
zerodds-dcps = { path = "$root/crates/dcps" }
zerodds-types = { path = "$root/crates/types" }
zerodds-wire = { path = "$root/crates/wire" }

[build-dependencies]
zerodds-build = { path = "$root/crates/zerodds-build" }
zerodds-ast-layout = { path = "$root/crates/ast-layout" }

[workspace]
TOML
printf '@final struct Warm { long a; };\n' > "$dir/idl.idl"
printf 'fn main() -> Result<(), zerodds_build::Error> {\n    zerodds_build::Config::new().dds_type_impls(true).compile(&["idl.idl"])\n}\n' > "$dir/build.rs"
printf '#![allow(dead_code)]\ninclude!(concat!(env!("OUT_DIR"), "/idl.rs"));\n' > "$dir/src/lib.rs"
cp "$root/Cargo.lock" "$dir/Cargo.lock"
# From the checkout, as the tests run cargo: the workspace's .cargo/config.toml applies.
cd "$root/crates/zerodds-build"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}" cargo test --offline --no-run --manifest-path "$dir/Cargo.toml"
