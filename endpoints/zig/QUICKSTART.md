<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Zig endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples as XRCE WRITE_DATA and delivers them
over a transport; a subscriber decodes **every field**.

```sh
cd endpoints/zig
zig build run-example_sync    # subscriber polls (pull)
zig build run-example_async   # subscriber uses the callback-reactor AsyncReader
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking pull). Zig has no
  async/await; this is the idiom when you own the run-loop.
- **`example_async`** — `AsyncReader` is a callback reactor: `run(n)` drains the
  transport and dispatches each decoded frame to your `on_sample` callback. The
  push idiom for event-driven consumers.

## Transport

`Transport` is a function-pointer vtable (`deliver` / `receive`, no heap). The
examples use a tiny in-memory `Fifo`; a real UDP or shared-memory link is a
drop-in.

## Wire

Byte-identical to the Rust core and every other ZeroDDS endpoint (XCDR2, align
cap 4, `f32` via `@bitCast`). The same types can be generated from IDL with
`zerodds-idlc --zig` — see
[`docs/specs/zerodds-xcdr2-zig-1.0.md`](../../docs/specs/zerodds-xcdr2-zig-1.0.md).
