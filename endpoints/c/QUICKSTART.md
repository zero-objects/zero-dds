<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS C endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/c
make examples
```

`make examples` builds and runs `build/example_sync` and `build/example_async`.
Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** (C89) — a poll loop over the transport (`zdw_endpoint_recv`
  + `zdw_xrce_read_frame`); the idiom when you own the run-loop.
- **`example_async`** (C11) — the event-driven reactor: `zdw_async_reader` binds
  an `on_sample` callback and `zdw_async_run` drains the transport, dispatching
  each decoded sample. No threads assumed, no malloc.

## Transport

A `zdw_transport` is `ctx` + `deliver` / `receive` function pointers. The examples
use an in-memory FIFO; a real UDP or serial link is a drop-in
(`examples/udp_endpoint.c` drives the same codec over a socket).

## Wire & codegen

The wire-core (`src/zerodds_wire.c`, `-std=c89 -pedantic`) is byte-identical to
the Rust core (XCDR2, align cap 4, `zdw_put_f32`). IDL types are generated with
`zerodds-idlc --c` (via `crates/idl-cpp` C-mode) — see
[`docs/specs/zerodds-xcdr2-c-1.0.md`](../../docs/specs/zerodds-xcdr2-c-1.0.md).
