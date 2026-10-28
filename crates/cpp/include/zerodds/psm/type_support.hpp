// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/psm/type_support.hpp — ZeroDDS's own helpers for
// `dds::topic::topic_type_support<T>`. DDS-PSM-Cxx 1.0 §2.2 keeps vendor
// interfaces out of the `dds` namespaces, so they live in `zerodds::psm`.

#ifndef ZERODDS_PSM_TYPE_SUPPORT_HPP
#define ZERODDS_PSM_TYPE_SUPPORT_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <cstddef>
#include <stdint.h>
#include <cstring>
#include <stdexcept>
#include <string>
#include <vector>

#if ZERODDS_CXX11
#include <type_traits>
#endif

namespace zerodds {
namespace psm {


/// The extensibility a topic type's samples are framed with — XTypes 1.3
/// §7.2.2.4.4.4. `topic_type_support<T>::extensibility()` returns one of
/// these, and `zerodds::TypedWriter<T>` (`zerodds/dds.hpp`) declares it to
/// the C ABI so the encapsulation identifier names the body honestly:
/// XTypes 1.3 §7.6.3.1.2 Table 59, DDSI-RTPS 2.5 §10.5.
///
/// Lives in `zerodds::psm`, outside the namespaces DDS-PSM-Cxx 1.0 §2.2 reserves.
///
/// C++11 and later: `enum class`. C++03 has no scoped enumeration; there it is a class that wraps a
/// plain enumeration, so `DataRepresentationKind::FINAL` and a `switch` over a value are written
/// the same way in both modes.
#if ZERODDS_CXX11
enum class DataRepresentationKind {
    FINAL,
    APPENDABLE,
    MUTABLE
};
#else
struct DataRepresentationKind {
    enum Value { FINAL, APPENDABLE, MUTABLE };
    DataRepresentationKind(Value v = FINAL) : value_(v) {}
    operator Value() const { return value_; }

private:
    Value value_;
};
#endif


// ---------------------------------------------------------------------------
// cdr_lite — plain-wire helper for generated specializations.
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
    const uint8_t* p = static_cast<const uint8_t*>(src);
    out.insert(out.end(), p, p + n);
}

inline void check_avail(size_t pos, size_t need, size_t len) {
    if (pos + need > len) {
        throw std::out_of_range("cdr_lite: buffer underrun");
    }
}

template <typename T>
inline void write_le(std::vector<uint8_t>& out, T v) {
#if ZERODDS_CXX11
    static_assert(std::is_trivially_copyable<T>::value, "write_le requires trivially-copyable T");
#endif
    append_bytes(out, &v, sizeof(T));
}

template <typename T>
inline T read_le(const uint8_t* buf, size_t& pos, size_t len) {
#if ZERODDS_CXX11
    static_assert(std::is_trivially_copyable<T>::value, "read_le requires trivially-copyable T");
#endif
    check_avail(pos, sizeof(T), len);
    T v;
    std::memcpy(&v, buf + pos, sizeof(T));
    pos += sizeof(T);
    return v;
}

inline void write_bool(std::vector<uint8_t>& out, bool v) {
    out.push_back(v ? static_cast<uint8_t>(1) : static_cast<uint8_t>(0));
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
    uint32_t n = read_le<uint32_t>(buf, pos, len);
    check_avail(pos, n, len);
    std::string s(reinterpret_cast<const char*>(buf + pos), n);
    pos += n;
    return s;
}

template <typename Elem, typename WriteFn>
inline void write_seq(std::vector<uint8_t>& out, const std::vector<Elem>& v, WriteFn write_elem) {
    write_le<uint32_t>(out, static_cast<uint32_t>(v.size()));
    for (typename std::vector<Elem>::const_iterator it = v.begin(); it != v.end(); ++it) {
        write_elem(out, *it);
    }
}

template <typename Elem, typename ReadFn>
inline std::vector<Elem> read_seq(const uint8_t* buf, size_t& pos, size_t len, ReadFn read_elem) {
    uint32_t n = read_le<uint32_t>(buf, pos, len);
    std::vector<Elem> v;
    v.reserve(n);
    for (uint32_t i = 0; i < n; ++i) {
        v.push_back(read_elem(buf, pos, len));
    }
    return v;
}

} // namespace cdr_lite

} // namespace psm
} // namespace zerodds

#endif // ZERODDS_PSM_TYPE_SUPPORT_HPP
