/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * wire-variable proof (ADR 0013): reflectively encode SensorReading via a
 * runtime field descriptor and get bytes byte-identical to the fixed/Rust
 * golden, then reflectively decode. Same wire, no per-type code.
 * usage: test_reflect <golden_le.bin> <golden_be.bin>
 */
#include <stdio.h>
#include <string.h>
#include "zerodds_reflect.h"

static long rd(const char *p, unsigned char *b, long c)
{ FILE *f = fopen(p, "rb"); long n; if (!f) { fprintf(stderr, "open %s\n", p); return -1; }
  n = (long)fread(b, 1, (size_t)c, f); fclose(f); return n; }

static int check(int endian, const char *gp, const char *tag)
{
    unsigned char raw[4], out[256], golden[256], rawbuf[64];
    char labelbuf[64];
    zdw_dyn_field f[7], g[7];
    long gn; zdw_writer w; zdw_reader r; size_t i;

    raw[0]=0xDE; raw[1]=0xAD; raw[2]=0xBE; raw[3]=0xEF;
    memset(f, 0, sizeof(f));
    f[0].kind=ZDW_K_U32; f[0].u32=0xA1B2C3D4uL;
    f[1].kind=ZDW_K_U16; f[1].u16=0x1234u;
    f[2].kind=ZDW_K_U8;  f[2].u8=0x5Au;
    f[3].kind=ZDW_K_F32; f[3].f32=3.5f;
    f[4].kind=ZDW_K_U64; f[4].u64.hi=0x01020304uL; f[4].u64.lo=0x05060708uL;
    f[5].kind=ZDW_K_STRING; f[5].str=(char*)"bay-12";
    f[6].kind=ZDW_K_SEQ_U8; f[6].bytes=raw; f[6].bytes_len=4;

    zdw_writer_init(&w, out, sizeof(out), endian);
    if (zdw_reflect_encode(&w, f, 7) != ZDW_OK) { fprintf(stderr, "%s: enc\n", tag); return 1; }
    gn = rd(gp, golden, (long)sizeof(golden)); if (gn < 0) return 1;
    if ((long)w.len != gn || memcmp(out, golden, (size_t)gn) != 0) {
        fprintf(stderr, "%s: reflective encode mismatch C=%lu golden=%ld\n", tag, (unsigned long)w.len, gn);
        return 1;
    }
    printf("%s: %lu bytes byte-identical to Rust golden (REFLECTIVE)\n", tag, (unsigned long)w.len);

    /* reflective decode */
    memset(g, 0, sizeof(g));
    for (i = 0; i < 7; i++) g[i].kind = f[i].kind;
    g[5].str = labelbuf; g[5].str_cap = sizeof(labelbuf);
    g[6].bytes = rawbuf; g[6].bytes_cap = sizeof(rawbuf);
    zdw_reader_init(&r, out, w.len, endian);
    if (zdw_reflect_decode(&r, g, 7) != ZDW_OK) { fprintf(stderr, "%s: dec\n", tag); return 1; }
    if (g[0].u32 != 0xA1B2C3D4uL || g[1].u16 != 0x1234u || g[2].u8 != 0x5Au ||
        g[3].f32 != 3.5f || g[4].u64.lo != 0x05060708uL ||
        strcmp(g[5].str, "bay-12") != 0 || g[6].bytes_len != 4) {
        fprintf(stderr, "%s: reflective decode mismatch\n", tag); return 1;
    }
    printf("%s: reflective decode ok\n", tag);
    return 0;
}
/* @mutable reflective: MutableM { @id(10) u32 x; @id(20) string s; @id(30) u16 k; } */
static int check_mutable(int endian, const char *gp, const char *tag)
{
    unsigned char out[256], golden[256];
    char sbuf[64];
    zdw_dyn_field f[3], g[3];
    unsigned long ids[3];
    long gn; zdw_writer w; zdw_reader r; size_t i;

    ids[0]=10; ids[1]=20; ids[2]=30;
    memset(f, 0, sizeof(f));
    f[0].kind=ZDW_K_U32; f[0].u32=0xDEADBEEFuL;
    f[1].kind=ZDW_K_STRING; f[1].str=(char*)"mut";
    f[2].kind=ZDW_K_U16; f[2].u16=0x0777u;

    zdw_writer_init(&w, out, sizeof(out), endian);
    if (zdw_reflect_encode_ext(&w, ZDW_X_MUTABLE, f, ids, 3) != ZDW_OK) { fprintf(stderr, "%s: menc\n", tag); return 1; }
    gn = rd(gp, golden, (long)sizeof(golden)); if (gn < 0) return 1;
    if ((long)w.len != gn || memcmp(out, golden, (size_t)gn) != 0) {
        fprintf(stderr, "%s: reflective @mutable mismatch C=%lu golden=%ld\n", tag, (unsigned long)w.len, gn);
        return 1;
    }
    printf("%s: %lu bytes byte-identical to Rust golden (REFLECTIVE @mutable)\n", tag, (unsigned long)w.len);

    memset(g, 0, sizeof(g));
    g[0].kind=ZDW_K_U32; g[1].kind=ZDW_K_STRING; g[1].str=sbuf; g[1].str_cap=sizeof(sbuf); g[2].kind=ZDW_K_U16;
    zdw_reader_init(&r, out, w.len, endian);
    if (zdw_reflect_decode_ext(&r, ZDW_X_MUTABLE, g, 3) != ZDW_OK) { fprintf(stderr, "%s: mdec\n", tag); return 1; }
    if (g[0].u32 != 0xDEADBEEFuL || strcmp(g[1].str, "mut") != 0 || g[2].u16 != 0x0777u) {
        fprintf(stderr, "%s: reflective @mutable decode mismatch\n", tag); return 1;
    }
    (void)i;
    printf("%s: reflective @mutable decode ok\n", tag);
    return 0;
}

int main(int argc, char **argv)
{
    int rc = 0;
    if (argc < 3) { fprintf(stderr, "usage: %s le be [mutable_le mutable_be]\n", argv[0]); return 2; }
    rc |= check(ZDW_LE, argv[1], "LE");
    rc |= check(ZDW_BE, argv[2], "BE");
    if (argc >= 5) {
        rc |= check_mutable(ZDW_LE, argv[3], "LE");
        rc |= check_mutable(ZDW_BE, argv[4], "BE");
    }
    if (rc == 0) printf("ALL OK\n");
    return rc;
}
