/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The digest the KeyHash uses.
 *
 * Not a security primitive here and not used as one: DDSI-RTPS 2.5
 * §9.6.3.8 names MD5 for this, and a different digest would produce
 * hashes no other vendor agrees with.
 *
 * Streaming over the caller's buffer rather than copying it: the write
 * path allocates nothing, and a key holder is not bounded by anything
 * this runtime could size a scratch buffer against.
 */
#include "zerodds_wire.h"

#include <string.h>

static const uint32_t S[64] = {
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9,  14, 20, 5, 9,  14, 20, 5, 9,  14, 20, 5, 9,  14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21};

static const uint32_t K[64] = {
    0xd76aa478u, 0xe8c7b756u, 0x242070dbu, 0xc1bdceeeu, 0xf57c0fafu,
    0x4787c62au, 0xa8304613u, 0xfd469501u, 0x698098d8u, 0x8b44f7afu,
    0xffff5bb1u, 0x895cd7beu, 0x6b901122u, 0xfd987193u, 0xa679438eu,
    0x49b40821u, 0xf61e2562u, 0xc040b340u, 0x265e5a51u, 0xe9b6c7aau,
    0xd62f105du, 0x02441453u, 0xd8a1e681u, 0xe7d3fbc8u, 0x21e1cde6u,
    0xc33707d6u, 0xf4d50d87u, 0x455a14edu, 0xa9e3e905u, 0xfcefa3f8u,
    0x676f02d9u, 0x8d2a4c8au, 0xfffa3942u, 0x8771f681u, 0x6d9d6122u,
    0xfde5380cu, 0xa4beea44u, 0x4bdecfa9u, 0xf6bb4b60u, 0xbebfbc70u,
    0x289b7ec6u, 0xeaa127fau, 0xd4ef3085u, 0x04881d05u, 0xd9d4d039u,
    0xe6db99e5u, 0x1fa27cf8u, 0xc4ac5665u, 0xf4292244u, 0x432aff97u,
    0xab9423a7u, 0xfc93a039u, 0x655b59c3u, 0x8f0ccc92u, 0xffeff47du,
    0x85845dd1u, 0x6fa87e4fu, 0xfe2ce6e0u, 0xa3014314u, 0x4e0811a1u,
    0xf7537e82u, 0xbd3af235u, 0x2ad7d2bbu, 0xeb86d391u};

static uint32_t rotate_left(uint32_t value, uint32_t by) {
    return (value << by) | (value >> (32u - by));
}

static void md5_block(uint32_t state[4], const uint8_t chunk[64]) {
    uint32_t words[16];
    uint32_t a = state[0];
    uint32_t b = state[1];
    uint32_t c = state[2];
    uint32_t d = state[3];
    int i;
    for (i = 0; i < 16; i++) {
        words[i] = (uint32_t)chunk[i * 4] | (uint32_t)chunk[i * 4 + 1] << 8 |
                   (uint32_t)chunk[i * 4 + 2] << 16 |
                   (uint32_t)chunk[i * 4 + 3] << 24;
    }
    for (i = 0; i < 64; i++) {
        uint32_t f;
        int g;
        uint32_t tmp;
        if (i < 16) {
            f = (b & c) | (~b & d);
            g = i;
        } else if (i < 32) {
            f = (d & b) | (~d & c);
            g = (5 * i + 1) % 16;
        } else if (i < 48) {
            f = b ^ c ^ d;
            g = (3 * i + 5) % 16;
        } else {
            f = c ^ (b | ~d);
            g = (7 * i) % 16;
        }
        tmp = d;
        d = c;
        c = b;
        b = b + rotate_left(a + f + K[i] + words[g], S[i]);
        a = tmp;
    }
    state[0] += a;
    state[1] += b;
    state[2] += c;
    state[3] += d;
}

void zdw_md5(const uint8_t *input, size_t length, uint8_t out[16]) {
    uint32_t state[4];
    uint8_t tail[128];
    size_t whole = length / 64u;
    size_t rest = length % 64u;
    size_t tail_len;
    uint64_t bits = (uint64_t)length * 8u;
    size_t i;

    state[0] = 0x67452301u;
    state[1] = 0xefcdab89u;
    state[2] = 0x98badcfeu;
    state[3] = 0x10325476u;

    for (i = 0; i < whole; i++) {
        md5_block(state, input + i * 64u);
    }

    memset(tail, 0, sizeof tail);
    if (rest > 0) {
        memcpy(tail, input + whole * 64u, rest);
    }
    tail[rest] = 0x80u;
    tail_len = rest + 1u <= 56u ? 64u : 128u;
    for (i = 0; i < 8; i++) {
        tail[tail_len - 8u + i] = (uint8_t)((bits >> (i * 8u)) & 0xffu);
    }
    md5_block(state, tail);
    if (tail_len == 128u) {
        md5_block(state, tail + 64u);
    }

    for (i = 0; i < 4; i++) {
        out[i * 4] = (uint8_t)(state[i] & 0xffu);
        out[i * 4 + 1] = (uint8_t)((state[i] >> 8) & 0xffu);
        out[i * 4 + 2] = (uint8_t)((state[i] >> 16) & 0xffu);
        out[i * 4 + 3] = (uint8_t)((state[i] >> 24) & 0xffu);
    }
}
