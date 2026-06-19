#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# C1+C3 cross-machine: PointCloud-sized samples over a REAL WiFi link,
# multicast-free. Publisher on the WiFi node, subscriber on the peer
# host; discovery only via unicast initial peers (WiFi drops multicast),
# data via RTPS DATA_FRAG + selective NACK_FRAG retransmit. Proves
# "PointCloud2 over WiFi just works".
#
# Env (to be set by the caller):
#   SUB_SSH   ssh target of the subscriber (peer host), e.g. root@<host>
#   PUB_SSH   ssh target of the publisher (WiFi node),   e.g. dev@<wifi-ip>
#   SUB_IP    IP of the subscriber host (for the publisher peer)
#   PUB_IP    IP of the publisher host (for the subscriber peer)
#   SUB_BIN   path to largedata_sub on SUB_SSH
#   PUB_BIN   path to largedata_pub on PUB_SSH
#   SIZE      sample size in bytes (default 2 MiB), COUNT (default 30)
set -uo pipefail
: "${SUB_SSH:?}"; : "${PUB_SSH:?}"; : "${SUB_IP:?}"; : "${PUB_IP:?}"
: "${SUB_BIN:?}"; : "${PUB_BIN:?}"
SIZE="${SIZE:-2097152}"; COUNT="${COUNT:-30}"

# Subscriber (peer host) in the background: multicast off, peer = publisher IP.
ssh "$SUB_SSH" "rm -f /tmp/wifi_sub.out; ZERODDS_NO_MULTICAST=1 ZERODDS_PEERS=$PUB_IP \
  nohup '$SUB_BIN' > /tmp/wifi_sub.out 2>&1 & echo sub-started" >/dev/null 2>&1

# Publisher (WiFi node): multicast off, peer = subscriber IP. Blocks.
echo "== Publisher $PUB_SSH -> Subscriber $SUB_SSH, ${SIZE} B x ${COUNT}, multicast-free over WiFi =="
ssh "$PUB_SSH" "ZERODDS_NO_MULTICAST=1 ZERODDS_PEERS=$SUB_IP '$PUB_BIN' $SIZE $COUNT 2>&1 | tail -1"

echo "== subscriber result (integrity check) =="
ssh "$SUB_SSH" 'sleep 3; grep -E "intact=|corrupt" /tmp/wifi_sub.out | tail -1'
echo "Expected: intact>=1 corrupt=0 (byte-perfect over a real WiFi link)."
