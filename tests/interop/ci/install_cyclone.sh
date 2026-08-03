#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Build + install a PINNED CycloneDDS and its Python binding for the #28
# interop gate. Neither Eclipse nor Ubuntu ships an apt package for the hosted
# runner, so the C library is built from a pinned git tag and the Python
# binding from a matching PyPI release — fully deterministic, no floating
# `latest`, no unverified third-party installer.
#
# Prints the resolved versions so the CI artifact states exactly what ran.
set -euo pipefail

CYCLONEDDS_VERSION="${CYCLONEDDS_VERSION:-0.10.5}"       # C library git tag
CYCLONEDDS_PY_VERSION="${CYCLONEDDS_PY_VERSION:-0.10.5}" # matching PyPI release
PREFIX="${CYCLONEDDS_HOME:-$HOME/cyclonedds-install}"
WORK="${WORK:-$HOME/cyclonedds-src}"

echo "=== install_cyclone: build deps ==="
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  git cmake g++ python3-dev python3-pip

echo "=== install_cyclone: build CycloneDDS ${CYCLONEDDS_VERSION} (source) ==="
if [ ! -x "$PREFIX/bin/idlc" ]; then
  rm -rf "$WORK"
  git clone --depth 1 --branch "$CYCLONEDDS_VERSION" \
    https://github.com/eclipse-cyclonedds/cyclonedds.git "$WORK"
  cmake -S "$WORK" -B "$WORK/build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DBUILD_IDLC=ON -DBUILD_EXAMPLES=OFF -DBUILD_TESTING=OFF
  cmake --build "$WORK/build" --target install --parallel
fi
export CYCLONEDDS_HOME="$PREFIX"
{
  echo "CYCLONEDDS_HOME=$PREFIX"
  echo "LD_LIBRARY_PATH=$PREFIX/lib:${LD_LIBRARY_PATH:-}"
} >>"${GITHUB_ENV:-/dev/null}"
export LD_LIBRARY_PATH="$PREFIX/lib:${LD_LIBRARY_PATH:-}"

echo "=== install_cyclone: Python binding cyclonedds==${CYCLONEDDS_PY_VERSION} ==="
python3 -m pip install --user --upgrade pip
CYCLONEDDS_HOME="$PREFIX" python3 -m pip install --user "cyclonedds==${CYCLONEDDS_PY_VERSION}"

echo "=== install_cyclone: versions ==="
echo "  cyclonedds-c ${CYCLONEDDS_VERSION} (prefix $PREFIX)"
python3 -c 'import cyclonedds; print("  cyclonedds-python", getattr(cyclonedds, "__version__", "?"))'
python3 -c 'from cyclonedds.domain import DomainParticipant; DomainParticipant(0); print("  cyclonedds runtime OK")'
