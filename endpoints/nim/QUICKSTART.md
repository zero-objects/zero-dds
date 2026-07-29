<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Nim endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/nim
nim c -r example_sync.nim     # subscriber polls with a bounded retry
nim c -r example_async.nim    # subscriber awaits the AsyncReader Future
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking, `Option`). The
  idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.recv()` returns a `Future[seq[byte]]`;
  `await` it on the `asyncdispatch` event loop. The idiom for `async`/`await`
  consumers.

## Transport

`Transport` is a pair of closures (`deliver` / `receive`). The examples use the
in-memory `memTransport()`; a real UDP or shared-memory link is a drop-in.

## Wire

Byte-identical to the Rust core and every other ZeroDDS endpoint (XCDR2, align
cap 4, `f32` via `cast[uint32]`). The same types can be generated from IDL with
`zerodds-idlc --nim` — see
[`docs/specs/zerodds-xcdr2-nim-1.0.md`](../../docs/specs/zerodds-xcdr2-nim-1.0.md).
