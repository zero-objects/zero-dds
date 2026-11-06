/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The oracle for `tests/gcc_binary128_ops.rs`: GCC's binary128 arithmetic.
 *
 * `__float128` addition, subtraction, multiplication and division on
 * x86-64 GCC are libgcc's soft-fp, correctly rounded to nearest with ties
 * to even — an implementation nobody here wrote, so it is the second
 * opinion for `src/arith.rs`.
 *
 * Reads `patterns.txt` on stdin (the first column, a 32-hex-digit bit
 * pattern) and writes one line per operand pair:
 *
 *     <a> <b> <a+b> <a-b> <a*b> <a/b>
 *
 * The pairs are every pattern with four neighbours in the list (the list
 * holds zeros, subnormals, the extremes and values around one), then
 * pseudo-random pairs from a fixed seed: random sign and fraction, the
 * exponents of the two operands drawn close to each other so that
 * cancellation and the halfway cases of the alignment are reached, and a
 * share drawn from the whole range so that overflow and the subnormal
 * results are reached.
 *
 *   gcc -O2 -o quad_ops quad_ops.c
 *   ./quad_ops < patterns.txt > x86_64-gcc-14.2-ops.txt
 */
#include <stdint.h>
#include <stdio.h>
#include <string.h>

typedef unsigned __int128 u128;

static u128 parse(const char *p) {
    u128 bits = 0;
    for (; *p != '\0'; p++) {
        unsigned digit;
        if (*p >= '0' && *p <= '9') {
            digit = (unsigned) (*p - '0');
        } else if (*p >= 'a' && *p <= 'f') {
            digit = (unsigned) (*p - 'a') + 10u;
        } else {
            break;
        }
        bits = (bits << 4) | digit;
    }
    return bits;
}

static void hex(u128 bits, char out[33]) {
    for (int i = 31; i >= 0; i--) {
        out[i] = "0123456789abcdef"[(unsigned) (bits & 15u)];
        bits >>= 4;
    }
    out[32] = '\0';
}

static u128 bits_of(__float128 value) {
    u128 bits;
    memcpy(&bits, &value, sizeof bits);
    return bits;
}

static __float128 value_of(u128 bits) {
    __float128 value;
    memcpy(&value, &bits, sizeof value);
    return value;
}

static void emit(u128 a, u128 b) {
    __float128 x = value_of(a);
    __float128 y = value_of(b);
    u128 results[6] = {a, b, bits_of(x + y), bits_of(x - y), bits_of(x * y), bits_of(x / y)};
    for (int i = 0; i < 6; i++) {
        char text[33];
        hex(results[i], text);
        printf(i == 5 ? "%s\n" : "%s ", text);
    }
}

/* xorshift64*, fixed seed: the same pairs on every run. */
static uint64_t state = 0x9E3779B97F4A7C15ull;
static uint64_t next(void) {
    state ^= state >> 12;
    state ^= state << 25;
    state ^= state >> 27;
    return state * 0x2545F4914F6CDD1Dull;
}

static u128 random_with_exponent(unsigned exponent) {
    u128 fraction = ((u128) next() << 64) | next();
    fraction &= (((u128) 1) << 112) - 1;
    u128 sign = (u128) (next() & 1u) << 127;
    return sign | ((u128) (exponent & 0x7fffu) << 112) | fraction;
}

int main(void) {
    static u128 patterns[4096];
    int count = 0;
    char line[4096];
    while (count < 4096 && fgets(line, sizeof line, stdin) != NULL) {
        char pattern[64];
        if (sscanf(line, "%63s", pattern) == 1) {
            patterns[count++] = parse(pattern);
        }
    }
    static const int neighbours[4] = {0, 1, 7, 31};
    for (int i = 0; i < count; i++) {
        for (int k = 0; k < 4; k++) {
            emit(patterns[i], patterns[(i + neighbours[k]) % count]);
        }
    }
    for (int i = 0; i < 4000; i++) {
        unsigned e = (unsigned) (next() % 0x7fffu);
        unsigned f;
        if (i % 4 == 3) {
            f = (unsigned) (next() % 0x7fffu);
        } else {
            int delta = (int) (next() % 241u) - 120;
            int g = (int) e + delta;
            f = g < 0 ? 0u : (g > 0x7ffe ? 0x7ffeu : (unsigned) g);
        }
        emit(random_with_exponent(e), random_with_exponent(f));
    }
    return 0;
}
