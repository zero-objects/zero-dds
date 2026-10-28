/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Proves the C XRCE serial framing is byte-identical to the
 * crates/xrce Annex C framer (golden_serial_le.bin, ADR 0013 P4): an XRCE
 * WRITE_DATA message from address 1 to the agent at 0, framed as
 * 7E | 01 | 00 | length LE | payload | FCS-16 LE (DDS-XRCE 1.0 Annex C.1.1).
 * Then a full round-trip: frame -> deframe -> unwrap WRITE_DATA -> decode,
 * the RFC 1662 check value, a byte-stuffing self-check and the refusals of
 * Annex C.1.3.
 *
 * usage: test_serial_frame <golden_serial_le.bin>
 */

#include <stdio.h>
#include <string.h>

#include "zerodds_endpoint.h"
#include "sample_sensor.h"

static void fill(sensor_reading *s)
{
    s->id = 0xA1B2C3D4uL; s->kind = 0x1234u; s->flags = 0x5Au; s->value = 3.5f;
    s->stamp.hi = 0x01020304uL; s->stamp.lo = 0x05060708uL;
    strcpy(s->label, "bay-12");
    s->raw[0] = 0xDE; s->raw[1] = 0xAD; s->raw[2] = 0xBE; s->raw[3] = 0xEF; s->raw_len = 4;
}

static int stuffing_selfcheck(void)
{
    /* 0x7E -> 0x7D 0x5E, 0x7D -> 0x7D 0x5D, also in the addresses. */
    unsigned char in[2], out[32], back[8], src = 0, dst = 0;
    size_t n, blen = 0, used = 0;
    in[0] = 0x7E; in[1] = 0x7D;
    n = zdw_serial_frame(out, sizeof(out), 0x7E, 0x7D, in, 2);
    /* frame: 7E | 7D 5E | 7D 5D | 02 00 | 7D 5E | 7D 5D | check */
    if (n < 11 || out[0] != 0x7E || out[1] != 0x7D || out[2] != 0x5E ||
        out[3] != 0x7D || out[4] != 0x5D || out[5] != 0x02 || out[6] != 0x00 ||
        out[7] != 0x7D || out[8] != 0x5E || out[9] != 0x7D || out[10] != 0x5D) {
        fprintf(stderr, "stuffing self-check failed\n");
        return 1;
    }
    if (zdw_serial_deframe(out, n, &src, &dst, back, sizeof(back), &blen, &used) != ZDW_T_OK
        || src != 0x7E || dst != 0x7D || blen != 2 || back[0] != 0x7E || back[1] != 0x7D
        || used != n) {
        fprintf(stderr, "stuffed round-trip failed\n");
        return 1;
    }
    printf("byte-stuffing: 0x7E->7D5E, 0x7D->7D5D ok\n");
    return 0;
}

/* RFC 1662 check value, and the refusals of Annex C.1.3: a wrong frameCheck
 * and a 0x7E before the frame is complete. */
static int integrity_selfcheck(void)
{
    unsigned char in[3] = {0x10, 0x20, 0x30}, out[32], back[8];
    size_t n, blen = 0;
    if (zdw_fcs16((const unsigned char *)"123456789", 9) != 0x906Eu) {
        fprintf(stderr, "fcs16 check value\n");
        return 1;
    }
    n = zdw_serial_frame(out, sizeof(out), 1, 0, in, 3);
    out[n - 1] ^= 0x01u;
    if (zdw_serial_deframe(out, n, 0, 0, back, sizeof(back), &blen, 0) != ZDW_T_ERROR) {
        fprintf(stderr, "a wrong frameCheck was taken\n");
        return 1;
    }
    out[n - 1] ^= 0x01u;
    out[6] = 0x7E;
    if (zdw_serial_deframe(out, n, 0, 0, back, sizeof(back), &blen, 0) != ZDW_T_ERROR) {
        fprintf(stderr, "a 0x7E inside the frame was taken\n");
        return 1;
    }
    printf("fcs16 0x906E, check and flag refusals ok\n");
    return 0;
}

int main(int argc, char **argv)
{
    sensor_reading tx, rx;
    unsigned char body[256], xframe[512], sframe[600], golden[600];
    unsigned char deframed[512];
    const unsigned char *rbody = 0;
    size_t blen, xlen, slen, dlen = 0, bl2 = 0, gn;
    zdw_writer w;
    zdw_reader r;
    FILE *gf;

    if (argc < 2) { fprintf(stderr, "usage: %s <golden_serial_le.bin>\n", argv[0]); return 2; }
    if (stuffing_selfcheck() != 0 || integrity_selfcheck() != 0) { return 1; }

    /* endpoint: sample -> XRCE frame -> serial (HDLC) frame */
    fill(&tx);
    zdw_writer_init(&w, body, sizeof(body), ZDW_LE);
    if (sensor_encode(&w, &tx) != ZDW_OK) { fprintf(stderr, "encode\n"); return 1; }
    blen = w.len;
    xlen = zdw_xrce_write_frame(xframe, sizeof(xframe),
                                ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT,
                                1, ZDW_XRCE_OBJECT_ID(0x001, ZDW_OBJK_DATAWRITER), body, blen);
    if (xlen == 0) { fprintf(stderr, "xrce frame\n"); return 1; }
    slen = zdw_serial_frame(sframe, sizeof(sframe), 1, 0, xframe, xlen);
    if (slen == 0) { fprintf(stderr, "serial frame\n"); return 1; }

    gf = fopen(argv[1], "rb");
    if (gf == NULL) { fprintf(stderr, "open golden\n"); return 1; }
    gn = fread(golden, 1, sizeof(golden), gf);
    fclose(gf);
    if (slen != gn || memcmp(sframe, golden, gn) != 0) {
        fprintf(stderr, "serial frame mismatch: C=%lu golden=%lu\n",
                (unsigned long)slen, (unsigned long)gn);
        return 1;
    }
    printf("XRCE serial frame %lu bytes byte-identical to crates/xrce\n",
           (unsigned long)slen);

    /* receiver: deframe (CRC-checked) -> unwrap WRITE_DATA -> decode */
    if (zdw_serial_deframe(sframe, slen, 0, 0, deframed, sizeof(deframed), &dlen, 0)
        != ZDW_T_OK) {
        fprintf(stderr, "deframe/crc\n"); return 1;
    }
    if (dlen != xlen || memcmp(deframed, xframe, xlen) != 0) {
        fprintf(stderr, "deframed payload mismatch\n"); return 1;
    }
    if (zdw_xrce_read_frame(deframed, dlen, &rbody, &bl2) != ZDW_T_OK) {
        fprintf(stderr, "unwrap\n"); return 1;
    }
    memset(&rx, 0, sizeof(rx));
    zdw_reader_init(&r, rbody, bl2, ZDW_LE);
    if (sensor_decode(&r, &rx) != ZDW_OK) { fprintf(stderr, "decode\n"); return 1; }
    if (rx.id != tx.id || strcmp(rx.label, tx.label) != 0) {
        fprintf(stderr, "round-trip mismatch\n"); return 1;
    }
    printf("serial round-trip: deframe + crc + unwrap + decode ok\n");
    printf("ALL OK\n");
    return 0;
}
