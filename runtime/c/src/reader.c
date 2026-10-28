/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Taking bytes off the wire, in C.
 *
 * Every read that cannot be satisfied leaves the reader failed and
 * returns a zero the caller is expected not to use. A reader that
 * returned a plausible value for a truncated stream would hand the
 * caller something nobody sent, which is the failure this project
 * treats as the worst one.
 */
#include "zerodds_wire.h"

#include <string.h>

void zdw_reader_init(zdw_reader *r, const uint8_t *bytes, size_t length,
                     zdw_endianness endianness, size_t max_align) {
    r->bytes = bytes;
    r->length = length;
    r->at = 0;
    r->origin = 0;
    r->max_align = max_align < 1 ? 1 : max_align;
    r->endianness = endianness;
    r->status = ZDW_OK;
    r->error_at = 0;
    r->fits = NULL;
    r->pl_cdr1 = zdw_pl_cdr1_default();
    r->heap = NULL;
    r->heap_capacity = 0;
    r->heap_used = 0;
}

void zdw_reader_set_pl_cdr1(zdw_reader *r, zdw_pl_cdr1_framing framing) {
    r->pl_cdr1 = framing;
}

void zdw_reader_set_heap(zdw_reader *r, uint8_t *buffer, size_t capacity) {
    r->heap = buffer;
    r->heap_capacity = buffer == NULL ? 0u : capacity;
    r->heap_used = 0;
}

zdw_status zdw_reader_status(const zdw_reader *r) { return r->status; }

size_t zdw_reader_error_offset(const zdw_reader *r) { return r->error_at; }

const char *zdw_reader_fits(const zdw_reader *r) { return r->fits; }

size_t zdw_reader_position(const zdw_reader *r) { return r->at; }

size_t zdw_reader_remaining(const zdw_reader *r) {
    return r->at >= r->length ? 0u : r->length - r->at;
}

/* The first failure sticks, and it takes the offset with it. A later
 * read overwriting either would name the place the reader gave up
 * rather than the place it went wrong. */
static void fail(zdw_reader *r, zdw_status status) {
    if (r->status == ZDW_OK) {
        r->status = status;
        r->error_at = r->at;
    }
}

/* The same, for the one status that also carries a way out. */
static void fail_framing(zdw_reader *r, size_t at, const char *fits) {
    if (r->status == ZDW_OK) {
        r->status = ZDW_WRONG_FRAMING;
        r->error_at = at;
        r->fits = fits;
    }
}

zdw_reader_mark zdw_reader_save(const zdw_reader *r) {
    zdw_reader_mark mark;
    mark.at = r->at;
    mark.origin = r->origin;
    mark.heap_used = r->heap_used;
    mark.status = r->status;
    return mark;
}

void zdw_reader_restore(zdw_reader *r, zdw_reader_mark mark) {
    r->at = mark.at;
    r->origin = mark.origin;
    r->heap_used = mark.heap_used;
    r->status = mark.status;
    if (mark.status == ZDW_OK) {
        r->fits = NULL;
    }
}

void zdw_reader_refuse(zdw_reader *r, zdw_status status) {
    if (status != ZDW_OK) {
        fail(r, status);
    }
}

bool zdw_reader_clear_discard(zdw_reader *r) {
    if (r->status != ZDW_DISCARDED) {
        return false;
    }
    r->status = ZDW_OK;
    return true;
}

void zdw_reader_skip_to(zdw_reader *r, size_t target) {
    if (r->status != ZDW_OK || target <= r->at) {
        return;
    }
    if (target > r->length) {
        fail(r, ZDW_TRUNCATED);
        return;
    }
    r->at = target;
}

void *zdw_reader_alloc(zdw_reader *r, size_t bytes) {
    /* Wide enough for every scalar C99 has here, so no caller has to
     * state an alignment it cannot express. */
    const size_t align = 16u;
    size_t start;
    if (r->status != ZDW_OK) {
        return NULL;
    }
    start = (r->heap_used + (align - 1u)) & ~(align - 1u);
    if (r->heap == NULL || start > r->heap_capacity ||
        bytes > r->heap_capacity - start) {
        fail(r, ZDW_SHORT_BUFFER);
        return NULL;
    }
    r->heap_used = start + bytes;
    return r->heap + start;
}

static const uint8_t *take(zdw_reader *r, size_t count) {
    const uint8_t *at;
    if (r->status != ZDW_OK) {
        return NULL;
    }
    if (count > zdw_reader_remaining(r)) {
        fail(r, ZDW_TRUNCATED);
        return NULL;
    }
    at = r->bytes + r->at;
    r->at += count;
    return at;
}

void zdw_reader_align(zdw_reader *r, size_t align) {
    size_t padding;
    if (r->status != ZDW_OK) {
        return;
    }
    if (align > r->max_align) {
        align = r->max_align;
    }
    if (align <= 1) {
        return;
    }
    /* Counted from the origin and not from zero. §7.4.1.2 makes a
     * PL_CDR1 parameter value "a CDR-encapsulated block of data", which
     * counts its own alignment: Cyclone, RTI and Fast DDS all put a
     * `double` directly behind an eight-byte parameter header, and a
     * reader counting from the stream puts four bytes of padding there
     * that nobody wrote. The double then comes out of the following
     * bytes and every member after it is read at the wrong offset,
     * with nothing to object to — eight bytes were available and eight
     * bytes are a double. `vendor_bytes.md` §2, header `01000800`. */
    padding = (align - ((r->at - r->origin) % align)) % align;
    if (padding > zdw_reader_remaining(r)) {
        fail(r, ZDW_TRUNCATED);
        return;
    }
    r->at += padding;
}

/* The read-side pair of `zdw_begin_packed_element`. */
size_t zdw_reader_begin_packed_element(zdw_reader *r) {
    size_t saved = r->origin;
    r->origin = r->at;
    return saved;
}

void zdw_reader_finish_packed_element(zdw_reader *r, size_t saved) {
    r->origin = saved;
}

uint8_t zdw_read_u8(zdw_reader *r) {
    const uint8_t *at = take(r, 1);
    return at == NULL ? 0u : *at;
}

bool zdw_read_bool(zdw_reader *r) {
    uint8_t byte = zdw_read_u8(r);
    if (r->status != ZDW_OK) {
        return false;
    }
    if (byte > 1u) {
        /* A boolean that is 2 is a stream disagreeing with its own
         * type; taking the low bit would hide it. */
        fail(r, ZDW_OUT_OF_RANGE);
        return false;
    }
    return byte == 1u;
}

int8_t zdw_read_i8(zdw_reader *r) { return (int8_t)zdw_read_u8(r); }

uint32_t zdw_read_char(zdw_reader *r) { return zdw_read_u8(r); }

uint16_t zdw_read_u16(zdw_reader *r) {
    const uint8_t *at;
    zdw_reader_align(r, 2);
    at = take(r, 2);
    if (at == NULL) {
        return 0u;
    }
    return r->endianness == ZDW_LITTLE
               ? (uint16_t)((uint16_t)at[0] | (uint16_t)at[1] << 8)
               : (uint16_t)((uint16_t)at[0] << 8 | (uint16_t)at[1]);
}

uint16_t zdw_read_u16_little(zdw_reader *r) {
    const uint8_t *at;
    zdw_reader_align(r, 2);
    at = take(r, 2);
    if (at == NULL) {
        return 0u;
    }
    return (uint16_t)((uint16_t)at[0] | (uint16_t)at[1] << 8);
}

uint16_t zdw_read_u16_big(zdw_reader *r) {
    const uint8_t *at;
    zdw_reader_align(r, 2);
    at = take(r, 2);
    if (at == NULL) {
        return 0u;
    }
    return (uint16_t)((uint16_t)at[0] << 8 | (uint16_t)at[1]);
}

uint32_t zdw_read_u32_little(zdw_reader *r) {
    const uint8_t *at;
    zdw_reader_align(r, 4);
    at = take(r, 4);
    if (at == NULL) {
        return 0u;
    }
    return (uint32_t)at[0] | (uint32_t)at[1] << 8 | (uint32_t)at[2] << 16 |
           (uint32_t)at[3] << 24;
}

uint32_t zdw_read_u32_big(zdw_reader *r) {
    const uint8_t *at;
    zdw_reader_align(r, 4);
    at = take(r, 4);
    if (at == NULL) {
        return 0u;
    }
    return (uint32_t)at[0] << 24 | (uint32_t)at[1] << 16 | (uint32_t)at[2] << 8 |
           (uint32_t)at[3];
}

int16_t zdw_read_i16(zdw_reader *r) { return (int16_t)zdw_read_u16(r); }

uint32_t zdw_read_wchar(zdw_reader *r, zdw_wchar_width requested,
                        zdw_wchar_framing framing,
                        zdw_wstring_unit_order order) {
    uint32_t wide;
    size_t width = (size_t)requested;
    if (framing == ZDW_WCHAR_GIOP_LENGTH_PREFIXED) {
        /* CORBA §9.3.1.6's GIOP 1.2 form. The count is checked against
         * the configured code-set width rather than believed: a stream
         * whose first octet says 0xE4 — which is what a bare two-octet
         * `wchar` looks like read this way — would otherwise consume
         * 228 octets of whatever followed and return a character
         * assembled out of the next four members. */
        size_t at = r->at;
        uint32_t declared = zdw_read_u8(r);
        const uint8_t *octets;
        size_t i;
        if (r->status != ZDW_OK) {
            return 0u;
        }
        if ((size_t)declared != width) {
            fail_framing(r, at, NULL);
            return 0u;
        }
        octets = take(r, width);
        if (octets == NULL) {
            return 0u;
        }
        wide = 0u;
        /* A scalar cannot carry a byte-order mark, so
         * ZDW_WSTRING_UNITS_BYTE_ORDER_MARK means CORBA's default here
         * — big-endian — exactly as BIG_ENDIAN_ALWAYS does. */
        if (order == ZDW_WSTRING_UNITS_STREAM && r->endianness == ZDW_LITTLE) {
            for (i = 0; i < width; i++) {
                wide |= (uint32_t)octets[i] << (8u * i);
            }
        } else {
            for (i = 0; i < width; i++) {
                wide = (wide << 8) | (uint32_t)octets[i];
            }
        }
        if (wide > 0xffffu) {
            fail(r, ZDW_RANGE);
            return 0u;
        }
        return wide;
    }
    if (width != 4u) {
        return zdw_read_u16(r);
    }
    wide = zdw_read_u32(r);
    if (wide > 0xffffu) {
        /* RTI's four-byte character is one UTF-16 code unit zero-
         * extended. Narrowing a wider value would return a character
         * the peer never sent. */
        fail(r, ZDW_RANGE);
        return 0u;
    }
    return wide;
}

uint32_t zdw_read_u32(zdw_reader *r) {
    const uint8_t *at;
    zdw_reader_align(r, 4);
    at = take(r, 4);
    if (at == NULL) {
        return 0u;
    }
    if (r->endianness == ZDW_LITTLE) {
        return (uint32_t)at[0] | (uint32_t)at[1] << 8 | (uint32_t)at[2] << 16 |
               (uint32_t)at[3] << 24;
    }
    return (uint32_t)at[0] << 24 | (uint32_t)at[1] << 16 |
           (uint32_t)at[2] << 8 | (uint32_t)at[3];
}

int32_t zdw_read_i32(zdw_reader *r) { return (int32_t)zdw_read_u32(r); }

uint64_t zdw_read_u64(zdw_reader *r) {
    const uint8_t *at;
    uint64_t value = 0;
    int i;
    zdw_reader_align(r, 8);
    at = take(r, 8);
    if (at == NULL) {
        return 0u;
    }
    for (i = 0; i < 8; i++) {
        int shift = r->endianness == ZDW_LITTLE ? i * 8 : (7 - i) * 8;
        value |= (uint64_t)at[i] << shift;
    }
    return value;
}

int64_t zdw_read_i64(zdw_reader *r) { return (int64_t)zdw_read_u64(r); }

float zdw_read_f32(zdw_reader *r) {
    uint32_t bits = zdw_read_u32(r);
    float value;
    memcpy(&value, &bits, sizeof value);
    return value;
}

double zdw_read_f64(zdw_reader *r) {
    uint64_t bits = zdw_read_u64(r);
    double value;
    memcpy(&value, &bits, sizeof value);
    return value;
}

zdw_f128 zdw_read_f128(zdw_reader *r) {
    /* The counterpart of `zdw_write_f128`. This used to read the first
     * eight bytes as a `double` and skip the rest, so every value a
     * `double` cannot hold decoded as a different number in silence. */
    const uint8_t *at;
    zdw_reader_align(r, 8);
    at = take(r, 16);
    if (at == NULL) {
        return zdw_f128_zero();
    }
    return zdw_f128_from_bytes(at, r->endianness);
}

size_t zdw_read_string(zdw_reader *r, char *buffer, size_t capacity) {
    uint32_t declared = zdw_read_u32(r);
    size_t text_length;
    const uint8_t *at;
    if (r->status != ZDW_OK) {
        return 0u;
    }
    /* The length counts the terminator, so zero is not a legal length —
     * an empty string is one. */
    if (declared == 0u || (size_t)declared > zdw_reader_remaining(r) + 1u) {
        fail(r, ZDW_IMPOSSIBLE_LENGTH);
        return 0u;
    }
    text_length = (size_t)declared - 1u;
    at = take(r, text_length);
    if (at == NULL) {
        return 0u;
    }
    (void)zdw_read_u8(r); /* the terminator */
    if (buffer == NULL || capacity == 0u) {
        return text_length;
    }
    if (text_length + 1u > capacity) {
        /* Refused rather than truncated: a shortened string is a value
         * the peer did not send. */
        fail(r, ZDW_SHORT_BUFFER);
        return 0u;
    }
    memcpy(buffer, at, text_length);
    buffer[text_length] = 0;
    return text_length;
}

const char *zdw_read_string_alloc(zdw_reader *r) {
    size_t at = r->at;
    size_t length = zdw_read_string(r, NULL, 0);
    char *out;
    if (r->status != ZDW_OK) {
        return NULL;
    }
    /* Measured first and copied second: the length is the stream's, so
     * there is no way to size the destination before reading it. The
     * cursor goes back because the measuring pass consumed the value. */
    r->at = at;
    out = (char *)zdw_reader_alloc(r, length + 1u);
    if (out == NULL) {
        return NULL;
    }
    (void)zdw_read_string(r, out, length + 1u);
    return r->status == ZDW_OK ? out : NULL;
}

/* How many bytes the UTF-8 character starting at `byte` occupies.
 *
 * A continuation byte or a malformed lead is one byte, which keeps the
 * walk moving over input that is not UTF-8. Nothing that reaches here
 * is: `zdw_read_string` refuses a string that is not, and the wstring
 * reader builds its output itself. */
static size_t utf8_step(unsigned char byte) {
    if (byte < 0x80u) {
        return 1u;
    }
    if ((byte & 0xe0u) == 0xc0u) {
        return 2u;
    }
    if ((byte & 0xf0u) == 0xe0u) {
        return 3u;
    }
    if ((byte & 0xf8u) == 0xf0u) {
        return 4u;
    }
    return 1u;
}

size_t zdw_utf8_octets(const char *text) {
    return text == NULL ? 0u : strlen(text);
}

size_t zdw_utf16_units(const char *text) {
    size_t units = 0u;
    size_t at = 0u;
    if (text == NULL) {
        return 0u;
    }
    while (text[at] != '\0') {
        size_t step = utf8_step((unsigned char)text[at]);
        /* A scalar value above the basic plane is a surrogate pair, and
         * four UTF-8 bytes is exactly the set of those. */
        units += (step == 4u) ? 2u : 1u;
        at += step;
    }
    return units;
}

/* The shared body of the two trims: walk whole characters, stop before
 * the one that would cross the limit, copy the prefix into the reader's
 * heap. `per_character` says what one character costs in the unit the
 * caller is counting. */
static const char *trim_to(zdw_reader *r, const char *text, size_t limit,
                           bool count_units) {
    size_t at = 0u;
    size_t used = 0u;
    char *out;
    if (text == NULL) {
        return NULL;
    }
    while (text[at] != '\0') {
        size_t step = utf8_step((unsigned char)text[at]);
        size_t cost = count_units ? ((step == 4u) ? 2u : 1u) : step;
        if (used + cost > limit) {
            break;
        }
        used += cost;
        at += step;
    }
    out = (char *)zdw_reader_alloc(r, at + 1u);
    if (out == NULL) {
        return NULL;
    }
    if (at > 0u) {
        memcpy(out, text, at);
    }
    out[at] = '\0';
    return out;
}

const char *zdw_trim_utf8(zdw_reader *r, const char *text, size_t octets) {
    return trim_to(r, text, octets, false);
}

const char *zdw_trim_utf16(zdw_reader *r, const char *text, size_t units) {
    return trim_to(r, text, units, true);
}

/* One UTF-16 code unit as UTF-8, appended at `out`. Returns how many
 * bytes it took. The caller has already checked the room. */
static size_t utf8_put(uint8_t *out, uint32_t code_point) {
    if (code_point < 0x80u) {
        out[0] = (uint8_t)code_point;
        return 1u;
    }
    if (code_point < 0x800u) {
        out[0] = (uint8_t)(0xc0u | (code_point >> 6));
        out[1] = (uint8_t)(0x80u | (code_point & 0x3fu));
        return 2u;
    }
    if (code_point < 0x10000u) {
        out[0] = (uint8_t)(0xe0u | (code_point >> 12));
        out[1] = (uint8_t)(0x80u | ((code_point >> 6) & 0x3fu));
        out[2] = (uint8_t)(0x80u | (code_point & 0x3fu));
        return 3u;
    }
    out[0] = (uint8_t)(0xf0u | (code_point >> 18));
    out[1] = (uint8_t)(0x80u | ((code_point >> 12) & 0x3fu));
    out[2] = (uint8_t)(0x80u | ((code_point >> 6) & 0x3fu));
    out[3] = (uint8_t)(0x80u | (code_point & 0x3fu));
    return 4u;
}

/* Whether a declared length is one `policy` could have written. Only
 * the shape of the number, which is all a refused read has to go on:
 * an odd count is not a byte count, and a count of zero cannot include
 * a terminator. */
static bool wstring_length_is_possible(zdw_wstring_policy policy,
                                       uint32_t declared) {
    /* The three coordinates, not a list of named forms: an octet count
     * has to divide by the unit width, and a count that includes a
     * terminator cannot be zero. A cell nobody has measured is checked
     * by the same two rules. */
    if (!zdw_wstring_counts_characters(policy)) {
        if (declared % (uint32_t)zdw_wstring_unit_width(policy) != 0u) {
            return false;
        }
    }
    if (zdw_wstring_terminated(policy) && declared < 1u) {
        return false;
    }
    return true;
}

/* The setting a length fits, when exactly the other conventions do.
 *
 * Sweeps the whole product and not a hand-written list of four: a cell
 * left out of the list is a cell a diagnostic can never name, and the
 * point of the diagnostic is to name the one the writer used. */
static const char *wstring_fits(zdw_wstring_policy policy, uint32_t declared) {
    static const struct {
        zdw_wstring_policy cell;
        const char *flag;
    } cells[] = {
        {ZDW_WSTRING_SPECIFICATION, "--wstring-length specification"},
        {ZDW_WSTRING_CHARACTER_COUNT, "--wstring-length character-count"},
        {ZDW_WSTRING_CHARACTER_COUNT_TERMINATED,
         "--wstring-length character-count-terminated"},
        {ZDW_WSTRING_SPECIFICATION_WIDE_UNITS,
         "--wstring-length specification-wide-units"},
        {ZDW_WSTRING_ROS_TYPE_SUPPORT, "--wstring-length ros-type-support"},
        {ZDW_WSTRING_OCTETS_TERMINATED_2, "--wstring-length octets/terminated/2"},
        {ZDW_WSTRING_OCTETS_TERMINATED_4, "--wstring-length octets/terminated/4"},
        {ZDW_WSTRING_CHARACTERS_TERMINATED_2,
         "--wstring-length characters/terminated/2"}};
    size_t i;
    for (i = 0; i < sizeof cells / sizeof cells[0]; i++) {
        if (cells[i].cell != policy &&
            wstring_length_is_possible(cells[i].cell, declared)) {
            return cells[i].flag;
        }
    }
    return NULL;
}

/* One `wstring` code unit at the policy's width and byte order.
 *
 * ZDW_WSTRING_UNITS_HOST_UNSWAPPED is the order whose content does not
 * follow the stream: Cyclone swaps the length word and not the units,
 * and `rosidl_typesupport_fastrtps` does the same over four-octet
 * units. It is one point of the order axis now, so the width picks the
 * reader and the order picks its endianness. */
static uint32_t read_wstring_unit(zdw_reader *r, size_t width,
                                  zdw_wstring_policy policy,
                                  zdw_wstring_unit_order order) {
    (void)policy;
    if (width == 4u) {
        switch (order) {
        case ZDW_WSTRING_UNITS_BIG_ENDIAN_ALWAYS:
            return zdw_read_u32_big(r);
        case ZDW_WSTRING_UNITS_BYTE_ORDER_MARK:
        case ZDW_WSTRING_UNITS_HOST_UNSWAPPED:
            return zdw_read_u32_little(r);
        default:
            return zdw_read_u32(r);
        }
    }
    switch (order) {
    case ZDW_WSTRING_UNITS_BIG_ENDIAN_ALWAYS:
        return zdw_read_u16_big(r);
    case ZDW_WSTRING_UNITS_BYTE_ORDER_MARK:
    case ZDW_WSTRING_UNITS_HOST_UNSWAPPED:
        return zdw_read_u16_little(r);
    default:
        return zdw_read_u16(r);
    }
}

const char *zdw_read_wstring_utf8(zdw_reader *r, zdw_wstring_policy policy,
                                  zdw_wstring_unit_order order) {
    size_t at;
    uint32_t declared;
    size_t units;
    size_t width;
    size_t index;
    size_t used = 0;
    uint8_t *out;
    /* Under ZDW_WSTRING_UNITS_BYTE_ORDER_MARK every unit is read
     * little-endian and the leading mark then says whether they have to
     * be swapped back — FEFF the right way up, FFFE the other way, and
     * no mark at all means UTF-16 big-endian, which is CORBA's rule for
     * a GIOP 1.2 `wstring` without one. Both are decided from the first
     * unit, so the loop below needs a prologue and not a second pass. */
    bool swap_units = false;

    at = r->at;
    declared = zdw_read_u32(r);
    if (r->status != ZDW_OK) {
        return NULL;
    }
    /* The unit count from the three coordinates. An octet count has to
     * divide by the width; a character count is already the count. A
     * terminator is inside the count and is dropped from the string
     * after the run, not here, because it still occupies a unit on the
     * wire. */
    width = zdw_wstring_unit_width(policy);
    if (!wstring_length_is_possible(policy, declared)) {
        fail_framing(r, at, wstring_fits(policy, declared));
        return NULL;
    }
    units = zdw_wstring_counts_characters(policy) ? (size_t)declared
                                                  : (size_t)declared / width;
    if (units > zdw_reader_remaining(r) / width) {
        fail(r, ZDW_IMPOSSIBLE_LENGTH);
        return NULL;
    }
    /* Four bytes per unit at most, plus the terminator. Sized before
     * the read because the heap is a bump allocator: a block grown
     * halfway through would move what was already written into it. */
    out = (uint8_t *)zdw_reader_alloc(r, units * 4u + 1u);
    if (out == NULL) {
        return NULL;
    }

    /* The mark, and what it says about everything behind it. An empty
     * `wstring` carries none: omniORB writes `00000000` for one. */
    if (order == ZDW_WSTRING_UNITS_BYTE_ORDER_MARK && units > 0u) {
        uint32_t mark = read_wstring_unit(r, width, policy, order);
        if (r->status != ZDW_OK) {
            return NULL;
        }
        if (mark == 0xfeffu) {
            units -= 1u;
        } else if (mark == 0xfffeu) {
            units -= 1u;
            swap_units = true;
        } else {
            /* No mark, so CORBA's default: UTF-16 big-endian. The unit
             * just read is the first character and is put back by
             * rewinding, which is exact because every unit is `width`
             * octets and the run is contiguous. */
            r->at -= width;
            swap_units = true;
        }
    }

    for (index = 0; index < units; index++) {
        uint32_t unit = read_wstring_unit(r, width, policy, order);
        if (r->status != ZDW_OK) {
            return NULL;
        }
        if (swap_units) {
            unit = width == 4u ? ((unit >> 24) & 0xffu) | ((unit >> 8) & 0xff00u) |
                                     ((unit << 8) & 0xff0000u) | ((unit << 24) & 0xff000000u)
                               : (uint32_t)((uint16_t)(((unit & 0xffu) << 8) |
                                                       ((unit >> 8) & 0xffu)));
        }
        if (width == 4u) {
            /* A four-octet code unit is a UTF-16 code unit
             * zero-extended, in RTI's XCDR1 form and in TAO's alike. A
             * value above 0xFFFF is not a form this reader knows how to
             * narrow, so it is refused rather than truncated. */
            if (unit > 0xffffu) {
                fail(r, ZDW_OUT_OF_RANGE);
                return NULL;
            }
        }
        /* The terminator is C2 and not C3. Four octets per unit with
         * no NUL is TAO's cell, so a check hung off the width refused
         * every wide `wstring` whose last character was not U+0000. */
        if (zdw_wstring_terminated(policy)) {
            if (index + 1u == units) {
                /* The count includes the NUL character. */
                if (unit != 0u) {
                    fail(r, ZDW_OUT_OF_RANGE);
                    return NULL;
                }
                break;
            }
            /* A NUL inside the text is the narrow string's problem in
             * wide form: every C peer stops at it, so the value this
             * reader returns and the value the peer returns would
             * differ for bytes both accepted. */
            if (unit == 0u) {
                fail(r, ZDW_OUT_OF_RANGE);
                return NULL;
            }
        }
        if (unit >= 0xd800u && unit <= 0xdbffu) {
            uint32_t low;
            if (index + 1u >= units) {
                /* An unpaired surrogate is refused rather than
                 * replaced: U+FFFD in its place is a character the
                 * sender never had. */
                fail(r, ZDW_OUT_OF_RANGE);
                return NULL;
            }
            low = read_wstring_unit(r, width, policy, order);
            index++;
            if (r->status != ZDW_OK) {
                return NULL;
            }
            if (swap_units) {
                low = width == 4u ? ((low >> 24) & 0xffu) | ((low >> 8) & 0xff00u) |
                                        ((low << 8) & 0xff0000u) | ((low << 24) & 0xff000000u)
                                  : (uint32_t)((uint16_t)(((low & 0xffu) << 8) |
                                                          ((low >> 8) & 0xffu)));
            }
            if (low < 0xdc00u || low > 0xdfffu) {
                fail(r, ZDW_OUT_OF_RANGE);
                return NULL;
            }
            used += utf8_put(out + used,
                             0x10000u + ((unit - 0xd800u) << 10) +
                                 (low - 0xdc00u));
            continue;
        }
        if (unit >= 0xdc00u && unit <= 0xdfffu) {
            fail(r, ZDW_OUT_OF_RANGE);
            return NULL;
        }
        used += utf8_put(out + used, unit);
    }
    out[used] = 0;
    return (const char *)out;
}

/* Whether `bytes` is well-formed UTF-8: a continuation per lead byte, no
 * overlong form, no surrogate and nothing above U+10FFFF. The same walk
 * `utf8_next` in the writer is, so the two directions agree about what
 * the axis means. */
static bool utf8_is_valid(const uint8_t *bytes, size_t length) {
    size_t i = 0;
    while (i < length) {
        unsigned char c = bytes[i];
        size_t extra;
        uint32_t value;
        size_t k;
        if (c < 0x80u) {
            i += 1u;
            continue;
        }
        if ((c & 0xe0u) == 0xc0u) {
            extra = 1u;
            value = (uint32_t)(c & 0x1fu);
        } else if ((c & 0xf0u) == 0xe0u) {
            extra = 2u;
            value = (uint32_t)(c & 0x0fu);
        } else if ((c & 0xf8u) == 0xf0u) {
            extra = 3u;
            value = (uint32_t)(c & 0x07u);
        } else {
            return false;
        }
        if (i + extra >= length) {
            return false;
        }
        for (k = 1u; k <= extra; k++) {
            if ((bytes[i + k] & 0xc0u) != 0x80u) {
                return false;
            }
            value = (value << 6) | (uint32_t)(bytes[i + k] & 0x3fu);
        }
        if (extra == 1u) {
            if (value < 0x80u) {
                return false;
            }
        } else if (extra == 2u) {
            if (value < 0x800u || (value >= 0xd800u && value <= 0xdfffu)) {
                return false;
            }
        } else if (value < 0x10000u || value > 0x10ffffu) {
            return false;
        }
        i += extra + 1u;
    }
    return true;
}

/* The inverse of `zdw_write_wstring_utf8_octets`, as
 * `zdw_read_wstring_utf8` is to `zdw_write_wstring_utf8`. */
const char *zdw_read_wstring_utf8_octets(zdw_reader *r,
                                         zdw_wstring_policy policy) {
    uint32_t declared;
    size_t length;
    size_t index;
    const uint8_t *at;
    uint8_t *out;
    declared = zdw_read_u32(r);
    if (r->status != ZDW_OK) {
        return NULL;
    }
    length = (size_t)declared;
    if (length > zdw_reader_remaining(r)) {
        fail(r, ZDW_IMPOSSIBLE_LENGTH);
        return NULL;
    }
    at = take(r, length);
    if (at == NULL) {
        return NULL;
    }
    if (zdw_wstring_terminated(policy)) {
        if (length == 0u || at[length - 1u] != 0u) {
            fail(r, ZDW_OUT_OF_RANGE);
            return NULL;
        }
        /* A NUL inside the text is the narrow string's problem in wide
         * form: every C peer stops at it, so the value this reader
         * returns and the value the peer returns would differ for bytes
         * both accepted. */
        for (index = 0; index + 1u < length; index++) {
            if (at[index] == 0u) {
                fail(r, ZDW_OUT_OF_RANGE);
                return NULL;
            }
        }
        length -= 1u;
    }
    if (!utf8_is_valid(at, length)) {
        fail(r, ZDW_NOT_UTF8);
        return NULL;
    }
    out = (uint8_t *)zdw_reader_alloc(r, length + 1u);
    if (out == NULL) {
        return NULL;
    }
    if (length > 0u) {
        memcpy(out, at, length);
    }
    out[length] = 0;
    return (const char *)out;
}

const char *zdw_read_fixed(zdw_reader *r, uint8_t digits, uint8_t scale,
                           zdw_fixed_digits count_policy) {
    /* One nibble per digit and one for the sign, plus the leading pad
     * nibble when the nibble count is odd. `fixed<31, S>` is the widest
     * IDL admits, which is sixteen octets. */
    uint8_t nibbles[68];
    size_t count = (size_t)digits + 1u;
    size_t packed = (count + 1u) / 2u;
    size_t start;
    size_t index;
    size_t split;
    size_t spelled;
    bool negative;
    bool any_digit = false;
    char *out;
    size_t used = 0;

    if (digits == 0u || scale > digits || digits > 31u) {
        fail(r, ZDW_BAD_ARGUMENT);
        return NULL;
    }
    if (count_policy == ZDW_FIXED_VALUE_PRECISION ||
        count_policy == ZDW_FIXED_VALUE_STRING_DIGITS) {
        /* The value decides how wide the field is, so its end is read
         * out of the octets: a digit nibble is `0` to `9` and a sign is
         * `0xa` upwards, and the nibble count is even by construction,
         * so the field ends at the first octet whose **low** nibble is a
         * sign. A field that never presents one within the widest
         * `fixed` IDL admits is refused rather than read on. */
        packed = 0;
        for (;;) {
            uint8_t byte;
            if (packed >= 16u) {
                fail(r, ZDW_BAD_DECIMAL);
                return NULL;
            }
            byte = zdw_read_u8(r);
            if (r->status != ZDW_OK) {
                return NULL;
            }
            nibbles[packed * 2u] = (uint8_t)(byte >> 4);
            nibbles[packed * 2u + 1u] = (uint8_t)(byte & 0x0fu);
            packed++;
            if ((byte & 0x0fu) >= ZDW_FIXED_FIRST_SIGN_NIBBLE) {
                break;
            }
        }
        count = packed * 2u;
    } else {
        for (index = 0; index < packed; index++) {
            uint8_t byte = zdw_read_u8(r);
            if (r->status != ZDW_OK) {
                return NULL;
            }
            nibbles[index * 2u] = (uint8_t)(byte >> 4);
            nibbles[index * 2u + 1u] = (uint8_t)(byte & 0x0fu);
        }
    }
    /* A leading zero nibble pads the pair when the nibble count is odd;
     * it is not a digit and the writer put it there for the packing
     * alone. */
    start = packed * 2u - count;
    switch (nibbles[packed * 2u - 1u]) {
    case 0x0du:
    case 0x0bu:
        negative = true;
        break;
    case 0x0cu:
    case 0x0au:
    case 0x0eu:
    case 0x0fu:
        negative = false;
        break;
    default:
        fail(r, ZDW_BAD_DECIMAL);
        return NULL;
    }
    for (index = start; index + 1u < packed * 2u; index++) {
        if (nibbles[index] > 9u) {
            fail(r, ZDW_BAD_DECIMAL);
            return NULL;
        }
        if (nibbles[index] != 0u) {
            any_digit = true;
        }
    }

    /* The canonical spelling: sign, the integer part with no leading
     * zeros beyond a single one, the point, exactly `scale` digits.
     *
     * The point sits `scale` digits from the **right** and not `P - S`
     * from the left. Under the declared count the two are the same
     * arithmetic, because the field carries exactly `P` digits; under
     * the value's count it carries fewer, and a value with fewer digits
     * than the scale — `1c` under `fixed<5,2>` — needs its integer part
     * written as a zero. `spelled` is the digit count after enough
     * leading zeros for the integer part to have one. */
    spelled = count - 1u;
    if (spelled < (size_t)scale + 1u) {
        spelled = (size_t)scale + 1u;
    }
    out = (char *)zdw_reader_alloc(r, spelled + 3u);
    if (out == NULL) {
        return NULL;
    }
    if (negative && any_digit) {
        out[used++] = '-';
    }
    split = spelled - (size_t)scale;
    {
        size_t pad = spelled - (count - 1u);
        size_t first = 0;
        while (first < split && (first < pad || nibbles[start + first - pad] == 0u)) {
            first++;
        }
        if (first == split) {
            out[used++] = '0';
        } else {
            for (index = first; index < split; index++) {
                out[used++] = (char)('0' + nibbles[start + index - pad]);
            }
        }
        if (scale > 0u) {
            out[used++] = '.';
            for (index = split; index < spelled; index++) {
                out[used++] =
                    (index < pad) ? '0' : (char)('0' + nibbles[start + index - pad]);
            }
        }
    }
    out[used] = 0;
    return out;
}

uint32_t zdw_read_dheader(zdw_reader *r) {
    uint32_t length = zdw_read_u32(r);
    if (r->status != ZDW_OK) {
        return 0u;
    }
    if ((size_t)length > zdw_reader_remaining(r)) {
        fail(r, ZDW_IMPOSSIBLE_LENGTH);
        return 0u;
    }
    return length;
}

/* The next four bytes without moving the cursor.
 *
 * A peek and not a read, for codes 5 to 7 below. */
static uint32_t peek_u32(zdw_reader *r) {
    size_t saved;
    uint32_t value;
    zdw_reader_align(r, 4);
    saved = r->at;
    value = zdw_read_u32(r);
    r->at = saved;
    return value;
}

zdw_emheader_read zdw_read_emheader(zdw_reader *r) {
    zdw_emheader_read header;
    uint32_t word = zdw_read_u32(r);
    size_t scale;
    header.member_id = word & 0x0fffffffu;
    header.length_code = (uint8_t)((word >> 28) & 0x7u);
    header.must_understand = (word >> 31) != 0u;
    header.next_int = 0u;
    header.has_next_int = false;
    header.extent = 0u;
    switch (header.length_code) {
    case 0:
        header.extent = 1u;
        break;
    case 1:
        header.extent = 2u;
        break;
    case 2:
        header.extent = 4u;
        break;
    case 3:
        header.extent = 8u;
        break;
    case 4:
        header.next_int = zdw_read_u32(r);
        header.has_next_int = true;
        header.extent = header.next_int;
        break;
    default:
        /* Peeked, not consumed: the member's own serialization begins
         * at this word, and it is the length. §7.4.3.4.2. */
        header.next_int = peek_u32(r);
        header.has_next_int = true;
        scale = header.length_code == 5u ? 1u : (header.length_code == 6u ? 4u : 8u);
        header.extent = (size_t)header.next_int * scale + 4u;
        break;
    }
    if (header.extent > zdw_reader_remaining(r)) {
        fail(r, ZDW_IMPOSSIBLE_LENGTH);
    }
    return header;
}

/* The low fourteen bits of a parameter id. §7.4.1.2 gives bit 14 to
 * must-understand and bit 15 to the implementation, so neither is part
 * of the id. */
#define ZDW_PID_ID_MASK 0x3fffu

/* Start a parameter's value, returning the origin it displaces. */
static size_t begin_parameter_value(zdw_reader *r) {
    size_t saved = r->origin;
    r->origin = r->at;
    return saved;
}

/* The setting that writes the flags the stream has, for a marker or a
 * sentinel whose flags are not the ones this reader is configured for.
 *
 * `--pid-flags` and not `--pl-cdr1-headers`: both positions are
 * `PidFlagPolicy`'s, the marker since eight recorded rows showed RTI
 * setting the bit under the specification's framing form. */
static const char *framing_for(bool is_marker, uint16_t found) {
    if ((found & (uint16_t)ZDW_PID_IMPL_EXTENSION) != 0u) {
        return "--pid-flags impl-extension-members";
    }
    if ((found & (uint16_t)ZDW_PID_MUST_UNDERSTAND) != 0u) {
        return "--pid-flags sentinel-must-understand";
    }
    return is_marker ? "--pid-flags bare"
                     : "--pid-flags bare or impl-extension-members";
}

zdw_pid_read zdw_read_pid(zdw_reader *r) {
    return zdw_read_pid_as(r, ZDW_PARAM_MEMBER);
}

zdw_pid_read zdw_read_pid_as(zdw_reader *r, zdw_parameter_role role) {
    zdw_pid_read header;
    uint16_t word;
    uint16_t short_length;
    uint16_t id;
    uint32_t member_header;
    size_t at;

    header.pid = 0;
    header.length = 0;
    header.end = false;
    header.must_understand = false;
    header.impl_extension = false;
    header.reserved = false;
    header.ignore = false;
    header.role = role;
    header.saved_origin = r->origin;

    zdw_reader_align(r, 4);
    word = zdw_read_u16(r);
    at = r->at;
    short_length = zdw_read_u16(r);
    if (r->status != ZDW_OK) {
        return header;
    }
    header.must_understand = (word & (uint16_t)ZDW_PID_MUST_UNDERSTAND) != 0u;
    /* Under a framing that puts `FLAG_IMPL_EXTENSION` on every member
     * id the bit no longer tells a vendor-private parameter from a user
     * member — OpenDDS writes `c0010004` for `@id(1) long a`. It is
     * reported absent there because under that framing it carries no
     * information, and reporting it would send every member down the
     * unknown-parameter path. */
    header.impl_extension =
        (word & (uint16_t)ZDW_PID_IMPL_EXTENSION) != 0u
        && (zdw_pid_flags_at(r->pl_cdr1.pid_flags, ZDW_POSITION_MEMBER)
            & (uint16_t)ZDW_PID_IMPL_EXTENSION) == 0u;
    id = (uint16_t)(word & (uint16_t)ZDW_PID_ID_MASK);

    if (id == (uint16_t)(ZDW_PID_EXTENDED & ZDW_PID_ID_MASK)) {
        /* The marker's flags are asked at
         * `ZDW_POSITION_EXTENDED_MARKER` and nowhere else: Cyclone and
         * RTI write `7f01` under two different framing forms while Fast
         * DDS and OpenDDS share a form and write `3f01` against
         * `ff01`. */
        uint16_t marker_flags =
            zdw_pid_flags_at(r->pl_cdr1.pid_flags, ZDW_POSITION_EXTENDED_MARKER);
        uint16_t found = (uint16_t)(word & (uint16_t)~ZDW_PID_ID_MASK);
        if (found != marker_flags) {
            fail_framing(r, at, framing_for(true, found));
            return header;
        }
        if (short_length != 8u) {
            fail(r, ZDW_OUT_OF_RANGE);
            return header;
        }
        /* The four flag bits are not part of the id. §7.4.1.2.1 splits
         * the `eMemberHeader` into `FLAG_1` to `FLAG_4` and a 28-bit
         * member id, and `FLAG_2` is the member's must-understand.
         *
         * Taking the whole word gave a Cyclone member declared
         * `@must_understand` an id of `0x40000001`, which matches no
         * member the model declares, and the member was silently
         * dropped. The marker's own bit — checked above — is set on
         * every Cyclone parameter and says nothing about the member. */
        member_header = zdw_read_u32(r);
        header.pid = member_header & ZDW_E_MEMBER_ID_MASK;
        header.must_understand =
            (member_header & ZDW_E_FLAG_MUST_UNDERSTAND) != 0u;
        /* FLAG_1 is FLAG_IMPL_EXTENSION here exactly as bit 15 is in the
         * short header. The extended form's id is 28 bits and has no
         * reserved range, which is why it is the form the ids above
         * 0x3F00 need. */
        header.impl_extension =
            (member_header & ZDW_E_FLAG_IMPL_EXTENSION) != 0u;
        at = r->at;
        header.length = zdw_read_u32(r);
        if (r->status == ZDW_OK && (size_t)header.length > zdw_reader_remaining(r)) {
            r->at = at;
            fail(r, ZDW_IMPOSSIBLE_LENGTH);
            return header;
        }
        header.saved_origin = begin_parameter_value(r);
        return header;
    }

    header.pid = id;
    header.length = short_length;
    header.end = id == (uint16_t)(ZDW_PID_LIST_END & ZDW_PID_ID_MASK);
    header.ignore = id == (uint16_t)ZDW_PID_IGNORE;
    /* The threshold is the framing's and not a constant: one
     * implementation carries the whole low fourteen bits. */
    header.reserved = !header.end && id > r->pl_cdr1.highest_short_id;
    if (header.end) {
        /* The sentinel's flags are asked at `ZDW_POSITION_SENTINEL` and
         * nowhere else. They are not the member's. */
        uint16_t sentinel_flags =
            zdw_pid_flags_at(r->pl_cdr1.pid_flags, ZDW_POSITION_SENTINEL);
        uint16_t found = (uint16_t)(word & (uint16_t)~ZDW_PID_ID_MASK);
        if (found != sentinel_flags) {
            fail_framing(r, at, framing_for(false, found));
        }
        return header;
    }
    if ((size_t)header.length > zdw_reader_remaining(r)) {
        fail(r, ZDW_IMPOSSIBLE_LENGTH);
        return header;
    }
    header.saved_origin = begin_parameter_value(r);
    return header;
}

bool zdw_pid_is_user_member(zdw_pid_read header) {
    return !header.impl_extension && !header.reserved && !header.ignore;
}

/* The declared length against what the configured framing would have
 * declared for the value that was just read.
 *
 * A value longer than the header allows is the malformed case and is
 * the caller's to refuse. This is the other one: a length that is well
 * formed and belongs to a different implementation. RTI rounds a
 * member's declared length up to the alignment where Fast DDS writes
 * the exact one, and `vendor_bytes.md` §2 records both for the same
 * `short`. A member whose value already fills its four-byte slot
 * reveals nothing either way, which is a property of the bytes.
 */
static void check_declared_length(zdw_reader *r, zdw_pid_read header) {
    size_t read = r->at >= r->origin ? r->at - r->origin : 0u;
    size_t padded = (read + 3u) & ~(size_t)3u;
    bool pads = header.role == ZDW_PARAM_OPTIONAL
                    ? r->pl_cdr1.pad_declared_optional_length
                    : r->pl_cdr1.pad_declared_length;
    size_t expected = pads ? padded : read;
    size_t other = pads ? read : padded;
    if ((size_t)header.length == expected) {
        return;
    }
    if ((size_t)header.length != other) {
        fail(r, ZDW_OUT_OF_RANGE);
        return;
    }
    fail_framing(r, r->at,
                 pads ? "--pl-cdr1-headers short-true-length"
                      : "--pl-cdr1-headers short-rounded-length or always-extended");
}

void zdw_finish_pid_read(zdw_reader *r, zdw_pid_read header) {
    if (!header.end) {
        check_declared_length(r, header);
    }
    r->origin = header.saved_origin;
    /* Only under a framing that writes the padding, mirroring
     * `zdw_finish_pid` in writer.c and `Reader::finish_pid` in
     * crates/wire/src/reader.rs. Under Fast DDS's and OpenDDS's framing a
     * parameter writes no padding of its own — the next header aligns itself
     * in `zdw_read_pid_as` — and a *final* `@optional` parameter of a
     * non-mutable type ends the stream where its value does. Aligning
     * unconditionally read two bytes past the end of a stream the writer had
     * closed correctly.
     *
     * Measured on the `annotation-must-understand-unannotated` cells of
     * `zerodds-c` under the OpenDDS dialect, XCDR1, both byte orders:
     * 22 octets written, and this alignment asked for 24 —
     * "at 22: fewer bytes remain than the read needs". The Rust, C++ and
     * Python columns read the same 22 octets back without complaint, because
     * the Rust reader had already learned the distinction; the C runtime
     * never got it. */
    if (r->pl_cdr1.pad_after_value) {
        zdw_reader_align(r, 4);
    }
}

int64_t zdw_read_enum(zdw_reader *r, uint16_t bit_bound) {
    uint64_t raw;
    uint64_t sign_bit;
    if (bit_bound == 0u || bit_bound > 32u) {
        fail(r, ZDW_BAD_ARGUMENT);
        return 0;
    }
    if (bit_bound <= 8u) {
        raw = zdw_read_u8(r);
    } else if (bit_bound <= 16u) {
        raw = zdw_read_u16(r);
    } else {
        raw = zdw_read_u32(r);
    }
    /* The holder is signed (§7.4.3.5.3, ENUM_TYPE), so a pattern with
     * the top bit of the bound set reads back negative. */
    sign_bit = (uint64_t)1 << (bit_bound - 1u);
    if ((raw & sign_bit) != 0u) {
        uint64_t span = bit_bound >= 64u ? 0u : ((uint64_t)1 << bit_bound);
        return (int64_t)raw - (int64_t)span;
    }
    return (int64_t)raw;
}

uint64_t zdw_read_bitmask(zdw_reader *r, uint16_t bit_bound) {
    if (bit_bound == 0u || bit_bound > 64u) {
        fail(r, ZDW_BAD_ARGUMENT);
        return 0u;
    }
    if (bit_bound <= 8u) {
        return zdw_read_u8(r);
    }
    if (bit_bound <= 16u) {
        return zdw_read_u16(r);
    }
    if (bit_bound <= 32u) {
        return zdw_read_u32(r);
    }
    return zdw_read_u64(r);
}

uint64_t zdw_read_bitset(zdw_reader *r, uint32_t anchor_bytes) {
    switch (anchor_bytes) {
    case 1:
        return zdw_read_u8(r);
    case 2:
        return zdw_read_u16(r);
    case 4:
        return zdw_read_u32(r);
    case 8:
        return zdw_read_u64(r);
    default:
        fail(r, ZDW_BAD_ARGUMENT);
        return 0u;
    }
}
