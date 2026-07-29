<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Rust endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cargo run -p zerodds-endpoint-rust --example example_sync    # subscriber polls
cargo run -p zerodds-endpoint-rust --example example_async   # subscriber blocks on a channel
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client::poll` in a loop (non-blocking; `None` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader::start` spawns a std thread that forwards
  decoded bodies over an `mpsc` channel; the consumer blocks on `recv`. The
  idiomatic std concurrency model — no async runtime dependency.

## Transport

`MemTransport` is an in-memory FIFO (a shared `VecDeque`); a real UDP or serial
link is a drop-in. XRCE WRITE_DATA framing is `xrce_write_frame` /
`xrce_read_frame`.

## Wire & codegen

The codec is the reference `zerodds-cdr` core itself (`BufferWriter` /
`BufferReader`, XCDR2, align cap 4) — so the bytes are byte-identical by
construction. IDL types are generated with `zerodds-idlc --rust` (a full IDL4
DataType codegen: `#[derive(DdsType)]`, enum/union/typedef) — see
[`docs/specs/zerodds-xcdr2-rust-1.0.md`](../../docs/specs/zerodds-xcdr2-rust-1.0.md).
