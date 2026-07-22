/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Example receiver (ADR 0013): a native endpoint binds a UDP port, receives a
 * DATA message the hub pushes, unwraps the WRITE_DATA/DATA submessage, and
 * decodes the SensorReading. The POSIX-UDP transport fills the frame-hook;
 * another target fills it with its own transport instead.
 *   udp_receiver <port>
 */
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <unistd.h>
#include "zerodds_endpoint.h"
#include "sample_sensor.h"

int main(int argc, char **argv)
{
    int fd;
    struct sockaddr_in addr;
    unsigned char frame[2048];
    const unsigned char *body = 0;
    size_t blen = 0;
    long n;
    sensor_reading s;
    zdw_reader r;

    if (argc < 2) { fprintf(stderr, "usage: %s <port>\n", argv[0]); return 2; }
    fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (fd < 0) { perror("socket"); return 1; }
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = INADDR_ANY;
    addr.sin_port = htons((unsigned short)atoi(argv[1]));
    if (bind(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) { perror("bind"); return 1; }
    printf("c receiver: listening on udp/%s\n", argv[1]);

    n = (long)recv(fd, frame, sizeof(frame), 0);
    if (n < 0) { perror("recv"); return 1; }
    if (zdw_xrce_read_frame(frame, (size_t)n, &body, &blen) != ZDW_T_OK) {
        fprintf(stderr, "not a DATA frame\n"); return 1;
    }
    memset(&s, 0, sizeof(s));
    zdw_reader_init(&r, body, blen, ZDW_LE);
    if (sensor_decode(&r, &s) != ZDW_OK) { fprintf(stderr, "decode\n"); return 1; }
    if (s.id != 0xA1B2C3D4uL || strcmp(s.label, "bay-12") != 0) { fprintf(stderr, "mismatch\n"); return 1; }
    printf("C RECEIVER OK: id=0x%08lX label=%s value=%g\n", s.id, s.label, (double)s.value);
    close(fd);
    return 0;
}
