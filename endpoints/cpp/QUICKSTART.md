<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS C++ endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/cpp
make examples
```

`make examples` builds and runs `build/example_sync` and `build/example_async`.
Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** (C++98 facade) — a poll loop over the transport
  (`zdw_xrce_read_frame` + `zerodds::Reader`); the idiom when you own the
  run-loop.
- **`example_async`** (C++17 facade) — the event-driven reactor:
  `zerodds::AsyncReader` binds a `std::function` callback and `run()` drains the
  transport, dispatching each decoded sample. `zerodds::AsyncWriter` publishes.

## Transport

A `zdw_transport` is `ctx` + `deliver` / `receive` function pointers. The examples
use an in-memory FIFO; a real UDP or serial link is a drop-in.

## Wire & codegen

The C++ wire facade (`include/zerodds_wire.hpp`) is a thin `zerodds::Writer` /
`Reader` over the C89 core (`../c`), byte-identical to the Rust core (XCDR2, align
cap 4). IDL types are generated with `zerodds-idlc --cpp` (a full IDL4 codegen:
struct/enum/union → `std::variant`/typedef/array/nested/map/@mutable) — see
[`docs/specs/zerodds-xcdr2-cpp-1.0.md`](../../docs/specs/zerodds-xcdr2-cpp-1.0.md).
