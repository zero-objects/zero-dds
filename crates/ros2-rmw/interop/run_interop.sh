#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# ROS wire interop: a ZeroDDS subscriber receives from the Cyclone ROS talker
# (= rmw_cyclonedds = real ROS 2) on rt/chatter. Runs on codepit.
# Env: CYCLONE (/opt/cyclone), COUNT (20).
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
CYCLONE="${CYCLONE:-/opt/cyclone}"
COUNT="${COUNT:-20}"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
export LD_LIBRARY_PATH="$CYCLONE/lib:${LD_LIBRARY_PATH:-}"

echo "== build ZeroDDS subscriber (release) =="
( cd "$ROOT" && cargo build -p zerodds-dcps --example ros2_chatter_subscriber --release ) || exit 1

echo "== build Cyclone ROS talker =="
( cd "$WORK" && "$CYCLONE/bin/idlc" "$HERE/std_msgs_string.idl" ) || exit 1
cc -O2 -I"$WORK" -I"$CYCLONE/include" \
   "$HERE/cyclone_ros_talker.c" "$WORK/std_msgs_string.c" \
   -L"$CYCLONE/lib" -lddsc -o "$WORK/talker" || exit 1

echo "== ZeroDDS sub (bg) + Cyclone talker (count=$COUNT) =="
# The actual match blocker (entityKind keyless vs keyed) is fixed in the DCPS
# (create_datawriter/reader derive is_keyed from DdsType::HAS_KEY).
# What remains is the data_representation gap: ROS 2/Cyclone sends XCDR1 for
# std_msgs/String (final/simple), ZeroDDS' reader default is
# XCDR2-only → the reader must also offer XCDR1, otherwise Cyclone's
# data_representation_match_p does not apply. Token syntax (XCDR1/XCDR2). For the
# cleaner fix (DataRepresentationQosPolicy) see
# internal/interop/ros2-reader-xcdr1-offer-followup.md.
ZERODDS_DATA_REPR_OFFER="${ZERODDS_DATA_REPR_OFFER:-XCDR1,XCDR2}" \
  "$ROOT/target/release/examples/ros2_chatter_subscriber" > "$WORK/zd.out" 2>&1 &
SUB=$!
sleep 2   # await the discovery match
"$WORK/talker" "$COUNT" | tail -3
wait "$SUB" 2>/dev/null; RC=$?

echo "== ZeroDDS subscriber output =="
cat "$WORK/zd.out"
echo "== exit $RC (0 = ZeroDDS received ROS samples from Cyclone) =="
exit $RC
