#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# TSN-Live-Transport e2e: zwei DcpsRuntime-Prozesse, je an ein Ende
# eines veth-Paars gebunden, Pub/Sub-Roundtrip ueber AF_PACKET-Ethernet-
# Frames (EtherType 0x88B5). Discovery (SPDP) laeuft ueber UDP-Multicast
# im (gemeinsamen) Root-Namespace; der User-Traffic geht ueber das
# veth-Paar als echter L2-Link.
#
# Hinweis: ein voll isoliertes Netns-Paar waere realistischer, ist aber
# in LXC-Containern (z.B. codepit) gesperrt (netns-bind-mount = EPERM).
# Das veth-Paar im Root-NS beweist den AF_PACKET-Transport ueber einen
# echten L2-Link ohne diese Einschraenkung.
#
# Braucht root (CAP_NET_RAW + CAP_NET_ADMIN). Linux-only. Exit 0 = ok.
set -euo pipefail

VETH0=zdtsn0
VETH1=zdtsn1
BIN=target/debug/examples/tsn_pingpong
PONG_OUT=$(mktemp)
PING_OUT=$(mktemp)

cleanup() {
    ip link del "$VETH0" 2>/dev/null || true
    rm -f "$PONG_OUT" "$PING_OUT" 2>/dev/null || true
}
trap cleanup EXIT

if [[ "$(id -u)" != "0" ]]; then
    echo "FAIL: braucht root (AF_PACKET + veth)" >&2
    exit 2
fi

echo "[tsn-e2e] baue Example-Binary ..."
cargo build --example tsn_pingpong -p zerodds-dcps --features tsn-live >&2

# Frisches veth-Paar (loescht evtl. Reste).
ip link del "$VETH0" 2>/dev/null || true
ip link add "$VETH0" type veth peer name "$VETH1"
ip link set "$VETH0" up
ip link set "$VETH1" up

echo "[tsn-e2e] starte pong auf $VETH1 ..."
env ZERODDS_TSN_IFACE="$VETH1" RUST_BACKTRACE=1 "$BIN" pong >"$PONG_OUT" 2>&1 &
PONG_PID=$!

sleep 1

echo "[tsn-e2e] starte ping auf $VETH0 ..."
env ZERODDS_TSN_IFACE="$VETH0" RUST_BACKTRACE=1 "$BIN" ping >"$PING_OUT" 2>&1 || true

wait "$PONG_PID" || true

echo "----- ping output -----"; cat "$PING_OUT"
echo "----- pong output -----"; cat "$PONG_OUT"

if grep -q "TSN-PONG-OK" "$PONG_OUT"; then
    echo "[tsn-e2e] PASS: TSN-Roundtrip erfolgreich"
    exit 0
fi
echo "[tsn-e2e] FAIL: kein TSN-PONG-OK" >&2
exit 1
