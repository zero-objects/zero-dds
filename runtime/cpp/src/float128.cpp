// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// IEEE 754 binary128 for the C++ column: the conversions, none of the
// arithmetic.
//
// Written from the same clause text as `crates/float128`,
// `runtime/c/src/float128.c` and `runtime/python/zerodds_wire/float128.py`
// rather than bound to any of them. A column that called another would agree
// with it by construction and prove nothing; the interop suite exists to
// catch the case where two readings of [CDR] §9.3.1.3 differ.
//
// Nothing here reads the bytes of a `long double`. On the three platforms
// this project builds for that type is three different formats, and two DDS
// vendors put whichever one their compiler gave them straight onto the wire.
//
// The decimal conversions are exact in both directions. Every finite binary
// value has finitely many decimal digits, because `m * 2^-k` is
// `m * 5^k / 10^k` and `m * 5^k` is an integer — so printing needs no
// rounding and no search, and parsing needs one rounding and no `double` in
// the middle. A two-step parse through a `double` rounds twice, and double
// rounding gets the last bit wrong for exactly the values this type exists
// to carry.

#include "zerodds/wire.hpp"

#include <cstring>
#include <string>
#include <utility>
#include <vector>

namespace zerodds::wire {

namespace {

// Fields of binary128. IEEE 754-2008 clause 3.6.
constexpr int FRACTION_BITS = 112;
constexpr int EXPONENT_BIAS = 16383;
constexpr std::uint32_t EXPONENT_MAX = 0x7FFFu;
// The fraction bits that live in the high half.
constexpr int HIGH_FRACTION_BITS = 48;
constexpr std::uint64_t HIGH_FRACTION_MASK = 0x0000FFFFFFFFFFFFull;
constexpr std::uint64_t SIGN_BIT = 0x8000000000000000ull;

// The value of a decimal literal is only ever built exactly, so the parser
// keeps enough significant digits that the tie between two binary128 values
// is always decided by digits it kept. The longest exact expansion any
// binary128 has is 11 563 digits; anything past this bound can only move the
// value strictly away from a tie, which one sticky bit records.
constexpr std::size_t MAX_SIGNIFICANT_DIGITS = 20000;

// -- a 128-bit significand --------------------------------------------------

struct Wide {
    std::uint64_t hi = 0;
    std::uint64_t lo = 0;
};

/// Where the highest set bit is, counted from zero; -1 for zero.
int bit_index(const Wide &value) {
    for (int i = 63; i >= 0; i--) {
        if ((value.hi >> i) & 1u) {
            return 64 + i;
        }
    }
    for (int i = 63; i >= 0; i--) {
        if ((value.lo >> i) & 1u) {
            return i;
        }
    }
    return -1;
}

/// Shift right by `shift`, rounding to nearest with ties to even. Sets
/// `lost` when anything was discarded.
void shift_round(Wide &value, unsigned shift, bool &lost) {
    if (shift == 0) {
        return;
    }
    if (shift >= 128) {
        lost = lost || value.hi != 0 || value.lo != 0;
        value.hi = 0;
        value.lo = 0;
        return;
    }
    // Note the round bit before the shift destroys it.
    std::uint64_t half_bit = 0;
    bool dropped_any = false;
    for (unsigned k = 0; k < shift; k++) {
        const std::uint64_t bit = k < 64 ? ((value.lo >> k) & 1u)
                                         : ((value.hi >> (k - 64)) & 1u);
        if (k + 1 == shift) {
            half_bit = bit;
        } else if (bit != 0) {
            dropped_any = true;
        }
    }
    if (shift >= 64) {
        value.lo = shift == 64 ? value.hi : (value.hi >> (shift - 64));
        value.hi = 0;
    } else {
        value.lo = (value.lo >> shift) | (value.hi << (64 - shift));
        value.hi = value.hi >> shift;
    }
    if (half_bit == 0 && !dropped_any) {
        return;
    }
    lost = true;
    if (half_bit != 0 && (dropped_any || (value.lo & 1u) != 0)) {
        const std::uint64_t before = value.lo;
        value.lo = value.lo + 1u;
        if (value.lo < before) {
            value.hi = value.hi + 1u;
        }
    }
}

/// Build a value from a sign, an integral significand and a binary exponent,
/// rounding to nearest with ties to even. Sets `lost` when the rounding
/// discarded anything, an overflow to infinity included.
Float128 compose(bool negative, Wide significand, int exponent, bool &lost) {
    const std::uint64_t sign = negative ? SIGN_BIT : 0u;
    if (significand.hi == 0 && significand.lo == 0) {
        return Float128::from_parts(sign, 0);
    }
    // Clamped before anything is added: a decimal parser can hand this an
    // exponent of a hundred thousand.
    if (exponent > static_cast<int>(EXPONENT_MAX)) {
        lost = true;
        return Float128::infinity(negative);
    }
    if (exponent < -(FRACTION_BITS + EXPONENT_BIAS + 200)) {
        lost = true;
        return Float128::from_parts(sign, 0);
    }

    int leading = bit_index(significand);
    if (leading > FRACTION_BITS) {
        const unsigned drop = static_cast<unsigned>(leading - FRACTION_BITS);
        shift_round(significand, drop, lost);
        exponent += static_cast<int>(drop);
        // Rounding up can carry into the next binade.
        if (bit_index(significand) > FRACTION_BITS) {
            bool ignored = false;
            shift_round(significand, 1, ignored);
            exponent += 1;
        }
    } else {
        const unsigned shift = static_cast<unsigned>(FRACTION_BITS - leading);
        for (unsigned k = 0; k < shift; k++) {
            significand.hi = (significand.hi << 1) | (significand.lo >> 63);
            significand.lo = significand.lo << 1;
        }
        exponent -= static_cast<int>(shift);
    }

    const int biased = exponent + EXPONENT_BIAS + FRACTION_BITS;
    if (biased >= static_cast<int>(EXPONENT_MAX)) {
        lost = true;
        return Float128::infinity(negative);
    }
    if (biased <= 0) {
        // Subnormal. A value that rounds up to the smallest normal carries
        // out of the fraction into the exponent field on its own, which is
        // why the high half is not masked here.
        shift_round(significand, static_cast<unsigned>(1 - biased), lost);
        return Float128::from_parts(sign | significand.hi, significand.lo);
    }
    return Float128::from_parts(sign |
                                    (static_cast<std::uint64_t>(biased) << HIGH_FRACTION_BITS) |
                                    (significand.hi & HIGH_FRACTION_MASK),
                                significand.lo);
}

struct Parts {
    bool negative = false;
    Wide significand;
    int exponent = 0;
};

/// The sign, an integral significand with the implied leading one
/// materialised for a normal value, and a binary exponent. Writing each
/// conversion against the encoded fields directly is how a subnormal case
/// gets forgotten in one of them.
Parts decompose(const Float128 &value) {
    Parts parts;
    const std::uint32_t biased =
        static_cast<std::uint32_t>((value.high >> HIGH_FRACTION_BITS) & EXPONENT_MAX);
    parts.negative = value.is_negative();
    parts.significand.hi = value.high & HIGH_FRACTION_MASK;
    parts.significand.lo = value.low;
    if (biased == 0) {
        // Subnormal and zero share the smallest normal's exponent, which is
        // what makes the two encodings continuous.
        parts.exponent = 1 - EXPONENT_BIAS - FRACTION_BITS;
    } else {
        parts.significand.hi |= std::uint64_t{1} << HIGH_FRACTION_BITS;
        parts.exponent = static_cast<int>(biased) - EXPONENT_BIAS - FRACTION_BITS;
    }
    return parts;
}

// -- a non-negative big integer ---------------------------------------------
//
// Little-endian 32-bit limbs, so a limb times a small factor fits a
// `std::uint64_t` with no 128-bit intermediate to get wrong. Only what the
// two decimal conversions need: multiply and divide by a machine-word
// factor, shift, and print.

class Big {
  public:
    Big() = default;

    static Big from_wide(const Wide &value) {
        Big out;
        out.limbs_ = {static_cast<std::uint32_t>(value.lo & 0xFFFFFFFFu),
                      static_cast<std::uint32_t>(value.lo >> 32),
                      static_cast<std::uint32_t>(value.hi & 0xFFFFFFFFu),
                      static_cast<std::uint32_t>(value.hi >> 32)};
        out.trim();
        return out;
    }

    bool is_zero() const { return limbs_.empty(); }

    /// How many bits the value takes; zero for zero.
    std::size_t bit_length() const {
        if (limbs_.empty()) {
            return 0;
        }
        std::uint32_t top = limbs_.back();
        std::size_t bits = 0;
        while (top != 0) {
            top >>= 1;
            bits++;
        }
        return (limbs_.size() - 1) * 32 + bits;
    }

    bool bit(std::size_t index) const {
        const std::size_t limb = index / 32;
        if (limb >= limbs_.size()) {
            return false;
        }
        return ((limbs_[limb] >> (index % 32)) & 1u) != 0;
    }

    void set_low_bit() {
        if (limbs_.empty()) {
            limbs_.push_back(1);
        } else {
            limbs_[0] |= 1u;
        }
    }

    void mul_small(std::uint32_t factor) {
        std::uint64_t carry = 0;
        for (std::uint32_t &limb : limbs_) {
            const std::uint64_t product =
                static_cast<std::uint64_t>(limb) * factor + carry;
            limb = static_cast<std::uint32_t>(product & 0xFFFFFFFFu);
            carry = product >> 32;
        }
        while (carry != 0) {
            limbs_.push_back(static_cast<std::uint32_t>(carry & 0xFFFFFFFFu));
            carry >>= 32;
        }
        trim();
    }

    void add_small(std::uint32_t value) {
        std::uint64_t carry = value;
        for (std::size_t i = 0; carry != 0; i++) {
            if (i >= limbs_.size()) {
                limbs_.push_back(0);
            }
            const std::uint64_t sum = static_cast<std::uint64_t>(limbs_[i]) + carry;
            limbs_[i] = static_cast<std::uint32_t>(sum & 0xFFFFFFFFu);
            carry = sum >> 32;
        }
        trim();
    }

    /// `*this /= divisor`, returning the remainder.
    std::uint32_t div_small(std::uint32_t divisor) {
        std::uint64_t remainder = 0;
        for (std::size_t i = limbs_.size(); i > 0; i--) {
            const std::uint64_t current = (remainder << 32) | limbs_[i - 1];
            limbs_[i - 1] = static_cast<std::uint32_t>(current / divisor);
            remainder = current % divisor;
        }
        trim();
        return static_cast<std::uint32_t>(remainder);
    }

    void shl(std::size_t bits) {
        if (limbs_.empty() || bits == 0) {
            return;
        }
        const std::size_t whole = bits / 32;
        const unsigned part = static_cast<unsigned>(bits % 32);
        if (part != 0) {
            std::uint32_t carry = 0;
            for (std::uint32_t &limb : limbs_) {
                const std::uint64_t shifted =
                    (static_cast<std::uint64_t>(limb) << part) | carry;
                limb = static_cast<std::uint32_t>(shifted & 0xFFFFFFFFu);
                carry = static_cast<std::uint32_t>(shifted >> 32);
            }
            if (carry != 0) {
                limbs_.push_back(carry);
            }
        }
        if (whole != 0) {
            limbs_.insert(limbs_.begin(), whole, std::uint32_t{0});
        }
        trim();
    }

    /// `*this >>= bits`, setting `sticky` when a set bit was discarded.
    void shr_sticky(std::size_t bits, bool &sticky) {
        if (bits == 0 || limbs_.empty()) {
            return;
        }
        for (std::size_t i = 0; i < bits && i < limbs_.size() * 32; i++) {
            if (bit(i)) {
                sticky = true;
                break;
            }
        }
        const std::size_t whole = bits / 32;
        const unsigned part = static_cast<unsigned>(bits % 32);
        if (whole >= limbs_.size()) {
            limbs_.clear();
            return;
        }
        limbs_.erase(limbs_.begin(),
                     limbs_.begin() + static_cast<std::ptrdiff_t>(whole));
        if (part != 0) {
            for (std::size_t i = 0; i < limbs_.size(); i++) {
                std::uint32_t next = i + 1 < limbs_.size() ? limbs_[i + 1] : 0u;
                limbs_[i] = static_cast<std::uint32_t>((limbs_[i] >> part) |
                                                       (next << (32u - part)));
            }
        }
        trim();
    }

    /// `*this *= 5^power`, in the largest chunks that fit a limb.
    void mul_pow5(std::size_t power) {
        // 5^13 = 1220703125 is the largest power of five below 2^32.
        while (power >= 13) {
            mul_small(1220703125u);
            power -= 13;
        }
        if (power != 0) {
            std::uint32_t factor = 1;
            for (std::size_t i = 0; i < power; i++) {
                factor *= 5u;
            }
            mul_small(factor);
        }
    }

    /// `*this /= 5^power`, setting `sticky` when the division was not exact.
    void div_pow5(std::size_t power, bool &sticky) {
        while (power >= 13) {
            if (div_small(1220703125u) != 0) {
                sticky = true;
            }
            power -= 13;
        }
        if (power != 0) {
            std::uint32_t factor = 1;
            for (std::size_t i = 0; i < power; i++) {
                factor *= 5u;
            }
            if (div_small(factor) != 0) {
                sticky = true;
            }
        }
    }

    void mul_pow10(std::size_t power) {
        mul_pow5(power);
        shl(power);
    }

    /// The decimal digits, most significant first; empty for zero.
    std::string digits() const {
        if (limbs_.empty()) {
            return {};
        }
        Big work = *this;
        std::string reversed;
        while (!work.is_zero()) {
            // 10^9 is the largest power of ten below 2^32, so one division
            // yields nine digits.
            std::uint32_t group = work.div_small(1000000000u);
            for (int k = 0; k < 9; k++) {
                reversed.push_back(static_cast<char>('0' + (group % 10u)));
                group /= 10u;
                if (work.is_zero() && group == 0) {
                    break;
                }
            }
        }
        return std::string(reversed.rbegin(), reversed.rend());
    }

    /// The low 128 bits. The caller has already checked the bit length.
    Wide to_wide() const {
        Wide out;
        for (std::size_t i = 0; i < limbs_.size() && i < 4; i++) {
            const std::uint64_t limb = limbs_[i];
            if (i < 2) {
                out.lo |= limb << (32 * i);
            } else {
                out.hi |= limb << (32 * (i - 2));
            }
        }
        return out;
    }

  private:
    void trim() {
        while (!limbs_.empty() && limbs_.back() == 0) {
            limbs_.pop_back();
        }
    }

    std::vector<std::uint32_t> limbs_;
};

/// `0.<digits> * 10^point`, in the three forms all four columns use.
std::string render(bool negative, const std::string &digits, int point) {
    const std::string sign = negative ? "-" : "";
    if (point > 0 && point <= 34) {
        if (digits.size() <= static_cast<std::size_t>(point)) {
            return sign + digits +
                   std::string(static_cast<std::size_t>(point) - digits.size(), '0') + ".0";
        }
        return sign + digits.substr(0, static_cast<std::size_t>(point)) + "." +
               digits.substr(static_cast<std::size_t>(point));
    }
    if (point <= 0 && point > -6) {
        return sign + "0." + std::string(static_cast<std::size_t>(-point), '0') + digits;
    }
    const std::string mantissa =
        digits.substr(0, 1) + "." + (digits.size() > 1 ? digits.substr(1) : std::string("0"));
    const int exponent = point - 1;
    return sign + mantissa + "e" + (exponent >= 0 ? "+" : "-") +
           std::to_string(exponent >= 0 ? exponent : -exponent);
}

bool equal_ignoring_case(std::string_view text, const char *word) {
    std::size_t i = 0;
    for (; word[i] != '\0'; i++) {
        if (i >= text.size()) {
            return false;
        }
        char c = text[i];
        if (c >= 'A' && c <= 'Z') {
            c = static_cast<char>(c - 'A' + 'a');
        }
        if (c != word[i]) {
            return false;
        }
    }
    return i == text.size();
}

}  // namespace

// -- constants --------------------------------------------------------------

Float128 Float128::zero(bool negative) noexcept {
    return from_parts(negative ? SIGN_BIT : 0u, 0u);
}

Float128 Float128::one() noexcept {
    return from_parts(static_cast<std::uint64_t>(EXPONENT_BIAS) << HIGH_FRACTION_BITS, 0u);
}

Float128 Float128::infinity(bool negative) noexcept {
    const std::uint64_t high = static_cast<std::uint64_t>(EXPONENT_MAX) << HIGH_FRACTION_BITS;
    return from_parts(negative ? (high | SIGN_BIT) : high, 0u);
}

Float128 Float128::quiet_nan() noexcept {
    // The quiet bit is the top fraction bit, which is bit 47 of the high
    // half. IEEE 754-2008 clause 6.2.1.
    return from_parts((static_cast<std::uint64_t>(EXPONENT_MAX) << HIGH_FRACTION_BITS) |
                          (std::uint64_t{1} << 47),
                      0u);
}

// -- classification ---------------------------------------------------------

namespace {

std::uint32_t biased_exponent(const Float128 &value) {
    return static_cast<std::uint32_t>((value.high >> HIGH_FRACTION_BITS) & EXPONENT_MAX);
}

bool fraction_is_zero(const Float128 &value) {
    return (value.high & HIGH_FRACTION_MASK) == 0 && value.low == 0;
}

}  // namespace

bool Float128::is_nan() const noexcept {
    return biased_exponent(*this) == EXPONENT_MAX && !fraction_is_zero(*this);
}

bool Float128::is_infinite() const noexcept {
    return biased_exponent(*this) == EXPONENT_MAX && fraction_is_zero(*this);
}

bool Float128::is_finite() const noexcept {
    return biased_exponent(*this) != EXPONENT_MAX;
}

bool Float128::is_zero() const noexcept {
    return biased_exponent(*this) == 0 && fraction_is_zero(*this);
}

bool Float128::is_negative() const noexcept { return (high & SIGN_BIT) != 0; }

bool Float128::is_subnormal() const noexcept {
    return biased_exponent(*this) == 0 && !fraction_is_zero(*this);
}

// -- the sixteen bytes ------------------------------------------------------

Float128 Float128::from_bytes(const std::array<std::uint8_t, 16> &bytes,
                              Endianness order) noexcept {
    Float128 value;
    if (order == Endianness::Big) {
        for (std::size_t i = 0; i < 8; i++) {
            value.high = (value.high << 8) | bytes[i];
        }
        for (std::size_t i = 8; i < 16; i++) {
            value.low = (value.low << 8) | bytes[i];
        }
    } else {
        // The whole sixteen bytes reversed, so the last byte carries the
        // sign and the top of the exponent. [CDR] Figure 9.2.
        for (std::size_t i = 16; i > 8; i--) {
            value.high = (value.high << 8) | bytes[i - 1];
        }
        for (std::size_t i = 8; i > 0; i--) {
            value.low = (value.low << 8) | bytes[i - 1];
        }
    }
    return value;
}

std::array<std::uint8_t, 16> Float128::to_bytes(Endianness order) const noexcept {
    std::array<std::uint8_t, 16> out{};
    if (order == Endianness::Big) {
        for (std::size_t i = 0; i < 8; i++) {
            out[i] = static_cast<std::uint8_t>((high >> (8 * (7 - i))) & 0xFFu);
            out[i + 8] = static_cast<std::uint8_t>((low >> (8 * (7 - i))) & 0xFFu);
        }
    } else {
        for (std::size_t i = 0; i < 8; i++) {
            out[i] = static_cast<std::uint8_t>((low >> (8 * i)) & 0xFFu);
            out[i + 8] = static_cast<std::uint8_t>((high >> (8 * i)) & 0xFFu);
        }
    }
    return out;
}

// -- double -----------------------------------------------------------------

Float128 Float128::from_double(double value) noexcept {
    std::uint64_t bits = 0;
    std::memcpy(&bits, &value, sizeof bits);
    const bool negative = (bits >> 63) != 0;
    const std::uint32_t biased = static_cast<std::uint32_t>((bits >> 52) & 0x7FFu);
    const std::uint64_t fraction = bits & 0x000FFFFFFFFFFFFFull;
    const std::uint64_t sign = negative ? SIGN_BIT : 0u;

    if (biased == 0x7FFu) {
        const std::uint64_t exponent_field =
            static_cast<std::uint64_t>(EXPONENT_MAX) << HIGH_FRACTION_BITS;
        if (fraction == 0) {
            return from_parts(sign | exponent_field, 0u);
        }
        // A NaN keeps its payload, aligned at the top of the fraction so
        // that the double's quiet bit lands on binary128's. The shift is 60;
        // the top 48 bits of the result live in the high half.
        return from_parts(sign | exponent_field | ((fraction >> 4) & HIGH_FRACTION_MASK),
                          fraction << 60);
    }
    if (biased == 0 && fraction == 0) {
        return from_parts(sign, 0u);
    }
    bool lost = false;
    if (biased == 0) {
        // A subnormal double becomes a binary128 *normal*: binary128's
        // exponent range swallows binary64's whole subnormal range.
        return compose(negative, Wide{0, fraction}, 1 - 1023 - 52, lost);
    }
    return compose(negative, Wide{0, fraction | (std::uint64_t{1} << 52)},
                   static_cast<int>(biased) - 1023 - 52, lost);
}

Float128 Float128::from_float(float value) noexcept {
    return from_double(static_cast<double>(value));
}

bool Float128::to_double_checked(double *out) const noexcept {
    std::uint64_t bits = 0;
    if (is_nan()) {
        std::uint64_t payload = ((high & HIGH_FRACTION_MASK) << 4) | (low >> 60);
        if (payload == 0) {
            payload = 1;
        }
        bits = (is_negative() ? (std::uint64_t{1} << 63) : 0u) |
               (std::uint64_t{0x7FF} << 52) | (payload & 0x000FFFFFFFFFFFFFull);
        std::memcpy(out, &bits, sizeof bits);
        return (low & 0x0FFFFFFFFFFFFFFFull) == 0;
    }
    if (is_infinite()) {
        bits = (is_negative() ? (std::uint64_t{1} << 63) : 0u) | (std::uint64_t{0x7FF} << 52);
        std::memcpy(out, &bits, sizeof bits);
        return true;
    }

    Parts parts = decompose(*this);
    const std::uint64_t sign = parts.negative ? (std::uint64_t{1} << 63) : 0u;
    if (parts.significand.hi == 0 && parts.significand.lo == 0) {
        std::memcpy(out, &sign, sizeof sign);
        return true;
    }
    const int leading = bit_index(parts.significand);
    const int unbiased = parts.exponent + leading;
    int target = unbiased + 1023;
    Wide significand = parts.significand;
    bool lost = false;

    if (target >= 0x7FF) {
        bits = sign | (std::uint64_t{0x7FF} << 52);
        std::memcpy(out, &bits, sizeof bits);
        return false;
    }
    if (target <= 0) {
        int drop = leading - 52 + (1 - target);
        if (drop < 0) {
            drop = 0;
        }
        shift_round(significand, static_cast<unsigned>(drop), lost);
        bits = sign | significand.lo;
        std::memcpy(out, &bits, sizeof bits);
        return !lost;
    }
    if (leading > 52) {
        shift_round(significand, static_cast<unsigned>(leading - 52), lost);
        if (bit_index(significand) > 52) {
            bool ignored = false;
            shift_round(significand, 1, ignored);
            target += 1;
            if (target >= 0x7FF) {
                bits = sign | (std::uint64_t{0x7FF} << 52);
                std::memcpy(out, &bits, sizeof bits);
                return false;
            }
        }
    } else {
        significand.lo = significand.lo << (52 - leading);
    }
    bits = sign | (static_cast<std::uint64_t>(target) << 52) |
           (significand.lo & 0x000FFFFFFFFFFFFFull);
    std::memcpy(out, &bits, sizeof bits);
    return !lost;
}

double Float128::to_double() const noexcept {
    double out = 0.0;
    to_double_checked(&out);
    return out;
}

// -- decimal ----------------------------------------------------------------

std::string Float128::to_exact_decimal() const {
    if (is_nan()) {
        return "nan";
    }
    if (is_infinite()) {
        return is_negative() ? "-inf" : "inf";
    }
    const Parts parts = decompose(*this);
    if (parts.significand.hi == 0 && parts.significand.lo == 0) {
        return parts.negative ? "-0.0" : "0.0";
    }
    Big big = Big::from_wide(parts.significand);
    int point = 0;
    if (parts.exponent >= 0) {
        big.shl(static_cast<std::size_t>(parts.exponent));
        std::string digits = big.digits();
        point = static_cast<int>(digits.size());
        while (!digits.empty() && digits.back() == '0') {
            digits.pop_back();
        }
        if (digits.empty()) {
            digits = "0";
        }
        return render(parts.negative, digits, point);
    }
    // m * 2^-k is exactly m * 5^k * 10^-k.
    const std::size_t scale = static_cast<std::size_t>(-parts.exponent);
    big.mul_pow5(scale);
    std::string digits = big.digits();
    point = static_cast<int>(digits.size()) - static_cast<int>(scale);
    // Trailing zeros are what the point already accounts for; every binary
    // fraction ends in a tail of them because `5^k` does.
    while (!digits.empty() && digits.back() == '0') {
        digits.pop_back();
    }
    if (digits.empty()) {
        digits = "0";
    }
    return render(parts.negative, digits, point);
}

Float128 Float128::from_decimal(std::string_view text) {
    std::string_view body = text;
    while (!body.empty() && (body.front() == ' ' || body.front() == '\t')) {
        body.remove_prefix(1);
    }
    while (!body.empty() && (body.back() == ' ' || body.back() == '\t')) {
        body.remove_suffix(1);
    }
    if (body.empty()) {
        throw OutOfRange(0, "a decimal, not an empty string");
    }

    for (const char *word : {"inf", "+inf", "infinity", "+infinity"}) {
        if (equal_ignoring_case(body, word)) {
            return infinity();
        }
    }
    for (const char *word : {"-inf", "-infinity"}) {
        if (equal_ignoring_case(body, word)) {
            return infinity(true);
        }
    }
    if (equal_ignoring_case(body, "nan") || equal_ignoring_case(body, "+nan")) {
        return quiet_nan();
    }
    if (equal_ignoring_case(body, "-nan")) {
        Float128 nan = quiet_nan();
        nan.high |= SIGN_BIT;
        return nan;
    }

    const std::string original(text);
    bool negative = false;
    if (body.front() == '+' || body.front() == '-') {
        negative = body.front() == '-';
        body.remove_prefix(1);
    }

    std::string_view mantissa = body;
    long long exponent = 0;
    const std::size_t marker = body.find_first_of("eE");
    if (marker != std::string_view::npos) {
        mantissa = body.substr(0, marker);
        std::string_view exponent_text = body.substr(marker + 1);
        bool exponent_negative = false;
        if (!exponent_text.empty() &&
            (exponent_text.front() == '+' || exponent_text.front() == '-')) {
            exponent_negative = exponent_text.front() == '-';
            exponent_text.remove_prefix(1);
        }
        if (exponent_text.empty()) {
            throw OutOfRange(0, "a decimal with a usable exponent, not `" + original + "`");
        }
        for (char character : exponent_text) {
            if (character < '0' || character > '9') {
                throw OutOfRange(0, "a decimal with a usable exponent, not `" + original + "`");
            }
            // Clamped as it is read: a literal exponent of a hundred
            // thousand is still an infinity or a zero, and letting it
            // overflow would make it something else.
            if (exponent < 1'000'000) {
                exponent = exponent * 10 + (character - '0');
            }
        }
        if (exponent_negative) {
            exponent = -exponent;
        }
    }

    const std::size_t dot = mantissa.find('.');
    const std::string_view whole =
        dot == std::string_view::npos ? mantissa : mantissa.substr(0, dot);
    const std::string_view fraction =
        dot == std::string_view::npos ? std::string_view{} : mantissa.substr(dot + 1);
    if (whole.empty() && fraction.empty()) {
        throw OutOfRange(0, "a decimal, not `" + original + "`");
    }
    std::string all;
    all.reserve(whole.size() + fraction.size());
    for (std::string_view part : {whole, fraction}) {
        for (char character : part) {
            // An underscore is refused rather than ignored: in a value file
            // `1_5` is a typo, and reading it as fifteen is the kind of
            // quiet reinterpretation this project treats as a defect.
            if (character < '0' || character > '9') {
                throw OutOfRange(0, "a decimal, not `" + original + "`");
            }
            all.push_back(character);
        }
    }
    exponent -= static_cast<long long>(fraction.size());

    std::size_t first = all.find_first_not_of('0');
    if (first == std::string::npos) {
        return zero(negative);
    }
    all = all.substr(first);

    bool sticky = false;
    if (all.size() > MAX_SIGNIFICANT_DIGITS) {
        for (std::size_t i = MAX_SIGNIFICANT_DIGITS; i < all.size(); i++) {
            if (all[i] != '0') {
                sticky = true;
                break;
            }
        }
        exponent += static_cast<long long>(all.size() - MAX_SIGNIFICANT_DIGITS);
        all = all.substr(0, MAX_SIGNIFICANT_DIGITS);
    }

    // The value is about 10^(digits + exponent - 1). The largest finite
    // binary128 is 1.19e+4932 and the smallest subnormal 6.48e-4966, so a
    // magnitude clearly outside that needs no arithmetic at all — and doing
    // it would mean building a bignum of a hundred thousand limbs for a
    // literal that was always going to be an infinity.
    const long long magnitude = static_cast<long long>(all.size()) + exponent - 1;
    if (magnitude > 4940) {
        return infinity(negative);
    }
    if (magnitude < -4980) {
        return zero(negative);
    }

    Big big;
    // Nine digits at a time keeps every step a machine-word multiply.
    for (std::size_t start = 0; start < all.size(); start += 9) {
        const std::size_t take = all.size() - start < 9 ? all.size() - start : 9;
        std::uint32_t chunk = 0;
        std::uint32_t scale = 1;
        for (std::size_t i = 0; i < take; i++) {
            chunk = chunk * 10u + static_cast<std::uint32_t>(all[start + i] - '0');
            scale *= 10u;
        }
        big.mul_small(scale);
        big.add_small(chunk);
    }

    int binary_exponent = 0;
    if (exponent >= 0) {
        big.mul_pow10(static_cast<std::size_t>(exponent));
    } else {
        const std::size_t scale = static_cast<std::size_t>(-exponent);
        // value = N / 10^k = (N * 2^shift / 5^k) * 2^-(k + shift). The shift
        // is chosen so the quotient carries more bits than binary128 needs,
        // and everything the division discards folds into one sticky bit —
        // which is what makes the single rounding at the end the correct one.
        const std::size_t bits_of_five =
            static_cast<std::size_t>((scale * 2321928ull) / 1000000ull) + 2;
        const std::size_t have = big.bit_length();
        std::size_t shift = 0;
        if (130 + bits_of_five > have) {
            shift = 130 + bits_of_five - have;
        }
        big.shl(shift);
        big.div_pow5(scale, sticky);
        binary_exponent =
            -static_cast<int>(scale) - static_cast<int>(shift);
    }
    if (sticky) {
        big.set_low_bit();
    }
    if (big.is_zero()) {
        return zero(negative);
    }

    const std::size_t bits = big.bit_length();
    if (bits > 128) {
        bool dropped = false;
        const std::size_t drop = bits - 128;
        big.shr_sticky(drop, dropped);
        binary_exponent += static_cast<int>(drop);
        if (dropped) {
            big.set_low_bit();
        }
    }
    bool lost = false;
    return compose(negative, big.to_wide(), binary_exponent, lost);
}

}  // namespace zerodds::wire
