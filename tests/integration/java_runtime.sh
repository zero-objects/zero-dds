#!/usr/bin/env bash
# Compiles the Java wire runtime (runtime/java/src/main/java/org/zerodds/wire)
# once for a codegen job into the temporary directory the Java gates read
# (`precompiled_java_runtime` in crates/lang-rules/tests/support/corpus.rs),
# with the sources it compiled beside the classes. A gate that finds the same
# sources there compiles its cases against these classes; otherwise it
# compiles the runtime itself, once per test process, which under nextest is
# once per test.
#
#     bash tests/integration/java_runtime.sh
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
dir="${TMPDIR:-/tmp}"
dir="${dir%/}/zd-java-runtime"
rm -rf "$dir"
mkdir -p "$dir/src" "$dir/classes"
cp "$root"/runtime/java/src/main/java/org/zerodds/wire/*.java "$dir/src/"
javac -encoding UTF-8 -d "$dir/classes" "$dir"/src/*.java
touch "$dir/complete"
