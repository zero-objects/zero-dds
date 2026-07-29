<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Python endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading(id, value, label)` samples and delivers them; a subscriber decodes
**every field**.

```sh
cd endpoints/python
python3 example_sync.py     # subscriber owns the run-loop and polls
python3 example_async.py    # subscriber iterates an asyncio async generator
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking; `None` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.stream()` is an `asyncio` async generator;
  the consumer iterates it with `async for`. The idiomatic asyncio model
  (Python 3 only).

## Transport

A transport is any object with `deliver(frame)` and `receive() -> frame|None`.
The examples use the in-memory `MemTransport` (a FIFO); a real UDP or serial link
is a drop-in — `zerodds_endpoint.py` also carries the XRCE and HDLC-serial framing.

## Wire & codegen

The endpoint wire-core (`zerodds_wire.py`) is pure stdlib, byte-identical to the
Rust core (XCDR2, align cap 4, `f32` via `struct.pack("<f")`). IDL types can also
be generated with `zerodds-idlc --python` — a full IDL4 DataType codegen
(`@idl_struct` dataclasses run against the `zerodds` runtime) — see
[`docs/specs/zerodds-xcdr2-python-1.0.md`](../../docs/specs/zerodds-xcdr2-python-1.0.md).
