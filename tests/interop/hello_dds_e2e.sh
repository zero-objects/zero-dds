#!/usr/bin/env bash
# tests/interop/hello_dds_e2e.sh — WP 2.1 Phase C3
#
# Cross-process Smoke-Test fuer die DCPS-Public-API: startet den
# Hello-World-Subscriber im Hintergrund, dann den Publisher, laesst
# sie 10 s miteinander reden und prueft, ob der Subscriber
# mindestens 3 "hello #"-Zeilen erhalten hat.
#
# Plattform: Linux (Multicast-Loopback). Auf macOS kann das je nach
# OS-Version instabil sein — nutze dort den in-process Integration-
# Test in crates/dcps/tests/e2e_dcps_api.rs.
#
# Verwendung:
#   ./tests/interop/hello_dds_e2e.sh
#
# Exit-Codes:
#   0  mind. 3 Samples empfangen
#   1  Discovery- oder Delivery-Failure
#   2  Build-Fehler

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

# Dem Subscriber einen Moment geben, SPDP-Beacon-Slot zu erwischen.
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
