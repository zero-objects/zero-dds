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
#include "golden.h"

static int check_wire(int endian, const char *golden_path, const char *tag)
{
    sensor_reading s, d;
    unsigned char out[256];
    zdw_writer w;
    zdw_reader r;

    sensor_fill_golden(&s);
    zdw_writer_init(&w, out, sizeof(out), endian);
    if (sensor_encode(&w, &s) != ZDW_OK) {
        fprintf(stderr, "%s: encode error %d\n", tag, w.error);
        return 1;
    }
    if (golden_matches(&w, golden_path, tag, "") != 0) return 1;

    memset(&d, 0, sizeof(d));
    zdw_reader_init(&r, out, w.len, endian);
    if (sensor_decode(&r, &d) != ZDW_OK) {
        fprintf(stderr, "%s: decode error %d\n", tag, r.error);
        return 1;
    }
    return golden_round_trip(d.id == s.id && d.kind == s.kind && d.flags == s.flags &&
                             d.value == s.value && d.stamp.hi == s.stamp.hi &&
                             d.stamp.lo == s.stamp.lo && strcmp(d.label, s.label) == 0 &&
                             d.raw_len == s.raw_len && memcmp(d.raw, s.raw, s.raw_len) == 0,
                             tag);
}

int main(int argc, char **argv)
{
    return golden_main(argc, argv, check_wire);
}
