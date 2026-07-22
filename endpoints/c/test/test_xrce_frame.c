/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Proves the C XRCE WRITE_DATA framing is byte-identical to a real
 * `crates/xrce` Message (golden_xrce_le.bin, ADR 0013 P4), then runs it
 * through the frame-hook: encode sample -> XRCE frame -> transport -> receive
 * -> unwrap WRITE_DATA -> decode. A crates/xrce agent accepts this frame.
 *
 * usage: test_xrce_frame <golden_xrce_le.bin>
 */

#include <stdio.h>
#include <string.h>

#include "zerodds_endpoint.h"
#include "sample_sensor.h"

typedef struct { unsigned char buf[512]; size_t len; int full; } loopback;

static int lb_deliver(void *c, const unsigned char *f, size_t n)
{
    loopback *l = (loopback *)c;
    if (n > sizeof(l->buf)) {
        return ZDW_T_ERROR;
    }
    memcpy(l->buf, f, n);
    l->len = n;
    l->full = 1;
    return ZDW_T_OK;
}

static int lb_receive(void *c, unsigned char *o, size_t cap, size_t *n)
{
    loopback *l = (loopback *)c;
    if (!l->full) {
        return ZDW_T_AGAIN;
    }
    if (l->len > cap) {
        return ZDW_T_ERROR;
    }
    memcpy(o, l->buf, l->len);
    *n = l->len;
    l->full = 0;
    return ZDW_T_OK;
}

static void fill(sensor_reading *s)
{
    s->id = 0xA1B2C3D4uL; s->kind = 0x1234u; s->flags = 0x5Au; s->value = 3.5f;
    s->stamp.hi = 0x01020304uL; s->stamp.lo = 0x05060708uL;
    strcpy(s->label, "bay-12");
    s->raw[0] = 0xDE; s->raw[1] = 0xAD; s->raw[2] = 0xBE; s->raw[3] = 0xEF; s->raw_len = 4;
}

int main(int argc, char **argv)
{
    sensor_reading tx, rx;
    unsigned char body[256], frame[512], inbox[512], golden[512];
    size_t blen, flen, rlen = 0, blen2 = 0, gn;
    const unsigned char *rbody = 0;
    zdw_writer w;
    zdw_reader r;
    loopback lb;
    zdw_transport t;
    FILE *gf;

    if (argc < 2) { fprintf(stderr, "usage: %s <golden_xrce_le.bin>\n", argv[0]); return 2; }

    /* endpoint: encode the sample body, then wrap it in an XRCE WRITE_DATA */
    fill(&tx);
    zdw_writer_init(&w, body, sizeof(body), ZDW_LE);
    if (sensor_encode(&w, &tx) != ZDW_OK) { fprintf(stderr, "encode\n"); return 1; }
    blen = w.len;
    flen = zdw_xrce_write_frame(frame, sizeof(frame),
                                ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT,
                                1, body, blen);
    if (flen == 0) { fprintf(stderr, "frame\n"); return 1; }

    /* byte-identity vs a real crates/xrce Message */
    gf = fopen(argv[1], "rb");
    if (gf == NULL) { fprintf(stderr, "open golden\n"); return 1; }
    gn = fread(golden, 1, sizeof(golden), gf);
    fclose(gf);
    if (flen != gn || memcmp(frame, golden, gn) != 0) {
        fprintf(stderr, "XRCE frame mismatch: C=%lu golden=%lu\n",
                (unsigned long)flen, (unsigned long)gn);
        return 1;
    }
    printf("XRCE WRITE_DATA frame %lu bytes byte-identical to crates/xrce\n",
           (unsigned long)flen);

    /* through the frame-hook, then unwrap + decode */
    lb.full = 0; t.ctx = &lb; t.deliver = lb_deliver; t.receive = lb_receive;
    if (zdw_endpoint_send(&t, frame, flen) != ZDW_T_OK) { fprintf(stderr, "send\n"); return 1; }
    if (zdw_endpoint_recv(&t, inbox, sizeof(inbox), &rlen) != ZDW_T_OK) { fprintf(stderr, "recv\n"); return 1; }
    if (zdw_xrce_read_frame(inbox, rlen, &rbody, &blen2) != ZDW_T_OK) { fprintf(stderr, "unwrap\n"); return 1; }
    memset(&rx, 0, sizeof(rx));
    zdw_reader_init(&r, rbody, blen2, ZDW_LE);
    if (sensor_decode(&r, &rx) != ZDW_OK) { fprintf(stderr, "decode\n"); return 1; }
    if (rx.id != tx.id || strcmp(rx.label, tx.label) != 0 || rx.raw_len != tx.raw_len) {
        fprintf(stderr, "round-trip mismatch\n"); return 1;
    }
    printf("frame-hook round-trip: unwrapped + decoded ok\n");
    printf("ALL OK\n");
    return 0;
}
