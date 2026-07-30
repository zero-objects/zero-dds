/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Producer-latency micro-bench: inline WRITE_DATA send (a syscall on the
 * producer) vs. the async-decoupled ring enqueue (wait-free, no kernel). Prints
 * ns per op for both -- the gap is the latency a tight aggregator loop avoids.
 * Sends to a bound-but-unread loopback socket so the syscall cost is real.
 */
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include "zerodds_endpoint.h"
#include "zerodds_reliable_async.h"

#define ITERS 100000
#define BURST 1000    /* < ZDW_RING_SLOTS: enqueue never blocks */
#define DELIVER 5000  /* delivery-correctness pass */

static zdw_async_ring ring;

/* In-process ACKNACK responder for the delivery-correctness pass: a lossless
 * loopback peer. `deliver` sends the frame AND tracks in-order reception;
 * `poll_ack` returns a cumulative ACKNACK whenever new samples arrived. Both run
 * on the drain thread, so `peer_expected`/`last_acked` need no synchronisation. */
typedef struct bench_ctx {
    int fd;
    unsigned short peer_expected; /* next in-order seq the peer wants */
    unsigned short last_acked;    /* highest base already ACKNACKed */
} bench_ctx;

static int deliver_udp(void *ctx, const unsigned char *frame, size_t len)
{
    bench_ctx *b = (bench_ctx *)ctx;
    if (len >= 8u && frame[4] == 0x07) { /* WRITE_DATA: advance in-order cursor */
        unsigned short seq = (unsigned short)(frame[2] | (frame[3] << 8));
        if (seq == b->peer_expected) {
            b->peer_expected = (unsigned short)(b->peer_expected + 1);
        }
    }
    return (int)send(b->fd, frame, len, 0);
}

static int poll_ack(void *ctx, unsigned char *buf, size_t cap)
{
    bench_ctx *b = (bench_ctx *)ctx;
    if (b->peer_expected == b->last_acked) {
        return 0; /* nothing new to acknowledge */
    }
    b->last_acked = b->peer_expected;
    return (int)zdw_xrce_acknack_frame(buf, cap, 0x80, 0x80, 1u,
                                       (int)b->peer_expected, 0, 0, 0x80);
}

static unsigned long long now_ns(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (unsigned long long)ts.tv_sec * 1000000000ull + (unsigned long long)ts.tv_nsec;
}

int main(void)
{
    int r_fd, s_fd, i;
    struct sockaddr_in a;
    socklen_t alen = sizeof(a);
    unsigned char sample[4], frame[64];
    unsigned long long t0, inline_ns, enq_ns;

    /* discard socket: bind r_fd, connect s_fd to it, never read. */
    r_fd = socket(AF_INET, SOCK_DGRAM, 0);
    s_fd = socket(AF_INET, SOCK_DGRAM, 0);
    memset(&a, 0, sizeof(a));
    a.sin_family = AF_INET;
    a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    a.sin_port = 0;
    if (bind(r_fd, (struct sockaddr *)&a, sizeof(a)) < 0) { perror("bind"); return 1; }
    getsockname(r_fd, (struct sockaddr *)&a, &alen);
    if (connect(s_fd, (struct sockaddr *)&a, sizeof(a)) < 0) { perror("connect"); return 1; }

    sample[0] = 1; sample[1] = 2; sample[2] = 3; sample[3] = 4;

    /* inline: frame + send() syscall on the producer, every sample. */
    t0 = now_ns();
    for (i = 0; i < ITERS; i++) {
        size_t flen = zdw_xrce_write_frame(frame, sizeof(frame), 0x80, 0x80,
                                           (unsigned)(i & 0xffff), sample, 4);
        (void)send(s_fd, frame, flen, 0);
    }
    inline_ns = (now_ns() - t0) / (unsigned long long)ITERS;

    /* decoupled producer latency: a BURST that fits the ring, with no drain
     * consuming, times the pure producer path (memcpy + atomics, no kernel) --
     * the per-sample cost a tight aggregator actually pays. */
    memset(&ring, 0, sizeof(ring));
    t0 = now_ns();
    for (i = 0; i < BURST; i++) {
        (void)zdw_async_ring_enqueue(&ring, sample, 4);
    }
    enq_ns = (now_ns() - t0) / (unsigned long long)BURST;

    /* correctness: the drain thread must deliver every enqueued sample -- now
     * ACKNACK-driven (the loopback peer acks), not self-acked. */
    memset(&ring, 0, sizeof(ring));
    {
        static bench_ctx bctx;
        bctx.fd = s_fd;
        bctx.peer_expected = 0;
        bctx.last_acked = 0;
        if (zdw_async_ring_start(&ring, 0x80, deliver_udp, poll_ack, &bctx) != 0) {
            fprintf(stderr, "ring start failed\n"); return 1;
        }
    }
    for (i = 0; i < DELIVER; i++) {
        while (zdw_async_ring_enqueue(&ring, sample, 4) != 0) {
            /* ring full -> let the drain catch up (backpressure) */
        }
    }
    zdw_async_ring_stop(&ring);

    printf("inline_ns_per_op=%llu decoupled_enqueue_ns_per_op=%llu delivered=%lu\n",
           inline_ns, enq_ns, zdw_async_ring_sent(&ring));
    close(s_fd);
    close(r_fd);
    return (enq_ns < inline_ns && zdw_async_ring_sent(&ring) == (unsigned long)DELIVER)
               ? 0 : 2;
}
