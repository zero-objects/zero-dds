#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the Cyclone ROS talker, captures an RTPS pcap of the ROS-2 wire
# (ground truth) and prints a summary. Runs on codepit.
#
# Env: CYCLONE (default /opt/cyclone), COUNT (default 20), OUT (pcap path).
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
CYCLONE="${CYCLONE:-/opt/cyclone}"
COUNT="${COUNT:-20}"
OUT="${OUT:-/tmp/ros_wire_chatter.pcap}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

export CYCLONEDDS_HOME="$CYCLONE"
export LD_LIBRARY_PATH="$CYCLONE/lib:${LD_LIBRARY_PATH:-}"
export PATH="$CYCLONE/bin:$PATH"

echo "== idlc: std_msgs/String =="
( cd "$WORK" && "$CYCLONE/bin/idlc" "$HERE/std_msgs_string.idl" ) || exit 1
echo "  generated type name/desc:"
grep -oE "std_msgs_msg_dds__String_[a-z_]*" "$WORK"/std_msgs_string.h | sort -u | head

echo "== build talker + listener =="
cc -O2 -I"$WORK" -I"$CYCLONE/include" \
   "$HERE/cyclone_ros_talker.c" "$WORK/std_msgs_string.c" \
   -L"$CYCLONE/lib" -lddsc -o "$WORK/talker" || exit 1
cc -O2 -I"$WORK" -I"$CYCLONE/include" \
   "$HERE/cyclone_ros_listener.c" "$WORK/std_msgs_string.c" \
   -L"$CYCLONE/lib" -lddsc -o "$WORK/listener" || exit 1

echo "== capture RTPS on all interfaces (UDP) → $OUT =="
# RTPS over UDP; Cyclone binds a non-loopback interface → -i any.
tcpdump -i any -w "$OUT" -U "udp and (port 7400 or portrange 7400-7500 or udp[8:4]=0x52545053)" >/dev/null 2>&1 &
TCPID=$!
sleep 1

echo "== run listener (bg) + talker (count=$COUNT) =="
"$WORK/listener" 10 &
LPID=$!
sleep 1   # await the discovery match
CYCLONEDDS_URI="${CYCLONEDDS_URI:-}" "$WORK/talker" "$COUNT"
wait "$LPID" 2>/dev/null

sleep 1
kill "$TCPID" 2>/dev/null; wait "$TCPID" 2>/dev/null

echo "== pcap summary =="
ls -la "$OUT"
if command -v tshark >/dev/null 2>&1; then
  echo "  RTPS submessages (top):"
  tshark -r "$OUT" -Y rtps -T fields -e rtps.sm.id 2>/dev/null | sort | uniq -c | sort -rn | head
  echo "  topic/type in the DATA(p)/DATA stream:"
  tshark -r "$OUT" -Y "rtps" -T fields -e _ws.col.Info 2>/dev/null | grep -iE "chatter|String|DATA" | head -5
else
  echo "  (tshark not installed — pcap saved raw)"
fi
echo "== done: $OUT =="
