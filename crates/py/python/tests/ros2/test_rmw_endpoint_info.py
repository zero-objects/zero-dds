"""Endpoint info through rmw_zerodds_cpp (rmw_get_publishers_info_by_topic),
which `ros2 topic info -v` prints.

Run by `crates/rmw-zerodds-shim/rmw_c/run_ros2_pytest.sh`; skipped outside
a ROS 2 environment with RMW_IMPLEMENTATION=rmw_zerodds_cpp (conftest.py).
"""

from __future__ import annotations

import time

import pytest

# rmw_time_t RMW_DURATION_INFINITE as rclpy reads it back: 9223372036 s and
# 854775807 ns, the largest signed 64-bit count of nanoseconds.
INFINITE_NS = 9_223_372_036_854_775_807


@pytest.fixture
def rclpy_session():
    import rclpy

    rclpy.init()
    try:
        yield rclpy
    finally:
        rclpy.shutdown()


def _publisher_qos(node, topic: str, timeout_s: float = 5.0):
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        infos = node.get_publishers_info_by_topic(topic)
        if infos:
            return infos[0].qos_profile
        time.sleep(0.05)
    raise AssertionError(f"no publisher info for {topic}")


def test_a_publishers_durations_reach_the_endpoint_info_to_the_nanosecond(
    rclpy_session,
) -> None:
    """A 150 ms deadline and a 2.5 s lifespan are reported as set; the
    endpoint info carried whole seconds and reported them as 0 s and 2 s.
    A publisher that sets neither reports INFINITE."""
    rclpy = rclpy_session
    from rclpy.duration import Duration
    from rclpy.qos import QoSProfile
    from std_msgs.msg import String

    node = rclpy.create_node("zerodds_endpoint_info")
    try:
        node.create_publisher(
            String,
            "/zerodds_endpoint_info_set",
            QoSProfile(
                depth=10,
                deadline=Duration(nanoseconds=150_000_000),
                lifespan=Duration(seconds=2, nanoseconds=500_000_000),
            ),
        )
        node.create_publisher(String, "/zerodds_endpoint_info_unset", QoSProfile(depth=10))

        qos = _publisher_qos(node, "/zerodds_endpoint_info_set")
        assert qos.deadline.nanoseconds == 150_000_000
        assert qos.lifespan.nanoseconds == 2_500_000_000

        unset = _publisher_qos(node, "/zerodds_endpoint_info_unset")
        assert unset.deadline.nanoseconds == INFINITE_NS
        assert unset.lifespan.nanoseconds == INFINITE_NS
    finally:
        node.destroy_node()
