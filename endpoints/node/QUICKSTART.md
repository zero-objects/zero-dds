<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Node endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/node
node example_sync.js     # subscriber polls
node example_async.js    # subscriber iterates the async stream (`for await`)
```

Both print five decoded readings and `ALL OK`. No dependencies — just Node 18+.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking; `null` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.stream()` is an async iterator; `for await
  (const body of ...)` yields decoded frames, `close()` stops it. The idiomatic
  Node async model.

## Transport

`Transport` is `{ deliver, receive }`. The examples use the in-memory
`MemTransport`; a real UDP (`dgram`) or shared-memory link is a drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via
`Buffer.writeFloatLE`, `u64` via `BigInt`). IDL types are generated as TypeScript
with `zerodds-idlc --ts` and run on Node — see
[`docs/specs/zerodds-xcdr2-node-1.0.md`](../../docs/specs/zerodds-xcdr2-node-1.0.md).
