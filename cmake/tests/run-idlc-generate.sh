#!/usr/bin/env bash
#
# Generator test for the zerodds CMake build contract (P0-1b).
#
# Builds the cmake/tests/idlc-generate mini-project with a real cmake + ninja and
# verifies the stamp/depfile/manifest wiring of zerodds_idlc_generate():
#
#   1. configure + build           -> generation runs, outputs + stamp exist
#   2. rebuild with no change       -> ninja "no work to do" (no rebuild)
#   3. touch the #included common.idl -> ninja regenerates (stamp mtime advances)
#      This is the core regression: the old extension-guessing generator only
#      put the top IDL in DEPENDS, so an include change was "no work to do".
#
# zerodds-idlc is used as a prebuilt HOST binary: built once via cargo and passed
# to cmake as -DZERODDS_IDLC_EXE, so the mini-project never invokes cargo itself.
#
# Requires: cmake, ninja, and (unless $ZERODDS_IDLC_EXE is preset) cargo.
# Exit 0 = pass.
#
# Usage:  cmake/tests/run-idlc-generate.sh
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
src="$here/idlc-generate"

if ! command -v cmake >/dev/null 2>&1; then
  echo "SKIP: cmake not found on PATH"; exit 0
fi
if ! command -v ninja >/dev/null 2>&1; then
  echo "SKIP: ninja not found on PATH (this test requires the Ninja generator)"; exit 0
fi

# Prebuilt host tool: honour a preset path, else cargo-build it once.
idlc="${ZERODDS_IDLC_EXE:-}"
if [[ -z "$idlc" ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "SKIP: cargo not found and ZERODDS_IDLC_EXE not set"; exit 0
  fi
  echo "== building host zerodds-idlc (cargo build -p zerodds-idlc) =="
  cargo build --manifest-path "$repo_root/Cargo.toml" -p zerodds-idlc >/dev/null
  idlc="$repo_root/target/debug/zerodds-idlc"
fi
if [[ ! -x "$idlc" ]]; then
  echo "FAIL: zerodds-idlc binary not executable: $idlc"; exit 1
fi

build="$(mktemp -d "${TMPDIR:-/tmp}/zerodds-idlc-generate.XXXXXX")"
# Scratch copy of just the IDL so `touch` never dirties the repo tree; the
# CMakeLists and generator module are still read from the real checkout.
idl_dir="$(mktemp -d "${TMPDIR:-/tmp}/zerodds-idlc-generate-idl.XXXXXX")"
cp -R "$src/idl/." "$idl_dir/"
trap 'rm -rf "$build" "$idl_dir"' EXIT

mtime() { stat -f %m "$1" 2>/dev/null || stat -c %Y "$1"; }

fail() { echo "FAIL: $*"; exit 1; }

echo "== configure (Ninja) =="
cmake -S "$src" -B "$build" -G Ninja \
  -DZERODDS_IDLC_EXE="$idlc" \
  -DZERODDS_CMAKE_DIR="$here/.." \
  -DIDL_DIR="$idl_dir" \
  > "$build/configure.log" 2>&1 || { cat "$build/configure.log"; fail "configure"; }

echo "== build 1 (initial generation) =="
cmake --build "$build" > "$build/build1.log" 2>&1 || { cat "$build/build1.log"; fail "build 1"; }

stamp="$build/generated/.zerodds-idlc/robot_idl__robot_idl.stamp"
header="$build/generated/robot.hpp"
manifest="$build/generated/.zerodds-idlc/robot_idl__robot_idl.manifest.json"
depfile="$build/generated/.zerodds-idlc/robot_idl__robot_idl.d"
[[ -f "$stamp" ]]    || fail "stamp not created: $stamp"
[[ -f "$header" ]]   || fail "generated header not created: $header"
[[ -f "$manifest" ]] || fail "manifest not created: $manifest"
[[ -f "$depfile" ]]  || fail "depfile not created: $depfile"
echo "   ok: stamp, header, manifest, depfile present"

echo "== build 2 (no-op: must be 'no work to do') =="
cmake --build "$build" > "$build/build2.log" 2>&1 || { cat "$build/build2.log"; fail "build 2"; }
if ! grep -q "no work to do" "$build/build2.log"; then
  echo "---- build2 output ----"; cat "$build/build2.log"
  fail "expected no-op rebuild, but ninja did work"
fi
echo "   ok: no rebuild"

echo "== build 3 (touch #included common.idl: must regenerate) =="
before="$(mtime "$stamp")"
# Ensure a strictly newer mtime than the stamp regardless of FS granularity.
sleep 1
touch "$idl_dir/common.idl"
cmake --build "$build" > "$build/build3.log" 2>&1 || { cat "$build/build3.log"; fail "build 3"; }
after="$(mtime "$stamp")"
if [[ "$after" -le "$before" ]]; then
  echo "---- build3 output ----"; cat "$build/build3.log"
  fail "include change did not regenerate (stamp mtime $before -> $after)"
fi
if grep -q "no work to do" "$build/build3.log"; then
  echo "---- build3 output ----"; cat "$build/build3.log"
  fail "include change reported 'no work to do' (depfile not honoured)"
fi
echo "   ok: regenerated on include change (stamp $before -> $after)"

echo "run-idlc-generate PASSED"
