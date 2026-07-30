/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Reliable stream state machine (ADR 0013) -- unit tests mirroring the
 * crates/xrce reliable.rs reference, plus HEARTBEAT/ACKNACK byte-golden.
 * usage: test_reliable_sm <golden_heartbeat_le.bin> <golden_acknack_le.bin>
 */
#include <stdio.h>
#include <string.h>
#include "zerodds_endpoint.h"

static int failures = 0;
#define CHECK(cond, msg) do { \
    if (!(cond)) { fprintf(stderr, "FAIL: %s\n", msg); failures++; } } while (0)

/* ~40 KiB each -- keep off the stack. */
static zdw_reliable R;
static zdw_reliable SND;
static zdw_reliable RCV;

static void t_submit_monotonic(void)
{
    unsigned short s0 = 99, s1 = 99;
    unsigned char p[2];
    zdw_reliable_init(&R, 0x80);
    p[0] = 1; p[1] = 2;
    CHECK(zdw_reliable_submit(&R, p, 2, &s0) == ZDW_REL_OK, "submit0 ok");
    CHECK(zdw_reliable_submit(&R, p, 2, &s1) == ZDW_REL_OK, "submit1 ok");
    CHECK(s0 == 0 && s1 == 1, "monotonic seqnrs 0,1");
    CHECK(zdw_reliable_in_flight_count(&R) == 2, "in_flight==2");
}

static void t_submit_too_large(void)
{
    static unsigned char big[ZDW_REL_SLOT_CAP + 1];
    zdw_reliable_init(&R, 0x80);
    CHECK(zdw_reliable_submit(&R, big, sizeof(big), 0) == ZDW_REL_TOO_LARGE,
          "submit rejects oversized payload");
}

static void t_submit_window_full(void)
{
    unsigned char p = 0;
    int i;
    zdw_reliable_init(&R, 0x80);
    for (i = 0; i < ZDW_REL_WINDOW; i++) {
        CHECK(zdw_reliable_submit(&R, &p, 1, 0) == ZDW_REL_OK, "fill window");
    }
    CHECK(zdw_reliable_submit(&R, &p, 1, 0) == ZDW_REL_WINDOW_FULL,
          "submit rejects when window full");
}

static void t_heartbeat(void)
{
    unsigned char p = 1;
    int first = -1, last = -1;
    zdw_reliable_init(&R, 0x80);
    CHECK(zdw_reliable_pending_heartbeat(&R, 0, &first, &last) == 0,
          "no heartbeat when window empty");
    zdw_reliable_submit(&R, &p, 1, 0);
    CHECK(zdw_reliable_pending_heartbeat(&R, 0, &first, &last) == 1,
          "heartbeat fires first time");
    CHECK(first == 0 && last == 0, "heartbeat first=last=0");
    CHECK(zdw_reliable_pending_heartbeat(&R, 100, &first, &last) == 0,
          "heartbeat silenced before 500ms");
    CHECK(zdw_reliable_pending_heartbeat(&R, 600, &first, &last) == 1,
          "heartbeat fires again after 500ms");
}

/* RFC-1982 regression: HEARTBEAT window + loss recovery across the 16-bit wrap
 * (mirrors crates/xrce's wrap regression tests). Window 0xFFFE,0xFFFF,0,1 -> the
 * old numeric min/max reported first=0/last=0xFFFF; correct is first=0xFFFE,
 * last=0x0001. */
static void t_wrap_heartbeat_and_recovery(void)
{
    unsigned char out[8];
    unsigned short seq, q0, q1, q2, q3;
    size_t len, pl_len;
    const unsigned char *pl;
    int first = -1, last = -1;
    unsigned short bm;
    long k;
    unsigned char b;

    /* --- sender HEARTBEAT window across the wrap --- */
    zdw_reliable_init(&SND, 0x80);
    SND.next_seq = 0xFFFE; /* seed just below the wrap */
    b = 10; zdw_reliable_submit(&SND, &b, 1, &q0); /* 0xFFFE */
    b = 11; zdw_reliable_submit(&SND, &b, 1, &q1); /* 0xFFFF (lost below) */
    b = 12; zdw_reliable_submit(&SND, &b, 1, &q2); /* 0x0000 */
    b = 13; zdw_reliable_submit(&SND, &b, 1, &q3); /* 0x0001 */
    CHECK(q0 == 0xFFFE && q1 == 0xFFFF && q2 == 0x0000 && q3 == 0x0001,
          "wrap seqs 0xFFFE..0x0001");
    CHECK(zdw_reliable_pending_heartbeat(&SND, 0, &first, &last) == 1, "wrap heartbeat fires");
    CHECK((unsigned short)first == 0xFFFE && (unsigned short)last == 0x0001,
          "heartbeat window across wrap = [0xFFFE,0x0001] (not numeric 0,0xFFFF)");

    /* --- receiver ACKNACK + retransmit across the wrap --- */
    zdw_reliable_init(&RCV, 0x80);
    RCV.expected_seq = 0xFFFE; /* seed receiver just below the wrap */
    b = 10; zdw_reliable_recv_data(&RCV, q0, &b, 1); /* 0xFFFF lost */
    b = 12; zdw_reliable_recv_data(&RCV, q2, &b, 1);
    b = 13; zdw_reliable_recv_data(&RCV, q3, &b, 1);
    k = 0;
    while (zdw_reliable_drain(&RCV, out, sizeof(out), &seq, &len)) { k++; }
    CHECK(k == 1, "only 0xFFFE delivered before recovery");
    CHECK(RCV.expected_seq == 0xFFFF, "receiver blocked at 0xFFFF");

    bm = zdw_reliable_pending_acknack(&RCV); /* base = expected = 0xFFFF */
    CHECK((bm & 0x1u) != 0u, "0xFFFF NACKed");
    CHECK((bm & 0x6u) == 0u, "0x0000/0x0001 present");

    zdw_reliable_recv_acknack(&SND, (int)0xFFFF, bm);
    CHECK(zdw_reliable_get_in_flight(&SND, q1, &pl_len) != 0, "0xFFFF retransmittable");
    CHECK(zdw_reliable_get_in_flight(&SND, q0, 0) == 0, "0xFFFE acked");
    CHECK(zdw_reliable_in_flight_count(&SND) == 1, "only 0xFFFF left in-flight");

    pl = zdw_reliable_get_in_flight(&SND, q1, &pl_len);
    if (pl != 0) { zdw_reliable_recv_data(&RCV, q1, pl, pl_len); }
    k = 0;
    while (zdw_reliable_drain(&RCV, out, sizeof(out), &seq, &len)) { k++; }
    CHECK(k == 3, "0xFFFF,0x0000,0x0001 deliver in RFC-1982 order after retransmit");
}

static void t_recv_acknack_clears(void)
{
    unsigned char p = 0;
    zdw_reliable_init(&R, 0x80);
    zdw_reliable_submit(&R, &p, 1, 0); /* seq 0 */
    zdw_reliable_submit(&R, &p, 1, 0); /* seq 1 */
    zdw_reliable_submit(&R, &p, 1, 0); /* seq 2 */
    CHECK(zdw_reliable_in_flight_count(&R) == 3, "3 in flight");
    /* base=2, bitmap=0x0001 -> seq2 missing, seq0/1 acked */
    zdw_reliable_recv_acknack(&R, 2, 0x0001);
    CHECK(zdw_reliable_in_flight_count(&R) == 1, "acknack clears 0,1");
    CHECK(zdw_reliable_get_in_flight(&R, 2, 0) != 0, "seq2 retransmittable");
}

static void t_recv_acknack_full_clear(void)
{
    unsigned char p = 0;
    int i;
    zdw_reliable_init(&R, 0x80);
    for (i = 0; i < 5; i++) { zdw_reliable_submit(&R, &p, 1, 0); }
    zdw_reliable_recv_acknack(&R, 5, 0x0000); /* base=5, bitmap 0 -> all acked */
    CHECK(zdw_reliable_in_flight_count(&R) == 0, "acknack full clear");
}

static void t_recv_in_order(void)
{
    unsigned char out[8], p0 = 10, p1 = 11;
    unsigned short seq = 0;
    size_t len = 0;
    zdw_reliable_init(&R, 0x80);
    zdw_reliable_recv_data(&R, 0, &p0, 1);
    zdw_reliable_recv_data(&R, 1, &p1, 1);
    CHECK(zdw_reliable_drain(&R, out, sizeof(out), &seq, &len) == 1 && seq == 0, "drain 0");
    CHECK(zdw_reliable_drain(&R, out, sizeof(out), &seq, &len) == 1 && seq == 1, "drain 1");
    CHECK(zdw_reliable_drain(&R, out, sizeof(out), &seq, &len) == 0, "drain empty");
    CHECK(zdw_reliable_expected(&R) == 2, "expected==2");
}

static void t_recv_reorder(void)
{
    unsigned char out[8], p = 0;
    unsigned short seq = 0;
    size_t len = 0;
    int n;
    zdw_reliable_init(&R, 0x80);
    zdw_reliable_recv_data(&R, 2, &p, 1);
    zdw_reliable_recv_data(&R, 0, &p, 1);
    n = 0;
    while (zdw_reliable_drain(&R, out, sizeof(out), &seq, &len)) { n++; }
    CHECK(n == 1, "reorder: only seq0 deliverable");
    zdw_reliable_recv_data(&R, 1, &p, 1);
    n = 0;
    while (zdw_reliable_drain(&R, out, sizeof(out), &seq, &len)) { n++; }
    CHECK(n == 2, "reorder: seq1+seq2 after gap filled");
}

static void t_recv_duplicate(void)
{
    unsigned char out[8], p = 1;
    unsigned short seq = 0;
    size_t len = 0;
    zdw_reliable_init(&R, 0x80);
    zdw_reliable_recv_data(&R, 0, &p, 1);
    zdw_reliable_drain(&R, out, sizeof(out), &seq, &len);
    zdw_reliable_recv_data(&R, 0, &p, 1); /* < expected -> dropped */
    CHECK(zdw_reliable_out_of_order_count(&R) == 0, "duplicate dropped");
}

static void t_recv_buffer_full(void)
{
    unsigned char p = 1;
    int i, rc = ZDW_REL_OK;
    zdw_reliable_init(&R, 0x80);
    /* fill 64 OOO slots (seq 1..64, expected stays 0) */
    for (i = 1; i <= ZDW_REL_RECV_BUF; i++) {
        rc = zdw_reliable_recv_data(&R, (unsigned short)i, &p, 1);
        CHECK(rc == ZDW_REL_OK, "fill recv buffer");
    }
    rc = zdw_reliable_recv_data(&R, (unsigned short)(ZDW_REL_RECV_BUF + 1), &p, 1);
    CHECK(rc == ZDW_REL_BUF_FULL, "recv buffer full rejected");
}

static void t_pending_acknack(void)
{
    unsigned char p = 1;
    unsigned short bm;
    zdw_reliable_init(&R, 0x80);
    /* expected=0; receive seq1 + seq3 -> 0 and 2 missing */
    zdw_reliable_recv_data(&R, 1, &p, 1);
    zdw_reliable_recv_data(&R, 3, &p, 1);
    bm = zdw_reliable_pending_acknack(&R);
    CHECK((bm & (1u << 0)) != 0, "seq0 missing");
    CHECK((bm & (1u << 2)) != 0, "seq2 missing");
    CHECK((bm & (1u << 1)) == 0, "seq1 present");
    CHECK((bm & (1u << 3)) == 0, "seq3 present");
}

static void t_reset(void)
{
    unsigned char p = 1;
    zdw_reliable_init(&R, 0x80);
    zdw_reliable_submit(&R, &p, 1, 0);
    zdw_reliable_recv_data(&R, 0, &p, 1);
    zdw_reliable_reset(&R);
    CHECK(zdw_reliable_in_flight_count(&R) == 0, "reset clears in_flight");
    CHECK(zdw_reliable_out_of_order_count(&R) == 0, "reset clears recv");
    CHECK(zdw_reliable_expected(&R) == 0, "reset clears expected");
}

/* Full sender<->receiver loss-recovery walk in-process (no network). */
static void t_end_to_end_loss_recovery(void)
{
    unsigned char out[8], sample[4];
    unsigned short seq = 0;
    size_t len = 0, i;
    int delivered = 0, first, last, guard;
    zdw_reliable_init(&SND, 0x80);
    zdw_reliable_init(&RCV, 0x80);
    /* submit 5, "send" all but drop seq 2 on the first pass */
    for (i = 0; i < 5; i++) {
        sample[0] = (unsigned char)i; sample[1] = sample[2] = sample[3] = 0;
        zdw_reliable_submit(&SND, sample, 4, 0);
        if (i != 2) {
            zdw_reliable_recv_data(&RCV, (unsigned short)i, sample, 4);
        }
    }
    while (zdw_reliable_drain(&RCV, out, sizeof(out), &seq, &len)) { delivered++; }
    CHECK(delivered == 2, "only seq0,1 before recovery");
    /* receiver ACKNACKs, sender recovers + retransmits set bits */
    (void)zdw_reliable_pending_heartbeat(&SND, 0, &first, &last);
    {
        unsigned short bm = zdw_reliable_pending_acknack(&RCV);
        unsigned short base = zdw_reliable_expected(&RCV);
        unsigned short b;
        zdw_reliable_recv_acknack(&SND, (int)base, bm);
        for (b = 0; b < 16; b++) {
            if ((bm >> b) & 1u) {
                unsigned short s = (unsigned short)(base + b);
                size_t l = 0;
                const unsigned char *pl = zdw_reliable_get_in_flight(&SND, s, &l);
                if (pl != 0) { zdw_reliable_recv_data(&RCV, s, pl, l); }
            }
        }
    }
    guard = 0;
    while (zdw_reliable_drain(&RCV, out, sizeof(out), &seq, &len)) {
        delivered++;
        if (++guard > 100) { break; }
    }
    CHECK(delivered == 5, "all 5 delivered gap-free after retransmit");
    CHECK(zdw_reliable_expected(&RCV) == 5, "expected advanced to 5");
}

static long rd(const char *p, unsigned char *b, long c)
{
    FILE *f = fopen(p, "rb");
    long n;
    if (!f) { fprintf(stderr, "open %s\n", p); return -1; }
    n = (long)fread(b, 1, (size_t)c, f); fclose(f); return n;
}

static void t_byte_golden(const char *hb_path, const char *an_path)
{
    unsigned char gold[64], mine[64];
    long gn;
    int first = 0, last = 0, fu = 0;
    unsigned int nack = 0;
    unsigned char stream = 0, session, hstream, seqlo, seqhi;
    size_t mlen;

    /* HEARTBEAT: parse the golden, rebuild it, expect byte-identity. */
    gn = rd(hb_path, gold, (long)sizeof(gold));
    CHECK(gn == 13, "heartbeat golden is 13 bytes");
    if (gn == 13) {
        session = gold[0]; hstream = gold[1]; seqlo = gold[2]; seqhi = gold[3];
        CHECK(zdw_xrce_heartbeat_read(gold, (size_t)gn, &first, &last, &stream) == ZDW_T_OK,
              "parse golden heartbeat");
        mlen = zdw_xrce_heartbeat_frame(mine, sizeof(mine), session, hstream,
                                        (unsigned int)seqlo | ((unsigned int)seqhi << 8),
                                        first, last, stream);
        CHECK(mlen == 13 && memcmp(mine, gold, 13) == 0,
              "heartbeat build byte-identical to golden");
    }

    /* ACKNACK: parse the golden, rebuild it, expect byte-identity. */
    gn = rd(an_path, gold, (long)sizeof(gold));
    CHECK(gn == 13, "acknack golden is 13 bytes");
    if (gn == 13) {
        session = gold[0]; hstream = gold[1]; seqlo = gold[2]; seqhi = gold[3];
        CHECK(zdw_xrce_acknack_read(gold, (size_t)gn, &fu, &nack, &stream) == ZDW_T_OK,
              "parse golden acknack");
        mlen = zdw_xrce_acknack_frame(mine, sizeof(mine), session, hstream,
                                      (unsigned int)seqlo | ((unsigned int)seqhi << 8),
                                      fu, (unsigned char)(nack & 0xff),
                                      (unsigned char)((nack >> 8) & 0xff), stream);
        CHECK(mlen == 13 && memcmp(mine, gold, 13) == 0,
              "acknack build byte-identical to golden");
    }
}

int main(int argc, char **argv)
{
    t_submit_monotonic();
    t_submit_too_large();
    t_submit_window_full();
    t_heartbeat();
    t_wrap_heartbeat_and_recovery();
    t_recv_acknack_clears();
    t_recv_acknack_full_clear();
    t_recv_in_order();
    t_recv_reorder();
    t_recv_duplicate();
    t_recv_buffer_full();
    t_pending_acknack();
    t_reset();
    t_end_to_end_loss_recovery();
    if (argc >= 3) {
        t_byte_golden(argv[1], argv[2]);
    } else {
        fprintf(stderr, "note: no goldens given, skipping byte-golden checks\n");
    }
    if (failures == 0) {
        printf("test_reliable_sm: ALL OK\n");
        return 0;
    }
    fprintf(stderr, "test_reliable_sm: %d failure(s)\n", failures);
    return 1;
}
