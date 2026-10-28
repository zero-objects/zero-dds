#!/usr/bin/env python3
# packaging/linux/deb/rmw-type-hash-check.py
# Type hashes (REP 2011) through an installed ros-<distro>-rmw-zerodds-cpp
# package, against itself and against the image's rmw_fastrtps_cpp.
#
# rmw_zerodds_cpp announces `typehash=RIHS01_<hex>;` in the USER_DATA of every
# endpoint and reads the peers' into rmw_topic_endpoint_info_t, the way
# rmw_fastrtps_cpp and rmw_cyclonedds_cpp do. This script is the check that a
# peer process sees it: the parent process creates a publisher under the first
# RMW, a child process a subscription under the second, and the parent asks
# rcl for both endpoints' info on the topic. It exits 0 only when both show
# the same RIHS01 hash. With the first RMW rmw_fastrtps_cpp and the second
# rmw_zerodds_cpp, the subscription's hash can only come from what
# rmw_zerodds_cpp announced; the other order reads Fast DDS's announcement.
# The caller sets ROS_DOMAIN_ID. Humble has no type hashes.
#
# Usage: rmw-type-hash-check.py <publisher RMW> <subscription RMW>
import os
import subprocess
import sys
import time

import rclpy
from std_msgs.msg import String

TOPIC = "/zerodds_rmw_deb_type_hash"
DISCOVERY_S = 15.0
PEER_S = 25.0


def run_peer() -> int:
    rclpy.init()
    node = rclpy.create_node("zerodds_rmw_deb_type_hash_peer")
    node.create_subscription(String, TOPIC, lambda _msg: None, 10)
    deadline = time.monotonic() + PEER_S
    while time.monotonic() < deadline:
        rclpy.spin_once(node, timeout_sec=0.2)
    node.destroy_node()
    rclpy.shutdown()
    return 0


def run_check(pub_rmw: str, sub_rmw: str) -> int:
    peer = subprocess.Popen(
        [sys.executable, __file__, "peer"],
        env=dict(os.environ, RMW_IMPLEMENTATION=sub_rmw),
    )
    os.environ["RMW_IMPLEMENTATION"] = pub_rmw
    rclpy.init()
    node = rclpy.create_node("zerodds_rmw_deb_type_hash_pub")
    node.create_publisher(String, TOPIC, 10)
    pubs, subs = [], []
    deadline = time.monotonic() + DISCOVERY_S
    while time.monotonic() < deadline:
        pubs = node.get_publishers_info_by_topic(TOPIC)
        subs = node.get_subscriptions_info_by_topic(TOPIC)
        if pubs and subs:
            break
        time.sleep(0.2)
    peer.kill()
    peer.wait()
    for kind, infos in (("publisher", pubs), ("subscription", subs)):
        for info in infos:
            print(f"{kind} {info.node_name}: {info.topic_type_hash}")
    node.destroy_node()
    rclpy.shutdown()
    ok = (
        len(pubs) == 1
        and len(subs) == 1
        and str(subs[0].topic_type_hash).startswith("RIHS01_")
        and str(subs[0].topic_type_hash) == str(pubs[0].topic_type_hash)
    )
    print(f"type-hash {pub_rmw} -> {sub_rmw}:", "ok" if ok else "FAIL")
    return 0 if ok else 1


def main(argv) -> int:
    if len(argv) == 2 and argv[1] == "peer":
        return run_peer()
    if len(argv) != 3:
        print("usage: rmw-type-hash-check.py <publisher RMW> <subscription RMW>", file=sys.stderr)
        return 2
    return run_check(argv[1], argv[2])


if __name__ == "__main__":
    sys.exit(main(sys.argv))
