#!/usr/bin/env bash
# tests/interop/shapes_zerodds_e2e.sh — WP 2.1 stage 1
#
# Cross-process ZeroDDS ↔ ZeroDDS test over the ShapesDemo schema.
# Ensures that our encoder/decoder for `ShapeType` can talk to each
# other in two separate processes (not only an in-process test).
# This is the precondition for the next step:
# ZeroDDS ↔ Cyclone Python ShapesDemo client.
#
# Platform: Linux (multicast loopback works between two
# separate processes on the same host). Possibly unstable on macOS.
#
# Usage:
#   ./tests/interop/shapes_zerodds_e2e.sh
#
# Parameters via env:
#   RUNTIME_SECS   — test duration in seconds (default: 8)
#   MIN_SAMPLES    — minimum number of delivered samples (default: 3)
#   TOPIC          — Square/Circle/Triangle (default: Square)
#   COLOR          — color of the publisher sample (default: BLUE)

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
OUTDIR="${OUTDIR:-$HERE/shapes-e2e-out}"
RUNTIME_SECS="${RUNTIME_SECS:-8}"
MIN_SAMPLES="${MIN_SAMPLES:-3}"
TOPIC="${TOPIC:-Square}"
COLOR="${COLOR:-BLUE}"

mkdir -p "$OUTDIR"
PUB_LOG="$OUTDIR/pub.log"
SUB_LOG="$OUTDIR/sub.log"
: >"$PUB_LOG"
: >"$SUB_LOG"

echo "[shapes-e2e] cargo build -p dds-dcps --examples"
if ! cargo build -p dds-dcps --examples 2>&1 | tee "$OUTDIR/build.log" >/dev/null; then
    echo "[shapes-e2e] build failed — see $OUTDIR/build.log" >&2
    exit 2
fi

PUB_BIN="$REPO/target/debug/examples/shapes_demo_publisher"
SUB_BIN="$REPO/target/debug/examples/shapes_demo_subscriber"

if [ ! -x "$PUB_BIN" ] || [ ! -x "$SUB_BIN" ]; then
    echo "[shapes-e2e] example binaries missing" >&2
    exit 2
fi

echo "[shapes-e2e] starting subscriber (background, Topic=$TOPIC)"
"$SUB_BIN" "$TOPIC" 0 >"$SUB_LOG" 2>&1 &
SUB_PID=$!
trap 'kill "$SUB_PID" "$PUB_PID" 2>/dev/null || true' EXIT

sleep 1 # the subscriber must have joined the multicast group

echo "[shapes-e2e] starting publisher (Topic=$TOPIC, Color=$COLOR, ${RUNTIME_SECS}s)"
"$PUB_BIN" "$TOPIC" "$COLOR" 0 >"$PUB_LOG" 2>&1 &
PUB_PID=$!

sleep "$RUNTIME_SECS"

kill "$PUB_PID" 2>/dev/null || true
kill "$SUB_PID" 2>/dev/null || true
wait 2>/dev/null || true

# Count lines with "  <- color=$COLOR" in the subscriber log
COUNT=$(grep -c "<- color=${COLOR}" "$SUB_LOG" || true)
echo "[shapes-e2e] subscriber received $COUNT samples with color=$COLOR (min: $MIN_SAMPLES)"
echo "[shapes-e2e] subscriber log: $SUB_LOG"
echo "[shapes-e2e] publisher log:  $PUB_LOG"

if [ "$COUNT" -lt "$MIN_SAMPLES" ]; then
    echo "[shapes-e2e] FAIL: only $COUNT < $MIN_SAMPLES samples delivered" >&2
    echo "--- sub.log (last 30 lines) ---" >&2
    tail -n 30 "$SUB_LOG" >&2
    echo "--- pub.log (last 30 lines) ---" >&2
    tail -n 30 "$PUB_LOG" >&2
    exit 1
fi

echo "[shapes-e2e] OK"
