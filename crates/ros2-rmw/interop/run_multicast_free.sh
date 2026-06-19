#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# C1 proof: multicast-FREE discovery between two ZeroDDS processes.
# `ZERODDS_NO_MULTICAST=1` turns off every SPDP multicast send; the
# processes find each other EXCLUSIVELY via `ZERODDS_PEERS` (unicast
# initial peers, well-known RTPS ports). Negative control: without peers
# nothing flows. Runs on a single host (codepit), domain 0.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
PEERS="${ZERODDS_PEERS:-127.0.0.1}"

echo "== build ZeroDDS examples (release) =="
( cd "$ROOT" && cargo build -q -p zerodds-dcps --release \
    --example ros2_chatter_subscriber --example ros2_chatter_publisher ) || exit 1
SUB="$ROOT/target/release/examples/ros2_chatter_subscriber"
PUB="$ROOT/target/release/examples/ros2_chatter_publisher"

run_case() {
  local label="$1" peers="$2"
  local out; out="$(mktemp)"
  ZERODDS_NO_MULTICAST=1 ZERODDS_PEERS="$peers" "$SUB" > "$out" 2>&1 &
  local s=$!
  sleep 1
  # Publisher long enough for the 5s SPDP period to take effect bidirectionally.
  ZERODDS_NO_MULTICAST=1 ZERODDS_PEERS="$peers" "$PUB" 60 > /dev/null 2>&1 &
  local p=$!
  wait "$s" 2>/dev/null
  kill "$p" 2>/dev/null
  echo "[$label] $(grep -oE 'received [0-9]+ samples' "$out" | tail -1)"
  rm -f "$out"
}

echo "== C1 multicast-free (ZERODDS_NO_MULTICAST=1) =="
run_case "WITH Peers=$PEERS (unicast-only)" "$PEERS"
run_case "WITHOUT Peers (negative control)" ""
echo "Expected: WITH Peers >0 samples, WITHOUT Peers 0 samples."
