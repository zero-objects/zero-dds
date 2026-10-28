#!/usr/bin/env python3
# packaging/linux/deb/rmw-pubsub-check.py
# Round trip through an installed ros-<distro>-rmw-zerodds-cpp package.
#
# build-rmw-deb.sh proves the staged file list before dpkg-deb runs and the
# CI job checks the .deb's own list and that rcl resolves librmw_zerodds_cpp.so
# through RMW_IMPLEMENTATION. Neither shows a sample travelling through the
# RMW. This script is that half, as two separate processes: `pub` publishes
# std_msgs/String "0".."19" at 10 Hz on a RELIABLE, KEEP_ALL topic, `sub`
# subscribes and exits 0 only after all twenty distinct payloads arrived. The
# caller sets RMW_IMPLEMENTATION=rmw_zerodds_cpp and ROS_DOMAIN_ID in both.
#
# Usage: rmw-pubsub-check.py {pub,sub}
import sys
import time

import rclpy
from rclpy.qos import HistoryPolicy, QoSProfile, ReliabilityPolicy
from std_msgs.msg import String

TOPIC = "/zerodds_rmw_deb_pubsub"
COUNT = 20
PERIOD_S = 0.1
TIMEOUT_S = 30.0


def qos_profile() -> QoSProfile:
    # RELIABLE + KEEP_ALL as zerodds-ros2-bridge-1.0.md §10 requires of the
    # transport: every sample waits for its acknowledgement and no history
    # depth drops one. `depth` is still required by QoSProfile; it is ignored
    # for KEEP_ALL.
    return QoSProfile(
        depth=COUNT,
        reliability=ReliabilityPolicy.RELIABLE,
        history=HistoryPolicy.KEEP_ALL,
    )


def run_pub() -> int:
    rclpy.init()
    node = rclpy.create_node("zerodds_rmw_deb_pub")
    publisher = node.create_publisher(String, TOPIC, qos_profile())
    deadline = time.monotonic() + TIMEOUT_S
    # Wait for the subscriber to match before the first sample: a check that
    # depends on the writer's history retaining samples for a late joiner would
    # test durability, not a live round trip.
    while publisher.get_subscription_count() == 0 and time.monotonic() < deadline:
        rclpy.spin_once(node, timeout_sec=0.05)
    # Cycle "0".."19" until the deadline; the caller kills this process once
    # the subscriber has all twenty, so the run publishes exactly as long as
    # the subscriber is alive and stops at 30 s on its own if it never is.
    i = 0
    while time.monotonic() < deadline:
        msg = String()
        msg.data = str(i % COUNT)
        publisher.publish(msg)
        i += 1
        # spin_once paces the loop at PERIOD_S (10 Hz) and lets this process
        # drive its own RMW discovery/acknowledgement work between samples.
        rclpy.spin_once(node, timeout_sec=PERIOD_S)
    # Keep the endpoint alive briefly so the last samples are acknowledged.
    time.sleep(1.0)
    node.destroy_node()
    rclpy.shutdown()
    return 0


def run_sub() -> int:
    rclpy.init()
    node = rclpy.create_node("zerodds_rmw_deb_sub")
    seen = set()
    foreign = []

    def on_message(msg) -> None:
        try:
            seen.add(int(msg.data))
        except ValueError:
            # A payload outside the numbered set is not part of this check;
            # record it so the failure output names it instead of hiding it.
            foreign.append(msg.data)

    node.create_subscription(String, TOPIC, on_message, qos_profile())
    deadline = time.monotonic() + TIMEOUT_S
    while time.monotonic() < deadline:
        rclpy.spin_once(node, timeout_sec=0.1)
        if all(i in seen for i in range(COUNT)):
            break
    node.destroy_node()
    rclpy.shutdown()

    received = sorted(i for i in seen if 0 <= i < COUNT)
    missing = [i for i in range(COUNT) if i not in seen]
    print(f"received {len(received)}/{COUNT}: {received}")
    if foreign:
        print(f"foreign payloads: {foreign}", file=sys.stderr)
    if missing:
        print(f"missing: {missing}", file=sys.stderr)
        return 1
    return 0


def main(argv) -> int:
    if len(argv) != 2 or argv[1] not in ("pub", "sub"):
        print("usage: rmw-pubsub-check.py {pub,sub}", file=sys.stderr)
        return 2
    return run_pub() if argv[1] == "pub" else run_sub()


if __name__ == "__main__":
    sys.exit(main(sys.argv))
