/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The exact decimal expansion of a binary128, which is the canonical
 * form the interop corpus compares columns in.
 *
 * Every finite binary value has finitely many decimal digits, because
 * `m * 2^-k` is `m * 5^k / 10^k` and `m * 5^k` is an integer. So this
 * needs no rounding and no search — one long multiplication and one
 * digit extraction — which is exactly why the corpus canonicalises to
 * the exact form and not the shortest one. Four columns that each
 * implemented a shortest-form search would disagree about which of two
 * equally short spellings to emit long before they disagreed about a
 * value, and the suite would read that as a wire difference.
 *
 * `crates/wire-probe/src/decode.rs` states the same rule for the Rust
 * column, and `crates/float128`'s `to_exact_decimal` is the same
 * algorithm written from the same clause.
 *
 * A separate translation unit from `float128.c`: the working buffers are
 * a few kilobytes and an embedded target that only moves samples should
 * not have to carry them. Nothing here allocates either — the buffers
 * are automatic, sized for the worst case binary128 has, and the caller
 * still owns the output.
 */
#include "zerodds_wire.h"

#include <string.h>

/* The smallest subnormal is `2^-16494`, whose exact decimal needs
 * `16494 * log2(5)` bits before the digits come out: about 38 300, plus
 * the significand's 113. 1250 limbs of 32 bits is 40 000, with room. */
#define F128_LIMBS 1250
/* `5^16494` has 11 530 digits, and the point sits 16 494 places from the
 * right, so the longest expansion is "0." and 16 494 digits. */
#define F128_MAX_DIGITS 16512

/* A non-negative integer, little-endian in 32-bit limbs.
 *
 * Thirty-two-bit limbs so that a limb times a small factor fits a
 * `uint64_t` with no 128-bit intermediate to get wrong.
 */
typedef struct f128_big {
    uint32_t limb[F128_LIMBS];
    /* How many limbs are in use; never counts a leading zero limb. */
    size_t used;
} f128_big;

static void big_zero(f128_big *b) {
    b->used = 0u;
    memset(b->limb, 0, sizeof b->limb);
}

static void big_trim(f128_big *b) {
    while (b->used > 0u && b->limb[b->used - 1u] == 0u) {
        b->used--;
    }
}

/* `b = b * factor`. Silently saturating is not an option here: a caller
 * that overran the buffer would print a wrong number, so the overflow
 * returns false and the printer refuses. */
static bool big_mul_small(f128_big *b, uint32_t factor) {
    uint64_t carry = 0u;
    size_t i;
    for (i = 0; i < b->used; i++) {
        uint64_t product = (uint64_t)b->limb[i] * (uint64_t)factor + carry;
        b->limb[i] = (uint32_t)(product & 0xffffffffu);
        carry = product >> 32;
    }
    while (carry != 0u) {
        if (b->used >= F128_LIMBS) {
            return false;
        }
        b->limb[b->used] = (uint32_t)(carry & 0xffffffffu);
        b->used++;
        carry >>= 32;
    }
    big_trim(b);
    return true;
}

/* `b /= divisor`, returning the remainder. */
static uint32_t big_div_small(f128_big *b, uint32_t divisor) {
    uint64_t remainder = 0u;
    size_t i;
    for (i = b->used; i > 0u; i--) {
        uint64_t current = (remainder << 32) | (uint64_t)b->limb[i - 1u];
        b->limb[i - 1u] = (uint32_t)(current / (uint64_t)divisor);
        remainder = current % (uint64_t)divisor;
    }
    big_trim(b);
    return (uint32_t)remainder;
}

/* `b <<= bits`. */
static bool big_shl(f128_big *b, unsigned bits) {
    unsigned whole = bits / 32u;
    unsigned part = bits % 32u;
    size_t i;
    if (b->used == 0u || bits == 0u) {
        return true;
    }
    if (part != 0u) {
        uint32_t carry = 0u;
        for (i = 0; i < b->used; i++) {
            uint64_t shifted = ((uint64_t)b->limb[i] << part) | (uint64_t)carry;
            b->limb[i] = (uint32_t)(shifted & 0xffffffffu);
            carry = (uint32_t)(shifted >> 32);
        }
        if (carry != 0u) {
            if (b->used >= F128_LIMBS) {
                return false;
            }
            b->limb[b->used] = carry;
            b->used++;
        }
    }
    if (whole != 0u) {
        if (b->used + (size_t)whole > F128_LIMBS) {
            return false;
        }
        for (i = b->used; i > 0u; i--) {
            b->limb[i - 1u + whole] = b->limb[i - 1u];
        }
        for (i = 0; i < whole; i++) {
            b->limb[i] = 0u;
        }
        b->used += whole;
    }
    big_trim(b);
    return true;
}

/* `b *= 5^power`, in the largest chunks that fit a limb. */
static bool big_mul_pow5(f128_big *b, unsigned power) {
    /* 5^13 = 1220703125 is the largest power of five below 2^32. */
    const uint32_t chunk = 1220703125u;
    unsigned left = power;
    while (left >= 13u) {
        if (!big_mul_small(b, chunk)) {
            return false;
        }
        left -= 13u;
    }
    if (left != 0u) {
        uint32_t factor = 1u;
        unsigned i;
        for (i = 0; i < left; i++) {
            factor *= 5u;
        }
        if (!big_mul_small(b, factor)) {
            return false;
        }
    }
    return true;
}

/* The decimal digits of `b`, most significant first, as ASCII into
 * `out`. Returns the count, or 0 for zero. */
static size_t big_digits(f128_big *b, char *out, size_t capacity) {
    /* 10^9 is the largest power of ten below 2^32, so one division
     * yields nine digits. */
    const uint32_t chunk = 1000000000u;
    char reversed[F128_MAX_DIGITS];
    size_t count = 0u;
    size_t i;
    if (b->used == 0u) {
        return 0u;
    }
    while (b->used != 0u) {
        uint32_t group = big_div_small(b, chunk);
        int k;
        for (k = 0; k < 9; k++) {
            if (count >= sizeof reversed) {
                return 0u;
            }
            reversed[count] = (char)('0' + (group % 10u));
            count++;
            group /= 10u;
            if (b->used == 0u && group == 0u) {
                break;
            }
        }
    }
    if (count > capacity) {
        return 0u;
    }
    for (i = 0; i < count; i++) {
        out[i] = reversed[count - 1u - i];
    }
    return count;
}

/* The significand and binary exponent, the same decomposition
 * `float128.c` uses; repeated here rather than exported because it is an
 * implementation detail of two files and not of the runtime's surface. */
static void f128_split(zdw_f128 value, uint64_t *hi, uint64_t *lo, int *exponent) {
    uint32_t biased = (uint32_t)((value.high >> 48) & 0x7fffu);
    *hi = value.high & 0x0000ffffffffffffULL;
    *lo = value.low;
    if (biased == 0u) {
        *exponent = 1 - 16383 - 112;
    } else {
        *hi |= 1ULL << 48;
        *exponent = (int)biased - 16383 - 112;
    }
}

static size_t f128_write_word(const char *word, char *out, size_t capacity) {
    size_t length = strlen(word);
    if (length + 1u > capacity) {
        return 0u;
    }
    memcpy(out, word, length + 1u);
    return length;
}

size_t zdw_f128_to_decimal(zdw_f128 value, char *out, size_t capacity) {
    f128_big big;
    char digits[F128_MAX_DIGITS];
    size_t digit_count;
    uint64_t hi;
    uint64_t lo;
    int exponent;
    int point;
    size_t written = 0u;
    bool negative;
    size_t i;

    if (out == NULL || capacity == 0u) {
        return 0u;
    }
    if (zdw_f128_is_nan(value)) {
        return f128_write_word("nan", out, capacity);
    }
    if (zdw_f128_is_infinite(value)) {
        return f128_write_word(zdw_f128_is_negative(value) ? "-inf" : "inf", out, capacity);
    }

    negative = zdw_f128_is_negative(value);
    f128_split(value, &hi, &lo, &exponent);
    if (hi == 0u && lo == 0u) {
        return f128_write_word(negative ? "-0.0" : "0.0", out, capacity);
    }

    big_zero(&big);
    big.limb[0] = (uint32_t)(lo & 0xffffffffu);
    big.limb[1] = (uint32_t)(lo >> 32);
    big.limb[2] = (uint32_t)(hi & 0xffffffffu);
    big.limb[3] = (uint32_t)(hi >> 32);
    big.used = 4u;
    big_trim(&big);

    if (exponent >= 0) {
        if (!big_shl(&big, (unsigned)exponent)) {
            out[0] = '\0';
            return 0u;
        }
        digit_count = big_digits(&big, digits, sizeof digits);
        if (digit_count == 0u) {
            out[0] = '\0';
            return 0u;
        }
        point = (int)digit_count;
    } else {
        /* m * 2^-k == m * 5^k * 10^-k. */
        unsigned scale = (unsigned)(-exponent);
        if (!big_mul_pow5(&big, scale)) {
            out[0] = '\0';
            return 0u;
        }
        digit_count = big_digits(&big, digits, sizeof digits);
        if (digit_count == 0u) {
            out[0] = '\0';
            return 0u;
        }
        point = (int)digit_count - (int)scale;
    }

    /* Trailing zeros are what the point already accounts for; every
     * binary fraction ends in a long tail of them because `5^k` does. */
    while (digit_count > 0u && digits[digit_count - 1u] == '0') {
        digit_count--;
    }
    if (digit_count == 0u) {
        /* Cannot happen for a non-zero value, but a printer that assumed
         * so and was wrong would emit an empty string. */
        return f128_write_word(negative ? "-0.0" : "0.0", out, capacity);
    }

    /* The same three forms `crates/float128`'s renderer writes, so the
     * two columns produce the same text and not merely the same value. */
/* A buffer that cannot hold the whole number gets an empty string and a
 * zero length — never a prefix. Half a number reads as a number. */
#define PUSH(ch)                                                                                   \
    do {                                                                                           \
        if (written + 1u >= capacity) {                                                            \
            out[0] = '\0';                                                                         \
            return 0u;                                                                             \
        }                                                                                          \
        out[written] = (ch);                                                                       \
        written++;                                                                                 \
    } while (0)

    if (negative) {
        PUSH('-');
    }
    if (point > 0 && point <= 34) {
        for (i = 0; i < digit_count; i++) {
            if ((int)i == point) {
                PUSH('.');
            }
            PUSH(digits[i]);
        }
        if ((int)digit_count <= point) {
            int pad;
            for (pad = (int)digit_count; pad < point; pad++) {
                PUSH('0');
            }
            PUSH('.');
            PUSH('0');
        }
    } else if (point <= 0 && point > -6) {
        int zeros;
        PUSH('0');
        PUSH('.');
        for (zeros = 0; zeros < -point; zeros++) {
            PUSH('0');
        }
        for (i = 0; i < digit_count; i++) {
            PUSH(digits[i]);
        }
    } else {
        char exponent_text[16];
        int exponent_value = point - 1;
        size_t length;
        PUSH(digits[0]);
        PUSH('.');
        if (digit_count == 1u) {
            PUSH('0');
        } else {
            for (i = 1; i < digit_count; i++) {
                PUSH(digits[i]);
            }
        }
        PUSH('e');
        if (exponent_value >= 0) {
            PUSH('+');
        } else {
            PUSH('-');
            exponent_value = -exponent_value;
        }
        length = 0u;
        do {
            exponent_text[length] = (char)('0' + (exponent_value % 10));
            length++;
            exponent_value /= 10;
        } while (exponent_value != 0 && length < sizeof exponent_text);
        while (length > 0u) {
            length--;
            PUSH(exponent_text[length]);
        }
    }
#undef PUSH

    out[written] = '\0';
    return written;
}
