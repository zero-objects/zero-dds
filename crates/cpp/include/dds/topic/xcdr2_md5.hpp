// SPDX-License-Identifier: Apache-2.0 OR Unlicense
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/xcdr2_md5.hpp -- RFC 1321 MD5 for key hashing.
//
// Public-domain implementation. Header-only, no external dependency.
// Used exclusively for XTypes 1.3 §7.6.8 PlainCdr2BeKeyHolder.
// Not for cryptographic purposes.

#ifndef ZERODDS_DDS_TOPIC_XCDR2_MD5_HPP
#define ZERODDS_DDS_TOPIC_XCDR2_MD5_HPP

#include <array>
#include <cstdint>
#include <cstring>
#include <vector>

namespace dds {
namespace topic {
namespace xcdr2_md5 {

namespace detail {

inline uint32_t md5_f(uint32_t x, uint32_t y, uint32_t z) { return (x & y) | (~x & z); }
inline uint32_t md5_g(uint32_t x, uint32_t y, uint32_t z) { return (x & z) | (y & ~z); }
inline uint32_t md5_h(uint32_t x, uint32_t y, uint32_t z) { return x ^ y ^ z; }
inline uint32_t md5_i(uint32_t x, uint32_t y, uint32_t z) { return y ^ (x | ~z); }

inline uint32_t rotl(uint32_t x, uint32_t n) {
    return (x << n) | (x >> (32 - n));
}

// RFC 1321 sine-table (T[i] = floor(2^32 * |sin(i+1)|)).
inline const uint32_t* md5_t() {
    static const uint32_t T[64] = {
        0xd76aa478u, 0xe8c7b756u, 0x242070dbu, 0xc1bdceeeu,
        0xf57c0fafu, 0x4787c62au, 0xa8304613u, 0xfd469501u,
        0x698098d8u, 0x8b44f7afu, 0xffff5bb1u, 0x895cd7beu,
        0x6b901122u, 0xfd987193u, 0xa679438eu, 0x49b40821u,
        0xf61e2562u, 0xc040b340u, 0x265e5a51u, 0xe9b6c7aau,
        0xd62f105du, 0x02441453u, 0xd8a1e681u, 0xe7d3fbc8u,
        0x21e1cde6u, 0xc33707d6u, 0xf4d50d87u, 0x455a14edu,
        0xa9e3e905u, 0xfcefa3f8u, 0x676f02d9u, 0x8d2a4c8au,
        0xfffa3942u, 0x8771f681u, 0x6d9d6122u, 0xfde5380cu,
        0xa4beea44u, 0x4bdecfa9u, 0xf6bb4b60u, 0xbebfbc70u,
        0x289b7ec6u, 0xeaa127fau, 0xd4ef3085u, 0x04881d05u,
        0xd9d4d039u, 0xe6db99e5u, 0x1fa27cf8u, 0xc4ac5665u,
        0xf4292244u, 0x432aff97u, 0xab9423a7u, 0xfc93a039u,
        0x655b59c3u, 0x8f0ccc92u, 0xffeff47du, 0x85845dd1u,
        0x6fa87e4fu, 0xfe2ce6e0u, 0xa3014314u, 0x4e0811a1u,
        0xf7537e82u, 0xbd3af235u, 0x2ad7d2bbu, 0xeb86d391u,
    };
    return T;
}

inline void process_block(uint32_t state[4], const uint8_t block[64]) {
    uint32_t M[16];
    for (int i = 0; i < 16; ++i) {
        M[i] = static_cast<uint32_t>(block[i * 4 + 0])
             | (static_cast<uint32_t>(block[i * 4 + 1]) << 8)
             | (static_cast<uint32_t>(block[i * 4 + 2]) << 16)
             | (static_cast<uint32_t>(block[i * 4 + 3]) << 24);
    }

    uint32_t a = state[0], b = state[1], c = state[2], d = state[3];
    const uint32_t* T = md5_t();

    static const uint32_t S1[4] = {7, 12, 17, 22};
    static const uint32_t S2[4] = {5, 9, 14, 20};
    static const uint32_t S3[4] = {4, 11, 16, 23};
    static const uint32_t S4[4] = {6, 10, 15, 21};

    // Round 1
    for (int i = 0; i < 16; ++i) {
        uint32_t f = md5_f(b, c, d);
        uint32_t g = static_cast<uint32_t>(i);
        uint32_t tmp = d;
        d = c;
        c = b;
        b = b + rotl(a + f + M[g] + T[i], S1[i % 4]);
        a = tmp;
    }
    // Round 2
    for (int i = 16; i < 32; ++i) {
        uint32_t f = md5_g(b, c, d);
        uint32_t g = (5 * static_cast<uint32_t>(i) + 1) % 16;
        uint32_t tmp = d;
        d = c;
        c = b;
        b = b + rotl(a + f + M[g] + T[i], S2[(i - 16) % 4]);
        a = tmp;
    }
    // Round 3
    for (int i = 32; i < 48; ++i) {
        uint32_t f = md5_h(b, c, d);
        uint32_t g = (3 * static_cast<uint32_t>(i) + 5) % 16;
        uint32_t tmp = d;
        d = c;
        c = b;
        b = b + rotl(a + f + M[g] + T[i], S3[(i - 32) % 4]);
        a = tmp;
    }
    // Round 4
    for (int i = 48; i < 64; ++i) {
        uint32_t f = md5_i(b, c, d);
        uint32_t g = (7 * static_cast<uint32_t>(i)) % 16;
        uint32_t tmp = d;
        d = c;
        c = b;
        b = b + rotl(a + f + M[g] + T[i], S4[(i - 48) % 4]);
        a = tmp;
    }

    state[0] += a;
    state[1] += b;
    state[2] += c;
    state[3] += d;
}

} // namespace detail

/// MD5 over `data` (RFC 1321). 16-byte digest, in reading order.
inline std::array<uint8_t, 16> md5(const uint8_t* data, size_t len) {
    uint32_t state[4] = {
        0x67452301u, 0xefcdab89u, 0x98badcfeu, 0x10325476u,
    };

    // Process all complete 64-byte blocks.
    size_t pos = 0;
    while (pos + 64 <= len) {
        detail::process_block(state, data + pos);
        pos += 64;
    }

    // Final block(s): pad with 0x80, zeros, then 8-byte little-endian bit-length.
    uint8_t buf[128] = {0};
    size_t rem = len - pos;
    std::memcpy(buf, data + pos, rem);
    buf[rem] = 0x80;

    size_t pad_end;
    if (rem + 1 + 8 <= 64) {
        pad_end = 64;
    } else {
        pad_end = 128;
    }

    uint64_t bit_len = static_cast<uint64_t>(len) * 8u;
    for (int i = 0; i < 8; ++i) {
        buf[pad_end - 8 + i] = static_cast<uint8_t>((bit_len >> (8 * i)) & 0xff);
    }

    detail::process_block(state, buf);
    if (pad_end == 128) {
        detail::process_block(state, buf + 64);
    }

    std::array<uint8_t, 16> out{};
    for (int i = 0; i < 4; ++i) {
        out[i * 4 + 0] = static_cast<uint8_t>(state[i] & 0xff);
        out[i * 4 + 1] = static_cast<uint8_t>((state[i] >> 8) & 0xff);
        out[i * 4 + 2] = static_cast<uint8_t>((state[i] >> 16) & 0xff);
        out[i * 4 + 3] = static_cast<uint8_t>((state[i] >> 24) & 0xff);
    }
    return out;
}

inline std::array<uint8_t, 16> md5(const std::vector<uint8_t>& data) {
    return md5(data.data(), data.size());
}

} // namespace xcdr2_md5
} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_XCDR2_MD5_HPP
