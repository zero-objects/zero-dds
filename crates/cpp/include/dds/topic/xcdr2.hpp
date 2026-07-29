// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/xcdr2.hpp -- XCDR2 (XTypes 1.3 §7.4) wire-format helpers.
//
// Header-only, C++17, no external dependency. Implements:
//
//   * Plain-CDR2 primitive encoding (LE and BE) with alignment §7.4.1.5.
//   * DHEADER (§7.4.4.4) for DELIMITED_CDR2 (Appendable types).
//   * EMHEADER + LC encoding (§7.4.2.2) for PL_CDR2 (Mutable types).
//   * String encoding with NUL terminator (§7.4.4.6).
//   * Sequence length prefix (§7.4.4.7).
//
// Used by the `idl-cpp`-emitted `topic_type_support<T>` specializations.
// Conformance: docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md.

#ifndef ZERODDS_DDS_TOPIC_XCDR2_HPP
#define ZERODDS_DDS_TOPIC_XCDR2_HPP

#include <cstdint>
#include <cstring>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <vector>

namespace dds {
namespace topic {

// Minimal definition for the extensibility tag (XTypes §7.2.2.4.4).
// If the full DDS-PSM-Cxx policy hierarchy is not present, this local
// definition provides the compile-time tag for
// `topic_type_support<T>::extensibility()`.
namespace core {
namespace policy {

enum class DataRepresentationKind {
    FINAL,
    APPENDABLE,
    MUTABLE,
};

} // namespace policy
} // namespace core

namespace xcdr2 {

// ---------------------------------------------------------------------------
// Endian detection -- all mainstream targets are little-endian; but we
// need BE write variants for key hashing and encode_be().
// ---------------------------------------------------------------------------

inline bool is_host_le() {
    uint16_t v = 0x0102;
    uint8_t b[2];
    std::memcpy(b, &v, 2);
    return b[0] == 0x02;
}

// ---------------------------------------------------------------------------
// Buffer-underrun check
// ---------------------------------------------------------------------------

inline void check_avail(size_t pos, size_t need, size_t len) {
    if (pos + need > len) {
        throw std::out_of_range("xcdr2: buffer underrun");
    }
}

// ---------------------------------------------------------------------------
// Padding -- alignment relative to buffer start (§7.4.1.5).
// ---------------------------------------------------------------------------

inline size_t align_up(size_t pos, size_t a) {
    return (pos + (a - 1)) & ~(a - 1);
}

// ---------------------------------------------------------------------------
// XCDR version -- determines the alignment rule on decode (XTypes 1.3
// §7.4.3.4.2). XCDR1/PLAIN_CDR aligns primitives to their natural
// size; XCDR2/PLAIN_CDR2 caps the alignment at 4 (8-byte types
// land on 4-byte boundaries). The version is in the 4-byte
// encapsulation header of every serialized payload (RTPS 2.5 §10.5):
// CDR_LE = 0x0001 -> Xcdr1, CDR2_LE/PLAIN_CDR2_LE = 0x0007 -> Xcdr2.
enum class XcdrVersion {
    Xcdr1,
    Xcdr2,
};

/// Maximum alignment for an XCDR version. XCDR2 caps at 4.
inline size_t xcdr_max_align(XcdrVersion v) {
    return v == XcdrVersion::Xcdr2 ? size_t{4} : size_t{8};
}

/// Effective alignment: natural size, capped at `max_align`.
inline size_t capped_align(size_t natural, size_t max_align) {
    return natural < max_align ? natural : max_align;
}

inline void pad_to(std::vector<uint8_t>& out, size_t a) {
    while ((out.size() % a) != 0) {
        out.push_back(0);
    }
}

inline void pad_to_from_origin(std::vector<uint8_t>& out, size_t origin, size_t a) {
    while (((out.size() - origin) % a) != 0) {
        out.push_back(0);
    }
}

inline void skip_pad(size_t& pos, size_t a) {
    pos = (pos + (a - 1)) & ~(a - 1);
}

inline void skip_pad_from_origin(size_t& pos, size_t origin, size_t a) {
    size_t off = pos - origin;
    size_t aligned = (off + (a - 1)) & ~(a - 1);
    pos = origin + aligned;
}

// ---------------------------------------------------------------------------
// Primitive-Writes (LE)
// ---------------------------------------------------------------------------

inline void append_bytes(std::vector<uint8_t>& out, const void* src, size_t n) {
    const auto* p = static_cast<const uint8_t*>(src);
    out.insert(out.end(), p, p + n);
}

inline void write_u8(std::vector<uint8_t>& out, uint8_t v) {
    out.push_back(v);
}

inline void write_bool(std::vector<uint8_t>& out, bool v) {
    out.push_back(v ? uint8_t{1} : uint8_t{0});
}

template <typename T>
inline void write_le_raw(std::vector<uint8_t>& out, T v) {
    static_assert(std::is_trivially_copyable<T>::value, "write_le_raw requires trivially-copyable T");
    uint8_t buf[sizeof(T)];
    std::memcpy(buf, &v, sizeof(T));
    if (!is_host_le()) {
        for (size_t i = 0; i < sizeof(T) / 2; ++i) {
            uint8_t tmp = buf[i];
            buf[i] = buf[sizeof(T) - 1 - i];
            buf[sizeof(T) - 1 - i] = tmp;
        }
    }
    out.insert(out.end(), buf, buf + sizeof(T));
}

template <typename T>
inline void write_be_raw(std::vector<uint8_t>& out, T v) {
    static_assert(std::is_trivially_copyable<T>::value, "write_be_raw requires trivially-copyable T");
    uint8_t buf[sizeof(T)];
    std::memcpy(buf, &v, sizeof(T));
    if (is_host_le()) {
        for (size_t i = 0; i < sizeof(T) / 2; ++i) {
            uint8_t tmp = buf[i];
            buf[i] = buf[sizeof(T) - 1 - i];
            buf[sizeof(T) - 1 - i] = tmp;
        }
    }
    out.insert(out.end(), buf, buf + sizeof(T));
}

// LE writer with alignment (relative to buffer-start).
template <typename T>
inline void write_le(std::vector<uint8_t>& out, T v) {
    pad_to(out, sizeof(T));
    write_le_raw<T>(out, v);
}

template <typename T>
inline void write_le_origin(std::vector<uint8_t>& out, size_t origin, T v) {
    pad_to_from_origin(out, origin, sizeof(T));
    write_le_raw<T>(out, v);
}

// Representation-aware writer: `max_align` caps the alignment
// (XCDR2 -> 4, XCDR1 -> 8). Symmetric to `read_le_origin(.., max_align)`.
template <typename T>
inline void write_le_origin(std::vector<uint8_t>& out, size_t origin, T v, size_t max_align) {
    pad_to_from_origin(out, origin, capped_align(sizeof(T), max_align));
    write_le_raw<T>(out, v);
}

template <typename T>
inline void write_be(std::vector<uint8_t>& out, T v) {
    pad_to(out, sizeof(T));
    write_be_raw<T>(out, v);
}

template <typename T>
inline void write_be_origin(std::vector<uint8_t>& out, size_t origin, T v) {
    pad_to_from_origin(out, origin, sizeof(T));
    write_be_raw<T>(out, v);
}

// Representation-aware big-endian writer: `max_align` caps the alignment
// (XCDR2 -> 4, XCDR1 -> 8), symmetric to `write_le_origin(.., max_align)`. A
// BE stream MUST apply the same alignment cap as LE — otherwise an 8-byte
// primitive over-aligns to 8 and diverges from the spec / a BE peer.
template <typename T>
inline void write_be_origin(std::vector<uint8_t>& out, size_t origin, T v, size_t max_align) {
    pad_to_from_origin(out, origin, capped_align(sizeof(T), max_align));
    write_be_raw<T>(out, v);
}

// ---------------------------------------------------------------------------
// Primitive-Reads (LE)
// ---------------------------------------------------------------------------

// Reads a primitive whose wire bytes are in `big_endian ? BE : LE` order. The
// `big_endian` flag defaults to false, so every existing little-endian caller
// is unchanged; a big-endian stream threads `true` through. We swap iff the
// stream's byte order differs from the host's: `big_endian == is_host_le()`.
template <typename T>
inline T read_le_raw(const uint8_t* buf, size_t& pos, size_t len, bool big_endian = false) {
    static_assert(std::is_trivially_copyable<T>::value, "read_le_raw requires trivially-copyable T");
    check_avail(pos, sizeof(T), len);
    uint8_t tmp[sizeof(T)];
    std::memcpy(tmp, buf + pos, sizeof(T));
    pos += sizeof(T);
    if (big_endian == is_host_le()) {
        for (size_t i = 0; i < sizeof(T) / 2; ++i) {
            uint8_t t = tmp[i];
            tmp[i] = tmp[sizeof(T) - 1 - i];
            tmp[sizeof(T) - 1 - i] = t;
        }
    }
    T v;
    std::memcpy(&v, tmp, sizeof(T));
    return v;
}

template <typename T>
inline T read_le(const uint8_t* buf, size_t& pos, size_t len, bool big_endian = false) {
    skip_pad(pos, sizeof(T));
    return read_le_raw<T>(buf, pos, len, big_endian);
}

// Representation-aware: `max_align` caps the alignment (XCDR2 → 4,
// XCDR1 → 8). See [`xcdr_max_align`]. `big_endian` selects the wire byte order.
template <typename T>
inline T read_le_origin(
    const uint8_t* buf, size_t& pos, size_t len, size_t origin, size_t max_align,
    bool big_endian = false) {
    skip_pad_from_origin(pos, origin, capped_align(sizeof(T), max_align));
    return read_le_raw<T>(buf, pos, len, big_endian);
}

inline bool read_bool(const uint8_t* buf, size_t& pos, size_t len) {
    check_avail(pos, 1, len);
    bool v = buf[pos] != 0;
    pos += 1;
    return v;
}

inline uint8_t read_u8(const uint8_t* buf, size_t& pos, size_t len) {
    check_avail(pos, 1, len);
    uint8_t v = buf[pos];
    pos += 1;
    return v;
}

// ---------------------------------------------------------------------------
// String -- §7.4.4.6: uint32 length (incl. NUL) + UTF-8 + NUL.
// ---------------------------------------------------------------------------

inline void write_string(std::vector<uint8_t>& out, const std::string& s) {
    uint32_t n = static_cast<uint32_t>(s.size() + 1);
    write_le<uint32_t>(out, n);
    append_bytes(out, s.data(), s.size());
    out.push_back(0);
}

inline void write_string_origin(std::vector<uint8_t>& out, size_t origin, const std::string& s) {
    uint32_t n = static_cast<uint32_t>(s.size() + 1);
    write_le_origin<uint32_t>(out, origin, n);
    append_bytes(out, s.data(), s.size());
    out.push_back(0);
}

// Representation-aware (XCDR2/XCDR1) string writer.
inline void write_string_origin(
    std::vector<uint8_t>& out, size_t origin, const std::string& s, size_t max_align) {
    uint32_t n = static_cast<uint32_t>(s.size() + 1);
    write_le_origin<uint32_t>(out, origin, n, max_align);
    append_bytes(out, s.data(), s.size());
    out.push_back(0);
}

inline void write_string_be(std::vector<uint8_t>& out, const std::string& s) {
    uint32_t n = static_cast<uint32_t>(s.size() + 1);
    write_be<uint32_t>(out, n);
    append_bytes(out, s.data(), s.size());
    out.push_back(0);
}

inline std::string read_string(const uint8_t* buf, size_t& pos, size_t len) {
    auto n = read_le<uint32_t>(buf, pos, len);
    if (n == 0) {
        throw std::out_of_range("xcdr2: zero-length string (must include NUL)");
    }
    check_avail(pos, n, len);
    // n includes NUL; payload = n - 1 bytes; verify last byte is NUL.
    std::string s(reinterpret_cast<const char*>(buf + pos), n - 1);
    pos += n;
    return s;
}

// ---------------------------------------------------------------------------
// wstring -- conformance §9.1 + GIOP 1.2 §15.3.2.7: uint32 length **in octets**
// (= (units + 1) * 2, the +1 is the leading BOM), then a byte-order-mark
// (0xFEFF in message byte order: LE -> FF FE, BE -> FE FF) and the UTF-16 code
// units, NO NUL terminator. Empty wstring = length 0, no BOM. Byte-identical to
// the Rust `WString` encoder (`crates/cdr/src/composite.rs`) so cross-language
// types interop. Bindings MUST emit UTF-16, not UTF-8 (conformance §9.1).
// ---------------------------------------------------------------------------

// Portable wchar_t -> UTF-16 code units. Where wchar_t is already 2 bytes
// (Windows) the units are taken verbatim; where it is 4 bytes (Linux/macOS,
// i.e. UTF-32 code points) each code point is re-encoded as UTF-16 (a surrogate
// pair for the supplementary planes), matching Rust `str::encode_utf16`.
inline std::vector<uint16_t> wstring_to_utf16(const std::wstring& s) {
    std::vector<uint16_t> units;
    units.reserve(s.size());
    if constexpr (sizeof(wchar_t) == 2) {
        for (wchar_t c : s) {
            units.push_back(static_cast<uint16_t>(c));
        }
    } else {
        for (wchar_t c : s) {
            uint32_t cp = static_cast<uint32_t>(c);
            if (cp <= 0xFFFFu) {
                units.push_back(static_cast<uint16_t>(cp));
            } else {
                cp -= 0x10000u;
                units.push_back(static_cast<uint16_t>(0xD800u + (cp >> 10)));
                units.push_back(static_cast<uint16_t>(0xDC00u + (cp & 0x3FFu)));
            }
        }
    }
    return units;
}

// XTypes 1.3 wstring wire form (matches the cross-vendor cdr-core reference,
// `crates/cdr` generated WString encode for XTypes types): uint32 byte-length =
// (#UTF-16 code units * 2), then the raw UTF-16 code units in message byte
// order — NO byte-order mark, NO terminator. (This differs from the CORBA-GIOP
// `WString` composite form which prepends a BOM; the XTypes/DDS golden does
// not.) An empty wstring encodes as length 0.
inline void write_wstring_origin(
    std::vector<uint8_t>& out, size_t origin, const std::wstring& s, size_t max_align) {
    auto units = wstring_to_utf16(s);
    uint32_t octets = static_cast<uint32_t>(units.size() * 2);
    write_le_origin<uint32_t>(out, origin, octets, max_align);
    for (uint16_t u : units) {
        write_le<uint16_t>(out, u);
    }
}

inline void write_wstring_be(std::vector<uint8_t>& out, const std::wstring& s) {
    auto units = wstring_to_utf16(s);
    uint32_t octets = static_cast<uint32_t>(units.size() * 2);
    write_be<uint32_t>(out, octets);
    for (uint16_t u : units) {
        write_be<uint16_t>(out, u);
    }
}

inline std::wstring read_wstring_origin(
    const uint8_t* buf, size_t& pos, size_t len, size_t origin, size_t max_align,
    bool stream_be = false) {
    auto octets = read_le_origin<uint32_t>(buf, pos, len, origin, max_align, stream_be);
    if (octets == 0) {
        return std::wstring();
    }
    if ((octets % 2) != 0) {
        throw std::out_of_range("xcdr2: wstring octet length must be even");
    }
    check_avail(pos, octets, len);
    // Unit byte order: an explicit BOM wins (BE 0xFEFF / LE 0xFFFE) for inbound
    // CORBA-GIOP-style wstrings; otherwise the XTypes/DDS form carries the raw
    // units in the **message byte order** (`stream_be`), mirroring the encoder.
    size_t start = 0;
    bool big_endian = stream_be;
    if (octets >= 2) {
        uint8_t b0 = buf[pos], b1 = buf[pos + 1];
        if (b0 == 0xFE && b1 == 0xFF) {
            start = 2;
            big_endian = true;
        } else if (b0 == 0xFF && b1 == 0xFE) {
            start = 2;
            big_endian = false;
        }
    }
    // Collect the raw UTF-16 code units (message byte order).
    std::vector<uint16_t> units;
    units.reserve(octets / 2);
    for (size_t i = start; i + 1 < octets; i += 2) {
        uint16_t hi = buf[pos + i], lo = buf[pos + i + 1];
        uint16_t unit = big_endian ? static_cast<uint16_t>((hi << 8) | lo)
                                   : static_cast<uint16_t>((lo << 8) | hi);
        units.push_back(unit);
    }
    std::wstring s;
    if constexpr (sizeof(wchar_t) == 2) {
        // wchar_t already a UTF-16 unit (Windows): copy verbatim.
        for (uint16_t u : units) {
            s.push_back(static_cast<wchar_t>(u));
        }
    } else {
        // 4-byte wchar_t (Linux/macOS = UTF-32 code points): recombine UTF-16
        // surrogate pairs into one code point, symmetric to `wstring_to_utf16`
        // (matches the Rust `String::from_utf16` reference reader, so 🎉 D83C
        // DF89 -> U+1F389 round-trips as ONE wchar_t).
        for (size_t i = 0; i < units.size(); ++i) {
            uint16_t u = units[i];
            if (u >= 0xD800u && u <= 0xDBFFu && i + 1 < units.size()
                && units[i + 1] >= 0xDC00u && units[i + 1] <= 0xDFFFu) {
                uint32_t cp = 0x10000u
                    + ((static_cast<uint32_t>(u - 0xD800u) << 10)
                       | static_cast<uint32_t>(units[i + 1] - 0xDC00u));
                s.push_back(static_cast<wchar_t>(cp));
                ++i;
            } else {
                s.push_back(static_cast<wchar_t>(u));
            }
        }
    }
    pos += octets;
    return s;
}

inline std::string read_string_origin(
    const uint8_t* buf, size_t& pos, size_t len, size_t origin, size_t max_align,
    bool big_endian = false) {
    auto n = read_le_origin<uint32_t>(buf, pos, len, origin, max_align, big_endian);
    if (n == 0) {
        throw std::out_of_range("xcdr2: zero-length string (must include NUL)");
    }
    check_avail(pos, n, len);
    std::string s(reinterpret_cast<const char*>(buf + pos), n - 1);
    pos += n;
    return s;
}

// ---------------------------------------------------------------------------
// DHEADER -- §7.4.4.4 (DELIMITED_CDR2).
// ---------------------------------------------------------------------------

/// Reserves 4 bytes at the current position for the DHEADER size.
/// Returns the offset where the size has been written.
inline size_t dheader_begin(std::vector<uint8_t>& out) {
    size_t off = out.size();
    write_le_raw<uint32_t>(out, 0u);
    return off;
}

/// Patches the DHEADER size = (current_size - off - 4). The size word is a
/// uint32 in the ambient stream byte order (XTypes 1.3 §7.4.3.4): `big_endian`
/// selects BE so a BE stream's DHEADER matches the spec / a BE peer's reader.
inline void dheader_end(std::vector<uint8_t>& out, size_t off, bool big_endian = false) {
    uint32_t size = static_cast<uint32_t>(out.size() - off - 4);
    uint8_t buf[4];
    std::memcpy(buf, &size, 4);
    // Swap to the target order when it differs from the host order.
    if (big_endian == is_host_le()) {
        uint8_t t = buf[0]; buf[0] = buf[3]; buf[3] = t;
        t = buf[1]; buf[1] = buf[2]; buf[2] = t;
    }
    std::memcpy(out.data() + off, buf, 4);
}

inline uint32_t dheader_read(const uint8_t* buf, size_t& pos, size_t len, bool big_endian = false) {
    return read_le<uint32_t>(buf, pos, len, big_endian);
}

// ---------------------------------------------------------------------------
// Representation-aware collection DHEADER. A delimited collection (array of
// non-primitives / sequence / map) carries a 4-byte DHEADER under XCDR2
// (XTypes 1.3 §7.4.3.5) but NOTHING under XCDR1 (classic CDR has no delimiters).
// These wrappers no-op under XCDR1 so the same emit covers both reprs. The
// DHEADER value is never used to BOUND such a collection here (its element
// count / fixed length drives the loop), so returning 0 under XCDR1 is safe.
// ---------------------------------------------------------------------------

constexpr size_t DHEADER_NONE = static_cast<size_t>(-1);

inline size_t dheader_begin_r(std::vector<uint8_t>& out, XcdrVersion repr) {
    if (repr == XcdrVersion::Xcdr1) return DHEADER_NONE;
    return dheader_begin(out);
}

inline void dheader_end_r(std::vector<uint8_t>& out, size_t off, bool big_endian, XcdrVersion repr) {
    if (repr == XcdrVersion::Xcdr1 || off == DHEADER_NONE) return;
    dheader_end(out, off, big_endian);
}

inline uint32_t dheader_read_r(const uint8_t* buf, size_t& pos, size_t len, bool big_endian, XcdrVersion repr) {
    if (repr == XcdrVersion::Xcdr1) return 0;
    return dheader_read(buf, pos, len, big_endian);
}

// ---------------------------------------------------------------------------
// EMHEADER -- XTypes 1.3 §7.4.3.4.2 (PL_CDR2 Length-Code).
//
// 32-bit EMHEADER1 in MSB-first bit-order:
//   * bit 31    = MU (must_understand).
//   * bits 30..28 = LC (length-code, 3 bits).
//   * bits 27..0  = member_id.
//
// Wire-byte-order: per OMG XTypes 1.3 §7.4.3.4.5 the EMHEADER1 word is
// encoded in the **ambient stream endian**. In the LE-default stream this
// means the LE byte-form of the packed u32. The default helpers below
// emit / read LE bytes; callers using BE streams must use the
// `_be` variants.
//
// LC encoding (OMG XTypes 1.3 §7.4.3.4.2 — these are the wire-normative
// values, identical to the Rust `LengthCode` enum in `zerodds-cdr`):
//   * LC=0 -> 1-byte body, no NEXTINT.
//   * LC=1 -> 2-byte body, no NEXTINT.
//   * LC=2 -> 4-byte body, no NEXTINT (long, float).
//   * LC=3 -> 8-byte body, no NEXTINT (long long, double).
//   * LC=4 -> NEXTINT uint32 = body length in bytes (string, sequence,
//             nested struct — the variable-length case).
//   * LC=5 -> NEXTINT = body length incl. the nested DHEADER.
//   * LC=6 -> NEXTINT = element count; body = 4 + 4*count (4-byte-prim array).
//   * LC=7 -> NEXTINT = element count; body = 4 + 8*count (8-byte-prim array).
//
// IMPORTANT: a variable-length member MUST use LC=4 (NOT LC=3). LC=3 tells a
// spec-compliant reader "read exactly 8 bytes, no NEXTINT"; emitting LC=3 for a
// string/sequence desyncs Rust/Cyclone/FastDDS readers. (This was a prior bug;
// `emheader_nextint_begin` now emits LC=4.) For optional-`Some` presence we just
// emit the EMHEADER; absent optionals are omitted.
// ---------------------------------------------------------------------------

constexpr uint32_t EMHEADER_MU_FLAG_BIT = 1u << 31;

inline uint32_t emheader_make(uint32_t lc, uint32_t member_id, bool must_understand) {
    uint32_t e = (lc & 0x7u) << 28;
    e |= (member_id & 0x0FFFFFFFu);
    if (must_understand) e |= EMHEADER_MU_FLAG_BIT;
    return e;
}

inline void emheader_write(std::vector<uint8_t>& out, uint32_t value, bool big_endian = false) {
    // EMHEADER1 is a u32 in the ambient stream byte order (XTypes §7.4.3.4.5):
    // little-endian for the default wire, big-endian for a BE stream.
    if (big_endian) {
        out.push_back(static_cast<uint8_t>((value >> 24) & 0xff));
        out.push_back(static_cast<uint8_t>((value >> 16) & 0xff));
        out.push_back(static_cast<uint8_t>((value >> 8) & 0xff));
        out.push_back(static_cast<uint8_t>(value & 0xff));
    } else {
        out.push_back(static_cast<uint8_t>(value & 0xff));
        out.push_back(static_cast<uint8_t>((value >> 8) & 0xff));
        out.push_back(static_cast<uint8_t>((value >> 16) & 0xff));
        out.push_back(static_cast<uint8_t>((value >> 24) & 0xff));
    }
}

inline uint32_t emheader_read_raw(const uint8_t* buf, size_t& pos, size_t len,
                                  bool big_endian = false) {
    // EMHEADER1 is a u32 in the ambient stream byte order (XTypes §7.4.3.4.5).
    return read_le_raw<uint32_t>(buf, pos, len, big_endian);
}

/// Begin a Mutable-body. Writes the outer DHEADER. Returns offset of the
/// DHEADER-size-field (for `dheader_end`) AND the origin offset (= start
/// of body, i.e. dheader-offset + 4) for later padding-from-origin.
struct MutableScope {
    size_t dheader_off;
    size_t origin;
};

inline MutableScope mutable_begin(std::vector<uint8_t>& out) {
    MutableScope s;
    s.dheader_off = dheader_begin(out);
    s.origin = out.size();
    return s;
}

inline void mutable_end(std::vector<uint8_t>& out, const MutableScope& s, bool big_endian = false) {
    dheader_end(out, s.dheader_off, big_endian);
}

/// Plain-1-byte member (LC=0). The 1-byte body needs no swap; only the
/// EMHEADER1 word follows the ambient stream order.
inline void emheader_u8(std::vector<uint8_t>& out, size_t origin, uint32_t id, bool must_understand, uint8_t v, bool big_endian = false) {
    pad_to_from_origin(out, origin, 4);
    emheader_write(out, emheader_make(0, id, must_understand), big_endian);
    out.push_back(v);
}

/// Plain-2-byte member (LC=1).
template <typename T>
inline void emheader_2(std::vector<uint8_t>& out, size_t origin, uint32_t id, bool must_understand, T v) {
    static_assert(sizeof(T) == 2, "emheader_2 needs 2-byte type");
    pad_to_from_origin(out, origin, 4);
    emheader_write(out, emheader_make(1, id, must_understand));
    write_le_raw<T>(out, v);
}

/// Plain-4-byte member (LC=2).
template <typename T>
inline void emheader_4(std::vector<uint8_t>& out, size_t origin, uint32_t id, bool must_understand, T v) {
    static_assert(sizeof(T) == 4, "emheader_4 needs 4-byte type");
    pad_to_from_origin(out, origin, 4);
    emheader_write(out, emheader_make(2, id, must_understand));
    write_le_raw<T>(out, v);
}

/// Plain-8-byte member (LC=3 = 8-byte body, no NEXTINT; XTypes §7.4.3.4.2).
template <typename T>
inline void emheader_8(std::vector<uint8_t>& out, size_t origin, uint32_t id, bool must_understand, T v) {
    static_assert(sizeof(T) == 8, "emheader_8 needs 8-byte type");
    pad_to_from_origin(out, origin, 4);
    emheader_write(out, emheader_make(3, id, must_understand));
    write_le_raw<T>(out, v);
}

/// Variable-length member with NEXTINT (LC=4, uint32 NEXTINT = body length in
/// bytes; XTypes 1.3 §7.4.3.4.2). LC=3 means "8-byte body, NO NEXTINT" — using
/// it for a variable member would desync any spec-compliant (Rust/Cyclone)
/// reader, which reads exactly 8 bytes and no NEXTINT. Caller writes the body
/// between begin/end; helper patches NEXTINT.
struct EmheaderNextintScope {
    size_t nextint_off;
    size_t body_start;
};

inline EmheaderNextintScope emheader_nextint_begin(std::vector<uint8_t>& out, size_t origin, uint32_t id, bool must_understand, bool big_endian = false) {
    pad_to_from_origin(out, origin, 4);
    emheader_write(out, emheader_make(4, id, must_understand), big_endian);
    EmheaderNextintScope s;
    s.nextint_off = out.size();
    // Placeholder NEXTINT; emheader_nextint_end patches it in ambient order.
    write_le_raw<uint32_t>(out, 0u);
    s.body_start = out.size();
    return s;
}

inline void emheader_nextint_end(std::vector<uint8_t>& out, const EmheaderNextintScope& s, bool big_endian = false) {
    // NEXTINT is a uint32 in the ambient stream byte order (XTypes §7.4.3.4.5).
    uint32_t size = static_cast<uint32_t>(out.size() - s.body_start);
    uint8_t buf[4];
    std::memcpy(buf, &size, 4);
    if (big_endian == is_host_le()) {
        uint8_t t = buf[0]; buf[0] = buf[3]; buf[3] = t;
        t = buf[1]; buf[1] = buf[2]; buf[2] = t;
    }
    std::memcpy(out.data() + s.nextint_off, buf, 4);
}

/// Reads an EMHEADER from the stream. Decodes (member_id, lc, must_understand).
struct EmheaderRead {
    uint32_t member_id;
    uint32_t lc;
    bool must_understand;
    uint32_t raw;
};

inline EmheaderRead emheader_read(const uint8_t* buf, size_t& pos, size_t len, size_t origin,
                                  bool big_endian = false) {
    skip_pad_from_origin(pos, origin, 4);
    uint32_t e = emheader_read_raw(buf, pos, len, big_endian);
    EmheaderRead r;
    r.raw = e;
    r.lc = (e >> 28) & 0x7u;
    r.member_id = e & 0x0FFFFFFFu;
    r.must_understand = (e & EMHEADER_MU_FLAG_BIT) != 0;
    return r;
}

inline uint32_t emheader_nextint_read(const uint8_t* buf, size_t& pos, size_t len,
                                      bool big_endian = false) {
    return read_le_raw<uint32_t>(buf, pos, len, big_endian);
}

// ---------------------------------------------------------------------------
// PL_CDR1 -- Plain CDR Version 1 parameter list for `@mutable` structs under
// XCDR1 (classic CDR). OMG XTypes 1.3 §7.4.1.2 / §7.4.2. This is the XCDR1
// counterpart to the XCDR2 EMHEADER/PL_CDR2 framing above; both encode a
// `@mutable` struct, but the wire forms differ:
//   * PL_CDR2 (XCDR2): outer DHEADER, then a 32-bit EMHEADER (LC) per member.
//   * PL_CDR1 (XCDR1): NO outer DHEADER; each member is a parameter with a
//     16-bit PID + 16-bit (UNPADDED) length, body padded to 4; the list ends
//     with the PID_LIST_END sentinel.
//
// Member header forms (stream byte order, LE default):
//   * Standard: `[id u16][len u16][body, pad to 4]` -- when id < 0x3F00 and
//     len <= 0xFFFF.
//   * Extended: `[0x3F01][8][id u32][len u32][body, pad to 4]` -- mandatory
//     when id >= 0x3F00 OR len > 0xFFFF.
//   * Sentinel: `[0x3F02][0]` (PID_LIST_END).
// The length is the UNPADDED value length; trailing pad bytes follow but are
// not counted. Byte-identical to the Rust `zerodds_cdr::xcdr1` reference and
// the FastDDS / FastCDR PL_CDR1 wire form.
// ---------------------------------------------------------------------------

constexpr uint16_t PL_CDR1_PID_EXTENDED = 0x3F01;
constexpr uint16_t PL_CDR1_PID_LIST_END = 0x3F02;
constexpr uint32_t PL_CDR1_EXTENDED_THRESHOLD = 0x3F00;

inline void pl_cdr1_put_u16(std::vector<uint8_t>& out, uint16_t v, bool big_endian) {
    if (big_endian) {
        out.push_back(static_cast<uint8_t>((v >> 8) & 0xff));
        out.push_back(static_cast<uint8_t>(v & 0xff));
    } else {
        out.push_back(static_cast<uint8_t>(v & 0xff));
        out.push_back(static_cast<uint8_t>((v >> 8) & 0xff));
    }
}

inline void pl_cdr1_put_u32(std::vector<uint8_t>& out, uint32_t v, bool big_endian) {
    if (big_endian) {
        for (int i = 3; i >= 0; --i) out.push_back(static_cast<uint8_t>((v >> (i * 8)) & 0xff));
    } else {
        for (int i = 0; i < 4; ++i) out.push_back(static_cast<uint8_t>((v >> (i * 8)) & 0xff));
    }
}

inline void pl_cdr1_patch_u16(std::vector<uint8_t>& out, size_t off, uint16_t v, bool big_endian) {
    if (big_endian) {
        out[off] = static_cast<uint8_t>((v >> 8) & 0xff);
        out[off + 1] = static_cast<uint8_t>(v & 0xff);
    } else {
        out[off] = static_cast<uint8_t>(v & 0xff);
        out[off + 1] = static_cast<uint8_t>((v >> 8) & 0xff);
    }
}

inline void pl_cdr1_patch_u32(std::vector<uint8_t>& out, size_t off, uint32_t v, bool big_endian) {
    for (int i = 0; i < 4; ++i) {
        int sh = big_endian ? (3 - i) * 8 : i * 8;
        out[off + static_cast<size_t>(i)] = static_cast<uint8_t>((v >> sh) & 0xff);
    }
}

/// PL_CDR1 member scope: tracks the placeholder length field and body start so
/// the body (encoded in place after the header) can be measured + patched.
struct PlCdr1MemberScope {
    uint32_t member_id;
    size_t len_off;     // byte offset of the length field to patch
    size_t body_start;  // byte offset where the value body begins
    bool extended;
};

/// Begins a PL_CDR1 member: writes the PID header (standard or extended, chosen
/// by `member_id`) with a placeholder length. The caller then encodes the value
/// body in place (origin = `body_start`); `pl_cdr1_member_end` patches the
/// length and appends the pad-to-4. The extended form is selected up front for
/// `member_id >= 0x3F00`; a body that grows past 0xFFFF under a standard header
/// is promoted to extended in `pl_cdr1_member_end` (no silent truncation).
inline PlCdr1MemberScope pl_cdr1_member_begin(std::vector<uint8_t>& out, uint32_t member_id, bool big_endian) {
    PlCdr1MemberScope s;
    s.member_id = member_id;
    s.extended = member_id >= PL_CDR1_EXTENDED_THRESHOLD;
    if (s.extended) {
        pl_cdr1_put_u16(out, PL_CDR1_PID_EXTENDED, big_endian);
        pl_cdr1_put_u16(out, 8, big_endian);
        pl_cdr1_put_u32(out, member_id, big_endian);
        s.len_off = out.size();
        pl_cdr1_put_u32(out, 0, big_endian);
    } else {
        pl_cdr1_put_u16(out, static_cast<uint16_t>(member_id), big_endian);
        s.len_off = out.size();
        pl_cdr1_put_u16(out, 0, big_endian);
    }
    s.body_start = out.size();
    return s;
}

inline void pl_cdr1_member_end(std::vector<uint8_t>& out, const PlCdr1MemberScope& s, bool big_endian) {
    size_t body_len = out.size() - s.body_start;
    if (!s.extended && body_len > 0xFFFF) {
        // Promote standard -> extended in place: replace the 4-byte standard
        // header `[id u16][len u16]` with the 12-byte extended header. The body
        // shifts by +8 bytes; both header sizes are multiples of 4 so body
        // alignment (relative to the parameter start) is preserved.
        size_t hdr_off = s.len_off - 2;
        out.erase(out.begin() + static_cast<std::ptrdiff_t>(hdr_off),
                  out.begin() + static_cast<std::ptrdiff_t>(hdr_off + 4));
        std::vector<uint8_t> ext;
        pl_cdr1_put_u16(ext, PL_CDR1_PID_EXTENDED, big_endian);
        pl_cdr1_put_u16(ext, 8, big_endian);
        pl_cdr1_put_u32(ext, s.member_id, big_endian);
        pl_cdr1_put_u32(ext, static_cast<uint32_t>(body_len), big_endian);
        out.insert(out.begin() + static_cast<std::ptrdiff_t>(hdr_off), ext.begin(), ext.end());
    } else if (s.extended) {
        pl_cdr1_patch_u32(out, s.len_off, static_cast<uint32_t>(body_len), big_endian);
    } else {
        pl_cdr1_patch_u16(out, s.len_off, static_cast<uint16_t>(body_len), big_endian);
    }
    size_t pad = (4 - (body_len % 4)) % 4;
    for (size_t i = 0; i < pad; ++i) out.push_back(0);
}

inline void pl_cdr1_write_sentinel(std::vector<uint8_t>& out, bool big_endian) {
    pl_cdr1_put_u16(out, PL_CDR1_PID_LIST_END, big_endian);
    pl_cdr1_put_u16(out, 0, big_endian);
}

/// Parsed PL_CDR1 member header. `is_end` is set when the sentinel is read.
struct PlCdr1Header {
    uint32_t member_id;
    size_t body_len;
    bool is_end;
};

inline PlCdr1Header pl_cdr1_read_header(const uint8_t* buf, size_t& pos, size_t len, bool big_endian) {
    PlCdr1Header h{};
    h.is_end = false;
    uint16_t pid = read_le_raw<uint16_t>(buf, pos, len, big_endian);
    uint16_t l = read_le_raw<uint16_t>(buf, pos, len, big_endian);
    if (pid == PL_CDR1_PID_LIST_END) {
        h.is_end = true;
        return h;
    }
    if (pid == PL_CDR1_PID_EXTENDED) {
        h.member_id = read_le_raw<uint32_t>(buf, pos, len, big_endian);
        h.body_len = read_le_raw<uint32_t>(buf, pos, len, big_endian);
    } else {
        h.member_id = pid;
        h.body_len = l;
    }
    return h;
}

/// Skips the trailing pad bytes after a PL_CDR1 member body (to the next 4-byte
/// boundary), tolerating truncation at end-of-buffer.
inline void pl_cdr1_skip_pad(size_t& pos, size_t len, size_t body_len) {
    size_t pad = (4 - (body_len % 4)) % 4;
    for (size_t i = 0; i < pad && pos < len; ++i) ++pos;
}

} // namespace xcdr2
} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_XCDR2_HPP
