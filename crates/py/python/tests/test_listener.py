"""Tests for §6.5 — DataWriterListener / DataReaderListener.

Verifies construction, callback registration, and set_listener on
reader/writer.
"""

from __future__ import annotations

import pytest

import zerodds

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core not compiled — maturin develop needed",
)


def test_writer_listener_constructible():
    """§6.5 — DataWriterListener accepts callbacks for all three slots."""
    listener = zerodds.DataWriterListener()
    listener.on_publication_matched(lambda handle, status: None)
    listener.on_liveliness_lost(lambda handle, status: None)
    listener.on_offered_deadline_missed(lambda handle, status: None)


def test_reader_listener_constructible():
    """§6.5 — DataReaderListener accepts all six reader callbacks."""
    listener = zerodds.DataReaderListener()
    listener.on_data_available(lambda handle: None)
    listener.on_sample_lost(lambda handle, status: None)
    listener.on_sample_rejected(lambda handle, status: None)
    listener.on_requested_deadline_missed(lambda handle, status: None)
    listener.on_liveliness_changed(lambda handle, status: None)
    listener.on_subscription_matched(lambda handle, status: None)


def test_set_listener_on_bytes_writer_offline():
    """§6.5 — Publisher.create_bytes_writer().set_listener(listener, mask)
    stores the listener without raising (offline path)."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(330)
    topic = p.create_bytes_topic("ListenerTopic")
    pub = p.create_publisher()
    writer = pub.create_bytes_writer(topic)
    listener = zerodds.DataWriterListener()
    listener.on_publication_matched(lambda handle, status: None)
    writer.set_listener(listener, 0)  # mask=0 → all bits
    writer.clear_listener()  # must likewise not raise


def test_set_listener_on_bytes_reader_offline():
    """§6.5 — Subscriber.create_bytes_reader().set_listener(listener, mask)."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(331)
    topic = p.create_bytes_topic("ListenerTopic2")
    sub = p.create_subscriber()
    reader = sub.create_bytes_reader(topic)
    listener = zerodds.DataReaderListener()
    listener.on_data_available(lambda handle: None)
    reader.set_listener(listener, 0)
    reader.clear_listener()


def test_set_listener_on_shape_writer_offline():
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(332)
    topic = p.create_shape_topic("Square")
    pub = p.create_publisher()
    writer = pub.create_shape_writer(topic)
    listener = zerodds.DataWriterListener()
    listener.on_publication_matched(lambda h, s: None)
    writer.set_listener(listener, 0)
    writer.clear_listener()


def test_set_listener_on_shape_reader_offline():
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(333)
    topic = p.create_shape_topic("Square")
    sub = p.create_subscriber()
    reader = sub.create_shape_reader(topic)
    listener = zerodds.DataReaderListener()
    listener.on_data_available(lambda h: None)
    reader.set_listener(listener, 0)
    reader.clear_listener()
