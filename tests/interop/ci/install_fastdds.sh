#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Install a pinned eProsima Fast DDS + Fast-DDS-Gen and build the fastdds_robot
# interop client for the #28 gate on a hosted Ubuntu runner.
#
# Fast DDS is installed from the official versioned .deb bundle published on the
# eProsima GitHub release; Fast-DDS-Gen from its official versioned release.
# Both are pinned by tag — never `latest`, never an unverified curl|sh.
#
# Prints the resolved versions so the CI artifact states exactly what ran.
set -euo pipefail

FASTDDS_VERSION="${FASTDDS_VERSION:-3.1.0}"
FASTDDSGEN_VERSION="${FASTDDSGEN_VERSION:-4.0.1}"
HERE="$(cd "$(dirname "$0")" && pwd)"
WORK="${WORK:-$HOME/fastdds-install}"
mkdir -p "$WORK"

echo "=== install_fastdds: build deps ==="
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  cmake g++ default-jre wget ca-certificates

echo "=== install_fastdds: Fast DDS ${FASTDDS_VERSION} (.deb bundle) ==="
DEB_TGZ="ubuntu-fastdds-${FASTDDS_VERSION}.tgz"
DEB_URL="https://github.com/eProsima/Fast-DDS/releases/download/v${FASTDDS_VERSION}/${DEB_TGZ}"
wget -q -O "$WORK/$DEB_TGZ" "$DEB_URL"
tar -xzf "$WORK/$DEB_TGZ" -C "$WORK"
# The bundle unpacks a set of .deb packages (fastcdr, fastdds, foonathan_memory).
sudo apt-get install -y "$WORK"/*.deb || {
  # Fall back to dpkg + fix-broken if apt cannot resolve the local files.
  sudo dpkg -i "$WORK"/*.deb || true
  sudo apt-get install -y -f
}
sudo ldconfig

echo "=== install_fastdds: Fast-DDS-Gen ${FASTDDSGEN_VERSION} ==="
GEN_URL="https://github.com/eProsima/Fast-DDS-Gen/releases/download/v${FASTDDSGEN_VERSION}/fastddsgen.tar.gz"
wget -q -O "$WORK/fastddsgen.tar.gz" "$GEN_URL"
tar -xzf "$WORK/fastddsgen.tar.gz" -C "$WORK"
GEN_BIN="$(find "$WORK" -type f -name fastddsgen | head -1)"
[ -n "$GEN_BIN" ] || { echo "fastddsgen binary not found after extract" >&2; exit 1; }
chmod +x "$GEN_BIN"
sudo ln -sf "$GEN_BIN" /usr/local/bin/fastddsgen

echo "=== install_fastdds: build fastdds_robot ==="
cmake -S "$HERE/fastdds" -B "$HERE/fastdds/build" -DCMAKE_BUILD_TYPE=Release
cmake --build "$HERE/fastdds/build" --parallel

echo "=== install_fastdds: versions ==="
echo "  fast-dds .deb bundle v${FASTDDS_VERSION}"
fastddsgen -version 2>/dev/null | sed 's/^/  /' || echo "  fastddsgen (version query unsupported)"
"$HERE/fastdds/build/fastdds_robot" version | sed 's/^/  /' || true
