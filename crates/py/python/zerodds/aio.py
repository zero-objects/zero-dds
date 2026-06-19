"""AsyncIO wrapper for the ZeroDDS Python binding (§6.3 vendor spec).

The `zerodds._core` PyO3 layer is sync-only and releases the GIL for
blocking calls. This wrapper binds the blocking calls onto
``asyncio.to_thread`` so that they are awaitable in an asyncio event loop
without blocking the event loop.

Example::

    import asyncio
    import zerodds
    import zerodds.aio as aio

    async def main():
        factory = zerodds.DomainParticipantFactory.instance()
        p = factory.create_participant_fast(0)
        topic = p.create_bytes_topic("Chatter")
        writer = aio.AsyncBytesWriter(p.create_publisher().create_bytes_writer(topic))
        reader = aio.AsyncBytesReader(p.create_subscriber().create_bytes_reader(topic))
        await writer.wait_for_matched_subscription(1, timeout_secs=5.0)
        await reader.wait_for_matched_publication(1, timeout_secs=5.0)
        await writer.write(b"hello")
        await reader.wait_for_data(3.0)
        for payload in reader.take():
            print(payload)

    asyncio.run(main())

The classes are thin wrappers: all methods that do not block
(e.g. ``take``, status getters) are passed through directly. Only the
``wait_*``/``write`` methods are lifted onto a worker thread via
``asyncio.to_thread``.
"""

from __future__ import annotations

import asyncio
from typing import Any


# ---------------------------------------------------------------------------
# Mixin: shared "to_thread" bridge
# ---------------------------------------------------------------------------


async def _to_thread(func: Any, /, *args: Any, **kwargs: Any) -> Any:
    """Backport-friendly ``asyncio.to_thread`` variant.

    ``asyncio.to_thread`` exists from Python 3.9. Since ``zerodds-py``
    is abi3-py38 and the outer Python is potentially Py3.8, we fall
    back here to an executor-based variant.
    """
    if hasattr(asyncio, "to_thread"):
        return await asyncio.to_thread(func, *args, **kwargs)
    loop = asyncio.get_running_loop()

    def _run() -> Any:
        return func(*args, **kwargs)

    return await loop.run_in_executor(None, _run)


# ---------------------------------------------------------------------------
# AsyncBytesWriter / AsyncBytesReader
# ---------------------------------------------------------------------------


class AsyncBytesWriter:
    """Async wrapper around ``zerodds.BytesWriter`` (§2.4 vendor spec)."""

    def __init__(self, inner: Any) -> None:
        self._inner = inner

    async def write(self, data: bytes) -> None:
        await _to_thread(self._inner.write, data)

    async def wait_for_matched_subscription(
        self, count: int, timeout_secs: float,
    ) -> None:
        await _to_thread(
            self._inner.wait_for_matched_subscription, count, timeout_secs,
        )

    # --- Pass-through for non-blocking calls ---
    def matched_subscription_count(self) -> int:
        return self._inner.matched_subscription_count()

    def publication_matched_status(self) -> tuple:
        return self._inner.publication_matched_status()

    def liveliness_lost_status(self) -> tuple:
        return self._inner.liveliness_lost_status()

    def offered_deadline_missed_status(self) -> tuple:
        return self._inner.offered_deadline_missed_status()


class AsyncBytesReader:
    """Async wrapper around ``zerodds.BytesReader`` (§2.5 vendor spec)."""

    def __init__(self, inner: Any) -> None:
        self._inner = inner

    async def wait_for_data(self, timeout_secs: float) -> None:
        await _to_thread(self._inner.wait_for_data, timeout_secs)

    async def wait_for_matched_publication(
        self, count: int, timeout_secs: float,
    ) -> None:
        await _to_thread(
            self._inner.wait_for_matched_publication, count, timeout_secs,
        )

    # --- non-blocking ---
    def take(self) -> list[bytes]:
        return self._inner.take()

    def matched_publication_count(self) -> int:
        return self._inner.matched_publication_count()

    def subscription_matched_status(self) -> tuple:
        return self._inner.subscription_matched_status()

    def sample_lost_status(self) -> tuple:
        return self._inner.sample_lost_status()

    def requested_deadline_missed_status(self) -> tuple:
        return self._inner.requested_deadline_missed_status()


# ---------------------------------------------------------------------------
# AsyncShapeWriter / AsyncShapeReader
# ---------------------------------------------------------------------------


class AsyncShapeWriter:
    """Async wrapper around ``zerodds.ShapeWriter`` (§2.4 + §2.7)."""

    def __init__(self, inner: Any) -> None:
        self._inner = inner

    async def write(self, shape: Any) -> None:
        await _to_thread(self._inner.write, shape)

    async def wait_for_matched_subscription(
        self, count: int, timeout_secs: float,
    ) -> None:
        await _to_thread(
            self._inner.wait_for_matched_subscription, count, timeout_secs,
        )

    def register_instance(self, shape: Any) -> int:
        return self._inner.register_instance(shape)

    def dispose(self, shape: Any) -> None:
        self._inner.dispose(shape)

    def unregister_instance(self, shape: Any) -> None:
        self._inner.unregister_instance(shape)


class AsyncShapeReader:
    """Async wrapper around ``zerodds.ShapeReader`` (§2.5 + §2.7)."""

    def __init__(self, inner: Any) -> None:
        self._inner = inner

    async def wait_for_data(self, timeout_secs: float) -> None:
        await _to_thread(self._inner.wait_for_data, timeout_secs)

    async def wait_for_matched_publication(
        self, count: int, timeout_secs: float,
    ) -> None:
        await _to_thread(
            self._inner.wait_for_matched_publication, count, timeout_secs,
        )

    def take(self) -> list[Any]:
        return self._inner.take()


# ---------------------------------------------------------------------------
# AsyncWaitSet
# ---------------------------------------------------------------------------


class AsyncWaitSet:
    """Async wrapper around ``zerodds.WaitSet`` (§2.6 + §1.2.13)."""

    def __init__(self, inner: Any) -> None:
        self._inner = inner

    def attach_guard_condition(self, gc: Any) -> None:
        self._inner.attach_guard_condition(gc)

    async def wait(self, timeout_secs: float) -> int:
        return await _to_thread(self._inner.wait, timeout_secs)


__all__ = [
    "AsyncBytesReader",
    "AsyncBytesWriter",
    "AsyncShapeReader",
    "AsyncShapeWriter",
    "AsyncWaitSet",
]
