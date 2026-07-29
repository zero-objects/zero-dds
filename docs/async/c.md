<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS async — C (native endpoint)

The native C endpoint SDK (ADR 0013) gains a **modern (C11) async reactor** as an
**additive** add-on: the conservative C89 wire-core and endpoint stay untouched,
and teams on a modern toolchain get event-driven, non-blocking receive instead
of a manual poll. No threads, no malloc — it runs on a bare-metal super-loop as
happily as under an OS event loop.

Header: [`endpoints/c/include/zerodds_async.h`](../../endpoints/c/include/zerodds_async.h)

## Model

The endpoint transport already exposes a non-blocking receive (`ZDW_T_AGAIN`
when nothing is ready). The reactor turns that into callback dispatch:

```c
#include "zerodds_async.h"

void on_sample(void *ctx, const unsigned char *body, size_t body_len) {
    /* decode `body` with the generated wire-fixed codec */
}

zdw_async_reader r;
zdw_async_reader_init(&r, &transport, rxbuf, sizeof rxbuf, on_sample, &ctx);

/* drive it from wherever your scheduler lives — no thread assumed */
for (;;) {
    zdw_async_run(&r, 0);   /* dispatch all ready frames, returns count */
    /* … do other work / wait on your event loop … */
}
```

Sending is fire-and-forget:

```c
zdw_async_writer w;
zdw_async_writer_init(&w, &transport, txbuf, sizeof txbuf,
                      ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT);
zdw_async_write(&w, sample_xcdr, sample_len);   /* frames as XRCE WRITE_DATA */
```

## API

| Function | Purpose |
|----------|---------|
| `zdw_async_reader_init` | bind a reader to a transport + receive buffer + `on_sample` callback |
| `zdw_async_poll` | dispatch one ready frame (`ZDW_T_OK` / `ZDW_T_AGAIN` / `ZDW_T_ERROR`) |
| `zdw_async_run` | drain the transport (dispatch until `ZDW_T_AGAIN`, or `max` frames) |
| `zdw_async_writer_init` / `zdw_async_write` | frame + deliver an XCDR sample, advancing the sequence |

## Tests

`make -C endpoints/c test` (in CI via the `endpoints-native` job):

- `test_async_loopback` — an async writer fires N samples into a FIFO transport;
  the reactor drains + decodes them in order.
- `test_async_udp` — the same over a real non-blocking UDP socket (live E2E):
  `recvfrom` → `EAGAIN` → `ZDW_T_AGAIN` drives the reactor.

Requires C11 (`-std=c11`); the wire-core below it stays ISO C89.
