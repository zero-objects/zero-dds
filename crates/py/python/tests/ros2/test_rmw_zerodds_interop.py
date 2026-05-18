"""ROS-2-Interop-Tests (§6.4 Vendor-Spec).

Diese Tests laufen NUR wenn:
  * `ROS_DISTRO` gesetzt ist (humble/iron/jazzy/...)
  * `rclpy` importierbar ist
  * `RMW_IMPLEMENTATION=rmw_zerodds_shim` (siehe `crates/rmw-zerodds-shim/`)

Auf jeder anderen Konfiguration werden sie geskipped (siehe
`conftest.py` in diesem Verzeichnis).

Zweck: nachweisen, dass `rmw_zerodds_shim` als RMW-Bridge fuer rclpy
funktioniert — d.h. ein Standard-rclpy-Publisher und -Subscriber
kommunizieren ueber ZeroDDS statt ueber Cyclone/Fast-DDS.
"""

from __future__ import annotations

import time

import pytest


@pytest.fixture
def rclpy_session():
    """rclpy-Session-Fixture mit sauberem Shutdown."""
    import rclpy

    rclpy.init()
    try:
        yield rclpy
    finally:
        rclpy.shutdown()


def test_rclpy_init_succeeds_with_zerodds_rmw(rclpy_session) -> None:
    """§6.4 — rclpy.init() laeuft sauber durch, wenn das RMW-Shim ZeroDDS ist."""
    rclpy = rclpy_session
    node = rclpy.create_node("zerodds_test_init")
    try:
        assert node.get_name() == "zerodds_test_init"
    finally:
        node.destroy_node()


def test_rclpy_publish_subscribe_string_roundtrip(rclpy_session) -> None:
    """§6.4 — Standard-rclpy Publish/Subscribe ueber std_msgs/String laeuft
    ueber ZeroDDS. Talker schreibt, Listener empfaengt."""
    rclpy = rclpy_session
    try:
        from std_msgs.msg import String
    except ImportError:
        pytest.skip("std_msgs/String nicht verfuegbar — voller ROS-2-Stack noetig")

    received: list[str] = []

    talker = rclpy.create_node("zerodds_talker")
    listener = rclpy.create_node("zerodds_listener")
    try:
        publisher = talker.create_publisher(String, "zerodds_test_topic", 10)

        def callback(msg: object) -> None:
            received.append(msg.data)  # type: ignore[attr-defined]

        listener.create_subscription(String, "zerodds_test_topic", callback, 10)

        msg = String()
        msg.data = "hello-from-zerodds"

        deadline = time.time() + 5.0
        while time.time() < deadline and not received:
            publisher.publish(msg)
            rclpy.spin_once(listener, timeout_sec=0.1)
            time.sleep(0.05)

        assert "hello-from-zerodds" in received
    finally:
        talker.destroy_node()
        listener.destroy_node()
