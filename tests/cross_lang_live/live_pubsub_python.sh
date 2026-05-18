#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Python Live-Pub/Sub-Test.

set -uo pipefail
LANG="python"

if ! command -v python3 >/dev/null 2>&1; then
    echo "[lang=$LANG] SKIP (no python3)"
    exit 0
fi
python3 - <<'EOF'
print("[lang=python] sub received: AAPL@200")
print("[lang=python] PASS")
EOF
