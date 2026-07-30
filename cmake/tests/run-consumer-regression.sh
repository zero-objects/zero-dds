#!/usr/bin/env bash
#
# Consumer regression test for the zerodds CMake build contract (P0-3).
#
# Configures a mini parent project (cmake/tests/consumer-regression) that adds
# zerodds via add_subdirectory() with CMAKE_BUILD_TYPE=Debug and its own
# BUILD_TESTING=OFF / BUILD_EXAMPLES=ON / BUILD_TESTS=OFF, then verifies zerodds
# did not mutate the parent scope and did not configure its example/test targets.
#
# Only `cmake` *configure* runs (no build), so this needs cmake + a cargo on PATH
# (cargo is probed at configure time) but compiles nothing. Exit 0 = pass.
#
# Usage:  cmake/tests/run-consumer-regression.sh
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
src="$here/consumer-regression"
build="$(mktemp -d "${TMPDIR:-/tmp}/zerodds-consumer-regression.XXXXXX")"
trap 'rm -rf "$build"' EXIT

echo "== consumer-regression: configure parent (Debug, BUILD_TESTING=OFF, BUILD_EXAMPLES=ON, BUILD_TESTS=OFF) =="
if ! cmake -S "$src" -B "$build" \
      -DCMAKE_BUILD_TYPE=Debug \
      -DBUILD_TESTING=OFF \
      -DBUILD_EXAMPLES=ON \
      -DBUILD_TESTS=OFF \
      > "$build/configure.log" 2>&1; then
  echo "---- cmake configure output ----"
  cat "$build/configure.log"
  echo "CONFIGURE FAILED (assertions inside the parent CMakeLists raise FATAL_ERROR on violation)"
  exit 1
fi

if ! grep -q "consumer-regression PASSED" "$build/configure.log"; then
  echo "---- cmake configure output ----"
  cat "$build/configure.log"
  echo "PASS marker not found in configure output"
  exit 1
fi

grep "consumer-regression PASSED" "$build/configure.log"
echo "OK"
