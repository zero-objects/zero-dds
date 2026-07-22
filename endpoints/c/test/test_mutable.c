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

static long read_file(const char *path, unsigned char *buf, long cap)
{
    FILE *f = fopen(path, "rb");
    long n;
    if (f == NULL) { fprintf(stderr, "cannot open %s\n", path); return -1; }
    n = (long)fread(buf, 1, (size_t)cap, f);
    fclose(f);
    return n;
}

static int check(int endian, const char *golden_path, const char *tag)
{
    mutable_m m, d;
    unsigned char out[256], golden[256];
    long gn;
    size_t i;
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
    gn = read_file(golden_path, golden, (long)sizeof(golden));
    if (gn < 0) return 1;
    if ((long)w.len != gn) {
        fprintf(stderr, "%s: length mismatch C=%lu golden=%ld\n",
                tag, (unsigned long)w.len, gn);
        return 1;
    }
    for (i = 0; i < w.len; i++) {
        if (out[i] != golden[i]) {
            fprintf(stderr, "%s: byte %lu differs C=0x%02X golden=0x%02X\n",
                    tag, (unsigned long)i, out[i], golden[i]);
            return 1;
        }
    }
    printf("%s: %lu bytes byte-identical to Rust golden (EMHEADER/@mutable)\n",
           tag, (unsigned long)w.len);

    memset(&d, 0, sizeof(d));
    zdw_reader_init(&r, out, w.len, endian);
    if (mutable_decode(&r, &d) != ZDW_OK) {
        fprintf(stderr, "%s: decode error %d\n", tag, r.error);
        return 1;
    }
    if (d.x != m.x || strcmp(d.s, m.s) != 0 || d.k != m.k) {
        fprintf(stderr, "%s: round-trip mismatch\n", tag);
        return 1;
    }
    printf("%s: round-trip decode ok\n", tag);
    return 0;
}

int main(int argc, char **argv)
{
    int rc = 0;
    if (argc < 3) {
        fprintf(stderr, "usage: %s <mutable_le> <mutable_be>\n", argv[0]);
        return 2;
    }
    rc |= check(ZDW_LE, argv[1], "LE");
    rc |= check(ZDW_BE, argv[2], "BE");
    if (rc == 0) printf("ALL OK\n");
    return rc;
}
