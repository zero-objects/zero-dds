#!/usr/bin/env bash
# tests/interop/shapes_cyclone_interop.sh — WP 2.1 Stufe 1
#
# Cross-Vendor-Interop-Test: unser ShapesDemo-Subscriber gegen einen
# Cyclone-DDS-Python-Publisher, und umgekehrt unser Publisher gegen
# Cyclone-Subscriber. Validiert, dass unsere XCDR2-LE-Encoding Wire-
# kompatibel ist mit Cyclone.
#
# Plattform: Linux (Docker + Multicast-Host-Mode). macOS NICHT.
#
# Verwendung:
#   ./tests/interop/shapes_cyclone_interop.sh [direction]
#
# direction:
#   pub2sub   — ZeroDDS-Pub → Cyclone-Sub
#   sub2pub   — Cyclone-Pub → ZeroDDS-Sub
#   both      — beide (default)

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
OUTDIR="${OUTDIR:-$HERE/shapes-cyclone-out}"
RUNTIME_SECS="${RUNTIME_SECS:-10}"
MIN_SAMPLES="${MIN_SAMPLES:-3}"
DIRECTION="${1:-both}"

mkdir -p "$OUTDIR"

if [ "$(uname -s)" != "Linux" ]; then
    echo "[shapes-cyclone] WARNING: Linux empfohlen fuer Multicast-Loopback."
fi

if ! command -v docker >/dev/null; then
    echo "[shapes-cyclone] ERROR: docker nicht installiert" >&2
    exit 2
fi

if ! (docker compose version >/dev/null 2>&1 || docker-compose version >/dev/null 2>&1); then
    echo "[shapes-cyclone] ERROR: docker compose (v2) oder docker-compose (v1) benoetigt" >&2
    exit 2
fi
COMPOSE="docker compose"
if ! docker compose version >/dev/null 2>&1; then
    COMPOSE="docker-compose"
fi

cd "$HERE"

echo "[shapes-cyclone] building Cyclone-Python image"
$COMPOSE build cyclone-shapes-pub cyclone-shapes-sub 2>&1 | tail -5

echo "[shapes-cyclone] building ZeroDDS shapes_demo_* examples"
( cd "$REPO" && cargo build -p dds-dcps --examples 2>&1 | tail -3 )

ZERO_PUB="$REPO/target/debug/examples/shapes_demo_publisher"
ZERO_SUB="$REPO/target/debug/examples/shapes_demo_subscriber"

run_pub2sub() {
    echo "[shapes-cyclone] ===== pub2sub: ZeroDDS-Pub → Cyclone-Sub ====="
    local log="$OUTDIR/pub2sub-cyclone.log"
    $COMPOSE up -d cyclone-shapes-sub >/dev/null
    # Subscriber-Log in Container folgen
    $COMPOSE logs -f cyclone-shapes-sub >"$log" 2>&1 &
    local LOG_PID=$!
    sleep 2

    "$ZERO_PUB" Square RED 0 >"$OUTDIR/pub2sub-zerodds.log" 2>&1 &
    local PUB_PID=$!

    sleep "$RUNTIME_SECS"
    kill "$PUB_PID" 2>/dev/null || true
    kill "$LOG_PID" 2>/dev/null || true
    $COMPOSE stop cyclone-shapes-sub >/dev/null
    $COMPOSE rm -f cyclone-shapes-sub >/dev/null

    local count
    count=$(grep -c "color=.*RED" "$log" || true)
    echo "[shapes-cyclone] pub2sub: Cyclone-Sub empfing $count Samples (min=$MIN_SAMPLES)"
    if [ "$count" -lt "$MIN_SAMPLES" ]; then
        echo "[shapes-cyclone] FAIL pub2sub" >&2
        tail -20 "$log" >&2
        return 1
    fi
    return 0
}

run_sub2pub() {
    echo "[shapes-cyclone] ===== sub2pub: Cyclone-Pub → ZeroDDS-Sub ====="
    local log="$OUTDIR/sub2pub-zerodds.log"
    "$ZERO_SUB" Square 0 >"$log" 2>&1 &
    local SUB_PID=$!
    sleep 2

    $COMPOSE up -d cyclone-shapes-pub >/dev/null
    sleep "$RUNTIME_SECS"

    kill "$SUB_PID" 2>/dev/null || true
    $COMPOSE stop cyclone-shapes-pub >/dev/null
    $COMPOSE rm -f cyclone-shapes-pub >/dev/null

    local count
    count=$(grep -c "<- color=.*RED" "$log" || true)
    echo "[shapes-cyclone] sub2pub: ZeroDDS-Sub empfing $count Samples (min=$MIN_SAMPLES)"
    if [ "$count" -lt "$MIN_SAMPLES" ]; then
        echo "[shapes-cyclone] FAIL sub2pub" >&2
        tail -20 "$log" >&2
        return 1
    fi
    return 0
}

rc=0
case "$DIRECTION" in
    pub2sub) run_pub2sub || rc=1 ;;
    sub2pub) run_sub2pub || rc=1 ;;
    both)
        run_pub2sub || rc=1
        run_sub2pub || rc=1
        ;;
    *)
        echo "Unknown direction '$DIRECTION' — use pub2sub/sub2pub/both" >&2
        exit 2
        ;;
esac

if [ "$rc" -eq 0 ]; then
    echo "[shapes-cyclone] OK"
else
    echo "[shapes-cyclone] FAIL" >&2
fi
exit $rc
