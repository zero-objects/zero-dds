/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Reliable-stream deep example (ADR 0013): an aggregator submits N samples on a
 * reliable stream; the link loses some; the receiver's ACKNACK drives the
 * sender's retransmit until every sample is delivered gap-free, in order. Pure
 * C89, no network -- the state machine is exercised end-to-end in one process.
 */
#include <stdio.h>
#include "zerodds_endpoint.h"

/* N <= ZDW_REL_WINDOW so all samples are in flight at once (this in-process
 * demo submits the whole batch up front; the live E2E drives the windowed
 * flow over UDP). */
#define N 12

static zdw_reliable snd;
static zdw_reliable rcv;

int main(void)
{
    unsigned char sample[4], out[8];
    unsigned short seq = 0;
    size_t len = 0;
    int i, round, delivered = 0, lost = 0;

    zdw_reliable_init(&snd, 0x80);
    zdw_reliable_init(&rcv, 0x80);

    /* Submit N and "transmit"; drop every 4th sample on this first pass. */
    for (i = 0; i < N; i++) {
        sample[0] = (unsigned char)i;
        sample[1] = (unsigned char)(i >> 8);
        sample[2] = 0; sample[3] = 0;
        zdw_reliable_submit(&snd, sample, 4, 0);
        if ((i % 4) == 3) {
            lost++; /* dropped in flight */
        } else {
            zdw_reliable_recv_data(&rcv, (unsigned short)i, sample, 4);
        }
    }
    while (zdw_reliable_drain(&rcv, out, sizeof(out), &seq, &len)) { delivered++; }
    printf("reliable: %d/%d delivered before recovery (%d lost)\n", delivered, N, lost);

    /* Recovery rounds: receiver ACKNACKs, sender retransmits the set bits. */
    for (round = 0; round < N && delivered < N; round++) {
        unsigned short base = zdw_reliable_expected(&rcv);
        unsigned short bm = zdw_reliable_pending_acknack(&rcv);
        unsigned short b;
        zdw_reliable_recv_acknack(&snd, (int)base, bm);
        for (b = 0; b < 16; b++) {
            if ((bm >> b) & 1u) {
                unsigned short s = (unsigned short)(base + b);
                size_t l = 0;
                const unsigned char *pl = zdw_reliable_get_in_flight(&snd, s, &l);
                if (pl != 0) {
                    zdw_reliable_recv_data(&rcv, s, pl, l);
                }
            }
        }
        while (zdw_reliable_drain(&rcv, out, sizeof(out), &seq, &len)) { delivered++; }
    }

    printf("reliable: delivered contiguous 0..%d, expected=%u\n",
           delivered - 1, (unsigned)zdw_reliable_expected(&rcv));
    if (delivered == N && zdw_reliable_expected(&rcv) == N) {
        printf("reliable: ALL %d samples recovered gap-free\n", N);
        return 0;
    }
    fprintf(stderr, "reliable: incomplete (%d/%d)\n", delivered, N);
    return 1;
}
