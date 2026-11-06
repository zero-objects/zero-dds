#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Builds what tests/perf/speed/speed.py measures, for one ZeroDDS tree, on
# the speed host:
#
#   <out>/target/release/libzerodds.so          the C API, default features
#                                               (as the packages ship it)
#   <out>/target/release/examples/zerodds_perf  row zerodds-rust
#   <out>/bench/<stack>-roundtrip               the apps of
#                                               tests/perf/dds-roundtrip-bench
#   <out>/bench/hdds-roundtrip                  the HDDS app (a cargo package
#                                               of its own, dds-roundtrip-bench/
#                                               hdds, target <out>/hdds-target)
#
# Usage: build.sh <zerodds-tree> <out-dir> [<harness-tree>]
#
# <harness-tree> holds the measuring code (the apps, bench_common.hpp,
# zerodds_perf.rs) when it is not <zerodds-tree>: the RC7 baseline measures
# the rc.7 library with the current harness, so the numbers differ by the
# library only. TARGETS limits the CMake targets (e.g. TARGETS=zerodds-roundtrip
# when the vendors' apps are built already). CARGO_BUILD_JOBS limits cargo.
set -euo pipefail

TREE=$(cd "$1" && pwd)
mkdir -p "$2"
OUT=$(cd "$2" && pwd)
HARNESS=$(cd "${3:-$1}" && pwd)
export CARGO_TARGET_DIR="$OUT/target"

if [ "$HARNESS" != "$TREE" ]; then
    cp "$HARNESS/crates/dcps/examples/zerodds_perf.rs" "$TREE/crates/dcps/examples/zerodds_perf.rs"
fi
# --locked: the tree's Cargo.lock as committed. Without it cargo rewrote a
# stale lock in the speed store's clone (at 7bf8c4fa5d7: 3cbbe443679 had
# dropped proptest from zerodds-amqp-bridge and left its lock entry), and
# the next checkout there stopped on the changed file (pipeline 9025).
(
    cd "$TREE"
    cargo build --locked --release -p zerodds-c-api
    cargo build --locked --release -p zerodds-dcps --example zerodds_perf
)

cmake -S "$HARNESS/tests/perf/dds-roundtrip-bench" -B "$OUT/bench" \
    -DCMAKE_BUILD_TYPE=Release \
    -DZERODDS_REPO="$TREE" \
    -DZERODDS_LIB="$OUT/target/release/libzerodds.so" \
    -DZERODDS_BENCH_NOSEC=ON >"$OUT/cmake-configure.log"
# TARGETS without hdds-roundtrip, which is not a CMake target.
cmake_targets=""
for t in ${TARGETS:-}; do
    [ "$t" = hdds-roundtrip ] || cmake_targets="$cmake_targets${cmake_targets:+ }$t"
done
if [ -z "${TARGETS:-}" ] || [ -n "$cmake_targets" ]; then
    # shellcheck disable=SC2086
    cmake --build "$OUT/bench" -j"$(nproc)" ${cmake_targets:+--target $cmake_targets}
fi
# HDDS (and its IDL code generator) from git.hdds.io at the revisions its
# Cargo.toml pins, with its Cargo.lock.
case " ${TARGETS:-hdds-roundtrip} " in
*" hdds-roundtrip "*)
    CARGO_TARGET_DIR="$OUT/hdds-target" \
        cargo build --locked --release --manifest-path "$HARNESS/tests/perf/dds-roundtrip-bench/hdds/Cargo.toml"
    cp "$OUT/hdds-target/release/hdds-roundtrip" "$OUT/bench/hdds-roundtrip"
    ;;
esac
echo "build.sh: $OUT/bench, $OUT/target/release/examples/zerodds_perf"
