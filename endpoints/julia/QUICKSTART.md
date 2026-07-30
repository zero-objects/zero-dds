<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Julia endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/julia
julia example_sync.jl     # subscriber owns the run-loop and polls
julia example_async.jl    # subscriber blocks on a Channel filled by a Task
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `ZeroDDS.poll` in a loop (non-blocking; `nothing` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `ZeroDDS.start_reader` spawns a `Task` that fills a
  `Channel`; the consumer blocks on `ZeroDDS.recv` (`take!`). The idiomatic Julia
  concurrency model.

## Transport

`Transport` is `(deliver::Function, receive::Function)`. The examples use the
in-memory `mem_transport()` (a FIFO); a real UDP or shared-memory link is a
drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via
`reinterpret(UInt32, Float32(v))`). The same types can be generated from IDL with
`zerodds-idlc --julia` — see
[`docs/specs/zerodds-xcdr2-julia-1.0.md`](../../docs/specs/zerodds-xcdr2-julia-1.0.md).
