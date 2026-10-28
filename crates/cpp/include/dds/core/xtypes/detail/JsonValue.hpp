// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/JsonValue.hpp — the C++ value of a member of a DynamicData and its JSON
// form (the C API's dialect: a 64-bit integer is a string, an enumeration an enumerator's name,
// a bit set an object of its bit fields).

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_JSONVALUE_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_JSONVALUE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <set>
#include <string>
#include <stdexcept>
#include <utility>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/xtypes/detail/Json.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

/// The semantics of `std::stoll`, `std::stoull` and `std::stold`: leading blanks are skipped,
/// trailing characters are ignored, `std::invalid_argument` when nothing converts,
/// `std::out_of_range` when the value does not fit.
inline int64_t parse_int64(const std::string& s) {
    errno = 0;
    char* end = 0;
    const long long v = std::strtoll(s.c_str(), &end, 10);
    if (end == s.c_str()) {
        throw std::invalid_argument("parse_int64");
    }
    if (errno == ERANGE) {
        throw std::out_of_range("parse_int64");
    }
    return static_cast<int64_t>(v);
}
inline uint64_t parse_uint64(const std::string& s) {
    errno = 0;
    char* end = 0;
    const unsigned long long v = std::strtoull(s.c_str(), &end, 10);
    if (end == s.c_str()) {
        throw std::invalid_argument("parse_uint64");
    }
    if (errno == ERANGE) {
        throw std::out_of_range("parse_uint64");
    }
    return static_cast<uint64_t>(v);
}
inline long double parse_long_double(const std::string& s) {
    errno = 0;
    char* end = 0;
    const long double v = std::strtold(s.c_str(), &end);
    if (end == s.c_str()) {
        throw std::invalid_argument("parse_long_double");
    }
    if (errno == ERANGE) {
        throw std::out_of_range("parse_long_double");
    }
    return v;
}

inline const std::string& number_text(const Json& j) {
    if (j.kind() != Json::Kind::Number && j.kind() != Json::Kind::String) {
        throw ::dds::core::InvalidDataError("DynamicData: the member is not a number");
    }
    return j.text();
}

template <typename T>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_same<T, bool>::value, T>::type from_json(const Json& j) {
    if (j.kind() != Json::Kind::Bool) {
        throw ::dds::core::InvalidDataError("DynamicData: the member is not a boolean");
    }
    return j.as_bool();
}
template <typename T>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_integral<T>::value && !::zerodds::cxx::is_same<T, bool>::value &&
                            ::zerodds::cxx::is_signed<T>::value,
                        T>::type
from_json(const Json& j) {
    return static_cast<T>(parse_int64(number_text(j)));
}
template <typename T>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_integral<T>::value && !::zerodds::cxx::is_same<T, bool>::value &&
                            ::zerodds::cxx::is_unsigned<T>::value,
                        T>::type
from_json(const Json& j) {
    return static_cast<T>(parse_uint64(number_text(j)));
}
template <typename T>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_floating_point<T>::value, T>::type from_json(const Json& j) {
    return static_cast<T>(parse_long_double(number_text(j)));
}
template <typename T>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_same<T, std::string>::value, T>::type from_json(const Json& j) {
    if (j.kind() != Json::Kind::String) {
        throw ::dds::core::InvalidDataError("DynamicData: the member is not a string");
    }
    return j.text();
}
template <typename T>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_same<T, std::map<std::string, uint64_t> >::value, T>::type
from_json(const Json& j) {
    if (j.kind() != Json::Kind::Object) {
        throw ::dds::core::InvalidDataError("DynamicData: the member is not a bit set");
    }
    T out;
    const std::vector<std::pair<std::string, Json> >& fields = j.fields();
    for (std::vector<std::pair<std::string, Json> >::const_iterator it = fields.begin();
         it != fields.end(); ++it) {
        out[it->first] = parse_uint64(number_text(it->second));
    }
    return out;
}

template <typename T>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_same<T, std::set<std::string> >::value, T>::type
from_json(const Json& j) {
    if (j.kind() != Json::Kind::Array) {
        throw ::dds::core::InvalidDataError("DynamicData: the member is not a bit mask");
    }
    T out;
    const std::vector<Json>& items = j.items();
    for (std::vector<Json>::const_iterator it = items.begin(); it != items.end(); ++it) {
        if (it->kind() != Json::Kind::String) {
            throw ::dds::core::InvalidDataError("DynamicData: a flag of a bit mask is a name");
        }
        out.insert(it->text());
    }
    return out;
}

template <typename T>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_same<T, bool>::value, Json>::type to_json(const T& v) {
    return Json::boolean(v);
}
template <typename T>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_integral<T>::value && !::zerodds::cxx::is_same<T, bool>::value, Json>::type
to_json(const T& v) {
    // 64-bit integers are strings in the dialect; narrower ones are numbers.
    return sizeof(T) >= 8 ? Json::string(::zerodds::cxx::to_string(v)) : Json::number(::zerodds::cxx::to_string(v));
}
template <typename T>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_floating_point<T>::value, Json>::type to_json(const T& v) {
    char buf[64];
    std::snprintf(buf, sizeof buf, "%.17g", static_cast<double>(v));
    return Json::number(buf);
}
inline Json to_json(const std::map<std::string, uint64_t>& v) {
    Json o = Json::object();
    for (std::map<std::string, uint64_t>::const_iterator it = v.begin(); it != v.end(); ++it) {
        o.set(it->first, Json::number(::zerodds::cxx::to_string(it->second)));
    }
    return o;
}
inline Json to_json(const std::set<std::string>& v) {
    Json a = Json::array();
    for (std::set<std::string>::const_iterator it = v.begin(); it != v.end(); ++it) {
        a.items().push_back(Json::string(*it));
    }
    return a;
}
inline Json to_json(const std::string& v) { return Json::string(v); }
inline Json to_json(const char* v) { return Json::string(v); }

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_JSONVALUE_HPP
