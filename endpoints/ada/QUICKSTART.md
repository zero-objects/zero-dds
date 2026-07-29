<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Ada endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { Id, Value, Label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/ada
cargo run -p zerodds-endpoint-golden -- build   # (from the workspace root, once)
make examples
```

`make examples` builds the gpr project and runs `build/example_sync` and
`build/example_async`. Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Mailbox.Try_Receive` in a loop (non-blocking). The idiom
  when you own the run-loop.
- **`example_async`** — a `Reader_Task` pulls frames from the transport and
  forwards decoded bodies into a protected `Mailbox`; the main task blocks on
  `Inbox.Receive`. The idiomatic Ada task + protected-object concurrency model.

## Transport

The in-memory transport is a protected `Mailbox` (a FIFO of framed samples). A
real datagram link is a drop-in — `test/test_udp_loopback.adb` drives the same
codec over a live `GNAT.Sockets` UDP socket.

## Wire & codegen

Stage 1 binds the audited C89 wire-core (`endpoints/c`) through `Interfaces.C`, so
the bytes are byte-identical to the Rust core (XCDR2, align cap 4). The same types
can be generated from IDL with `zerodds-idlc --ada` (pure-Ada package `Zdgen`) —
see [`docs/specs/zerodds-xcdr2-ada-1.0.md`](../../docs/specs/zerodds-xcdr2-ada-1.0.md).
