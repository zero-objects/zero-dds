/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The frame of the byte-identity tests of the C wire-core and of the codecs
 * zerodds-endpoint-codegen writes: the octets a test encoded against the
 * golden file endpoints/golden-gen wrote, the report of the round trip, and
 * a main that runs the test for the little-endian and the big-endian golden
 * file. C89. Every test that includes it calls all three functions; the
 * builds with -Werror warn about an unused static function.
 */

#ifndef ZERODDS_TEST_GOLDEN_H
#define ZERODDS_TEST_GOLDEN_H

#include <stdio.h>
#include <string.h>

#include "zerodds_wire.h"

/* 0 when the octets `w` wrote are the golden file `golden_path`; otherwise
 * the length or the first octet that differs is reported. `what` follows
 * "byte-identical to Rust golden" in the report. */
static int golden_matches(const zdw_writer *w, const char *golden_path,
                          const char *tag, const char *what)
{
    unsigned char golden[256];
    FILE *f = fopen(golden_path, "rb");
    long gn;
    size_t i;
    if (f == NULL) { fprintf(stderr, "cannot open %s\n", golden_path); return 1; }
    gn = (long)fread(golden, 1, sizeof(golden), f);
    fclose(f);
    if ((long)w->len != gn) {
        fprintf(stderr, "%s: length mismatch: C=%lu golden=%ld\n",
                tag, (unsigned long)w->len, gn);
        return 1;
    }
    for (i = 0; i < w->len; i++) {
        if (w->buf[i] != golden[i]) {
            fprintf(stderr, "%s: byte %lu differs: C=0x%02X golden=0x%02X\n",
                    tag, (unsigned long)i, w->buf[i], golden[i]);
            return 1;
        }
    }
    printf("%s: %lu bytes byte-identical to Rust golden%s\n",
           tag, (unsigned long)w->len, what);
    return 0;
}

/* 0 when the decoded values are the encoded ones (`decoded` non-zero). */
static int golden_round_trip(int decoded, const char *tag)
{
    if (!decoded) { fprintf(stderr, "%s: round-trip mismatch\n", tag); return 1; }
    printf("%s: round-trip decode ok\n", tag);
    return 0;
}

/* The main of a test: `check` against the LE and then the BE golden file
 * named on the command line. */
static int golden_main(int argc, char **argv,
                       int (*check)(int endian, const char *golden_path, const char *tag))
{
    int rc;
    if (argc < 3) {
        fprintf(stderr, "usage: %s <golden_le.bin> <golden_be.bin>\n", argv[0]);
        return 2;
    }
    rc = check(ZDW_LE, argv[1], "LE") | check(ZDW_BE, argv[2], "BE");
    if (rc == 0) printf("ALL OK\n");
    return rc;
}

#endif
