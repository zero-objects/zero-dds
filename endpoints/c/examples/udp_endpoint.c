/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Example frame-hook fill for a POSIX UDP transport (ADR 0013). Encodes a
 * SensorReading, frames it as XRCE WRITE_DATA, and delivers it to a
 * host:port -- e.g. a live crates/xrce agent. This is the "full" (POSIX)
 * substrate filling zdw_transport; another target fills it with its own
 * transport instead.
 *
 * usage: udp_endpoint <host> <port>
 */

#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <unistd.h>

#include "zerodds_endpoint.h"
#include "sample_sensor.h"

/* transport ctx: a connected UDP socket */
typedef struct { int fd; } udp_ctx;

static int udp_deliver(void *c, const unsigned char *frame, size_t len)
{
    udp_ctx *u = (udp_ctx *)c;
    if (send(u->fd, frame, len, 0) < 0) {
        return ZDW_T_ERROR;
    }
    return ZDW_T_OK;
}

static int udp_receive(void *c, unsigned char *buf, size_t cap, size_t *len)
{
    (void)c; (void)buf; (void)cap; (void)len;
    return ZDW_T_AGAIN; /* send-only example */
}

int main(int argc, char **argv)
{
    udp_ctx u;
    zdw_transport t;
    sensor_reading s;
    unsigned char body[256], frame[512];
    size_t blen, flen;
    zdw_writer w;
    struct sockaddr_in addr;

    if (argc < 3) { fprintf(stderr, "usage: %s <host> <port>\n", argv[0]); return 2; }

    u.fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (u.fd < 0) { perror("socket"); return 1; }
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_port = htons((unsigned short)atoi(argv[2]));
    if (inet_pton(AF_INET, argv[1], &addr.sin_addr) != 1) { fprintf(stderr, "bad host\n"); return 1; }
    if (connect(u.fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) { perror("connect"); return 1; }

    t.ctx = &u; t.deliver = udp_deliver; t.receive = udp_receive;

    s.id = 0xA1B2C3D4uL; s.kind = 0x1234u; s.flags = 0x5Au; s.value = 3.5f;
    s.stamp.hi = 0x01020304uL; s.stamp.lo = 0x05060708uL;
    strcpy(s.label, "bay-12");
    s.raw[0] = 0xDE; s.raw[1] = 0xAD; s.raw[2] = 0xBE; s.raw[3] = 0xEF; s.raw_len = 4;

    zdw_writer_init(&w, body, sizeof(body), ZDW_LE);
    if (sensor_encode(&w, &s) != ZDW_OK) { fprintf(stderr, "encode\n"); return 1; }
    blen = w.len;
    flen = zdw_xrce_write_frame(frame, sizeof(frame), ZDW_XRCE_SESSION_NOKEY,
                                ZDW_XRCE_STREAM_BEST_EFFORT, 1, body, blen);
    if (zdw_endpoint_send(&t, frame, flen) != ZDW_T_OK) { fprintf(stderr, "send\n"); return 1; }
    printf("sent %lu-byte XRCE WRITE_DATA frame to %s:%s\n",
           (unsigned long)flen, argv[1], argv[2]);
    close(u.fd);
    return 0;
}
