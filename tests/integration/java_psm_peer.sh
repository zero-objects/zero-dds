#!/usr/bin/env bash
# The Java subscriber and publisher of tests/integration/tests/bindings_ffi.rs
# (zerodds-ffi-loader-1.0 §5, the Java PSM of §3.2; DDS 1.4 §2.2.4.1):
# compiles tests/integration/peers/ffi/java/{FfiSub,FfiPub}.java against the classes of
# crates/java-omgdds, which the job's Maven step built, once per job before
# the tests, so that no test compiles inside its 25 s. The PSM has no
# dependency at run time beyond its own classes.
#
# Prints `ENV ZERODDS_JAVA_PSM_CLASSPATH=<classes>:<subscriber>` for the
# nextest step after it (ci/run_integration.py).
#
#     bash tests/integration/java_psm_peer.sh
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$repo/target}"
peers="$target/ffi-peers/java"
pom="$repo/crates/java-omgdds/java/pom.xml"
classes="$repo/crates/java-omgdds/java/target/classes"
mkdir -p "$peers"

if [ ! -d "$classes/org/omg/dds" ]; then
  echo "== the classes of the Java PSM =="
  mvn -B --no-transfer-progress -q -f "$pom" -DskipTests compile
fi

echo "== the Java subscriber and publisher =="
javac -d "$peers" -cp "$classes" "$repo/tests/integration/peers/ffi/java/FfiSub.java" \
  "$repo/tests/integration/peers/ffi/java/FfiPub.java"

echo "ENV ZERODDS_JAVA_PSM_CLASSPATH=$classes:$peers"
