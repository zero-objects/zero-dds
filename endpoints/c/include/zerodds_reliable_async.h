/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * zerodds_reliable_async.h -- async-decoupled reliable writer (ADR 0013).
 *
 * A single-producer/single-consumer ring plus a drain thread that holds the
 * reliable sender state: the producer enqueues a sample wait-free (no syscall,
 * no lock -- ns) and returns to its hot loop; the drain thread assigns the
 * sequence number, frames the WRITE_DATA and performs the I/O syscall. This is
 * the latency decoupling for a tight ns-cadence aggregator (the syscall never
 * runs on the producer core).
 *
 * This is a GENUINE reliable writer: samples stay in-flight until an inbound
 * ACKNACK acknowledges them (there is no self-ack -- that would be false
 * reliability). The drain thread polls ACKNACKs through `poll`, prunes the
 * acknowledged samples, retransmits the ones the peer still marks missing, and
 * emits periodic HEARTBEATs. When the 16-sample window is full it stops pulling
 * from the ring (the producer back-pressures via a full-ring enqueue), so no
 * sample is ever dropped for lack of an ack.
 *
 * C11 + pthread (kept out of the strict-C89 wire core and the header-only C++
 * SDK). The struct is large (~300 KiB) -- allocate it static or on the heap. */

#ifndef ZERODDS_RELIABLE_ASYNC_H
#define ZERODDS_RELIABLE_ASYNC_H

#include <stddef.h>
#include <stdatomic.h>
#include <pthread.h>
#include "zerodds_endpoint.h"

#define ZDW_RING_SLOTS    1024  /* power of two */
#define ZDW_RING_SLOTCAP  256

/* Delivers one framed message to the peer (the I/O syscall). */
typedef int (*zdw_deliver_fn)(void *ctx, const unsigned char *frame, size_t len);
/* Polls one inbound frame (typically an ACKNACK) into `buf` (capacity `cap`);
 * returns the byte count, or <= 0 when nothing is pending. May be NULL, but then
 * the window can never be acknowledged and blocks at ZDW_REL_WINDOW -- only pass
 * NULL where a real ACKNACK responder is genuinely absent (and expect the
 * producer to back-pressure). */
typedef int (*zdw_poll_fn)(void *ctx, unsigned char *buf, size_t cap);

typedef struct zdw_ring_slot {
    unsigned short len;
    unsigned char buf[ZDW_RING_SLOTCAP];
} zdw_ring_slot;

typedef struct zdw_async_ring {
    _Atomic unsigned long head; /* producer publishes */
    _Atomic unsigned long tail; /* consumer publishes */
    _Atomic int running;
    unsigned long sent;         /* distinct new samples framed + sent */
    unsigned char stream_id;
    zdw_deliver_fn deliver;
    zdw_poll_fn poll;
    void *ctx;
    zdw_reliable rel;
    pthread_t thread;
    zdw_ring_slot slot[ZDW_RING_SLOTS];
} zdw_async_ring;

/* Starts the drain thread. `poll` supplies inbound ACKNACKs (may be NULL).
 * Returns 0 on success, -1 on pthread failure. */
int zdw_async_ring_start(zdw_async_ring *r, unsigned char stream_id,
                         zdw_deliver_fn deliver, zdw_poll_fn poll, void *ctx);
/* Wait-free enqueue (no syscall). 0 on success, -1 if the ring is full or the
 * payload exceeds ZDW_RING_SLOTCAP. */
int zdw_async_ring_enqueue(zdw_async_ring *r, const unsigned char *payload,
                           size_t len);
/* Signals the drain thread to finish the backlog and join. It drains the ring
 * and the unacked window while ACKNACKs keep arriving, but never past a bounded
 * grace deadline -- a silent peer cannot wedge the join. */
void zdw_async_ring_stop(zdw_async_ring *r);
/* Samples delivered by the drain thread so far. */
unsigned long zdw_async_ring_sent(const zdw_async_ring *r);

#endif /* ZERODDS_RELIABLE_ASYNC_H */
