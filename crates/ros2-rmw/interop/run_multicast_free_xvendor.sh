#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# C1+C5: multicast-FREE cross-vendor discovery ZeroDDS <-> CycloneDDS
# (= real ROS 2). BOTH sides without multicast, discovery only via
# unicast initial peers (well-known SPDP ports). This is the WiFi/cloud
# VPC scenario in which multicast DDS breaks. Runs on codepit
# (needs /opt/cyclone). Expected: talker matched=1, sub received 20.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
CY="${CYCLONE:-/opt/cyclone}"
export LD_LIBRARY_PATH="$CY/lib:${LD_LIBRARY_PATH:-}"
IP="${HOST_IP:-$(ip -4 -o addr show scope global | awk '{print $4}' | cut -d/ -f1 | head -1)}"
echo "Host IP (unicast discovery): $IP"

echo "== build ZeroDDS sub + Cyclone talker =="
( cd "$ROOT" && cargo build -q -p zerodds-dcps --release --example ros2_chatter_subscriber ) || exit 1
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
( cd "$W" && "$CY/bin/idlc" "$HERE/std_msgs_string.idl" ) >/dev/null 2>&1 || exit 11
cc -O2 -I"$W" -I"$CY/include" "$HERE/cyclone_ros_talker.c" "$W/std_msgs_string.c" \
   -L"$CY/lib" -lddsc -o "$W/talker" || exit 12

# Cyclone: multicast OFF, unicast peer = host IP.
cat > "$W/cyc.xml" <<XML
<?xml version="1.0"?>
<CycloneDDS><Domain Id="any">
  <General><Interfaces><NetworkInterface address="$IP"/></Interfaces><AllowMulticast>false</AllowMulticast></General>
  <Discovery><ParticipantIndex>auto</ParticipantIndex><Peers><Peer address="$IP"/></Peers></Discovery>
</Domain></CycloneDDS>
XML

# ZeroDDS sub: multicast OFF + unicast peer = host IP. XCDR1,XCDR2 for
# the XCDR1 writer from ROS/Cyclone (separate reader-offer gap).
ZERODDS_NO_MULTICAST=1 ZERODDS_PEERS="$IP" ZERODDS_DATA_REPR_OFFER=XCDR1,XCDR2 \
  "$ROOT/target/release/examples/ros2_chatter_subscriber" > "$W/sub.out" 2>&1 &
S=$!
sleep 1
# Talker long enough for the 5s SPDP period to take effect bidirectionally (~9s).
CYCLONEDDS_URI="file://$W/cyc.xml" "$W/talker" 80 > "$W/talk.out" 2>&1 &
P=$!
wait "$S" 2>/dev/null
kill "$P" 2>/dev/null
echo "== result (multicast-free, cross-vendor) =="
grep -iE "matched subscriber" "$W/talk.out" | head -1
grep -iE "received" "$W/sub.out" | tail -1
echo "Expected: matched=1, received 20 (pure unicast discovery, no multicast)."
