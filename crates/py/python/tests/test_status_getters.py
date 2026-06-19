"""Tests for §2.5 — DataReader/Writer status getters.

Verifies the tuple shape of the status getters against the spec
(DDS 1.4 §2.2.4). On an offline participant the counters are
all 0 and the last handle 0; the shape must be correct nonetheless.
"""

from __future__ import annotations

import pytest

import zerodds

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core not compiled — maturin develop needed",
)


def _make_writer_reader(domain: int):
    p = zerodds.DomainParticipantFactory.instance().create_participant_offline(domain)
    topic = p.create_bytes_topic("StatusTopic")
    writer = p.create_publisher().create_bytes_writer(topic)
    reader = p.create_subscriber().create_bytes_reader(topic)
    return writer, reader


def test_publication_matched_status_shape():
    """§2.5 — publication_matched_status returns
    `(total_count, total_count_change, current_count, current_count_change, last_subscription_handle)`
    per DDS 1.4 §2.2.4.6 PublicationMatchedStatus."""
    writer, _r = _make_writer_reader(140)
    status = writer.publication_matched_status()
    assert len(status) == 5
    total_count, total_change, current_count, current_change, last_handle = status
    assert all(isinstance(v, int) for v in status)
    assert total_count == 0
    assert total_change == 0
    assert current_count == 0
    assert current_change == 0
    assert last_handle == 0  # HANDLE_NIL


def test_subscription_matched_status_shape():
    """§2.5 — DDS 1.4 §2.2.4.5 SubscriptionMatchedStatus."""
    _w, reader = _make_writer_reader(141)
    status = reader.subscription_matched_status()
    assert len(status) == 5
    assert all(isinstance(v, int) for v in status)
    assert status == (0, 0, 0, 0, 0)


def test_liveliness_lost_status_shape():
    """§2.5 — DDS 1.4 §2.2.4.2 LivelinessLostStatus = (total_count, total_count_change)."""
    writer, _r = _make_writer_reader(142)
    status = writer.liveliness_lost_status()
    assert len(status) == 2
    assert all(isinstance(v, int) for v in status)
    assert status == (0, 0)


def test_offered_deadline_missed_status_shape():
    """§2.5 — DDS 1.4 §2.2.4.1 OfferedDeadlineMissedStatus = (total_count, total_count_change)."""
    writer, _r = _make_writer_reader(143)
    status = writer.offered_deadline_missed_status()
    assert len(status) == 2
    assert all(isinstance(v, int) for v in status)
    assert status == (0, 0)


def test_sample_lost_status_shape():
    """§2.5 — DDS 1.4 §2.2.4.3 SampleLostStatus = (total_count, total_count_change)."""
    _w, reader = _make_writer_reader(144)
    status = reader.sample_lost_status()
    assert len(status) == 2
    assert all(isinstance(v, int) for v in status)
    assert status == (0, 0)


def test_requested_deadline_missed_status_shape():
    """§2.5 — DDS 1.4 §2.2.4.4 RequestedDeadlineMissedStatus = (total_count, total_count_change)."""
    _w, reader = _make_writer_reader(145)
    status = reader.requested_deadline_missed_status()
    assert len(status) == 2
    assert all(isinstance(v, int) for v in status)
    assert status == (0, 0)


def test_matched_subscription_count_zero_offline():
    writer, _r = _make_writer_reader(146)
    assert writer.matched_subscription_count() == 0


def test_matched_publication_count_zero_offline():
    _w, reader = _make_writer_reader(147)
    assert reader.matched_publication_count() == 0
