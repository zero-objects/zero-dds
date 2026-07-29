<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Go endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples as XRCE WRITE_DATA and delivers
them over a transport; a subscriber decodes **every field**.

```sh
cd endpoints/go
go run ./example_sync     # subscriber polls with a 2s deadline
go run ./example_async    # subscriber ranges the AsyncReader.Samples channel
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.Poll()` in a loop with a deadline (non-blocking
  receive; sleep-retry when empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader` runs a goroutine that pushes decoded frames
  onto a `Samples` channel; you `range` it and `Close()` when done. The idiom for
  event-driven consumers.

## Transport

The examples use a small in-memory `loopback` transport. It implements the
one-method-each `Transport` interface (`Deliver` / `Receive`), so swapping in a
real UDP or shared-memory link is a drop-in — see `async_test.go`'s
`udpTransport` for a live-UDP `net.UDPConn` implementation.

## Wire

The wire is byte-identical to the Rust core and every other ZeroDDS endpoint
(XCDR2, alignment cap 4, `f32` via `math.Float32bits`). The same types can be
generated from IDL with `zerodds-idlc --go` — see
[`docs/specs/zerodds-xcdr2-go-1.0.md`](../../docs/specs/zerodds-xcdr2-go-1.0.md).
