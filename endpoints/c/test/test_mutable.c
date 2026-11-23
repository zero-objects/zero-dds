/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Byte-identity test for the EMHEADER / @mutable path (ADR 0013). Same vector
 * as endpoints/golden-gen encode_mutable.
 *
 * usage: test_mutable <golden_mutable_le.bin> <golden_mutable_be.bin>
 */

#include <stdio.h>
#include <string.h>

#include "sample_mutable.h"
#include "golden.h"

static int check(int endian, const char *golden_path, const char *tag)
{
    mutable_m m, d;
    unsigned char out[256];
    zdw_writer w;
    zdw_reader r;

    m.x = 0xDEADBEEFuL;
    strcpy(m.s, "mut");
    m.k = 0x0777u;

    zdw_writer_init(&w, out, sizeof(out), endian);
    if (mutable_encode(&w, &m) != ZDW_OK) {
        fprintf(stderr, "%s: encode error %d\n", tag, w.error);
        return 1;
    }
    if (golden_matches(&w, golden_path, tag, " (EMHEADER/@mutable)") != 0) return 1;

    memset(&d, 0, sizeof(d));
    zdw_reader_init(&r, out, w.len, endian);
    if (mutable_decode(&r, &d) != ZDW_OK) {
        fprintf(stderr, "%s: decode error %d\n", tag, r.error);
        return 1;
    }
    return golden_round_trip(d.x == m.x && strcmp(d.s, m.s) == 0 && d.k == m.k, tag);
}

int main(int argc, char **argv)
{
    return golden_main(argc, argv, check);
}
