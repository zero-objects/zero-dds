"""Behavioral regression tests for the DDS QoS + lifecycle surface added to
the Python binding (task QB-cluster).

Covers, over real (live) DomainParticipant/DataWriter/DataReader:
  - SampleInfo via `take_with_info()` (instance_state / valid_data /
    per-instance handle) — DDS 1.4 §2.2.2.5.1.
  - Keyed lifecycle: register_instance / dispose → NOT_ALIVE_DISPOSED
    observable on the reader — §2.2.2.4.2.5/.10 + §2.2.2.5.1.13.
  - liveliness_changed_status getter — §2.2.4.2.14.
  - ContentFilteredTopic predicate filtering — §2.2.2.5.4.
  - BytesWriter lifecycle method presence — §2.2.2.4.2.

Live discovery uses `create_participant`; each test runs on its own domain
to avoid cross-test bleed.
"""

from __future__ import annotations

import time

import pytest

import zerodds
from zerodds import _core

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core not compiled — maturin develop needed",
)

_DOM = [40]


def _domain() -> int:
    _DOM[0] += 1
    return _DOM[0]


def _drain_info(reader, timeout=3.0):
    out = []
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            reader.wait_for_data(0.3)
        except Exception:
            pass
        batch = reader.take_with_info()
        if batch:
            out.extend(batch)
        elif out:
            break
    return out


# ---------------------------------------------------------------------------
# SampleInfo surface
# ---------------------------------------------------------------------------


def test_bytes_take_with_info_returns_sample_info():
    """§2.2.2.5.3.5 — take_with_info yields (bytes, SampleInfo) with an
    ALIVE instance_state for live data."""
    p = _core.DomainParticipantFactory.instance().create_participant(_domain())
    t = p.create_bytes_topic("InfoT")
    w = p.create_publisher().create_bytes_writer(t)
    r = p.create_subscriber().create_bytes_reader(t)
    w.wait_for_matched_subscription(1, 5.0)
    r.wait_for_matched_publication(1, 5.0)
    w.write(b"hello")
    got = _drain_info(r, timeout=3.0)
    assert len(got) >= 1
    data, info = got[0]
    assert data == b"hello"
    assert info.instance_state == "Alive"
    assert info.valid_data is True
    assert info.sample_state in ("Read", "NotRead")
    assert info.view_state in ("New", "NotNew")


def test_sample_info_exposed_in_module():
    assert hasattr(_core, "SampleInfo")


# ---------------------------------------------------------------------------
# Keyed lifecycle — dispose → NOT_ALIVE_DISPOSED
# ---------------------------------------------------------------------------


def test_keyed_dispose_observed_as_not_alive_disposed():
    """§2.2.2.4.2.10 dispose + §2.2.2.5.1.13 — the reader observes a
    NotAliveDisposed marker (valid_data False) carrying only the key."""
    p = _core.DomainParticipantFactory.instance().create_participant(_domain())
    t = p.create_keyed_topic("KeyT")
    w = p.create_publisher().create_keyed_writer(t)
    r = p.create_subscriber().create_keyed_reader(t)
    w.wait_for_matched_subscription(1, 5.0)
    r.wait_for_matched_publication(1, 5.0)

    w.write(_core.KeyedReading(id=1, seq=0, value=1.0))
    w.write(_core.KeyedReading(id=2, seq=0, value=2.0))
    alive = _drain_info(r, timeout=3.0)
    handles = {s.id: info.instance_handle for (s, info) in alive if info.valid_data}
    states = {s.id: info.instance_state for (s, info) in alive if info.valid_data}
    assert states.get(1) == "Alive"
    assert states.get(2) == "Alive"
    # Distinct per-key instance handles.
    assert len(set(handles.values())) == 2

    w.dispose(_core.KeyedReading(id=1))
    disp = _drain_info(r, timeout=3.0)
    disposed = [(s, info) for (s, info) in disp if info.instance_state == "NotAliveDisposed"]
    assert disposed, "expected a NotAliveDisposed marker after dispose(id=1)"
    # The dispose marker carries only the key (valid_data == False) and the
    # disposed instance is id=1.
    assert all(info.valid_data is False for (_s, info) in disposed)
    assert 1 in {s.id for (s, _info) in disposed}


def test_keyed_register_instance_returns_real_handle():
    """§2.2.2.4.2.5 — register_instance returns a non-nil per-key handle."""
    p = _core.DomainParticipantFactory.instance().create_participant(_domain())
    t = p.create_keyed_topic("KeyRegT")
    w = p.create_publisher().create_keyed_writer(t)
    h1 = w.register_instance(_core.KeyedReading(id=5))
    h2 = w.register_instance(_core.KeyedReading(id=5))
    h3 = w.register_instance(_core.KeyedReading(id=6))
    assert h1 != 0
    assert h1 == h2  # same key → same handle
    assert h1 != h3  # different key → different handle


# ---------------------------------------------------------------------------
# liveliness_changed_status getter
# ---------------------------------------------------------------------------


def test_reader_liveliness_changed_status_getter():
    """§2.2.4.2.14 — the reader exposes liveliness_changed_status as a
    (alive_now, alive_count, not_alive_count) tuple."""
    p = _core.DomainParticipantFactory.instance().create_participant(_domain())
    t = p.create_bytes_topic("LiveStatusT")
    r = p.create_subscriber().create_bytes_reader(t)
    st = r.liveliness_changed_status()
    assert isinstance(st, tuple) and len(st) == 3
    assert isinstance(st[0], bool)
    assert isinstance(st[1], int) and isinstance(st[2], int)


# ---------------------------------------------------------------------------
# ContentFilteredTopic
# ---------------------------------------------------------------------------


def test_content_filtered_topic_filters_samples():
    """§2.2.2.5.4 — a CFT predicate over the raw payload discards
    non-matching samples on the reader's take path."""
    p = _core.DomainParticipantFactory.instance().create_participant(_domain())
    t = p.create_bytes_topic("CftT")

    def keep(raw: bytes) -> bool:
        # Deliver only payloads whose first byte is even.
        return len(raw) > 0 and raw[0] % 2 == 0

    cft = p.create_bytes_contentfilteredtopic("CftEven", t, keep)
    assert cft.related_topic_name == "CftT"
    w = p.create_publisher().create_bytes_writer(t)
    r = p.create_subscriber().create_bytes_reader_cft(cft)
    w.wait_for_matched_subscription(1, 5.0)
    r.wait_for_matched_publication(1, 5.0)
    for i in range(10):
        w.write(bytes([i]))
    out = []
    deadline = time.time() + 4.0
    while len(out) < 5 and time.time() < deadline:
        try:
            r.wait_for_data(0.3)
        except Exception:
            pass
        out.extend(r.take())
    assert out, "expected some samples through the CFT"
    assert all(b[0] % 2 == 0 for b in out)


def test_cft_empty_name_rejected():
    p = _core.DomainParticipantFactory.instance().create_participant(_domain())
    t = p.create_bytes_topic("CftBadT")
    with pytest.raises(Exception):
        p.create_bytes_contentfilteredtopic("", t, lambda b: True)


# ---------------------------------------------------------------------------
# BytesWriter lifecycle surface (presence + no-raise)
# ---------------------------------------------------------------------------


def test_bytes_writer_lifecycle_methods_present():
    p = _core.DomainParticipantFactory.instance().create_participant_offline(_domain())
    t = p.create_bytes_topic("ByteLifeT")
    w = p.create_publisher().create_bytes_writer(t)
    for m in ("register_instance", "dispose", "unregister_instance", "lookup_instance"):
        assert hasattr(w, m), f"BytesWriter missing {m}"
    # RawBytes is unkeyed → lookup is HANDLE_NIL (0) and register is a no-op
    # returning the nil handle. dispose/unregister on an unkeyed type have no
    # per-instance target (DCPS returns BadParameter); the surface exists for
    # spec-shaped parity. Keyed lifecycle lives on KeyedWriter/ShapeWriter.
    assert w.lookup_instance(b"x") == 0
    assert w.register_instance(b"x") == 0
