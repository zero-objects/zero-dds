#!/usr/bin/env bash
# Compiles the Elixir wire runtime (runtime/elixir/lib) once for a codegen
# job into the temporary directory crates/lang-rules/tests/elixir_codec.rs
# reads (`precompiled_runtime`), with the sources it compiled beside the
# ebin directory. A test that finds the same sources there loads this ebin;
# otherwise it compiles the runtime itself, once per test process, which
# under nextest is once per test.
#
#     bash tests/integration/elixir_runtime.sh
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
dir="${TMPDIR:-/tmp}"
dir="${dir%/}/zd-elixir-runtime"
rm -rf "$dir"
mkdir -p "$dir/src" "$dir/ebin"
cp "$root/runtime/elixir/lib/zerodds_wire.ex" "$root"/runtime/elixir/lib/zerodds_wire/* "$dir/src/"
elixirc --ignore-module-conflict -o "$dir/ebin" "$dir"/src/*
touch "$dir/complete"
