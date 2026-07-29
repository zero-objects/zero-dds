/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * zerodds_reliable_async.c -- SPSC ring + drain thread (see the header). The
 * producer never enters the kernel; the drain thread owns the reliable sender
 * state and does the framing + I/O syscall. */

#define _POSIX_C_SOURCE 200809L

#include "zerodds_reliable_async.h"
#include <string.h>
#include <time.h>

static void *zdw_ring_drain(void *arg)
{
    zdw_async_ring *r = (zdw_async_ring *)arg;
    unsigned char frame[8 + ZDW_RING_SLOTCAP];

    for (;;) {
        unsigned long tail = atomic_load_explicit(&r->tail, memory_order_relaxed);
        unsigned long head = atomic_load_explicit(&r->head, memory_order_acquire);
        if (tail == head) {
            struct timespec ts;
            if (!atomic_load_explicit(&r->running, memory_order_acquire)) {
                break; /* stopped and backlog drained */
            }
            ts.tv_sec = 0; ts.tv_nsec = 1000; /* 1 us idle nap */
            nanosleep(&ts, 0);
            continue;
        }
        {
            unsigned idx = (unsigned)(tail & (ZDW_RING_SLOTS - 1u));
            unsigned short len = r->slot[idx].len;
            unsigned short seq = 0;
            size_t flen;
            /* Hold the reliable sender state on the drain side. In this writer
             * the drain acks each sample as it is framed (the bench measures the
             * producer path; the E2E app drives the full ACKNACK/retransmit
             * loop), so the 16-sample window never blocks the producer. */
            if (zdw_reliable_submit(&r->rel, r->slot[idx].buf, len, &seq) == ZDW_REL_OK) {
                flen = zdw_xrce_write_frame(frame, sizeof(frame), 0x80, 0x80,
                                            seq, r->slot[idx].buf, len);
                if (flen > 0 && r->deliver != 0) {
                    r->deliver(r->ctx, frame, flen);
                }
                zdw_reliable_recv_acknack(&r->rel, (int)(unsigned short)(seq + 1), 0);
                r->sent++;
            }
            atomic_store_explicit(&r->tail, tail + 1, memory_order_release);
        }
    }
    return 0;
}

int zdw_async_ring_start(zdw_async_ring *r, unsigned char stream_id,
                         zdw_deliver_fn deliver, void *ctx)
{
    atomic_store_explicit(&r->head, 0, memory_order_relaxed);
    atomic_store_explicit(&r->tail, 0, memory_order_relaxed);
    atomic_store_explicit(&r->running, 1, memory_order_relaxed);
    r->sent = 0;
    r->deliver = deliver;
    r->ctx = ctx;
    zdw_reliable_init(&r->rel, stream_id);
    if (pthread_create(&r->thread, 0, zdw_ring_drain, r) != 0) {
        atomic_store_explicit(&r->running, 0, memory_order_relaxed);
        return -1;
    }
    return 0;
}

int zdw_async_ring_enqueue(zdw_async_ring *r, const unsigned char *payload,
                           size_t len)
{
    unsigned long head, tail;
    unsigned idx;
    if (len > ZDW_RING_SLOTCAP) {
        return -1;
    }
    head = atomic_load_explicit(&r->head, memory_order_relaxed);
    tail = atomic_load_explicit(&r->tail, memory_order_acquire);
    if (head - tail >= ZDW_RING_SLOTS) {
        return -1; /* ring full -- backpressure */
    }
    idx = (unsigned)(head & (ZDW_RING_SLOTS - 1u));
    if (len > 0) {
        memcpy(r->slot[idx].buf, payload, len);
    }
    r->slot[idx].len = (unsigned short)len;
    atomic_store_explicit(&r->head, head + 1, memory_order_release);
    return 0;
}

void zdw_async_ring_stop(zdw_async_ring *r)
{
    atomic_store_explicit(&r->running, 0, memory_order_release);
    pthread_join(r->thread, 0);
}

unsigned long zdw_async_ring_sent(const zdw_async_ring *r)
{
    return r->sent;
}
