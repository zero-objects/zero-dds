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
#include "../../c/test/golden.h"

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

static int check(int endian, const char *golden_path, const char *tag)
{
    SensorReading s, d;
    unsigned char out[256];
    zdw_writer w;
    zdw_reader r;

    fill(&s);
    zdw_writer_init(&w, out, sizeof(out), endian);
    if (sensor_reading_encode(&w, &s) != ZDW_OK) {
        fprintf(stderr, "%s: encode error\n", tag);
        return 1;
    }
    if (golden_matches(&w, golden_path, tag, " (GENERATED codec)") != 0) return 1;

    memset(&d, 0, sizeof(d));
    zdw_reader_init(&r, out, w.len, endian);
    if (sensor_reading_decode(&r, &d) != ZDW_OK) {
        fprintf(stderr, "%s: decode error\n", tag);
        return 1;
    }
    return golden_round_trip(d.id == s.id && d.kind == s.kind && d.flags == s.flags &&
                             d.value == s.value && d.stamp.hi == s.stamp.hi &&
                             d.stamp.lo == s.stamp.lo && strcmp(d.label, s.label) == 0 &&
                             d.raw_len == s.raw_len &&
                             memcmp(d.raw, s.raw, s.raw_len) == 0, tag);
}

int main(int argc, char **argv)
{
    return golden_main(argc, argv, check);
}
