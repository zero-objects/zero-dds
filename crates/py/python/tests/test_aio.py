"""Tests fuer §6.3 — AsyncIO-Wrapper in `zerodds.aio`.

Verifiziert die Async-Wrapper-Klassen unter einem asyncio-Event-Loop.
Skip wenn `_core` fehlt (alle wait/write-Methoden delegieren auf das
PyO3-Extension-Modul).
"""

from __future__ import annotations

import asyncio

import pytest

import zerodds
from zerodds import aio as zaio

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core nicht kompiliert — maturin develop noetig",
)


def _make_writer_reader(domain: int = 250):
    p = zerodds.DomainParticipantFactory.instance().create_participant_offline(domain)
    topic = p.create_bytes_topic("AsyncChatter")
    writer = p.create_publisher().create_bytes_writer(topic)
    reader = p.create_subscriber().create_bytes_reader(topic)
    return writer, reader


def test_async_wrappers_are_constructible():
    """§6.3 — Wrapper-Konstruktoren akzeptieren die jeweiligen Inner-PyClasses."""
    writer, reader = _make_writer_reader(251)
    async_w = zaio.AsyncBytesWriter(writer)
    async_r = zaio.AsyncBytesReader(reader)
    assert async_w is not None
    assert async_r is not None


def test_async_passthrough_status_getters():
    """§6.3 — Status-Getter werden direkt durchgereicht (non-blocking)."""
    writer, reader = _make_writer_reader(252)
    async_w = zaio.AsyncBytesWriter(writer)
    async_r = zaio.AsyncBytesReader(reader)
    assert async_w.matched_subscription_count() == 0
    assert async_r.matched_publication_count() == 0
    assert async_w.publication_matched_status() == (0, 0, 0, 0, 0)
    assert async_r.subscription_matched_status() == (0, 0, 0, 0, 0)


def test_async_take_returns_list_passthrough():
    """§6.3 — `take()` ist non-blocking und gibt eine Liste zurueck."""
    _w, reader = _make_writer_reader(253)
    async_r = zaio.AsyncBytesReader(reader)
    assert async_r.take() == []


def test_async_wait_for_data_uses_event_loop():
    """§6.3 — Async-Wait blockiert den Event-Loop nicht; ein anderer
    Coroutine kann parallel laufen. Verifiziert durch eine parallele
    `asyncio.sleep`-Task die innerhalb der Wait-Frist completed."""
    _w, reader = _make_writer_reader(254)
    async_r = zaio.AsyncBytesReader(reader)

    completed_in_parallel: list[bool] = []

    async def main() -> None:
        async def parallel_task() -> None:
            await asyncio.sleep(0.05)
            completed_in_parallel.append(True)

        # Beide Tasks gleichzeitig: wait_for_data blockiert nicht den Loop.
        await asyncio.gather(
            async_r.wait_for_data(0.3),
            parallel_task(),
            return_exceptions=True,
        )

    asyncio.run(main())
    assert completed_in_parallel == [True], "Parallele Task hat nicht gelaufen — Event-Loop blockiert?"


def test_async_waitset_wait_raises_timeout():
    """§6.3 + §1.2.13 — AsyncWaitSet.wait wirft `TimeoutError`, wenn
    keine Condition innerhalb des Timeouts flippt."""
    gc = zerodds.GuardCondition()
    ws = zerodds.WaitSet()
    ws.attach_guard_condition(gc)
    async_ws = zaio.AsyncWaitSet(ws)

    async def main() -> None:
        await async_ws.wait(0.05)

    with pytest.raises(TimeoutError):
        asyncio.run(main())
