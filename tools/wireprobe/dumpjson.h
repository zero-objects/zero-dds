/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The eleven helpers `wireprobe-dump`'s output calls, and nothing else.
 *
 * A vendor probe's decode direction prints the sample its own
 * deserialiser produced as the canonical JSON of
 * `crates/wire-probe/src/canonical.rs`. The punctuation is emitted as
 * statements rather than as one format string per aggregate, because a
 * sequence's length, a union's selected member and an @optional member's
 * presence are all decided by the bytes; `zdj_key` is what puts the
 * comma in only when something has already been printed at that level.
 *
 * ## What is canonical here and what is left to the normaliser
 *
 * Two things have to be right in this file, because no reader can
 * recover them:
 *
 *   - a 64-bit integer is a **string**, since a JSON number is a double
 *     and loses the low bits past 2^53;
 *   - an absent @optional member is a key that is **not printed**, never
 *     a `null`.
 *
 * Everything about the *spelling* of a number is not: the floating
 * helpers print enough digits to round-trip — `%.9g` for a `float` and
 * `%.17g` for a `double`, which is what IEEE 754 requires for
 * twenty-four and fifty-three bits of mantissa — and the Rust side
 * reduces both to the shortest form that reads back the same value
 * before anything is compared. Reproducing Rust's float formatting in C
 * would be a second implementation of it to get wrong.
 *
 * ## Why the helpers are `static inline`
 *
 * A probe includes this header and calls the subset its own sample
 * needs; the rest are unreferenced, and `-Wunused-function` names every
 * one of them. Both gcc 14 and Apple clang do, so under `-Werror` the
 * file would not compile at all. `static inline` is the form neither
 * compiler warns about — and a generated probe that warns is one whose
 * warnings stop being read.
 *
 * ## Why the wide forms go out as \uXXXX
 *
 * A `wstring` on the wire is UTF-16 code units. Escaping each unit is
 * exactly what JSON's own escape is defined to carry — a code point
 * above the BMP is the surrogate pair, which any JSON reader joins —
 * so the probe needs no UTF-8 encoder and cannot get one wrong.
 */
#ifndef ZERODDS_WIREPROBE_DUMPJSON_H
#define ZERODDS_WIREPROBE_DUMPJSON_H

#include <stdio.h>
#include <stddef.h>

/* Longer than any wstring in the corpus. A sample that reached it would
   be truncated, so the helper says so rather than printing a shorter
   string that reads as the value. */
#define ZDJ_WSTRING_MAX 4096

static inline void zdj_raw(const char *text) { fputs(text, stdout); }

/* The separator and the key together: the comma belongs to the second
   and later members, and only the printer knows which those are. */
static inline void zdj_key(int *first, const char *name) {
    if (!*first) {
        fputs(",", stdout);
    }
    *first = 0;
    printf("\"%s\":", name);
}

static inline void zdj_bool(int value) { fputs(value ? "true" : "false", stdout); }

static inline void zdj_i64(long long value) { printf("%lld", value); }

static inline void zdj_u64(unsigned long long value) { printf("%llu", value); }

static inline void zdj_i64_string(long long value) { printf("\"%lld\"", value); }

static inline void zdj_u64_string(unsigned long long value) { printf("\"%llu\"", value); }

static inline void zdj_f32(float value) { printf("%.9g", (double)value); }

static inline void zdj_f64(double value) { printf("%.17g", value); }

/* A `long double` as a string holding every digit it has.
 *
 * Guarded on `ZERODDS_WIRE_H`, because the four *vendor* probes include
 * this header too and link no ZeroDDS runtime — an unconditional
 * declaration of `zdw_f128` would stop Cyclone's and RTI's probes
 * compiling for a function they never call. Only the ZeroDDS C column's
 * generated dumper includes `zerodds_wire.h` first, and only it calls
 * this.
 *
 * The canonical form for the type, and a string for the reason a 64-bit
 * integer is one: a JSON number is a double and 60 of a long double's
 * 113 significand bits do not fit one. `zdw_f128_to_decimal` is the
 * runtime's exact printer; a refusal — which only a buffer this function
 * sized wrongly could cause — comes out as `null` rather than as an
 * empty string, so it fails a comparison instead of matching a value
 * nobody wrote. */
#ifdef ZERODDS_WIRE_H
static inline void zdj_f128_string(zdw_f128 value) {
    static char text[ZDW_F128_DECIMAL_MAX];
    if (zdw_f128_to_decimal(value, text, sizeof text) == 0u) {
        fputs("null", stdout);
        return;
    }
    printf("\"%s\"", text);
}
#endif /* ZERODDS_WIRE_H */

/* One octet of an IDL `char`, as the one-character string the canonical
   form calls for. The escape is by code point, and an IDL `char` is one
   octet, so 0xE4 is U+00E4 — the same mapping the Rust reader makes. */
static inline void zdj_char(char value) {
    printf("\"\\u%04x\"", (unsigned)(unsigned char)value);
}

static inline void zdj_wchar(unsigned long value) {
    printf("\"\\u%04x\"", (unsigned)(value & 0xFFFFu));
}

/* A narrow string: UTF-8 on the wire and UTF-8 in JSON, so the bytes go
   through untouched and only the six characters JSON reserves are
   escaped. A null pointer prints `null`, which the canonical form
   refuses — that is the point: the member arrived unset and the
   comparison has to say so rather than substitute an empty string. */
static inline void zdj_string(const char *text) {
    if (text == NULL) {
        fputs("null", stdout);
        return;
    }
    fputc('"', stdout);
    for (; *text != '\0'; text++) {
        unsigned char byte = (unsigned char)*text;
        switch (byte) {
        case '"': fputs("\\\"", stdout); break;
        case '\\': fputs("\\\\", stdout); break;
        case '\n': fputs("\\n", stdout); break;
        case '\r': fputs("\\r", stdout); break;
        case '\t': fputs("\\t", stdout); break;
        default:
            if (byte < 0x20u) {
                printf("\\u%04x", (unsigned)byte);
            } else {
                fputc((int)byte, stdout);
            }
            break;
        }
    }
    fputc('"', stdout);
}

static inline void zdj_wstring(const unsigned short *units, size_t count) {
    size_t index;
    if (count >= ZDJ_WSTRING_MAX) {
        fputs("\"<wstring longer than the probe's buffer>\"", stdout);
        return;
    }
    fputc('"', stdout);
    for (index = 0; index < count; index++) {
        printf("\\u%04x", (unsigned)units[index]);
    }
    fputc('"', stdout);
}

/* A value that names no enumerator. Printed as a string no enumerator
   has, so the comparison refuses it by name — a bare number here would
   read as some other member's value and pass. */
static inline void zdj_enum_unknown(long long value) {
    printf("\"<enumerator %lld>\"", value);
}

#endif /* ZERODDS_WIREPROBE_DUMPJSON_H */
