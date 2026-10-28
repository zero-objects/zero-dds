"""QoS events through rmw_zerodds_cpp (rmw/event.h).

rclpy creates an rmw event per registered event callback, waits on it in
the node's wait set (rmw_wait) and takes it (rmw_take_event). Before, every
event was reported as never pending, so these callbacks never ran.

Run by `crates/rmw-zerodds-shim/rmw_c/run_ros2_pytest.sh`; skipped outside
a ROS 2 environment with RMW_IMPLEMENTATION=rmw_zerodds_cpp (conftest.py).
"""

from __future__ import annotations

import time

import pytest


@pytest.fixture
def rclpy_session():
    import rclpy

    rclpy.init()
    try:
        yield rclpy
    finally:
        rclpy.shutdown()


def _spin_until(rclpy, nodes, predicate, timeout_s: float) -> bool:
    from rclpy.executors import SingleThreadedExecutor

    executor = SingleThreadedExecutor()
    for node in nodes:
        executor.add_node(node)
    deadline = time.monotonic() + timeout_s
    try:
        while time.monotonic() < deadline:
            executor.spin_once(timeout_sec=0.05)
            if predicate():
                return True
        return predicate()
    finally:
        for node in nodes:
            executor.remove_node(node)


def test_requested_deadline_missed_reaches_the_callback(rclpy_session) -> None:
    """A subscription with a 200 ms deadline, a publisher that writes once:
    the deadline is missed and the callback sees it."""
    rclpy = rclpy_session
    from rclpy.duration import Duration
    from rclpy.qos import QoSProfile
    from rclpy.qos_event import PublisherEventCallbacks, SubscriptionEventCallbacks
    from std_msgs.msg import String

    qos = QoSProfile(depth=10, deadline=Duration(seconds=0, nanoseconds=200_000_000))
    missed: list[int] = []
    offered: list[int] = []
    talker = rclpy.create_node("zerodds_deadline_talker")
    listener = rclpy.create_node("zerodds_deadline_listener")
    try:
        pub = talker.create_publisher(
            String,
            "zerodds_deadline_topic",
            qos,
            event_callbacks=PublisherEventCallbacks(
                deadline=lambda e: offered.append(e.total_count)
            ),
        )
        listener.create_subscription(
            String,
            "zerodds_deadline_topic",
            lambda _m: None,
            qos,
            event_callbacks=SubscriptionEventCallbacks(
                deadline=lambda e: missed.append(e.total_count)
            ),
        )
        assert _spin_until(
            rclpy, [talker, listener], lambda: pub.get_subscription_count() >= 1, 10.0
        ), "publisher and subscription did not match"
        pub.publish(String(data="once"))
        assert _spin_until(
            rclpy, [talker, listener], lambda: bool(missed) and bool(offered), 5.0
        ), f"deadline events: requested={missed} offered={offered}"
        assert missed[-1] >= 1 and offered[-1] >= 1
    finally:
        talker.destroy_node()
        listener.destroy_node()


def test_incompatible_qos_reaches_both_sides(rclpy_session) -> None:
    """A RELIABLE subscription and a BEST_EFFORT publisher do not match
    (DDS 1.4 §2.2.3.14); both sides report the policy."""
    rclpy = rclpy_session
    from rclpy.qos import QoSPolicyKind, QoSProfile, ReliabilityPolicy
    from rclpy.qos_event import PublisherEventCallbacks, SubscriptionEventCallbacks
    from std_msgs.msg import String

    requested: list[object] = []
    offered: list[object] = []
    talker = rclpy.create_node("zerodds_incompat_talker")
    listener = rclpy.create_node("zerodds_incompat_listener")
    try:
        talker.create_publisher(
            String,
            "zerodds_incompat_topic",
            QoSProfile(depth=10, reliability=ReliabilityPolicy.BEST_EFFORT),
            event_callbacks=PublisherEventCallbacks(
                incompatible_qos=lambda e: offered.append(e.last_policy_kind)
            ),
        )
        listener.create_subscription(
            String,
            "zerodds_incompat_topic",
            lambda _m: None,
            QoSProfile(depth=10, reliability=ReliabilityPolicy.RELIABLE),
            event_callbacks=SubscriptionEventCallbacks(
                incompatible_qos=lambda e: requested.append(e.last_policy_kind)
            ),
        )
        assert _spin_until(
            rclpy, [talker, listener], lambda: bool(requested) and bool(offered), 10.0
        ), f"incompatible-QoS events: requested={requested} offered={offered}"
        assert requested[-1] == QoSPolicyKind.RELIABILITY
        assert offered[-1] == QoSPolicyKind.RELIABILITY
    finally:
        talker.destroy_node()
        listener.destroy_node()


def test_a_publisher_reports_its_network_flow_endpoints(rclpy_session) -> None:
    """rmw_publisher_get_network_flow_endpoints: the unicast locators the
    participant announces, at least one UDP endpoint with an address."""
    rclpy = rclpy_session
    from std_msgs.msg import String

    node = rclpy.create_node("zerodds_flow_endpoints")
    try:
        pub = node.create_publisher(String, "zerodds_flow_topic", 10)
        if not hasattr(pub, "get_network_flow_endpoints"):
            pytest.skip("rclpy has no Publisher.get_network_flow_endpoints")
        endpoints = pub.get_network_flow_endpoints()
        assert endpoints, "no network flow endpoint reported"
        assert any(
            e.transport_protocol.name == "UDP" and e.internet_address for e in endpoints
        ), [str(e) for e in endpoints]
    finally:
        node.destroy_node()


def test_a_content_filter_delivers_only_what_it_accepts(rclpy_session) -> None:
    """rmw_subscription_set_content_filter: `data > %0` with %0 = 5 lets
    6..9 through out of 0..9, and the subscription reports the filter."""
    rclpy = rclpy_session
    try:
        from rclpy.subscription_content_filter_options import ContentFilterOptions
    except ImportError:
        pytest.skip("rclpy has no content filter options")
    from std_msgs.msg import Int32

    received: list[int] = []
    talker = rclpy.create_node("zerodds_cft_talker")
    listener = rclpy.create_node("zerodds_cft_listener")
    try:
        pub = talker.create_publisher(Int32, "zerodds_cft_topic", 20)
        sub = listener.create_subscription(
            Int32,
            "zerodds_cft_topic",
            lambda m: received.append(m.data),
            20,
            content_filter_options=ContentFilterOptions(
                filter_expression="data > %0", expression_parameters=["5"]
            ),
        )
        assert sub.is_cft_enabled
        assert _spin_until(
            rclpy, [talker, listener], lambda: pub.get_subscription_count() >= 1, 10.0
        ), "publisher and subscription did not match"
        for i in range(10):
            pub.publish(Int32(data=i))
        _spin_until(rclpy, [talker, listener], lambda: len(received) >= 4, 5.0)
        _spin_until(rclpy, [talker, listener], lambda: False, 0.3)
        assert received == [6, 7, 8, 9], received
        options = sub.get_content_filter()
        assert options.filter_expression == "data > %0"
        assert list(options.expression_parameters) == ["5"]
    finally:
        talker.destroy_node()
        listener.destroy_node()


def test_message_info_names_the_source_timestamp() -> None:
    """rmw_message_info_t.source_timestamp comes from the writer (INFO_TS),
    not 0: a publisher and a subscription in two contexts, so the message
    travels over RTPS."""
    import rclpy
    from rclpy.context import Context
    from std_msgs.msg import String

    ctx_pub, ctx_sub = Context(), Context()
    rclpy.init(context=ctx_pub)
    rclpy.init(context=ctx_sub)
    try:
        talker = rclpy.create_node("zerodds_info_talker", context=ctx_pub)
        listener = rclpy.create_node("zerodds_info_listener", context=ctx_sub)
        pub = talker.create_publisher(String, "zerodds_info_topic", 10)
        sub = listener.create_subscription(String, "zerodds_info_topic", lambda _m: None, 10)
        deadline = time.monotonic() + 10.0
        while pub.get_subscription_count() < 1 and time.monotonic() < deadline:
            time.sleep(0.05)
        assert pub.get_subscription_count() >= 1, "no match across contexts"
        before_ns = time.time_ns()
        taken = None
        while taken is None and time.monotonic() < deadline:
            pub.publish(String(data="info"))
            time.sleep(0.05)
            with sub.handle:
                taken = sub.handle.take_message(String, False)
        assert taken is not None, "nothing taken"
        _msg, info = taken
        source = info["source_timestamp"]
        assert before_ns - 5_000_000_000 < source <= time.time_ns(), info
    finally:
        rclpy.shutdown(context=ctx_pub)
        rclpy.shutdown(context=ctx_sub)
