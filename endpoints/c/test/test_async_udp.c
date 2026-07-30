/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Live-UDP async E2E (C11, POSIX): a real non-blocking UDP socket is the
 * transport. An async writer sends N SensorReadings over the loopback; the
 * async reader drains them via `zdw_async_run` (returns ZDW_T_AGAIN when the
 * socket has nothing), decoding each in the callback. Proves the reactor drives
 * real datagram I/O, not just an in-memory queue.
 */

#define _DEFAULT_SOURCE 1 /* usleep + BSD socket decls under -std=c11 -pedantic */

#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#include "zerodds_async.h"
#include "sample_sensor.h"

/* ---- a non-blocking UDP loopback transport ---- */

typedef struct {
    int                fd;
    struct sockaddr_in peer; /* where deliver() sends */
} udp_ctx;

static int udp_deliver(void *ctx, const unsigned char *frame, size_t len)
{
    udp_ctx *u = (udp_ctx *)ctx;
    ssize_t n = sendto(u->fd, frame, len, 0,
                       (struct sockaddr *)&u->peer, sizeof(u->peer));
    return (n == (ssize_t)len) ? ZDW_T_OK : ZDW_T_ERROR;
}

static int udp_receive(void *ctx, unsigned char *out, size_t cap, size_t *len)
{
    udp_ctx *u = (udp_ctx *)ctx;
    ssize_t n = recvfrom(u->fd, out, cap, 0, NULL, NULL);
    if (n < 0) {
        return (errno == EAGAIN || errno == EWOULDBLOCK) ? ZDW_T_AGAIN
                                                         : ZDW_T_ERROR;
    }
    *len = (size_t)n;
    return ZDW_T_OK;
}

#define N 5

typedef struct { int got; } collector;

static void on_sample(void *ctx, const unsigned char *body, size_t body_len)
{
    collector *c = (collector *)ctx;
    sensor_reading s;
    zdw_reader r;
    zdw_reader_init(&r, body, body_len, ZDW_LE);
    memset(&s, 0, sizeof(s));
    if (sensor_decode(&r, &s) == ZDW_OK) {
        c->got++;
    }
}

int main(void)
{
    udp_ctx u;
    struct sockaddr_in addr;
    socklen_t addrlen = sizeof(addr);

    u.fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (u.fd < 0) { perror("socket"); return 1; }

    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    addr.sin_port = 0; /* ephemeral */
    if (bind(u.fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        perror("bind"); return 1;
    }
    if (getsockname(u.fd, (struct sockaddr *)&addr, &addrlen) < 0) {
        perror("getsockname"); return 1;
    }
    /* Non-blocking: the reactor relies on recvfrom returning EAGAIN
     * (ZDW_T_AGAIN) once the socket is drained. */
    if (fcntl(u.fd, F_SETFL, O_NONBLOCK) < 0) {
        perror("fcntl"); return 1;
    }
    u.peer = addr; /* send to ourselves */

    /* Async writer sends N. */
    {
        unsigned char txbuf[256], body[128];
        zdw_async_writer w;
        int i;
        zdw_async_writer_init(&w, &(zdw_transport){&u, udp_deliver, udp_receive},
                              txbuf, sizeof(txbuf),
                              ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT);
        for (i = 0; i < N; i++) {
            sensor_reading s;
            zdw_writer bw;
            memset(&s, 0, sizeof(s));
            s.id = 0x2000u + (unsigned long)i;
            strcpy(s.label, "udp");
            s.raw_len = 0;
            zdw_writer_init(&bw, body, sizeof(body), ZDW_LE);
            sensor_encode(&bw, &s);
            if (zdw_async_write(&w, body, bw.len) != ZDW_T_OK) {
                fprintf(stderr, "udp write %d failed\n", i); return 1;
            }
        }
    }

    /* Reader drains with a bounded retry (datagrams may still be in flight). */
    {
        collector col = {0};
        unsigned char rxbuf[256];
        zdw_transport t = {&u, udp_deliver, udp_receive};
        zdw_async_reader r;
        int tries;
        zdw_async_reader_init(&r, &t, rxbuf, sizeof(rxbuf), on_sample, &col);
        for (tries = 0; tries < 1000 && col.got < N; tries++) {
            zdw_async_run(&r, 0);
            if (col.got < N) { usleep(1000); }
        }
        close(u.fd);
        if (col.got != N) {
            fprintf(stderr, "received %d/%d over UDP\n", col.got, N);
            return 1;
        }
        printf("async UDP: %d/%d samples received + decoded via reactor\n", col.got, N);
    }

    printf("ALL OK\n");
    return 0;
}
