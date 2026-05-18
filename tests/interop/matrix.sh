#!/usr/bin/env bash
# tests/interop/matrix.sh
#
# WP 1.11 T2 — Pub/Sub-Matrix-Test zwischen ZeroDDS, Cyclone DDS und
# eProsima Fast-DDS.
#
# Muss auf einem Linux-Host mit Docker laufen (Multicast zwischen
# Container und Host funktioniert nur mit `network_mode: host` auf
# Linux, nicht auf macOS Docker Desktop).
#
# Ablauf:
# 1. Startet Cyclone + Fast-DDS Publisher-Container.
# 2. Laeuft ZeroDDS-spdp_demo fuer 15 s.
# 3. Zaehlt Foreign-Vendor-Discovered-Events im Output.
# 4. Erwartung: mindestens je 1 Foreign-Vendor pro Lauf.

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

# Erfolgs-Kriterium: mindestens 1 Foreign-Vendor entdeckt.
FOREIGN=$(grep -c '✓ DISCOVERED' "$OUTDIR/demo.out" \
  | head -1 || echo 0)
# Nur Nicht-ZeroDDS zaehlen (VendorId != [1, 240]).
REAL=$(grep '✓ DISCOVERED' "$OUTDIR/demo.out" \
  | grep -v 'vendor=VendorId(\[1, 240\])' | wc -l)
echo "[matrix] total discoveries=$FOREIGN, foreign=$REAL"

if [ "$REAL" -lt 1 ]; then
    echo "[matrix] FAIL: no foreign vendors discovered"
    exit 1
fi
echo "[matrix] OK: $REAL foreign vendor(s) discovered"
