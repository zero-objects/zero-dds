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
`udp.js` provides a `UdpTransport` that carries the sync/async loopback over a
real socket:

```sh
node example_ping_pong.js sync  <peer-port>   # Client.poll() owns the loop
node example_ping_pong.js async <peer-port>   # AsyncReader.stream() drains it
```

## Reliable stream (RFC-1982 HEARTBEAT / ACKNACK / retransmit)

`reliable.js` adds a genuine reliable writer (DDS-XRCE §8.4.10/§8.4.11),
byte-identical to `endpoints/c` and `endpoints/java`: a 16-bit RFC-1982 window,
`WRITE_DATA` / `HEARTBEAT` / `ACKNACK` frames, and ACKNACK-driven retransmit.
`AsyncReliableWriter` is the async-decoupled sender — the producer `submit(...)`
is a Promise-based enqueue with backpressure on overflow (no sample dropped, no
producer hang); a drain loop owns the send window and does the I/O.

```sh
node example_reliable.js <peer-port> [N]   # submit N, recover loss, drain window
node reliable_selftest.js                  # unit + byte-golden (prints ALL OK)
```

Live loss-recovery + baseline + unit/golden run against the shared Rust peer in
`crates/endpoint-e2e/tests/node_reliable.rs`; sync/async ping-pong in `node.rs`.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via
`Buffer.writeFloatLE`, `u64` via `BigInt`). IDL types are generated as TypeScript
with `zerodds-idlc --ts` and run on Node — see
[`docs/specs/zerodds-xcdr2-node-1.0.md`](../../docs/specs/zerodds-xcdr2-node-1.0.md).
