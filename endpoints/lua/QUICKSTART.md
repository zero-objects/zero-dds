<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Lua endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/lua
lua5.4 example_sync.lua     # subscriber owns the run-loop and polls
lua5.4 example_async.lua    # subscriber resumes a coroutine producer
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client:poll` in a loop (non-blocking; `nil` when empty).
  The idiom when you own the run-loop.
- **`example_async`** — `asyncReader` is a `coroutine.wrap` producer; each resume
  yields the next decoded sample (or `nil` when momentarily empty). The idiomatic
  Lua cooperative-concurrency model.

## Transport

`transport` is `{ deliver = function(frame), receive = function() }`. The examples
use the in-memory `memTransport` (a FIFO); a real UDP or shared-memory link is a
drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4) — built on `string.pack` /
`string.unpack` (Lua 5.3+), `f32` via the `"f"` format. The same types can be
generated from IDL with `zerodds-idlc --lua` — see
[`docs/specs/zerodds-xcdr2-lua-1.0.md`](../../docs/specs/zerodds-xcdr2-lua-1.0.md).
