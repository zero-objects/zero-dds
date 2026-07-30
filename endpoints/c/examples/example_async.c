/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Deep example (async, C11): the same sensor-telemetry flow, but the subscriber
 * does not own the run-loop. An async writer fires five typed Reading samples
 * into an in-memory FIFO; the event-driven reactor drains them and dispatches
 * each to an on_sample callback that decodes EVERY field. The idiomatic C
 * event-driven add-on (no threads assumed, no malloc).
 */

#include <stdio.h>
#include <string.h>

#include "zerodds_async.h"
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

/* ---- the reactor callback decodes + prints each sample ---- */

typedef struct {
    int got;
} collector;

static void on_sample(void *ctx, const unsigned char *body, size_t body_len)
{
    collector *c = (collector *)ctx;
    reading rd;
    zdw_reader r;

    zdw_reader_init(&r, body, body_len, ZDW_LE);
    memset(&rd, 0, sizeof(rd));
    if (reading_decode(&r, &rd) == ZDW_OK) {
        printf("async reading %d: id=0x%lx value=%.1f label=\"%s\"\n",
               c->got, rd.id, (double)rd.value, rd.label);
        c->got++;
    }
}

int main(void)
{
    fifo fq;
    zdw_transport t = {0};
    collector col = {0};
    unsigned char rxbuf[FIFO_FRAME];
    unsigned char txbuf[FIFO_FRAME];
    zdw_async_reader reader;
    zdw_async_writer writer;
    int i, dispatched;

    memset(&fq, 0, sizeof(fq));
    t.ctx = &fq;
    t.deliver = fifo_deliver;
    t.receive = fifo_receive;

    zdw_async_reader_init(&reader, &t, rxbuf, sizeof(rxbuf), on_sample, &col);
    zdw_async_writer_init(&writer, &t, txbuf, sizeof(txbuf),
                          ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT);

    for (i = 0; i < TOTAL; i++) {
        reading rd;
        zdw_writer w;
        unsigned char body[128];

        rd.id = 0x2000UL + (unsigned long)i;
        rd.value = 100.0f - (float)i;
        sprintf(rd.label, "sensor-%02d", i);

        zdw_writer_init(&w, body, sizeof(body), ZDW_LE);
        if (reading_encode(&w, &rd) != ZDW_OK) {
            fprintf(stderr, "encode %d failed\n", i);
            return 1;
        }
        if (zdw_async_write(&writer, body, w.len) != ZDW_T_OK) {
            fprintf(stderr, "async write %d failed\n", i);
            return 1;
        }
    }

    dispatched = zdw_async_run(&reader, 0);
    if (dispatched != TOTAL || col.got != TOTAL) {
        fprintf(stderr, "dispatched %d, decoded %d, expected %d\n",
                dispatched, col.got, TOTAL);
        return 1;
    }

    printf("ALL OK\n");
    return 0;
}
