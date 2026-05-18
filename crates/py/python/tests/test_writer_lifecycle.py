"""Tests fuer §2.4 — DataWriter Instance-Lifecycle.

`ShapeWriter.register_instance`/`dispose`/`unregister_instance` werden
in den Smoke-Tests nur indirekt durch das Live-Roundtrip aufgerufen.
Hier sequenziell, gegen einen offline-Participant, ohne Network-Traffic.
"""

from __future__ import annotations

import pytest

import zerodds

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core nicht kompiliert — maturin develop noetig",
)


def _writer_for(color: str = "BLUE", domain: int = 130):
    p = zerodds.DomainParticipantFactory.instance().create_participant_offline(domain)
    topic = p.create_shape_topic("Square")
    pub = p.create_publisher()
    return pub.create_shape_writer(topic), p


def test_register_instance_returns_nonzero_handle():
    """§2.4 — register_instance liefert einen positiven InstanceHandle
    (DDS 1.4 §2.2.2.4.2.5)."""
    writer, _p = _writer_for()
    handle = writer.register_instance(zerodds.Shape("RED", 0, 0, 30))
    assert isinstance(handle, int)
    assert handle != 0  # HANDLE_NIL ist 0; register liefert echten Handle


def test_register_instance_is_idempotent_for_same_key():
    """§2.4 — register_instance fuer den gleichen Key (color) liefert
    den gleichen Handle (DDS 1.4 §2.2.2.4.2.5: "If the instance was
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
    """§2.4 — dispose markiert die Instance als not-alive_disposed
    (DDS 1.4 §2.2.2.5.1.7); kein Error auf offline-Pfad."""
    writer, _p = _writer_for(domain=133)
    shape = zerodds.Shape("YELLOW", 10, 20, 30)
    writer.register_instance(shape)
    writer.dispose(shape)  # darf nicht raisen


def test_unregister_instance_does_not_raise():
    """§2.4 — unregister_instance entfernt die Instance aus dem
    Writer-Cache (DDS 1.4 §2.2.2.4.2.7)."""
    writer, _p = _writer_for(domain=134)
    shape = zerodds.Shape("MAGENTA", 5, 5, 30)
    writer.register_instance(shape)
    writer.unregister_instance(shape)  # darf nicht raisen


def test_lifecycle_full_sequence():
    """§2.4 — register → write → dispose → unregister-Sequenz ohne Raise."""
    writer, _p = _writer_for(domain=135)
    shape = zerodds.Shape("CYAN", 7, 8, 30)
    writer.register_instance(shape)
    writer.write(shape)
    writer.dispose(shape)
    writer.unregister_instance(shape)
