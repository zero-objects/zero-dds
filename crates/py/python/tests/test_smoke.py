"""Smoke tests for the Python binding.

Offline paths only — full E2E tests come with the ROS2 pytest integration
in the multi-process test setup. To run:

    maturin develop --features extension-module
    pytest crates/py/python/tests/
"""

import pytest

import zerodds


def test_version_exposed():
    """`__version__` is exposed even without `_core` (fallback in `__init__.py`)."""
    assert isinstance(zerodds.__version__, str)
    assert len(zerodds.__version__) > 0


_NO_CORE = not getattr(zerodds, "_CORE_AVAILABLE", False)
_NO_CORE_REASON = "zerodds._core not compiled — maturin develop needed"


@pytest.mark.skipif(_NO_CORE, reason=_NO_CORE_REASON)
def test_factory_singleton_and_offline_participant():
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(99)
    assert p.domain_id == 99
    assert p.topics_len() == 0


@pytest.mark.skipif(_NO_CORE, reason=_NO_CORE_REASON)
def test_bytes_topic_and_reader_are_creatable_offline():
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(100)
    topic = p.create_bytes_topic("Chatter")
    assert topic.name == "Chatter"
    assert topic.type_name == "zerodds::RawBytes"

    subscriber = p.create_subscriber()
    reader = subscriber.create_bytes_reader(topic)
    # Nothing written → take returns an empty list.
    samples = reader.take()
    assert samples == []


@pytest.mark.skipif(_NO_CORE, reason=_NO_CORE_REASON)
def test_shape_topic_matches_vendor_interop_type_name():
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(101)
    topic = p.create_shape_topic("Square")
    # This type name is interop-critical. Changing it would break SEDP
    # matching with Cyclone/Fast-DDS ShapesDemo.
    assert topic.type_name == "ShapeType"


@pytest.mark.skipif(_NO_CORE, reason=_NO_CORE_REASON)
def test_shape_constructor_and_repr():
    s = zerodds.Shape("RED", 10, 20, 30)
    assert s.color == "RED"
    assert s.x == 10
    assert s.y == 20
    assert s.shapesize == 30
    assert "RED" in repr(s)


@pytest.mark.skipif(_NO_CORE, reason=_NO_CORE_REASON)
def test_shape_default_shapesize_is_30():
    s = zerodds.Shape("BLUE", 5, 6)
    assert s.shapesize == 30


@pytest.mark.skipif(True, reason="live E2E needs Linux multicast — in the multi-process test setup with pytest-xdist")
def test_pub_sub_roundtrip_live():
    """Placeholder for live E2E. Activated in the multi-process test setup with
    multi-process fixtures."""
    factory = zerodds.DomainParticipantFactory.instance()
    pub_p = factory.create_participant_fast(200)
    sub_p = factory.create_participant_fast(200)
    topic_pub = pub_p.create_bytes_topic("PyChatter")
    topic_sub = sub_p.create_bytes_topic("PyChatter")
    writer = pub_p.create_publisher().create_bytes_writer(topic_pub)
    reader = sub_p.create_subscriber().create_bytes_reader(topic_sub)
    writer.wait_for_matched_subscription(1, 5.0)
    reader.wait_for_matched_publication(1, 5.0)
    writer.write(b"python hello")
    reader.wait_for_data(3.0)
    samples = reader.take()
    assert b"python hello" in samples
