#!/usr/bin/env bash
# tests/interop/shapes_zerodds_e2e.sh — WP 2.1 Stufe 1
#
# Cross-Process ZeroDDS ↔ ZeroDDS Test ueber das ShapesDemo-Schema.
# Stellt sicher, dass unser Encoder/Decoder fuer `ShapeType` in zwei
# separaten Prozessen (nicht nur in-process Test) miteinander reden
# koennen. Das ist die Voraussetzung fuer den naechsten Schritt:
# ZeroDDS ↔ Cyclone-Python-ShapesDemo-Client.
#
# Plattform: Linux (Multicast-Loopback funktioniert zwischen zwei
# separaten Prozessen auf demselben Host). Auf macOS ggf. instabil.
#
# Verwendung:
#   ./tests/interop/shapes_zerodds_e2e.sh
#
# Parameter via env:
#   RUNTIME_SECS   — Test-Dauer in Sekunden (default: 8)
#   MIN_SAMPLES    — Mindest-Anzahl zugestellter Samples (default: 3)
#   TOPIC          — Square/Circle/Triangle (default: Square)
#   COLOR          — Farbe des Publisher-Samples (default: BLUE)

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

sleep 1 # Subscriber muss die Multicast-Gruppe gejoint haben

echo "[shapes-e2e] starting publisher (Topic=$TOPIC, Color=$COLOR, ${RUNTIME_SECS}s)"
"$PUB_BIN" "$TOPIC" "$COLOR" 0 >"$PUB_LOG" 2>&1 &
PUB_PID=$!

sleep "$RUNTIME_SECS"

kill "$PUB_PID" 2>/dev/null || true
kill "$SUB_PID" 2>/dev/null || true
wait 2>/dev/null || true

# Subscriber-Log zaehlt Zeilen mit "  <- color=$COLOR"
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
