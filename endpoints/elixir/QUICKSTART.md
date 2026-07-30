<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Elixir endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/elixir
elixir -r lib/zerodds.ex example_sync.exs     # subscriber polls
elixir -r lib/zerodds.ex example_async.exs    # subscriber receives in its mailbox
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll/1` in a loop (non-blocking; `nil` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.start/2` spawns a BEAM process that sends
  `{:zerodds_sample, body}` to your mailbox; you `receive` them. The idiomatic
  OTP concurrency model (back-pressure, supervision, distribution all follow).

## Transport

`Transport` is `%{deliver: fun/1, receive: fun/0}`. The examples use the in-memory
`MemTransport` (an Agent-backed FIFO); a real UDP or shared-memory link is a
drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via
`<<v::float-little-32>>`). The same types can be generated from IDL with
`zerodds-idlc --elixir` — see
[`docs/specs/zerodds-xcdr2-elixir-1.0.md`](../../docs/specs/zerodds-xcdr2-elixir-1.0.md).
