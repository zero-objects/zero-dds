#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# TSN live transport e2e: two DcpsRuntime processes, each bound to one end
# of a veth pair, pub/sub roundtrip over AF_PACKET Ethernet
# frames (EtherType 0x88B5). Discovery (SPDP) runs over UDP multicast
# in the (shared) root namespace; the user traffic goes over the
# veth pair as a real L2 link.
#
# Note: a fully isolated netns pair would be more realistic, but is
# blocked in LXC containers (e.g. on the Linux test host) (netns bind mount = EPERM).
# The veth pair in the root NS proves the AF_PACKET transport over a
# real L2 link without this restriction.
#
# Needs root (CAP_NET_RAW + CAP_NET_ADMIN). Linux only. Exit 0 = ok.
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
    echo "FAIL: needs root (AF_PACKET + veth)" >&2
    exit 2
fi

echo "[tsn-e2e] building example binary ..."
cargo build --example tsn_pingpong -p zerodds-dcps --features tsn-live >&2

# Fresh veth pair (removes any leftovers).
ip link del "$VETH0" 2>/dev/null || true
ip link add "$VETH0" type veth peer name "$VETH1"
ip link set "$VETH0" up
ip link set "$VETH1" up

echo "[tsn-e2e] starting pong on $VETH1 ..."
env ZERODDS_TSN_IFACE="$VETH1" RUST_BACKTRACE=1 "$BIN" pong >"$PONG_OUT" 2>&1 &
PONG_PID=$!

sleep 1

echo "[tsn-e2e] starting ping on $VETH0 ..."
env ZERODDS_TSN_IFACE="$VETH0" RUST_BACKTRACE=1 "$BIN" ping >"$PING_OUT" 2>&1 || true

wait "$PONG_PID" || true

echo "----- ping output -----"; cat "$PING_OUT"
echo "----- pong output -----"; cat "$PONG_OUT"

if grep -q "TSN-PONG-OK" "$PONG_OUT"; then
    echo "[tsn-e2e] PASS: TSN roundtrip successful"
    exit 0
fi
echo "[tsn-e2e] FAIL: no TSN-PONG-OK" >&2
exit 1
