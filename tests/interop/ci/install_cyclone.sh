#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# NON-GATING LEGACY DEV HELPER. The required #28 CI gate does NOT use this
# script — it runs on the pinned `ci-interop` image (ci/Dockerfile.interop).
# This helper only reproduces the CycloneDDS side locally.
#
# Canonical CycloneDDS: cyclonedds==11.0.1 — a self-contained manylinux wheel
# that bundles libddsc 11.0.1 (no separate C-library build). This matches the
# version verified on our Linux CI host and baked into the ci-interop image.
set -euo pipefail

CYCLONEDDS_VERSION="${CYCLONEDDS_VERSION:-11.0.1}"
VENV="${CYCLONEDDS_VENV:-$HOME/cyclonedds-venv}"

echo "=== install_cyclone (legacy dev helper): cyclonedds==${CYCLONEDDS_VERSION} wheel ==="
python3 -m venv "$VENV"
"$VENV/bin/pip" install --upgrade pip
"$VENV/bin/pip" install "cyclonedds==${CYCLONEDDS_VERSION}"
"$VENV/bin/python3" -c "from cyclonedds.domain import DomainParticipant; DomainParticipant(0); print(\"  cyclonedds runtime OK (venv: $VENV)\")"
echo "  set PYBIN=$VENV/bin/python3 when running tests/interop/ci/run_vendor.sh cyclone"
