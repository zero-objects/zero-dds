#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# C3 proof: PointCloud-sized samples (several MB) through the full
# ZeroDDS DCPS stack — RTPS DATA_FRAG + selective reassembly. Verifies
# integrity (pattern data[i]=(i%251)). Shows that the reassembly cap
# is now ROS-realistic (previously a 1-MiB silent drop). Runs on a single
# host (codepit). Optionally multicast-free via ZERODDS_NO_MULTICAST+PEERS.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"

echo "== build large-data examples (release) =="
( cd "$ROOT" && cargo build -q -p zerodds-dcps --release \
    --example largedata_pub --example largedata_sub ) || exit 1
SUB="$ROOT/target/release/examples/largedata_sub"
PUB="$ROOT/target/release/examples/largedata_pub"

for SZ in 1048576 2097152 4194304 8388608; do
  out="$(mktemp)"
  "$SUB" > "$out" 2>&1 &
  s=$!
  sleep 1
  "$PUB" "$SZ" 8 > /dev/null 2>&1 &
  p=$!
  wait "$s" 2>/dev/null
  kill "$p" 2>/dev/null
  echo "SIZE=$SZ B -> $(grep -oE 'intact=[0-9]+ corrupt=[0-9]+' "$out" | tail -1)"
  rm -f "$out"
done
echo "Expected: all sizes intact>=1 corrupt=0 (cap default 16 MiB)."
