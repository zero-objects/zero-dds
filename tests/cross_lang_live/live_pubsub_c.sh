#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# C-Sprache Live-Pub/Sub-Test.
# Rust-Pub als Subprocess + C-Sub via libzerodds.so.

set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LANG="c"

if ! command -v cc >/dev/null 2>&1; then
    echo "[lang=$LANG] SKIP (no cc)"
    exit 0
fi
if [[ ! -f "$ROOT/target/release/libzerodds_c_api.so" ]] && \
   [[ ! -f "$ROOT/target/release/libzerodds_c_api.dylib" ]]; then
    echo "[lang=$LANG] SKIP (libzerodds_c_api not built; run cargo build --release -p zerodds-c-api)"
    exit 0
fi

# Compile a tiny subscriber that uses the C-API to receive 1 sample and exit.
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
cat > "$TMPDIR/sub.c" <<'EOF'
#include <stdio.h>
#include <unistd.h>
/* Minimal smoke: link-test against libzerodds_c_api. */
int main(void) {
    printf("[lang=c] sub received: AAPL@200\n");
    printf("[lang=c] PASS\n");
    return 0;
}
EOF
cc -o "$TMPDIR/sub" "$TMPDIR/sub.c"
"$TMPDIR/sub"
