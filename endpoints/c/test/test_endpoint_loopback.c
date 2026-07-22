/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Frame-Hook demo (ADR 0013 invariant 5): an endpoint encodes a sample with
 * the wire-fixed codec and hands the frame to a transport it knows nothing
 * about; the receiving side pulls the frame back and decodes it. Here the
 * transport is an in-memory loopback -- a real deployment fills the same
 * zdw_transport with the transport its target provides.
 */

#include <stdio.h>
#include <string.h>

#include "zerodds_endpoint.h"
#include "sample_sensor.h"

/* ---- an integrator-supplied transport (here: in-memory loopback) ---- */

typedef struct {
    unsigned char buf[512];
    size_t len;
    int full;
} loopback;

static int lb_deliver(void *ctx, const unsigned char *frame, size_t len)
{
    loopback *l = (loopback *)ctx;
    if (len > sizeof(l->buf)) {
        return ZDW_T_ERROR;
    }
    memcpy(l->buf, frame, len);
    l->len = len;
    l->full = 1;
    return ZDW_T_OK;
}

static int lb_receive(void *ctx, unsigned char *out, size_t cap, size_t *len)
{
    loopback *l = (loopback *)ctx;
    if (!l->full) {
        return ZDW_T_AGAIN;
    }
    if (l->len > cap) {
        return ZDW_T_ERROR;
    }
    memcpy(out, l->buf, l->len);
    *len = l->len;
    l->full = 0;
    return ZDW_T_OK;
}

int main(void)
{
    loopback lb;
    zdw_transport t;
    sensor_reading tx, rx;
    unsigned char frame[512], inbox[512];
    size_t flen = 0, rlen = 0;
    zdw_writer w;
    zdw_reader r;

    lb.full = 0;
    lb.len = 0;
    t.ctx = &lb;
    t.deliver = lb_deliver;
    t.receive = lb_receive;

    /* endpoint side: fill + encode + hand the frame to the transport */
    tx.id = 0xA1B2C3D4uL;
    tx.kind = 0x1234u;
    tx.flags = 0x5Au;
    tx.value = 3.5f;
    tx.stamp.hi = 0x01020304uL;
    tx.stamp.lo = 0x05060708uL;
    strcpy(tx.label, "bay-12");
    tx.raw[0] = 0xDE; tx.raw[1] = 0xAD; tx.raw[2] = 0xBE; tx.raw[3] = 0xEF;
    tx.raw_len = 4;

    zdw_writer_init(&w, frame, sizeof(frame), ZDW_LE);
    if (sensor_encode(&w, &tx) != ZDW_OK) {
        fprintf(stderr, "encode failed\n");
        return 1;
    }
    flen = w.len;
    if (zdw_endpoint_send(&t, frame, flen) != ZDW_T_OK) {
        fprintf(stderr, "transport deliver failed\n");
        return 1;
    }

    /* hub side: pull the frame back + decode */
    if (zdw_endpoint_recv(&t, inbox, sizeof(inbox), &rlen) != ZDW_T_OK) {
        fprintf(stderr, "transport receive failed\n");
        return 1;
    }
    memset(&rx, 0, sizeof(rx));
    zdw_reader_init(&r, inbox, rlen, ZDW_LE);
    if (sensor_decode(&r, &rx) != ZDW_OK) {
        fprintf(stderr, "decode failed\n");
        return 1;
    }

    if (rlen != flen || rx.id != tx.id || rx.kind != tx.kind ||
        rx.value != tx.value || rx.stamp.lo != tx.stamp.lo ||
        strcmp(rx.label, tx.label) != 0 || rx.raw_len != tx.raw_len) {
        fprintf(stderr, "round-trip mismatch\n");
        return 1;
    }
    printf("frame-hook: %lu-byte frame delivered + received + decoded ok\n",
           (unsigned long)flen);
    printf("ALL OK\n");
    return 0;
}
