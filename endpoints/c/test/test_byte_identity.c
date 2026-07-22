/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Byte-identity test for the C89 wire-core (ADR 0013 P1).
 *
 * Encodes a fixed SensorReading in both LE and BE wire order and compares the
 * bytes to goldens produced by the Rust core (endpoints/golden-gen, using
 * zerodds-cdr). Then round-trips a decode. The same executable, cross-compiled
 * for a big-endian target and run under qemu, proves host-endian independence:
 * the wire bytes must be identical on an LE and a BE host.
 *
 * usage: test_byte_identity <golden_le.bin> <golden_be.bin>
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "sample_sensor.h"

/* The fixed test vector -- MUST match endpoints/golden-gen/src/main.rs. */
static void fill_sample(sensor_reading *s)
{
    s->id = 0xA1B2C3D4uL;
    s->kind = 0x1234u;
    s->flags = 0x5Au;
    s->value = 3.5f; /* 0x40600000 */
    s->stamp.hi = 0x01020304uL;
    s->stamp.lo = 0x05060708uL;
    strcpy(s->label, "bay-12");
    s->raw[0] = 0xDE;
    s->raw[1] = 0xAD;
    s->raw[2] = 0xBE;
    s->raw[3] = 0xEF;
    s->raw_len = 4;
}

static long read_file(const char *path, unsigned char *buf, long cap)
{
    FILE *f = fopen(path, "rb");
    long n;
    if (f == NULL) {
        fprintf(stderr, "cannot open %s\n", path);
        return -1;
    }
    n = (long)fread(buf, 1, (size_t)cap, f);
    fclose(f);
    return n;
}

static int check_wire(int endian, const char *golden_path, const char *tag)
{
    sensor_reading s;
    unsigned char out[256];
    unsigned char golden[256];
    long gn;
    zdw_writer w;
    size_t i;

    fill_sample(&s);
    zdw_writer_init(&w, out, sizeof(out), endian);
    if (sensor_encode(&w, &s) != ZDW_OK) {
        fprintf(stderr, "%s: encode error %d\n", tag, w.error);
        return 1;
    }
    gn = read_file(golden_path, golden, (long)sizeof(golden));
    if (gn < 0) {
        return 1;
    }
    if ((long)w.len != gn) {
        fprintf(stderr, "%s: length mismatch: C=%lu golden=%ld\n",
                tag, (unsigned long)w.len, gn);
        return 1;
    }
    for (i = 0; i < w.len; i++) {
        if (out[i] != golden[i]) {
            fprintf(stderr, "%s: byte %lu differs: C=0x%02X golden=0x%02X\n",
                    tag, (unsigned long)i, out[i], golden[i]);
            return 1;
        }
    }
    printf("%s: %lu bytes byte-identical to Rust golden\n",
           tag, (unsigned long)w.len);

    /* round-trip decode */
    {
        sensor_reading d;
        zdw_reader r;
        memset(&d, 0, sizeof(d));
        zdw_reader_init(&r, out, w.len, endian);
        if (sensor_decode(&r, &d) != ZDW_OK) {
            fprintf(stderr, "%s: decode error %d\n", tag, r.error);
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
    }
    return 0;
}

int main(int argc, char **argv)
{
    int rc = 0;
    if (argc < 3) {
        fprintf(stderr, "usage: %s <golden_le.bin> <golden_be.bin>\n", argv[0]);
        return 2;
    }
    rc |= check_wire(ZDW_LE, argv[1], "LE");
    rc |= check_wire(ZDW_BE, argv[2], "BE");
    if (rc == 0) {
        printf("ALL OK\n");
    }
    return rc;
}
