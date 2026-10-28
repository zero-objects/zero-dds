// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-AMQP 1.0 §7 / §8.1 — the C++ runtime the generated helpers call.
//
// Header-only and standard-library-only: a generated file includes this
// and needs nothing else. The octets it writes are OASIS AMQP 1.0 Part 1
// §1.6 primitives; the framing decisions — which field goes where, which
// body form, which descriptor — belong to the generated helper and are
// not repeated here.
//
// This is a runtime and not generated code. `crates/amqp-bridge/`'s Rust
// encoder implements the same clause independently, and
// `tests/cross_language_agreement.rs` compiles this, runs it, and
// compares the octets. Where the two disagree one of them has misread
// the specification, which is the only reason to have a second.

#ifndef ZERODDS_AMQP_CODEC_HPP
#define ZERODDS_AMQP_CODEC_HPP

#include <charconv>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <string>
#include <vector>

namespace zerodds {
namespace amqp {

using Bytes = std::vector<std::uint8_t>;

// §7.1.3 — the two halves of a DDS `Time_t`, as a generated carrier
// declares them. Here and not in the generated file because every
// generated file that mentions a `Time_t` would otherwise declare its
// own, and two of them in one translation unit do not link.
struct TimeT {
    std::int64_t sec;
    std::uint32_t nanosec;
};

// -- primitives ------------------------------------------------------

inline void putBigEndian(Bytes& out, std::uint64_t value, int width) {
    for (int i = width - 1; i >= 0; --i) {
        out.push_back(static_cast<std::uint8_t>((value >> (8 * i)) & 0xFFu));
    }
}

inline Bytes fixed(std::uint8_t code, std::uint64_t value, int width) {
    Bytes out;
    out.push_back(code);
    putBigEndian(out, value, width);
    return out;
}

// §1.6.1
inline Bytes encodeNull() { return Bytes(1, 0x40u); }

// §1.6.2 — the compact form.
inline Bytes encodeBoolean(bool value) {
    return Bytes(1, static_cast<std::uint8_t>(value ? 0x41u : 0x42u));
}

inline Bytes encodeUbyte(std::uint8_t value) { return fixed(0x50u, value, 1); }
inline Bytes encodeByte(std::int8_t value) {
    return fixed(0x51u, static_cast<std::uint8_t>(value), 1);
}
inline Bytes encodeUshort(std::uint16_t value) { return fixed(0x60u, value, 2); }
inline Bytes encodeShort(std::int16_t value) {
    return fixed(0x61u, static_cast<std::uint16_t>(value), 2);
}
// §7.1's table gives these four rows two or three codes each — "compact
// form for -128 <= n <= 127", "compact / zero" — and the choice is part
// of the encoding a peer compares. The narrow form whenever it fits, in
// every column and in the Rust encoder.
inline Bytes encodeUint(std::uint32_t value) {
    if (value == 0u) return Bytes(1, 0x43u);
    if (value <= 0xFFu) return fixed(0x52u, value, 1);
    return fixed(0x70u, value, 4);
}
inline Bytes encodeInt(std::int32_t value) {
    if (value >= -128 && value <= 127) {
        return fixed(0x54u, static_cast<std::uint8_t>(static_cast<std::int8_t>(value)), 1);
    }
    return fixed(0x71u, static_cast<std::uint32_t>(value), 4);
}
inline Bytes encodeUlong(std::uint64_t value) {
    if (value == 0u) return Bytes(1, 0x44u);
    if (value <= 0xFFu) return fixed(0x53u, value, 1);
    return fixed(0x80u, value, 8);
}
inline Bytes encodeLong(std::int64_t value) {
    if (value >= -128 && value <= 127) {
        return fixed(0x55u, static_cast<std::uint8_t>(static_cast<std::int8_t>(value)), 1);
    }
    return fixed(0x81u, static_cast<std::uint64_t>(value), 8);
}

// §1.6.11 / §1.6.12 — IEEE 754 big-endian.
inline Bytes encodeFloat(float value) {
    std::uint32_t bits = 0;
    std::memcpy(&bits, &value, sizeof(bits));
    return fixed(0x72u, bits, 4);
}

inline Bytes encodeDouble(double value) {
    std::uint64_t bits = 0;
    std::memcpy(&bits, &value, sizeof(bits));
    return fixed(0x82u, bits, 8);
}

// §1.6.16 — a UTF-32 code point.
inline Bytes encodeChar(std::uint32_t codepoint) { return fixed(0x73u, codepoint, 4); }

// §1.6.17 — milliseconds since the Unix epoch.
inline Bytes encodeTimestamp(std::int64_t millis) {
    return fixed(0x83u, static_cast<std::uint64_t>(millis), 8);
}

// The narrow form for up to 255 octets and the wide one beyond, which
// is the pair §1.6.19 to §1.6.21 define for each of binary, string and
// symbol.
inline Bytes lengthPrefixed(std::uint8_t narrow, std::uint8_t wide, const std::uint8_t* data,
                            std::size_t size) {
    Bytes out;
    if (size <= 0xFFu) {
        out.push_back(narrow);
        out.push_back(static_cast<std::uint8_t>(size));
    } else {
        out.push_back(wide);
        putBigEndian(out, static_cast<std::uint64_t>(size), 4);
    }
    out.insert(out.end(), data, data + size);
    return out;
}

// §1.6.20 — str8 / str32-utf8.
inline Bytes encodeStr(const std::string& value) {
    return lengthPrefixed(0xA1u, 0xB1u, reinterpret_cast<const std::uint8_t*>(value.data()),
                          value.size());
}

// §1.6.21 — sym8 / sym32.
inline Bytes encodeSymbol(const std::string& value) {
    return lengthPrefixed(0xA3u, 0xB3u, reinterpret_cast<const std::uint8_t*>(value.data()),
                          value.size());
}

// §1.6.19 — vbin8 / vbin32.
inline Bytes encodeBinary(const Bytes& value) {
    return lengthPrefixed(0xA0u, 0xB0u, value.data(), value.size());
}

// -- compound --------------------------------------------------------

// §1.6.22 — list0 / list8 / list32.
//
// §7.1.7: "An empty sequence or array SHALL be encoded as the list0
// type-code 0x45, not as an empty 0xC0."
inline Bytes encodeList(const std::vector<Bytes>& items) {
    if (items.empty()) return Bytes(1, 0x45u);
    Bytes body;
    for (std::size_t i = 0; i < items.size(); ++i) {
        body.insert(body.end(), items[i].begin(), items[i].end());
    }
    Bytes out;
    if (body.size() <= 254u && items.size() <= 255u) {
        out.push_back(0xC0u);
        out.push_back(static_cast<std::uint8_t>(body.size() + 1));
        out.push_back(static_cast<std::uint8_t>(items.size()));
    } else {
        out.push_back(0xD0u);
        putBigEndian(out, static_cast<std::uint64_t>(body.size() + 4), 4);
        putBigEndian(out, static_cast<std::uint64_t>(items.size()), 4);
    }
    out.insert(out.end(), body.begin(), body.end());
    return out;
}

// §1.6.23 — map32.
//
// The argument is the keys and values already flattened, alternating,
// which is the form all three runtimes take: a `std::pair` has no Java
// or TypeScript counterpart that survives the same generated walk, and
// the count on the wire is the flattened length either way.
inline Bytes encodeMap(const std::vector<Bytes>& flattened) {
    Bytes body;
    for (std::size_t i = 0; i < flattened.size(); ++i) {
        body.insert(body.end(), flattened[i].begin(), flattened[i].end());
    }
    Bytes out;
    if (body.size() <= 254u && flattened.size() <= 255u) {
        out.push_back(0xC1u);
        out.push_back(static_cast<std::uint8_t>(body.size() + 1));
        out.push_back(static_cast<std::uint8_t>(flattened.size()));
    } else {
        out.push_back(0xD1u);
        putBigEndian(out, static_cast<std::uint64_t>(body.size() + 4), 4);
        putBigEndian(out, static_cast<std::uint64_t>(flattened.size()), 4);
    }
    out.insert(out.end(), body.begin(), body.end());
    return out;
}

// §7.2.1.1 — the truncated descriptor form: `0x00`, a `ulong`, the body.
inline Bytes describedUlong(std::uint64_t descriptor, const Bytes& body) {
    Bytes out;
    out.push_back(0x00u);
    Bytes code = encodeUlong(descriptor);
    out.insert(out.end(), code.begin(), code.end());
    out.insert(out.end(), body.begin(), body.end());
    return out;
}

// §7.2.1.2 — the full-identifier form: a `symbol` and the body, which is
// AMQP §1.3.4's two-element described type.
inline Bytes describedSymbol(const std::string& symbol, const Bytes& body) {
    std::vector<Bytes> items;
    items.push_back(encodeSymbol(symbol));
    items.push_back(body);
    return encodeList(items);
}

// §8.1.2 — the symbolic name of an enumerator.
inline std::string enumName(std::int64_t ordinal, const std::vector<std::string>& literals) {
    if (ordinal < 0 || static_cast<std::size_t>(ordinal) >= literals.size()) {
        throw std::out_of_range("no enumerator has that ordinal");
    }
    return literals[static_cast<std::size_t>(ordinal)];
}

// -- §8.1.2 JSON -----------------------------------------------------

// RFC 8259 §7.
inline std::string jsonStr(const std::string& value) {
    std::string out;
    out.push_back('"');
    for (std::size_t i = 0; i < value.size(); ++i) {
        unsigned char c = static_cast<unsigned char>(value[i]);
        switch (c) {
            case '"': out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n"; break;
            case '\r': out += "\\r"; break;
            case '\t': out += "\\t"; break;
            case 0x08: out += "\\b"; break;
            case 0x0C: out += "\\f"; break;
            default:
                if (c < 0x20) {
                    char buffer[8];
                    std::snprintf(buffer, sizeof(buffer), "\\u%04x", c);
                    out += buffer;
                } else {
                    out.push_back(static_cast<char>(c));
                }
        }
    }
    out.push_back('"');
    return out;
}

inline std::string jsonBool(bool value) { return value ? "true" : "false"; }
inline std::string jsonI64(std::int64_t value) { return std::to_string(value); }
inline std::string jsonU64(std::uint64_t value) { return std::to_string(value); }

// §8.1.2 — "Floating-point NaN and infinities are not representable in
// JSON and SHALL cause the encoder to raise an error."
inline std::string jsonDouble(double value) {
    if (!std::isfinite(value)) {
        throw std::domain_error("DDS-AMQP 1.0 8.1.2: NaN and infinities are not JSON");
    }
    // The shortest representation that round-trips, which is what
    // Rust's `{}`, Java's `Double.toString` and JavaScript's `String`
    // all write. `%.17g` would write `1.5000000000000000` for a value
    // the other three write as `1.5`; RFC 8259 admits both and the four
    // columns are compared against each other.
    char buffer[64];
    std::to_chars_result result = std::to_chars(buffer, buffer + sizeof(buffer), value);
    std::string out(buffer, result.ptr);
    if (out.find('.') == std::string::npos && out.find('e') == std::string::npos &&
        out.find('E') == std::string::npos) {
        out += ".0";
    }
    return out;
}

inline std::string jsonChar(std::uint32_t codepoint) {
    std::string utf8;
    if (codepoint < 0x80u) {
        utf8.push_back(static_cast<char>(codepoint));
    } else if (codepoint < 0x800u) {
        utf8.push_back(static_cast<char>(0xC0u | (codepoint >> 6)));
        utf8.push_back(static_cast<char>(0x80u | (codepoint & 0x3Fu)));
    } else if (codepoint < 0x10000u) {
        utf8.push_back(static_cast<char>(0xE0u | (codepoint >> 12)));
        utf8.push_back(static_cast<char>(0x80u | ((codepoint >> 6) & 0x3Fu)));
        utf8.push_back(static_cast<char>(0x80u | (codepoint & 0x3Fu)));
    } else {
        utf8.push_back(static_cast<char>(0xF0u | (codepoint >> 18)));
        utf8.push_back(static_cast<char>(0x80u | ((codepoint >> 12) & 0x3Fu)));
        utf8.push_back(static_cast<char>(0x80u | ((codepoint >> 6) & 0x3Fu)));
        utf8.push_back(static_cast<char>(0x80u | (codepoint & 0x3Fu)));
    }
    return jsonStr(utf8);
}

// §8.1.2 — "octet sequences: Base64 (RFC 4648) string."
inline std::string jsonBase64(const Bytes& bytes) {
    static const char* alphabet =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    std::string out;
    out.push_back('"');
    for (std::size_t i = 0; i < bytes.size(); i += 3) {
        std::size_t remaining = bytes.size() - i;
        std::uint32_t triple = static_cast<std::uint32_t>(bytes[i]) << 16;
        if (remaining > 1) triple |= static_cast<std::uint32_t>(bytes[i + 1]) << 8;
        if (remaining > 2) triple |= static_cast<std::uint32_t>(bytes[i + 2]);
        std::size_t chunk = remaining < 3 ? remaining : 3;
        for (std::size_t position = 0; position < 4; ++position) {
            if (position > chunk) {
                out.push_back('=');
            } else {
                out.push_back(alphabet[(triple >> (18 - 6 * position)) & 0x3Fu]);
            }
        }
    }
    out.push_back('"');
    return out;
}

// §8.1.2 — "Time_t: JSON object {"sec": n, "nanosec": m}."
inline std::string jsonTime(std::int64_t sec, std::uint32_t nanosec) {
    return "{\"sec\":" + std::to_string(sec) + ",\"nanosec\":" + std::to_string(nanosec) + "}";
}

// §7.1.3 — the standard Time_t mapping, in milliseconds.
inline std::int64_t timeMillis(std::int64_t sec, std::uint32_t nanosec) {
    return sec * 1000 + static_cast<std::int64_t>(nanosec / 1000000u);
}

}  // namespace amqp
}  // namespace zerodds

#endif  // ZERODDS_AMQP_CODEC_HPP
