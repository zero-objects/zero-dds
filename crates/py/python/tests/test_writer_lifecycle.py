"""Tests for §2.4 — DataWriter instance lifecycle.

`ShapeWriter.register_instance`/`dispose`/`unregister_instance` are
called only indirectly via the live roundtrip in the smoke tests.
Here sequentially, against an offline participant, without network traffic.
"""

from __future__ import annotations

import pytest

import zerodds

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core not compiled — maturin develop needed",
)


def _writer_for(color: str = "BLUE", domain: int = 130):
    p = zerodds.DomainParticipantFactory.instance().create_participant_offline(domain)
    topic = p.create_shape_topic("Square")
    pub = p.create_publisher()
    return pub.create_shape_writer(topic), p


def test_register_instance_returns_nonzero_handle():
    """§2.4 — register_instance returns a positive InstanceHandle
    (DDS 1.4 §2.2.2.4.2.5)."""
    writer, _p = _writer_for()
    handle = writer.register_instance(zerodds.Shape("RED", 0, 0, 30))
    assert isinstance(handle, int)
    assert handle != 0  # HANDLE_NIL is 0; register returns a real handle


def test_register_instance_is_idempotent_for_same_key():
    """§2.4 — register_instance for the same key (color) returns
    the same handle (DDS 1.4 §2.2.2.4.2.5: "If the instance was
    already registered ... an InstanceHandle_t is returned")."""
    writer, _p = _writer_for(domain=131)
    h1 = writer.register_instance(zerodds.Shape("GREEN", 1, 2, 30))
    h2 = writer.register_instance(zerodds.Shape("GREEN", 5, 6, 30))
    assert h1 == h2


def test_register_different_keys_yield_different_handles():
    writer, _p = _writer_for(domain=132)
    h_red = writer.register_instance(zerodds.Shape("RED", 0, 0, 30))
    h_blue = writer.register_instance(zerodds.Shape("BLUE", 0, 0, 30))
    assert h_red != h_blue


def test_dispose_does_not_raise():
    """§2.4 — dispose marks the instance as not-alive_disposed
    (DDS 1.4 §2.2.2.5.1.7); no error on the offline path."""
    writer, _p = _writer_for(domain=133)
    shape = zerodds.Shape("YELLOW", 10, 20, 30)
    writer.register_instance(shape)
    writer.dispose(shape)  # must not raise


def test_unregister_instance_does_not_raise():
    """§2.4 — unregister_instance removes the instance from the
    writer cache (DDS 1.4 §2.2.2.4.2.7)."""
    writer, _p = _writer_for(domain=134)
    shape = zerodds.Shape("MAGENTA", 5, 5, 30)
    writer.register_instance(shape)
    writer.unregister_instance(shape)  # must not raise


def test_lifecycle_full_sequence():
    """§2.4 — register → write → dispose → unregister sequence without raising."""
    writer, _p = _writer_for(domain=135)
    shape = zerodds.Shape("CYAN", 7, 8, 30)
    writer.register_instance(shape)
    writer.write(shape)
    writer.dispose(shape)
    writer.unregister_instance(shape)
