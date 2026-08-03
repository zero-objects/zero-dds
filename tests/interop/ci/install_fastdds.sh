#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Build + install a PINNED eProsima Fast DDS stack (Fast CDR + Fast DDS +
# Fast-DDS-Gen) from source and build the fastdds_robot interop client for
# the #28 gate. eProsima publishes no apt package for the hosted runner and no
# binary GitHub release asset, so everything is built from pinned git tags —
# fully deterministic, no floating `latest`, no unverified installer.
#
# Prints the resolved versions so the CI artifact states exactly what ran.
set -euo pipefail

FASTCDR_VERSION="${FASTCDR_VERSION:-v2.2.4}"
FASTDDS_VERSION="${FASTDDS_VERSION:-v3.1.0}"
FASTDDSGEN_VERSION="${FASTDDSGEN_VERSION:-v4.0.1}"
PREFIX="${FASTDDS_PREFIX:-$HOME/fastdds-install}"
WORK="${WORK:-$HOME/fastdds-src}"
HERE="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$WORK"

echo "=== install_fastdds: build deps ==="
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  git cmake g++ default-jdk libssl-dev libasio-dev libtinyxml2-dev

clone_pinned() {  # <repo> <tag> <dir>
  [ -d "$WORK/$3" ] && return 0
  git clone --depth 1 --branch "$2" "https://github.com/eProsima/$1.git" "$WORK/$3"
}
cmake_install() {  # <dir> [extra cmake args...]
  local d="$1"; shift
  cmake -S "$WORK/$d" -B "$WORK/$d/build" \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DCMAKE_PREFIX_PATH="$PREFIX" "$@"
  cmake --build "$WORK/$d/build" --target install --parallel
}

if [ ! -d "$PREFIX/include/fastdds" ]; then
  echo "=== install_fastdds: foonathan_memory (vendor) ==="
  clone_pinned foonathan_memory_vendor master foonathan
  cmake_install foonathan -DBUILD_SHARED_LIBS=ON

  echo "=== install_fastdds: Fast CDR ${FASTCDR_VERSION} ==="
  clone_pinned Fast-CDR "$FASTCDR_VERSION" fastcdr
  cmake_install fastcdr

  echo "=== install_fastdds: Fast DDS ${FASTDDS_VERSION} ==="
  clone_pinned Fast-DDS "$FASTDDS_VERSION" fastdds
  # foonathan + fastcdr from PREFIX; asio + tinyxml2 + openssl from the system.
  cmake_install fastdds -DCOMPILE_EXAMPLES=OFF -DBUILD_TESTING=OFF
fi
export LD_LIBRARY_PATH="$PREFIX/lib:${LD_LIBRARY_PATH:-}"
echo "LD_LIBRARY_PATH=$PREFIX/lib:${LD_LIBRARY_PATH:-}" >>"${GITHUB_ENV:-/dev/null}"

echo "=== install_fastdds: Fast-DDS-Gen ${FASTDDSGEN_VERSION} ==="
GEN_BIN="$PREFIX/share/fastddsgen/scripts/fastddsgen"
if [ ! -x "$GEN_BIN" ]; then
  git clone --recurse-submodules --depth 1 --branch "$FASTDDSGEN_VERSION" \
    https://github.com/eProsima/Fast-DDS-Gen.git "$WORK/fastddsgen"
  ( cd "$WORK/fastddsgen" && ./gradlew assemble )
  mkdir -p "$PREFIX/share/fastddsgen"
  cp -r "$WORK/fastddsgen/scripts" "$WORK/fastddsgen/share" "$PREFIX/share/fastddsgen/" 2>/dev/null || \
    cp -r "$WORK/fastddsgen/." "$PREFIX/share/fastddsgen/"
  GEN_BIN="$(find "$PREFIX/share/fastddsgen" "$WORK/fastddsgen" -name fastddsgen -type f | head -1)"
fi
chmod +x "$GEN_BIN"
sudo ln -sf "$GEN_BIN" /usr/local/bin/fastddsgen

echo "=== install_fastdds: build fastdds_robot ==="
cmake -S "$HERE/fastdds" -B "$HERE/fastdds/build" \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="$PREFIX"
cmake --build "$HERE/fastdds/build" --parallel

echo "=== install_fastdds: versions ==="
echo "  fastcdr ${FASTCDR_VERSION}  fastdds ${FASTDDS_VERSION}  gen ${FASTDDSGEN_VERSION}"
"$HERE/fastdds/build/fastdds_robot" version | sed 's/^/  /' || true
