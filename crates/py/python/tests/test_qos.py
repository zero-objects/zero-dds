"""Tests fuer §6.2 — QoS-Builder (DataWriterQos + DataReaderQos).

Verifiziert die 22-Policy-Surface (DDS 1.4 §2.2.3) ueber die
`DataWriterQos`/`DataReaderQos`-PyClasses und die
`create_*_writer_with_qos`/`create_*_reader_with_qos`-Methoden.
"""

from __future__ import annotations

import pytest

import zerodds

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core nicht kompiliert — maturin develop noetig",
)


# ---------------------------------------------------------------------------
# Reflection-API
# ---------------------------------------------------------------------------


def test_writer_qos_defaults():
    """§6.2 — Default ist Reliable + Volatile + KeepLast(1) (DDS 1.4 §2.2.3 Defaults)."""
    qos = zerodds.DataWriterQos()
    assert qos.reliability_kind() == "Reliable"
    assert qos.durability_kind() == "Volatile"
    assert qos.history_kind() == "KeepLast"
    assert qos.history_depth() == 1


def test_reader_qos_defaults():
    """§6.2 — Reader-Default ist BestEffort (DDS 1.4 §2.2.3.14.3)."""
    qos = zerodds.DataReaderQos()
    assert qos.reliability_kind() == "BestEffort"
    assert qos.durability_kind() == "Volatile"


def test_writer_qos_reliability_setter():
    """§6.2 — `set_reliability` flippt zwischen Reliable und BestEffort."""
    qos = zerodds.DataWriterQos()
    qos.set_reliability("BestEffort", 0.0)
    assert qos.reliability_kind() == "BestEffort"
    qos.set_reliability("Reliable", 0.5)
    assert qos.reliability_kind() == "Reliable"


def test_writer_qos_rejects_unknown_reliability_kind():
    qos = zerodds.DataWriterQos()
    with pytest.raises(ValueError, match="ReliabilityKind"):
        qos.set_reliability("Wishful", 0.1)


def test_writer_qos_durability_all_kinds():
    """§6.2 — DurabilityKind Volatile/TransientLocal/Transient/Persistent."""
    qos = zerodds.DataWriterQos()
    for k in ("Volatile", "TransientLocal", "Transient", "Persistent"):
        qos.set_durability(k)
        assert qos.durability_kind() == k


def test_writer_qos_history_setter():
    qos = zerodds.DataWriterQos()
    qos.set_history("KeepAll", 0)
    assert qos.history_kind() == "KeepAll"
    qos.set_history("KeepLast", 100)
    assert qos.history_kind() == "KeepLast"
    assert qos.history_depth() == 100


def test_writer_qos_setters_with_durations():
    """§6.2 — Duration-Setter (Deadline, Lifespan, LatencyBudget, Liveliness)."""
    qos = zerodds.DataWriterQos()
    qos.set_deadline(2.5)
    qos.set_lifespan(60.0)
    qos.set_latency_budget(0.01)
    qos.set_liveliness("ManualByTopic", 5.0)
    qos.set_ownership("Exclusive")
    qos.set_ownership_strength(100)
    qos.set_partition(["odom", "scan"])
    qos.set_presentation("Topic", True, False)
    qos.set_destination_order("BySourceTimestamp")
    qos.set_resource_limits(10000, 100, 100)
    qos.set_transport_priority(7)
    qos.set_writer_data_lifecycle(False)
    qos.set_user_data(b"hello")
    qos.set_topic_data(b"topic-meta")
    qos.set_group_data(b"group-meta")
    qos.set_durability_service(10.0, "KeepLast", 50, 5000, 50, 100)


def test_reader_qos_full_setter_chain():
    """§6.2 — DataReaderQos hat reader-spezifische Policies (time_based_filter,
    reader_data_lifecycle); Writer-spezifische (ownership_strength,
    writer_data_lifecycle, durability_service, transport_priority) sind
    NICHT auf Reader."""
    qos = zerodds.DataReaderQos()
    qos.set_reliability("Reliable", 0.1)
    qos.set_durability("TransientLocal")
    qos.set_deadline(1.0)
    qos.set_latency_budget(0.001)
    qos.set_liveliness("Automatic", -1.0)  # negativ → INFINITE
    qos.set_destination_order("ByReceptionTimestamp")
    qos.set_ownership("Shared")
    qos.set_partition([])
    qos.set_presentation("Instance", False, False)
    qos.set_history("KeepAll", 0)
    qos.set_resource_limits(500, 10, 50)
    qos.set_time_based_filter(0.05)
    qos.set_reader_data_lifecycle(60.0, 30.0)
    qos.set_user_data(b"")
    qos.set_topic_data(b"")
    qos.set_group_data(b"")
    assert qos.reliability_kind() == "Reliable"
    assert qos.durability_kind() == "TransientLocal"


# ---------------------------------------------------------------------------
# Integration: create_*_with_qos uses the QoS
# ---------------------------------------------------------------------------


def test_create_bytes_writer_with_qos_offline():
    """§6.2 — Publisher.create_bytes_writer_with_qos akzeptiert eine
    DataWriterQos und erzeugt einen Writer."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(310)
    topic = p.create_bytes_topic("QosTopic")
    pub = p.create_publisher()
    qos = zerodds.DataWriterQos()
    qos.set_reliability("BestEffort", 0.0)
    qos.set_history("KeepLast", 5)
    writer = pub.create_bytes_writer_with_qos(topic, qos)
    assert writer is not None
    assert writer.matched_subscription_count() == 0


def test_create_bytes_reader_with_qos_offline():
    """§6.2 — Subscriber.create_bytes_reader_with_qos akzeptiert eine
    DataReaderQos."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(311)
    topic = p.create_bytes_topic("QosTopic2")
    sub = p.create_subscriber()
    qos = zerodds.DataReaderQos()
    qos.set_reliability("Reliable", 0.1)
    qos.set_history("KeepAll", 0)
    reader = sub.create_bytes_reader_with_qos(topic, qos)
    assert reader is not None
    assert reader.matched_publication_count() == 0


def test_create_shape_writer_with_qos_offline():
    """§6.2 + §2.7 — Publisher.create_shape_writer_with_qos."""
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(312)
    topic = p.create_shape_topic("Square")
    pub = p.create_publisher()
    qos = zerodds.DataWriterQos()
    qos.set_reliability("Reliable", 0.05)
    writer = pub.create_shape_writer_with_qos(topic, qos)
    assert writer is not None


def test_create_shape_reader_with_qos_offline():
    factory = zerodds.DomainParticipantFactory.instance()
    p = factory.create_participant_offline(313)
    topic = p.create_shape_topic("Square")
    sub = p.create_subscriber()
    qos = zerodds.DataReaderQos()
    qos.set_reliability("Reliable", 0.05)
    reader = sub.create_shape_reader_with_qos(topic, qos)
    assert reader is not None
