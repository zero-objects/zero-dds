/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Reliable stream (ADR 0013 P4): the endpoint parses an agent HEARTBEAT and
 * builds a matching ACKNACK, byte-identical to crates/xrce.
 * usage: test_reliable <golden_heartbeat_le.bin> <golden_acknack_le.bin>
 */
#include <stdio.h>
#include <string.h>
#include "zerodds_endpoint.h"

static long rd(const char *p, unsigned char *b, long c)
{ FILE *f = fopen(p, "rb"); long n; if (!f) { fprintf(stderr, "open %s\n", p); return -1; }
  n = (long)fread(b, 1, (size_t)c, f); fclose(f); return n; }

int main(int argc, char **argv)
{
    unsigned char hb[64], an_golden[64], an[64];
    long hn, gn;
    int first = 0, last = 0;
    unsigned char stream = 0;
    size_t alen;

    if (argc < 3) { fprintf(stderr, "usage: %s hb an\n", argv[0]); return 2; }
    hn = rd(argv[1], hb, (long)sizeof(hb));
    gn = rd(argv[2], an_golden, (long)sizeof(an_golden));
    if (hn < 0 || gn < 0) { return 1; }

    /* parse the agent HEARTBEAT */
    if (zdw_xrce_heartbeat_read(hb, (size_t)hn, &first, &last, &stream) != ZDW_T_OK) {
        fprintf(stderr, "heartbeat parse failed\n"); return 1;
    }
    if (first != 1 || last != 3 || stream != 0x80) {
        fprintf(stderr, "heartbeat values: first=%d last=%d stream=0x%02X\n", first, last, stream);
        return 1;
    }
    printf("HEARTBEAT parsed: first=%d last=%d stream=0x%02X\n", first, last, stream);

    /* reply with ACKNACK (all received) -- byte-identical to crates/xrce */
    alen = zdw_xrce_acknack_frame(an, sizeof(an), 0x80, 0x00, 1, 1, 0x00, 0x00, 0x80);
    if (alen == 0) { fprintf(stderr, "acknack build\n"); return 1; }
    if ((long)alen != gn || memcmp(an, an_golden, (size_t)gn) != 0) {
        fprintf(stderr, "ACKNACK mismatch C=%lu golden=%ld\n", (unsigned long)alen, gn);
        return 1;
    }
    printf("ACKNACK %lu bytes byte-identical to crates/xrce\n", (unsigned long)alen);
    printf("ALL OK\n");
    return 0;
}
