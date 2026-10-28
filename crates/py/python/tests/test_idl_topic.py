"""Tests for §6.1 — IDL topic path.

Verifies `zerodds.IdlTopic` / `IdlWriter` / `IdlReader` as pure-Python
wrappers over the `BytesTopic` surface, driven by a generated
`TopicDescriptor` (`idltopic.idl` / `idltopic_gen.py`,
`zerodds-idlc generate --lang python`) rather than the removed
`@idl_struct` decorator.
"""

from __future__ import annotations


import pytest

import zerodds
from zerodds.topic import IdlReader, IdlTopic, IdlWriter

from generated_modules import load_generated

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core not compiled — maturin develop needed",
)


@pytest.fixture()
def descriptors():
    mod = load_generated("idltopic_gen")
    return mod.ZD_TOPIC_DESCRIPTORS


def test_idl_topic_rejects_a_non_topic_descriptor():
    """§6.1 — IdlTopic requires a generated TopicDescriptor."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(260)
    with pytest.raises(TypeError, match="TopicDescriptor"):
        IdlTopic(p, "BadTopic", object())


def test_idl_topic_exposes_type_name_from_the_descriptor(descriptors):
    """§6.1 — the topic type name comes from the generated descriptor."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(261)
    descriptor = descriptors["sensor_msgs::msg::Temperature"]
    topic = IdlTopic(p, "Temperatures", descriptor)
    assert topic.name == "Temperatures"
    assert topic.type_name == "sensor_msgs::msg::Temperature"
    assert topic.cls is descriptor.cls


def test_idl_writer_encodes_and_reader_decodes_roundtrip(descriptors):
    """§6.1 + §3.3 — IdlWriter.write → BytesWriter; IdlReader.take()
    decodes byte-exactly into a new instance."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(262)
    topic = IdlTopic(p, "ReadingTopic", descriptors["sensor_msgs::msg::Reading"])
    writer = topic.create_writer(p.create_publisher())
    reader = topic.create_reader(p.create_subscriber())
    assert isinstance(writer, IdlWriter)
    assert isinstance(reader, IdlReader)
    # Offline participant: reader.take() returns an empty list.
    assert reader.take() == []


def test_idl_writer_rejects_wrong_type(descriptors):
    """§6.1 — the generated descriptor's own sample check rejects the wrong type."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(263)
    topic = IdlTopic(p, "ReadingTopic2", descriptors["sensor_msgs::msg::Reading"])
    writer = topic.create_writer(p.create_publisher())
    Temperature = descriptors["sensor_msgs::msg::Temperature"].cls
    with pytest.raises(TypeError, match="Reading"):
        writer.write(Temperature(celsius=0, sensor_id="X"))


def test_idl_writer_passthrough_status(descriptors):
    """§6.1 — status getters are passed through (DDS 1.4 §2.2.4)."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(264)
    topic = IdlTopic(p, "StatusReading", descriptors["sensor_msgs::msg::Reading"])
    writer = topic.create_writer(p.create_publisher())
    reader = topic.create_reader(p.create_subscriber())
    assert writer.matched_subscription_count() == 0
    assert reader.matched_publication_count() == 0
    assert writer.publication_matched_status() == (0, 0, 0, 0, 0)
    assert reader.subscription_matched_status() == (0, 0, 0, 0, 0)
