#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The proof that counts for the ROS 2 column: `ros2 topic pub` into a
# ZeroDDS subscriber, with the publisher being ROS 2's own CLI over
# rmw_cyclonedds_cpp and no ZeroDDS code in that process.
#
#   ROS_PREFIX=/root/micromamba/envs/ros2 crates/dcps/proofs/ros2_topic_pub.sh
#
# Step 0 runs first and is a round trip with **no ZeroDDS in either
# process tree**. Without it a dead counterparty reads as a ZeroDDS
# failure, and a harness that treats "no samples" as anything other than
# a failure reads it as a pass.
#
# TWO THINGS ABOUT THE COUNTERPARTY, BOTH FOUND BY RUNNING IT
#
#   1. `ros2 topic echo` asks the ros2cli daemon over XML-RPC for the
#      publisher's QoS before it subscribes, and a *stale* daemon makes
#      it raise rather than return nothing. Naming the QoS on the
#      command line skips that call, so step 0 measures DDS and not the
#      daemon. This is why $QOS is passed to every `echo` below and not
#      left to default.
#
#   2. `ros2 topic echo` installs a SIGTERM handler that does not exit.
#      A plain `timeout` therefore leaves it running for ever and the
#      script never returns. Every `timeout` here is `-s KILL`.
#
# Neither of those is in any ROS documentation this project found, and
# neither shows up as an error — the first looks like a ZeroDDS
# discovery failure and the second looks like a hang.
#
# TOPIC AND TYPE NAMES
#
# ROS 2 prefixes a topic with `rt/` and mangles the type name, so the
# ZeroDDS side subscribes to `rt/chatter` with the type
# `std_msgs::msg::dds_::String_`. The example does that; it is named
# here because a mismatch on either looks exactly like a wire failure.

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
ROS_PREFIX="${ROS_PREFIX:-/root/micromamba/envs/ros2}"
DOMAIN="${ROS_DOMAIN_ID:-0}"
LOGDIR="${LOGDIR:-$(mktemp -d)}"

QOS="--qos-reliability reliable --qos-durability volatile --qos-history keep_last --qos-depth 10"

if [ ! -f "$ROS_PREFIX/setup.bash" ]; then
    echo "PROOF-SKIP: no ROS 2 installation at ROS_PREFIX=$ROS_PREFIX"
    exit 0
fi

# shellcheck disable=SC1091
source "$ROS_PREFIX/setup.bash" >/dev/null 2>&1
export RMW_IMPLEMENTATION=rmw_cyclonedds_cpp
export ROS_DOMAIN_ID="$DOMAIN"
cd "$REPO"

echo "==> step 0: ROS to ROS on domain $DOMAIN, no ZeroDDS in either process"
timeout -s KILL 18 ros2 topic pub -r 2 /chatter std_msgs/msg/String "{data: LIVENESS}" \
    >/dev/null 2>&1 &
sleep 3
# shellcheck disable=SC2086
timeout -s KILL 14 ros2 topic echo /chatter std_msgs/msg/String $QOS \
    > "$LOGDIR/ros0-echo.log" 2>&1
N=$(grep -c LIVENESS "$LOGDIR/ros0-echo.log")
wait 2>/dev/null
echo "    ros -> ros samples: $N"
if [ "$N" -lt 1 ]; then
    echo "PROOF-FAIL: the counterparty is dead — every number below would be meaningless"
    exit 90
fi

echo "==> step 1: build the ZeroDDS subscriber"
cargo build -q -p zerodds-dcps --example ros2_chatter_subscriber || {
    echo "PROOF-FAIL: the subscriber did not build"
    exit 91
}

echo "==> step 2: ros2 topic pub -> ZeroDDS subscriber"
echo "    publisher:  rmw_cyclonedds_cpp, topic /chatter"
echo "    subscriber: zerodds-dcps, topic rt/chatter, std_msgs::msg::dds_::String_"
timeout -s KILL 60 ./target/debug/examples/ros2_chatter_subscriber \
    > "$LOGDIR/zd-sub.log" 2>&1 &
SUBSCRIBER=$!
sleep 5
timeout -s KILL 30 ros2 topic pub -r 2 /chatter std_msgs/msg/String \
    "{data: hello from ros2 topic pub}" > "$LOGDIR/ros-pub.log" 2>&1 &
wait $SUBSCRIBER
RC=$?

echo "--- the ZeroDDS subscriber's output ---"
cat "$LOGDIR/zd-sub.log"

# The example exits 2 when it received nothing, which is the one outcome
# an exit status is allowed to stand for here: everything else is
# decided by what it printed.
if [ "$RC" -eq 2 ]; then
    echo "PROOF-FAIL: the ZeroDDS subscriber received no sample"
    exit 2
fi
if [ "$RC" -ne 0 ]; then
    echo "PROOF-FAIL: the ZeroDDS subscriber exited $RC"
    exit "$RC"
fi
echo "PROOF-PASS: ros2 topic pub reached the ZeroDDS subscriber"
