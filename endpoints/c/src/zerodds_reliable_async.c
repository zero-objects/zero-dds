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

/* Bounded grace after stop(): drain the window while ACKNACKs still arrive, but
 * a silent peer must not wedge the join. */
#define ZDW_STOP_GRACE_MS 2000UL

static unsigned long zdw_now_ms(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (unsigned long)ts.tv_sec * 1000UL + (unsigned long)(ts.tv_nsec / 1000000L);
}

static void zdw_nap(long nsec)
{
    struct timespec ts;
    ts.tv_sec = 0;
    ts.tv_nsec = nsec;
    nanosleep(&ts, 0);
}

/* Retransmits every still-in-flight sample the ACKNACK marks missing (set bit). */
static void zdw_retransmit(zdw_async_ring *r, int first_unacked, unsigned int nack)
{
    unsigned char frame[8 + ZDW_RING_SLOTCAP];
    unsigned short i;
    for (i = 0; i < 16u; i++) {
        unsigned short seq;
        const unsigned char *p;
        size_t plen = 0;
        if (((nack >> i) & 1u) == 0u) {
            continue; /* acknowledged (or never sent) -- nothing to resend */
        }
        seq = (unsigned short)(first_unacked + (int)i);
        p = zdw_reliable_get_in_flight(&r->rel, seq, &plen);
        if (p != 0) {
            size_t flen = zdw_xrce_write_frame(frame, sizeof(frame), 0x80,
                                               r->stream_id, seq, p, plen);
            if (flen > 0 && r->deliver != 0) {
                r->deliver(r->ctx, frame, flen);
            }
        }
    }
}

/* Processes every pending ACKNACK: prune acknowledged samples + retransmit the
 * still-missing. Returns 1 if any ACKNACK was consumed. */
static int zdw_drain_acknacks(zdw_async_ring *r)
{
    unsigned char rx[64];
    int any = 0;
    int n;
    if (r->poll == 0) {
        return 0;
    }
    while ((n = r->poll(r->ctx, rx, sizeof(rx))) > 0) {
        int first_unacked;
        unsigned int nack;
        unsigned char st;
        if (zdw_xrce_acknack_read(rx, (size_t)n, &first_unacked, &nack, &st) != ZDW_T_OK) {
            break; /* not an ACKNACK -- ignore the rest of this drain */
        }
        zdw_reliable_recv_acknack(&r->rel, first_unacked, (unsigned short)nack);
        zdw_retransmit(r, first_unacked, nack);
        any = 1;
    }
    return any;
}

static void *zdw_ring_drain(void *arg)
{
    zdw_async_ring *r = (zdw_async_ring *)arg;
    unsigned char frame[8 + ZDW_RING_SLOTCAP];
    unsigned long stop_deadline = 0;
    int have_deadline = 0;

    for (;;) {
        int idle = 1;
        unsigned long tail = atomic_load_explicit(&r->tail, memory_order_relaxed);
        unsigned long head = atomic_load_explicit(&r->head, memory_order_acquire);

        /* 1) Pull ONE sample into the window. The tail advances only after a
         *    successful submit (or an unsendable drop) -- never on WINDOW_FULL,
         *    which would silently lose the sample. */
        if (tail != head && zdw_reliable_in_flight_count(&r->rel) < (size_t)ZDW_REL_WINDOW) {
            unsigned idx = (unsigned)(tail & (ZDW_RING_SLOTS - 1u));
            unsigned short len = r->slot[idx].len;
            unsigned short seq = 0;
            int rc = zdw_reliable_submit(&r->rel, r->slot[idx].buf, len, &seq);
            if (rc == ZDW_REL_OK) {
                size_t flen = zdw_xrce_write_frame(frame, sizeof(frame), 0x80,
                                                   r->stream_id, seq,
                                                   r->slot[idx].buf, len);
                if (flen > 0 && r->deliver != 0) {
                    r->deliver(r->ctx, frame, flen);
                }
                r->sent++;
                atomic_store_explicit(&r->tail, tail + 1, memory_order_release);
                idle = 0;
            } else if (rc == ZDW_REL_TOO_LARGE) {
                /* Unsendable: drop it so it cannot wedge the ring head. */
                atomic_store_explicit(&r->tail, tail + 1, memory_order_release);
                idle = 0;
            }
            /* ZDW_REL_WINDOW_FULL: leave the sample; ACKNACKs below free a slot. */
        }

        /* 2) Periodic HEARTBEAT announces the unacked window to the peer. */
        {
            int first = 0, last = 0;
            if (zdw_reliable_pending_heartbeat(&r->rel, zdw_now_ms(), &first, &last)) {
                /* golden control-frame layout: header stream NONE, msg-seq 1,
                 * payload stream = the reliable stream id. */
                size_t hlen = zdw_xrce_heartbeat_frame(frame, sizeof(frame), 0x80,
                                                       0x00, 1u, first, last,
                                                       r->stream_id);
                if (hlen > 0 && r->deliver != 0) {
                    r->deliver(r->ctx, frame, hlen);
                }
            }
        }

        /* 3) ACKNACK-driven confirmation + retransmit. NO self-ack. */
        if (zdw_drain_acknacks(r)) {
            idle = 0;
        }

        /* 4) Termination: stopped and (backlog + window drained), or the bounded
         *    grace elapsed so a silent peer never wedges the join. */
        if (!atomic_load_explicit(&r->running, memory_order_acquire)) {
            tail = atomic_load_explicit(&r->tail, memory_order_relaxed);
            head = atomic_load_explicit(&r->head, memory_order_acquire);
            if (tail == head && zdw_reliable_in_flight_count(&r->rel) == 0) {
                break;
            }
            if (!have_deadline) {
                stop_deadline = zdw_now_ms() + ZDW_STOP_GRACE_MS;
                have_deadline = 1;
            }
            if (zdw_now_ms() >= stop_deadline) {
                break;
            }
        }

        if (idle) {
            zdw_nap(200000L); /* 200 us */
        }
    }
    return 0;
}

int zdw_async_ring_start(zdw_async_ring *r, unsigned char stream_id,
                         zdw_deliver_fn deliver, zdw_poll_fn poll, void *ctx)
{
    atomic_store_explicit(&r->head, 0, memory_order_relaxed);
    atomic_store_explicit(&r->tail, 0, memory_order_relaxed);
    atomic_store_explicit(&r->running, 1, memory_order_relaxed);
    r->sent = 0;
    r->stream_id = stream_id;
    r->deliver = deliver;
    r->poll = poll;
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
