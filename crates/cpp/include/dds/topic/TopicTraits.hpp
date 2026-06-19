// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TopicTraits.hpp — trait requirement for DDS sample types.
//
// `dds::topic::Topic<T>` requires for every T a specialization
// of `topic_type_support<T>` with:
//   * `static const char* type_name();`
//   * `static std::vector<uint8_t> encode(const T& v);`
//   * `static T decode(const uint8_t* buf, size_t len);`
//
// IDL bindings (`crates/idl-cpp`) emit this specialization
// automatically for every `struct` definition. Applications without IDL
// can provide it by hand or use the `dds::core::ByteSeq`
// default specialization, which sends raw byte buffers over the
// wire.
//
// Wire format of the idl-cpp-emitted specializations
// -----------------------------------------------------
// Since zerodds-xcdr2-cpp-1.0 idl-cpp emits full XCDR2 per
// XTypes 1.3 §7.4: PLAIN_CDR2 LE with alignment, DHEADER for
// Appendable, EMHEADER + LC for Mutable. The wire helpers live
// in `dds/topic/xcdr2.hpp` and `dds/topic/xcdr2_md5.hpp`. The
// `cdr_lite` namespace defined here is kept for existing users
// (hand-written `topic_type_support<T>` specializations without
// IDL), but is no longer used by the codegen.

#ifndef ZERODDS_DDS_TOPIC_TOPICTRAITS_HPP
#define ZERODDS_DDS_TOPIC_TOPICTRAITS_HPP

#include <cstdint>
#include <cstring>
#include <stdexcept>
#include <string>
#include <vector>

namespace dds {
namespace core {

/// Raw-bytes sample type for topics without IDL.
using ByteSeq = std::vector<uint8_t>;

} // namespace core

namespace topic {

/// Trait that is specialized per `T`.
template <typename T>
struct topic_type_support;

// ---------------------------------------------------------------------------
// cdr_lite — plain-wire helper for idl-cpp-emitted specializations.
//
// Format: little-endian, no padding, no DHEADER/EMHEADER.
//   * Primitives: raw bytes in declared type-size (LE).
//   * std::string: 4-byte LE length + UTF-8 bytes (no NUL terminator).
//   * std::vector<T>: 4-byte LE count + elements (recursive).
//
// Read functions throw std::out_of_range on buffer underrun.
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

/// Default specialization for `dds::core::ByteSeq` (raw bytes).
template <>
struct topic_type_support<::dds::core::ByteSeq> {
    /// Type name for the built-in bytes topic.
    static const char* type_name() { return "DDS::Bytes"; }
    /// Encode = identity.
    static std::vector<uint8_t> encode(const ::dds::core::ByteSeq& v) { return v; }
    /// Decode = identity.
    static ::dds::core::ByteSeq decode(const uint8_t* buf, size_t len) {
        return ::dds::core::ByteSeq(buf, buf + len);
    }
};

/// Default specialization for `std::string` (UTF-8 strings).
template <>
struct topic_type_support<std::string> {
    /// Type name.
    static const char* type_name() { return "DDS::String"; }
    /// Encode = string bytes.
    static std::vector<uint8_t> encode(const std::string& v) {
        return std::vector<uint8_t>(v.begin(), v.end());
    }
    /// Decode = string bytes.
    static std::string decode(const uint8_t* buf, size_t len) {
        return std::string(reinterpret_cast<const char*>(buf), len);
    }
};

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_TOPICTRAITS_HPP
