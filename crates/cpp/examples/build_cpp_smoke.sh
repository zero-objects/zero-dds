#!/usr/bin/env bash
# Baut den C++-Smoke-Test gegen libzerodds.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../../.." && pwd)"
cd "$REPO"

echo "[cpp-smoke] cargo build --release -p zerodds-c-api"
cargo build --release -p zerodds-c-api

LIB_DIR="$REPO/target/release"

case "$(uname -s)" in
    Linux)  LINK_FLAGS="-L $LIB_DIR -lzerodds -lpthread -ldl -lm";;
    Darwin) LINK_FLAGS="-L $LIB_DIR -lzerodds -lpthread -ldl -lm \
                        -framework CoreFoundation -framework Security";;
    *) echo "unsupported OS: $(uname -s)" >&2; exit 1;;
esac

echo "[cpp-smoke] g++ -std=c++17 ..."
g++ -std=c++17 -O2 -Wall -Wextra \
    -I "$REPO/crates/zerodds-c-api/include" \
    -I "$REPO/crates/cpp/include" \
    -o /tmp/zerodds_cpp_smoke \
    "$REPO/crates/cpp/examples/cpp_smoke.cpp" \
    $LINK_FLAGS

echo "[cpp-smoke] running /tmp/zerodds_cpp_smoke"
LD_LIBRARY_PATH="$LIB_DIR" \
DYLD_LIBRARY_PATH="$LIB_DIR" \
/tmp/zerodds_cpp_smoke
