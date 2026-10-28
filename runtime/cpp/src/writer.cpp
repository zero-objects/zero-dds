// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Putting bytes on the wire, in C++.
//
// Alignment is relative to the start of the stream, not to the start of the
// value: XCDR aligns each primitive to its own size counted from the
// encapsulation, which is why a `long` after a `char` sits at 4 and not
// at 1. §7.4.1.1.
//
// This file also carries what the writer and the reader share and neither
// should own — the errors, the encapsulation header, the dialect tables and
// the UTF-8 decode — because a second copy of any of them is a second thing
// to get wrong.

#include "zerodds/wire.hpp"

#include <cstring>
#include <string>

namespace zerodds::wire {

// -- errors -----------------------------------------------------------------

namespace {

std::string decimal(std::size_t value) { return std::to_string(value); }

std::string hex4(std::uint32_t value) {
    static const char digits[] = "0123456789ABCDEF";
    std::string out;
    unsigned width = value > 0xFFFFu ? 6u : 4u;
    for (unsigned i = width; i > 0; i--) {
        out.push_back(digits[(value >> (4u * (i - 1u))) & 0xFu]);
    }
    return out;
}

}  // namespace

Truncated::Truncated(std::size_t at, std::size_t needed, std::size_t available)
    : WireError(at, "at " + decimal(at) + ": " + decimal(needed) +
                        " bytes needed, " + decimal(available) + " available"),
      needed_(needed),
      available_(available) {}

ImpossibleLength::ImpossibleLength(std::size_t at, std::size_t claimed,
                                   std::size_t available)
    : WireError(at, "at " + decimal(at) + ": a length of " + decimal(claimed) +
                        " with " + decimal(available) + " bytes left"),
      claimed_(claimed),
      available_(available) {}

NotUtf8::NotUtf8(std::size_t at)
    : WireError(at, "at " + decimal(at) + ": the string is not UTF-8") {}

OutOfRange::OutOfRange(std::size_t at, const std::string &expected)
    : WireError(at, "at " + decimal(at) + ": expected " + expected),
      expected_(expected) {}

// -- byte order and encodings ----------------------------------------------

std::optional<Endianness> parse_endianness(std::string_view name) {
    if (name == "le" || name == "little") {
        return Endianness::Little;
    }
    if (name == "be" || name == "big") {
        return Endianness::Big;
    }
    return std::nullopt;
}

std::optional<Encoding> parse_encoding(std::string_view name) {
    if (name == "xcdr1" || name == "plain-cdr1") {
        return Encoding::PlainCdr1;
    }
    if (name == "xcdr2" || name == "plain-cdr2") {
        return Encoding::PlainCdr2;
    }
    if (name == "pl-cdr1") {
        return Encoding::PlCdr1;
    }
    if (name == "delimited-cdr") {
        return Encoding::DelimitedCdr;
    }
    if (name == "pl-cdr2") {
        return Encoding::PlCdr2;
    }
    return std::nullopt;
}

std::array<std::uint8_t, 4> encapsulation_header(Encoding encoding,
                                                 Endianness endianness,
                                                 unsigned padding) {
    // DDSI-RTPS 2.5 §10.5 and §7.4.1.1. The identifier is always big-endian
    // on the wire, whatever the payload behind it is: it is what tells a
    // reader which order to switch to.
    std::uint16_t identifier = 0;
    switch (encoding) {
        case Encoding::PlainCdr1:
            identifier = 0x0000;
            break;
        case Encoding::PlCdr1:
            identifier = 0x0002;
            break;
        case Encoding::PlainCdr2:
            identifier = 0x0006;
            break;
        case Encoding::DelimitedCdr:
            identifier = 0x0008;
            break;
        case Encoding::PlCdr2:
            identifier = 0x000A;
            break;
    }
    if (is_little(endianness)) {
        identifier = static_cast<std::uint16_t>(identifier + 1u);
    }
    return {static_cast<std::uint8_t>(identifier >> 8),
            static_cast<std::uint8_t>(identifier & 0xFFu), 0u,
            static_cast<std::uint8_t>(padding & 0x3u)};
}

// -- the PL_CDR1 dialect ----------------------------------------------------

std::uint16_t pid_flags_at(PidFlagPolicy policy, WirePosition position) {
    switch (policy) {
        case PidFlagPolicy::SentinelMustUnderstand:
            return (position == WirePosition::Sentinel ||
                    position == WirePosition::ExtendedMarker)
                       ? PID_MUST_UNDERSTAND
                       : std::uint16_t{0};
        case PidFlagPolicy::ImplExtensionMembers:
            return (position == WirePosition::Member ||
                    position == WirePosition::ExtendedMarker)
                       ? static_cast<std::uint16_t>(PID_IMPL_EXTENSION |
                                                    PID_MUST_UNDERSTAND)
                       : std::uint16_t{0};
        case PidFlagPolicy::Bare:
            break;
    }
    return 0;
}

// -- the wide-character axes ------------------------------------------------

// What each cell answers: C1 counts characters, C2 writes a terminator, C3 is
// the unit width. One table rather than a predicate per question, so a cell
// cannot answer one of the three and be forgotten by another.
namespace {

struct WstringCell {
    bool counts_characters;
    bool terminated;
    std::size_t width;
};

WstringCell cell_of(WstringLengthPolicy policy) {
    switch (policy) {
        case WstringLengthPolicy::Specification:
            return {false, false, 2};
        case WstringLengthPolicy::CharacterCount:
            return {true, false, 2};
        case WstringLengthPolicy::CharacterCountTerminated:
            return {true, true, 4};
        case WstringLengthPolicy::SpecificationWideUnits:
            return {false, false, 4};
        case WstringLengthPolicy::RosTypeSupport:
            return {true, false, 4};
        case WstringLengthPolicy::OctetsTerminated2:
            return {false, true, 2};
        case WstringLengthPolicy::OctetsTerminated4:
            return {false, true, 4};
        case WstringLengthPolicy::CharactersTerminated2:
            return {true, true, 2};
    }
    return {false, false, 2};
}

}  // namespace

bool wstring_counts_characters(WstringLengthPolicy policy) {
    return cell_of(policy).counts_characters;
}

bool wstring_terminated(WstringLengthPolicy policy) {
    return cell_of(policy).terminated;
}

std::size_t wstring_unit_width(WstringLengthPolicy policy) {
    return cell_of(policy).width;
}

std::optional<WstringLengthPolicy> parse_wstring_policy(std::string_view name) {
    // Every cell is reachable by its coordinates as well as by its name,
    // exactly as `WstringLengthPolicy::parse` accepts both. A cell that can
    // only be named cannot be asked for by someone who has measured it and
    // not yet named it, which is how the ROS form stayed unreachable.
    static const struct {
        const char *name;
        WstringLengthPolicy policy;
    } table[] = {
        {"specification", WstringLengthPolicy::Specification},
        {"character-count", WstringLengthPolicy::CharacterCount},
        {"character-count-terminated", WstringLengthPolicy::CharacterCountTerminated},
        {"specification-wide-units", WstringLengthPolicy::SpecificationWideUnits},
        {"ros-type-support", WstringLengthPolicy::RosTypeSupport},
        {"octets/unterminated/2", WstringLengthPolicy::Specification},
        {"characters/unterminated/2", WstringLengthPolicy::CharacterCount},
        {"characters/terminated/4", WstringLengthPolicy::CharacterCountTerminated},
        {"octets/unterminated/4", WstringLengthPolicy::SpecificationWideUnits},
        {"characters/unterminated/4", WstringLengthPolicy::RosTypeSupport},
        {"octets/terminated/2", WstringLengthPolicy::OctetsTerminated2},
        {"octets/terminated/4", WstringLengthPolicy::OctetsTerminated4},
        {"characters/terminated/2", WstringLengthPolicy::CharactersTerminated2},
    };
    for (const auto &entry : table) {
        if (name == entry.name) {
            return entry.policy;
        }
    }
    return std::nullopt;
}

// -- text -------------------------------------------------------------------

namespace detail {

std::vector<char32_t> utf8_code_points(std::string_view text) {
    std::vector<char32_t> points;
    // Counted as the write side counts, so the offset a lone surrogate is
    // reported at is the same one the Python column reports.
    std::size_t units = 0;
    std::size_t i = 0;
    const auto *s = reinterpret_cast<const unsigned char *>(text.data());
    const std::size_t size = text.size();
    while (i < size) {
        const unsigned char lead = s[i];
        char32_t point = 0;
        std::size_t taken = 0;
        if (lead < 0x80u) {
            point = lead;
            taken = 1;
        } else if ((lead & 0xE0u) == 0xC0u) {
            if (i + 1 >= size || (s[i + 1] & 0xC0u) != 0x80u) {
                throw NotUtf8(i);
            }
            point = static_cast<char32_t>((lead & 0x1Fu) << 6 | (s[i + 1] & 0x3Fu));
            if (point < 0x80u) {
                throw NotUtf8(i);  // an overlong form is not the character
            }
            taken = 2;
        } else if ((lead & 0xF0u) == 0xE0u) {
            if (i + 2 >= size || (s[i + 1] & 0xC0u) != 0x80u ||
                (s[i + 2] & 0xC0u) != 0x80u) {
                throw NotUtf8(i);
            }
            point = static_cast<char32_t>(static_cast<std::uint32_t>(lead & 0x0Fu) << 12 |
                                          static_cast<std::uint32_t>(s[i + 1] & 0x3Fu) << 6 |
                                          static_cast<std::uint32_t>(s[i + 2] & 0x3Fu));
            if (point < 0x800u) {
                throw NotUtf8(i);
            }
            if (point >= 0xD800u && point <= 0xDFFFu) {
                // CESU-8. Python permits a lone surrogate in a `str` and
                // raises here; neither is a code point, and writing one
                // would put a sequence on the wire that is not UTF-16.
                throw OutOfRange(units * 2,
                                 "a Unicode code point, not a lone surrogate");
            }
            taken = 3;
        } else if ((lead & 0xF8u) == 0xF0u) {
            if (i + 3 >= size || (s[i + 1] & 0xC0u) != 0x80u ||
                (s[i + 2] & 0xC0u) != 0x80u || (s[i + 3] & 0xC0u) != 0x80u) {
                throw NotUtf8(i);
            }
            point = static_cast<char32_t>(static_cast<std::uint32_t>(lead & 0x07u) << 18 |
                                          static_cast<std::uint32_t>(s[i + 1] & 0x3Fu) << 12 |
                                          static_cast<std::uint32_t>(s[i + 2] & 0x3Fu) << 6 |
                                          static_cast<std::uint32_t>(s[i + 3] & 0x3Fu));
            if (point < 0x10000u || point > 0x10FFFFu) {
                throw NotUtf8(i);
            }
            taken = 4;
        } else {
            throw NotUtf8(i);
        }
        units += point >= 0x10000u ? 2u : 1u;
        points.push_back(point);
        i += taken;
    }
    return points;
}

std::vector<char16_t> utf16_units(const std::vector<char32_t> &points) {
    std::vector<char16_t> units;
    units.reserve(points.size());
    for (char32_t point : points) {
        if (point < 0x10000u) {
            units.push_back(static_cast<char16_t>(point));
        } else {
            const std::uint32_t offset = static_cast<std::uint32_t>(point) - 0x10000u;
            units.push_back(static_cast<char16_t>(0xD800u + (offset >> 10)));
            units.push_back(static_cast<char16_t>(0xDC00u + (offset & 0x3FFu)));
        }
    }
    return units;
}

}  // namespace detail

// -- fixed<P,S> -------------------------------------------------------------

std::vector<std::uint8_t> fixed_nibbles(std::string_view text, unsigned digits,
                                        unsigned scale, FixedDigitCount count) {
    if (digits == 0) {
        throw OutOfRange(0, "a fixed with at least one digit");
    }
    // OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and `fixed<31, S>`
    // is the widest declaration IDL admits. A wider one has no packing defined
    // for it, so it is refused rather than packed into octets no reader can be
    // told the width of.
    if (digits > 31) {
        throw OutOfRange(0, "a fixed of at most 31 digits, not " + std::to_string(digits));
    }
    if (scale > digits) {
        throw OutOfRange(0, "a scale no larger than the digit count");
    }

    const std::string original(text);
    const std::string suffix =
        "a decimal that fits fixed<" + std::to_string(digits) + ", " +
        std::to_string(scale) + ">, not `" + original + "`";

    bool negative = false;
    std::string_view magnitude = text;
    if (!magnitude.empty() && (magnitude.front() == '-' || magnitude.front() == '+')) {
        negative = magnitude.front() == '-';
        magnitude.remove_prefix(1);
    }

    const std::size_t point = magnitude.find('.');
    std::string_view whole =
        point == std::string_view::npos ? magnitude : magnitude.substr(0, point);
    const std::string_view fraction =
        point == std::string_view::npos ? std::string_view{} : magnitude.substr(point + 1);
    if (whole.empty() && fraction.empty()) {
        throw OutOfRange(0, suffix);
    }
    const std::size_t as_written = whole.size() + scale;
    for (char character : whole) {
        if (character < '0' || character > '9') {
            throw OutOfRange(0, suffix);
        }
    }
    for (char character : fraction) {
        if (character < '0' || character > '9') {
            throw OutOfRange(0, suffix);
        }
    }

    const unsigned integral_width = digits - scale;
    // Leading zeros do not count against the integer part's width; a value
    // that still overruns is one this type cannot represent, and truncating
    // it would put a different number on the wire.
    while (!whole.empty() && whole.front() == '0') {
        whole.remove_prefix(1);
    }
    if (whole.size() > integral_width) {
        throw OutOfRange(0, "a decimal whose integer part fits " +
                                std::to_string(integral_width) + " digits, not `" +
                                original + "`");
    }
    if (fraction.size() > scale) {
        throw OutOfRange(0, "a decimal whose fraction fits " + std::to_string(scale) +
                                " digits, not `" + original + "`");
    }

    // The full `digits`-wide string first, whatever the count: the value's
    // precision is that string with its leading zeros removed, so both
    // counts are read off one construction.
    std::vector<std::uint8_t> every;
    every.reserve(digits);
    for (std::size_t i = whole.size(); i < integral_width; i++) {
        every.push_back(0);
    }
    for (char character : whole) {
        every.push_back(static_cast<std::uint8_t>(character - '0'));
    }
    for (char character : fraction) {
        every.push_back(static_cast<std::uint8_t>(character - '0'));
    }
    for (std::size_t i = fraction.size(); i < scale; i++) {
        every.push_back(0);
    }

    std::size_t leading = 0;
    while (leading < every.size() && every[leading] == 0) {
        leading++;
    }
    const std::size_t significant = every.size() - leading;
    // Never fewer than one: `BigDecimal("0.00").precision()` is 1 in Java,
    // and a field of no digits could not carry the sign nibble's pair.
    std::size_t wire = every.size();
    if (count == FixedDigitCount::ValuePrecision) {
        wire = significant == 0 ? std::size_t{1} : significant;
    } else if (count == FixedDigitCount::ValueStringDigits) {
        // ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one leading
        // zero. Past 31 no reader can be told the field's width.
        wire = as_written + (as_written % 2 == 0 ? 1 : 0);
        if (wire > 31) {
            throw OutOfRange(0, "a fixed of at most 31 digits on the wire, not " +
                                    std::to_string(wire) + " for `" + original + "`");
        }
    }

    std::vector<std::uint8_t> nibbles;
    if ((wire + 1) % 2 == 1) {
        nibbles.push_back(0);
    }
    // A count wider than `P` is filled with leading zeros.
    for (std::size_t i = every.size(); i < wire; i++) {
        nibbles.push_back(0);
    }
    for (std::size_t i = wire < every.size() ? every.size() - wire : 0; i < every.size(); i++) {
        nibbles.push_back(every[i]);
    }
    // [CDR] §9.3.2.8: `0xd` marks a negative value and `0xc` everything
    // else, zero included — a negative zero is written positive, because
    // there is one zero.
    nibbles.push_back(negative && significant != 0 ? std::uint8_t{0x0D} : std::uint8_t{0x0C});
    return nibbles;
}

// -- the writer -------------------------------------------------------------

Writer::Writer(Endianness endianness, std::size_t max_align, PlCdr1Framing framing,
               bool short_codes)
    : endianness_(endianness),
      // A cap of zero would make every alignment a division by zero.
      max_align_(max_align < 1 ? std::size_t{1} : max_align),
      pl_cdr1_(framing),
      short_length_codes_(short_codes) {}

void Writer::align_to(std::size_t align) {
    if (align > max_align_) {
        align = max_align_;
    }
    if (align <= 1) {
        return;
    }
    const std::size_t offset = out_.size() - origin_;
    const std::size_t padding = (align - (offset % align)) % align;
    out_.insert(out_.end(), padding, std::uint8_t{0});
}

std::size_t Writer::begin_packed_element() {
    const std::size_t saved = origin_;
    origin_ = out_.size();
    return saved;
}

void Writer::finish_packed_element(std::size_t saved) { origin_ = saved; }

void Writer::check(std::int64_t value, std::int64_t low, std::int64_t high,
                   const char *expected) const {
    if (value < low || value > high) {
        throw OutOfRange(out_.size(), std::string(expected) + " in [" +
                                          std::to_string(low) + ", " +
                                          std::to_string(high) + "], not " +
                                          std::to_string(value));
    }
}

void Writer::check_unsigned(std::uint64_t value, std::uint64_t high,
                            const char *expected) const {
    if (value > high) {
        throw OutOfRange(out_.size(), std::string(expected) + " in [0, " +
                                          std::to_string(high) + "], not " +
                                          std::to_string(value));
    }
}

void Writer::write_raw(std::uint64_t value, std::size_t width) {
    for (std::size_t i = 0; i < width; i++) {
        const std::size_t shift = is_little(endianness_) ? i : width - 1 - i;
        out_.push_back(static_cast<std::uint8_t>((value >> (8 * shift)) & 0xFFu));
    }
}

void Writer::write_int(std::uint64_t value, std::size_t width) {
    align_to(width);
    write_raw(value, width);
}

void Writer::patch(std::size_t at, std::uint64_t value, std::size_t width) {
    if (width < 8 && value >= (std::uint64_t{1} << (width * 8))) {
        throw OutOfRange(at, "a " + std::to_string(width) + "-byte length, not " +
                                 std::to_string(value));
    }
    for (std::size_t i = 0; i < width; i++) {
        const std::size_t shift = is_little(endianness_) ? i : width - 1 - i;
        out_[at + i] = static_cast<std::uint8_t>((value >> (8 * shift)) & 0xFFu);
    }
}

void Writer::write_bool(bool value) { out_.push_back(value ? 1u : 0u); }

void Writer::write_u8(std::uint64_t value) {
    check_unsigned(value, 0xFFu, "an octet");
    out_.push_back(static_cast<std::uint8_t>(value));
}

void Writer::write_octets(const std::uint8_t *data, std::size_t count) {
    if (count == 0) {
        return;
    }
    out_.insert(out_.end(), data, data + count);
}

void Writer::write_i8(std::int64_t value) {
    check(value, -0x80, 0x7F, "an int8");
    out_.push_back(static_cast<std::uint8_t>(static_cast<std::uint64_t>(value) & 0xFFu));
}

void Writer::write_char(char32_t value) {
    if (value > 0xFFu) {
        throw OutOfRange(out_.size(), "a character below U+0100, not U+" +
                                          hex4(static_cast<std::uint32_t>(value)));
    }
    out_.push_back(static_cast<std::uint8_t>(value));
}

void Writer::write_wchar(char32_t value, WcharWidth width, WcharFraming framing,
                         WstringUnitOrder order) {
    if (value > 0xFFFFu) {
        throw OutOfRange(out_.size(),
                         "a BMP character, not U+" + hex4(static_cast<std::uint32_t>(value)));
    }
    const std::size_t octets = static_cast<std::size_t>(width);
    if (framing == WcharFraming::GiopLengthPrefixed) {
        // [CDR] §9.3.1.6's GIOP 1.2 form: the octet count first, then the
        // code unit's octets, all aligned to one. The count is the
        // transmission code set's width and not the value's, so it is the
        // same octet for every character — omniORB writes `02` for both
        // U+00E4 and U+1234.
        //
        // A scalar cannot carry a byte-order mark, so CORBA's default
        // applies and any order but the stream's means big-endian here.
        // Measured 2026-08-10 in a little-endian stream: omniORB writes
        // `02 00e4` and JacORB `02 00e4`, both big-endian; TAO writes
        // `04 e4000000`, the stream's order.
        out_.push_back(static_cast<std::uint8_t>(octets));
        const bool little =
            order == WstringUnitOrder::Stream && is_little(endianness_);
        for (std::size_t i = 0; i < octets; i++) {
            const std::size_t shift = little ? i : octets - 1 - i;
            out_.push_back(static_cast<std::uint8_t>(
                (static_cast<std::uint32_t>(value) >> (8 * shift)) & 0xFFu));
        }
        return;
    }
    if (octets == 4) {
        write_u32(value);
    } else {
        write_u16(value);
    }
}

void Writer::write_i16(std::int64_t value) {
    check(value, -0x8000, 0x7FFF, "a short");
    write_int(static_cast<std::uint64_t>(value) & 0xFFFFu, 2);
}

void Writer::write_u16(std::uint64_t value) {
    check_unsigned(value, 0xFFFFu, "an unsigned short");
    write_int(value, 2);
}

void Writer::write_i32(std::int64_t value) {
    check(value, -0x8000'0000LL, 0x7FFF'FFFFLL, "a long");
    write_int(static_cast<std::uint64_t>(value) & 0xFFFF'FFFFu, 4);
}

void Writer::write_u32(std::uint64_t value) {
    check_unsigned(value, 0xFFFF'FFFFu, "an unsigned long");
    write_int(value, 4);
}

void Writer::write_i64(std::int64_t value) {
    write_int(static_cast<std::uint64_t>(value), 8);
}

void Writer::write_u64(std::uint64_t value) { write_int(value, 8); }

void Writer::write_f32(double value) {
    // Rounded to single precision once, here. A magnitude no `float` can
    // hold becomes an infinity, which is the wire-correct answer: it is what
    // a C `(float)` cast produces and what every peer will read back.
    const float narrowed = static_cast<float>(value);
    std::uint32_t bits = 0;
    std::memcpy(&bits, &narrowed, sizeof bits);
    write_int(bits, 4);
}

void Writer::write_f64(double value) {
    std::uint64_t bits = 0;
    std::memcpy(&bits, &value, sizeof bits);
    write_int(bits, 8);
}

void Writer::write_f128(const Float128 &value) {
    const std::array<std::uint8_t, 16> bytes = value.to_bytes(endianness_);
    align_to(8);
    out_.insert(out_.end(), bytes.begin(), bytes.end());
}

void Writer::write_u16_little(std::uint64_t value) {
    check_unsigned(value, 0xFFFFu, "an unsigned 16-bit value");
    align_to(2);
    out_.push_back(static_cast<std::uint8_t>(value & 0xFFu));
    out_.push_back(static_cast<std::uint8_t>((value >> 8) & 0xFFu));
}

void Writer::write_u16_big(std::uint64_t value) {
    check_unsigned(value, 0xFFFFu, "an unsigned 16-bit value");
    align_to(2);
    out_.push_back(static_cast<std::uint8_t>((value >> 8) & 0xFFu));
    out_.push_back(static_cast<std::uint8_t>(value & 0xFFu));
}

void Writer::write_u32_little(std::uint64_t value) {
    check_unsigned(value, 0xFFFF'FFFFu, "an unsigned 32-bit value");
    align_to(4);
    for (std::size_t i = 0; i < 4; i++) {
        out_.push_back(static_cast<std::uint8_t>((value >> (8 * i)) & 0xFFu));
    }
}

void Writer::write_u32_big(std::uint64_t value) {
    check_unsigned(value, 0xFFFF'FFFFu, "an unsigned 32-bit value");
    align_to(4);
    for (std::size_t i = 0; i < 4; i++) {
        out_.push_back(static_cast<std::uint8_t>((value >> (8 * (3 - i))) & 0xFFu));
    }
}

// -- strings ----------------------------------------------------------------

void Writer::write_string(std::string_view value) {
    write_u32(value.size() + 1);
    out_.insert(out_.end(), value.begin(), value.end());
    out_.push_back(0);
}

void Writer::write_string_bounded(std::string_view value, std::size_t bound) {
    if (value.size() > bound) {
        throw OutOfRange(out_.size(),
                          "at most " + std::to_string(bound) +
                              " octets in a bounded string, and this one has " +
                              std::to_string(value.size()));
    }
    write_string(value);
}

void Writer::write_wstring_bounded(std::string_view utf8, std::size_t bound,
                                   WstringLengthPolicy policy, WstringUnitOrder order) {
    const std::size_t units = detail::utf16_units(detail::utf8_code_points(utf8)).size();
    if (units > bound) {
        throw OutOfRange(out_.size(),
                          "at most " + std::to_string(bound) +
                              " UTF-16 code units in a bounded wstring, and this one has " +
                              std::to_string(units));
    }
    write_wstring_utf8(utf8, policy, order);
}

void Writer::check_sequence_bound(std::size_t actual, std::size_t bound) {
    if (actual > bound) {
        throw OutOfRange(out_.size(),
                          "at most " + std::to_string(bound) +
                              " elements in a bounded sequence, and this one has " +
                              std::to_string(actual));
    }
}

void Writer::check_map_bound(std::size_t actual, std::size_t bound) {
    if (actual > bound) {
        throw OutOfRange(out_.size(),
                          "at most " + std::to_string(bound) +
                              " entries in a bounded map, and this one has " +
                              std::to_string(actual));
    }
}

void Writer::write_wstring_unit(std::uint16_t unit, std::size_t width,
                                WstringUnitOrder order) {
    // C3 picks the writer and the unit order picks its endianness. There is
    // no arm for a length policy that decides a byte order: `HostUnswapped`
    // is a point of the order axis.
    const bool little = order == WstringUnitOrder::ByteOrderMark ||
                        order == WstringUnitOrder::HostUnswapped;
    if (width == 4) {
        if (order == WstringUnitOrder::BigEndianAlways) {
            write_u32_big(unit);
        } else if (little) {
            write_u32_little(unit);
        } else {
            write_u32(unit);
        }
    } else if (order == WstringUnitOrder::BigEndianAlways) {
        write_u16_big(unit);
    } else if (little) {
        write_u16_little(unit);
    } else {
        write_u16(unit);
    }
}

void Writer::write_wstring_utf8(std::string_view utf8, WstringLengthPolicy policy,
                           WstringUnitOrder order) {
    const WstringCell cell = cell_of(policy);
    const std::vector<char32_t> points = detail::utf8_code_points(utf8);

    if (cell.counts_characters && cell.width == 2) {
        // Fast CDR counts characters and writes each four-byte `wchar_t` as
        // two bytes, so a code point outside the BMP goes out as its low
        // sixteen bits and arrives as a different character. Reproducing a
        // vendor's length convention is a policy; reproducing its data loss
        // is not, so the value is refused.
        //
        // The refusal is on the pair (characters, two octets) and not on a
        // named form: a character count over four-octet units carries the
        // surrogate pair intact.
        for (std::size_t index = 0; index < points.size(); index++) {
            if (points[index] > 0xFFFFu) {
                throw OutOfRange(index,
                                 "a code point inside the BMP: a character count over "
                                 "two-octet units has no way to carry one outside it");
            }
        }
    }

    std::vector<char16_t> units = detail::utf16_units(points);
    if (cell.terminated) {
        // [CDR] §9.3.1.6's GIOP 1.1 rule, and RTI's XCDR1 form: a NUL code
        // unit at the end, inside whatever the length counts. Nothing is
        // lost above the BMP — both units are written.
        units.push_back(0);
    }
    // An empty `wstring` carries no mark: omniORB writes `00000000` for one,
    // byte-identical to every other implementation. A mark on an empty
    // string would be a two-octet string that decodes to nothing.
    if (order == WstringUnitOrder::ByteOrderMark && !units.empty()) {
        units.insert(units.begin(), static_cast<char16_t>(BYTE_ORDER_MARK));
    }

    // C1, over units that already include the terminator and the mark. An
    // octet count multiplies by C3; a character count is the run's length.
    // One expression rather than an arm per named form, so a cell nobody has
    // measured computes its length the same way.
    write_u32(cell.counts_characters ? units.size() : units.size() * cell.width);
    for (char16_t unit : units) {
        write_wstring_unit(static_cast<std::uint16_t>(unit), cell.width, order);
    }
}

void Writer::write_wstring_utf8_octets(std::string_view utf8, WstringLengthPolicy policy) {
    // The same malformed-UTF-8 refusal the UTF-16 writer makes: this
    // direction has to check what the Rust column's type checked for it.
    (void)detail::utf8_code_points(utf8);
    const std::size_t length = utf8.size() + (wstring_terminated(policy) ? 1u : 0u);
    if (length > 0xFFFFFFFFu) {
        throw OutOfRange(out_.size(), "a wstring length the UInt32 field can hold");
    }
    write_u32(length);
    out_.insert(out_.end(), utf8.begin(), utf8.end());
    if (wstring_terminated(policy)) {
        out_.push_back(0);
    }
}

void Writer::write_wstring_utf8_octets_bounded(std::string_view utf8, std::size_t bound,
                                               WstringLengthPolicy policy) {
    // A `wstring<N>` bound counts UTF-16 code units whatever the code units
    // the wire carries, measured in the same walk the UTF-16 writer uses.
    const std::size_t units = detail::utf16_units(detail::utf8_code_points(utf8)).size();
    if (units > bound) {
        throw OutOfRange(out_.size(),
                          "at most " + std::to_string(bound) +
                              " UTF-16 code units in a bounded wstring, and this one has " +
                              std::to_string(units));
    }
    write_wstring_utf8_octets(utf8, policy);
}

void Writer::write_fixed(std::string_view value, unsigned digits, unsigned scale,
                         FixedDigitCount count) {
    const std::vector<std::uint8_t> nibbles = fixed_nibbles(value, digits, scale, count);
    // The count is even by construction: `fixed_nibbles` prepends a zero
    // nibble when the digits and the sign would otherwise be odd.
    for (std::size_t i = 0; i + 1 < nibbles.size(); i += 2) {
        out_.push_back(static_cast<std::uint8_t>(
            (static_cast<unsigned>(nibbles[i]) << 4) | nibbles[i + 1]));
    }
}

// -- the bit-bounded kinds --------------------------------------------------

void Writer::write_enum(unsigned bit_bound, std::int64_t value) {
    if (bit_bound == 0 || bit_bound > 32) {
        throw OutOfRange(out_.size(), "an @bit_bound between 1 and 32, not " +
                                          std::to_string(bit_bound));
    }
    const std::int64_t low = -(std::int64_t{1} << (bit_bound - 1));
    const std::int64_t high = (std::int64_t{1} << bit_bound) - 1;
    if (value < low || value > high) {
        throw OutOfRange(out_.size(), "an enumerator a " + std::to_string(bit_bound) +
                                          "-bit holder can hold, not " +
                                          std::to_string(value));
    }
    const std::uint64_t pattern =
        static_cast<std::uint64_t>(value) & ((std::uint64_t{1} << bit_bound) - 1u);
    if (bit_bound <= 8) {
        write_u8(pattern);
    } else if (bit_bound <= 16) {
        write_u16(pattern);
    } else {
        write_u32(pattern);
    }
}

void Writer::write_bitmask(unsigned bit_bound, std::uint64_t value) {
    if (bit_bound == 0 || bit_bound > 64) {
        throw OutOfRange(out_.size(), "an @bit_bound between 1 and 64, not " +
                                          std::to_string(bit_bound));
    }
    if (bit_bound < 64 && (value >> bit_bound) != 0) {
        throw OutOfRange(out_.size(), "a bitmask below 2^" + std::to_string(bit_bound) +
                                          ", not " + std::to_string(value));
    }
    if (bit_bound <= 8) {
        write_u8(value);
    } else if (bit_bound <= 16) {
        write_u16(value);
    } else if (bit_bound <= 32) {
        write_u32(value);
    } else {
        write_u64(value);
    }
}

void Writer::write_bitset(unsigned anchor_bytes, std::uint64_t value) {
    switch (anchor_bytes) {
        case 1:
            write_u8(value & 0xFFu);
            return;
        case 2:
            write_u16(value & 0xFFFFu);
            return;
        case 4:
            write_u32(value & 0xFFFF'FFFFu);
            return;
        case 8:
            write_u64(value);
            return;
        default:
            throw OutOfRange(out_.size(), "a bitset anchor of 1, 2, 4 or 8 bytes, not " +
                                              std::to_string(anchor_bytes));
    }
}

void Writer::write_union_overlay(std::uint64_t pattern, unsigned octets) {
    switch (octets) {
        case 1:
            write_u8(pattern & 0xFFu);
            return;
        case 2:
            write_u16(pattern & 0xFFFFu);
            return;
        case 4:
            write_u32(pattern & 0xFFFF'FFFFu);
            return;
        case 8:
            write_u64(pattern);
            return;
        default:
            throw OutOfRange(out_.size(), "a union arm of 1, 2, 4 or 8 octets, not " +
                                              std::to_string(octets));
    }
}

std::uint64_t Writer::bitfield(std::uint64_t value, unsigned width,
                               unsigned position) const {
    if (width == 0 || width > 64 || position + width > 64) {
        throw OutOfRange(out_.size(), "a bitfield inside its anchor, not " +
                                          std::to_string(width) + " bits at " +
                                          std::to_string(position));
    }
    const std::uint64_t limit =
        width >= 64 ? ~std::uint64_t{0} : (std::uint64_t{1} << width) - 1u;
    if (value > limit) {
        throw OutOfRange(out_.size(), "a value a " + std::to_string(width) +
                                          "-bit field can hold, not " +
                                          std::to_string(value));
    }
    return value << position;
}

// -- framing ----------------------------------------------------------------

DHeader Writer::write_dheader() {
    align_to(4);
    DHeader header;
    header.at = out_.size();
    header.present = true;
    out_.insert(out_.end(), 4, std::uint8_t{0});
    return header;
}

void Writer::finish_dheader(DHeader header) {
    if (!header.present) {
        return;
    }
    patch(header.at, out_.size() - header.at - 4, 4);
}

EmHeader Writer::write_emheader(std::uint32_t member_id, unsigned length_code,
                                bool must_understand) {
    if (length_code > 7) {
        // The Python column masks with `& 0x7`. Masking a code the caller
        // got wrong writes a frame that reads back as a different member
        // extent and says nothing, which is the failure class this project
        // treats as the worst one, so it is refused here instead.
        throw OutOfRange(out_.size(),
                         "a length code between 0 and 7, not " + std::to_string(length_code));
    }
    // §7.2.2.4.4.4.4 runs member ids from 0 to 0x0FFFFFFF and §7.4.3.4.2
    // leaves them the low 28 bits of the word. Masking a larger one writes a
    // well-formed header for `id & 0x0FFFFFFF`: the peer reads a member the
    // caller did not name and nothing at either end says so.
    if (member_id > E_MEMBER_ID_MASK) {
        throw OutOfRange(out_.size(), "a member id inside the 28 bits of an EMHEADER, not " +
                                          std::to_string(member_id));
    }
    align_to(4);
    const std::uint32_t word = (static_cast<std::uint32_t>(must_understand ? 1u : 0u) << 31) |
                               (static_cast<std::uint32_t>(length_code & 0x7u) << 28) |
                               (member_id & E_MEMBER_ID_MASK);
    write_u32(word);
    EmHeader header;
    header.word = out_.size() - 4;
    header.length_code = length_code;
    if (length_code == 4) {
        header.next_int = out_.size();
        header.has_next_int = true;
        out_.insert(out_.end(), 4, std::uint8_t{0});
    }
    return header;
}

void Writer::shorten_emheader(EmHeader header, std::size_t length) {
    // Remove an LC4 header's NEXTINT and raise its length code.
    //
    // Every byte of the member's value is unchanged and only its position
    // moves, which is safe for one reason and only under XCDR2: MAXALIGN is
    // four there, so the value began at a four-aligned offset with the
    // NEXTINT in front of it and begins at a four-aligned offset without it.
    unsigned code = 3;
    switch (length) {
        case 1:
            code = 0;
            break;
        case 2:
            code = 1;
            break;
        case 4:
            code = 2;
            break;
        default:
            code = 3;
            break;
    }
    const auto first = out_.begin() + static_cast<std::ptrdiff_t>(header.next_int);
    out_.erase(first, first + 4);
    std::uint32_t previous = 0;
    for (std::size_t i = 0; i < 4; i++) {
        const std::size_t shift = is_little(endianness_) ? i : 3 - i;
        previous |= static_cast<std::uint32_t>(out_[header.word + i]) << (8 * shift);
    }
    patch(header.word, (previous & ~(std::uint32_t{0x7} << 28)) |
                           (static_cast<std::uint32_t>(code) << 28),
          4);
}

void Writer::finish_emheader(EmHeader header) {
    if (!header.has_next_int) {
        // Under codes 0..3 the code *is* the length — 1, 2, 4 or 8 octets,
        // §7.4.3.4.2. A value of another size misdeclares it and the reader's
        // skip lands inside data, so the mismatch is refused here rather than
        // written. Codes 5..7 declare nothing of their own: the member's
        // leading length word is the NEXTINT.
        if (header.length_code <= 3 && header.word + 4 <= out_.size()) {
            const std::size_t declared = std::size_t{1} << header.length_code;
            const std::size_t body = out_.size() - header.word - 4;
            if (body != declared) {
                throw OutOfRange(out_.size(),
                                 "a member of " + std::to_string(declared) +
                                     " octets under length code " +
                                     std::to_string(header.length_code) + ", not " +
                                     std::to_string(body));
            }
        }
        return;
    }
    const std::size_t length = out_.size() - header.next_int - 4;
    if (short_length_codes_ &&
        (length == 1 || length == 2 || length == 4 || length == 8)) {
        shorten_emheader(header, length);
        return;
    }
    patch(header.next_int, length, 4);
}

std::size_t Writer::begin_parameter_value() {
    // "Each element, or parameter, within a parameter list data structure is
    // simply a CDR-encapsulated block of data" — §7.4.1.2. A block counts
    // alignment from its own start, so an eight-byte member at the head of
    // one is already aligned and takes no padding inside its own length.
    const std::size_t saved = origin_;
    origin_ = out_.size();
    return saved;
}

Pid Writer::write_pid(std::uint16_t pid, ParameterRole role, bool must_understand) {
    // §7.4.1.2.2 gives the short form the ids 0 to `highest_short_id`
    // inclusive and reserves what is above it — PID_EXTENDED is 0x3F01
    // itself. A short header with a reserved id collides with the protocol's
    // own parameters, so such an id takes the extended form or none.
    if (pid > pl_cdr1_.highest_short_id) {
        throw OutOfRange(out_.size(),
                         "a parameter id the short header carries, not " + std::to_string(pid) +
                             " — the extended header takes it");
    }
    align_to(4);
    // Two claimants on the same word, deliberately: the member's own
    // declaration, and whatever the framing puts on every id regardless.
    // OpenDDS's `c0010004` is the second alone.
    if (must_understand && pl_cdr1_.member_must_understand) {
        pid = static_cast<std::uint16_t>(pid | PID_MUST_UNDERSTAND);
    }
    pid = static_cast<std::uint16_t>(pid | pid_flags_at(pl_cdr1_.pid_flags,
                                                        WirePosition::Member));
    // The id without either flag: FLAG_IMPL_EXTENSION left in it would be
    // carried into an `eMemberHeader` by a promotion, where it is a
    // different bit.
    const std::uint32_t bare =
        pid & static_cast<std::uint16_t>(~(PID_MUST_UNDERSTAND | PID_IMPL_EXTENSION));
    write_u16(pid);
    Pid header;
    header.at = out_.size();
    header.extended = false;
    header.role = role;
    header.pid = bare;
    header.must_understand = must_understand;
    out_.insert(out_.end(), 2, std::uint8_t{0});
    header.saved_origin = begin_parameter_value();
    return header;
}

Pid Writer::write_pid_extended(std::uint32_t member_id, ParameterRole role,
                               bool must_understand) {
    // The same 28 bits as an EMHEADER's, here in the `eMemberHeader` of
    // §7.4.1.2.1. Masking aliases the parameter onto another member exactly
    // as it does there.
    if (member_id > E_MEMBER_ID_MASK) {
        throw OutOfRange(out_.size(),
                         "a member id inside the 28 bits of an eMemberHeader, not " +
                             std::to_string(member_id));
    }
    align_to(4);
    const std::uint16_t marker = static_cast<std::uint16_t>(
        PID_EXTENDED | pid_flags_at(pl_cdr1_.pid_flags, WirePosition::ExtendedMarker));
    write_u16(marker);
    write_u16(8);
    // The member's must-understand is `FLAG_2` of the `eMemberHeader`
    // (§7.4.1.2.1) and not the marker's bit above it: Cyclone sets the
    // marker's on every parameter and `FLAG_2` only where the member
    // declares it.
    std::uint32_t member_header = member_id & E_MEMBER_ID_MASK;
    if (must_understand && pl_cdr1_.member_must_understand) {
        member_header |= E_FLAG_MUST_UNDERSTAND;
    }
    write_u32(member_header);
    Pid header;
    header.at = out_.size();
    header.extended = true;
    header.role = role;
    header.pid = member_id & E_MEMBER_ID_MASK;
    header.must_understand = must_understand;
    out_.insert(out_.end(), 4, std::uint8_t{0});
    header.saved_origin = begin_parameter_value();
    return header;
}

void Writer::promote_pid(Pid header, std::size_t length) {
    // Rewrite a short parameter header as the extended form.
    //
    // §7.4.1.2.2's `ssize` is sixteen bits, and which values overrun it is a
    // property of the sample rather than of the type — a `string` member is
    // short in almost every sample and over 64 KiB in some. So the plan
    // writes the short form for an unbounded member and the fallback lives
    // here, where the length is known.
    //
    // Eight bytes are inserted between the header and the value. The value's
    // own bytes do not change: XCDR1's MAXALIGN is eight and the insertion
    // is eight, so every offset inside it keeps its remainder.
    const std::size_t start = header.at - 2;
    out_.insert(out_.begin() + static_cast<std::ptrdiff_t>(header.at + 2), 8,
                std::uint8_t{0});
    patch(start,
          PID_EXTENDED | pid_flags_at(pl_cdr1_.pid_flags, WirePosition::ExtendedMarker), 2);
    patch(start + 2, 8, 2);
    std::uint32_t member_header = header.pid & E_MEMBER_ID_MASK;
    if (header.must_understand && pl_cdr1_.member_must_understand) {
        member_header |= E_FLAG_MUST_UNDERSTAND;
    }
    patch(start + 4, member_header, 4);
    patch(start + 8, length, 4);
}

void Writer::finish_pid(Pid header) {
    const std::size_t width = header.extended ? 4u : 2u;
    std::size_t length = out_.size() - header.at - width;
    if (pl_cdr1_.pads_declared_length(header.role)) {
        // Counted from the origin the padding will be measured from, which
        // is the enclosing stream's and not the parameter block's — the
        // realignment below runs after the restore.
        const std::size_t past = (out_.size() - header.saved_origin) % 4;
        if (past != 0) {
            length += 4 - past;
        }
    }
    if (!header.extended && length > PID_SHORT_MAX_LEN) {
        promote_pid(header, length);
    } else {
        patch(header.at, length, width);
    }
    // The padding follows the value and is outside the length. It is
    // measured from the enclosing stream, so the origin goes back before it
    // is written. Under Fast DDS's framing the parameter writes no padding
    // of its own — the next header aligns itself.
    origin_ = header.saved_origin;
    if (pl_cdr1_.pad_after_value) {
        align_to(4);
    }
}

void Writer::write_sentinel() {
    // The flags come from the framing asked at the sentinel position, which
    // is the only thing that decides them: `PID_LIST_END` is not a member
    // and has no declaration of its own.
    align_to(4);
    write_u16(PID_LIST_END | pid_flags_at(pl_cdr1_.pid_flags, WirePosition::Sentinel));
    write_u16(0);
}

// -- the KeyHash ------------------------------------------------------------

std::array<std::uint8_t, KEY_HASH_LEN> Writer::zero_padded() const {
    std::array<std::uint8_t, KEY_HASH_LEN> out{};
    const std::size_t take = out_.size() < KEY_HASH_LEN ? out_.size() : KEY_HASH_LEN;
    for (std::size_t i = 0; i < take; i++) {
        out[i] = out_[i];
    }
    return out;
}

std::array<std::uint8_t, KEY_HASH_LEN> Writer::md5() const {
    return ::zerodds::wire::md5(out_.data(), out_.size());
}

}  // namespace zerodds::wire
