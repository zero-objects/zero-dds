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
#include "golden.h"

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

static int check(int endian, const char *golden_path, const char *tag)
{
    outer_t o, d;
    unsigned char out[256];
    zdw_writer w;
    zdw_reader r;

    fill(&o);
    zdw_writer_init(&w, out, sizeof(out), endian);
    if (outer_encode(&w, &o) != ZDW_OK) {
        fprintf(stderr, "%s: encode error %d\n", tag, w.error);
        return 1;
    }
    if (golden_matches(&w, golden_path, tag, " (DHEADER/nested/seq)") != 0) return 1;

    memset(&d, 0, sizeof(d));
    zdw_reader_init(&r, out, w.len, endian);
    if (outer_decode(&r, &d) != ZDW_OK) {
        fprintf(stderr, "%s: decode error %d\n", tag, r.error);
        return 1;
    }
    return golden_round_trip(d.id == o.id && d.one.a == o.one.a && d.one.b == o.one.b &&
                             d.many_len == o.many_len && d.many[0].a == o.many[0].a &&
                             d.many[0].b == o.many[0].b && d.many[1].a == o.many[1].a &&
                             d.many[1].b == o.many[1].b && strcmp(d.label, o.label) == 0, tag);
}

int main(int argc, char **argv)
{
    return golden_main(argc, argv, check);
}
