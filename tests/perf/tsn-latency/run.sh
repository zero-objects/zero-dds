#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# TSN roundtrip latency bench: AF_PACKET (DDS-TSN Annex A, EtherType
# 0x88B5) against a UDP baseline over THE SAME veth pair in the root NS.
#
# Measures the host transport path latency (request→echo→request). This is
# deliberately NOT a TSN bounded-latency measurement — the TSN advantage arises in the
# switch (taprio/ETF + gPTP), not in the host socket. This bench is a
# baseline + regression guard for the AF_PACKET path and an honest
# comparison with UDP on the same link.
#
# Needs root (CAP_NET_RAW + CAP_NET_ADMIN), Linux only.
# Usage: tests/perf/tsn-latency/run.sh [COUNT]   (default 20000)
set -euo pipefail

COUNT="${1:-20000}"
VA=zdtsnbA
VB=zdtsnbB
IP_A=10.99.0.1
IP_B=10.99.0.2
UDP_PORT=7400
BIN=target/release/examples/tsn_latency

if [[ "$(id -u)" != "0" ]]; then
    echo "FAIL: needs root (AF_PACKET + veth)" >&2
    exit 2
fi

cleanup() {
    [[ -n "${PONG_PID:-}" ]] && kill "$PONG_PID" 2>/dev/null || true
    ip link del "$VA" 2>/dev/null || true
}
trap cleanup EXIT

echo "[tsn-lat] building bench binary (release) ..." >&2
cargo build --release --example tsn_latency -p zerodds-transport-tsn --features live >&2

# Fresh veth pair with IPs (for the UDP comparison).
ip link del "$VA" 2>/dev/null || true
ip link add "$VA" type veth peer name "$VB"
ip addr add "$IP_A/24" dev "$VA"
ip addr add "$IP_B/24" dev "$VB"
ip link set "$VA" up
ip link set "$VB" up

MAC_B=$(cat "/sys/class/net/$VB/address")

run_one() {
    local label="$1"; shift
    local pong_cmd=("$1" "$2"); shift 2
    "$BIN" "${pong_cmd[@]}" >/dev/null 2>&1 &
    PONG_PID=$!
    sleep 0.4
    echo "[tsn-lat] $label: $COUNT samples ..." >&2
    "$BIN" "$@"
    kill "$PONG_PID" 2>/dev/null || true
    wait "$PONG_PID" 2>/dev/null || true
    PONG_PID=
}

echo "=== TSN (AF_PACKET, 0x88B5) ==="
TSN_JSON=$(run_one "TSN" tsn-pong "$VB" tsn-ping "$VA" "$MAC_B" "$COUNT")
echo "$TSN_JSON"

echo "=== UDP baseline (same veth link) ==="
UDP_JSON=$(run_one "UDP" udp-pong "$IP_B:$UDP_PORT" udp-ping "$IP_A:0" "$IP_B:$UDP_PORT" "$COUNT")
echo "$UDP_JSON"

echo "=== Summary ==="
echo "$TSN_JSON"
echo "$UDP_JSON"
