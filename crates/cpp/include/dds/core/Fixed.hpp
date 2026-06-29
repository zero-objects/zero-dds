// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/Fixed.hpp — IDL `fixed<P,S>` decimal (IDL 4.2 §7.4.1.4.4).
//
// Wire format: packed binary-coded decimal (BCD), CORBA/GIOP §9.3.2.7
// (identical in XCDR2 §7.4.4.5). A direct C++ port of the oracle-validated
// reference `crates/cdr/src/fixed.rs` (byte-exact against JacORB 3.9 +
// omniORB 4.3). `fixed` is NOT an XTypes type (no TK_FIXED): it carries no
// TypeObject and the other DDS stacks reject it in a topic — ZeroDDS supports
// it as a CORBA-bridging differentiator across every language binding.
//
// - Nibbles big-endian `[d0 .. d(P-1)][sign]`; sign 0xC = positive/zero,
//   0xD = negative; a leading 0x0 pad nibble when `P+1` is odd.
// - Byte count = (P + 2) / 2, alignment 1, NO length prefix (P is static),
//   endian-independent (it is an octet array). `fixed<5,2>` of 123.45 is the
//   3 octets `12 34 5c`; `fixed<4,0>` of 1234 is `01 23 4c`.

#ifndef ZERODDS_DDS_CORE_FIXED_HPP
#define ZERODDS_DDS_CORE_FIXED_HPP

#include <array>
#include <cstddef>
#include <cstdint>
#include <stdexcept>
#include <string>

namespace dds {
namespace core {

/// IDL `fixed<P, S>`: `P` total decimal digits, `S` after the point.
/// Offers round-trip + string conversion (no decimal arithmetic), exactly
/// like the Rust reference — combine with a decimal library if arithmetic is
/// needed.
template <std::uint32_t P, std::uint32_t S>
class Fixed {
public:
    /// Packed-BCD byte count, `(P + 2) / 2`.
    static constexpr std::size_t kByteCount = (static_cast<std::size_t>(P) + 2) / 2;

    /// Default = positive zero (sign nibble 0xC in the last byte).
    Fixed() {
        digits_.fill(0);
        digits_[kByteCount - 1] = 0x0C;
    }

    /// Constructs from `kByteCount` raw BCD octets (decode path).
    explicit Fixed(const std::uint8_t* bcd) {
        for (std::size_t i = 0; i < kByteCount; ++i) {
            digits_[i] = bcd[i];
        }
    }

    /// Raw BCD storage (encode path: iterate begin()..end()).
    const std::array<std::uint8_t, kByteCount>& bcd_bytes() const { return digits_; }
    std::array<std::uint8_t, kByteCount>& bcd_bytes() { return digits_; }

    /// Parses a decimal string (e.g. "123.45", "-1.5"). Mirrors
    /// `Fixed::from_str_repr` in the Rust reference.
    ///
    /// @throws std::invalid_argument on non-digit input, or std::out_of_range
    ///         when the integer part exceeds `P-S` or the fraction exceeds `S`.
    static Fixed from_string(const std::string& s) {
        bool sign = true; // positive
        std::size_t start = 0;
        if (!s.empty() && (s[0] == '-' || s[0] == '+')) {
            sign = (s[0] != '-');
            start = 1;
        }
        std::string rest = s.substr(start);
        std::string int_part = rest;
        std::string frac_part;
        const auto dot = rest.find('.');
        if (dot != std::string::npos) {
            int_part = rest.substr(0, dot);
            frac_part = rest.substr(dot + 1);
        }
        const std::size_t total_p = P;
        const std::size_t total_s = S;
        const std::size_t int_needed = total_p - total_s;
        if (int_part.size() > int_needed) {
            throw std::out_of_range("fixed: integer part exceeds P-S");
        }
        if (frac_part.size() > total_s) {
            throw std::out_of_range("fixed: fractional part exceeds S");
        }
        std::string digits_buf;
        digits_buf.reserve(total_p);
        for (std::size_t i = int_part.size(); i < int_needed; ++i) {
            digits_buf.push_back('0');
        }
        digits_buf += int_part;
        digits_buf += frac_part;
        for (std::size_t i = frac_part.size(); i < total_s; ++i) {
            digits_buf.push_back('0');
        }
        // BCD nibbles big-endian: optional leading pad, P digits, sign nibble.
        std::array<std::uint8_t, 2 * kByteCount> nibbles{};
        std::size_t n = 0;
        if ((P + 1) % 2 == 1) {
            nibbles[n++] = 0;
        }
        for (char c : digits_buf) {
            if (c < '0' || c > '9') {
                throw std::invalid_argument("fixed: non-digit char");
            }
            nibbles[n++] = static_cast<std::uint8_t>(c - '0');
        }
        nibbles[n++] = sign ? 0x0C : 0x0D;
        // n is even by construction; pack high nibble first.
        Fixed out;
        for (std::size_t b = 0; b < kByteCount; ++b) {
            out.digits_[b] = static_cast<std::uint8_t>((nibbles[2 * b] << 4) | nibbles[2 * b + 1]);
        }
        return out;
    }

    /// Decimal string representation (e.g. "123.45"). Mirrors
    /// `Fixed::to_string_repr` in the Rust reference.
    std::string to_string() const {
        std::string chars;
        char sign = '+';
        for (std::size_t idx = 0; idx < kByteCount; ++idx) {
            const std::uint8_t hi = (digits_[idx] >> 4) & 0x0F;
            const std::uint8_t lo = digits_[idx] & 0x0F;
            if (idx == kByteCount - 1) {
                chars.push_back(static_cast<char>('0' + (hi % 10)));
                sign = (lo == 0x0D) ? '-' : '+';
            } else {
                chars.push_back(static_cast<char>('0' + (hi % 10)));
                chars.push_back(static_cast<char>('0' + (lo % 10)));
            }
        }
        // Trim leading zeros, keep at least S+1 digits.
        while (chars.size() > (static_cast<std::size_t>(S) + 1) && chars.front() == '0') {
            chars.erase(chars.begin());
        }
        std::string out;
        if (sign == '-') {
            out.push_back('-');
        }
        if (S > 0) {
            const std::size_t dot_pos =
                chars.size() > static_cast<std::size_t>(S) ? chars.size() - S : 0;
            for (std::size_t i = 0; i < chars.size(); ++i) {
                if (i == dot_pos) {
                    out.push_back('.');
                }
                out.push_back(chars[i]);
            }
        } else {
            out += chars;
        }
        return out;
    }

    bool operator==(const Fixed& rhs) const { return digits_ == rhs.digits_; }
    bool operator!=(const Fixed& rhs) const { return !(*this == rhs); }

private:
    std::array<std::uint8_t, kByteCount> digits_{};
};

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_FIXED_HPP
