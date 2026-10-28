/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * IEEE 754 binary128 for the C column: the conversions, none of the
 * arithmetic.
 *
 * The counterpart of `crates/float128`, written from the same clause
 * text rather than bound to it. A C column that called the Rust one
 * would agree with it by construction and prove nothing; the interop
 * suite exists to catch the case where two readings of §9.3.1.3 differ.
 *
 * Nothing here reads the bytes of a `long double`. The header says why
 * at length: on the three platforms this project builds for, that type
 * is three different formats, and two DDS vendors put whichever one
 * their compiler gave them straight onto the wire.
 */
#include "zerodds_wire.h"

#include <math.h>
#include <string.h>

/* Fields of binary128. IEEE 754-2008 clause 3.6. */
#define F128_FRACTION_BITS 112
#define F128_EXPONENT_BIAS 16383
#define F128_EXPONENT_MAX 0x7fffu
/* The fraction bits that live in the high half. */
#define F128_HIGH_FRACTION_BITS 48
#define F128_HIGH_FRACTION_MASK 0x0000ffffffffffffULL
#define F128_SIGN_BIT 0x8000000000000000ULL
/* Significand bits including the implied leading one. */
#define F128_SIGNIFICAND_BITS 113

zdw_f128 zdw_f128_from_parts(uint64_t high, uint64_t low) {
    zdw_f128 value;
    value.high = high;
    value.low = low;
    return value;
}

zdw_f128 zdw_f128_zero(void) { return zdw_f128_from_parts(0u, 0u); }

zdw_f128 zdw_f128_one(void) {
    return zdw_f128_from_parts((uint64_t)F128_EXPONENT_BIAS << F128_HIGH_FRACTION_BITS, 0u);
}

zdw_f128 zdw_f128_infinity(void) {
    return zdw_f128_from_parts((uint64_t)F128_EXPONENT_MAX << F128_HIGH_FRACTION_BITS, 0u);
}

zdw_f128 zdw_f128_quiet_nan(void) {
    /* The quiet bit is the top fraction bit, which is bit 47 of the
     * high half. IEEE 754-2008 clause 6.2.1. */
    return zdw_f128_from_parts(
        ((uint64_t)F128_EXPONENT_MAX << F128_HIGH_FRACTION_BITS) | (1ULL << 47), 0u);
}

/* -- the sixteen bytes ---------------------------------------------------- */

zdw_f128 zdw_f128_from_bytes(const uint8_t bytes[16], zdw_endianness order) {
    uint64_t high = 0u;
    uint64_t low = 0u;
    int i;
    if (order == ZDW_BIG) {
        for (i = 0; i < 8; i++) {
            high = (high << 8) | (uint64_t)bytes[i];
        }
        for (i = 8; i < 16; i++) {
            low = (low << 8) | (uint64_t)bytes[i];
        }
    } else {
        /* The whole sixteen bytes reversed, so the last byte carries the
         * sign and the top of the exponent. CORBA Figure 9.2. */
        for (i = 15; i >= 8; i--) {
            high = (high << 8) | (uint64_t)bytes[i];
        }
        for (i = 7; i >= 0; i--) {
            low = (low << 8) | (uint64_t)bytes[i];
        }
    }
    return zdw_f128_from_parts(high, low);
}

void zdw_f128_to_bytes(zdw_f128 value, zdw_endianness order, uint8_t out[16]) {
    int i;
    if (order == ZDW_BIG) {
        for (i = 0; i < 8; i++) {
            out[i] = (uint8_t)((value.high >> (8 * (7 - i))) & 0xffu);
            out[i + 8] = (uint8_t)((value.low >> (8 * (7 - i))) & 0xffu);
        }
    } else {
        for (i = 0; i < 8; i++) {
            out[i] = (uint8_t)((value.low >> (8 * i)) & 0xffu);
            out[i + 8] = (uint8_t)((value.high >> (8 * i)) & 0xffu);
        }
    }
}

/* -- classification ------------------------------------------------------- */

static uint32_t f128_biased_exponent(zdw_f128 value) {
    return (uint32_t)((value.high >> F128_HIGH_FRACTION_BITS) & F128_EXPONENT_MAX);
}

static bool f128_fraction_is_zero(zdw_f128 value) {
    return (value.high & F128_HIGH_FRACTION_MASK) == 0u && value.low == 0u;
}

bool zdw_f128_is_nan(zdw_f128 value) {
    return f128_biased_exponent(value) == F128_EXPONENT_MAX && !f128_fraction_is_zero(value);
}

bool zdw_f128_is_infinite(zdw_f128 value) {
    return f128_biased_exponent(value) == F128_EXPONENT_MAX && f128_fraction_is_zero(value);
}

bool zdw_f128_is_finite(zdw_f128 value) {
    return f128_biased_exponent(value) != F128_EXPONENT_MAX;
}

bool zdw_f128_is_zero(zdw_f128 value) {
    return f128_biased_exponent(value) == 0u && f128_fraction_is_zero(value);
}

bool zdw_f128_is_negative(zdw_f128 value) { return (value.high & F128_SIGN_BIT) != 0u; }

bool zdw_f128_same_bits(zdw_f128 a, zdw_f128 b) {
    return a.high == b.high && a.low == b.low;
}

/* -- the significand, as a 113-bit integer in two halves ------------------
 *
 * `zdw_f128_parts` is what every conversion here works in: a sign, an
 * integral significand `hi:lo` with the implied leading one materialised
 * for a normal value, and a binary exponent. Writing each conversion
 * against the encoded fields directly is how a subnormal case gets
 * forgotten in one of them.
 */
typedef struct f128_parts {
    bool negative;
    uint64_t hi; /* bits 64..112 of the significand */
    uint64_t lo; /* bits 0..63 */
    int exponent;
} f128_parts;

static f128_parts f128_decompose(zdw_f128 value) {
    f128_parts parts;
    uint32_t biased = f128_biased_exponent(value);
    parts.negative = zdw_f128_is_negative(value);
    parts.hi = value.high & F128_HIGH_FRACTION_MASK;
    parts.lo = value.low;
    if (biased == 0u) {
        /* Subnormal and zero share the smallest normal's exponent, which
         * is what makes the two encodings continuous. */
        parts.exponent = 1 - F128_EXPONENT_BIAS - F128_FRACTION_BITS;
    } else {
        parts.hi |= 1ULL << F128_HIGH_FRACTION_BITS;
        parts.exponent = (int)biased - F128_EXPONENT_BIAS - F128_FRACTION_BITS;
    }
    return parts;
}

/* Where the highest set bit is, counted from zero; -1 for zero. */
static int f128_bit_length(uint64_t hi, uint64_t lo) {
    int i;
    for (i = 63; i >= 0; i--) {
        if ((hi >> i) & 1u) {
            return 64 + i;
        }
    }
    for (i = 63; i >= 0; i--) {
        if ((lo >> i) & 1u) {
            return i;
        }
    }
    return -1;
}

/* Shift a 128-bit `hi:lo` right by `shift`, rounding to nearest with ties
 * to even. Sets `*lost` when anything was discarded. */
static void f128_shift_round(uint64_t *hi, uint64_t *lo, unsigned shift, bool *lost) {
    uint64_t dropped_any = 0u;
    uint64_t half_bit;
    unsigned k;
    if (shift == 0u) {
        return;
    }
    if (shift >= 128u) {
        *lost = *lost || *hi != 0u || *lo != 0u;
        *hi = 0u;
        *lo = 0u;
        return;
    }
    /* Note the round bit before the shift destroys it. */
    half_bit = 0u;
    for (k = 0; k < shift; k++) {
        uint64_t bit = (k < 64u) ? ((*lo >> k) & 1u) : ((*hi >> (k - 64u)) & 1u);
        if (k + 1u == shift) {
            half_bit = bit;
        } else if (bit != 0u) {
            dropped_any = 1u;
        }
    }
    if (shift >= 64u) {
        *lo = (shift == 64u) ? *hi : (*hi >> (shift - 64u));
        *hi = 0u;
    } else {
        *lo = (*lo >> shift) | (*hi << (64u - shift));
        *hi = *hi >> shift;
    }
    if (half_bit == 0u && dropped_any == 0u) {
        return;
    }
    *lost = true;
    if (half_bit != 0u && (dropped_any != 0u || (*lo & 1u) != 0u)) {
        uint64_t before = *lo;
        *lo = *lo + 1u;
        if (*lo < before) {
            *hi = *hi + 1u;
        }
    }
}

/* Build a value from a sign, an integral significand and a binary
 * exponent, rounding to nearest with ties to even. Sets `*lost` when the
 * rounding discarded anything, an overflow to infinity included. */
static zdw_f128 f128_compose(bool negative, uint64_t hi, uint64_t lo, int exponent, bool *lost) {
    int leading;
    int biased;
    uint64_t sign = negative ? F128_SIGN_BIT : 0u;

    if (hi == 0u && lo == 0u) {
        return zdw_f128_from_parts(sign, 0u);
    }
    leading = f128_bit_length(hi, lo);
    if (leading > F128_FRACTION_BITS) {
        unsigned drop = (unsigned)(leading - F128_FRACTION_BITS);
        f128_shift_round(&hi, &lo, drop, lost);
        exponent += (int)drop;
        /* Rounding up can carry into the next binade. */
        if (f128_bit_length(hi, lo) > F128_FRACTION_BITS) {
            bool ignored = false;
            f128_shift_round(&hi, &lo, 1u, &ignored);
            exponent += 1;
        }
    } else {
        unsigned shift = (unsigned)(F128_FRACTION_BITS - leading);
        unsigned k;
        for (k = 0; k < shift; k++) {
            hi = (hi << 1) | (lo >> 63);
            lo = lo << 1;
        }
        exponent -= (int)shift;
    }

    biased = exponent + F128_EXPONENT_BIAS + F128_FRACTION_BITS;
    if (biased >= (int)F128_EXPONENT_MAX) {
        *lost = true;
        return zdw_f128_from_parts(sign | ((uint64_t)F128_EXPONENT_MAX << F128_HIGH_FRACTION_BITS),
                                   0u);
    }
    if (biased <= 0) {
        /* Subnormal. A value that rounds up to the smallest normal
         * carries out of the fraction into the exponent field on its
         * own, which is why the high half is not masked here. */
        unsigned drop = (unsigned)(1 - biased);
        f128_shift_round(&hi, &lo, drop, lost);
        return zdw_f128_from_parts(sign | hi, lo);
    }
    return zdw_f128_from_parts(sign | ((uint64_t)biased << F128_HIGH_FRACTION_BITS) |
                                   (hi & F128_HIGH_FRACTION_MASK),
                               lo);
}

/* -- double --------------------------------------------------------------- */

zdw_f128 zdw_f128_from_double(double value) {
    uint64_t bits;
    uint64_t fraction;
    uint32_t biased;
    bool negative;
    bool lost = false;
    memcpy(&bits, &value, sizeof bits);
    negative = (bits >> 63) != 0u;
    biased = (uint32_t)((bits >> 52) & 0x7ffu);
    fraction = bits & 0x000fffffffffffffULL;

    if (biased == 0x7ffu) {
        uint64_t high = (negative ? F128_SIGN_BIT : 0u) |
                        ((uint64_t)F128_EXPONENT_MAX << F128_HIGH_FRACTION_BITS);
        if (fraction == 0u) {
            return zdw_f128_from_parts(high, 0u);
        }
        /* A NaN keeps its payload, aligned at the top of the fraction so
         * that the double's quiet bit lands on binary128's. The shift is
         * 60; the top 48 bits of the result live in the high half. */
        return zdw_f128_from_parts(high | ((fraction >> 4) & F128_HIGH_FRACTION_MASK),
                                   fraction << 60);
    }
    if (biased == 0u && fraction == 0u) {
        return zdw_f128_from_parts(negative ? F128_SIGN_BIT : 0u, 0u);
    }
    if (biased == 0u) {
        /* A subnormal double becomes a binary128 *normal*: binary128's
         * exponent range swallows binary64's whole subnormal range. */
        return f128_compose(negative, 0u, fraction, 1 - 1023 - 52, &lost);
    }
    return f128_compose(negative, 0u, fraction | (1ULL << 52), (int)biased - 1023 - 52, &lost);
}

zdw_f128 zdw_f128_from_float(float value) { return zdw_f128_from_double((double)value); }

bool zdw_f128_to_double(zdw_f128 value, double *out) {
    f128_parts parts;
    int leading;
    int unbiased;
    int target;
    uint64_t hi;
    uint64_t lo;
    uint64_t bits;
    bool lost = false;
    uint64_t sign;

    if (zdw_f128_is_nan(value)) {
        uint64_t payload = ((value.high & F128_HIGH_FRACTION_MASK) << 4) | (value.low >> 60);
        if (payload == 0u) {
            payload = 1u;
        }
        bits = (zdw_f128_is_negative(value) ? (1ULL << 63) : 0u) | (0x7ffULL << 52) |
               (payload & 0x000fffffffffffffULL);
        memcpy(out, &bits, sizeof bits);
        return (value.low & 0x0fffffffffffffffULL) == 0u;
    }
    if (zdw_f128_is_infinite(value)) {
        bits = (zdw_f128_is_negative(value) ? (1ULL << 63) : 0u) | (0x7ffULL << 52);
        memcpy(out, &bits, sizeof bits);
        return true;
    }

    parts = f128_decompose(value);
    sign = parts.negative ? (1ULL << 63) : 0u;
    if (parts.hi == 0u && parts.lo == 0u) {
        memcpy(out, &sign, sizeof sign);
        return true;
    }
    leading = f128_bit_length(parts.hi, parts.lo);
    unbiased = parts.exponent + leading;
    target = unbiased + 1023;
    hi = parts.hi;
    lo = parts.lo;

    if (target >= 0x7ff) {
        bits = sign | (0x7ffULL << 52);
        memcpy(out, &bits, sizeof bits);
        return false;
    }
    if (target <= 0) {
        int drop = leading - 52 + (1 - target);
        if (drop < 0) {
            drop = 0;
        }
        f128_shift_round(&hi, &lo, (unsigned)drop, &lost);
        bits = sign | lo;
        memcpy(out, &bits, sizeof bits);
        return !lost;
    }
    if (leading > 52) {
        f128_shift_round(&hi, &lo, (unsigned)(leading - 52), &lost);
        if (f128_bit_length(hi, lo) > 52) {
            bool ignored = false;
            f128_shift_round(&hi, &lo, 1u, &ignored);
            target += 1;
            if (target >= 0x7ff) {
                bits = sign | (0x7ffULL << 52);
                memcpy(out, &bits, sizeof bits);
                return false;
            }
        }
    } else {
        lo = lo << (52 - leading);
    }
    bits = sign | ((uint64_t)target << 52) | (lo & 0x000fffffffffffffULL);
    memcpy(out, &bits, sizeof bits);
    return !lost;
}

/* -- the platform's `long double`, converted and never copied ------------- */

zdw_f128 zdw_f128_from_long_double(long double value) {
    int exponent;
    long double mantissa;
    long double scaled;
    long double remainder;
    uint64_t hi;
    uint64_t lo;
    bool negative;
    bool lost = false;

    /* `isnan` and `isinf` are the type-generic macros, so they answer for
     * a `long double` on every platform without a cast that would narrow
     * the value before the question is asked. */
    if (isnan(value)) {
        return zdw_f128_quiet_nan();
    }
    if (isinf(value)) {
        zdw_f128 infinite = zdw_f128_infinity();
        if (value < 0.0L) {
            infinite.high |= F128_SIGN_BIT;
        }
        return infinite;
    }
    if (value == 0.0L) {
        /* `signbit` and not `value < 0`, because negative zero is not
         * less than zero and is a different sixteen bytes. */
        return zdw_f128_from_parts(signbit(value) ? F128_SIGN_BIT : 0u, 0u);
    }

    negative = value < 0.0L;
    mantissa = frexpl(negative ? -value : value, &exponent); /* 0.5 <= m < 1 */

    /* Every `long double` this project meets has 53, 64 or 113
     * significand bits, so `m * 2^113` is an integer in all three cases
     * and the extraction below is exact. A platform with a wider one
     * would lose the bits past 113 here, and binary128 could not hold
     * them either. */
    scaled = ldexpl(mantissa, F128_SIGNIFICAND_BITS);
    hi = (uint64_t)ldexpl(scaled, -64);
    remainder = scaled - ldexpl((long double)hi, 64);
    lo = (uint64_t)remainder;

    /* value = 0.m * 2^exponent = (hi:lo) * 2^(exponent - 113). */
    return f128_compose(negative, hi, lo, exponent - F128_SIGNIFICAND_BITS, &lost);
}

bool zdw_f128_to_long_double(zdw_f128 value, long double *out) {
    f128_parts parts;
    long double built;

    if (zdw_f128_is_nan(value)) {
        *out = (long double)NAN;
        return false;
    }
    if (zdw_f128_is_infinite(value)) {
        *out = zdw_f128_is_negative(value) ? -(long double)INFINITY : (long double)INFINITY;
        return true;
    }
    parts = f128_decompose(value);
    built = ldexpl((long double)parts.hi, 64) + (long double)parts.lo;
    built = ldexpl(built, parts.exponent);
    if (parts.negative) {
        built = -built;
    }
    *out = built;
    /* Exactness by measurement rather than by a table of platform
     * significand widths: convert back and compare the sixteen bytes. A
     * table would be wrong the first time this is built somewhere the
     * table's author did not have. */
    return zdw_f128_same_bits(zdw_f128_from_long_double(built), value);
}
