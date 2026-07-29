/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Reliable-stream UDP sender app for the live E2E (crates/endpoint-e2e). Plays
 * the reliable SENDER against the shared Rust ReliablePeer: submit N samples on
 * stream 0x80, WRITE_DATA each, then loop { HEARTBEAT; on ACKNACK recv_acknack
 * + retransmit the still-missing set bits } until the send window drains.
 *
 * POSIX sockets (compiled with the default gcc std, not the strict-C89 Makefile).
 * usage: reliable_udp_app <port> [count]
 */
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
#include <sys/time.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include "zerodds_endpoint.h"

static zdw_reliable snd;

static unsigned long now_ms(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (unsigned long)ts.tv_sec * 1000ul + (unsigned long)(ts.tv_nsec / 1000000l);
}

int main(int argc, char **argv)
{
    int fd, count, i;
    unsigned short port;
    struct sockaddr_in peer;
    struct timeval tv;
    unsigned char frame[600], tx[600];
    unsigned long deadline;

    if (argc < 2) { fprintf(stderr, "usage: %s <port> [count]\n", argv[0]); return 2; }
    port = (unsigned short)atoi(argv[1]);
    count = (argc > 2) ? atoi(argv[2]) : 12;
    if (count < 1 || count > ZDW_REL_WINDOW) { count = 12; }

    fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (fd < 0) { perror("socket"); return 1; }
    memset(&peer, 0, sizeof(peer));
    peer.sin_family = AF_INET;
    peer.sin_port = htons(port);
    peer.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    if (connect(fd, (struct sockaddr *)&peer, sizeof(peer)) < 0) {
        perror("connect"); return 1;
    }
    tv.tv_sec = 0; tv.tv_usec = 50000; /* 50 ms recv timeout */
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));

    zdw_reliable_init(&snd, 0x80);

    /* submit + transmit N samples; sample = seq as u32 LE. */
    for (i = 0; i < count; i++) {
        unsigned char sample[4];
        unsigned short seq = 0;
        size_t flen;
        sample[0] = (unsigned char)(i & 0xff);
        sample[1] = (unsigned char)((i >> 8) & 0xff);
        sample[2] = 0; sample[3] = 0;
        if (zdw_reliable_submit(&snd, sample, 4, &seq) != ZDW_REL_OK) {
            fprintf(stderr, "submit %d failed\n", i); return 1;
        }
        flen = zdw_xrce_write_frame(tx, sizeof(tx), 0x80, 0x80, seq, sample, 4);
        if (flen == 0 || send(fd, tx, flen, 0) < 0) { perror("send"); return 1; }
    }

    /* recover: heartbeat + ACKNACK-driven retransmit until the window drains. */
    deadline = now_ms() + 15000ul;
    while (zdw_reliable_in_flight_count(&snd) > 0 && now_ms() < deadline) {
        int first = 0, last = 0, fu = 0;
        unsigned int nack = 0;
        unsigned char stream = 0;
        ssize_t r;
        unsigned short b;

        if (zdw_reliable_pending_heartbeat(&snd, now_ms(), &first, &last)) {
            size_t hlen = zdw_xrce_heartbeat_frame(tx, sizeof(tx), 0x80, 0x80, 0,
                                                   first, last, 0x80);
            if (hlen > 0) { (void)send(fd, tx, hlen, 0); }
        }

        r = recv(fd, frame, sizeof(frame), 0);
        if (r < 13) { continue; } /* timeout or short frame */
        if (zdw_xrce_acknack_read(frame, (size_t)r, &fu, &nack, &stream) != ZDW_T_OK) {
            continue;
        }
        zdw_reliable_recv_acknack(&snd, fu, (unsigned short)nack);
        for (b = 0; b < 16; b++) {
            if ((nack >> b) & 1u) {
                unsigned short seq = (unsigned short)(fu + b);
                size_t l = 0;
                const unsigned char *pl = zdw_reliable_get_in_flight(&snd, seq, &l);
                if (pl != 0) {
                    size_t flen = zdw_xrce_write_frame(tx, sizeof(tx), 0x80, 0x80,
                                                       seq, pl, l);
                    if (flen > 0) { (void)send(fd, tx, flen, 0); }
                }
            }
        }
    }

    if (zdw_reliable_in_flight_count(&snd) == 0) {
        printf("reliable sender: all %d samples acknowledged\n", count);
        close(fd);
        return 0;
    }
    fprintf(stderr, "reliable sender: window not drained (%lu left)\n",
            (unsigned long)zdw_reliable_in_flight_count(&snd));
    close(fd);
    return 1;
}
