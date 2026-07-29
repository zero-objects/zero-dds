/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * zerodds_async.c -- C11 async reactor add-on (see zerodds_async.h). A thin,
 * allocation-free, thread-free dispatch layer over the transport's
 * non-blocking receive and the XRCE WRITE_DATA framing. */

#include "zerodds_async.h"

void zdw_async_reader_init(zdw_async_reader *r, const zdw_transport *t,
                           unsigned char *rxbuf, size_t rxcap,
                           zdw_on_sample on_sample, void *ctx)
{
    /* Plain field assignment (no compound literal) so this one .c compiles
     * clean under both gcc -std=c11 and g++ -std=c++17 -- the C++ SDK links it
     * directly, like the rest of the wire-core. */
    r->transport = t;
    r->rxbuf     = rxbuf;
    r->rxcap     = rxcap;
    r->on_sample = on_sample;
    r->ctx       = ctx;
}

int zdw_async_poll(zdw_async_reader *r)
{
    size_t frame_len = 0;
    int rc = zdw_endpoint_recv(r->transport, r->rxbuf, r->rxcap, &frame_len);
    if (rc != ZDW_T_OK) {
        return rc; /* ZDW_T_AGAIN or ZDW_T_ERROR */
    }

    const unsigned char *body = NULL;
    size_t body_len = 0;
    if (zdw_xrce_read_frame(r->rxbuf, frame_len, &body, &body_len) != ZDW_T_OK) {
        return ZDW_T_ERROR;
    }
    if (r->on_sample != NULL) {
        r->on_sample(r->ctx, body, body_len);
    }
    return ZDW_T_OK;
}

int zdw_async_run(zdw_async_reader *r, int max)
{
    int count = 0;
    for (;;) {
        if (max > 0 && count >= max) {
            break;
        }
        int rc = zdw_async_poll(r);
        if (rc == ZDW_T_OK) {
            count++;
        } else {
            break; /* AGAIN (drained) or ERROR */
        }
    }
    return count;
}

void zdw_async_writer_init(zdw_async_writer *w, const zdw_transport *t,
                           unsigned char *txbuf, size_t txcap,
                           unsigned char session, unsigned char stream)
{
    w->transport = t;
    w->txbuf     = txbuf;
    w->txcap     = txcap;
    w->session   = session;
    w->stream    = stream;
    w->seq       = 1;
}

int zdw_async_write(zdw_async_writer *w, const unsigned char *sample, size_t len)
{
    size_t frame_len = zdw_xrce_write_frame(w->txbuf, w->txcap, w->session,
                                            w->stream, w->seq, sample, len);
    if (frame_len == 0) {
        return ZDW_T_ERROR;
    }
    w->seq++;
    return zdw_endpoint_send(w->transport, w->txbuf, frame_len);
}
