/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Proves the CODEGEN-emitted wire-fixed codec (sensor_gen.c, generated from
 * sensor.idl by zerodds-endpoint-codegen) is byte-identical to the Rust core
 * (ADR 0013). Same test vector as the hand-written sample. Generate first:
 *   cargo run -p zerodds-endpoint-codegen -- idl/sensor.idl <dir>
 *
 * usage: test_gen <golden_le.bin> <golden_be.bin>
 */

#include <stdio.h>
#include <string.h>

#include "sensor_gen.h" /* generated */

static void fill(SensorReading *s)
{
    s->id = 0xA1B2C3D4uL;
    s->kind = 0x1234u;
    s->flags = 0x5Au;
    s->value = 3.5f;
    s->stamp.hi = 0x01020304uL;
    s->stamp.lo = 0x05060708uL;
    strcpy(s->label, "bay-12");
    s->raw[0] = 0xDE; s->raw[1] = 0xAD; s->raw[2] = 0xBE; s->raw[3] = 0xEF;
    s->raw_len = 4;
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
    SensorReading s, d;
    unsigned char out[256], golden[256];
    long gn;
    size_t i;
    zdw_writer w;
    zdw_reader r;

    fill(&s);
    zdw_writer_init(&w, out, sizeof(out), endian);
    if (sensor_reading_encode(&w, &s) != ZDW_OK) {
        fprintf(stderr, "%s: encode error\n", tag);
        return 1;
    }
    gn = read_file(golden_path, golden, (long)sizeof(golden));
    if (gn < 0) return 1;
    if ((long)w.len != gn || memcmp(out, golden, (size_t)gn) != 0) {
        fprintf(stderr, "%s: byte mismatch (gen=%lu golden=%ld)\n",
                tag, (unsigned long)w.len, gn);
        return 1;
    }
    printf("%s: %lu bytes byte-identical to Rust golden (GENERATED codec)\n",
           tag, (unsigned long)w.len);

    memset(&d, 0, sizeof(d));
    zdw_reader_init(&r, out, w.len, endian);
    if (sensor_reading_decode(&r, &d) != ZDW_OK) {
        fprintf(stderr, "%s: decode error\n", tag);
        return 1;
    }
    if (d.id != s.id || d.kind != s.kind || d.flags != s.flags ||
        d.value != s.value || d.stamp.hi != s.stamp.hi ||
        d.stamp.lo != s.stamp.lo || strcmp(d.label, s.label) != 0 ||
        d.raw_len != s.raw_len || memcmp(d.raw, s.raw, s.raw_len) != 0) {
        fprintf(stderr, "%s: round-trip mismatch\n", tag);
        return 1;
    }
    printf("%s: round-trip decode ok\n", tag);
    (void)i;
    return 0;
}

int main(int argc, char **argv)
{
    int rc = 0;
    if (argc < 3) {
        fprintf(stderr, "usage: %s <golden_le.bin> <golden_be.bin>\n", argv[0]);
        return 2;
    }
    rc |= check(ZDW_LE, argv[1], "LE");
    rc |= check(ZDW_BE, argv[2], "BE");
    if (rc == 0) printf("ALL OK\n");
    return rc;
}
