// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/BuiltinTopicTypes.hpp — DDS-PSM-Cxx 1.0 (core): `BytesTopicType`, `StringTopicType`,
// `KeyedBytesTopicType`, `KeyedStringTopicType`, with the `topic_type_support` that makes them
// topic types: `DDS::Bytes`, `DDS::String`, `DDS::KeyedBytes` and `DDS::KeyedString` of
// DDS-XTypes 1.3 Annex E (appendable, the member `value`, the key `key`), XCDR2.

#ifndef ZERODDS_DDS_CORE_BUILTINTOPICTYPES_HPP
#define ZERODDS_DDS_CORE_BUILTINTOPICTYPES_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <cstdlib>
#include <map>
#include <string>
#include <vector>

#include "dds/core/TBuiltinTopicTypes.hpp"
#include "dds/core/detail/BuiltinTopicTypes.hpp"
#include "dds/core/xtypes/detail/DynTypeHandle.hpp"
#include "dds/core/xtypes/detail/Json.hpp"
#include "dds/topic/TopicTraits.hpp"
#include "zerodds/psm/encoding.hpp"

#if defined(OMG_DDS_X_TYPES_BUILTIN_TOPIC_TYPES_SUPPORT)

namespace dds {
namespace core {

typedef TBytesTopicType<detail::BytesTopicType> BytesTopicType;
typedef TStringTopicType<detail::StringTopicType> StringTopicType;
typedef TKeyedBytesTopicType<detail::KeyedBytesTopicType> KeyedBytesTopicType;
typedef TKeyedStringTopicType<detail::KeyedStringTopicType> KeyedStringTopicType;

}  // namespace core
}  // namespace dds

namespace zerodds {
namespace psm {
namespace builtin_topic_detail {

using ::dds::core::xtypes::detail::DynTypeHandle;
using ::dds::core::xtypes::detail::Json;

/// The codec of one built-in type: the type written as IDL and lowered by the C API, so the
/// TypeObject and the octets are those of a compiled type of the same IDL.
inline const DynTypeHandle& handle(const char* name) {
    static const char* kIdl =
        "module DDS { @appendable struct _String { string value; }; "
        "@appendable struct KeyedString { @key string key; string value; }; "
        "@appendable struct Bytes { sequence<octet> value; }; "
        "@appendable struct KeyedBytes { @key string key; sequence<octet> value; }; };";
    static ::zerodds::cxx::mutex mutex;
    static std::map<std::string, ::zerodds::cxx::shared_ptr<DynTypeHandle> > handles;
    ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex);
    ::zerodds::cxx::shared_ptr<DynTypeHandle>& slot = handles[name];
    if (slot.get() == 0) {
        slot.reset(new DynTypeHandle(kIdl, name));
    }
    return *slot;
}

inline Json bytes_json(const std::vector<uint8_t>& v) {
    Json a = Json::array();
    for (std::vector<uint8_t>::const_iterator it = v.begin(); it != v.end(); ++it) {
        a.items().push_back(Json::number(::zerodds::cxx::to_string(static_cast<unsigned>(*it))));
    }
    return a;
}
inline std::vector<uint8_t> bytes_of(const Json& j) {
    std::vector<uint8_t> out;
    for (std::vector<Json>::const_iterator it = j.items().begin(); it != j.items().end(); ++it) {
        out.push_back(static_cast<uint8_t>(std::strtoul(it->text().c_str(), 0, 10)));
    }
    return out;
}
inline const Json& member(const Json& value, const char* name) {
    const Json* m = value.find(name);
    if (m == nullptr) {
        throw ::dds::core::InvalidDataError(std::string("built-in topic type: no member ") + name);
    }
    return *m;
}

/// The members `TS` needs of a built-in type's codec; `T` converts to and from JSON.
template <typename T>
struct Codec;

template <>
struct Codec< ::dds::core::StringTopicType> {
    static const char* name() { return "DDS::String"; }
    static Json to_json(const ::dds::core::StringTopicType& v) {
        Json o = Json::object();
        o.set("value", Json::string(v.data()));
        return o;
    }
    static ::dds::core::StringTopicType from_json(const Json& j) {
        return ::dds::core::StringTopicType(member(j, "value").text());
    }
};
template <>
struct Codec< ::dds::core::BytesTopicType> {
    static const char* name() { return "DDS::Bytes"; }
    static Json to_json(const ::dds::core::BytesTopicType& v) {
        Json o = Json::object();
        o.set("value", bytes_json(v.data()));
        return o;
    }
    static ::dds::core::BytesTopicType from_json(const Json& j) {
        return ::dds::core::BytesTopicType(bytes_of(member(j, "value")));
    }
};
template <>
struct Codec< ::dds::core::KeyedStringTopicType> {
    static const char* name() { return "DDS::KeyedString"; }
    static Json to_json(const ::dds::core::KeyedStringTopicType& v) {
        Json o = Json::object();
        o.set("key", Json::string(v.key()));
        o.set("value", Json::string(v.value()));
        return o;
    }
    static ::dds::core::KeyedStringTopicType from_json(const Json& j) {
        return ::dds::core::KeyedStringTopicType(member(j, "key").text(), member(j, "value").text());
    }
};
template <>
struct Codec< ::dds::core::KeyedBytesTopicType> {
    static const char* name() { return "DDS::KeyedBytes"; }
    static Json to_json(const ::dds::core::KeyedBytesTopicType& v) {
        Json o = Json::object();
        o.set("key", Json::string(v.key()));
        o.set("value", bytes_json(v.value()));
        return o;
    }
    static ::dds::core::KeyedBytesTopicType from_json(const Json& j) {
        return ::dds::core::KeyedBytesTopicType(member(j, "key").text(),
                                                bytes_of(member(j, "value")));
    }
};

/// The `topic_type_support` of a built-in type.
template <typename T>
struct Support {
    static const char* type_name() { return Codec<T>::name(); }
    static const uint8_t* type_object() {
        const std::vector<uint8_t>& octets = handle(Codec<T>::name()).type_object();
        return octets.empty() ? static_cast<const uint8_t*>(0) : &octets[0];
    }
    static size_t type_object_len() { return handle(Codec<T>::name()).type_object().size(); }
    static bool is_keyed() { return handle(Codec<T>::name()).keyed(); }
    /// `encode` writes XCDR2, little-endian.
    static bool wire_xcdr2() { return true; }
    static ::zerodds::psm::DataRepresentationKind extensibility() {
        return ::zerodds::psm::DataRepresentationKind::APPENDABLE;
    }
    static size_t type_object_dependency_count() { return 0; }
    static const uint8_t* type_object_dependency(size_t) { return nullptr; }
    static size_t type_object_dependency_len(size_t) { return 0; }

    static std::vector<uint8_t> encode(const T& v) {
        return handle(Codec<T>::name()).encode(Codec<T>::to_json(v), true, false);
    }
    static T decode(const uint8_t* buf, size_t len, ::zerodds::wire::Encoding version,
                    bool big_endian) {
        return Codec<T>::from_json(handle(Codec<T>::name())
                                       .decode(buf, len, version == ::zerodds::wire::Encoding::PlainCdr2,
                                               big_endian));
    }
};

}  // namespace builtin_topic_detail
}  // namespace psm
}  // namespace zerodds

namespace dds {
namespace topic {

template <>
struct topic_type_support< ::dds::core::StringTopicType>
    : ::zerodds::psm::builtin_topic_detail::Support< ::dds::core::StringTopicType> {};
template <>
struct topic_type_support< ::dds::core::BytesTopicType>
    : ::zerodds::psm::builtin_topic_detail::Support< ::dds::core::BytesTopicType> {};
template <>
struct topic_type_support< ::dds::core::KeyedStringTopicType>
    : ::zerodds::psm::builtin_topic_detail::Support< ::dds::core::KeyedStringTopicType> {};
template <>
struct topic_type_support< ::dds::core::KeyedBytesTopicType>
    : ::zerodds::psm::builtin_topic_detail::Support< ::dds::core::KeyedBytesTopicType> {};

}  // namespace topic
}  // namespace dds

#endif  // OMG_DDS_X_TYPES_BUILTIN_TOPIC_TYPES_SUPPORT

#endif  // ZERODDS_DDS_CORE_BUILTINTOPICTYPES_HPP
