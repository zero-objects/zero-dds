#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# NON-GATING LEGACY DEV HELPER. The required #28 CI gate does NOT use this
# script — it runs on the pinned `ci-interop` image (ci/Dockerfile.interop).
# This helper only reproduces the Fast DDS side locally, from the same
# commit-verified pinned sources baked into that image.
#
# Canonical Fast DDS stack (matches the ci-interop image):
#   Fast DDS     v3.6.1  4e81e8b71bcd6e7c5213c000503cba8e49d6022a
#   Fast CDR     v2.3.5  7d33a3b51a1585f5631b0a8d905bcc4f249d0f34
#   Fast-DDS-Gen v4.3.0  cc0072b8849b35c67bd7e187990efad58e2871ae
#   foonathan    v1.4.1  347cb67581e51273a612780eb256a3c134c10bae
set -euo pipefail

FASTCDR_TAG="${FASTCDR_TAG:-v2.3.5}";    FASTCDR_COMMIT="${FASTCDR_COMMIT:-7d33a3b51a1585f5631b0a8d905bcc4f249d0f34}"
FASTDDS_TAG="${FASTDDS_TAG:-v3.6.1}";    FASTDDS_COMMIT="${FASTDDS_COMMIT:-4e81e8b71bcd6e7c5213c000503cba8e49d6022a}"
FASTDDSGEN_TAG="${FASTDDSGEN_TAG:-v4.3.0}"; FASTDDSGEN_COMMIT="${FASTDDSGEN_COMMIT:-cc0072b8849b35c67bd7e187990efad58e2871ae}"
FOONATHAN_TAG="${FOONATHAN_TAG:-v1.4.1}";   FOONATHAN_COMMIT="${FOONATHAN_COMMIT:-347cb67581e51273a612780eb256a3c134c10bae}"
PREFIX="${FASTDDS_PREFIX:-$HOME/fastdds-install}"
WORK="${WORK:-$HOME/fastdds-src}"
HERE="$(cd "$(dirname "$0")" && pwd)"
export CMAKE_BUILD_PARALLEL_LEVEL="${BUILD_JOBS:-2}"
mkdir -p "$WORK"

verify() { test "$(git -C "$1" rev-parse HEAD)" = "$2" || { echo "commit mismatch $1: $(git -C "$1" rev-parse HEAD) != $2" >&2; exit 1; }; }
clone() { [ -d "$WORK/$3" ] || git clone --depth 1 --branch "$2" "https://github.com/eProsima/$1.git" "$WORK/$3"; verify "$WORK/$3" "$4"; }
cmake_install() { local d="$1"; shift; cmake --compile-no-warning-as-error -S "$WORK/$d" -B "$WORK/$d/b" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$PREFIX" -DCMAKE_PREFIX_PATH="$PREFIX" "$@"; cmake --build "$WORK/$d/b" --target install; }

echo "=== install_fastdds (legacy dev helper): build deps ==="
sudo apt-get update
sudo apt-get install -y --no-install-recommends git cmake g++ default-jdk libssl-dev libasio-dev libtinyxml2-dev

if [ ! -d "$PREFIX/include/fastdds" ]; then
  clone foonathan_memory_vendor "$FOONATHAN_TAG" foonathan "$FOONATHAN_COMMIT"; cmake_install foonathan -DBUILD_SHARED_LIBS=ON
  clone Fast-CDR "$FASTCDR_TAG" fastcdr "$FASTCDR_COMMIT";                       cmake_install fastcdr
  clone Fast-DDS "$FASTDDS_TAG" fastdds "$FASTDDS_COMMIT";                       cmake_install fastdds -DCOMPILE_EXAMPLES=OFF -DBUILD_TESTING=OFF
fi
export LD_LIBRARY_PATH="$PREFIX/lib:${LD_LIBRARY_PATH:-}"

GEN="$PREFIX/share/fastddsgen/scripts/fastddsgen"
if [ ! -x "$GEN" ]; then
  clone Fast-DDS-Gen "$FASTDDSGEN_TAG" gen "$FASTDDSGEN_COMMIT"
  ( cd "$WORK/gen" && git submodule update --init --recursive && ./gradlew assemble --no-daemon --max-workers=2 )
  mkdir -p "$PREFIX/share/fastddsgen"; cp -r "$WORK/gen/scripts" "$WORK/gen/share" "$PREFIX/share/fastddsgen/"
fi
sudo ln -sf "$GEN" /usr/local/bin/fastddsgen

echo "=== build fastdds_robot ==="
cmake -S "$HERE/fastdds" -B "$HERE/fastdds/build" -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="$PREFIX"
cmake --build "$HERE/fastdds/build" --parallel
echo "  fastcdr ${FASTCDR_TAG} / fastdds ${FASTDDS_TAG} / gen ${FASTDDSGEN_TAG} installed under $PREFIX"
echo "  run: FASTDDS_ROBOT=$HERE/fastdds/build/fastdds_robot LD_LIBRARY_PATH=$PREFIX/lib tests/interop/ci/run_vendor.sh fastdds <out>"
