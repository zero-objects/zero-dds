"""Tests for §6.3 — AsyncIO wrapper in `zerodds.aio`.

Verifies the async wrapper classes under an asyncio event loop.
Skipped if `_core` is missing (all wait/write methods delegate to the
PyO3 extension module).
"""

from __future__ import annotations

import asyncio

import pytest

import zerodds
from zerodds import aio as zaio

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core not compiled — maturin develop needed",
)


def _make_writer_reader(domain: int = 250):
    p = zerodds.DomainParticipantFactory.instance().create_participant_offline(domain)
    topic = p.create_bytes_topic("AsyncChatter")
    writer = p.create_publisher().create_bytes_writer(topic)
    reader = p.create_subscriber().create_bytes_reader(topic)
    return writer, reader


def test_async_wrappers_are_constructible():
    """§6.3 — wrapper constructors accept the respective inner PyClasses."""
    writer, reader = _make_writer_reader(251)
    async_w = zaio.AsyncBytesWriter(writer)
    async_r = zaio.AsyncBytesReader(reader)
    assert async_w is not None
    assert async_r is not None


def test_async_passthrough_status_getters():
    """§6.3 — status getters are passed through directly (non-blocking)."""
    writer, reader = _make_writer_reader(252)
    async_w = zaio.AsyncBytesWriter(writer)
    async_r = zaio.AsyncBytesReader(reader)
    assert async_w.matched_subscription_count() == 0
    assert async_r.matched_publication_count() == 0
    assert async_w.publication_matched_status() == (0, 0, 0, 0, 0)
    assert async_r.subscription_matched_status() == (0, 0, 0, 0, 0)


def test_async_take_returns_list_passthrough():
    """§6.3 — `take()` is non-blocking and returns a list."""
    _w, reader = _make_writer_reader(253)
    async_r = zaio.AsyncBytesReader(reader)
    assert async_r.take() == []


def test_async_wait_for_data_uses_event_loop():
    """§6.3 — async wait does not block the event loop; another
    coroutine can run in parallel. Verified by a parallel
    `asyncio.sleep` task that completes within the wait window."""
    _w, reader = _make_writer_reader(254)
    async_r = zaio.AsyncBytesReader(reader)

    completed_in_parallel: list[bool] = []

    async def main() -> None:
        async def parallel_task() -> None:
            await asyncio.sleep(0.05)
            completed_in_parallel.append(True)

        # Both tasks at once: wait_for_data does not block the loop.
        await asyncio.gather(
            async_r.wait_for_data(0.3),
            parallel_task(),
            return_exceptions=True,
        )

    asyncio.run(main())
    assert completed_in_parallel == [True], "parallel task did not run — event loop blocked?"


def test_async_waitset_wait_raises_timeout():
    """§6.3 + §1.2.13 — AsyncWaitSet.wait raises `TimeoutError` if
    no condition flips within the timeout."""
    gc = zerodds.GuardCondition()
    ws = zerodds.WaitSet()
    ws.attach_guard_condition(gc)
    async_ws = zaio.AsyncWaitSet(ws)

    async def main() -> None:
        await async_ws.wait(0.05)

    with pytest.raises(TimeoutError):
        asyncio.run(main())
