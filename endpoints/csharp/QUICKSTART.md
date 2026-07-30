<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS C# endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading(Id, Value, Label)` samples and delivers them; a subscriber decodes
**every field**.

```sh
cd endpoints/csharp
dotnet run --project ExampleSync.csproj     # subscriber owns the run-loop and polls
dotnet run --project ExampleAsync.csproj    # subscriber iterates an IAsyncEnumerable
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`ExampleSync`** — `Client.Poll()` in a loop (non-blocking; `null` when empty).
  The idiom when you own the run-loop.
- **`ExampleAsync`** — `AsyncReader.Stream()` is an `IAsyncEnumerable<byte[]>`; the
  consumer iterates it with `await foreach`. The idiomatic C# async model.

## Byte-identity

```sh
cargo run -p zerodds-endpoint-golden -- build          # (from the workspace root)
dotnet run --project ByteIdentity.csproj -- build/golden_le.bin build/golden_be.bin
```

`ByteIdentity` proves the `@final` encoding is byte-for-byte identical to the Rust
golden (LE + BE).

## Transport

`ITransport` is `Deliver(frame)` + `Receive() -> frame?`. The examples use the
in-memory `MemTransport` (a locked FIFO); a real UDP or serial link is a drop-in.

## Wire & codegen

The pure-C# wire-core (`Zerodds.cs`) is byte-identical to the Rust core (XCDR2,
align cap 4, `f32` via `BitConverter`). IDL types are generated with
`zerodds-idlc --csharp` (a full IDL4 codegen: struct/enum/union → discriminated
record/typedef/array/map/@mutable) — see
[`docs/specs/zerodds-xcdr2-csharp-1.0.md`](../../docs/specs/zerodds-xcdr2-csharp-1.0.md).
