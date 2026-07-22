/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Reflective nested + sequence<struct> (ADR 0013): reflectively encode the
 * @appendable Outer{ id, Inner one, seq<Inner> many, label } via the recursive
 * struct codec; byte-identical to golden_nested.
 * usage: test_reflect_nested <golden_nested_le> <golden_nested_be>
 */
#include <stdio.h>
#include <string.h>
#include "zerodds_reflect.h"

static long rd(const char *p, unsigned char *b, long c)
{ FILE *f = fopen(p, "rb"); long n; if (!f) { fprintf(stderr, "open %s\n", p); return -1; }
  n = (long)fread(b, 1, (size_t)c, f); fclose(f); return n; }

static void inner(zdw_dyn_field *f, unsigned int a, unsigned long b)
{
    memset(f, 0, 2 * sizeof(zdw_dyn_field));
    f[0].kind = ZDW_K_U16; f[0].u16 = a;
    f[1].kind = ZDW_K_U32; f[1].u32 = b;
}

static int check(int endian, const char *gp, const char *tag)
{
    zdw_dyn_field of[4], onef[2], m0[2], m1[2];
    zdw_dyn_struct one_s, many_s[2], outer_s;
    unsigned char out[256], golden[256];
    long gn; zdw_writer w;

    inner(onef, 0x1111u, 0x22223333uL);
    inner(m0, 0xAAAAu, 0xBBBBCCCCuL);
    inner(m1, 0xDDDDu, 0xEEEEFFFFuL);
    one_s.ext = ZDW_X_APPENDABLE; one_s.fields = onef; one_s.ids = 0; one_s.n = 2;
    many_s[0].ext = ZDW_X_APPENDABLE; many_s[0].fields = m0; many_s[0].ids = 0; many_s[0].n = 2;
    many_s[1].ext = ZDW_X_APPENDABLE; many_s[1].fields = m1; many_s[1].ids = 0; many_s[1].n = 2;

    memset(of, 0, sizeof(of));
    of[0].kind = ZDW_K_U32; of[0].u32 = 0xCAFEBABEuL;
    of[1].kind = ZDW_K_NESTED; of[1].nested = &one_s;
    of[2].kind = ZDW_K_SEQ_STRUCT; of[2].elems = many_s; of[2].elems_len = 2;
    of[3].kind = ZDW_K_STRING; of[3].str = (char *)"nested";
    outer_s.ext = ZDW_X_APPENDABLE; outer_s.fields = of; outer_s.ids = 0; outer_s.n = 4;

    zdw_writer_init(&w, out, sizeof(out), endian);
    if (zdw_reflect_encode_struct(&w, &outer_s) != ZDW_OK) { fprintf(stderr, "%s: enc\n", tag); return 1; }
    gn = rd(gp, golden, (long)sizeof(golden)); if (gn < 0) return 1;
    if ((long)w.len != gn || memcmp(out, golden, (size_t)gn) != 0) {
        fprintf(stderr, "%s: reflective nested mismatch C=%lu golden=%ld\n", tag, (unsigned long)w.len, gn);
        return 1;
    }
    printf("%s: %lu bytes byte-identical to Rust golden (REFLECTIVE nested/seq<struct>)\n", tag, (unsigned long)w.len);
    return 0;
}
int main(int argc, char **argv)
{
    int rc = 0;
    if (argc < 3) { fprintf(stderr, "usage: %s nle nbe\n", argv[0]); return 2; }
    rc |= check(ZDW_LE, argv[1], "LE");
    rc |= check(ZDW_BE, argv[2], "BE");
    if (rc == 0) printf("ALL OK\n");
    return rc;
}
