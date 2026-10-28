#!/usr/bin/env bash
# tests/interop/matrix.sh
#
# WP 1.11 T2 — pub/sub matrix test between ZeroDDS, Cyclone DDS and
# eProsima Fast-DDS.
#
# Must run on a Linux host with Docker (multicast between
# container and host works only with `network_mode: host` on
# Linux, not on macOS Docker Desktop).
#
# Procedure:
# 1. Starts Cyclone + Fast-DDS publisher containers.
# 2. Runs the ZeroDDS spdp_demo for 15 s.
# 3. Counts foreign-vendor discovered events in the output.
# 4. Expectation: at least 1 foreign vendor per run.

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
OUTDIR="${OUTDIR:-$HERE/matrix-out}"
mkdir -p "$OUTDIR"

echo "[matrix] docker compose up -d cyclone-pub fastdds-pub"
( cd "$HERE" && docker compose up -d cyclone-pub fastdds-pub ) \
  | tee "$OUTDIR/compose.log"

# Build + run zerodds demo on host.
echo "[matrix] cargo build --example spdp_demo -p dds-discovery"
cargo build --example spdp_demo -p dds-discovery 2>&1 \
  | tee "$OUTDIR/build.log"

echo "[matrix] running ZeroDDS spdp_demo 15s"
timeout 15s "$REPO/target/debug/examples/spdp_demo" \
  2>&1 | tee "$OUTDIR/demo.out" || true

echo "[matrix] docker compose down"
( cd "$HERE" && docker compose down ) | tee -a "$OUTDIR/compose.log"

# Success criterion: at least 1 foreign vendor discovered.
FOREIGN=$(grep -c '✓ DISCOVERED' "$OUTDIR/demo.out" \
  | head -1 || echo 0)
# Count only non-ZeroDDS (VendorId != [1, 240]).
REAL=$(grep '✓ DISCOVERED' "$OUTDIR/demo.out" \
  | grep -v 'vendor=VendorId(\[1, 240\])' | wc -l)
echo "[matrix] total discoveries=$FOREIGN, foreign=$REAL"

if [ "$REAL" -lt 1 ]; then
    echo "[matrix] FAIL: no foreign vendors discovered"
    exit 1
fi
echo "[matrix] OK: $REAL foreign vendor(s) discovered"
