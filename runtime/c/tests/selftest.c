/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The rules the C runtime would be wrong about silently.
 *
 * Every expectation here is a byte string, not a property: a property
 * test over an encoder can pass while the encoder and the specification
 * disagree, because the property was written from the same reading.
 * The values below come from `crates/ast-layout/tests/fixtures/
 * vendor_bytes.md` where a vendor measured them, and from the clause
 * otherwise, with the clause named.
 */
#include "zerodds_wire.h"

#include <stdio.h>
#include <string.h>

static int failures = 0;

static void hex(const uint8_t *bytes, size_t length, char *out) {
    size_t i;
    for (i = 0; i < length; i++) {
        sprintf(out + i * 2, "%02x", bytes[i]);
    }
    out[length * 2] = 0;
}

static void expect(const char *what, const zdw_writer *w,
                   const char *expected) {
    char got[512];
    if (zdw_writer_status(w) != ZDW_OK) {
        printf("FAIL %s: %s\n", what, zdw_status_name(zdw_writer_status(w)));
        failures++;
        return;
    }
    hex(w->out, zdw_writer_length(w), got);
    if (strcmp(got, expected) != 0) {
        printf("FAIL %s\n  expected %s\n  got      %s\n", what, expected, got);
        failures++;
    }
}

static void expect_status(const char *what, const zdw_writer *w,
                          zdw_status expected) {
    if (zdw_writer_status(w) != expected) {
        printf("FAIL %s: expected %s, got %s\n", what,
               zdw_status_name(expected), zdw_status_name(zdw_writer_status(w)));
        failures++;
    }
}

#define WRITER(name, endian, maxalign)                                         \
    uint8_t name##_buf[4096];                                                  \
    zdw_writer name;                                                           \
    zdw_writer_init(&name, name##_buf, sizeof name##_buf, endian, maxalign)

/* §7.4.1: MAXALIGN is 4 under XCDR2 and 8 under XCDR1. Measured on
 * Cyclone, RTI and Fast DDS — `@final struct Point { long x; double y; }`
 * is twelve bytes under XCDR2 and sixteen under XCDR1. */
static void max_alignment_caps_the_padding(void) {
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_i32(&w, 1);
        zdw_write_f64(&w, 2.0);
        expect("XCDR2 caps alignment at four", &w, "010000000000000000000040");
    }
    {
        WRITER(w, ZDW_LITTLE, 8);
        zdw_write_i32(&w, 1);
        zdw_write_f64(&w, 2.0);
        expect("XCDR1 aligns a double to eight", &w,
               "01000000000000000000000000000040");
    }
}

/* §7.4.1.1: the length counts the terminator. */
static void a_string_counts_its_terminator(void) {
    WRITER(w, ZDW_LITTLE, 4);
    zdw_write_string(&w, "abc");
    expect("a string is length+1, bytes, NUL", &w, "0400000061626300");
}

/* §7.4.1.1.2: the length is a byte count of UTF-16 units, no BOM and no
 * terminating NUL. A code point outside the BMP is two units, which is
 * the case a character count gets wrong. */
static void a_wstring_counts_utf16_bytes(void) {
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wstring_utf8(&w, "\xc3\xa4Z", ZDW_WSTRING_SPECIFICATION,
                               ZDW_WSTRING_UNITS_STREAM);
        expect("a wstring is twice its unit count", &w, "04000000e4005a00");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        /* U+1D11E, which UTF-16 spells as a surrogate pair. */
        zdw_write_wstring_utf8(&w, "\xf0\x9d\x84\x9e", ZDW_WSTRING_SPECIFICATION,
                               ZDW_WSTRING_UNITS_STREAM);
        expect("a supplementary code point is four bytes", &w,
               "0400000034d81edd");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wstring_utf8(&w, "", ZDW_WSTRING_SPECIFICATION,
                               ZDW_WSTRING_UNITS_STREAM);
        expect("an empty wstring is a zero length", &w, "00000000");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wstring_utf8(&w, "\xc3", ZDW_WSTRING_SPECIFICATION,
                               ZDW_WSTRING_UNITS_STREAM);
        expect_status("malformed UTF-8 is refused", &w, ZDW_NOT_UTF8);
    }
}

/* §7.4.1.2: a parameter value is its own CDR block, so alignment inside
 * it starts at the block. Without that reset a `double` took four bytes
 * of padding inside its own declared length — `01000c00` where Cyclone,
 * RTI and Fast DDS all write `01000800`. */
static void a_pl_cdr1_parameter_counts_alignment_from_its_own_start(void) {
    WRITER(w, ZDW_LITTLE, 8);
    zdw_write_i16(&w, 1);
    {
        zdw_pid pid = zdw_write_pid(&w, 1, ZDW_PARAM_MEMBER, false);
        zdw_write_f64(&w, 2.0);
        zdw_finish_pid(&w, pid);
    }
    expect("a parameter block realigns to its own start", &w,
           "0100" "0000" "01000800" "0000000000000040");
}

/* §7.4.3.4.2: codes 5 to 7 reuse the member's own leading length word,
 * so no NEXTINT placeholder is reserved. Reserving one writes the length
 * twice and puts every following member four bytes late. */
static void length_code_five_reuses_the_members_own_length(void) {
    WRITER(w, ZDW_LITTLE, 4);
    {
        zdw_emheader h = zdw_write_emheader(&w, 3, 5, false);
        zdw_write_string(&w, "ab");
        zdw_finish_emheader(&w, h);
    }
    expect("LC5 writes no NEXTINT of its own", &w,
           "03000050" "03000000" "616200");
    {
        WRITER(v, ZDW_LITTLE, 4);
        zdw_emheader h = zdw_write_emheader(&v, 3, 4, false);
        zdw_write_string(&v, "ab");
        zdw_finish_emheader(&v, h);
        expect("LC4 patches an explicit NEXTINT", &v,
               "03000040" "07000000" "03000000" "616200");
    }
}

/* §7.4.3.5.3 rule (5) with the ENUM_TYPE holder: signed, so `@value(-1)`
 * at the default bound is `ffffffff`. RTI Connext 7.7 writes exactly
 * that; Cyclone refuses the annotation, so RTI is the only oracle. */
static void an_enumerator_is_written_into_a_signed_holder(void) {
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_enum(&w, 32, -1);
        expect("a negative enumerator is two's complement", &w, "ffffffff");
    }
    {
        /* Cyclone takes `@value(200)` under `@bit_bound(8)` and writes
         * `c8`; the accepted range is therefore both signednesses. */
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_enum(&w, 8, 200);
        expect("an unsigned-looking enumerator fits its bound", &w, "c8");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_enum(&w, 8, 256);
        expect_status("a value past the bound is refused", &w, ZDW_RANGE);
    }
}

/* The two length conventions the field writes beside the clause's, from
 * `zerodds_ast_layout::WstringLengthPolicy`. Reproducing a vendor's
 * length convention is a policy; reproducing its data loss is not, so
 * the character-count form refuses a code point it cannot carry. */
static void the_other_two_wstring_conventions(void) {
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wstring_utf8(&w, "\xc3\xa4Z", ZDW_WSTRING_CHARACTER_COUNT,
                               ZDW_WSTRING_UNITS_STREAM);
        expect("Fast DDS counts characters, not bytes", &w, "02000000e4005a00");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wstring_utf8(&w, "\xf0\x9d\x84\x9e", ZDW_WSTRING_CHARACTER_COUNT,
                               ZDW_WSTRING_UNITS_STREAM);
        expect_status("a supplementary code point has no character-count form", &w,
                      ZDW_RANGE);
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wstring_utf8(&w, "\xc3\xa4Z", ZDW_WSTRING_CHARACTER_COUNT_TERMINATED,
                               ZDW_WSTRING_UNITS_STREAM);
        expect("RTI's XCDR1 form counts the terminator", &w,
               "03000000" "e4000000" "5a000000" "00000000");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wstring_utf8(&w, "\xf0\x9d\x84\x9e",
                               ZDW_WSTRING_CHARACTER_COUNT_TERMINATED,
                               ZDW_WSTRING_UNITS_STREAM);
        expect("a surrogate pair keeps both units", &w,
               "03000000" "34d80000" "1edd0000" "00000000");
    }
    {
        /* Cyclone's big-endian form: the length word swaps and the code
         * units do not. Measured 2026-08-09 through
         * `/root/byte-gate/gate-be`, case `wstring_member`, `w = "ab"`:
         * Cyclone writes `00000004 61006200` where RTI and Fast DDS
         * write `00000004 00610062`. A conforming peer reads Cyclone's
         * as U+6100 U+6200 and has nothing to object to. */
        WRITER(w, ZDW_BIG, 4);
        zdw_write_wstring_utf8(&w, "ab", ZDW_WSTRING_SPECIFICATION,
                               ZDW_WSTRING_UNITS_HOST_UNSWAPPED);
        expect("Cyclone does not swap a wstring's code units", &w,
               "00000004" "61006200");
    }
    {
        /* In little endian it is the specification's form byte for
         * byte, which is why it took a big-endian run to find. */
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wstring_utf8(&w, "ab", ZDW_WSTRING_SPECIFICATION,
                               ZDW_WSTRING_UNITS_HOST_UNSWAPPED);
        expect("little endian tells the two forms apart in nothing", &w,
               "04000000" "61006200");
    }
}

/* The three parts of a PL_CDR1 dialect that are the writer's rather
 * than the plan's. `zerodds_ast_layout::PlCdr1HeaderPolicy` carries the
 * measurement. */
static void the_pl_cdr1_framing_is_selectable(void) {
    {
        /* Cyclone: PID_EXTENDED and PID_LIST_END both must-understand. */
        WRITER(w, ZDW_LITTLE, 8);
        zdw_pid pid;
        zdw_writer_set_pl_cdr1(&w, zdw_pl_cdr1_default());
        pid = zdw_write_pid_extended(&w, 1, ZDW_PARAM_MEMBER, false);
        zdw_write_i32(&w, 1);
        zdw_finish_pid(&w, pid);
        zdw_write_sentinel(&w);
        expect("Cyclone sets the must-understand bit on both", &w,
               "017f0800" "01000000" "04000000" "01000000" "027f0000");
    }
    {
        /* RTI: the declared length covers the trailing padding. */
        WRITER(w, ZDW_LITTLE, 8);
        zdw_pid pid;
        zdw_pl_cdr1_framing rti = zdw_pl_cdr1_default();
        rti.pid_flags = ZDW_PID_FLAGS_SENTINEL_MUST_UNDERSTAND;
        rti.pad_declared_length = true;
        zdw_writer_set_pl_cdr1(&w, rti);
        pid = zdw_write_pid(&w, 1, ZDW_PARAM_MEMBER, false);
        zdw_write_i16(&w, 2);
        zdw_finish_pid(&w, pid);
        expect("RTI counts the padding into the length", &w, "01000400" "02000000");
    }
    {
        /* The clause: the value's own unpadded length. */
        WRITER(w, ZDW_LITTLE, 8);
        zdw_pid pid;
        zdw_pl_cdr1_framing spec = zdw_pl_cdr1_default();
        spec.pid_flags = ZDW_PID_FLAGS_BARE;
        spec.pad_declared_length = false;
        zdw_writer_set_pl_cdr1(&w, spec);
        pid = zdw_write_pid(&w, 1, ZDW_PARAM_MEMBER, false);
        zdw_write_i16(&w, 2);
        zdw_finish_pid(&w, pid);
        zdw_write_sentinel(&w);
        expect("the clause leaves the padding out of the length", &w,
               "01000200" "02000000" "023f0000");
    }
}

/* [CDR] §9.3.2.8. The sign nibble is `0xc` for zero, because there is
 * only one zero to write. */
static void a_fixed_is_packed_bcd_with_a_sign_nibble(void) {
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_fixed(&w, "123.45", 9, 3, ZDW_FIXED_DECLARED);
        expect("fixed<9,3> pads both halves", &w, "000123450c");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_fixed(&w, "-0.5", 9, 3, ZDW_FIXED_DECLARED);
        expect("a negative value takes 0xd", &w, "000000500d");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_fixed(&w, "-0.0", 9, 3, ZDW_FIXED_DECLARED);
        expect("a negative zero is written positive", &w, "000000000c");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_fixed(&w, "1234567.0", 9, 3, ZDW_FIXED_DECLARED);
        expect_status("a value that overruns its digits is refused", &w,
                      ZDW_BAD_DECIMAL);
    }
}

/* §7.2.2.2.1.2.2: a `CHAR_8_TYPE` "shall be representable using 8
 * bits", and §7.2.2.2.1.2.5 restricts a `CHAR_16_TYPE` to the Basic
 * Multilingual Plane. Writing the low bits would put a different
 * character on the wire with nothing said. */
static void a_character_that_does_not_fit_is_refused(void) {
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_char(&w, 0x161);
        expect_status("U+0161 has no char", &w, ZDW_RANGE);
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wchar(&w, 0x1f600, ZDW_WCHAR_WIDTH_2, ZDW_WCHAR_FIXED,
                        ZDW_WSTRING_UNITS_STREAM);
        expect_status("U+1F600 has no wchar", &w, ZDW_RANGE);
    }
    {
        /* RTI Connext writes a four-byte `wchar` under XCDR1, the same
         * width its XCDR1 `wstring` characters take. Case
         * `wchar_member` on 2026-08-09: `61000000` where Cyclone and
         * Fast DDS write `6100`. */
        WRITER(w, ZDW_LITTLE, 8);
        zdw_write_wchar(&w, 0x0061, ZDW_WCHAR_WIDTH_4, ZDW_WCHAR_FIXED,
                        ZDW_WSTRING_UNITS_STREAM);
        expect("an RTI XCDR1 wchar is four bytes", &w, "61000000");
    }
}

/* The buffer is the caller's, and a short one is an error rather than a
 * shorter sample. */
static void a_short_buffer_fails_rather_than_truncates(void) {
    uint8_t small[6];
    zdw_writer w;
    zdw_writer_init(&w, small, sizeof small, ZDW_LITTLE, 4);
    zdw_write_i32(&w, 1);
    zdw_write_i32(&w, 2);
    expect_status("a short buffer is loud", &w, ZDW_SHORT_BUFFER);
}

/* RFC 1321's own test vectors. */
static void the_digest_is_md5(void) {
    struct {
        const char *input;
        const char *expected;
    } vectors[] = {
        {"", "d41d8cd98f00b204e9800998ecf8427e"},
        {"abc", "900150983cd24fb0d6963f7d28e17f72"},
        {"message digest", "f96b697d7cb7938d525a2f31aaf161d0"},
        {"abcdefghijklmnopqrstuvwxyz", "c3fcd3d76192e4007dfb496cca67e13b"},
        {"12345678901234567890123456789012345678901234567890123456789012345"
         "678901234567890",
         "57edf4a22be3c955ac49da2e2107b67a"}};
    size_t i;
    for (i = 0; i < sizeof vectors / sizeof vectors[0]; i++) {
        uint8_t out[16];
        char got[33];
        zdw_md5((const uint8_t *)vectors[i].input, strlen(vectors[i].input),
                out);
        hex(out, 16, got);
        if (strcmp(got, vectors[i].expected) != 0) {
            printf("FAIL md5(%s)\n  expected %s\n  got      %s\n",
                   vectors[i].input, vectors[i].expected, got);
            failures++;
        }
    }
}

/* The reader is the writer's mirror, so a value that survives one and
 * not the other is a difference between them. */
static void what_the_writer_wrote_the_reader_reads(void) {
    WRITER(w, ZDW_BIG, 4);
    char text[16];
    zdw_reader r;
    zdw_write_i32(&w, -1234567);
    zdw_write_string(&w, "über");
    zdw_write_f64(&w, 3.5);
    zdw_write_enum(&w, 16, -3);
    if (zdw_writer_status(&w) != ZDW_OK) {
        printf("FAIL round trip: the write failed\n");
        failures++;
        return;
    }
    zdw_reader_init(&r, w.out, zdw_writer_length(&w), ZDW_BIG, 4);
    if (zdw_read_i32(&r) != -1234567) {
        printf("FAIL round trip: the long\n");
        failures++;
    }
    (void)zdw_read_string(&r, text, sizeof text);
    if (strcmp(text, "über") != 0) {
        printf("FAIL round trip: the string\n");
        failures++;
    }
    if (zdw_read_f64(&r) != 3.5) {
        printf("FAIL round trip: the double\n");
        failures++;
    }
    if (zdw_read_enum(&r, 16) != -3) {
        printf("FAIL round trip: the enumerator\n");
        failures++;
    }
    if (zdw_reader_status(&r) != ZDW_OK) {
        printf("FAIL round trip: %s\n", zdw_status_name(zdw_reader_status(&r)));
        failures++;
    }
}

/* §7.4.3.4.2: the word after an EMHEADER of code 5, 6 or 7 is the
 * member's own leading length. A reader that consumes it starts the
 * member one word late and reads whatever follows. */
static void an_emheader_of_code_five_leaves_the_length_word(void) {
    WRITER(w, ZDW_LITTLE, 4);
    zdw_reader r;
    zdw_emheader_read header;
    char text[8];
    {
        zdw_emheader h = zdw_write_emheader(&w, 3, 5, false);
        zdw_write_string(&w, "ab");
        zdw_finish_emheader(&w, h);
    }
    zdw_reader_init(&r, w.out, zdw_writer_length(&w), ZDW_LITTLE, 4);
    header = zdw_read_emheader(&r);
    if (header.length_code != 5 || header.next_int != 3) {
        printf("FAIL the LC5 header reads wrong: code=%u next=%u\n",
               (unsigned)header.length_code, (unsigned)header.next_int);
        failures++;
        return;
    }
    (void)zdw_read_string(&r, text, sizeof text);
    if (zdw_reader_status(&r) != ZDW_OK || strcmp(text, "ab") != 0) {
        printf("FAIL the LC5 member did not read back: %s\n",
               zdw_status_name(zdw_reader_status(&r)));
        failures++;
    }
}

/* -- the read direction --------------------------------------------------
 *
 * The expectations below are the reader's own, not the writer's played
 * backwards. Three of them are places where the two were written apart
 * and only one of them was right.
 */

#define READER(name, bytes, endian, maxalign)                                  \
    zdw_reader name;                                                           \
    uint8_t name##_heap[4096];                                                 \
    zdw_reader_init(&name, (bytes), sizeof(bytes), endian, maxalign);          \
    zdw_reader_set_heap(&name, name##_heap, sizeof name##_heap)

static void expect_read_status(const char *what, const zdw_reader *r,
                               zdw_status expected) {
    if (zdw_reader_status(r) != expected) {
        printf("FAIL %s: expected %s, got %s\n", what,
               zdw_status_name(expected),
               zdw_status_name(zdw_reader_status(r)));
        failures++;
    }
}

static void expect_text(const char *what, const char *got,
                        const char *expected) {
    if (got == NULL || strcmp(got, expected) != 0) {
        printf("FAIL %s\n  expected %s\n  got      %s\n", what, expected,
               got == NULL ? "(nothing)" : got);
        failures++;
    }
}

/* §7.4.1.2 makes a parameter value "a CDR-encapsulated block of data",
 * so alignment inside one counts from the value's own start. The writer
 * was given that and the reader was not: an eight-byte member directly
 * behind an eight-byte header took four bytes of padding that no vendor
 * writes, the double came out of the following bytes, and every member
 * after it was read at the wrong offset with nothing to object to.
 * `vendor_bytes.md` §2 records the header as `01000800`. */
static void a_parameter_value_counts_alignment_from_its_own_start(void) {
    static const uint8_t bytes[] = {
        0x01, 0x00, 0x08, 0x00,                         /* id 1, 8 bytes */
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf8, 0x3f, /* 1.5 */
        0x02, 0x7f, 0x00, 0x00                          /* PID_LIST_END */
    };
    READER(r, bytes, ZDW_LITTLE, 8);
    zdw_pid_read header = zdw_read_pid(&r);
    double value = zdw_read_f64(&r);
    zdw_finish_pid_read(&r, header);
    if (header.pid != 1u || value != 1.5) {
        printf("FAIL the parameter's double: pid=%u value=%f\n",
               (unsigned)header.pid, value);
        failures++;
    }
    expect_read_status("a parameter value is its own block", &r, ZDW_OK);
    if (!zdw_read_pid(&r).end) {
        printf("FAIL the sentinel did not close the list\n");
        failures++;
    }
}

/* §7.4.1.2 splits the sixteen-bit field: the id is the low fourteen
 * bits, bit 14 is must-understand. Cyclone sets it on every parameter,
 * so its PID_EXTENDED goes out as `7f01` and its sentinel as `7f02` —
 * two of the three implementations this project is measured against.
 * A reader comparing the whole word takes the extended header's id and
 * length for eight bytes of value and runs past the end of the list. */
static void the_must_understand_bit_is_not_part_of_the_id(void) {
    static const uint8_t bytes[] = {
        0x01, 0x7f, 0x08, 0x00,                         /* PID_EXTENDED */
        0x09, 0x00, 0x00, 0x00,                         /* member id 9 */
        0x04, 0x00, 0x00, 0x00,                         /* 4 bytes */
        0x2a, 0x00, 0x00, 0x00,                         /* 42 */
        0x02, 0x7f, 0x00, 0x00                          /* PID_LIST_END */
    };
    READER(r, bytes, ZDW_LITTLE, 8);
    zdw_pid_read header = zdw_read_pid(&r);
    int32_t value;
    if (header.pid != 9u || header.length != 4u) {
        printf("FAIL the extended header: id=%u length=%u\n",
               (unsigned)header.pid, (unsigned)header.length);
        failures++;
        return;
    }
    value = zdw_read_i32(&r);
    zdw_finish_pid_read(&r, header);
    if (value != 42) {
        printf("FAIL the extended parameter's value: %d\n", value);
        failures++;
    }
    if (!zdw_read_pid(&r).end) {
        printf("FAIL the must-understand sentinel was not recognised\n");
        failures++;
    }
    expect_read_status("Cyclone's flagged markers are read", &r, ZDW_OK);
}

/* §7.4.1.2.2 gives bit 15 to FLAG_IMPL_EXTENSION and says a user-defined
 * member "will never set" it; it reserves 0x3F01-0x3FFF; and it says of
 * 0x3F03 that "all consumers of this Data Representation shall ignore
 * parameters with this ID". A reader that masked all three off with the
 * rest of the flags read a foreign implementation's private parameter
 * as the member with the same low fourteen bits. */
static void a_vendor_private_or_reserved_parameter_is_not_a_member(void) {
    static const uint8_t bytes[] = {
        0x02, 0x80, 0x04, 0x00,                         /* id 2, FLAG_IMPL_EXTENSION */
        0xef, 0xbe, 0xad, 0xde,
        0x03, 0x7f, 0x04, 0x00,                         /* PID_IGNORE, must-understand */
        0x11, 0x22, 0x33, 0x44,
        0x0c, 0x3f, 0x04, 0x00,                         /* a reserved id */
        0x55, 0x66, 0x77, 0x88,
        0x02, 0x00, 0x04, 0x00,                         /* member id 2 */
        0x2a, 0x00, 0x00, 0x00,
        0x02, 0x7f, 0x00, 0x00                          /* PID_LIST_END */
    };
    READER(r, bytes, ZDW_LITTLE, 8);
    zdw_pid_read a = zdw_read_pid(&r);
    zdw_pid_read b;
    zdw_pid_read c;
    zdw_pid_read d;
    if (!a.impl_extension || zdw_pid_is_user_member(a)) {
        printf("FAIL FLAG_IMPL_EXTENSION was read as a member: pid=%u\n",
               (unsigned)a.pid);
        failures++;
    }
    zdw_reader_skip_to(&r, 4u + (size_t)a.length);
    zdw_finish_pid_read(&r, a);

    b = zdw_read_pid(&r);
    if (!b.ignore || zdw_pid_is_user_member(b) || !b.must_understand) {
        printf("FAIL PID_IGNORE: ignore=%d user=%d mu=%d\n", (int)b.ignore,
               (int)zdw_pid_is_user_member(b), (int)b.must_understand);
        failures++;
    }
    zdw_reader_skip_to(&r, zdw_reader_position(&r) + (size_t)b.length);
    zdw_finish_pid_read(&r, b);

    c = zdw_read_pid(&r);
    if (!c.reserved || zdw_pid_is_user_member(c)) {
        printf("FAIL a reserved id was read as a member: pid=%u\n",
               (unsigned)c.pid);
        failures++;
    }
    zdw_reader_skip_to(&r, zdw_reader_position(&r) + (size_t)c.length);
    zdw_finish_pid_read(&r, c);

    d = zdw_read_pid(&r);
    if (!zdw_pid_is_user_member(d) || d.pid != 2u) {
        printf("FAIL the real member id 2: pid=%u user=%d\n", (unsigned)d.pid,
               (int)zdw_pid_is_user_member(d));
        failures++;
    }
    if (zdw_read_i32(&r) != 42) {
        printf("FAIL the real member's value\n");
        failures++;
    }
    zdw_finish_pid_read(&r, d);
    if (!zdw_read_pid(&r).end) {
        printf("FAIL the sentinel did not close the list\n");
        failures++;
    }
    expect_read_status("a vendor-private parameter is not a member", &r, ZDW_OK);
}

/* A reader configured for one peer and given another's bytes has to say
 * which, and say it apart from "this stream is malformed". */
static void the_wrong_dialect_is_a_status_of_its_own(void) {
    {
        /* Fast DDS's exact length where the configured framing rounds
         * up. Well formed, and the wrong setting. */
        static const uint8_t bytes[] = {
            0x01, 0x00, 0x02, 0x00, 0x02, 0x00, 0x00, 0x00
        };
        zdw_pl_cdr1_framing rti = zdw_pl_cdr1_default();
        READER(r, bytes, ZDW_LITTLE, 8);
        rti.pid_flags = ZDW_PID_FLAGS_SENTINEL_MUST_UNDERSTAND;
        rti.pad_declared_length = true;
        zdw_reader_set_pl_cdr1(&r, rti);
        {
            zdw_pid_read header = zdw_read_pid(&r);
            (void)zdw_read_i16(&r);
            zdw_finish_pid_read(&r, header);
        }
        expect_read_status("a length from the other convention", &r,
                           ZDW_WRONG_FRAMING);
        expect_text("the setting the bytes fit", zdw_reader_fits(&r),
                    "--pl-cdr1-headers short-true-length");
    }
    {
        /* An odd byte count is not a byte count. The character-count
         * form is the one that could have written it. */
        static const uint8_t bytes[] = {
            0x03, 0x00, 0x00, 0x00, 0x61, 0x00, 0x62, 0x00, 0x63, 0x00
        };
        READER(r, bytes, ZDW_LITTLE, 4);
        (void)zdw_read_wstring_utf8(&r, ZDW_WSTRING_SPECIFICATION, ZDW_WSTRING_UNITS_STREAM);
        expect_read_status("an odd wstring byte count", &r, ZDW_WRONG_FRAMING);
        expect_text("the wstring setting the bytes fit", zdw_reader_fits(&r),
                    "--wstring-length character-count");
    }
    {
        /* A boolean of 2 is the other thing: no setting produces it. */
        static const uint8_t bytes[] = { 0x02 };
        READER(r, bytes, ZDW_LITTLE, 4);
        (void)zdw_read_bool(&r);
        expect_read_status("a malformed value is not a framing mismatch", &r,
                           ZDW_OUT_OF_RANGE);
        if (zdw_reader_fits(&r) != NULL) {
            printf("FAIL a malformed stream named a setting\n");
            failures++;
        }
        if (zdw_reader_error_offset(&r) != 1u) {
            printf("FAIL the offset of the refusal: %u\n",
                   (unsigned)zdw_reader_error_offset(&r));
            failures++;
        }
    }
}

/* Each `wstring` convention read back into the UTF-8 the binding holds.
 * Written against the clause and the two measurements rather than by
 * inverting `zdw_write_wstring_utf8`, so that a mistake in one shows up
 * as a disagreement rather than as a round trip that closes. */
static void each_wstring_convention_reads_back(void) {
    {
        /* §7.4.1.1.2: the count is bytes, "twice the number of
         * characters", and a supplementary code point is a pair. */
        static const uint8_t bytes[] = {
            0x08, 0x00, 0x00, 0x00, 0xe4, 0x00, 0x5a, 0x00,
            0x34, 0xd8, 0x1e, 0xdd
        };
        READER(r, bytes, ZDW_LITTLE, 4);
        expect_text("the specification form",
                    zdw_read_wstring_utf8(&r, ZDW_WSTRING_SPECIFICATION, ZDW_WSTRING_UNITS_STREAM),
                    "\xc3\xa4Z\xf0\x9d\x84\x9e");
        expect_read_status("the specification form", &r, ZDW_OK);
    }
    {
        static const uint8_t bytes[] = {
            0x02, 0x00, 0x00, 0x00, 0xe4, 0x00, 0x5a, 0x00
        };
        READER(r, bytes, ZDW_LITTLE, 4);
        expect_text("Fast DDS's character count",
                    zdw_read_wstring_utf8(&r, ZDW_WSTRING_CHARACTER_COUNT, ZDW_WSTRING_UNITS_STREAM),
                    "\xc3\xa4Z");
        expect_read_status("Fast DDS's character count", &r, ZDW_OK);
    }
    {
        /* Cyclone's big-endian bytes, read the two ways. Under the
         * specification's form they are two well-formed characters
         * nobody sent; under the form that names them they are "ab". */
        static const uint8_t bytes[] = {
            0x00, 0x00, 0x00, 0x04, 0x61, 0x00, 0x62, 0x00
        };
        {
            READER(r, bytes, ZDW_BIG, 4);
            expect_text("a conforming read of Cyclone's big-endian wstring",
                        zdw_read_wstring_utf8(&r, ZDW_WSTRING_SPECIFICATION, ZDW_WSTRING_UNITS_STREAM),
                        "\xe6\x84\x80\xe6\x88\x80");
            expect_read_status("a conforming read of Cyclone's wstring", &r,
                               ZDW_OK);
        }
        {
            READER(r, bytes, ZDW_BIG, 4);
            expect_text("the unswapped form reads Cyclone's own string",
                        zdw_read_wstring_utf8(&r, ZDW_WSTRING_SPECIFICATION,
                                              ZDW_WSTRING_UNITS_HOST_UNSWAPPED),
                        "ab");
            expect_read_status("the unswapped form", &r, ZDW_OK);
        }
    }
    {
        static const uint8_t bytes[] = {
            0x03, 0x00, 0x00, 0x00, 0xe4, 0x00, 0x00, 0x00,
            0x5a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        };
        READER(r, bytes, ZDW_LITTLE, 4);
        expect_text("RTI's terminated form",
                    zdw_read_wstring_utf8(&r,
                                          ZDW_WSTRING_CHARACTER_COUNT_TERMINATED,
                                          ZDW_WSTRING_UNITS_STREAM),
                    "\xc3\xa4Z");
        expect_read_status("RTI's terminated form", &r, ZDW_OK);
    }
    {
        /* A NUL inside the text is where a C peer stops, so the value
         * this reader returns and the value that peer returns would
         * differ for bytes both accepted. */
        static const uint8_t bytes[] = {
            0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x5a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        };
        READER(r, bytes, ZDW_LITTLE, 4);
        (void)zdw_read_wstring_utf8(&r, ZDW_WSTRING_CHARACTER_COUNT_TERMINATED, ZDW_WSTRING_UNITS_STREAM);
        expect_read_status("an embedded NUL", &r, ZDW_OUT_OF_RANGE);
    }
    {
        /* An unpaired surrogate is refused rather than replaced: U+FFFD
         * in its place is a character the sender never had. */
        static const uint8_t bytes[] = {
            0x02, 0x00, 0x00, 0x00, 0x34, 0xd8
        };
        READER(r, bytes, ZDW_LITTLE, 4);
        (void)zdw_read_wstring_utf8(&r, ZDW_WSTRING_SPECIFICATION, ZDW_WSTRING_UNITS_STREAM);
        expect_read_status("an unpaired surrogate", &r, ZDW_OUT_OF_RANGE);
    }
}

/* [CDR] §9.3.2.8 read back into the one decimal the type has: no
 * leading zeros beyond a single one, exactly `S` digits after the
 * point, and a `-` only on a non-zero negative. */
static void a_fixed_reads_back_as_its_canonical_decimal(void) {
    {
        static const uint8_t bytes[] = { 0x00, 0x01, 0x23, 0x45, 0x0c };
        READER(r, bytes, ZDW_LITTLE, 4);
        expect_text("fixed<9,3>", zdw_read_fixed(&r, 9, 3, ZDW_FIXED_DECLARED), "123.450");
    }
    {
        static const uint8_t bytes[] = { 0x00, 0x00, 0x00, 0x50, 0x0d };
        READER(r, bytes, ZDW_LITTLE, 4);
        expect_text("a negative fraction", zdw_read_fixed(&r, 9, 3, ZDW_FIXED_DECLARED), "-0.500");
    }
    {
        static const uint8_t bytes[] = { 0x00, 0x00, 0x00, 0x00, 0x0c };
        READER(r, bytes, ZDW_LITTLE, 4);
        expect_text("one zero", zdw_read_fixed(&r, 9, 3, ZDW_FIXED_DECLARED), "0.000");
    }
    {
        /* `fixed<4,4>` is all fraction, so the integer part is the
         * single zero the canonical form writes. */
        static const uint8_t bytes[] = { 0x01, 0x23, 0x4c };
        READER(r, bytes, ZDW_LITTLE, 4);
        expect_text("scale equal to the digits", zdw_read_fixed(&r, 4, 4, ZDW_FIXED_DECLARED),
                    "0.1234");
    }
    {
        static const uint8_t bytes[] = { 0x00, 0x0a, 0xbc, 0x00, 0x0c };
        READER(r, bytes, ZDW_LITTLE, 4);
        (void)zdw_read_fixed(&r, 9, 3, ZDW_FIXED_DECLARED);
        expect_read_status("a nibble that is not a digit", &r,
                           ZDW_BAD_DECIMAL);
    }
}

/* The other count: as many digits as the value has, and never fewer
 * than one. What JacORB and ACE/TAO write, measured on the recorded
 * `fixed-basic`, `array-1d-fixed` and `sequence-fixed` rows.
 *
 * The read half scans for the sign nibble, which is what makes the form
 * decodable at all — a digit is `0` to `9` and a sign is `0xa` upwards,
 * and the nibble count is even, so the field ends at the first octet
 * whose low nibble is a sign. */
static void a_fixed_can_carry_the_values_digits_instead(void) {
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_fixed(&w, "-000000.001", 9, 3, ZDW_FIXED_VALUE_PRECISION);
        expect("one digit and the sign", &w, "1d");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_fixed(&w, "123456.789", 9, 3, ZDW_FIXED_VALUE_PRECISION);
        expect("nine digits is the declared field", &w, "123456789c");
    }
    {
        /* `BigDecimal("0.00").precision()` is 1: a zero is one digit
         * and the sign, not none. */
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_fixed(&w, "0.000", 9, 3, ZDW_FIXED_VALUE_PRECISION);
        expect("a zero is still one digit", &w, "0c");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_fixed(&w, "-12345.678", 9, 3, ZDW_FIXED_VALUE_PRECISION);
        expect("eight digits pads to nine nibbles", &w, "012345678d");
    }
    {
        static const uint8_t bytes[] = { 0x1d };
        READER(r, bytes, ZDW_LITTLE, 4);
        expect_text("one digit, the scale on the right",
                    zdw_read_fixed(&r, 9, 3, ZDW_FIXED_VALUE_PRECISION),
                    "-0.001");
    }
    {
        static const uint8_t bytes[] = { 0x0c };
        READER(r, bytes, ZDW_LITTLE, 4);
        expect_text("a single zero digit",
                    zdw_read_fixed(&r, 9, 3, ZDW_FIXED_VALUE_PRECISION),
                    "0.000");
    }
    {
        static const uint8_t bytes[] = { 0x01, 0x23, 0x45, 0x67, 0x8d };
        READER(r, bytes, ZDW_LITTLE, 4);
        expect_text("the pad nibble is not a digit",
                    zdw_read_fixed(&r, 9, 3, ZDW_FIXED_VALUE_PRECISION),
                    "-12345.678");
    }
    {
        /* Seventeen octets of digits and no sign: refused rather than
         * read past the widest `fixed` IDL admits. */
        static const uint8_t bytes[] = { 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
                                         0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
                                         0x11, 0x11, 0x11, 0x11, 0x11 };
        READER(r, bytes, ZDW_LITTLE, 4);
        (void)zdw_read_fixed(&r, 31, 0, ZDW_FIXED_VALUE_PRECISION);
        expect_read_status("a field with no sign nibble", &r,
                           ZDW_BAD_DECIMAL);
    }
}

/* The third count: the integer digits of the decimal as written plus S, made odd by one
 * leading zero. What ACE/TAO 2.5.24's `ACE_CDR::Fixed::from_string`
 * writes, recorded in
 * `crates/ast-layout/tests/fixtures/unspecified_constructs.md` §3. */
static void a_fixed_can_carry_the_digits_of_its_string(void) {
    static const struct {
        const char *text;
        uint8_t digits;
        uint8_t scale;
        const char *octets;
        const char *back;
    } recorded[] = {
        { "123.45", 5, 2, "12345c", "123.45" },
        { "-123.45", 5, 2, "12345d", "-123.45" },
        { "12.34", 4, 2, "01234c", "12.34" },
        { "7", 1, 0, "7c", "7" },
        { "0.12345", 5, 5, "0012345c", "0.12345" },
        { "0.00", 5, 2, "000c", "0.00" },
        /* The scale and not the fraction as written: one digit counted
         * would keep only the last nibble, `0c`. */
        { "7", 5, 2, "700c", "7.00" },
    };
    size_t i;
    for (i = 0; i < sizeof recorded / sizeof recorded[0]; i++) {
        uint8_t bytes[16];
        size_t n = strlen(recorded[i].octets) / 2u;
        size_t k;
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_fixed(&w, recorded[i].text, recorded[i].digits, recorded[i].scale,
                        ZDW_FIXED_VALUE_STRING_DIGITS);
        expect(recorded[i].text, &w, recorded[i].octets);
        for (k = 0; k < n; k++) {
            unsigned int octet = 0;
            (void)sscanf(recorded[i].octets + 2u * k, "%2x", &octet);
            bytes[k] = (uint8_t)octet;
        }
        {
            zdw_reader r;
            uint8_t heap[4096];
            zdw_reader_init(&r, bytes, n, ZDW_LITTLE, 4);
            zdw_reader_set_heap(&r, heap, sizeof heap);
            expect_text(recorded[i].text,
                        zdw_read_fixed(&r, recorded[i].digits, recorded[i].scale,
                                       ZDW_FIXED_VALUE_STRING_DIGITS),
                        recorded[i].back);
        }
    }
    {
        /* Thirty-two digits as written: no width a reader can take. */
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_fixed(&w, "00000000000000000000000000000001", 31, 0,
                        ZDW_FIXED_VALUE_STRING_DIGITS);
        expect_status("thirty-two digits as written", &w, ZDW_BAD_DECIMAL);
    }
}

/* The heap is the caller's, and one that runs out is an error rather
 * than a shorter value. */
static void a_full_heap_fails_rather_than_shortens(void) {
    static const uint8_t bytes[] = { 0x06, 0x00, 0x00, 0x00,
                                     0x61, 0x62, 0x63, 0x64, 0x65, 0x00 };
    uint8_t heap[4];
    zdw_reader r;
    zdw_reader_init(&r, bytes, sizeof bytes, ZDW_LITTLE, 4);
    zdw_reader_set_heap(&r, heap, sizeof heap);
    if (zdw_read_string_alloc(&r) != NULL) {
        printf("FAIL a short heap returned a string\n");
        failures++;
    }
    expect_read_status("a short heap", &r, ZDW_SHORT_BUFFER);
}

/* An `@optional` member in a type that is not `@mutable` has nothing on
 * the wire marking its absence, so its header is read and put back when
 * the member id is not the expected one. */
static void a_speculative_read_puts_everything_back(void) {
    static const uint8_t bytes[] = { 0x04, 0x00, 0x00, 0x00,
                                     0x61, 0x62, 0x63, 0x00 };
    READER(r, bytes, ZDW_LITTLE, 4);
    zdw_reader_mark mark = zdw_reader_save(&r);
    const char *first = zdw_read_string_alloc(&r);
    size_t used_once = r.heap_used;
    expect_text("the speculative read", first, "abc");
    zdw_reader_restore(&r, mark);
    if (zdw_reader_position(&r) != 0u || r.heap_used != 0u) {
        printf("FAIL the rewind left %u bytes read and %u allocated\n",
               (unsigned)zdw_reader_position(&r), (unsigned)r.heap_used);
        failures++;
    }
    expect_text("the second read", zdw_read_string_alloc(&r), "abc");
    if (r.heap_used != used_once) {
        printf("FAIL the rewound heap did not come back to the same place\n");
        failures++;
    }
}


/* -- binary128 ------------------------------------------------------------
 *
 * The bytes come from three places, none of them this runtime: the
 * pattern for 1.5 was measured from GCC's `__float128` and from RTI
 * Connext 7.7's own serialiser on the Linux host, the layout is CORBA
 * 3.3 part 2 Figure 9.2, and the decimals are what `crates/float128`
 * prints, which is itself checked against libquadmath.
 * `docs/float128-column.md` has the runs.
 */
static void a_long_double_is_sixteen_bytes_of_binary128(void) {
    uint8_t buffer[64];
    zdw_writer w;
    zdw_writer_init(&w, buffer, sizeof buffer, ZDW_LITTLE, 8);
    zdw_write_f128(&w, zdw_f128_from_double(1.5));
    zdw_write_f128(&w, zdw_f128_from_double(-2.75));
    expect("two long doubles, little-endian",
           &w,
           "0000000000000000000000000080ff3f"
           "000000000000000000000000006000c0");
}

static void the_two_byte_orders_are_the_whole_value_reversed(void) {
    uint8_t buffer[64];
    zdw_writer w;
    zdw_writer_init(&w, buffer, sizeof buffer, ZDW_BIG, 8);
    zdw_write_f128(&w, zdw_f128_from_double(1.5));
    expect("1.5 big-endian", &w, "3fff8000000000000000000000000000");
}

static void a_long_double_reads_back_as_it_was_written(void) {
    static const double values[6] = { 1.5, -2.75, 0.0, -0.0, 1e300, 4.9e-324 };
    int i;
    for (i = 0; i < 6; i++) {
        uint8_t buffer[32];
        zdw_writer w;
        zdw_reader r;
        zdw_f128 back;
        zdw_f128 sent = zdw_f128_from_double(values[i]);
        zdw_writer_init(&w, buffer, sizeof buffer, ZDW_BIG, 8);
        zdw_write_f128(&w, sent);
        zdw_reader_init(&r, buffer, zdw_writer_length(&w), ZDW_BIG, 8);
        back = zdw_read_f128(&r);
        if (!zdw_f128_same_bits(back, sent)) {
            printf("FAIL a long double did not read back: %016llx%016llx vs %016llx%016llx\n",
                   (unsigned long long)back.high, (unsigned long long)back.low,
                   (unsigned long long)sent.high, (unsigned long long)sent.low);
            failures++;
        }
    }
}

static void a_value_a_double_cannot_hold_survives_and_is_refused(void) {
    /* 1 + 2^-100 needs 101 significand bits: past a double at 53 and
     * past x87 extended at 64, so it is the value that tells a real
     * binary128 column from a padded one. */
    zdw_f128 wide = zdw_f128_from_parts(0x3fff000000000000ULL, 0x0000000000001000ULL);
    uint8_t buffer[32];
    zdw_writer w;
    zdw_reader r;
    zdw_f128 back;
    double narrowed = 0.0;

    if (zdw_f128_to_double(wide, &narrowed)) {
        printf("FAIL 1 + 2^-100 claimed to fit a double\n");
        failures++;
    }
    if (narrowed != 1.0) {
        printf("FAIL the rounded value was not 1.0\n");
        failures++;
    }
    zdw_writer_init(&w, buffer, sizeof buffer, ZDW_LITTLE, 8);
    zdw_write_f128(&w, wide);
    zdw_reader_init(&r, buffer, zdw_writer_length(&w), ZDW_LITTLE, 8);
    back = zdw_read_f128(&r);
    if (!zdw_f128_same_bits(back, wide)) {
        printf("FAIL a value only binary128 can hold did not survive the wire\n");
        failures++;
    }
}

static void the_platforms_long_double_converts_and_never_copies(void) {
    /* Whatever this platform's `long double` is — 53, 64 or 113
     * significand bits — widening it is exact, and the value comes back
     * unchanged. A runtime that memcpy'd the object would fail this on
     * every platform but one. */
    static const long double values[5] = { 1.5L, -2.75L, 0.0L, 1.0L, 1234.5678L };
    int i;
    for (i = 0; i < 5; i++) {
        zdw_f128 wide = zdw_f128_from_long_double(values[i]);
        long double back = 0.0L;
        if (!zdw_f128_to_long_double(wide, &back)) {
            printf("FAIL a long double did not survive its own round trip at index %d\n", i);
            failures++;
        }
        if (back != values[i]) {
            printf("FAIL a long double changed value at index %d\n", i);
            failures++;
        }
    }
    /* And 1.5 lands on the same sixteen bytes whichever type it came
     * through, which is the whole claim: the wire format does not depend
     * on what the compiler calls `long double`. */
    if (!zdw_f128_same_bits(zdw_f128_from_long_double(1.5L), zdw_f128_from_double(1.5))) {
        printf("FAIL 1.5 differs between the long double and the double path\n");
        failures++;
    }
}

static void expect_decimal(const char *what, zdw_f128 value, const char *expected) {
    char text[ZDW_F128_DECIMAL_MAX];
    size_t length = zdw_f128_to_decimal(value, text, sizeof text);
    if (length == 0u && expected[0] != 0) {
        printf("FAIL %s: the decimal printer refused\n", what);
        failures++;
        return;
    }
    if (strcmp(text, expected) != 0) {
        printf("FAIL %s\n  expected %s\n  got      %s\n", what, expected, text);
        failures++;
    }
}

static void the_exact_decimal_is_the_canonical_form(void) {
    expect_decimal("1.5", zdw_f128_from_double(1.5), "1.5");
    expect_decimal("-2.75", zdw_f128_from_double(-2.75), "-2.75");
    expect_decimal("zero", zdw_f128_zero(), "0.0");
    expect_decimal("negative zero",
                   zdw_f128_from_parts(0x8000000000000000ULL, 0u), "-0.0");
    expect_decimal("one", zdw_f128_one(), "1.0");
    expect_decimal("infinity", zdw_f128_infinity(), "inf");
    expect_decimal("nan", zdw_f128_quiet_nan(), "nan");
    /* 1 + 2^-112, the step above one, which no shorter decimal names. */
    expect_decimal(
        "one plus epsilon",
        zdw_f128_from_parts(0x3fff000000000000ULL, 1u),
        "1.0000000000000000000000000000000001925929944387235853055977942584927"
        "318538101648215388195239938795566558837890625");
}

static void a_short_decimal_buffer_writes_nothing(void) {
    char text[4];
    if (zdw_f128_to_decimal(zdw_f128_from_double(1234.5), text, sizeof text) != 0u) {
        printf("FAIL a short decimal buffer produced a truncated number\n");
        failures++;
    }
}

/* A trim cuts to the bound, keeps the order and never lands inside a
 * character. DDS-XTypes 1.3 Table 11: "The order of the characters in
 * the string or elements in the sequence or map is preserved."
 *
 * The units are what the bound means to a peer that has to hold the
 * value: octets for a `string<N>`, whose C mapping is `char[N + 1]`,
 * and UTF-16 code units for a `wstring<N>`. */
static void a_trim_cuts_on_a_character_boundary(void) {
    static uint8_t heap[256];
    zdw_reader r;
    const char *got;
    zdw_reader_init(&r, heap, 0u, ZDW_LITTLE, 8u);
    zdw_reader_set_heap(&r, heap, sizeof heap);

    if (zdw_utf8_octets("Hello World!") != 12u) {
        printf("FAIL: utf8_octets counted %lu, not 12\n",
               (unsigned long)zdw_utf8_octets("Hello World!"));
        failures++;
    }
    got = zdw_trim_utf8(&r, "Hello World!", 5u);
    if (got == NULL || strcmp(got, "Hello") != 0) {
        printf("FAIL: trim_utf8 gave %s, not Hello\n", got == NULL ? "(null)" : got);
        failures++;
    }

    /* Five two-octet characters against a bound of five octets: the
     * third would need a sixth, so two come back and never one and a
     * half. */
    got = zdw_trim_utf8(&r, "\xc3\xa4\xc3\xb6\xc3\xbc\xc3\xa4\xc3\xb6", 5u);
    if (got == NULL || strcmp(got, "\xc3\xa4\xc3\xb6") != 0) {
        printf("FAIL: trim_utf8 split a character\n");
        failures++;
    }

    /* A scalar above the basic plane is two UTF-16 code units, so it
     * does not fit a bound of one. U+1F600 as UTF-8. */
    if (zdw_utf16_units("\xf0\x9f\x98\x80") != 2u) {
        printf("FAIL: utf16_units did not count a surrogate pair as two\n");
        failures++;
    }
    got = zdw_trim_utf16(&r, "a\xf0\x9f\x98\x80", 2u);
    if (got == NULL || strcmp(got, "a") != 0) {
        printf("FAIL: trim_utf16 split a surrogate pair\n");
        failures++;
    }

    /* A value already inside the bound comes back whole. */
    got = zdw_trim_utf8(&r, "abc", 5u);
    if (got == NULL || strcmp(got, "abc") != 0) {
        printf("FAIL: trim_utf8 shortened a value that fitted\n");
        failures++;
    }
}

/* The discard status is its own, apart from every other. DDS-XTypes 1.3
 * §7.2.2.7: a sample this type cannot hold is dropped, which is not the
 * same thing as a stream this reader cannot parse. */
static void a_discard_is_not_one_of_the_wire_errors(void) {
    if (ZDW_DISCARDED == ZDW_OUT_OF_RANGE || ZDW_DISCARDED == ZDW_WRONG_FRAMING
        || ZDW_DISCARDED == ZDW_TRUNCATED) {
        printf("FAIL: ZDW_DISCARDED shares a value with a wire error\n");
        failures++;
    }
    if (strcmp(zdw_status_name(ZDW_DISCARDED), "an unnamed status") == 0) {
        printf("FAIL: ZDW_DISCARDED has no name\n");
        failures++;
    }
}

/* DDS-XTypes 1.3 §7.5.1.2.3.1.1: an external member that is not optional
 * "is not allowed [...] to be NULL at the time of a write". A generated
 * encoder records it with `zdw_writer_refuse`; the first refusal sticks and
 * the writer writes nothing after it. */
static void an_unset_external_member_is_a_refusal_of_its_own(void) {
    WRITER(w, ZDW_LITTLE, 8);
    zdw_write_u8(&w, 7);
    zdw_writer_refuse(&w, ZDW_OK);
    if (zdw_writer_status(&w) != ZDW_OK) {
        printf("FAIL: refusing with ZDW_OK recorded a failure\n");
        failures++;
    }
    zdw_writer_refuse(&w, ZDW_UNSET_EXTERNAL);
    expect_status("an unset external member", &w, ZDW_UNSET_EXTERNAL);
    zdw_write_u8(&w, 9);
    zdw_writer_refuse(&w, ZDW_RANGE);
    expect_status("the first refusal is the one that sticks", &w,
                  ZDW_UNSET_EXTERNAL);
    if (zdw_writer_length(&w) != 1u) {
        printf("FAIL: the writer went on after the refusal\n");
        failures++;
    }
    if (strcmp(zdw_status_name(ZDW_UNSET_EXTERNAL), "an unnamed status") == 0) {
        printf("FAIL: ZDW_UNSET_EXTERNAL has no name\n");
        failures++;
    }
}

/* The three CORBA ORB forms, in the column that has to write the same
 * bytes as the other two.
 *
 * Measured 2026-08-10 on the Linux test host through
 * `/root/corba-oracle/tools/wireprobe-{omniorb,tao,jacorb}` — omniORB
 * 4.3.3, TAO 2.5.24, JacORB 3.9. CORBA 3.3 part 2 §9.3.1.6 has two
 * wide-character encodings, one per GIOP version, and XTypes Table 31
 * replaced one half of the pair; these are the other half.
 * `crates/wire-probe/tests/corba_wide_characters.rs` is the same run in
 * the Rust column and carries the argument. */
static void the_three_corba_wide_character_forms(void) {
    {
        /* omniORB: a byte-order mark, then the units, both host order —
         * so the length word swaps into the stream and the content does
         * not. `"a<U+1D11E>b"` is ten octets over five units, the mark
         * counted. */
        WRITER(w, ZDW_LITTLE, 8);
        zdw_write_wstring_utf8(&w, "a\xf0\x9d\x84\x9e" "b", ZDW_WSTRING_SPECIFICATION,
                               ZDW_WSTRING_UNITS_BYTE_ORDER_MARK);
        expect("omniORB's wstring carries its mark", &w,
               "0a000000" "fffe" "6100" "34d8" "1edd" "6200");
    }
    {
        /* The same octets in a big-endian stream: only the length word
         * swaps, so the mark now reads as FFFE and says so. */
        WRITER(w, ZDW_BIG, 8);
        zdw_write_wstring_utf8(&w, "a\xf0\x9d\x84\x9e" "b", ZDW_WSTRING_SPECIFICATION,
                               ZDW_WSTRING_UNITS_BYTE_ORDER_MARK);
        expect("omniORB swaps the length and not the mark", &w,
               "0000000a" "fffe" "6100" "34d8" "1edd" "6200");
    }
    {
        /* An empty wstring carries no mark: omniORB writes `00000000`,
         * byte-identical to every other implementation. */
        WRITER(w, ZDW_LITTLE, 8);
        zdw_write_wstring_utf8(&w, "", ZDW_WSTRING_SPECIFICATION,
                               ZDW_WSTRING_UNITS_BYTE_ORDER_MARK);
        expect("an empty wstring carries no mark", &w, "00000000");
    }
    {
        /* TAO: GIOP 1.2's octet count over a four-octet transmission
         * code set — the one form no DDS implementation writes. The
         * units are UTF-16 code units zero-extended, not UCS-4 code
         * points, so the surrogate pair survives. */
        WRITER(w, ZDW_LITTLE, 8);
        zdw_write_wstring_utf8(&w, "a\xf0\x9d\x84\x9e" "b",
                               ZDW_WSTRING_SPECIFICATION_WIDE_UNITS,
                               ZDW_WSTRING_UNITS_STREAM);
        expect("TAO counts octets over four-octet units", &w,
               "10000000" "61000000" "34d80000" "1edd0000" "62000000");
    }
    {
        /* JacORB: UTF-16 big-endian whatever the stream says, no mark.
         * Measured in its read direction — `CDROutputStream` has no
         * `setLittleEndian`, so its writes are big-endian only and the
         * two candidate orders coincide there. */
        WRITER(w, ZDW_LITTLE, 8);
        zdw_write_wstring_utf8(&w, "a\xf0\x9d\x84\x9e" "b", ZDW_WSTRING_SPECIFICATION,
                               ZDW_WSTRING_UNITS_BIG_ENDIAN_ALWAYS);
        expect("JacORB reads the units big-endian whatever the stream is", &w,
               "08000000" "0061" "d834" "dd1e" "0062");
    }
    {
        /* The scalar. All three ORBs write GIOP 1.2's `wchar` — an
         * octet count, then that many octets, aligned to one. omniORB
         * and JacORB write the octets big-endian in a *little-endian*
         * stream, because a scalar cannot carry a mark and CORBA's
         * default without one is big-endian. */
        WRITER(w, ZDW_LITTLE, 8);
        zdw_write_wchar(&w, 0x00e4, ZDW_WCHAR_WIDTH_2,
                        ZDW_WCHAR_GIOP_LENGTH_PREFIXED,
                        ZDW_WSTRING_UNITS_BYTE_ORDER_MARK);
        zdw_write_wchar(&w, 0x1234, ZDW_WCHAR_WIDTH_2,
                        ZDW_WCHAR_GIOP_LENGTH_PREFIXED,
                        ZDW_WSTRING_UNITS_BYTE_ORDER_MARK);
        expect("omniORB's wchar is length-prefixed and big-endian", &w,
               "02" "00e4" "02" "1234");
    }
    {
        /* TAO's is four octets in the stream's order. Aligned to one in
         * both: `zdw_write_u8` before it would otherwise pad. */
        WRITER(w, ZDW_LITTLE, 8);
        zdw_write_wchar(&w, 0x00e4, ZDW_WCHAR_WIDTH_4,
                        ZDW_WCHAR_GIOP_LENGTH_PREFIXED,
                        ZDW_WSTRING_UNITS_STREAM);
        zdw_write_wchar(&w, 0x1234, ZDW_WCHAR_WIDTH_4,
                        ZDW_WCHAR_GIOP_LENGTH_PREFIXED,
                        ZDW_WSTRING_UNITS_STREAM);
        expect("TAO's wchar is four octets in the stream's order", &w,
               "04" "e4000000" "04" "34120000");
    }
    {
        /* The framing aligns to one, which moves every member behind
         * it: an `octet` then a `wchar` is three octets under an ORB and
         * four under Table 31. */
        WRITER(w, ZDW_LITTLE, 8);
        zdw_write_u8(&w, 1);
        zdw_write_wchar(&w, 0x0061, ZDW_WCHAR_WIDTH_2,
                        ZDW_WCHAR_GIOP_LENGTH_PREFIXED,
                        ZDW_WSTRING_UNITS_BYTE_ORDER_MARK);
        zdw_write_i32(&w, 7);
        expect("a length-prefixed wchar aligns to one", &w,
               "01" "02" "0061" "07000000");
    }
    {
        /* And every one of them reads back under its own settings. */
        static const uint8_t bytes[] = {
            0x0a, 0x00, 0x00, 0x00, 0xff, 0xfe, 0x61, 0x00,
            0x34, 0xd8, 0x1e, 0xdd, 0x62, 0x00
        };
        READER(r, bytes, ZDW_LITTLE, 8);
        expect_text("omniORB's own wstring reads back",
                    zdw_read_wstring_utf8(&r, ZDW_WSTRING_SPECIFICATION,
                                          ZDW_WSTRING_UNITS_BYTE_ORDER_MARK),
                    "a\xf0\x9d\x84\x9e" "b");
    }
    {
        /* No mark at all means UTF-16 big-endian — CORBA's rule for a
         * GIOP 1.2 `wstring` without one, and what makes this setting
         * read a JacORB peer as well as an omniORB one. */
        static const uint8_t bytes[] = {
            0x04, 0x00, 0x00, 0x00, 0x00, 0x61, 0x00, 0x62
        };
        READER(r, bytes, ZDW_LITTLE, 8);
        expect_text("a wstring with no mark is big-endian",
                    zdw_read_wstring_utf8(&r, ZDW_WSTRING_SPECIFICATION,
                                          ZDW_WSTRING_UNITS_BYTE_ORDER_MARK),
                    "ab");
    }
    {
        /* TAO's, which has no terminator: a check hung off the width
         * rather than the policy refused every wide `wstring` whose
         * last character was not U+0000. */
        static const uint8_t bytes[] = {
            0x08, 0x00, 0x00, 0x00, 0x61, 0x00, 0x00, 0x00,
            0x62, 0x00, 0x00, 0x00
        };
        READER(r, bytes, ZDW_LITTLE, 8);
        expect_text("TAO's wide units carry no terminator",
                    zdw_read_wstring_utf8(&r, ZDW_WSTRING_SPECIFICATION_WIDE_UNITS,
                                          ZDW_WSTRING_UNITS_STREAM),
                    "ab");
    }
    {
        /* The octet count is checked and never believed. A bare
         * two-octet `wchar` read this way starts with 0xE4, which would
         * otherwise consume 228 octets of whatever followed. */
        static const uint8_t bytes[] = { 0xe4, 0x00, 0x34, 0x12 };
        READER(r, bytes, ZDW_LITTLE, 8);
        (void)zdw_read_wchar(&r, ZDW_WCHAR_WIDTH_2,
                             ZDW_WCHAR_GIOP_LENGTH_PREFIXED,
                             ZDW_WSTRING_UNITS_BYTE_ORDER_MARK);
        expect_read_status("a GIOP wchar count that is not the code set's width",
                           &r, ZDW_WRONG_FRAMING);
    }
}

/* `WstringCodeUnits::Utf8` — the characters' own octets, with the length
 * an octet count rather than §7.4.1.1.2's "twice the number of
 * characters", and with C2 still reached. Expected octets from
 * `tests/interop-results/wstring-member/hdds/plain-cdr1.le.bin`
 * (`04 00 00 00 61 62 63 00` for `"abc"`),
 * `tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr1.le.bin`
 * (`0d 00 00 00` then the twelve octets of `"äöü ሴ Z"` and a NUL) and
 * `tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr1.le.bin`
 * (`07 00 00 00 e1 88 b4 c3 a4 5a 00` for `wstring<16> "ሴäZ"`). */
static void a_wstring_written_as_utf8_octets(void) {
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wstring_utf8_octets(&w, "abc",
                                      ZDW_WSTRING_OCTETS_TERMINATED_2);
        expect("a UTF-8 wstring is its octets and a NUL", &w,
               "0400000061626300");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wstring_utf8_octets(&w, "\xc3\xa4\xc3\xb6\xc3\xbc \xe1\x88\xb4 Z",
                                      ZDW_WSTRING_OCTETS_TERMINATED_2);
        expect("a multi-byte UTF-8 wstring counts its octets", &w,
               "0d000000c3a4c3b6c3bc20e188b4205a00");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        zdw_write_wstring_utf8_octets(&w, "\xc3",
                                      ZDW_WSTRING_OCTETS_TERMINATED_2);
        expect_status("malformed UTF-8 is refused", &w, ZDW_NOT_UTF8);
    }
}

static void a_bounded_utf8_wstring_counts_units(void) {
    WRITER(w, ZDW_LITTLE, 4);
    /* `wstring<16>` bounds the three UTF-16 code units, and the wire
     * carries the six octets plus the terminator. */
    zdw_write_wstring_utf8_octets_bounded(&w, "\xe1\x88\xb4\xc3\xa4Z", 3,
                                          ZDW_WSTRING_OCTETS_TERMINATED_2);
    expect("a bounded UTF-8 wstring holds three units", &w,
           "07000000e188b4c3a45a00");
}

static void a_utf8_wstring_reads_back_as_its_octets(void) {
    static const uint8_t bytes[] = {
        0x0d, 0x00, 0x00, 0x00, 0xc3, 0xa4, 0xc3, 0xb6, 0xc3, 0xbc,
        0x20, 0xe1, 0x88, 0xb4, 0x20, 0x5a, 0x00
    };
    READER(r, bytes, ZDW_LITTLE, 4);
    expect_text("a UTF-8 wstring reads back",
                zdw_read_wstring_utf8_octets(&r, ZDW_WSTRING_OCTETS_TERMINATED_2),
                "\xc3\xa4\xc3\xb6\xc3\xbc \xe1\x88\xb4 Z");
    expect_read_status("a UTF-8 wstring is well formed", &r, ZDW_OK);
}

/* `ElementPadding::Packed`: each collection element and each aggregate
 * value counts alignment from its own first octet. Expected octets from
 * `tests/interop-results/array-1d-struct-final/hdds/plain-cdr1.le.bin`
 * (`01000000 0200 03000000 0400 77`, the second `Cell`'s `long` at
 * offset six) and `tests/interop-results/typedef-of-struct/hdds/
 * plain-cdr1.le.bin` (`7929edff feff 01000000 ff7f`, the same inside
 * two `@final` members). */
static void a_packed_element_counts_alignment_from_its_own_start(void) {
    {
        WRITER(w, ZDW_LITTLE, 4);
        size_t saved = zdw_begin_packed_element(&w);
        zdw_write_i32(&w, 1);
        zdw_write_i16(&w, 2);
        zdw_finish_packed_element(&w, saved);
        saved = zdw_begin_packed_element(&w);
        zdw_write_i32(&w, 3);
        zdw_write_i16(&w, 4);
        zdw_finish_packed_element(&w, saved);
        zdw_write_u8(&w, 119);
        expect("a packed array element starts where the last ended", &w,
               "01000000020003000000040077");
    }
    {
        WRITER(w, ZDW_LITTLE, 4);
        size_t saved = zdw_begin_packed_element(&w);
        zdw_write_i32(&w, -1234567);
        zdw_write_i16(&w, -2);
        zdw_finish_packed_element(&w, saved);
        saved = zdw_begin_packed_element(&w);
        zdw_write_i32(&w, 1);
        zdw_write_i16(&w, 32767);
        zdw_finish_packed_element(&w, saved);
        expect("a packed aggregate counts alignment from its own start", &w,
               "7929edfffeff01000000ff7f");
    }
}

static void a_packed_element_reads_back(void) {
    static const uint8_t bytes[] = {
        0x01, 0x00, 0x00, 0x00, 0x02, 0x00,
        0x03, 0x00, 0x00, 0x00, 0x04, 0x00,
        0x77
    };
    int32_t a1;
    int32_t a2;
    int16_t b1;
    int16_t b2;
    size_t saved;
    READER(r, bytes, ZDW_LITTLE, 4);
    saved = zdw_reader_begin_packed_element(&r);
    a1 = zdw_read_i32(&r);
    b1 = zdw_read_i16(&r);
    zdw_reader_finish_packed_element(&r, saved);
    saved = zdw_reader_begin_packed_element(&r);
    a2 = zdw_read_i32(&r);
    b2 = zdw_read_i16(&r);
    zdw_reader_finish_packed_element(&r, saved);
    if (a1 != 1 || b1 != 2 || a2 != 3 || b2 != 4 || zdw_read_u8(&r) != 119u) {
        printf("FAIL a packed array element reads back: %d %d %d %d\n",
               (int)a1, (int)b1, (int)a2, (int)b2);
        failures++;
    }
    expect_read_status("a packed array element is read at its own origin", &r,
                       ZDW_OK);
}

/* §7.5.1.1.1, Table 42: the typedefs the Service provides, with the sizes
 * the table's C99 column gives. A negative array size is the compile-time
 * check; the run-time comparison names the one that failed. */
typedef char dds_int32_is_four_octets[sizeof(DDS_Int32) == 4 ? 1 : -1];
typedef char dds_int8_is_one_octet[sizeof(DDS_Int8) == 1 ? 1 : -1];
typedef char dds_int64_is_eight_octets[sizeof(DDS_Int64) == 8 ? 1 : -1];
typedef char dds_float128_is_sixteen_octets[sizeof(DDS_Float128) == 16 ? 1 : -1];

static void the_primitive_typedefs_of_table_42_exist(void) {
    DDS_Int32 i32 = -1;
    DDS_UInt32 u32 = 0xffffffffu;
    DDS_Int8 i8 = -1;
    DDS_UInt8 u8 = 255u;
    DDS_Int16 i16 = -1;
    DDS_UInt16 u16 = 65535u;
    DDS_Int64 i64 = -1;
    DDS_UInt64 u64 = 0xffffffffffffffffu;
    DDS_Float32 f32 = 1.5f;
    DDS_Float64 f64 = 2.5;
    DDS_Float128 f128 = { 0, 0 };
    DDS_Char8 c8 = 'a';
    DDS_Char16 c16 = 0x20acu;
    DDS_Boolean flag = 2;
    DDS_Byte byte = 0xa5u;
    if (sizeof i32 != 4 || sizeof u32 != 4 || sizeof i8 != 1 || sizeof u8 != 1 ||
        sizeof i16 != 2 || sizeof u16 != 2 || sizeof i64 != 8 ||
        sizeof u64 != 8 || sizeof f32 != 4 || sizeof f64 != 8 ||
        sizeof f128 != 16 || sizeof c8 != 1 || sizeof c16 != 2 ||
        sizeof byte != 1) {
        printf("FAIL a Table 42 typedef has the wrong size\n");
        failures++;
    }
    /* A _Bool holds 0 or 1 whatever it is assigned. */
    if (flag != 1 || i32 >= 0 || i8 >= 0 || i16 >= 0 || i64 >= 0 ||
        u32 != 0xffffffffu || u8 != 255u || u16 != 65535u ||
        u64 != 0xffffffffffffffffu || f32 != 1.5f || f64 != 2.5 ||
        c8 != 'a' || c16 != 0x20acu || byte != 0xa5u) {
        printf("FAIL a Table 42 typedef does not hold its value\n");
        failures++;
    }
}

int main(void) {
    max_alignment_caps_the_padding();
    a_string_counts_its_terminator();
    a_wstring_counts_utf16_bytes();
    a_wstring_written_as_utf8_octets();
    a_utf8_wstring_reads_back_as_its_octets();
    a_bounded_utf8_wstring_counts_units();
    a_packed_element_counts_alignment_from_its_own_start();
    a_packed_element_reads_back();
    a_pl_cdr1_parameter_counts_alignment_from_its_own_start();
    length_code_five_reuses_the_members_own_length();
    an_enumerator_is_written_into_a_signed_holder();
    the_other_two_wstring_conventions();
    the_three_corba_wide_character_forms();
    the_pl_cdr1_framing_is_selectable();
    a_fixed_is_packed_bcd_with_a_sign_nibble();
    a_character_that_does_not_fit_is_refused();
    a_short_buffer_fails_rather_than_truncates();
    an_unset_external_member_is_a_refusal_of_its_own();
    the_digest_is_md5();
    what_the_writer_wrote_the_reader_reads();
    an_emheader_of_code_five_leaves_the_length_word();
    a_parameter_value_counts_alignment_from_its_own_start();
    the_must_understand_bit_is_not_part_of_the_id();
    a_vendor_private_or_reserved_parameter_is_not_a_member();
    the_wrong_dialect_is_a_status_of_its_own();
    each_wstring_convention_reads_back();
    a_fixed_reads_back_as_its_canonical_decimal();
    a_fixed_can_carry_the_values_digits_instead();
    a_fixed_can_carry_the_digits_of_its_string();
    a_full_heap_fails_rather_than_shortens();
    a_speculative_read_puts_everything_back();
    a_long_double_is_sixteen_bytes_of_binary128();
    the_two_byte_orders_are_the_whole_value_reversed();
    a_long_double_reads_back_as_it_was_written();
    a_value_a_double_cannot_hold_survives_and_is_refused();
    the_platforms_long_double_converts_and_never_copies();
    the_exact_decimal_is_the_canonical_form();
    a_short_decimal_buffer_writes_nothing();
    a_trim_cuts_on_a_character_boundary();
    a_discard_is_not_one_of_the_wire_errors();
    the_primitive_typedefs_of_table_42_exist();
    if (failures != 0) {
        printf("%d expectation(s) did not hold\n", failures);
        return 1;
    }
    printf("the C wire runtime holds every expectation\n");
    return 0;
}
