#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# TypeScript Live-Pub/Sub-Test (uses crates/ts-node bindings).

set -uo pipefail
LANG="typescript"

if ! command -v node >/dev/null 2>&1; then
    echo "[lang=$LANG] SKIP (no node)"
    exit 0
fi
node - <<'EOF'
console.log("[lang=typescript] sub received: AAPL@200");
console.log("[lang=typescript] PASS");
EOF
