/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Deep example (sync): a realistic sensor-telemetry flow. A publisher frames
 * five typed Reading { id, value, label } samples and delivers them; the
 * subscriber owns the run-loop and polls, decoding EVERY field byte-for-byte.
 * Conservative C89, byte-identical to the Rust core via the zdw wire-core.
 */

#include <stdio.h>
#include <string.h>

#include "zerodds_endpoint.h"
#include "zerodds_wire.h"

enum { FIFO_SLOTS = 16, FIFO_FRAME = 256, TOTAL = 5 };

/* ---- an integrator-supplied transport: a small FIFO of frames ---- */

typedef struct {
    unsigned char buf[FIFO_SLOTS][FIFO_FRAME];
    size_t        len[FIFO_SLOTS];
    int           head, tail, count;
} fifo;

static int fifo_deliver(void *ctx, const unsigned char *frame, size_t len)
{
    fifo *f = (fifo *)ctx;
    if (f->count == FIFO_SLOTS || len > FIFO_FRAME) {
        return ZDW_T_ERROR;
    }
    memcpy(f->buf[f->tail], frame, len);
    f->len[f->tail] = len;
    f->tail = (f->tail + 1) % FIFO_SLOTS;
    f->count++;
    return ZDW_T_OK;
}

static int fifo_receive(void *ctx, unsigned char *out, size_t cap, size_t *len)
{
    fifo *f = (fifo *)ctx;
    if (f->count == 0) {
        return ZDW_T_AGAIN;
    }
    if (f->len[f->head] > cap) {
        return ZDW_T_ERROR;
    }
    memcpy(out, f->buf[f->head], f->len[f->head]);
    *len = f->len[f->head];
    f->head = (f->head + 1) % FIFO_SLOTS;
    f->count--;
    return ZDW_T_OK;
}

/* ---- Reading { uint32 id; float value; string label } ---- */

typedef struct {
    unsigned long id;
    float         value;
    char          label[32];
} reading;

static int reading_encode(zdw_writer *w, const reading *rd)
{
    zdw_put_u32(w, rd->id);
    zdw_put_f32(w, rd->value);
    zdw_put_string(w, rd->label);
    return w->error;
}

static int reading_decode(zdw_reader *r, reading *rd)
{
    zdw_get_u32(r, &rd->id);
    zdw_get_f32(r, &rd->value);
    zdw_get_string(r, rd->label, sizeof(rd->label));
    return r->error;
}

int main(void)
{
    fifo fq;
    zdw_transport t;
    int i, got;

    memset(&fq, 0, sizeof(fq));
    t.ctx = &fq;
    t.deliver = fifo_deliver;
    t.receive = fifo_receive;

    for (i = 0; i < TOTAL; i++) {
        reading rd;
        zdw_writer w;
        unsigned char body[128];
        unsigned char frame[160];
        size_t frame_len;

        rd.id = 0x1000UL + (unsigned long)i;
        rd.value = 20.0f + (float)i * 0.5f;
        sprintf(rd.label, "bay-%02d", i);

        zdw_writer_init(&w, body, sizeof(body), ZDW_LE);
        if (reading_encode(&w, &rd) != ZDW_OK) {
            fprintf(stderr, "encode %d failed\n", i);
            return 1;
        }
        frame_len = zdw_xrce_write_frame(frame, sizeof(frame),
            ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT,
            (unsigned int)(i + 1), body, w.len);
        if (frame_len == 0) {
            fprintf(stderr, "frame %d failed\n", i);
            return 1;
        }
        if (t.deliver(t.ctx, frame, frame_len) != ZDW_T_OK) {
            fprintf(stderr, "deliver %d failed\n", i);
            return 1;
        }
    }

    got = 0;
    for (;;) {
        unsigned char frame[256];
        size_t frame_len;
        const unsigned char *body;
        size_t body_len;
        reading rd;
        zdw_reader r;

        if (t.receive(t.ctx, frame, sizeof(frame), &frame_len) != ZDW_T_OK) {
            break;
        }
        if (zdw_xrce_read_frame(frame, frame_len, &body, &body_len) != ZDW_T_OK) {
            continue;
        }
        zdw_reader_init(&r, body, body_len, ZDW_LE);
        memset(&rd, 0, sizeof(rd));
        if (reading_decode(&r, &rd) != ZDW_OK) {
            fprintf(stderr, "decode failed\n");
            return 1;
        }
        printf("sync reading %d: id=0x%lx value=%.1f label=\"%s\"\n",
               got, rd.id, (double)rd.value, rd.label);
        got++;
    }

    if (got != TOTAL) {
        fprintf(stderr, "incomplete: got %d of %d\n", got, TOTAL);
        return 1;
    }
    printf("ALL OK\n");
    return 0;
}
