"""Tests fuer §6.1 — IDL-Topic-Pfad.

Verifiziert `zerodds.IdlTopic` / `IdlWriter` / `IdlReader` als
pure-Python-Wrapper ueber `BytesTopic`-Surface.
"""

from __future__ import annotations

from dataclasses import dataclass

import pytest

import zerodds
from zerodds.idl import Bytes, Float32, Int32, String, idl_struct
from zerodds.topic import IdlReader, IdlTopic, IdlWriter

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core nicht kompiliert — maturin develop noetig",
)


@idl_struct(typename="sensor_msgs::msg::Temperature")
@dataclass
class Temperature:
    celsius: Int32
    sensor_id: String


@idl_struct(typename="sensor_msgs::msg::Reading")
@dataclass
class Reading:
    value: Float32
    label: String
    payload: Bytes = b""


def test_idl_topic_rejects_non_idl_struct():
    """§6.1 — IdlTopic verlangt eine @idl_struct-dekorierte Klasse."""
    @dataclass
    class NotAnIdlStruct:
        x: int = 0

    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(260)
    with pytest.raises(TypeError, match="@idl_struct"):
        IdlTopic(p, "BadTopic", NotAnIdlStruct)


def test_idl_topic_exposes_type_name_from_decorator():
    """§6.1 — Topic-Type-Name kommt aus dem `@idl_struct(typename=...)`-Decorator."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(261)
    topic = IdlTopic(p, "Temperatures", Temperature)
    assert topic.name == "Temperatures"
    assert topic.type_name == "sensor_msgs::msg::Temperature"
    assert topic.cls is Temperature


def test_idl_writer_encodes_and_reader_decodes_roundtrip():
    """§6.1 + §3.3 — IdlWriter.encode → BytesWriter; IdlReader.take()
    decoded byte-genau in eine neue Instance."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(262)
    topic = IdlTopic(p, "ReadingTopic", Reading)
    writer = topic.create_writer(p.create_publisher())
    reader = topic.create_reader(p.create_subscriber())
    assert isinstance(writer, IdlWriter)
    assert isinstance(reader, IdlReader)
    # Offline-Participant: reader.take() liefert leere Liste.
    assert reader.take() == []


def test_idl_writer_rejects_wrong_type():
    """§6.1 — IdlWriter[T].write rejected ein Argument vom falschen Typ."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(263)
    topic = IdlTopic(p, "ReadingTopic2", Reading)
    writer = topic.create_writer(p.create_publisher())
    with pytest.raises(TypeError, match="erwartet Reading"):
        writer.write(Temperature(celsius=0, sensor_id="X"))


def test_idl_writer_passthrough_status():
    """§6.1 — Status-Getter werden durchgereicht (DDS 1.4 §2.2.4)."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(264)
    topic = IdlTopic(p, "StatusReading", Reading)
    writer = topic.create_writer(p.create_publisher())
    reader = topic.create_reader(p.create_subscriber())
    assert writer.matched_subscription_count() == 0
    assert reader.matched_publication_count() == 0
    assert writer.publication_matched_status() == (0, 0, 0, 0, 0)
    assert reader.subscription_matched_status() == (0, 0, 0, 0, 0)
