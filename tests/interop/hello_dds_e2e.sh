#!/usr/bin/env bash
# tests/interop/hello_dds_e2e.sh — WP 2.1 Phase C3
#
# Cross-process smoke test for the DCPS public API: starts the
# hello-world subscriber in the background, then the publisher, lets
# them talk to each other for 10 s and checks whether the subscriber
# received at least 3 "hello #" lines.
#
# Platform: Linux (multicast loopback). On macOS this can be unstable
# depending on the OS version — use the in-process integration
# test in crates/dcps/tests/e2e_dcps_api.rs there.
#
# Usage:
#   ./tests/interop/hello_dds_e2e.sh
#
# Exit codes:
#   0  at least 3 samples received
#   1  discovery or delivery failure
#   2  build error

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
OUTDIR="${OUTDIR:-$HERE/hello-e2e-out}"
RUNTIME_SECS="${RUNTIME_SECS:-10}"
MIN_SAMPLES="${MIN_SAMPLES:-3}"

mkdir -p "$OUTDIR"
SUB_LOG="$OUTDIR/sub.log"
PUB_LOG="$OUTDIR/pub.log"
: >"$SUB_LOG"
: >"$PUB_LOG"

echo "[hello-e2e] cargo build -p dds-dcps --examples"
if ! cargo build -p dds-dcps --examples 2>&1 | tee "$OUTDIR/build.log"; then
    echo "[hello-e2e] build failed" >&2
    exit 2
fi

SUB_BIN="$REPO/target/debug/examples/hello_dds_subscriber"
PUB_BIN="$REPO/target/debug/examples/hello_dds_publisher"

if [ ! -x "$SUB_BIN" ] || [ ! -x "$PUB_BIN" ]; then
    echo "[hello-e2e] example binaries missing: $SUB_BIN / $PUB_BIN" >&2
    exit 2
fi

echo "[hello-e2e] starting subscriber (background)"
"$SUB_BIN" >"$SUB_LOG" 2>&1 &
SUB_PID=$!
trap 'kill "$SUB_PID" 2>/dev/null || true; kill "$PUB_PID" 2>/dev/null || true' EXIT

# Give the subscriber a moment to catch an SPDP beacon slot.
sleep 1

echo "[hello-e2e] starting publisher (background, ${RUNTIME_SECS}s)"
"$PUB_BIN" >"$PUB_LOG" 2>&1 &
PUB_PID=$!

sleep "$RUNTIME_SECS"

kill "$PUB_PID" 2>/dev/null || true
kill "$SUB_PID" 2>/dev/null || true
wait 2>/dev/null || true

COUNT=$(grep -c '^  <- hello #' "$SUB_LOG" || true)
echo "[hello-e2e] subscriber received $COUNT samples (min: $MIN_SAMPLES)"
echo "[hello-e2e] subscriber log: $SUB_LOG"
echo "[hello-e2e] publisher log:  $PUB_LOG"

if [ "$COUNT" -lt "$MIN_SAMPLES" ]; then
    echo "[hello-e2e] FAIL: only $COUNT < $MIN_SAMPLES samples delivered"
    echo "--- sub.log (last 20 lines) ---"
    tail -n 20 "$SUB_LOG" >&2
    echo "--- pub.log (last 20 lines) ---"
    tail -n 20 "$PUB_LOG" >&2
    exit 1
fi

echo "[hello-e2e] OK"
