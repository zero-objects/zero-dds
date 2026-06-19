"""ROS-2 interop tests (§6.4 vendor spec).

These tests run ONLY when:
  * `ROS_DISTRO` is set (humble/iron/jazzy/...)
  * `rclpy` is importable
  * `RMW_IMPLEMENTATION=rmw_zerodds_cpp` (see `crates/rmw-zerodds-shim/`)

On any other configuration they are skipped (see
`conftest.py` in this directory).

Purpose: demonstrate that `rmw_zerodds_cpp` works as an RMW bridge for
rclpy — i.e. a standard rclpy publisher and subscriber
communicate over ZeroDDS instead of over Cyclone/Fast-DDS.
"""

from __future__ import annotations

import time

import pytest


@pytest.fixture
def rclpy_session():
    """rclpy session fixture with a clean shutdown."""
    import rclpy

    rclpy.init()
    try:
        yield rclpy
    finally:
        rclpy.shutdown()


def test_rclpy_init_succeeds_with_zerodds_rmw(rclpy_session) -> None:
    """§6.4 — rclpy.init() runs through cleanly when the RMW shim is ZeroDDS."""
    rclpy = rclpy_session
    node = rclpy.create_node("zerodds_test_init")
    try:
        assert node.get_name() == "zerodds_test_init"
    finally:
        node.destroy_node()


def test_rclpy_publish_subscribe_string_roundtrip(rclpy_session) -> None:
    """§6.4 — standard rclpy publish/subscribe over std_msgs/String runs
    over ZeroDDS. The talker writes, the listener receives."""
    rclpy = rclpy_session
    try:
        from std_msgs.msg import String
    except ImportError:
        pytest.skip("std_msgs/String not available — full ROS-2 stack needed")

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


def test_rclpy_service_call_roundtrip(rclpy_session) -> None:
    """Service request/reply over ZeroDDS: a standard rclpy service server and
    client complete an AddTwoInts round-trip through rmw_zerodds (request CDR +
    correlation header → reply CDR back to the calling client)."""
    rclpy = rclpy_session
    try:
        from example_interfaces.srv import AddTwoInts
    except ImportError:
        pytest.skip("example_interfaces/srv/AddTwoInts not available")

    from rclpy.executors import SingleThreadedExecutor

    server_node = rclpy.create_node("zerodds_srv_server")
    client_node = rclpy.create_node("zerodds_srv_client")

    def handle(request, response):  # type: ignore[no-untyped-def]
        response.sum = request.a + request.b
        return response

    try:
        server_node.create_service(AddTwoInts, "zerodds_add", handle)
        client = client_node.create_client(AddTwoInts, "zerodds_add")

        exec_ = SingleThreadedExecutor()
        exec_.add_node(server_node)
        exec_.add_node(client_node)

        # Wait for the server to be discovered (drives rmw_service_server_is_available).
        deadline = time.time() + 10.0
        while time.time() < deadline and not client.service_is_ready():
            exec_.spin_once(timeout_sec=0.1)
        assert client.service_is_ready(), "service server not discovered"

        req = AddTwoInts.Request()
        req.a = 41
        req.b = 1
        future = client.call_async(req)

        deadline = time.time() + 10.0
        while time.time() < deadline and not future.done():
            exec_.spin_once(timeout_sec=0.1)

        assert future.done(), "service call did not complete"
        assert future.result().sum == 42
    finally:
        server_node.destroy_node()
        client_node.destroy_node()


def test_rclpy_topic_graph_introspection(rclpy_session) -> None:
    """Graph introspection over ZeroDDS: a node discovers another node's
    publisher topic + type via rmw_get_topic_names_and_types, and
    rmw_count_publishers counts it (P4a — topic-level graph from SEDP)."""
    rclpy = rclpy_session
    try:
        from std_msgs.msg import String
    except ImportError:
        pytest.skip("std_msgs/String not available")

    talker = rclpy.create_node("zerodds_graph_talker")
    observer = rclpy.create_node("zerodds_graph_observer")
    try:
        talker.create_publisher(String, "zerodds_graph_topic", 10)

        found = False
        deadline = time.time() + 10.0
        while time.time() < deadline and not found:
            for name, types in observer.get_topic_names_and_types():
                if name == "/zerodds_graph_topic":
                    assert any("std_msgs/msg/String" in t for t in types), types
                    found = True
            rclpy.spin_once(observer, timeout_sec=0.1)
            time.sleep(0.1)

        assert found, "publisher topic not discovered in the graph"
        assert observer.count_publishers("/zerodds_graph_topic") >= 1
    finally:
        talker.destroy_node()
        observer.destroy_node()


def test_rclpy_node_names_graph(rclpy_session) -> None:
    """Node-graph introspection over ZeroDDS: rmw_get_node_names lists the
    nodes (via the ros_discovery_info ParticipantEntitiesInfo path)."""
    rclpy = rclpy_session
    n1 = rclpy.create_node("zerodds_graph_n1")
    n2 = rclpy.create_node("zerodds_graph_n2")
    try:
        want = {"zerodds_graph_n1", "zerodds_graph_n2"}
        found: set[str] = set()
        deadline = time.time() + 10.0
        while time.time() < deadline and not want <= found:
            found.update(n1.get_node_names())
            rclpy.spin_once(n1, timeout_sec=0.1)
            time.sleep(0.1)
        assert want <= found, f"nodes not listed; got {found}"
    finally:
        n1.destroy_node()
        n2.destroy_node()


def test_rclpy_endpoint_info_by_topic(rclpy_session) -> None:
    """Endpoint-info introspection over ZeroDDS (REP-2009, `ros2 topic info -v`):
    `get_publishers_info_by_topic` returns the publisher's type, QoS and — via
    the ros_discovery_info per-node endpoint gid sequences — the owning node
    name resolved from the endpoint GUID."""
    rclpy = rclpy_session
    try:
        from std_msgs.msg import String
    except ImportError:
        pytest.skip("std_msgs/String not available")

    talker = rclpy.create_node("zerodds_ep_talker", namespace="/zd")
    observer = rclpy.create_node("zerodds_ep_observer")
    try:
        talker.create_publisher(String, "zerodds_ep_topic", 10)

        # The talker is in namespace "/zd", so the fully-qualified topic is
        # "/zd/zerodds_ep_topic".
        infos: list = []
        deadline = time.time() + 10.0
        while time.time() < deadline and not infos:
            infos = observer.get_publishers_info_by_topic("/zd/zerodds_ep_topic")
            rclpy.spin_once(observer, timeout_sec=0.1)
            time.sleep(0.1)

        assert infos, "publisher endpoint not discovered"
        info = infos[0]
        # Type is reported (demangled).
        assert "std_msgs/msg/String" in "/".join(info.topic_type) \
            if isinstance(info.topic_type, (list, tuple)) \
            else "std_msgs/msg/String" in info.topic_type
        # Node identity resolved from the endpoint GUID prefix (the REP-2009 fix).
        assert info.node_name == "zerodds_ep_talker", info.node_name
        assert info.node_namespace == "/zd", info.node_namespace
        # A non-zero endpoint GID is reported. `endpoint_gid` is a list of ints
        # in rclpy (the rmw_gid_t byte array exposed directly).
        gid = info.endpoint_gid
        gid = gid.data if hasattr(gid, "data") else gid
        assert any(b != 0 for b in bytes(gid)), "zero GID"
    finally:
        talker.destroy_node()
        observer.destroy_node()
