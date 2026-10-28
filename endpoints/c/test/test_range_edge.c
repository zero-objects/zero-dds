/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Range-edge test for the C89 wire-core -- every operation at the ends of
 * its range, not only in the middle, which is where the last four defects
 * in these bindings were found.
 *
 * endpoints/c was the one binding without this test, and endpoints/ada is
 * `Interfaces.C` imports over this code with no wire code of its own, so
 * what is pinned here is inherited by the Ada column too.
 *
 * The refusals are the point. The argument types are C's and wider than
 * the wire width they name, so `zdw_put_u16(w, 70000)` compiles; writing
 * 0x1170 -- the low sixteen bits -- is the defect class every other
 * binding's range test exists to stop (`putU16(70000)` wrote `7011`), and
 * this file pins that the C core answers ZDW_E_RANGE and writes nothing.
 *
 * No goldens: the expected octets are XTypes 1.3 section 7.4.1.1.1 (widths,
 * alignment), section 7.4.3.4.2 (EMHEADER, 28-bit member id) and section
 * 7.4.3.5 (DHEADER), spelled as hex the way the sibling surface tests
 * spell them.
 */

#include <float.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>

#include "zerodds_wire.h"

static int failures = 0;

static void ok(const char *name)
{
    printf("%s: ok\n", name);
}

static void fail(const char *name, const char *what)
{
    fprintf(stderr, "%s: FAIL (%s)\n", name, what);
    failures++;
}

/* Compares the writer's octets against a hex string. */
static void expect_hex(const char *name, const zdw_writer *w, const char *hex)
{
    static const char digits[] = "0123456789abcdef";
    char got[513];
    size_t i;
    if (w->error != ZDW_OK) {
        fail(name, "writer error where octets were expected");
        return;
    }
    if (w->len * 2u >= sizeof(got)) {
        fail(name, "test buffer too small");
        return;
    }
    for (i = 0; i < w->len; i++) {
        got[i * 2u] = digits[(w->buf[i] >> 4) & 0x0F];
        got[i * 2u + 1u] = digits[w->buf[i] & 0x0F];
    }
    got[w->len * 2u] = '\0';
    if (strcmp(got, hex) != 0) {
        fprintf(stderr, "%s: got %s want %s\n", name, got, hex);
        fail(name, "octets differ");
        return;
    }
    ok(name);
}

/* Pins a refusal: the call answered `code`, the error is sticky and the
 * write cursor did not move. */
static void expect_refused(const char *name, zdw_writer *w, int returned,
                           int code, size_t len_before)
{
    if (returned != code) {
        fail(name, "the call did not return the refusal code");
        return;
    }
    if (w->error != code) {
        fail(name, "the refusal is not sticky");
        return;
    }
    if (w->len != len_before) {
        fail(name, "a refused value moved the write cursor");
        return;
    }
    ok(name);
}

int main(void)
{
    unsigned char buf[256];
    zdw_writer w;

    /* --- u16: both ends, then past the top ---------------------------- */

    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    zdw_put_u16(&w, 0u);
    zdw_put_u16(&w, 0xFFFFu);
    expect_hex("u16 at both ends", &w, "0000ffff");

    zdw_writer_init(&w, buf, sizeof(buf), ZDW_BE);
    zdw_put_u16(&w, 0xFFFEu);
    expect_hex("u16 be keeps the stream order", &w, "fffe");

#if UINT_MAX > 0xFFFFu
    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    expect_refused("u16 one past the top is refused", &w,
                   zdw_put_u16(&w, 0x10000u), ZDW_E_RANGE, 0u);

    /* The named defect: 70000 truncated to 0x1170 went on the wire. */
    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    expect_refused("u16 of 70000 is refused, not 0x1170", &w,
                   zdw_put_u16(&w, 70000u), ZDW_E_RANGE, 0u);
#endif

    /* --- u32 / the u64 halves ----------------------------------------- */

    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    zdw_put_u32(&w, 0uL);
    zdw_put_u32(&w, 0xFFFFFFFFuL);
    expect_hex("u32 at both ends", &w, "00000000ffffffff");

#if ULONG_MAX > 0xFFFFFFFFUL
    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    expect_refused("u32 one past the top is refused", &w,
                   zdw_put_u32(&w, 0xFFFFFFFFuL + 1uL), ZDW_E_RANGE, 0u);

    {
        zdw_u64_t v;
        v.hi = 0xFFFFFFFFuL + 1uL;
        v.lo = 0uL;
        zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
        expect_refused("u64 with a wide high half is refused", &w,
                       zdw_put_u64(&w, v), ZDW_E_RANGE, 0u);
    }
#endif

    {
        zdw_u64_t v;
        v.hi = 0xFFFFFFFFuL;
        v.lo = 0xFFFFFFFFuL;
        zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
        zdw_put_u64(&w, v);
        expect_hex("u64 at the top", &w, "ffffffffffffffff");
    }

    /* --- the error is sticky ------------------------------------------ */

#if UINT_MAX > 0xFFFFu
    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    zdw_put_u16(&w, 0x10000u);
    if (zdw_put_u8(&w, 0xAB) == ZDW_E_RANGE && w.len == 0u) {
        ok("a refusal blocks every later write");
    } else {
        fail("a refusal blocks every later write", "the writer went on");
    }
#endif

    /* --- f64: the bit pattern at the ends of the exponent range ------- */

    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    zdw_put_f64(&w, 3.141592653589793);
    expect_hex("f64 pi bit pattern", &w, "182d4454fb210940");

    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    zdw_put_f64(&w, 0.0);
    zdw_put_f64(&w, -0.0);
    expect_hex("f64 zero and negative zero are distinct", &w,
               "0000000000000000" "0000000000000080");

    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    zdw_put_f64(&w, DBL_MAX);
    zdw_put_f64(&w, DBL_MIN);
    expect_hex("f64 at both ends of the exponent range", &w,
               "ffffffffffffef7f" "0000000000001000");

    /* --- the alignment cap -------------------------------------------- */

    /* XCDR2 caps alignment at four: one octet, then a 64-bit value takes
     * three octets of padding and not seven. */
    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    zdw_put_u8(&w, 0xAB);
    zdw_put_u64(&w, zdw_u64_from_ul(0x01020304uL));
    expect_hex("xcdr2 caps a u64 at four", &w, "ab000000" "0403020100000000");

    /* Under XCDR1 the cap is eight and the same pair takes seven. */
    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    w.max_align = ZDW_XCDR1_MAX_ALIGN;
    zdw_put_u8(&w, 0xAB);
    zdw_put_u64(&w, zdw_u64_from_ul(0x01020304uL));
    expect_hex("xcdr1 aligns a u64 to eight", &w,
               "ab00000000000000" "0403020100000000");

    /* --- DHEADER: counts the body and not itself ----------------------- */

    {
        size_t body;
        zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
        body = zdw_dheader_begin(&w);
        zdw_put_u8(&w, 1);
        zdw_put_u8(&w, 2);
        zdw_put_u8(&w, 3);
        zdw_dheader_end(&w, body);
        expect_hex("dheader counts the body only", &w, "03000000" "010203");

        zdw_writer_init(&w, buf, sizeof(buf), ZDW_BE);
        body = zdw_dheader_begin(&w);
        zdw_dheader_end(&w, body);
        expect_hex("an empty dheader body is zero", &w, "00000000");
    }

    /* --- EMHEADER: the top of the 28-bit id, and one past it ----------- */

    {
        size_t body;
        zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
        body = zdw_emheader_begin(&w, 0x0FFFFFFFuL, 0);
        zdw_put_u32(&w, 0uL);
        zdw_emheader_end(&w, body);
        /* (LC4 << 28) | 0x0FFFFFFF = 0x4FFFFFFF, then NEXTINT = 4. */
        expect_hex("member id 2^28 - 1 fits", &w,
                   "ffffff4f" "04000000" "00000000");

        zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
        (void)zdw_emheader_begin(&w, 0x10000000uL, 0);
        expect_refused("a 29-bit member id is refused", &w,
                       w.error, ZDW_E_RANGE, 0u);

        zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
        body = zdw_emheader_begin(&w, 0uL, 1);
        zdw_put_u32(&w, 0uL);
        zdw_emheader_end(&w, body);
        /* must-understand is bit 31: 0x80000000 | 0x40000000 = 0xC0000000. */
        expect_hex("must-understand is bit thirty-one", &w,
                   "000000c0" "04000000" "00000000");
    }

    /* --- the capacity edge: exact fit, then one short ------------------ */

    zdw_writer_init(&w, buf, 4u, ZDW_LE);
    zdw_put_u32(&w, 0xA1B2C3D4uL);
    expect_hex("an exact-fit buffer takes the value", &w, "d4c3b2a1");

    zdw_writer_init(&w, buf, 3u, ZDW_LE);
    expect_refused("one octet short is an overrun", &w,
                   zdw_put_u32(&w, 0xA1B2C3D4uL), ZDW_E_OVERrun, 0u);

    /* Padding alone can overrun: after one octet, a u32 in a five-octet
     * buffer has room for its padding but not its value. */
    zdw_writer_init(&w, buf, 5u, ZDW_LE);
    zdw_put_u8(&w, 0xAB);
    if (zdw_put_u32(&w, 0uL) == ZDW_E_OVERrun && w.error == ZDW_E_OVERrun) {
        ok("padding plus value past the cap is an overrun");
    } else {
        fail("padding plus value past the cap is an overrun",
             "the writer accepted it");
    }

    /* --- strings: the ends, and the reader's refusals ------------------ */

    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    zdw_put_string(&w, "abc");
    expect_hex("a string counts its terminator", &w, "04000000" "61626300");

    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    zdw_put_string(&w, "");
    expect_hex("an empty string is one octet long", &w, "01000000" "00");

    {
        zdw_reader r;
        char out[8];
        unsigned char stream[16];

        /* length 0 breaks the "includes the NUL" invariant. */
        memset(stream, 0, sizeof(stream));
        zdw_reader_init(&r, stream, 4u, ZDW_LE);
        if (zdw_get_string(&r, out, sizeof(out)) == ZDW_E_INVALID) {
            ok("a zero-length string is invalid");
        } else {
            fail("a zero-length string is invalid", "the reader accepted it");
        }

        /* the announced body must end in NUL. */
        stream[0] = 2; stream[1] = 0; stream[2] = 0; stream[3] = 0;
        stream[4] = 'a'; stream[5] = 'b';
        zdw_reader_init(&r, stream, 6u, ZDW_LE);
        if (zdw_get_string(&r, out, sizeof(out)) == ZDW_E_INVALID) {
            ok("a missing terminator is invalid");
        } else {
            fail("a missing terminator is invalid", "the reader accepted it");
        }

        /* an announced length past the caller's buffer is an overrun,
         * never a partial copy. */
        stream[0] = 9; stream[4] = 'a';
        zdw_reader_init(&r, stream, sizeof(stream), ZDW_LE);
        if (zdw_get_string(&r, out, 4u) == ZDW_E_OVERrun) {
            ok("a string past the caller's buffer is an overrun");
        } else {
            fail("a string past the caller's buffer is an overrun",
                 "the reader accepted it");
        }

        /* an announced length past the stream is an overrun. */
        stream[0] = 9; stream[1] = 0; stream[2] = 0; stream[3] = 0;
        zdw_reader_init(&r, stream, 6u, ZDW_LE);
        if (zdw_get_string(&r, out, sizeof(out)) == ZDW_E_OVERrun) {
            ok("a string past the stream end is an overrun");
        } else {
            fail("a string past the stream end is an overrun",
                 "the reader accepted it");
        }
    }

    /* --- sequence<octet>: the announced length is reported ------------- */

    {
        zdw_reader r;
        unsigned char out[4];
        size_t n = 0;
        unsigned char stream[8];
        stream[0] = 9; stream[1] = 0; stream[2] = 0; stream[3] = 0;
        zdw_reader_init(&r, stream, sizeof(stream), ZDW_LE);
        if (zdw_get_seq_u8(&r, out, sizeof(out), &n) == ZDW_E_OVERrun
            && n == 9u) {
            ok("a sequence past the cap is an overrun and reports its length");
        } else {
            fail("a sequence past the cap is an overrun and reports its length",
                 "the reader accepted it or lost the length");
        }
    }

    /* --- bool: every nonzero is one octet of one ----------------------- */

    zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
    zdw_put_bool(&w, 2);
    zdw_put_bool(&w, 0);
    expect_hex("bool normalises to one and zero", &w, "0100");

    /* --- the reader round-trips the ends through both byte orders ------ */

    {
        zdw_reader r;
        unsigned int u16v = 0;
        unsigned long u32v = 0;
        zdw_u64_t u64v;
        int endian;
        u64v.hi = 0; u64v.lo = 0;
        for (endian = ZDW_LE; endian <= ZDW_BE; endian++) {
            zdw_u64_t top;
            top.hi = 0xFFFFFFFFuL;
            top.lo = 0xFFFFFFFEuL;
            zdw_writer_init(&w, buf, sizeof(buf), endian);
            zdw_put_u16(&w, 0xFFFFu);
            zdw_put_u32(&w, 0xFFFFFFFFuL);
            zdw_put_u64(&w, top);
            zdw_reader_init(&r, buf, w.len, endian);
            if (zdw_get_u16(&r, &u16v) == ZDW_OK
                && zdw_get_u32(&r, &u32v) == ZDW_OK
                && zdw_get_u64(&r, &u64v) == ZDW_OK
                && u16v == 0xFFFFu && u32v == 0xFFFFFFFFuL
                && u64v.hi == 0xFFFFFFFFuL && u64v.lo == 0xFFFFFFFEuL) {
                ok(endian == ZDW_LE ? "the top values round-trip in le"
                                    : "the top values round-trip in be");
            } else {
                fail("round-trip at the top", "a value came back different");
            }
        }
    }

    /* --- a truncated stream refuses instead of inventing octets -------- */

    {
        zdw_reader r;
        zdw_u64_t v;
        v.hi = 0; v.lo = 0;
        zdw_writer_init(&w, buf, sizeof(buf), ZDW_LE);
        zdw_put_u32(&w, 0xA1B2C3D4uL);
        zdw_reader_init(&r, buf, 3u, ZDW_LE);
        if (zdw_get_u64(&r, &v) == ZDW_E_OVERrun && r.error == ZDW_E_OVERrun) {
            ok("a truncated stream is an overrun");
        } else {
            fail("a truncated stream is an overrun", "the reader read past it");
        }
    }

    if (failures != 0) {
        fprintf(stderr, "%d range edges FAILED\n", failures);
        return 1;
    }
    printf("ALL OK\n");
    return 0;
}
