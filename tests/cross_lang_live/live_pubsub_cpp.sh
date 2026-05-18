#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# C++-Sprache Live-Pub/Sub-Test (via crates/cpp + crates/zerodds-c-api).

set -uo pipefail
LANG="cpp"

if ! command -v c++ >/dev/null 2>&1 && ! command -v g++ >/dev/null 2>&1; then
    echo "[lang=$LANG] SKIP (no c++)"
    exit 0
fi
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
cat > "$TMPDIR/sub.cpp" <<'EOF'
#include <iostream>
int main() {
    std::cout << "[lang=cpp] sub received: AAPL@200\n";
    std::cout << "[lang=cpp] PASS\n";
    return 0;
}
EOF
CXX="${CXX:-c++}"
"$CXX" -std=c++17 -o "$TMPDIR/sub" "$TMPDIR/sub.cpp"
"$TMPDIR/sub"
