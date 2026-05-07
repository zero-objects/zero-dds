// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/xcdr2.hpp -- XCDR2 (XTypes 1.3 §7.4) Wire-Format-Helpers.
//
// Header-only, C++17, kein external Dep. Implementiert:
//
//   * Plain-CDR2-Primitive-Encoding (LE und BE) mit Alignment §7.4.1.5.
//   * DHEADER (§7.4.4.4) fuer DELIMITED_CDR2 (Appendable-Types).
//   * EMHEADER + LC encoding (§7.4.2.2) fuer PL_CDR2 (Mutable-Types).
//   * String-Encoding mit NUL-Terminator (§7.4.4.6).
//   * Sequence-Length-Prefix (§7.4.4.7).
//
// Wird von `idl-cpp`-emittierten `topic_type_support<T>`-Spezialisierungen
// verwendet. Konformanz: docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md.

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

// Minimal-Definition fuer Extensibility-Tag (XTypes §7.2.2.4.4).
// Falls die volle DDS-PSM-Cxx-Policy-Hierarchie nicht praesent ist,
// liefert diese Lokal-Definition den Compile-Time-Tag fuer
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
// Endian-Detection -- alle Mainstream-Targets sind Little-Endian; wir
// brauchen aber BE-Schreibvarianten fuer Key-Hash und encode_be().
// ---------------------------------------------------------------------------

inline bool is_host_le() {
    uint16_t v = 0x0102;
    uint8_t b[2];
    std::memcpy(b, &v, 2);
    return b[0] == 0x02;
}

// ---------------------------------------------------------------------------
// Buffer-Underrun-Check
// ---------------------------------------------------------------------------

inline void check_avail(size_t pos, size_t need, size_t len) {
    if (pos + need > len) {
        throw std::out_of_range("xcdr2: buffer underrun");
    }
}

// ---------------------------------------------------------------------------
// Padding -- Alignment relativ zum Buffer-Start (§7.4.1.5).
// ---------------------------------------------------------------------------

inline size_t align_up(size_t pos, size_t a) {
    return (pos + (a - 1)) & ~(a - 1);
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

// ---------------------------------------------------------------------------
// Primitive-Reads (LE)
// ---------------------------------------------------------------------------

template <typename T>
inline T read_le_raw(const uint8_t* buf, size_t& pos, size_t len) {
    static_assert(std::is_trivially_copyable<T>::value, "read_le_raw requires trivially-copyable T");
    check_avail(pos, sizeof(T), len);
    uint8_t tmp[sizeof(T)];
    std::memcpy(tmp, buf + pos, sizeof(T));
    pos += sizeof(T);
    if (!is_host_le()) {
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
inline T read_le(const uint8_t* buf, size_t& pos, size_t len) {
    skip_pad(pos, sizeof(T));
    return read_le_raw<T>(buf, pos, len);
}

template <typename T>
inline T read_le_origin(const uint8_t* buf, size_t& pos, size_t len, size_t origin) {
    skip_pad_from_origin(pos, origin, sizeof(T));
    return read_le_raw<T>(buf, pos, len);
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

inline std::string read_string_origin(const uint8_t* buf, size_t& pos, size_t len, size_t origin) {
    auto n = read_le_origin<uint32_t>(buf, pos, len, origin);
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

/// Patches the DHEADER size = (current_size - off - 4).
inline void dheader_end(std::vector<uint8_t>& out, size_t off) {
    uint32_t size = static_cast<uint32_t>(out.size() - off - 4);
    uint8_t buf[4];
    std::memcpy(buf, &size, 4);
    if (!is_host_le()) {
        uint8_t t = buf[0]; buf[0] = buf[3]; buf[3] = t;
        t = buf[1]; buf[1] = buf[2]; buf[2] = t;
    }
    std::memcpy(out.data() + off, buf, 4);
}

inline uint32_t dheader_read(const uint8_t* buf, size_t& pos, size_t len) {
    return read_le<uint32_t>(buf, pos, len);
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
// LC encoding (per conformance-spec annotations on V-10):
//   * LC=0 -> 1-byte primitive, no extra length.
//   * LC=1 -> 2-byte primitive.
//   * LC=2 -> 4-byte primitive (long, float).
//   * LC=3 -> NEXTINT uint32 follows = length of remaining body bytes.
//
// Note: OMG XTypes 1.3 §7.4.3.4.2 also defines LC=4..7 with NEXTINT
// variants for large/element-counted bodies. The conformance-spec V-10
// uses LC=3 with NEXTINT, which matches OMG-LC=4 semantics; we
// follow the conformance-spec convention here (LC=3 = "8-byte primitive
// OR NEXTINT-follows depending on member type"). For optional-`Some`
// presence we just emit the EMHEADER; absent optionals are omitted.
// ---------------------------------------------------------------------------

constexpr uint32_t EMHEADER_MU_FLAG_BIT = 1u << 31;

inline uint32_t emheader_make(uint32_t lc, uint32_t member_id, bool must_understand) {
    uint32_t e = (lc & 0x7u) << 28;
    e |= (member_id & 0x0FFFFFFFu);
    if (must_understand) e |= EMHEADER_MU_FLAG_BIT;
    return e;
}

inline void emheader_write(std::vector<uint8_t>& out, uint32_t value) {
    // Ambient-LE: u32 little-endian wire bytes (XTypes §7.4.3.4.5).
    out.push_back(static_cast<uint8_t>(value & 0xff));
    out.push_back(static_cast<uint8_t>((value >> 8) & 0xff));
    out.push_back(static_cast<uint8_t>((value >> 16) & 0xff));
    out.push_back(static_cast<uint8_t>((value >> 24) & 0xff));
}

inline uint32_t emheader_read_raw(const uint8_t* buf, size_t& pos, size_t len) {
    check_avail(pos, 4, len);
    uint32_t v = static_cast<uint32_t>(buf[pos + 0])
               | (static_cast<uint32_t>(buf[pos + 1]) << 8)
               | (static_cast<uint32_t>(buf[pos + 2]) << 16)
               | (static_cast<uint32_t>(buf[pos + 3]) << 24);
    pos += 4;
    return v;
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

inline void mutable_end(std::vector<uint8_t>& out, const MutableScope& s) {
    dheader_end(out, s.dheader_off);
}

/// Plain-1-byte member (LC=0).
inline void emheader_u8(std::vector<uint8_t>& out, size_t origin, uint32_t id, bool must_understand, uint8_t v) {
    pad_to_from_origin(out, origin, 4);
    emheader_write(out, emheader_make(0, id, must_understand));
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

/// Plain-8-byte member (LC=3 used as 8-byte-primitive in this flavour).
template <typename T>
inline void emheader_8(std::vector<uint8_t>& out, size_t origin, uint32_t id, bool must_understand, T v) {
    static_assert(sizeof(T) == 8, "emheader_8 needs 8-byte type");
    pad_to_from_origin(out, origin, 4);
    emheader_write(out, emheader_make(3, id, must_understand));
    write_le_raw<T>(out, v);
}

/// Variable-length member with NEXTINT (LC=3 with following uint32-NEXTINT).
/// Caller writes the body between begin/end; helper patches NEXTINT.
struct EmheaderNextintScope {
    size_t nextint_off;
    size_t body_start;
};

inline EmheaderNextintScope emheader_nextint_begin(std::vector<uint8_t>& out, size_t origin, uint32_t id, bool must_understand) {
    pad_to_from_origin(out, origin, 4);
    emheader_write(out, emheader_make(3, id, must_understand));
    EmheaderNextintScope s;
    s.nextint_off = out.size();
    write_le_raw<uint32_t>(out, 0u);
    s.body_start = out.size();
    return s;
}

inline void emheader_nextint_end(std::vector<uint8_t>& out, const EmheaderNextintScope& s) {
    uint32_t size = static_cast<uint32_t>(out.size() - s.body_start);
    uint8_t buf[4];
    std::memcpy(buf, &size, 4);
    if (!is_host_le()) {
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

inline EmheaderRead emheader_read(const uint8_t* buf, size_t& pos, size_t len, size_t origin) {
    skip_pad_from_origin(pos, origin, 4);
    uint32_t e = emheader_read_raw(buf, pos, len);
    EmheaderRead r;
    r.raw = e;
    r.lc = (e >> 28) & 0x7u;
    r.member_id = e & 0x0FFFFFFFu;
    r.must_understand = (e & EMHEADER_MU_FLAG_BIT) != 0;
    return r;
}

inline uint32_t emheader_nextint_read(const uint8_t* buf, size_t& pos, size_t len) {
    return read_le_raw<uint32_t>(buf, pos, len);
}

} // namespace xcdr2
} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_XCDR2_HPP
