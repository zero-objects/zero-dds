// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TopicTraits.hpp — Trait-Pflicht fuer DDS-Sample-Types.
//
// `dds::topic::Topic<T>` benoetigt fuer jeden T eine Spezialisierung
// von `topic_type_support<T>` mit:
//   * `static const char* type_name();`
//   * `static std::vector<uint8_t> encode(const T& v);`
//   * `static T decode(const uint8_t* buf, size_t len);`
//
// IDL-Bindings (`crates/idl-cpp`) emittieren diese Spezialisierung
// automatisch fuer jede `struct`-Definition. Anwendungen ohne IDL
// koennen sie von Hand bereitstellen oder die `dds::core::ByteSeq`-
// Default-Spezialisierung benutzen, die rohe Byte-Buffer ueber das
// Wire schickt.
//
// Wire-Format der idl-cpp-emittierten Spezialisierungen
// -----------------------------------------------------
// Seit zerodds-xcdr2-cpp-1.0 emittiert idl-cpp voll-XCDR2 gemaess
// XTypes 1.3 §7.4: PLAIN_CDR2 LE mit Alignment, DHEADER fuer
// Appendable, EMHEADER + LC fuer Mutable. Die Wire-Helpers liegen
// in `dds/topic/xcdr2.hpp` und `dds/topic/xcdr2_md5.hpp`. Das hier
// definierte `cdr_lite`-Namespace bleibt fuer Bestandsbenutzer
// (Hand-geschriebene `topic_type_support<T>`-Spezialisierungen ohne
// IDL) erhalten, wird vom Codegen aber nicht mehr verwendet.

#ifndef ZERODDS_DDS_TOPIC_TOPICTRAITS_HPP
#define ZERODDS_DDS_TOPIC_TOPICTRAITS_HPP

#include <cstdint>
#include <cstring>
#include <stdexcept>
#include <string>
#include <vector>

namespace dds {
namespace core {

/// Raw-Bytes Sample-Type fuer Topics ohne IDL.
using ByteSeq = std::vector<uint8_t>;

} // namespace core

namespace topic {

/// Trait der pro `T` spezialisiert wird.
template <typename T>
struct topic_type_support;

// ---------------------------------------------------------------------------
// cdr_lite — Plain-Wire-Helper fuer idl-cpp-emittierte Spezialisierungen.
//
// Format: Little-Endian, kein Padding, kein DHEADER/EMHEADER.
//   * Primitives: raw bytes in declared type-size (LE).
//   * std::string: 4-Byte LE length + UTF-8 bytes (kein NUL-Terminator).
//   * std::vector<T>: 4-Byte LE count + elements (rekursiv).
//
// Read-Funktionen werfen std::out_of_range bei buffer-underrun.
// ---------------------------------------------------------------------------
namespace cdr_lite {

inline void append_bytes(std::vector<uint8_t>& out, const void* src, size_t n) {
    const auto* p = static_cast<const uint8_t*>(src);
    out.insert(out.end(), p, p + n);
}

inline void check_avail(size_t pos, size_t need, size_t len) {
    if (pos + need > len) {
        throw std::out_of_range("cdr_lite: buffer underrun");
    }
}

template <typename T>
inline void write_le(std::vector<uint8_t>& out, T v) {
    static_assert(std::is_trivially_copyable<T>::value, "write_le requires trivially-copyable T");
    append_bytes(out, &v, sizeof(T));
}

template <typename T>
inline T read_le(const uint8_t* buf, size_t& pos, size_t len) {
    static_assert(std::is_trivially_copyable<T>::value, "read_le requires trivially-copyable T");
    check_avail(pos, sizeof(T), len);
    T v;
    std::memcpy(&v, buf + pos, sizeof(T));
    pos += sizeof(T);
    return v;
}

inline void write_bool(std::vector<uint8_t>& out, bool v) {
    out.push_back(v ? uint8_t{1} : uint8_t{0});
}

inline bool read_bool(const uint8_t* buf, size_t& pos, size_t len) {
    check_avail(pos, 1, len);
    bool v = buf[pos] != 0;
    pos += 1;
    return v;
}

inline void write_string(std::vector<uint8_t>& out, const std::string& s) {
    write_le<uint32_t>(out, static_cast<uint32_t>(s.size()));
    append_bytes(out, s.data(), s.size());
}

inline std::string read_string(const uint8_t* buf, size_t& pos, size_t len) {
    auto n = read_le<uint32_t>(buf, pos, len);
    check_avail(pos, n, len);
    std::string s(reinterpret_cast<const char*>(buf + pos), n);
    pos += n;
    return s;
}

template <typename Elem, typename WriteFn>
inline void write_seq(std::vector<uint8_t>& out, const std::vector<Elem>& v, WriteFn write_elem) {
    write_le<uint32_t>(out, static_cast<uint32_t>(v.size()));
    for (const auto& e : v) {
        write_elem(out, e);
    }
}

template <typename Elem, typename ReadFn>
inline std::vector<Elem> read_seq(const uint8_t* buf, size_t& pos, size_t len, ReadFn read_elem) {
    auto n = read_le<uint32_t>(buf, pos, len);
    std::vector<Elem> v;
    v.reserve(n);
    for (uint32_t i = 0; i < n; ++i) {
        v.push_back(read_elem(buf, pos, len));
    }
    return v;
}

} // namespace cdr_lite

/// Default-Spezialisierung fuer `dds::core::ByteSeq` (raw bytes).
template <>
struct topic_type_support<::dds::core::ByteSeq> {
    /// Type-Name fuer den built-in Bytes-Topic.
    static const char* type_name() { return "DDS::Bytes"; }
    /// Encode = identity.
    static std::vector<uint8_t> encode(const ::dds::core::ByteSeq& v) { return v; }
    /// Decode = identity.
    static ::dds::core::ByteSeq decode(const uint8_t* buf, size_t len) {
        return ::dds::core::ByteSeq(buf, buf + len);
    }
};

/// Default-Spezialisierung fuer `std::string` (UTF-8-Strings).
template <>
struct topic_type_support<std::string> {
    /// Type-Name.
    static const char* type_name() { return "DDS::String"; }
    /// Encode = string-bytes.
    static std::vector<uint8_t> encode(const std::string& v) {
        return std::vector<uint8_t>(v.begin(), v.end());
    }
    /// Decode = string-bytes.
    static std::string decode(const uint8_t* buf, size_t len) {
        return std::string(reinterpret_cast<const char*>(buf), len);
    }
};

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_TOPICTRAITS_HPP
