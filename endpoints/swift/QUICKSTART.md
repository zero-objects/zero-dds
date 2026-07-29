<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Swift endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/swift
swift run ZeroddsExampleSync     # subscriber owns the run-loop and polls
swift run ZeroddsExampleAsync    # subscriber iterates an AsyncStream
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`ZeroddsExampleSync`** — `Client.poll()` in a loop (non-blocking; `nil` when
  empty). The idiom when you own the run-loop.
- **`ZeroddsExampleAsync`** — `AsyncReader.stream()` is an `AsyncStream`; the
  consumer iterates it with `for await`. The idiomatic Swift structured-concurrency
  model.

## Transport

`Transport` is a protocol (`deliver(_:)`, `receive() -> [UInt8]?`). The examples use
the in-memory `MemTransport` (an `NSLock`-guarded FIFO); a real UDP or shared-memory
link is a drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via `.bitPattern` /
`Float(bitPattern:)`). The same types can be generated from IDL with
`zerodds-idlc --swift` — see
[`docs/specs/zerodds-xcdr2-swift-1.0.md`](../../docs/specs/zerodds-xcdr2-swift-1.0.md).
