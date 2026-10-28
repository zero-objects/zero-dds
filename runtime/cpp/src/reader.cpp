// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Taking bytes off the wire, in C++, and the mirror of `writer.cpp` clause
// for clause. Every read that cannot be satisfied throws a `WireError`
// naming what was expected and where: a reader that returned a zero for a
// truncated stream would hand the caller a value that was never sent.

#include "zerodds/wire.hpp"

#include <cstring>
#include <string>

namespace zerodds::wire {

namespace {

/// Whether `text` is well-formed UTF-8.
///
/// Written out rather than borrowed for the same reason the encoder is: a
/// surrogate encoded in UTF-8 is refused here, because a decoder that
/// accepted CESU-8 would silently change the text.
bool is_utf8(const std::uint8_t *s, std::size_t size) {
    std::size_t i = 0;
    while (i < size) {
        const std::uint8_t lead = s[i];
        if (lead < 0x80u) {
            i += 1;
            continue;
        }
        if ((lead & 0xE0u) == 0xC0u) {
            if (i + 1 >= size || (s[i + 1] & 0xC0u) != 0x80u) {
                return false;
            }
            const std::uint32_t point =
                static_cast<std::uint32_t>(lead & 0x1Fu) << 6 | (s[i + 1] & 0x3Fu);
            if (point < 0x80u) {
                return false;
            }
            i += 2;
            continue;
        }
        if ((lead & 0xF0u) == 0xE0u) {
            if (i + 2 >= size || (s[i + 1] & 0xC0u) != 0x80u ||
                (s[i + 2] & 0xC0u) != 0x80u) {
                return false;
            }
            const std::uint32_t point =
                static_cast<std::uint32_t>(lead & 0x0Fu) << 12 |
                static_cast<std::uint32_t>(s[i + 1] & 0x3Fu) << 6 | (s[i + 2] & 0x3Fu);
            if (point < 0x800u || (point >= 0xD800u && point <= 0xDFFFu)) {
                return false;
            }
            i += 3;
            continue;
        }
        if ((lead & 0xF8u) == 0xF0u) {
            if (i + 3 >= size || (s[i + 1] & 0xC0u) != 0x80u ||
                (s[i + 2] & 0xC0u) != 0x80u || (s[i + 3] & 0xC0u) != 0x80u) {
                return false;
            }
            const std::uint32_t point =
                static_cast<std::uint32_t>(lead & 0x07u) << 18 |
                static_cast<std::uint32_t>(s[i + 1] & 0x3Fu) << 12 |
                static_cast<std::uint32_t>(s[i + 2] & 0x3Fu) << 6 | (s[i + 3] & 0x3Fu);
            if (point < 0x10000u || point > 0x10FFFFu) {
                return false;
            }
            i += 4;
            continue;
        }
        return false;
    }
    return true;
}

/// Take the byte-order mark off the front and apply what it said.
///
/// The units arrived little-endian, because that is what omniORB writes and
/// what the `ByteOrderMark` order therefore reads. Three cases, and the
/// third is the one that matters:
///
/// - `FEFF` — the mark is the right way up, the units are as read.
/// - `FFFE` — the mark is byte-swapped, so every unit behind it is too, and
///   each one is swapped back. This is what a conforming big-endian omniORB
///   peer produces and what a reader that ignored the mark would return as
///   different characters, well formed and the right length.
/// - no mark — CORBA's rule for a GIOP 1.2 `wstring` without one is UTF-16
///   big-endian, so every unit read little-endian is swapped.
///
/// The mark is never a character of the returned string in any of the three,
/// which is the half §7.4.1.1.2 cares about. The stream's own byte order is
/// not an argument and does not need to be: the mark is written in the same
/// order as the content it introduces. That is what a byte-order mark is for.
std::vector<std::uint16_t> strip_byte_order_mark(std::vector<std::uint16_t> units) {
    if (units.empty()) {
        return units;
    }
    if (units.front() == BYTE_ORDER_MARK) {
        units.erase(units.begin());
        return units;
    }
    if (units.front() == SWAPPED_BYTE_ORDER_MARK) {
        units.erase(units.begin());
    }
    for (std::uint16_t &unit : units) {
        unit = static_cast<std::uint16_t>((unit & 0xFFu) << 8 | (unit >> 8));
    }
    return units;
}

}  // namespace

namespace detail {

std::string utf8_of(const std::vector<char32_t> &points) {
    std::string out;
    for (char32_t point : points) {
        const std::uint32_t value = static_cast<std::uint32_t>(point);
        if (value < 0x80u) {
            out.push_back(static_cast<char>(value));
        } else if (value < 0x800u) {
            out.push_back(static_cast<char>(0xC0u | (value >> 6)));
            out.push_back(static_cast<char>(0x80u | (value & 0x3Fu)));
        } else if (value < 0x10000u) {
            out.push_back(static_cast<char>(0xE0u | (value >> 12)));
            out.push_back(static_cast<char>(0x80u | ((value >> 6) & 0x3Fu)));
            out.push_back(static_cast<char>(0x80u | (value & 0x3Fu)));
        } else {
            out.push_back(static_cast<char>(0xF0u | (value >> 18)));
            out.push_back(static_cast<char>(0x80u | ((value >> 12) & 0x3Fu)));
            out.push_back(static_cast<char>(0x80u | ((value >> 6) & 0x3Fu)));
            out.push_back(static_cast<char>(0x80u | (value & 0x3Fu)));
        }
    }
    return out;
}

}  // namespace detail

// -- state ------------------------------------------------------------------

Reader::Reader(const std::uint8_t *bytes, std::size_t length, Endianness endianness,
               std::size_t max_align, PlCdr1Framing framing)
    : bytes_(bytes),
      length_(length),
      endianness_(endianness),
      max_align_(max_align < 1 ? std::size_t{1} : max_align),
      pl_cdr1_(framing) {}

Reader::Reader(const std::vector<std::uint8_t> &bytes, Endianness endianness,
               std::size_t max_align, PlCdr1Framing framing)
    : Reader(bytes.data(), bytes.size(), endianness, max_align, framing) {}

void Reader::align_to(std::size_t align) {
    if (align > max_align_) {
        align = max_align_;
    }
    if (align <= 1) {
        return;
    }
    const std::size_t offset = at_ - origin_;
    at_ += (align - (offset % align)) % align;
}

std::size_t Reader::reader_begin_packed_element() {
    const std::size_t saved = origin_;
    origin_ = at_;
    return saved;
}

void Reader::reader_finish_packed_element(std::size_t saved) { origin_ = saved; }

const std::uint8_t *Reader::take(std::size_t count) {
    if (at_ > length_ || count > length_ - at_) {
        throw Truncated(at_, count, remaining());
    }
    const std::uint8_t *chunk = bytes_ + at_;
    at_ += count;
    return chunk;
}

void Reader::skip(std::size_t count) { take(count); }

void Reader::seek(std::size_t at) {
    if (at > length_) {
        throw Truncated(at_, at, length_);
    }
    at_ = at;
}

std::uint64_t Reader::read_bits(std::size_t width) {
    align_to(width);
    const std::uint8_t *raw = take(width);
    std::uint64_t value = 0;
    for (std::size_t i = 0; i < width; i++) {
        const std::size_t shift = is_little(endianness_) ? i : width - 1 - i;
        value |= static_cast<std::uint64_t>(raw[i]) << (8 * shift);
    }
    return value;
}

// -- primitives -------------------------------------------------------------

bool Reader::read_bool() {
    const std::size_t at = at_;
    const std::uint8_t value = read_u8();
    if (value == 0) {
        return false;
    }
    if (value == 1) {
        return true;
    }
    throw OutOfRange(at, "a boolean of 0 or 1");
}

std::uint8_t Reader::read_u8() { return take(1)[0]; }

void Reader::read_octets(std::uint8_t *out, std::size_t count) {
    const std::uint8_t *chunk = take(count);
    if (count != 0) {
        std::memcpy(out, chunk, count);
    }
}

std::int8_t Reader::read_i8() {
    const std::uint8_t value = take(1)[0];
    return static_cast<std::int8_t>(value >= 0x80u ? static_cast<int>(value) - 0x100
                                                   : static_cast<int>(value));
}

char32_t Reader::read_char() { return static_cast<char32_t>(take(1)[0]); }

char32_t Reader::read_wchar(WcharWidth width, WcharFraming framing,
                            WstringUnitOrder order) {
    const std::size_t at = at_;
    const std::size_t octets = static_cast<std::size_t>(width);
    std::uint32_t value = 0;
    if (framing == WcharFraming::GiopLengthPrefixed) {
        // The octet count is checked against the configured code-set width
        // rather than believed: a stream whose first octet says 0xE4 — what
        // a bare two-octet `wchar` looks like read this way — would
        // otherwise consume 228 octets of whatever followed and return a
        // character assembled out of the next four members.
        const std::uint8_t declared = read_u8();
        if (declared != octets) {
            throw OutOfRange(at, "a GIOP 1.2 wchar of " + std::to_string(octets) +
                                     " octets, the configured transmission code set, "
                                     "and not " +
                                     std::to_string(declared));
        }
        // A scalar cannot carry a byte-order mark, so CORBA's default
        // applies here and any order but the stream's means big-endian.
        const bool little =
            order == WstringUnitOrder::Stream && is_little(endianness_);
        const std::uint8_t *raw = take(octets);
        for (std::size_t i = 0; i < octets; i++) {
            const std::size_t shift = little ? i : octets - 1 - i;
            value |= static_cast<std::uint32_t>(raw[i]) << (8 * shift);
        }
    } else if (octets == 4) {
        value = read_u32();
        if (value > 0xFFFFu) {
            throw OutOfRange(at, "a four-byte wchar holding one UTF-16 code unit");
        }
    } else {
        value = read_u16();
    }
    if (value >= 0xD800u && value <= 0xDFFFu) {
        throw OutOfRange(at, "a Unicode code point");
    }
    return static_cast<char32_t>(value);
}

std::int16_t Reader::read_i16() {
    return static_cast<std::int16_t>(static_cast<std::uint16_t>(read_bits(2)));
}

std::uint16_t Reader::read_u16() { return static_cast<std::uint16_t>(read_bits(2)); }

std::int32_t Reader::read_i32() {
    return static_cast<std::int32_t>(static_cast<std::uint32_t>(read_bits(4)));
}

std::uint32_t Reader::read_u32() { return static_cast<std::uint32_t>(read_bits(4)); }

std::int64_t Reader::read_i64() { return static_cast<std::int64_t>(read_bits(8)); }

std::uint64_t Reader::read_u64() { return read_bits(8); }

float Reader::read_f32() {
    const std::uint32_t bits = static_cast<std::uint32_t>(read_bits(4));
    float value = 0.0f;
    std::memcpy(&value, &bits, sizeof value);
    return value;
}

double Reader::read_f64() {
    const std::uint64_t bits = read_bits(8);
    double value = 0.0;
    std::memcpy(&value, &bits, sizeof value);
    return value;
}

Float128 Reader::read_f128() {
    align_to(8);
    const std::uint8_t *raw = take(16);
    std::array<std::uint8_t, 16> bytes{};
    for (std::size_t i = 0; i < 16; i++) {
        bytes[i] = raw[i];
    }
    return Float128::from_bytes(bytes, endianness_);
}

std::uint16_t Reader::read_u16_little() {
    align_to(2);
    const std::uint8_t *raw = take(2);
    return static_cast<std::uint16_t>(raw[0] | (static_cast<unsigned>(raw[1]) << 8));
}

std::uint16_t Reader::read_u16_big() {
    align_to(2);
    const std::uint8_t *raw = take(2);
    return static_cast<std::uint16_t>((static_cast<unsigned>(raw[0]) << 8) | raw[1]);
}

std::uint32_t Reader::read_u32_little() {
    align_to(4);
    const std::uint8_t *raw = take(4);
    std::uint32_t value = 0;
    for (std::size_t i = 0; i < 4; i++) {
        value |= static_cast<std::uint32_t>(raw[i]) << (8 * i);
    }
    return value;
}

std::uint32_t Reader::read_u32_big() {
    align_to(4);
    const std::uint8_t *raw = take(4);
    std::uint32_t value = 0;
    for (std::size_t i = 0; i < 4; i++) {
        value |= static_cast<std::uint32_t>(raw[i]) << (8 * (3 - i));
    }
    return value;
}

// -- strings ----------------------------------------------------------------

std::string Reader::read_string() {
    const std::size_t at = at_;
    const std::uint32_t length = read_u32();
    if (length == 0 || length > remaining() + 1) {
        throw ImpossibleLength(at, length, remaining());
    }
    const std::uint8_t *raw = take(length - 1);
    if (!is_utf8(raw, length - 1)) {
        throw NotUtf8(at);
    }
    read_u8();  // the terminator the length counted
    return std::string(reinterpret_cast<const char *>(raw), length - 1);
}

std::vector<std::uint16_t> Reader::wstring_units(std::size_t at, std::size_t count,
                                                 std::size_t width,
                                                 WstringUnitOrder order) {
    if (count > remaining() / width || count * width > remaining()) {
        throw ImpossibleLength(at, count * width, remaining());
    }
    // C3 picks the reader and the unit order picks its endianness.
    const bool little = order == WstringUnitOrder::ByteOrderMark ||
                        order == WstringUnitOrder::HostUnswapped;
    std::vector<std::uint16_t> units;
    units.reserve(count);
    for (std::size_t i = 0; i < count; i++) {
        if (width == 4) {
            std::uint32_t wide = 0;
            if (order == WstringUnitOrder::BigEndianAlways) {
                wide = read_u32_big();
            } else if (little) {
                wide = read_u32_little();
            } else {
                wide = read_u32();
            }
            // The four-byte width is RTI's XCDR1 form, where a character is
            // a code unit zero-extended. A value above 0xFFFF is not a form
            // this reader knows how to narrow, so it is refused rather than
            // truncated — truncating is the defect this whole policy exists
            // because of.
            if (wide > 0xFFFFu) {
                throw OutOfRange(at, "a four-byte wstring character holding one UTF-16 unit");
            }
            units.push_back(static_cast<std::uint16_t>(wide));
        } else if (order == WstringUnitOrder::BigEndianAlways) {
            units.push_back(read_u16_big());
        } else if (little) {
            units.push_back(read_u16_little());
        } else {
            units.push_back(read_u16());
        }
    }
    return units;
}

std::string Reader::read_wstring(WstringLengthPolicy policy, WstringUnitOrder order) {
    const std::size_t at = at_;
    const std::uint32_t declared = read_u32();
    // The three coordinates, not a list of named forms. C1 says whether the
    // count is of units or of the octets they occupy, C3 says what an octet
    // count divides by, and C2 is handled after the run because a terminator
    // occupies a unit on the wire whichever way it is counted.
    const bool counts_characters = wstring_counts_characters(policy);
    const bool terminated = wstring_terminated(policy);
    const std::size_t width = wstring_unit_width(policy);

    std::vector<std::uint16_t> units;
    if (counts_characters) {
        units = wstring_units(at, declared, width, order);
    } else {
        if (declared % width != 0) {
            throw ImpossibleLength(at, declared, remaining());
        }
        units = wstring_units(at, declared / width, width, order);
    }
    if (terminated) {
        // The count includes the NUL, so a count of zero is a frame that
        // does not hold its own terminator.
        if (declared < 1 || units.empty()) {
            throw ImpossibleLength(at, declared, remaining());
        }
        const std::uint16_t last = units.back();
        units.pop_back();
        if (last != 0) {
            throw OutOfRange(at, "a NUL character ending the wstring, as its count includes one");
        }
        // A NUL inside the text is the narrow string's problem in wide form:
        // every C peer stops at it. Only where a terminator is expected — a
        // cell without one is entitled to carry U+0000 as a character.
        for (std::uint16_t unit : units) {
            if (unit == 0) {
                throw OutOfRange(at, "no NUL inside a wstring a C peer would stop at");
            }
        }
    }
    if (order == WstringUnitOrder::ByteOrderMark) {
        units = strip_byte_order_mark(std::move(units));
    }

    std::vector<char32_t> points;
    points.reserve(units.size());
    std::size_t index = 0;
    while (index < units.size()) {
        const std::uint16_t unit = units[index];
        if (unit >= 0xD800u && unit <= 0xDBFFu && index + 1 < units.size()) {
            const std::uint16_t low = units[index + 1];
            if (low >= 0xDC00u && low <= 0xDFFFu) {
                points.push_back(static_cast<char32_t>(
                    0x10000u + ((static_cast<std::uint32_t>(unit) - 0xD800u) << 10) +
                    (static_cast<std::uint32_t>(low) - 0xDC00u)));
                index += 2;
                continue;
            }
        }
        if (unit >= 0xD800u && unit <= 0xDFFFu) {
            throw OutOfRange(at, "a UTF-16 sequence with no lone surrogate");
        }
        points.push_back(static_cast<char32_t>(unit));
        index += 1;
    }
    return detail::utf8_of(points);
}

std::string Reader::read_wstring_utf8_octets(WstringLengthPolicy policy) {
    const std::size_t at = at_;
    const std::uint32_t declared = read_u32();
    if (declared > remaining()) {
        throw ImpossibleLength(at, declared, remaining());
    }
    const std::uint8_t *raw = take(declared);
    std::string text(reinterpret_cast<const char *>(raw), declared);
    if (wstring_terminated(policy)) {
        if (declared == 0 || text.back() != '\0') {
            throw OutOfRange(at, "a NUL octet ending the wstring, as its count includes one");
        }
        text.pop_back();
        // A NUL inside the text is the narrow string's problem in wide form:
        // every C peer stops at it, so the value this reader returns and the
        // value the peer returns would differ for bytes both accepted.
        if (text.find('\0') != std::string::npos) {
            throw OutOfRange(at, "no NUL inside a wstring a C peer would stop at");
        }
    }
    // The same walk the writer's UTF-16 form uses, so the two directions
    // agree about what the axis means; an ill-formed sequence throws
    // `NotUtf8` and is never replaced or truncated.
    (void)detail::utf8_code_points(text);
    return text;
}

std::string Reader::read_fixed(unsigned digits, unsigned scale, FixedDigitCount count) {
    // The mirror of `Writer::write_fixed`: `ceil((digits + 1) / 2)` bytes of
    // BCD, digits most-significant first, sign nibble last.
    //
    // The same bound as the writer's, on this side too: OMG IDL 4.2 rule (42)
    // caps the BCD rule at 31 digits, and a reader asked for a wider field
    // would take octets no declaration can name.
    if (digits == 0 || digits > 31 || scale > digits) {
        throw OutOfRange(at_, "a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<" +
                                  std::to_string(digits) + ", " + std::to_string(scale) + ">");
    }
    std::vector<std::uint8_t> nibbles;
    std::uint8_t sign = 0;
    if (count == FixedDigitCount::ValuePrecision || count == FixedDigitCount::ValueStringDigits) {
        // [CDR] §9.3.2.8 makes a digit `0` to `9` and a sign `0xa` upwards,
        // and the nibble count is even by construction, so the field ends at
        // the first octet whose **low** nibble is a sign. A field that never
        // presents one within the sixteen octets of the widest `fixed` IDL
        // admits is refused.
        for (;;) {
            if (nibbles.size() >= 32) {
                throw OutOfRange(at_, "a sign nibble within sixteen octets");
            }
            const std::uint8_t byte = take(1)[0];
            nibbles.push_back(static_cast<std::uint8_t>(byte >> 4));
            nibbles.push_back(static_cast<std::uint8_t>(byte & 0x0Fu));
            if ((byte & 0x0Fu) >= 0x0Au) {
                break;
            }
        }
        sign = nibbles.back();
        nibbles.pop_back();
    } else {
        const std::size_t octets = (static_cast<std::size_t>(digits) + 2) / 2;
        const std::uint8_t *raw = take(octets);
        for (std::size_t i = 0; i < octets; i++) {
            nibbles.push_back(static_cast<std::uint8_t>(raw[i] >> 4));
            nibbles.push_back(static_cast<std::uint8_t>(raw[i] & 0x0Fu));
        }
        sign = nibbles.back();
        nibbles.pop_back();
        if ((digits + 1) % 2 == 1) {
            nibbles.erase(nibbles.begin());
        }
    }
    for (std::uint8_t nibble : nibbles) {
        if (nibble > 9) {
            throw OutOfRange(at_, "BCD digits");
        }
    }
    // The point sits `scale` digits from the right and not `digits - scale`
    // from the left: under the value's count the field carries fewer than
    // `digits`, and one that carries fewer than the scale — `1c` under
    // `fixed<5,2>` — needs its integer part written as a zero.
    std::string text;
    text.reserve(nibbles.size());
    for (std::uint8_t nibble : nibbles) {
        text.push_back(static_cast<char>('0' + nibble));
    }
    while (text.size() < static_cast<std::size_t>(scale) + 1) {
        text.insert(text.begin(), '0');
    }
    const std::size_t split = text.size() - scale;
    std::string whole = text.substr(0, split);
    const std::string fraction = text.substr(split);
    std::size_t first = whole.find_first_not_of('0');
    whole = first == std::string::npos ? std::string("0") : whole.substr(first);
    // Leading zeros go because the BCD field is `digits` wide whatever the
    // value: returning `"000.01"` would put a decimal on the read side that
    // the write side would never produce from the same value.
    std::string out = scale == 0 ? whole : whole + "." + fraction;
    const bool zero =
        whole == "0" && fraction.find_first_not_of('0') == std::string::npos;
    // A negative zero loses its sign for the same reason: `-0.00` and `0.00`
    // are one value, and the sign nibble is the only place they differ.
    return (sign == 0x0D && !zero) ? "-" + out : out;
}

// -- the bit-bounded kinds --------------------------------------------------

std::int64_t Reader::read_enum(unsigned bit_bound) {
    if (bit_bound == 0 || bit_bound > 32) {
        throw OutOfRange(at_, "an @bit_bound between 1 and 32, not " +
                                  std::to_string(bit_bound));
    }
    std::uint64_t raw = 0;
    if (bit_bound <= 8) {
        raw = read_u8();
    } else if (bit_bound <= 16) {
        raw = read_u16();
    } else {
        raw = read_u32();
    }
    // The holder is signed — §7.4.3.5.3 rule (5) — so a pattern with the
    // top bit of the bound set reads back negative.
    const std::uint64_t sign_bit = std::uint64_t{1} << (bit_bound - 1);
    if ((raw & sign_bit) != 0) {
        const std::uint64_t span = std::uint64_t{1} << bit_bound;
        return static_cast<std::int64_t>(raw) - static_cast<std::int64_t>(span);
    }
    return static_cast<std::int64_t>(raw);
}

std::uint64_t Reader::read_bitmask(unsigned bit_bound) {
    if (bit_bound == 0 || bit_bound > 64) {
        throw OutOfRange(at_, "an @bit_bound between 1 and 64, not " +
                                  std::to_string(bit_bound));
    }
    if (bit_bound <= 8) {
        return read_u8();
    }
    if (bit_bound <= 16) {
        return read_u16();
    }
    if (bit_bound <= 32) {
        return read_u32();
    }
    return read_u64();
}

std::uint64_t Reader::read_bitset(unsigned anchor_bytes) {
    switch (anchor_bytes) {
        case 1:
            return read_u8();
        case 2:
            return read_u16();
        case 4:
            return read_u32();
        case 8:
            return read_u64();
        default:
            throw OutOfRange(at_, "a bitset anchor of 1, 2, 4 or 8 bytes, not " +
                                      std::to_string(anchor_bytes));
    }
}

// -- framing ----------------------------------------------------------------

std::uint32_t Reader::read_sequence_length() {
    const std::size_t at = at_;
    const std::uint32_t count = read_u32();
    if (count > remaining()) {
        throw ImpossibleLength(at, count, remaining());
    }
    return count;
}

std::uint32_t Reader::read_dheader() {
    const std::size_t at = at_;
    const std::uint32_t length = read_u32();
    if (length > remaining()) {
        throw ImpossibleLength(at, length, remaining());
    }
    return length;
}

EmHeaderRead Reader::read_emheader() {
    const std::uint32_t word = read_u32();
    EmHeaderRead header;
    header.member_id = word & E_MEMBER_ID_MASK;
    header.length_code = (word >> 28) & 0x7u;
    header.must_understand = (word >> 31) != 0;
    if (header.length_code <= 3) {
        // Codes 0 to 3 state a size of one, two, four or eight bytes
        // outright. §7.4.3.4.2.
        header.extent = std::size_t{1} << header.length_code;
    } else if (header.length_code == 4) {
        header.next_int = read_u32();
        header.has_next_int = true;
        header.extent = header.next_int;
    } else {
        // Codes 5 to 7 report the NEXTINT they read, but the reader is put
        // back before it: that word is the member's own leading length and
        // the member's decoder has to see it. §7.4.3.4.2. So the member
        // spans that word plus the length it states, times one, four or
        // eight.
        const std::size_t at = at_;
        header.next_int = read_u32();
        header.has_next_int = true;
        at_ = at;
        const std::size_t scale =
            header.length_code == 5 ? 1u : (header.length_code == 6 ? 4u : 8u);
        header.extent = static_cast<std::size_t>(header.next_int) * scale + 4u;
    }
    // A member longer than what is left is a header that disagrees with
    // its own stream. Refused rather than thrown, so a `@mutable`
    // aggregate's loop sees it the way it sees every other refusal.
    if (header.extent > remaining()) {
        reader_refuse(Status::ImpossibleLength);
    }
    return header;
}

PidRead Reader::read_pid(ParameterRole role) {
    align_to(4);
    const std::uint16_t pid = read_u16();
    const std::uint16_t length = read_u16();
    PidRead header;
    header.role = role;
    // Where the origin was, so a header that throws leaves it named. The
    // move itself happens on each success path below, after the last of
    // the header's own words has been read — a parameter is its own
    // encapsulated block (§7.4.1.2) and the block starts at the value.
    header.saved_origin = origin_;
    if ((pid & PID_ID_MASK) == (PID_EXTENDED & PID_ID_MASK)) {
        // The four flag bits are not part of the id: §7.4.1.2.1 splits the
        // `eMemberHeader` into `FLAG_1` to `FLAG_4` and a 28-bit member id,
        // and `FLAG_2` is the member's must-understand. Taking the whole
        // word gave a Cyclone member declared `@must_understand` an id of
        // `0x40000001`, which matches nothing, and the member was dropped.
        const std::uint32_t member_header = read_u32();
        header.length = read_u32();
        header.pid = member_header & E_MEMBER_ID_MASK;
        header.end = false;
        header.must_understand = (member_header & E_FLAG_MUST_UNDERSTAND) != 0;
        // FLAG_1 is FLAG_IMPL_EXTENSION here exactly as bit 15 is in the
        // short header. The extended id is 28 bits and has no reserved range.
        header.impl_extension = (member_header & E_FLAG_IMPL_EXTENSION) != 0;
        header.saved_origin = begin_parameter_value();
        return header;
    }
    const std::uint16_t identifier = pid & PID_ID_MASK;
    header.pid = identifier;
    header.length = length;
    header.end = identifier == (PID_LIST_END & PID_ID_MASK);
    header.must_understand = (pid & PID_MUST_UNDERSTAND) != 0;
    // Under a framing that puts FLAG_IMPL_EXTENSION on every member id the
    // bit no longer tells a vendor-private parameter from a user member —
    // OpenDDS writes `c0010004` for `@id(1) long a`. It is reported absent
    // there because under that framing it carries no information, and
    // reporting it would send every member down the unknown-parameter path.
    header.impl_extension =
        (pid & PID_IMPL_EXTENSION) != 0 &&
        (pid_flags_at(pl_cdr1_.pid_flags, WirePosition::Member) & PID_IMPL_EXTENSION) == 0;
    header.reserved = !header.end && identifier > pl_cdr1_.highest_short_id;
    header.ignore = identifier == PID_IGNORE;
    // The sentinel ends the list and opens no value block, so the origin
    // stays where it is. A decoder stops at the sentinel without calling
    // finish_pid_read; an origin moved past it stayed moved, and the
    // parameter that holds the list then measured its value from the
    // sentinel. A mutable value nested in a parameter of a mutable one was
    // refused that way, its own bytes and the Rust column's alike
    // (recursive-optional-external under XCDR1: "at 24: a value the type
    // does not admit", the inner sentinel at 20, the enclosing `next`
    // declared 12 and measured 0).
    if (header.end) {
        return header;
    }
    header.saved_origin = begin_parameter_value();
    return header;
}

void Reader::skip_member(const EmHeaderRead &header) {
    // The whole point of the length code. §7.4.3.4.2: codes 0 to 3 state a
    // size of one, two, four or eight bytes outright; code 4 is followed by
    // a NEXTINT holding the member's length; and codes 5, 6 and 7 have a
    // NEXTINT that is *also* the member's own leading word, so the member
    // spans that word plus the length it states, times one, four or eight.
    if (header.length_code <= 3) {
        skip(std::size_t{1} << header.length_code);
        return;
    }
    if (!header.has_next_int) {
        throw OutOfRange(at_, "a NEXTINT for a length code of 4 or above");
    }
    if (header.length_code == 4) {
        skip(header.next_int);
        return;
    }
    const std::size_t scale =
        header.length_code == 5 ? 1u : (header.length_code == 6 ? 4u : 8u);
    skip(4 + static_cast<std::size_t>(header.next_int) * scale);
}

std::size_t Reader::begin_parameter_value() {
    const std::size_t saved = origin_;
    origin_ = at_;
    return saved;
}

void Reader::finish_parameter_value(std::size_t saved_origin) {
    origin_ = saved_origin;
    align_to(4);
}

namespace {

/// The declared length against what the configured framing would have
/// declared for the value that was just read.
///
/// A value longer than the header allows is the malformed case and is the
/// caller's to refuse. This is the other one: a length that is well formed
/// and belongs to a different implementation. RTI rounds a member's
/// declared length up to the alignment where Fast DDS writes the exact one,
/// and `vendor_bytes.md` §2 records both for the same `short`. A member
/// whose value already fills its four-byte slot reveals nothing either way,
/// which is a property of the bytes and not of this check.
///
/// The second derivation of `runtime/c`'s `check_declared_length`, clause
/// for clause and not a call into it.
struct DeclaredLength {
    std::size_t expected;
    std::size_t other;
    bool pads;
};

DeclaredLength declared_length(std::size_t read, const PlCdr1Framing &framing,
                               ParameterRole role) {
    const std::size_t padded = (read + 3u) & ~std::size_t{3};
    const bool pads = framing.pads_declared_length(role);
    return DeclaredLength{pads ? padded : read, pads ? read : padded, pads};
}

}  // namespace

void Reader::finish_pid_read(const PidRead &header) {
    if (!header.end) {
        const std::size_t read = at_ >= origin_ ? at_ - origin_ : std::size_t{0};
        const DeclaredLength shape = declared_length(read, pl_cdr1_, header.role);
        if (static_cast<std::size_t>(header.length) != shape.expected) {
            if (static_cast<std::size_t>(header.length) == shape.other) {
                reader_refuse(Status::WrongFraming);
            } else {
                reader_refuse(Status::OutOfRange);
            }
        }
    }
    finish_parameter_value(header.saved_origin);
}

// -- what a generated decoder says and asks ---------------------------------

const char *status_name(Status status) noexcept {
    switch (status) {
        case Status::Ok:
            return "ok";
        case Status::ShortBuffer:
            return "no room for the value";
        case Status::Range:
            return "a value outside what its field holds";
        case Status::BadArgument:
            return "a parameter no clause allows";
        case Status::NotUtf8:
            return "bytes that are not UTF-8";
        case Status::BadDecimal:
            return "a fixed whose digits are not a decimal";
        case Status::Truncated:
            return "the stream ended where a value was expected";
        case Status::ImpossibleLength:
            return "a length no stream that long can carry";
        case Status::OutOfRange:
            return "a value the type does not admit";
        case Status::WrongFraming:
            return "well-formed bytes framed for another implementation";
        case Status::Discarded:
            return "the sample is discarded";
    }
    // Not reachable for a value of the enumeration, and answered rather
    // than left to fall off the end: a `switch` with no `default` is what
    // makes a new status a compile error here, and a function with no
    // return after it is undefined behaviour in the one case the compiler
    // cannot rule out.
    return "an unnamed status";
}

void Reader::reader_refuse(Status status) noexcept {
    if (status == Status::Ok || status_ != Status::Ok) {
        return;
    }
    status_ = status;
    error_at_ = at_;
}

bool Reader::reader_clear_discard() noexcept {
    if (status_ != Status::Discarded) {
        return false;
    }
    status_ = Status::Ok;
    return true;
}

ReaderMark Reader::reader_save() const noexcept {
    return ReaderMark{at_, origin_, status_};
}

void Reader::reader_restore(ReaderMark mark) noexcept {
    at_ = mark.at;
    origin_ = mark.origin;
    status_ = mark.status;
}

void Reader::reader_skip_to(std::size_t target) {
    if (status_ != Status::Ok || target <= at_) {
        return;
    }
    if (target > length_) {
        reader_refuse(Status::Truncated);
        return;
    }
    at_ = target;
}

namespace {

/// How many octets the UTF-8 sequence starting with `lead` takes.
///
/// A byte that leads nothing takes one, so a walk over ill-formed text
/// still terminates. The reads that produced the text have already refused
/// anything that is not UTF-8.
std::size_t utf8_step(std::uint8_t lead) noexcept {
    if ((lead & 0xE0u) == 0xC0u) {
        return 2;
    }
    if ((lead & 0xF0u) == 0xE0u) {
        return 3;
    }
    if ((lead & 0xF8u) == 0xF0u) {
        return 4;
    }
    return 1;
}

/// The shared body of the two trims: walk whole characters and stop before
/// the one that would cross the limit. `count_units` says whether a
/// character costs its UTF-8 octets or its UTF-16 code units.
std::string trim_to(const std::string &text, std::size_t limit, bool count_units) {
    std::size_t at = 0;
    std::size_t used = 0;
    while (at < text.size()) {
        const std::size_t step = utf8_step(static_cast<std::uint8_t>(text[at]));
        // A scalar value above the Basic Multilingual Plane is a surrogate
        // pair, and four UTF-8 bytes is exactly the set of those.
        const std::size_t cost = count_units ? (step == 4 ? 2u : 1u) : step;
        if (used + cost > limit) {
            break;
        }
        used += cost;
        at += step;
    }
    return text.substr(0, at);
}

}  // namespace

std::size_t utf8_octets(const std::string &text) noexcept { return text.size(); }

std::size_t utf16_units(const std::string &text) noexcept {
    std::size_t units = 0;
    std::size_t at = 0;
    while (at < text.size()) {
        const std::size_t step = utf8_step(static_cast<std::uint8_t>(text[at]));
        units += (step == 4 ? 2u : 1u);
        at += step;
    }
    return units;
}

std::string Reader::trim_utf8(const std::string &text, std::size_t octets) const {
    return trim_to(text, octets, false);
}

std::string Reader::trim_utf16(const std::string &text, std::size_t units) const {
    return trim_to(text, units, true);
}

bool pid_is_user_member(const PidRead &header) noexcept {
    return header.is_user_member();
}

}  // namespace zerodds::wire
