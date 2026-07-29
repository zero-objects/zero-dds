<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS D endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/d
make example_sync     # subscriber polls
make example_async    # subscriber consumes via the std.concurrency actor
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking; `null` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader` spawns a `std.concurrency` thread; you
  `feed` frames and `recv()` decoded bodies (message passing, `immutable(ubyte)[]`
  payloads). The idiom for actor-style consumers.

## Transport

`Transport` is a pair of delegates (`deliver` / `receive`). The examples use the
in-memory `memTransport()`; a real UDP or shared-memory link is a drop-in.

## Wire

Byte-identical to the Rust core and every other ZeroDDS endpoint (XCDR2, align
cap 4, `f32` via `*cast(uint*)&v`). The same types can be generated from IDL with
`zerodds-idlc --d` — see
[`docs/specs/zerodds-xcdr2-d-1.0.md`](../../docs/specs/zerodds-xcdr2-d-1.0.md).
