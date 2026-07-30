<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — Python (binding async surface)

The Python binding (`zerodds`, PyO3) has an **asyncio** wrapper in
[`zerodds.aio`](../../crates/py/python/zerodds/aio.py). The PyO3 core is
sync-only and releases the GIL on blocking calls; `aio` lifts those onto
`asyncio.to_thread` (with a `run_in_executor` fallback for Python 3.8) so they
are awaitable without blocking the event loop. Runtime-agnostic — no
`pyo3-asyncio` runtime is pinned.

Tests: [`test_aio.py`](../../crates/py/python/tests/test_aio.py).

## Surface

```python
import asyncio, zerodds
import zerodds.aio as aio

async def main():
    p = zerodds.DomainParticipantFactory.instance().create_participant_fast(0)
    topic = p.create_bytes_topic("Chatter")
    writer = aio.AsyncBytesWriter(p.create_publisher().create_bytes_writer(topic))
    reader = aio.AsyncBytesReader(p.create_subscriber().create_bytes_reader(topic))
    await writer.wait_for_matched_subscription(1, timeout_secs=5.0)
    await writer.write(b"hello")
    await reader.wait_for_data(3.0)
    for payload in reader.take():
        print(payload)

asyncio.run(main())
```

Non-blocking methods (`take`, status getters) pass through directly; only the
`wait_*`/`write` methods are lifted onto a worker thread.

The tests skip gracefully when `zerodds._core` is not compiled (`maturin
develop`); the async wrapper delegates to the PyO3 extension module.
