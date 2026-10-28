// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// MD5, RFC 1321, written out.
//
// DDSI-RTPS 2.5 §9.6.3.8 names MD5 for the KeyHash of a key holder longer
// than sixteen bytes. It is not a security claim: the KeyHash identifies an
// instance and is not a signature. It is here rather than pulled in from a
// crypto library for the reason every other clause in this runtime is —
// a column that borrowed it would prove nothing about whether this column
// hashes the same bytes the others do — and because a wire runtime that
// dragged in a TLS dependency to name an instance would be unlinkable on
// the targets this project cares about.
//
// The message is padded with a `0x80` byte, then zeros to 56 mod 64, then
// the bit length as a little-endian 64-bit word (§3.1, §3.2), and the four
// registers go out little-endian (§3.5).

#include "zerodds/wire.hpp"

#include <cstring>

namespace zerodds::wire {

namespace {

// T[i] = floor(2^32 * abs(sin(i + 1))), RFC 1321 §3.4.
constexpr std::uint32_t T[64] = {
    0xd76aa478u, 0xe8c7b756u, 0x242070dbu, 0xc1bdceeeu, 0xf57c0fafu, 0x4787c62au,
    0xa8304613u, 0xfd469501u, 0x698098d8u, 0x8b44f7afu, 0xffff5bb1u, 0x895cd7beu,
    0x6b901122u, 0xfd987193u, 0xa679438eu, 0x49b40821u, 0xf61e2562u, 0xc040b340u,
    0x265e5a51u, 0xe9b6c7aau, 0xd62f105du, 0x02441453u, 0xd8a1e681u, 0xe7d3fbc8u,
    0x21e1cde6u, 0xc33707d6u, 0xf4d50d87u, 0x455a14edu, 0xa9e3e905u, 0xfcefa3f8u,
    0x676f02d9u, 0x8d2a4c8au, 0xfffa3942u, 0x8771f681u, 0x6d9d6122u, 0xfde5380cu,
    0xa4beea44u, 0x4bdecfa9u, 0xf6bb4b60u, 0xbebfbc70u, 0x289b7ec6u, 0xeaa127fau,
    0xd4ef3085u, 0x04881d05u, 0xd9d4d039u, 0xe6db99e5u, 0x1fa27cf8u, 0xc4ac5665u,
    0xf4292244u, 0x432aff97u, 0xab9423a7u, 0xfc93a039u, 0x655b59c3u, 0x8f0ccc92u,
    0xffeff47du, 0x85845dd1u, 0x6fa87e4fu, 0xfe2ce6e0u, 0xa3014314u, 0x4e0811a1u,
    0xf7537e82u, 0xbd3af235u, 0x2ad7d2bbu, 0xeb86d391u,
};

// The per-round left rotations, §3.4.
constexpr unsigned S[64] = {
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9,  14, 20, 5, 9,  14, 20, 5, 9,  14, 20, 5, 9,  14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
};

constexpr std::uint32_t rotate_left(std::uint32_t value, unsigned bits) {
    return static_cast<std::uint32_t>((value << bits) | (value >> (32u - bits)));
}

void transform(std::uint32_t state[4], const std::uint8_t block[64]) {
    std::uint32_t words[16];
    for (std::size_t i = 0; i < 16; i++) {
        words[i] = static_cast<std::uint32_t>(block[4 * i]) |
                   static_cast<std::uint32_t>(block[4 * i + 1]) << 8 |
                   static_cast<std::uint32_t>(block[4 * i + 2]) << 16 |
                   static_cast<std::uint32_t>(block[4 * i + 3]) << 24;
    }
    std::uint32_t a = state[0];
    std::uint32_t b = state[1];
    std::uint32_t c = state[2];
    std::uint32_t d = state[3];
    for (unsigned i = 0; i < 64; i++) {
        std::uint32_t f = 0;
        unsigned g = 0;
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
        const std::uint32_t rotated =
            rotate_left(static_cast<std::uint32_t>(a + f + T[i] + words[g]), S[i]);
        a = d;
        d = c;
        c = b;
        b = static_cast<std::uint32_t>(b + rotated);
    }
    state[0] = static_cast<std::uint32_t>(state[0] + a);
    state[1] = static_cast<std::uint32_t>(state[1] + b);
    state[2] = static_cast<std::uint32_t>(state[2] + c);
    state[3] = static_cast<std::uint32_t>(state[3] + d);
}

}  // namespace

std::array<std::uint8_t, 16> md5(const std::uint8_t *data, std::size_t length) {
    // §3.3: the four registers, low-order byte first.
    std::uint32_t state[4] = {0x67452301u, 0xefcdab89u, 0x98badcfeu, 0x10325476u};

    std::size_t at = 0;
    while (length - at >= 64) {
        transform(state, data + at);
        at += 64;
    }

    // The tail, the `0x80` byte, the zeros and the bit length. Two blocks at
    // most, because the tail is under 64 bytes and the padding adds at most
    // 64 more.
    std::uint8_t tail[128] = {};
    const std::size_t rest = length - at;
    for (std::size_t i = 0; i < rest; i++) {
        tail[i] = data[at + i];
    }
    tail[rest] = 0x80u;
    const std::size_t padded = rest < 56 ? 64u : 128u;
    const std::uint64_t bits = static_cast<std::uint64_t>(length) * 8u;
    for (std::size_t i = 0; i < 8; i++) {
        tail[padded - 8 + i] = static_cast<std::uint8_t>((bits >> (8 * i)) & 0xFFu);
    }
    for (std::size_t i = 0; i < padded; i += 64) {
        transform(state, tail + i);
    }

    std::array<std::uint8_t, 16> out{};
    for (std::size_t i = 0; i < 4; i++) {
        for (std::size_t k = 0; k < 4; k++) {
            out[4 * i + k] = static_cast<std::uint8_t>((state[i] >> (8 * k)) & 0xFFu);
        }
    }
    return out;
}

std::array<std::uint8_t, 16> md5(std::string_view text) {
    return md5(reinterpret_cast<const std::uint8_t *>(text.data()), text.size());
}

}  // namespace zerodds::wire
