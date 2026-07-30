/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Async reactor test (C11): an async writer fires N SensorReadings into an
 * in-memory FIFO transport; the async reader drains them via a callback and
 * decodes each. Proves the event-driven receive dispatches every sample.
 */

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "zerodds_async.h"
#include "sample_sensor.h"

/* ---- an integrator-supplied transport: a small FIFO of frames ---- */

enum { FIFO_SLOTS = 16, FIFO_FRAME = 256 };

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

/* ---- the callback collects decoded values ---- */

#define N 5

typedef struct {
    unsigned long ids[N];
    int           got;
} collector;

static void on_sample(void *ctx, const unsigned char *body, size_t body_len)
{
    collector *c = (collector *)ctx;
    sensor_reading s;
    zdw_reader r;
    zdw_reader_init(&r, body, body_len, ZDW_LE);
    memset(&s, 0, sizeof(s));
    if (sensor_decode(&r, &s) == ZDW_OK && c->got < N) {
        c->ids[c->got++] = s.id;
    }
}

int main(void)
{
    fifo fq = {0};
    zdw_transport t = {&fq, fifo_deliver, fifo_receive};

    collector col = {0};
    unsigned char rxbuf[FIFO_FRAME];
    zdw_async_reader reader;
    zdw_async_reader_init(&reader, &t, rxbuf, sizeof(rxbuf), on_sample, &col);

    unsigned char txbuf[FIFO_FRAME];
    zdw_async_writer writer;
    zdw_async_writer_init(&writer, &t, txbuf, sizeof(txbuf),
                          ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT);

    /* Fire N samples with distinct ids. */
    {
        int i;
        unsigned char body[128];
        for (i = 0; i < N; i++) {
            sensor_reading s;
            zdw_writer w;
            memset(&s, 0, sizeof(s));
            s.id = 0x1000u + (unsigned long)i;
            s.value = 1.0f;
            strcpy(s.label, "async");
            s.raw_len = 0;
            zdw_writer_init(&w, body, sizeof(body), ZDW_LE);
            if (sensor_encode(&w, &s) != ZDW_OK) {
                fprintf(stderr, "encode %d failed\n", i);
                return 1;
            }
            if (zdw_async_write(&writer, body, w.len) != ZDW_T_OK) {
                fprintf(stderr, "async write %d failed\n", i);
                return 1;
            }
        }
    }

    /* Drain the reactor. */
    {
        int dispatched = zdw_async_run(&reader, 0);
        if (dispatched != N) {
            fprintf(stderr, "dispatched %d, expected %d\n", dispatched, N);
            return 1;
        }
    }

    /* Every sample arrived, in order. */
    if (col.got != N) {
        fprintf(stderr, "got %d samples, expected %d\n", col.got, N);
        return 1;
    }
    {
        int i;
        for (i = 0; i < N; i++) {
            if (col.ids[i] != 0x1000u + (unsigned long)i) {
                fprintf(stderr, "sample %d: id 0x%lX, expected 0x%X\n",
                        i, col.ids[i], 0x1000u + (unsigned)i);
                return 1;
            }
        }
    }

    printf("async loopback: %d samples dispatched + decoded in order\n", N);
    printf("ALL OK\n");
    return 0;
}
