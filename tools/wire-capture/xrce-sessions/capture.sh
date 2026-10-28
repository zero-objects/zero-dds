#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Records a DDS-XRCE session between the eProsima Micro-XRCE-DDS Agent
# and the reference implementation's own example clients. Nothing in
# this script is ZeroDDS: both endpoints are the reference
# implementation, and neither process loads a line of this tree.
#
#   XRCE_ROOT=~/xrce-foreign XRCE_OUT=~/xrce-capture \
#       tools/wire-capture/xrce-sessions/capture.sh
#   tools/wire-capture/xrce-sessions/split.py --capture ~/xrce-capture
#
# Build the counterparty with `build-foreign.sh` first. `tcpdump` needs
# to be able to open the loopback interface, so this runs as root on the
# measurement host.
#
# The five sections below are chosen for submessage coverage, not for
# realism — each one is here because it is the only example of the
# reference implementation that emits a particular submessage kind.
# README.md carries the counts the recorded session actually produced.

set -uo pipefail
set -x

ROOT="${XRCE_ROOT:-$HOME/xrce-foreign}"
OUT="${XRCE_OUT:-$HOME/xrce-capture}"
PORT="${XRCE_PORT:-2018}"
DISCOVERY_PORT="${XRCE_DISCOVERY_PORT:-2019}"

AGENT="$ROOT/agent/build/MicroXRCEAgent"
EX="$ROOT/client/build/examples"

if [ ! -x "$AGENT" ]; then
    echo "NOT PRESENT: $AGENT — run build-foreign.sh first" >&2
    exit 2
fi

mkdir -p "$OUT"

pkill -f MicroXRCEAgent
sleep 1

tcpdump -i lo -n -s 0 -w "$OUT/session-udp.pcap" "udp port $PORT" &
TCPDUMP=$!
sleep 2

"$AGENT" udp4 -p "$PORT" -v 6 > "$OUT/agent.log" 2>&1 &
AGENT_PID=$!
sleep 3

# 1. reliable publish + subscribe over UDP4
"$EX/SubscribeHelloWorld/SubscribeHelloWorldClient" 127.0.0.1 "$PORT" 5 > "$OUT/sub.log" 2>&1 &
SUB=$!
sleep 2
"$EX/PublishHelloWorld/PublishHelloWorldClient" 127.0.0.1 "$PORT" 5 > "$OUT/pub.log" 2>&1
sleep 3

# 2. best-effort, so the session also carries a non-reliable stream
"$EX/SubscribeHelloWorldBestEffort/SubscribeHelloWorldClientBestEffort" 127.0.0.1 "$PORT" 3 \
    > "$OUT/sub-be.log" 2>&1 &
sleep 2
"$EX/PublishHelloWorldBestEffort/PublishHelloWorldClientBestEffort" 127.0.0.1 "$PORT" 3 \
    > "$OUT/pub-be.log" 2>&1
sleep 2

# 3. payloads above the MTU, so FRAGMENT appears
"$EX/SubscribeBigHelloWorld/SubscribBigBigClient" 127.0.0.1 "$PORT" 2 > "$OUT/sub-big.log" 2>&1 &
sleep 2
"$EX/PublishBigHelloWorld/PublishBigHelloWorldClient" 127.0.0.1 "$PORT" 2 > "$OUT/pub-big.log" 2>&1
sleep 2

# 4. CREATE with a binary object representation rather than the XML one
"$EX/BinaryEntityCreation/BinaryEntityCreation" 127.0.0.1 "$PORT" 3 > "$OUT/binary.log" 2>&1
sleep 2

# 5. TIMESTAMP / TIMESTAMP_REPLY
"$EX/TimeSync/TimeSync" 127.0.0.1 "$PORT" 3 > "$OUT/timesync.log" 2>&1
sleep 2

kill "$SUB" 2>/dev/null
kill "$AGENT_PID" 2>/dev/null
sleep 2
kill "$TCPDUMP" 2>/dev/null
sleep 1

# 6. agent discovery — GET_INFO / INFO.
#
# This section is recorded and its octets are NOT in the tree. The INFO
# body is a list of the transport locators the agent is reachable at, so
# it carries every IPv4 address of the measurement host. See the section
# in README.md.
tcpdump -i any -n -s 0 -w "$OUT/session-discovery.pcap" \
    "udp port 7400 or udp port $DISCOVERY_PORT" &
TCPDUMP2=$!
sleep 2
"$AGENT" udp4 -p "$DISCOVERY_PORT" -d -v 6 > "$OUT/agent-disc.log" 2>&1 &
AGENT2=$!
sleep 3
"$EX/Discovery/Discovery" > "$OUT/discovery.log" 2>&1
sleep 3
kill "$AGENT2" 2>/dev/null
sleep 2
kill "$TCPDUMP2" 2>/dev/null
sleep 1

set +x
echo "CAPTURE_DONE — now: split.py --capture $OUT"
