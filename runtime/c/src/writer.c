/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Putting bytes on the wire, in C.
 *
 * Alignment is relative to the start of the stream and not to the start
 * of the value: XCDR aligns each primitive to its own size counted from
 * the encapsulation, which is why a `long` after a `char` sits at 4 and
 * not at 1. §7.4.1.1.
 */
#include "zerodds_wire.h"

#include <string.h>

const char *zdw_status_name(zdw_status status) {
    switch (status) {
    case ZDW_OK:
        return "ok";
    case ZDW_SHORT_BUFFER:
        return "the buffer is too small for the sample";
    case ZDW_RANGE:
        return "a value outside what its holder can represent";
    case ZDW_BAD_ARGUMENT:
        return "a parameter the runtime has no rule for";
    case ZDW_NOT_UTF8:
        return "text that is not well-formed UTF-8";
    case ZDW_BAD_DECIMAL:
        return "a decimal that the fixed-point type cannot represent";
    case ZDW_TRUNCATED:
        return "fewer bytes remain than the read needs";
    case ZDW_IMPOSSIBLE_LENGTH:
        return "a length the stream cannot hold";
    case ZDW_OUT_OF_RANGE:
        return "a value the type does not admit";
    case ZDW_WRONG_FRAMING:
        return "the stream is framed the way another implementation frames it";
    case ZDW_DISCARDED:
        return "the sample is discarded: this type cannot hold that value";
    case ZDW_UNSET_EXTERNAL:
        return "an external member that is not optional has no value";
    }
    return "an unnamed status";
}

/* -- the buffer ---------------------------------------------------------- */

uint16_t zdw_pid_flags_at(zdw_pid_flag_policy policy,
                          zdw_wire_position position) {
    switch (policy) {
    case ZDW_PID_FLAGS_SENTINEL_MUST_UNDERSTAND:
        return (position == ZDW_POSITION_SENTINEL
                || position == ZDW_POSITION_EXTENDED_MARKER)
                   ? (uint16_t)ZDW_PID_MUST_UNDERSTAND
                   : (uint16_t)0;
    case ZDW_PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS:
        return (position == ZDW_POSITION_MEMBER
                || position == ZDW_POSITION_EXTENDED_MARKER)
                   ? (uint16_t)(ZDW_PID_IMPL_EXTENSION | ZDW_PID_MUST_UNDERSTAND)
                   : (uint16_t)0;
    case ZDW_PID_FLAGS_BARE:
    default:
        return (uint16_t)0;
    }
}

zdw_pl_cdr1_framing zdw_pl_cdr1_default(void) {
    /* The clause's framing, which is this project's default dialect:
     * `Dialect::Specification`. It was Cyclone's, and that is what the
     * three `false`s here used to be — a default whose warrant is a
     * product hands a caller that product's answers, which is why the
     * default moved. `docs/specs/zerodds-interop-suite-1.0.md` §7.
     *
     * A generated codec never reaches this: it calls
     * `zdw_writer_set_pl_cdr1` with the framing its own plan was made
     * under. This is what a hand-written reader gets when it sets
     * nothing, so it has to be the same answers
     * `zerodds_wire::PlCdr1Framing::DEFAULT` gives. */
    zdw_pl_cdr1_framing framing;
    framing.pid_flags = ZDW_PID_FLAGS_SENTINEL_MUST_UNDERSTAND;
    framing.highest_short_id = (uint16_t)ZDW_PID_SHORT_MAX_ID;
    /* XTypes 1.3 §7.4.1.2: the declared length is the exact length of the
     * member, without the padding behind it. The length rounded up to four
     * (DDSI-RTPS 2.5 §9.4.2.11, Cyclone DDS, RTI Connext) is set by the
     * caller with pad_declared_length / pad_declared_optional_length. */
    framing.pad_declared_length = false;
    framing.pad_declared_optional_length = false;
    /* §7.4.1.2.2 makes the member's own must-understand bit
     * unconditional; Fast DDS is the one measured implementation that
     * leaves it off. */
    framing.member_must_understand = true;
    framing.pad_after_value = true;
    return framing;
}

void zdw_writer_set_pl_cdr1(zdw_writer *w, zdw_pl_cdr1_framing framing) {
    w->pl_cdr1 = framing;
}

void zdw_writer_set_short_length_codes(zdw_writer *w, bool shorten) {
    w->short_length_codes = shorten;
}

void zdw_writer_init(zdw_writer *w, uint8_t *buffer, size_t capacity,
                     zdw_endianness endianness, size_t max_align) {
    w->out = buffer;
    w->capacity = capacity;
    w->len = 0;
    w->origin = 0;
    /* A cap of zero would make every alignment a division by zero; the
     * Rust runtime clamps the same way. */
    w->max_align = max_align < 1 ? 1 : max_align;
    w->endianness = endianness;
    w->status = ZDW_OK;
    w->pl_cdr1 = zdw_pl_cdr1_default();
    w->short_length_codes = false;
}

size_t zdw_writer_length(const zdw_writer *w) { return w->len; }

zdw_status zdw_writer_status(const zdw_writer *w) { return w->status; }

static void fail(zdw_writer *w, zdw_status status) {
    /* The first failure is the one that explains the rest. */
    if (w->status == ZDW_OK) {
        w->status = status;
    }
}

void zdw_writer_refuse(zdw_writer *w, zdw_status status) {
    if (status != ZDW_OK) {
        fail(w, status);
    }
}

/* Room for `count` bytes, or NULL with the writer already failed.
 *
 * Nothing is written on failure, so a caller that ignores the return
 * cannot half-write a value — the buffer keeps the length it had. */
static uint8_t *reserve(zdw_writer *w, size_t count) {
    uint8_t *at;
    if (w->status != ZDW_OK) {
        return NULL;
    }
    if (count > w->capacity - w->len) {
        fail(w, ZDW_SHORT_BUFFER);
        return NULL;
    }
    at = w->out + w->len;
    w->len += count;
    return at;
}

void zdw_align(zdw_writer *w, size_t align) {
    size_t offset;
    size_t padding;
    uint8_t *at;
    if (w->status != ZDW_OK) {
        return;
    }
    if (align > w->max_align) {
        align = w->max_align;
    }
    if (align <= 1) {
        return;
    }
    offset = w->len - w->origin;
    padding = (align - (offset % align)) % align;
    if (padding == 0) {
        return;
    }
    at = reserve(w, padding);
    if (at != NULL) {
        memset(at, 0, padding);
    }
}

/* -- byte order ---------------------------------------------------------- */

static void put2(zdw_writer *w, uint16_t value) {
    uint8_t *at = reserve(w, 2);
    if (at == NULL) {
        return;
    }
    if (w->endianness == ZDW_LITTLE) {
        at[0] = (uint8_t)(value & 0xffu);
        at[1] = (uint8_t)((value >> 8) & 0xffu);
    } else {
        at[0] = (uint8_t)((value >> 8) & 0xffu);
        at[1] = (uint8_t)(value & 0xffu);
    }
}

static void put4(zdw_writer *w, uint32_t value) {
    uint8_t *at = reserve(w, 4);
    if (at == NULL) {
        return;
    }
    if (w->endianness == ZDW_LITTLE) {
        at[0] = (uint8_t)(value & 0xffu);
        at[1] = (uint8_t)((value >> 8) & 0xffu);
        at[2] = (uint8_t)((value >> 16) & 0xffu);
        at[3] = (uint8_t)((value >> 24) & 0xffu);
    } else {
        at[0] = (uint8_t)((value >> 24) & 0xffu);
        at[1] = (uint8_t)((value >> 16) & 0xffu);
        at[2] = (uint8_t)((value >> 8) & 0xffu);
        at[3] = (uint8_t)(value & 0xffu);
    }
}

static void put8(zdw_writer *w, uint64_t value) {
    uint8_t *at = reserve(w, 8);
    int i;
    if (at == NULL) {
        return;
    }
    for (i = 0; i < 8; i++) {
        int shift = w->endianness == ZDW_LITTLE ? i * 8 : (7 - i) * 8;
        at[i] = (uint8_t)((value >> shift) & 0xffu);
    }
}

/* Patch four bytes already written, in the stream's byte order. */
static void patch4(zdw_writer *w, size_t at, uint32_t value) {
    if (w->status != ZDW_OK || at + 4 > w->len) {
        return;
    }
    if (w->endianness == ZDW_LITTLE) {
        w->out[at] = (uint8_t)(value & 0xffu);
        w->out[at + 1] = (uint8_t)((value >> 8) & 0xffu);
        w->out[at + 2] = (uint8_t)((value >> 16) & 0xffu);
        w->out[at + 3] = (uint8_t)((value >> 24) & 0xffu);
    } else {
        w->out[at] = (uint8_t)((value >> 24) & 0xffu);
        w->out[at + 1] = (uint8_t)((value >> 16) & 0xffu);
        w->out[at + 2] = (uint8_t)((value >> 8) & 0xffu);
        w->out[at + 3] = (uint8_t)(value & 0xffu);
    }
}

static void patch2(zdw_writer *w, size_t at, uint16_t value) {
    if (w->status != ZDW_OK || at + 2 > w->len) {
        return;
    }
    if (w->endianness == ZDW_LITTLE) {
        w->out[at] = (uint8_t)(value & 0xffu);
        w->out[at + 1] = (uint8_t)((value >> 8) & 0xffu);
    } else {
        w->out[at] = (uint8_t)((value >> 8) & 0xffu);
        w->out[at + 1] = (uint8_t)(value & 0xffu);
    }
}

/* -- primitives ---------------------------------------------------------- */

void zdw_write_bool(zdw_writer *w, bool value) {
    uint8_t *at = reserve(w, 1);
    if (at != NULL) {
        *at = value ? 1u : 0u;
    }
}

void zdw_write_u8(zdw_writer *w, uint8_t value) {
    uint8_t *at = reserve(w, 1);
    if (at != NULL) {
        *at = value;
    }
}

void zdw_write_i8(zdw_writer *w, int8_t value) {
    zdw_write_u8(w, (uint8_t)value);
}

void zdw_write_char(zdw_writer *w, uint32_t code_point) {
    if (code_point > 0xffu) {
        fail(w, ZDW_RANGE);
        return;
    }
    zdw_write_u8(w, (uint8_t)code_point);
}

/* C3 — how wide one of a `wstring`'s code units is under this cell of
 * the product. A scalar `wchar`'s width is `zdw_wchar_width`, which is
 * an argument and not a reading of this: `rmw_cyclonedds_cpp` writes
 * two octets there and four here. */
size_t zdw_wstring_unit_width(zdw_wstring_policy policy) {
    switch (policy) {
    case ZDW_WSTRING_CHARACTER_COUNT_TERMINATED:
    case ZDW_WSTRING_SPECIFICATION_WIDE_UNITS:
    case ZDW_WSTRING_ROS_TYPE_SUPPORT:
    case ZDW_WSTRING_OCTETS_TERMINATED_4:
        return 4u;
    default:
        return 2u;
    }
}

/* C1 — whether the length prefix counts characters rather than octets. */
int zdw_wstring_counts_characters(zdw_wstring_policy policy) {
    switch (policy) {
    case ZDW_WSTRING_CHARACTER_COUNT:
    case ZDW_WSTRING_CHARACTER_COUNT_TERMINATED:
    case ZDW_WSTRING_ROS_TYPE_SUPPORT:
    case ZDW_WSTRING_CHARACTERS_TERMINATED_2:
        return 1;
    default:
        return 0;
    }
}

/* C2 — whether a terminating NUL is written and counted. */
int zdw_wstring_terminated(zdw_wstring_policy policy) {
    switch (policy) {
    case ZDW_WSTRING_CHARACTER_COUNT_TERMINATED:
    case ZDW_WSTRING_OCTETS_TERMINATED_2:
    case ZDW_WSTRING_OCTETS_TERMINATED_4:
    case ZDW_WSTRING_CHARACTERS_TERMINATED_2:
        return 1;
    default:
        return 0;
    }
}

void zdw_write_wchar(zdw_writer *w, uint32_t code_point,
                     zdw_wchar_width requested,
                     zdw_wchar_framing framing,
                     zdw_wstring_unit_order order) {
    size_t width;
    if (code_point > 0xffffu) {
        fail(w, ZDW_RANGE);
        return;
    }
    width = (size_t)requested;
    /* The BMP restriction is a property of the type and not of the
     * width: the four-byte form zero-extends a code unit, it does not
     * carry a code point. */
    if (framing == ZDW_WCHAR_GIOP_LENGTH_PREFIXED) {
        /* CORBA §9.3.1.6's GIOP 1.2 form: the octet count first, then
         * the code unit's octets in the stream's order, all aligned to
         * one. The count is the transmission code set's width and not
         * the value's, so it is the same octet for every character —
         * omniORB writes `02` for both U+00E4 and U+1234. */
        /* The octets follow the same axis a `wstring`'s code units do,
         * with one difference the clause makes: a scalar cannot carry a
         * byte-order mark, so CORBA's default applies and
         * ZDW_WSTRING_UNITS_BYTE_ORDER_MARK means big-endian here.
         * Measured 2026-08-10 in a little-endian stream: omniORB writes
         * `02 00e4` and JacORB `02 00e4`, both big-endian; TAO writes
         * `04 e4000000`, the stream's order. */
        zdw_write_u8(w, (uint8_t)width);
        if (order != ZDW_WSTRING_UNITS_STREAM) {
            uint8_t *at = reserve(w, width);
            if (at == NULL) {
                return;
            }
            if (width == 4u) {
                at[0] = (uint8_t)((code_point >> 24) & 0xffu);
                at[1] = (uint8_t)((code_point >> 16) & 0xffu);
                at[2] = (uint8_t)((code_point >> 8) & 0xffu);
                at[3] = (uint8_t)(code_point & 0xffu);
            } else {
                at[0] = (uint8_t)((code_point >> 8) & 0xffu);
                at[1] = (uint8_t)(code_point & 0xffu);
            }
            return;
        }
        if (width == 4u) {
            put4(w, code_point);
        } else {
            put2(w, (uint16_t)code_point);
        }
        return;
    }
    if (width == 4u) {
        zdw_write_u32(w, code_point);
    } else {
        zdw_write_u16(w, (uint16_t)code_point);
    }
}

void zdw_write_u16(zdw_writer *w, uint16_t value) {
    zdw_align(w, 2);
    put2(w, value);
}

void zdw_write_u16_little(zdw_writer *w, uint16_t value) {
    uint8_t *at;
    zdw_align(w, 2);
    at = reserve(w, 2);
    if (at == NULL) {
        return;
    }
    at[0] = (uint8_t)(value & 0xffu);
    at[1] = (uint8_t)((value >> 8) & 0xffu);
}

void zdw_write_u16_big(zdw_writer *w, uint16_t value) {
    uint8_t *at;
    zdw_align(w, 2);
    at = reserve(w, 2);
    if (at == NULL) {
        return;
    }
    at[0] = (uint8_t)((value >> 8) & 0xffu);
    at[1] = (uint8_t)(value & 0xffu);
}

void zdw_write_u32_little(zdw_writer *w, uint32_t value) {
    uint8_t *at;
    zdw_align(w, 4);
    at = reserve(w, 4);
    if (at == NULL) {
        return;
    }
    at[0] = (uint8_t)(value & 0xffu);
    at[1] = (uint8_t)((value >> 8) & 0xffu);
    at[2] = (uint8_t)((value >> 16) & 0xffu);
    at[3] = (uint8_t)((value >> 24) & 0xffu);
}

void zdw_write_u32_big(zdw_writer *w, uint32_t value) {
    uint8_t *at;
    zdw_align(w, 4);
    at = reserve(w, 4);
    if (at == NULL) {
        return;
    }
    at[0] = (uint8_t)((value >> 24) & 0xffu);
    at[1] = (uint8_t)((value >> 16) & 0xffu);
    at[2] = (uint8_t)((value >> 8) & 0xffu);
    at[3] = (uint8_t)(value & 0xffu);
}

void zdw_write_i16(zdw_writer *w, int16_t value) {
    zdw_write_u16(w, (uint16_t)value);
}

void zdw_write_u32(zdw_writer *w, uint32_t value) {
    zdw_align(w, 4);
    put4(w, value);
}

void zdw_write_i32(zdw_writer *w, int32_t value) {
    zdw_write_u32(w, (uint32_t)value);
}

void zdw_write_u64(zdw_writer *w, uint64_t value) {
    zdw_align(w, 8);
    put8(w, value);
}

void zdw_write_i64(zdw_writer *w, int64_t value) {
    zdw_write_u64(w, (uint64_t)value);
}

void zdw_write_f32(zdw_writer *w, float value) {
    /* Through memcpy rather than a union or a cast: this is the only
     * spelling that is defined for every C99 implementation, and the
     * bit pattern is the thing on the wire. */
    uint32_t bits;
    memcpy(&bits, &value, sizeof bits);
    zdw_write_u32(w, bits);
}

void zdw_write_f64(zdw_writer *w, double value) {
    uint64_t bits;
    memcpy(&bits, &value, sizeof bits);
    zdw_write_u64(w, bits);
}

void zdw_write_f128(zdw_writer *w, zdw_f128 value) {
    /* All sixteen bytes of the value, and the whole sixteen reversed for
     * a big-endian stream — CORBA 3.3 part 2 Figure 9.2, not two
     * eight-byte halves swapped in place.
     *
     * This used to take a `double`, write it, and follow it with eight
     * zero bytes, because neither runtime had a 128-bit float. That was
     * the right length and the wrong value and it reported success.
     * Alignment is eight, capped to four under XCDR2 by `zdw_align`. */
    uint8_t bytes[16];
    uint8_t *at;
    zdw_f128_to_bytes(value, w->endianness, bytes);
    zdw_align(w, 8);
    at = reserve(w, 16);
    if (at != NULL) {
        memcpy(at, bytes, sizeof bytes);
    }
}

/* -- strings ------------------------------------------------------------- */

void zdw_write_string(zdw_writer *w, const char *text) {
    size_t length;
    uint8_t *at;
    if (text == NULL) {
        /* An empty string, not a skipped member: a NULL `char *` in a C
         * binding is how "" arrives, and writing nothing would shift
         * every member after it. */
        text = "";
    }
    length = strlen(text);
    if (length + 1 > 0xffffffffu) {
        fail(w, ZDW_RANGE);
        return;
    }
    zdw_write_u32(w, (uint32_t)(length + 1));
    at = reserve(w, length + 1);
    if (at == NULL) {
        return;
    }
    memcpy(at, text, length);
    at[length] = 0;
}

void zdw_write_string_bounded(zdw_writer *w, const char *text, uint32_t bound) {
    size_t length;
    if (text == NULL) {
        text = "";
    }
    length = strlen(text);
    if (length > bound) {
        fail(w, ZDW_RANGE);
        return;
    }
    zdw_write_string(w, text);
}

void zdw_check_sequence_bound(zdw_writer *w, uint32_t actual, uint32_t bound) {
    if (actual > bound) {
        fail(w, ZDW_RANGE);
    }
}

void zdw_check_map_bound(zdw_writer *w, uint32_t actual, uint32_t bound) {
    if (actual > bound) {
        fail(w, ZDW_RANGE);
    }
}

/* One UTF-8 code point, or a negative count on malformed input.
 *
 * Written out rather than taken from a library: the surrogate rule in
 * `zdw_write_wstring_utf8` is what §7.4.1.1.2's byte count depends on,
 * and a runtime that borrowed the decode would not be reimplementing
 * the clause.
 */
static int utf8_next(const unsigned char *s, uint32_t *out) {
    unsigned char c = s[0];
    if (c < 0x80u) {
        *out = c;
        return 1;
    }
    if ((c & 0xe0u) == 0xc0u) {
        if ((s[1] & 0xc0u) != 0x80u) {
            return -1;
        }
        *out = (uint32_t)(c & 0x1fu) << 6 | (uint32_t)(s[1] & 0x3fu);
        return *out < 0x80u ? -1 : 2;
    }
    if ((c & 0xf0u) == 0xe0u) {
        if ((s[1] & 0xc0u) != 0x80u || (s[2] & 0xc0u) != 0x80u) {
            return -1;
        }
        *out = (uint32_t)(c & 0x0fu) << 12 | (uint32_t)(s[1] & 0x3fu) << 6 |
               (uint32_t)(s[2] & 0x3fu);
        if (*out < 0x800u) {
            return -1;
        }
        /* A lone surrogate is not a code point; it is how a UTF-8
         * decoder that accepts CESU-8 silently changes the text. */
        if (*out >= 0xd800u && *out <= 0xdfffu) {
            return -1;
        }
        return 3;
    }
    if ((c & 0xf8u) == 0xf0u) {
        if ((s[1] & 0xc0u) != 0x80u || (s[2] & 0xc0u) != 0x80u ||
            (s[3] & 0xc0u) != 0x80u) {
            return -1;
        }
        *out = (uint32_t)(c & 0x07u) << 18 | (uint32_t)(s[1] & 0x3fu) << 12 |
               (uint32_t)(s[2] & 0x3fu) << 6 | (uint32_t)(s[3] & 0x3fu);
        if (*out < 0x10000u || *out > 0x10ffffu) {
            return -1;
        }
        return 4;
    }
    return -1;
}

/* How many UTF-16 code units the text holds, and whether any code point
 * needed two of them.
 *
 * Two passes over the same walk rather than one over a buffer: the
 * length goes out before the units, and a length computed from a
 * different walk than the one that writes them is how the two come to
 * disagree.
 *
 * Code points are not counted separately any more. They were, and the
 * character-count length was written from that count, which declared
 * one word fewer than followed it for a supplementary code point at a
 * four-octet unit width.
 */
static bool wstring_measure(zdw_writer *w, const unsigned char *s,
                            size_t *units, bool *outside_bmp) {
    size_t i;
    *units = 0;
    *outside_bmp = false;
    for (i = 0; s[i] != 0;) {
        uint32_t code_point = 0;
        int taken = utf8_next(s + i, &code_point);
        if (taken < 0) {
            fail(w, ZDW_NOT_UTF8);
            return false;
        }
        if (code_point >= 0x10000u) {
            *units += 2;
            *outside_bmp = true;
        } else {
            *units += 1;
        }
        i += (size_t)taken;
    }
    return true;
}

/* One `wstring` code unit, at its width and in its order.
 *
 * The order and the length convention are separate settings and this is
 * where the two meet, once, so that a form cannot be written one way in
 * the length field and another in the content.
 *
 * The order is now one axis of four points, so the width picks the
 * writer and the order picks its endianness. There is no longer a
 * policy value that decides a byte order, and so no pair to refuse. */
static void write_wstring_unit(zdw_writer *w, uint16_t unit,
                               zdw_wstring_policy policy,
                               zdw_wstring_unit_order order) {
    if (zdw_wstring_unit_width(policy) == 4u) {
        switch (order) {
        case ZDW_WSTRING_UNITS_BIG_ENDIAN_ALWAYS:
            zdw_write_u32_big(w, unit);
            return;
        case ZDW_WSTRING_UNITS_BYTE_ORDER_MARK:
        case ZDW_WSTRING_UNITS_HOST_UNSWAPPED:
            zdw_write_u32_little(w, unit);
            return;
        default:
            zdw_write_u32(w, unit);
            return;
        }
    }
    switch (order) {
    case ZDW_WSTRING_UNITS_BIG_ENDIAN_ALWAYS:
        zdw_write_u16_big(w, unit);
        return;
    case ZDW_WSTRING_UNITS_BYTE_ORDER_MARK:
    case ZDW_WSTRING_UNITS_HOST_UNSWAPPED:
        zdw_write_u16_little(w, unit);
        return;
    default:
        zdw_write_u16(w, unit);
        return;
    }
}

void zdw_write_wstring_utf8(zdw_writer *w, const char *utf8,
                            zdw_wstring_policy policy,
                            zdw_wstring_unit_order order) {
    const unsigned char *s;
    size_t units = 0;
    bool outside_bmp = false;
    size_t i;
    if (utf8 == NULL) {
        utf8 = "";
    }
    s = (const unsigned char *)utf8;
    if (!wstring_measure(w, s, &units, &outside_bmp)) {
        return;
    }

    /* The byte-order mark is a code unit like any other and its octets
     * are counted in the length: omniORB writes `0a000000 fffe 6100
     * 34d8 1edd 6200` for `"a<U+1D11E>b"`, ten octets over five units.
     * §7.4.1.1.2 forbids it outright, so it is written only under
     * ZDW_WSTRING_UNITS_BYTE_ORDER_MARK, which no DDS dialect selects. */
    if (order == ZDW_WSTRING_UNITS_BYTE_ORDER_MARK && units > 0) {
        units += 1;
    }

    /* The length is the product's three coordinates and no longer a
     * switch over named forms: C2 says whether a terminator joins the
     * units, C1 says whether the prefix counts them or their octets,
     * and C3 says what an octet count multiplies by. A cell nobody has
     * measured computes its length here exactly as a measured one
     * does. */
    if (zdw_wstring_terminated(policy)) {
        units += 1;
    }
    if (zdw_wstring_counts_characters(policy)) {
        /* Fast CDR counts characters and then writes each four-byte
         * `wchar_t` as two bytes. A code point outside the BMP loses
         * its high bits there and the peer reads a different character
         * with no diagnostic at either end. Reproducing a vendor's
         * length convention is a policy; reproducing its data loss is
         * not, so the value is refused — but only where the units are
         * two octets wide, because a four-octet unit carries the
         * surrogate pair intact. */
        if (outside_bmp && zdw_wstring_unit_width(policy) == 2u) {
            fail(w, ZDW_RANGE);
            return;
        }
        if (units > 0xffffffffu) {
            fail(w, ZDW_RANGE);
            return;
        }
        /* Over the run that is written, which is code *units* and not
         * code points. A surrogate pair is two four-octet units on the
         * wire, so a count of code points declares one word fewer than
         * follows it and the reader takes the last word for the next
         * member. The Rust column counts `units.len()` here for the
         * same reason. It could only ever differ at a four-octet unit
         * width, because the two-octet width refuses a supplementary
         * code point outright — which is why the corpus never reached
         * it and only `tests/selftest.c` did. */
        zdw_write_u32(w, (uint32_t)units);
    } else {
        size_t width = zdw_wstring_unit_width(policy);
        if (units > 0xffffffffu / width) {
            fail(w, ZDW_RANGE);
            return;
        }
        zdw_write_u32(w, (uint32_t)(units * width));
    }

    /* An empty `wstring` carries no mark: omniORB writes `00000000` for
     * one, byte-identical to every other implementation. */
    if (order == ZDW_WSTRING_UNITS_BYTE_ORDER_MARK && s[0] != 0) {
        write_wstring_unit(w, 0xfeffu, policy, order);
    }

    for (i = 0; s[i] != 0;) {
        uint32_t code_point = 0;
        int taken = utf8_next(s + i, &code_point);
        if (taken < 0) {
            fail(w, ZDW_NOT_UTF8);
            return;
        }
        if (code_point >= 0x10000u) {
            uint32_t rest = code_point - 0x10000u;
            uint16_t high = (uint16_t)(0xd800u + (rest >> 10));
            uint16_t low = (uint16_t)(0xdc00u + (rest & 0x3ffu));
            write_wstring_unit(w, high, policy, order);
            write_wstring_unit(w, low, policy, order);
        } else {
            write_wstring_unit(w, (uint16_t)code_point, policy, order);
        }
        i += (size_t)taken;
    }
    if (zdw_wstring_terminated(policy)) {
        write_wstring_unit(w, 0, policy, order);
    }
}

void zdw_write_wstring_bounded(zdw_writer *w, const char *utf8, uint32_t bound,
                               zdw_wstring_policy policy,
                               zdw_wstring_unit_order order) {
    const unsigned char *s;
    size_t units = 0;
    bool outside_bmp = false;
    if (utf8 == NULL) {
        utf8 = "";
    }
    s = (const unsigned char *)utf8;
    if (!wstring_measure(w, s, &units, &outside_bmp)) {
        return;
    }
    if (units > bound) {
        fail(w, ZDW_RANGE);
        return;
    }
    zdw_write_wstring_utf8(w, utf8, policy, order);
}

/* `WstringCodeUnits::Utf8`'s write direction. See the header for the
 * measured departure and the clause it departs from. The length is the
 * UTF-8 octet count plus the terminator; nothing here consults C1 or C3,
 * because a byte with no fixed width has nothing for a character count
 * to count and no width for an octet count to multiply by. */
void zdw_write_wstring_utf8_octets(zdw_writer *w, const char *utf8,
                                   zdw_wstring_policy policy) {
    size_t bytes;
    size_t length;
    size_t index;
    size_t units = 0;
    bool outside_bmp = false;
    if (utf8 == NULL) {
        utf8 = "";
    }
    /* The same malformed-UTF-8 refusal the UTF-16 writer makes. A
     * `const char *` is not a `String`, so this direction has to check
     * what the Rust column's type checked for it. */
    if (!wstring_measure(w, (const unsigned char *)utf8, &units,
                         &outside_bmp)) {
        return;
    }
    (void)units;
    (void)outside_bmp;
    bytes = strlen(utf8);
    length = bytes + (zdw_wstring_terminated(policy) ? 1u : 0u);
    if (length > 0xffffffffu) {
        fail(w, ZDW_RANGE);
        return;
    }
    zdw_write_u32(w, (uint32_t)length);
    for (index = 0; index < bytes; index++) {
        zdw_write_u8(w, (uint8_t)utf8[index]);
    }
    if (zdw_wstring_terminated(policy)) {
        zdw_write_u8(w, 0u);
    }
}

void zdw_write_wstring_utf8_octets_bounded(zdw_writer *w, const char *utf8,
                                           uint32_t bound,
                                           zdw_wstring_policy policy) {
    size_t units = 0;
    bool outside_bmp = false;
    if (utf8 == NULL) {
        utf8 = "";
    }
    /* A `wstring<N>` bound counts UTF-16 code units whatever the code
     * units the wire carries, measured in the same walk the UTF-16
     * writer uses. */
    if (!wstring_measure(w, (const unsigned char *)utf8, &units,
                         &outside_bmp)) {
        return;
    }
    (void)outside_bmp;
    if (units > bound) {
        fail(w, ZDW_RANGE);
        return;
    }
    zdw_write_wstring_utf8_octets(w, utf8, policy);
}

/* -- packed collection elements ------------------------------------------ */

size_t zdw_begin_packed_element(zdw_writer *w) {
    size_t saved = w->origin;
    w->origin = w->len;
    return saved;
}

void zdw_finish_packed_element(zdw_writer *w, size_t saved) {
    w->origin = saved;
}

/* -- framing ------------------------------------------------------------- */

zdw_dheader zdw_no_dheader(void) {
    zdw_dheader header;
    header.at = 0;
    header.present = false;
    return header;
}

zdw_dheader zdw_write_dheader(zdw_writer *w) {
    zdw_dheader header;
    uint8_t *at;
    zdw_align(w, 4);
    header.at = w->len;
    header.present = true;
    at = reserve(w, 4);
    if (at != NULL) {
        memset(at, 0, 4);
    }
    return header;
}

void zdw_finish_dheader(zdw_writer *w, zdw_dheader header) {
    size_t span;
    if (!header.present) {
        return;
    }
    span = w->len - header.at - 4;
#if SIZE_MAX > 0xffffffffu
    /* XTypes 1.3 7.4.3.4.1: O.ssize is smaller than 4 GiB. A longer span has
     * no DHEADER; the writer fails instead of writing it modulo 2^32. */
    if (span > 0xffffffffu) {
        fail(w, ZDW_RANGE);
        return;
    }
#endif
    patch4(w, header.at, (uint32_t)span);
}

zdw_emheader zdw_write_emheader(zdw_writer *w, uint32_t member_id,
                                uint8_t length_code, bool must_understand) {
    zdw_emheader header;
    uint32_t word;
    header.word = 0;
    header.next_int = 0;
    header.has_next_int = false;
    header.length_code = (uint8_t)ZDW_EMHEADER_NO_CODE;
    if (length_code > 7) {
        fail(w, ZDW_BAD_ARGUMENT);
        return header;
    }
    /* §7.2.2.4.4.4.4 gives a member id the range 0 to 0x0FFFFFFF and
     * §7.4.3.4.2 leaves it the low 28 bits of the word. Masking a larger
     * one writes a well-formed header for `id & 0x0FFFFFFF`: the peer
     * reads a different member than the caller named, and nothing at
     * either end says so. */
    if (member_id > ZDW_E_MEMBER_ID_MASK) {
        fail(w, ZDW_RANGE);
        return header;
    }
    header.length_code = length_code;
    zdw_align(w, 4);
    word = ((uint32_t)(must_understand ? 1u : 0u) << 31) |
           ((uint32_t)(length_code & 0x7u) << 28) | (member_id & 0x0fffffffu);
    zdw_write_u32(w, word);
    header.word = w->len >= 4u ? w->len - 4u : 0u;

    /* Only code 4 reserves a NEXTINT of its own; 5 through 7 borrow the
     * member's leading length word, and reserving one here would write
     * the length twice. §7.4.3.4.2. */
    if (length_code == 4) {
        uint8_t *at;
        header.next_int = w->len;
        header.has_next_int = true;
        at = reserve(w, 4);
        if (at != NULL) {
            memset(at, 0, 4);
        }
    }
    return header;
}

/* Replace an LC4 header's NEXTINT with a short length code.
 *
 * The four placeholder bytes are removed and the code in the header
 * word is raised from 4 to the one that spells `length`. Every byte of
 * the member's value is unchanged and only its position moves, which is
 * safe for one reason and only under XCDR2: MAXALIGN is four there, so
 * the value began at a four-aligned offset with the NEXTINT in front of
 * it and begins at a four-aligned offset without it, and no padding
 * inside it was computed against an offset that has moved. */
static void shorten_emheader(zdw_writer *w, zdw_emheader header,
                             uint32_t length) {
    uint32_t code;
    uint32_t previous;
    size_t at = header.next_int;
    if (w->status != ZDW_OK || at + 4u > w->len || header.word + 4u > w->len) {
        return;
    }
    switch (length) {
    case 1: code = 0u; break;
    case 2: code = 1u; break;
    case 4: code = 2u; break;
    default: code = 3u; break;
    }
    memmove(w->out + at, w->out + at + 4u, w->len - at - 4u);
    w->len -= 4u;
    if (w->endianness == ZDW_LITTLE) {
        previous = (uint32_t)w->out[header.word] |
                   (uint32_t)w->out[header.word + 1] << 8 |
                   (uint32_t)w->out[header.word + 2] << 16 |
                   (uint32_t)w->out[header.word + 3] << 24;
    } else {
        previous = (uint32_t)w->out[header.word] << 24 |
                   (uint32_t)w->out[header.word + 1] << 16 |
                   (uint32_t)w->out[header.word + 2] << 8 |
                   (uint32_t)w->out[header.word + 3];
    }
    patch4(w, header.word, (previous & ~(0x7u << 28)) | (code << 28));
}

void zdw_finish_emheader(zdw_writer *w, zdw_emheader header) {
    uint32_t length;
    if (!header.has_next_int) {
        /* Under codes 0..3 the code *is* the length — 1, 2, 4 or 8
         * octets, §7.4.3.4.2. A value of another size misdeclares it and
         * the reader's skip lands inside data, so the mismatch is
         * refused here rather than written. Codes 5..7 declare nothing
         * of their own: the member's leading length word is the NEXTINT.
         */
        if (header.length_code <= 3u && header.word + 4u <= w->len) {
            size_t declared = (size_t)1u << header.length_code;
            size_t body = w->len - header.word - 4u;
            if (body != declared) {
                fail(w, ZDW_BAD_ARGUMENT);
            }
        }
        return;
    }
    length = (uint32_t)(w->len - header.next_int - 4);
    if (w->short_length_codes &&
        (length == 1u || length == 2u || length == 4u || length == 8u)) {
        shorten_emheader(w, header, length);
        return;
    }
    patch4(w, header.next_int, length);
}

/* Start a parameter's value, returning the origin it displaces.
 *
 * "Each element, or parameter, within a parameter list data structure
 * is simply a CDR-encapsulated block of data" — §7.4.1.2. A block
 * counts alignment from its own start, so an eight-byte member at the
 * head of one is already aligned and takes no padding inside its own
 * declared length.
 */
static size_t begin_parameter_value(zdw_writer *w) {
    size_t saved = w->origin;
    w->origin = w->len;
    return saved;
}

zdw_pid zdw_write_pid(zdw_writer *w, uint16_t pid, zdw_parameter_role role,
                      bool must_understand) {
    zdw_pid header;
    uint8_t *at;
    /* §7.4.1.2.2 gives the short form the ids 0 to `highest_short_id`
     * inclusive and reserves what is above it — `PID_EXTENDED` is 0x3F01
     * itself. A short header with a reserved id collides with the
     * protocol's own parameters, so the id takes the extended form or it
     * does not go out. */
    if (pid > w->pl_cdr1.highest_short_id) {
        fail(w, ZDW_RANGE);
        header.at = w->len;
        header.extended = false;
        header.pid = pid;
        header.must_understand = must_understand;
        header.role = role;
        header.saved_origin = w->origin;
        return header;
    }
    zdw_align(w, 4);
    /* Two claimants on the same word, deliberately: the member's own
     * declaration, and whatever the framing puts on every id
     * regardless. OpenDDS's `c0010004` is the second alone. */
    if (must_understand && w->pl_cdr1.member_must_understand) {
        pid = (uint16_t)(pid | (uint16_t)ZDW_PID_MUST_UNDERSTAND);
    }
    pid = (uint16_t)(pid | zdw_pid_flags_at(w->pl_cdr1.pid_flags,
                                            ZDW_POSITION_MEMBER));
    zdw_write_u16(w, pid);
    header.at = w->len;
    header.extended = false;
    /* The id without either flag: `ZDW_PID_IMPL_EXTENSION` left in it
     * would be carried into an `eMemberHeader` by a promotion, where it
     * is a different bit. */
    header.pid = (uint32_t)(pid & (uint16_t)~(ZDW_PID_MUST_UNDERSTAND
                                              | ZDW_PID_IMPL_EXTENSION));
    header.must_understand = must_understand;
    header.role = role;
    at = reserve(w, 2);
    if (at != NULL) {
        memset(at, 0, 2);
    }
    header.saved_origin = begin_parameter_value(w);
    return header;
}

zdw_pid zdw_write_pid_extended(zdw_writer *w, uint32_t member_id,
                               zdw_parameter_role role, bool must_understand) {
    zdw_pid header;
    uint8_t *at;
    /* The same 28 bits as an EMHEADER's, in the `eMemberHeader` of
     * §7.4.1.2.1. Masking here aliases the parameter onto another
     * member exactly as it does there. */
    if (member_id > ZDW_E_MEMBER_ID_MASK) {
        fail(w, ZDW_RANGE);
        header.at = w->len;
        header.extended = true;
        header.pid = member_id;
        header.must_understand = must_understand;
        header.role = role;
        header.saved_origin = w->origin;
        return header;
    }
    zdw_align(w, 4);
    zdw_write_u16(w, (uint16_t)(ZDW_PID_EXTENDED
                                | zdw_pid_flags_at(w->pl_cdr1.pid_flags,
                                                   ZDW_POSITION_EXTENDED_MARKER)));
    zdw_write_u16(w, 8);
    /* The member's must-understand is `FLAG_2` of the `eMemberHeader`
     * (§7.4.1.2.1) and not the marker's bit above it: Cyclone sets the
     * marker's on every parameter and `FLAG_2` only where the member
     * declares it. */
    member_id &= ZDW_E_MEMBER_ID_MASK;
    if (must_understand && w->pl_cdr1.member_must_understand) {
        member_id |= ZDW_E_FLAG_MUST_UNDERSTAND;
    }
    zdw_write_u32(w, member_id);
    header.at = w->len;
    header.extended = true;
    header.pid = member_id & ZDW_E_MEMBER_ID_MASK;
    header.must_understand = must_understand;
    header.role = role;
    at = reserve(w, 4);
    if (at != NULL) {
        memset(at, 0, 4);
    }
    header.saved_origin = begin_parameter_value(w);
    return header;
}

/* Rewrite a short parameter header as the extended form.
 *
 * Eight bytes are inserted between the header and the value and the
 * four fields are written over them. The value's own bytes do not
 * change: XCDR1's MAXALIGN is eight and the insertion is eight, so every
 * offset inside the value keeps its remainder and no padding it already
 * contains was computed against an offset that moved. The realignment
 * after the value is likewise unaffected, eight being a multiple of
 * four.
 *
 * `zerodds_ast_layout::needs_extended_pid` carries the measurement: RTI
 * and Fast DDS write the short form for `mut_string`'s `string s`, 24
 * bytes against the 32 an always-extended plan produced.
 */
static void promote_pid(zdw_writer *w, zdw_pid header, size_t length) {
    size_t start = header.at - 2u;
    uint32_t member_header;
    if (w->len + 8u > w->capacity) {
        fail(w, ZDW_SHORT_BUFFER);
        return;
    }
    memmove(w->out + header.at + 10u, w->out + header.at + 2u,
            w->len - header.at - 2u);
    w->len += 8u;
    patch2(w, start,
           (uint16_t)(ZDW_PID_EXTENDED
                      | zdw_pid_flags_at(w->pl_cdr1.pid_flags,
                                         ZDW_POSITION_EXTENDED_MARKER)));
    patch2(w, start + 2u, 8u);
    member_header = header.pid & ZDW_E_MEMBER_ID_MASK;
    if (header.must_understand && w->pl_cdr1.member_must_understand) {
        member_header |= ZDW_E_FLAG_MUST_UNDERSTAND;
    }
    patch4(w, start + 4u, member_header);
    patch4(w, start + 8u, (uint32_t)length);
}

void zdw_finish_pid(zdw_writer *w, zdw_pid header) {
    size_t width = header.extended ? 4u : 2u;
    size_t length;
    if (w->status != ZDW_OK) {
        return;
    }
    length = w->len - header.at - width;
    if (header.role == ZDW_PARAM_OPTIONAL ? w->pl_cdr1.pad_declared_optional_length
                                          : w->pl_cdr1.pad_declared_length) {
        /* Counted from the origin the padding will be measured from,
         * which is the enclosing stream's and not the parameter
         * block's — the realignment below runs after the restore.
         * Computing it against the block's own origin would agree
         * today, because a parameter value starts four-aligned, and
         * would stop agreeing the moment that changed. */
        size_t past = (w->len - header.saved_origin) % 4u;
        if (past != 0u) {
            length += 4u - past;
        }
    }
    if (header.extended) {
        patch4(w, header.at, (uint32_t)length);
    } else if (length > 0xffffu) {
        /* The short form cannot state this length, so the header is
         * rewritten as the extended one. Which values overrun sixteen
         * bits is a property of the sample and not of the type, so the
         * plan writes the short form for an unbounded member and the
         * fallback lives here, where the length is known. */
        promote_pid(w, header, length);
    } else {
        patch2(w, header.at, (uint16_t)length);
    }
    /* The padding follows the value and is outside the length. It is
     * measured from the enclosing stream, so the origin goes back
     * before it is written. Under Fast DDS's framing the parameter
     * writes no padding of its own — the next header aligns itself. */
    w->origin = header.saved_origin;
    if (w->pl_cdr1.pad_after_value) {
        zdw_align(w, 4);
    }
}

void zdw_write_sentinel(zdw_writer *w) {
    /* The flags come from the framing asked at `ZDW_POSITION_SENTINEL`,
     * which is the only thing that decides them: `PID_LIST_END` is not
     * a member and has no declaration of its own to carry. */
    zdw_align(w, 4);
    zdw_write_u16(w, (uint16_t)(ZDW_PID_LIST_END
                                | zdw_pid_flags_at(w->pl_cdr1.pid_flags,
                                                   ZDW_POSITION_SENTINEL)));
    zdw_write_u16(w, 0);
}

/* -- the constructed scalars --------------------------------------------- */

void zdw_write_enum(zdw_writer *w, uint16_t bit_bound, int64_t value) {
    uint64_t mask;
    uint64_t pattern;
    int64_t low;
    int64_t high;
    if (bit_bound == 0 || bit_bound > 32) {
        fail(w, ZDW_BAD_ARGUMENT);
        return;
    }
    low = -((int64_t)1 << (bit_bound - 1));
    high = ((int64_t)1 << bit_bound) - 1;
    if (value < low || value > high) {
        fail(w, ZDW_RANGE);
        return;
    }
    mask = bit_bound >= 64 ? ~(uint64_t)0 : (((uint64_t)1 << bit_bound) - 1u);
    pattern = (uint64_t)value & mask;
    if (bit_bound <= 8) {
        zdw_write_u8(w, (uint8_t)pattern);
    } else if (bit_bound <= 16) {
        zdw_write_u16(w, (uint16_t)pattern);
    } else {
        zdw_write_u32(w, (uint32_t)pattern);
    }
}

/* The Cyclone union overlay: the low `octets` of `pattern`, written as
 * one unsigned integer of that width in the stream's byte order.
 *
 * A discriminator carrying a label the `union-labels` dialect axis says
 * selects nothing makes the peer fall through to `default:` and
 * serialise its one C union storage *as that arm* — the selected
 * member's octets, reinterpreted. `pattern` is the u64 whose
 * little-endian bytes are the selected member's little-endian storage
 * octets, zero-extended; the `zdw_bit_pattern_*` functions produce it. */
void zdw_write_union_overlay(zdw_writer *w, uint64_t pattern, uint32_t octets) {
    switch (octets) {
    case 1:
        zdw_write_u8(w, (uint8_t)pattern);
        return;
    case 2:
        zdw_write_u16(w, (uint16_t)pattern);
        return;
    case 4:
        zdw_write_u32(w, (uint32_t)pattern);
        return;
    case 8:
        zdw_write_u64(w, pattern);
        return;
    default:
        fail(w, ZDW_BAD_ARGUMENT);
    }
}

uint64_t zdw_bit_pattern_int(uint64_t value, uint32_t octets) {
    /* The mask is the function: the value arrives through the caller's
     * widest integer, so a negative one arrives sign-extended and `-2`
     * in four octets has to come out 0x00000000FFFFFFFE. */
    return octets >= 8 ? value : (value & ((UINT64_C(1) << (octets * 8)) - 1));
}

uint64_t zdw_bit_pattern_bool(bool value) { return value ? 1 : 0; }

uint64_t zdw_bit_pattern_f32(float value) {
    uint32_t bits;
    memcpy(&bits, &value, sizeof bits);
    return bits;
}

uint64_t zdw_bit_pattern_f64(double value) {
    uint64_t bits;
    memcpy(&bits, &value, sizeof bits);
    return bits;
}

void zdw_write_bitmask(zdw_writer *w, uint16_t bit_bound, uint64_t bits) {
    if (bit_bound == 0 || bit_bound > 64) {
        fail(w, ZDW_BAD_ARGUMENT);
        return;
    }
    if (bit_bound < 64 && (bits >> bit_bound) != 0) {
        fail(w, ZDW_RANGE);
        return;
    }
    if (bit_bound <= 8) {
        zdw_write_u8(w, (uint8_t)bits);
    } else if (bit_bound <= 16) {
        zdw_write_u16(w, (uint16_t)bits);
    } else if (bit_bound <= 32) {
        zdw_write_u32(w, (uint32_t)bits);
    } else {
        zdw_write_u64(w, bits);
    }
}

void zdw_write_bitset(zdw_writer *w, uint32_t anchor_bytes, uint64_t bits) {
    switch (anchor_bytes) {
    case 1:
        zdw_write_u8(w, (uint8_t)bits);
        return;
    case 2:
        zdw_write_u16(w, (uint16_t)bits);
        return;
    case 4:
        zdw_write_u32(w, (uint32_t)bits);
        return;
    case 8:
        zdw_write_u64(w, bits);
        return;
    default:
        fail(w, ZDW_BAD_ARGUMENT);
        return;
    }
}

uint64_t zdw_bitfield(zdw_writer *w, uint64_t value, uint32_t width,
                      uint32_t position) {
    uint64_t limit;
    if (width == 0 || width > 64 || position + width > 64) {
        fail(w, ZDW_BAD_ARGUMENT);
        return 0;
    }
    limit = width >= 64 ? ~(uint64_t)0 : (((uint64_t)1 << width) - 1u);
    if (value > limit) {
        fail(w, ZDW_RANGE);
        return 0;
    }
    return value << position;
}

void zdw_write_fixed(zdw_writer *w, const char *decimal, uint8_t digits,
                     uint8_t scale, zdw_fixed_digits count) {
    /* Sixty-four nibbles is more than `fixed<31, S>`, the widest IDL
     * admits, needs; the buffer is on the stack so the write path still
     * allocates nothing. */
    uint8_t nibbles[68];
    uint8_t all[68];
    size_t filled = 0;
    size_t significant;
    size_t wire;
    size_t leading;
    size_t written = 0;
    size_t i;
    const char *whole;
    size_t whole_len = 0;
    const char *fraction = "";
    size_t fraction_len = 0;
    bool negative = false;
    bool any_digit = false;
    size_t integral_width;
    size_t as_written;

    if (decimal == NULL || digits == 0 || scale > digits || digits > 31) {
        fail(w, ZDW_BAD_DECIMAL);
        return;
    }
    if (*decimal == '-') {
        negative = true;
        decimal++;
    } else if (*decimal == '+') {
        decimal++;
    }
    whole = decimal;
    while (whole[whole_len] >= '0' && whole[whole_len] <= '9') {
        whole_len++;
    }
    if (whole[whole_len] == '.') {
        fraction = whole + whole_len + 1;
        while (fraction[fraction_len] >= '0' && fraction[fraction_len] <= '9') {
            fraction_len++;
        }
        if (fraction[fraction_len] != 0) {
            fail(w, ZDW_BAD_DECIMAL);
            return;
        }
    } else if (whole[whole_len] != 0) {
        fail(w, ZDW_BAD_DECIMAL);
        return;
    }
    if (whole_len == 0 && fraction_len == 0) {
        fail(w, ZDW_BAD_DECIMAL);
        return;
    }
    as_written = whole_len + (size_t)scale;

    /* Leading zeros do not count against the integer part's width; a
     * value that still overruns is one this type cannot represent, and
     * truncating it would put a different number on the wire. */
    while (whole_len > 0 && *whole == '0') {
        whole++;
        whole_len--;
    }
    integral_width = (size_t)(digits - scale);
    if (whole_len > integral_width || fraction_len > (size_t)scale) {
        fail(w, ZDW_BAD_DECIMAL);
        return;
    }

    /* The full `P`-digit string first, whatever the policy: the
     * value's precision is that string with its leading zeros removed,
     * so both counts are read off one construction. */
    for (i = whole_len; i < integral_width; i++) {
        all[filled++] = 0;
    }
    for (i = 0; i < whole_len; i++) {
        all[filled] = (uint8_t)(whole[i] - '0');
        any_digit = any_digit || all[filled] != 0;
        filled++;
    }
    for (i = 0; i < fraction_len; i++) {
        all[filled] = (uint8_t)(fraction[i] - '0');
        any_digit = any_digit || all[filled] != 0;
        filled++;
    }
    for (i = fraction_len; i < (size_t)scale; i++) {
        all[filled++] = 0;
    }

    leading = 0;
    while (leading < filled && all[leading] == 0u) {
        leading++;
    }
    significant = filled - leading;
    /* Never fewer than one: `BigDecimal("0.00").precision()` is 1, and a
     * field of no digits could not carry the sign nibble's pair. */
    if (significant == 0u) {
        significant = 1u;
    }
    if (count == ZDW_FIXED_VALUE_STRING_DIGITS) {
        /* ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one leading
         * zero. Past 31 no reader can be told the field's width. */
        wire = as_written + ((as_written % 2u == 0u) ? 1u : 0u);
        if (wire > 31u) {
            fail(w, ZDW_BAD_DECIMAL);
            return;
        }
    } else {
        wire = (count == ZDW_FIXED_VALUE_PRECISION) ? significant : filled;
    }

    if ((wire + 1u) % 2u == 1u) {
        nibbles[written++] = 0;
    }
    /* A count wider than `P` is filled with leading zeros. */
    for (i = filled; i < wire; i++) {
        nibbles[written++] = 0;
    }
    for (i = (wire < filled) ? filled - wire : 0; i < filled; i++) {
        nibbles[written++] = all[i];
    }
    /* [CDR] §9.3.2.8: `0xd` marks a negative value and `0xc` everything
     * else, zero included — there is only one zero to write. */
    nibbles[written++] = (negative && any_digit) ? 0x0du : 0x0cu;

    for (i = 0; i + 1 < written; i += 2) {
        zdw_write_u8(w, (uint8_t)((nibbles[i] << 4) | nibbles[i + 1]));
    }
}

/* -- the KeyHash --------------------------------------------------------- */

void zdw_zero_padded(const zdw_writer *w, uint8_t out[16]) {
    size_t take = w->len < 16 ? w->len : 16;
    memset(out, 0, 16);
    memcpy(out, w->out, take);
}

void zdw_md5_of(const zdw_writer *w, uint8_t out[16]) {
    zdw_md5(w->out, w->len, out);
}
