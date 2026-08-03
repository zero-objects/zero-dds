#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Install a pinned CycloneDDS + Python binding for the #28 interop gate on a
# hosted Ubuntu runner. The CycloneDDS C library comes from the runner image's
# apt archive (deterministic for a pinned `runs-on: ubuntu-24.04` image), the
# Python binding is pinned to a matching release.
#
# Prints the resolved versions so the CI artifact states exactly what ran.
set -euo pipefail

CYCLONEDDS_PY_VERSION="${CYCLONEDDS_PY_VERSION:-0.10.5}"

echo "=== install_cyclone: apt CycloneDDS C library ==="
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  libcyclonedds-dev cyclonedds-tools python3-pip

# The Python binding builds against an installed ddsc; apt puts it under /usr.
export CYCLONEDDS_HOME=/usr
echo "CYCLONEDDS_HOME=$CYCLONEDDS_HOME" >>"${GITHUB_ENV:-/dev/null}"

echo "=== install_cyclone: Python binding cyclonedds==${CYCLONEDDS_PY_VERSION} ==="
python3 -m pip install --user --upgrade pip
CYCLONEDDS_HOME=/usr python3 -m pip install --user "cyclonedds==${CYCLONEDDS_PY_VERSION}"

echo "=== install_cyclone: versions ==="
dpkg -s libcyclonedds-dev 2>/dev/null | sed -n 's/^Version: /  libcyclonedds-dev /p' || true
python3 -c 'import cyclonedds; print("  cyclonedds-python", getattr(cyclonedds, "__version__", "?"))'
python3 -c 'from cyclonedds.domain import DomainParticipant; DomainParticipant(0); print("  cyclonedds runtime OK")'
