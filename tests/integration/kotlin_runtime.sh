#!/usr/bin/env bash
# Compiles runtime/kotlin/ZdWire.kt once for a codegen job, under the
# `kotlinc -Werror` the Kotlin tests use, into the temporary directory they
# read (`precompiled_runtime` in crates/lang-rules/tests/kotlin_end_to_end.rs).
# A test then compiles its case against these classes instead of compiling
# the runtime beside every case: one small case took 5.3 s against the
# classes and 9.6 s with the runtime source on the Linux test host. A test
# that finds no directory, or one compiled from another text, compiles the
# runtime itself.
#
#     bash tests/integration/kotlin_runtime.sh
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
dir="${TMPDIR:-/tmp}"
dir="${dir%/}/zd-kotlin-runtime"
rm -rf "$dir"
mkdir -p "$dir"
cp "$root/runtime/kotlin/ZdWire.kt" "$dir/ZdWire.kt"
kotlinc -Werror "$dir/ZdWire.kt" -d "$dir/classes"
touch "$dir/complete"
