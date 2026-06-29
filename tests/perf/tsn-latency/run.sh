#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# TSN-Roundtrip-Latenz-Bench: AF_PACKET (DDS-TSN Annex A, EtherType
# 0x88B5) gegen eine UDP-Baseline ueber DASSELBE veth-Paar im Root-NS.
#
# Misst die Host-Transport-Pfad-Latenz (Request→Echo→Request). Das ist
# bewusst KEINE TSN-bounded-latency-Messung — der TSN-Vorteil entsteht im
# Switch (taprio/ETF + gPTP), nicht im Host-Socket. Dieser Bench ist
# Baseline + Regressions-Guard fuer den AF_PACKET-Pfad und ein ehrlicher
# Vergleich zu UDP auf demselben Link.
#
# Braucht root (CAP_NET_RAW + CAP_NET_ADMIN), Linux-only.
# Nutzung: tests/perf/tsn-latency/run.sh [COUNT]   (Default 20000)
set -euo pipefail

COUNT="${1:-20000}"
VA=zdtsnbA
VB=zdtsnbB
IP_A=10.99.0.1
IP_B=10.99.0.2
UDP_PORT=7400
BIN=target/release/examples/tsn_latency

if [[ "$(id -u)" != "0" ]]; then
    echo "FAIL: braucht root (AF_PACKET + veth)" >&2
    exit 2
fi

cleanup() {
    [[ -n "${PONG_PID:-}" ]] && kill "$PONG_PID" 2>/dev/null || true
    ip link del "$VA" 2>/dev/null || true
}
trap cleanup EXIT

echo "[tsn-lat] baue Bench-Binary (release) ..." >&2
cargo build --release --example tsn_latency -p zerodds-transport-tsn --features live >&2

# Frisches veth-Paar mit IPs (fuer den UDP-Vergleich).
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
    echo "[tsn-lat] $label: $COUNT Samples ..." >&2
    "$BIN" "$@"
    kill "$PONG_PID" 2>/dev/null || true
    wait "$PONG_PID" 2>/dev/null || true
    PONG_PID=
}

echo "=== TSN (AF_PACKET, 0x88B5) ==="
TSN_JSON=$(run_one "TSN" tsn-pong "$VB" tsn-ping "$VA" "$MAC_B" "$COUNT")
echo "$TSN_JSON"

echo "=== UDP-Baseline (gleicher veth-Link) ==="
UDP_JSON=$(run_one "UDP" udp-pong "$IP_B:$UDP_PORT" udp-ping "$IP_A:0" "$IP_B:$UDP_PORT" "$COUNT")
echo "$UDP_JSON"

echo "=== Zusammenfassung ==="
echo "$TSN_JSON"
echo "$UDP_JSON"
