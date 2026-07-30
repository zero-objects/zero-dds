#!/usr/bin/env bash
#
# Installed-SDK consumer gate for the zerodds CMake package (spec P0-4/P0-5).
#
# Builds zerodds from source, `cmake --install`s it into a throwaway staging
# tree, then configures + builds a standalone consumer project that only ever
# sees that staging tree (find_package(zerodds CONFIG REQUIRED)). This is the
# single-staging-tree contract end to end: if any header / lib / config file is
# missing from the install, the consumer configure or build fails here.
#
# By default it installs BOTH library flavours into ONE prefix (static, then
# shared) to prove they coexist and that zeroddsConfig.cmake selects one
# correctly. Needs cmake + cargo on PATH. Exit 0 = pass.
#
# Usage:  cmake/tests/run-sdk-consumer.sh
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
consumer="$here/sdk-consumer"

work="$(mktemp -d "${TMPDIR:-/tmp}/zerodds-sdk-consumer.XXXXXX")"
trap 'rm -rf "$work"' EXIT

stage="$work/stage"

echo "== build + install zerodds (static) into staging tree =="
cmake -S "$repo" -B "$work/build-static" \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=OFF \
    -DZERODDS_BUILD_CPP=ON \
    -DZERODDS_BUILD_IDLC=ON \
    -DZERODDS_BUILD_EXAMPLES=OFF \
    -DZERODDS_BUILD_TESTS=OFF
cmake --build "$work/build-static"
cmake --install "$work/build-static" --prefix "$stage"

echo "== build + install zerodds (shared) into the SAME staging tree =="
cmake -S "$repo" -B "$work/build-shared" \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=ON \
    -DZERODDS_BUILD_CPP=ON \
    -DZERODDS_BUILD_IDLC=ON \
    -DZERODDS_BUILD_EXAMPLES=OFF \
    -DZERODDS_BUILD_TESTS=OFF
cmake --build "$work/build-shared"
cmake --install "$work/build-shared" --prefix "$stage"

echo "== staging tree contents =="
find "$stage" -type f -o -type l | sort

echo "== configure + build consumer against staging tree ONLY =="
cmake -S "$consumer" -B "$work/consumer" \
    -DCMAKE_PREFIX_PATH="$stage"
cmake --build "$work/consumer"

echo "OK: SDK consumer configured + built against the install staging tree"
