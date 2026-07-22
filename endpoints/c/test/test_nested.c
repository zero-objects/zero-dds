/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Byte-identity test for the DHEADER / nested / sequence<@appendable> path
 * (ADR 0013). Same vector as endpoints/golden-gen encode_nested.
 *
 * usage: test_nested <golden_nested_le.bin> <golden_nested_be.bin>
 */

#include <stdio.h>
#include <string.h>

#include "sample_nested.h"

static void fill(outer_t *o)
{
    o->id = 0xCAFEBABEuL;
    o->one.a = 0x1111u;
    o->one.b = 0x22223333uL;
    o->many[0].a = 0xAAAAu;
    o->many[0].b = 0xBBBBCCCCuL;
    o->many[1].a = 0xDDDDu;
    o->many[1].b = 0xEEEEFFFFuL;
    o->many_len = 2;
    strcpy(o->label, "nested");
}

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
    outer_t o, d;
    unsigned char out[256], golden[256];
    long gn;
    size_t i;
    zdw_writer w;
    zdw_reader r;

    fill(&o);
    zdw_writer_init(&w, out, sizeof(out), endian);
    if (outer_encode(&w, &o) != ZDW_OK) {
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
    printf("%s: %lu bytes byte-identical to Rust golden (DHEADER/nested/seq)\n",
           tag, (unsigned long)w.len);

    memset(&d, 0, sizeof(d));
    zdw_reader_init(&r, out, w.len, endian);
    if (outer_decode(&r, &d) != ZDW_OK) {
        fprintf(stderr, "%s: decode error %d\n", tag, r.error);
        return 1;
    }
    if (d.id != o.id || d.one.a != o.one.a || d.one.b != o.one.b ||
        d.many_len != o.many_len ||
        d.many[0].a != o.many[0].a || d.many[0].b != o.many[0].b ||
        d.many[1].a != o.many[1].a || d.many[1].b != o.many[1].b ||
        strcmp(d.label, o.label) != 0) {
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
        fprintf(stderr, "usage: %s <nested_le> <nested_be>\n", argv[0]);
        return 2;
    }
    rc |= check(ZDW_LE, argv[1], "LE");
    rc |= check(ZDW_BE, argv[2], "BE");
    if (rc == 0) printf("ALL OK\n");
    return rc;
}
