/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Async reliable writer -- window-overflow + no-self-ack test.
 *
 * Enqueues MORE than the 16-sample window before any ACKNACK is allowed through.
 *   Phase 1 (gate closed): a genuine reliable writer cannot confirm anything
 *     without an ACKNACK, so exactly ZDW_REL_WINDOW (16) samples are delivered
 *     and the rest wait in the ring. The pre-fix writer self-acked every sample,
 *     so it would deliver ALL of them here -- false reliability.
 *   Phase 2 (gate open): ACKNACKs flow, the window drains, and the remaining
 *     samples are delivered. Every sample must arrive (no loss) and stop() must
 *     return within its bounded grace (no hang).
 */
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <stdatomic.h>
#include "zerodds_endpoint.h"
#include "zerodds_reliable_async.h"

#define N 24 /* > ZDW_REL_WINDOW (16) */

typedef struct peer {
    _Atomic int gate;             /* 0: withhold ACKNACKs; 1: release */
    unsigned char delivered[256]; /* delivered[seq] = payload + 1 (0 = none) */
    _Atomic unsigned distinct;    /* distinct delivered seqs */
    unsigned short peer_expected; /* drain-thread only: next in-order seq */
    unsigned short last_acked;    /* drain-thread only: highest ACKNACK base */
} peer;

static peer g_peer;
static zdw_async_ring ring;

/* Lossless loopback receiver: records the first delivery of each seq and tracks
 * the in-order cursor. Retransmits of already-delivered seqs are de-duplicated. */
static int deliver_cb(void *ctx, const unsigned char *frame, size_t len)
{
    peer *p = (peer *)ctx;
    if (len >= 9u && frame[4] == 0x07) { /* WRITE_DATA, 1-byte payload */
        unsigned short seq = (unsigned short)(frame[2] | (frame[3] << 8));
        if (p->delivered[seq & 0xff] == 0) {
            p->delivered[seq & 0xff] = (unsigned char)(frame[8] + 1);
            atomic_fetch_add_explicit(&p->distinct, 1u, memory_order_relaxed);
        }
        if (seq == p->peer_expected) {
            p->peer_expected = (unsigned short)(p->peer_expected + 1);
        }
    }
    return (int)len;
}

/* Cumulative ACKNACK, released only once the gate is open. */
static int poll_cb(void *ctx, unsigned char *buf, size_t cap)
{
    peer *p = (peer *)ctx;
    if (!atomic_load_explicit(&p->gate, memory_order_acquire)) {
        return 0;
    }
    if (p->peer_expected == p->last_acked) {
        return 0;
    }
    p->last_acked = p->peer_expected;
    return (int)zdw_xrce_acknack_frame(buf, cap, 0x80, 0x80, 1u,
                                       (int)p->peer_expected, 0, 0, 0x80);
}

static void nap_ms(long ms)
{
    struct timespec ts;
    ts.tv_sec = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000L;
    nanosleep(&ts, 0);
}

int main(void)
{
    int i, waited, fail = 0;

    memset(&ring, 0, sizeof(ring));
    memset(&g_peer, 0, sizeof(g_peer));
    atomic_store(&g_peer.gate, 0);

    if (zdw_async_ring_start(&ring, 0x80, deliver_cb, poll_cb, &g_peer) != 0) {
        fprintf(stderr, "ring start failed\n");
        return 1;
    }

    /* Enqueue N distinct 1-byte samples (wait-free; the ring dwarfs N). */
    for (i = 0; i < N; i++) {
        unsigned char s = (unsigned char)i;
        if (zdw_async_ring_enqueue(&ring, &s, 1u) != 0) {
            fprintf(stderr, "FAIL: enqueue %d rejected\n", i);
            fail = 1;
        }
    }

    /* Phase 1: gate closed -- the window must fill and STOP at 16. */
    for (waited = 0; waited < 3000; waited++) {
        if (atomic_load_explicit(&g_peer.distinct, memory_order_relaxed) >= (unsigned)ZDW_REL_WINDOW) {
            break;
        }
        nap_ms(1);
    }
    nap_ms(100); /* settle: nothing more may get through without an ACKNACK */

    {
        unsigned d = atomic_load_explicit(&g_peer.distinct, memory_order_relaxed);
        if (d != (unsigned)ZDW_REL_WINDOW) {
            fprintf(stderr,
                    "FAIL: delivered %u without ACKNACK; a real reliable writer "
                    "must confirm nothing beyond the %d-sample window (self-ack?)\n",
                    d, ZDW_REL_WINDOW);
            fail = 1;
        }
    }

    /* Phase 2: open the gate -- ACKNACKs flow, everything drains. */
    atomic_store_explicit(&g_peer.gate, 1, memory_order_release);
    for (waited = 0; waited < 5000; waited++) {
        if (atomic_load_explicit(&g_peer.distinct, memory_order_relaxed) >= (unsigned)N) {
            break;
        }
        nap_ms(1);
    }

    zdw_async_ring_stop(&ring); /* bounded: must return, not hang */

    {
        unsigned d = atomic_load_explicit(&g_peer.distinct, memory_order_relaxed);
        if (d != (unsigned)N) {
            fprintf(stderr, "FAIL: delivered %u of %d after ACKNACK (loss/hang)\n", d, N);
            fail = 1;
        }
    }
    for (i = 0; i < N; i++) {
        if (g_peer.delivered[i] != (unsigned char)(i + 1)) {
            fprintf(stderr, "FAIL: sample %d missing or corrupt (got %d)\n",
                    i, (int)g_peer.delivered[i] - 1);
            fail = 1;
        }
    }

    if (fail) {
        fprintf(stderr, "test_reliable_async_overflow FAILED\n");
        return 1;
    }
    printf("ALL OK (delivered %d/%d, no self-ack, no hang)\n", N, N);
    return 0;
}
