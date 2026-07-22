/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Receive path (ADR 0013 P4): the endpoint decodes a DATA message (id=9) that
 * a crates/xrce agent pushes to it. golden_data_le.bin is a real zerodds-xrce
 * DATA message; the endpoint unwraps it and decodes the sample.
 *
 * usage: test_receive <golden_data_le.bin>
 */
#include <stdio.h>
#include <string.h>
#include "zerodds_endpoint.h"
#include "sample_sensor.h"

int main(int argc, char **argv)
{
    unsigned char frame[512];
    const unsigned char *body = 0;
    size_t blen = 0, fn;
    sensor_reading rx;
    zdw_reader r;
    FILE *f;

    if (argc < 2) { fprintf(stderr, "usage: %s <golden_data_le.bin>\n", argv[0]); return 2; }
    f = fopen(argv[1], "rb");
    if (f == NULL) { fprintf(stderr, "open\n"); return 1; }
    fn = fread(frame, 1, sizeof(frame), f);
    fclose(f);

    if (zdw_xrce_read_frame(frame, fn, &body, &blen) != ZDW_T_OK) {
        fprintf(stderr, "not a DATA/WRITE_DATA frame\n"); return 1;
    }
    printf("received DATA frame, %lu-byte sample body\n", (unsigned long)blen);
    memset(&rx, 0, sizeof(rx));
    zdw_reader_init(&r, body, blen, ZDW_LE);
    if (sensor_decode(&r, &rx) != ZDW_OK) { fprintf(stderr, "decode\n"); return 1; }
    if (rx.id != 0xA1B2C3D4uL || strcmp(rx.label, "bay-12") != 0 || rx.raw_len != 4) {
        fprintf(stderr, "decoded sample mismatch\n"); return 1;
    }
    printf("agent DATA decoded: id=0x%08lX label=%s\n", rx.id, rx.label);
    printf("ALL OK\n");
    return 0;
}
