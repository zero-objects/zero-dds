#!/usr/bin/env bash
# tests/interop/shapes_cyclone_interop.sh — WP 2.1 stage 1
#
# Cross-vendor interop test: our ShapesDemo subscriber against a
# Cyclone DDS Python publisher, and conversely our publisher against a
# Cyclone subscriber. Validates that our XCDR2-LE encoding is wire-
# compatible with Cyclone.
#
# Platform: Linux (Docker + multicast host mode). NOT macOS.
#
# Usage:
#   ./tests/interop/shapes_cyclone_interop.sh [direction]
#
# direction:
#   pub2sub   — ZeroDDS pub → Cyclone sub
#   sub2pub   — Cyclone pub → ZeroDDS sub
#   both      — both (default)

set -euo pipefail

# Empty ACKNACK of a reader that holds nothing: Fast DDS (and, as recorded, OpenDDS
# and RTI Connext) send it with bitmapBase 0 and numBits 0, which DDSI-RTPS 2.5
# section 8.3.5.5 makes invalid; ZeroDDS by default drops the rest of such a
# datagram (section 8.3.4.1 rule 6), and with it the SEDP data that follows in
# it (measured: sec-diag16/17, every red zerodds->fastdds cell had rejections
# with left_out > 0, every green cell none). The ZeroDDS processes of the
# vendor pairs, writers and readers, therefore accept it
# (wire.accept_empty_acknack_base_zero). This is the only place the runner sets it.
export ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO=1
# PL_CDR1 parameters with the rounded length. Cyclone DDS (extended header) and
# RTI Connext (short header) write the length of a PL_CDR1 parameter rounded up
# to four, the form of DDSI-RTPS 2.5 section 9.4.2.11; XTypes 1.3 section
# 7.4.1.2.2 has the exact length and the default reader of ZeroDDS refuses the
# other form. This runner meets Cyclone DDS, so every pair reads tolerantly; the vendors whose writers use the rounded form (taken from the
# recorded rows: ast-layout Dialect::Cyclone and Dialect::Rti,
# pads_declared_length) get a ZeroDDS reader with wire.pl_cdr1_reading=tolerant;
# Fast DDS and OpenDDS write the exact length. This is the only place the runner
# sets it.
export ZERODDS_WIRE_PL_CDR1_READING=tolerant

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
OUTDIR="${OUTDIR:-$HERE/shapes-cyclone-out}"
RUNTIME_SECS="${RUNTIME_SECS:-10}"
MIN_SAMPLES="${MIN_SAMPLES:-3}"
DIRECTION="${1:-both}"

mkdir -p "$OUTDIR"

if [ "$(uname -s)" != "Linux" ]; then
    echo "[shapes-cyclone] WARNING: Linux recommended for multicast loopback."
fi

if ! command -v docker >/dev/null; then
    echo "[shapes-cyclone] ERROR: docker not installed" >&2
    exit 2
fi

if ! (docker compose version >/dev/null 2>&1 || docker-compose version >/dev/null 2>&1); then
    echo "[shapes-cyclone] ERROR: docker compose (v2) or docker-compose (v1) required" >&2
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
    # Follow the subscriber log in the container
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
    echo "[shapes-cyclone] pub2sub: Cyclone sub received $count samples (min=$MIN_SAMPLES)"
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
    echo "[shapes-cyclone] sub2pub: ZeroDDS sub received $count samples (min=$MIN_SAMPLES)"
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
