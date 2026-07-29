<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS F# endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/fsharp
dotnet fsi example_sync.fsx     # subscriber polls
dotnet fsi example_async.fsx    # subscriber awaits the MailboxProcessor agent
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.Poll()` in a loop (non-blocking; `None` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.RecvAsync()` returns `Async<byte[]>` served
  by a `MailboxProcessor` agent; `let! body = ...` inside an `async { }` workflow.
  The idiomatic F# async model.

## Transport

`Transport` is a record of two functions (`Deliver` / `Receive`). The examples
use the in-memory `memTransport ()`; a real UDP or shared-memory link is a drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via `BitConverter`).
F# is .NET-native, so IDL types are generated with `zerodds-idlc --csharp` and
consumed directly — see
[`docs/specs/zerodds-xcdr2-fsharp-1.0.md`](../../docs/specs/zerodds-xcdr2-fsharp-1.0.md).
