/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * zerodds_async.h -- modern (C11) async reactor add-on for the native C
 * endpoint SDK (ADR 0013). ADDITIVE: the conservative C89 wire-core + endpoint
 * stay untouched; this is the modern-toolchain option for teams that want an
 * event-driven, non-blocking receive instead of a manual poll.
 *
 * Model: the endpoint transport already exposes a NON-BLOCKING receive
 * (ZDW_T_AGAIN when no frame is ready). This layer turns that into a
 * callback-dispatch reactor -- no threads assumed, no malloc, so it runs on a
 * bare-metal super-loop as happily as under an OS event loop. The application
 * registers an `on_sample` callback and drives it with `zdw_async_poll` /
 * `zdw_async_run` from wherever its scheduler lives.
 *
 * Requires C11 (stdint/stdbool, designated initializers); the wire-core below
 * it remains ISO C89.
 */

#ifndef ZERODDS_ASYNC_H
#define ZERODDS_ASYNC_H

#include <stddef.h>
#include <stdbool.h>

#include "zerodds_endpoint.h"

#ifdef __cplusplus
extern "C" {
#endif

/* Invoked once per received sample. `body` points to the XCDR sample body
 * inside the receive buffer (valid only during the call); `ctx` is the
 * caller's cookie. */
typedef void (*zdw_on_sample)(void *ctx, const unsigned char *body, size_t body_len);

/* An event-driven reader over a caller-supplied transport + receive buffer. */
typedef struct {
    const zdw_transport *transport;
    unsigned char       *rxbuf;
    size_t               rxcap;
    zdw_on_sample        on_sample;
    void                *ctx;
} zdw_async_reader;

/* Bind a reader to a transport, a receive buffer, and a sample callback. */
void zdw_async_reader_init(zdw_async_reader *r, const zdw_transport *t,
                           unsigned char *rxbuf, size_t rxcap,
                           zdw_on_sample on_sample, void *ctx);

/* Poll once. If a WRITE_DATA frame is available, decode its XRCE envelope and
 * invoke the callback with the sample body. Returns ZDW_T_OK (dispatched),
 * ZDW_T_AGAIN (nothing ready), or ZDW_T_ERROR. Non-blocking. */
int zdw_async_poll(zdw_async_reader *r);

/* Drain the transport: dispatch frames until ZDW_T_AGAIN (or `max` frames, if
 * `max > 0`). Returns the number of samples dispatched. */
int zdw_async_run(zdw_async_reader *r, int max);

/* A fire-and-forget async writer: encodes an XCDR sample into an XRCE
 * WRITE_DATA frame and hands it to the transport, advancing the sequence. */
typedef struct {
    const zdw_transport *transport;
    unsigned char       *txbuf;
    size_t               txcap;
    unsigned char        session;
    unsigned char        stream;
    unsigned int         seq;
} zdw_async_writer;

/* Bind a writer to a transport + scratch buffer. `session >= 0x80` is a
 * best-effort no-key session (ZDW_XRCE_SESSION_NOKEY). */
void zdw_async_writer_init(zdw_async_writer *w, const zdw_transport *t,
                           unsigned char *txbuf, size_t txcap,
                           unsigned char session, unsigned char stream);

/* Frame `sample` (an XCDR body) as WRITE_DATA and deliver it. Returns
 * ZDW_T_OK or ZDW_T_ERROR. */
int zdw_async_write(zdw_async_writer *w, const unsigned char *sample, size_t len);

#ifdef __cplusplus
}
#endif

#endif /* ZERODDS_ASYNC_H */
