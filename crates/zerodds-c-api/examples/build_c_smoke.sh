#!/usr/bin/env bash
# Builds the C smoke test against the staticlib or cdylib.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../../.." && pwd)"
cd "$REPO"

echo "[c-smoke] cargo build --release -p zerodds-c-api"
cargo build --release -p zerodds-c-api

LIB_DIR="$REPO/target/release"

# OS-spezifische Linker-Flags.
case "$(uname -s)" in
    Linux)  LINK_FLAGS="-L $LIB_DIR -lzerodds -lpthread -ldl -lm";;
    Darwin) LINK_FLAGS="-L $LIB_DIR -lzerodds -lpthread -ldl -lm \
                        -framework CoreFoundation -framework Security";;
    *) echo "unsupported OS: $(uname -s)" >&2; exit 1;;
esac

echo "[c-smoke] gcc -I$REPO/crates/zerodds-c-api/include ..."
gcc -O2 -Wall -Wextra \
    -I "$REPO/crates/zerodds-c-api/include" \
    -o /tmp/zerodds_c_smoke \
    "$REPO/crates/zerodds-c-api/examples/c_smoke.c" \
    $LINK_FLAGS

echo "[c-smoke] running /tmp/zerodds_c_smoke"
LD_LIBRARY_PATH="$LIB_DIR" /tmp/zerodds_c_smoke
