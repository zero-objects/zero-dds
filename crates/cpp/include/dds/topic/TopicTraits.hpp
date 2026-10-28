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
// IDL bindings (`crates/lang-rules`) emit this specialization
// automatically for every `struct` definition. Applications without IDL
// can provide it by hand or use the `dds::core::ByteSeq`
// default specialization, which sends raw byte buffers over the
// wire.
//
// Wire format of the generated specializations
// -----------------------------------------------------
// the code generator emits full XCDR2 (or, under `--encoding xcdr1`, XCDR1) per
// XTypes 1.3 §7.4: PLAIN_CDR{,2} with alignment, DHEADER for Appendable,
// EMHEADER + LC for Mutable under XCDR2. The codec is `zerodds::wire::Writer`
// / `Reader` (`runtime/cpp/include/zerodds/wire.hpp`) — the one C++ wire
// codec; a `topic_type_support<T>::encode`/`decode` calls into it and
// nothing else. The `zerodds::psm::cdr_lite` namespace is kept for existing
// users (hand-written `topic_type_support<T>` specializations without IDL,
// e.g. the two below), but is no longer used by the codegen.

#ifndef ZERODDS_DDS_TOPIC_TOPICTRAITS_HPP
#define ZERODDS_DDS_TOPIC_TOPICTRAITS_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <cstring>
#include <stdexcept>
#include <string>
#include <vector>

#include "dds/core/types.hpp"
#include "zerodds/psm/type_support.hpp"

namespace dds {
namespace topic {

/// Trait that is specialized per `T`.
template <typename T>
struct topic_type_support;


namespace detail {

/// The unused-type tags of the detection traits below. A trait asks whether an expression on
/// `TS` compiles by putting it into a `sizeof` in a function template's parameter (ISO C++03
/// has no `decltype`/`void_t`: substitution failure in the expression removes the overload).
typedef char detect_yes;
struct detect_no {
    char pad[2];
};
template <std::size_t N>
struct detect_size {};

/// Whether `TS` carries a generated wire contract — the CompleteTypeObject,
/// the `@key` flag and the extensibility that `zerodds-idlc generate --lang
/// c++` writes into every `topic_type_support<T>`. The hand-written
/// `ByteSeq` and `std::string` specializations below carry none, and their
/// endpoints stay untyped.
template <typename TS>
struct has_wire_contract {
    template <typename U>
    static detect_yes test(detect_size<sizeof((void)U::type_object(), (void)U::type_object_len(),
                                              (void)U::is_keyed(), (void)U::extensibility(), 0)>*);
    template <typename U>
    static detect_no test(...);
    static const bool value = sizeof(test<TS>(0)) == sizeof(detect_yes);
};
template <typename TS>
const bool has_wire_contract<TS>::value;

/// Whether `TS` finds the type of a topic by the topic's type name at run time
/// (`Topic<DynamicData>`): `TS::contract_for(type_name)` names the TypeObject, the key and the
/// extensibility, `TS::decode_for(type_name, ...)` decodes a sample.
template <typename TS>
struct has_dynamic_contract {
    static const std::string& name_arg();
    template <typename U>
    static detect_yes test(detect_size<sizeof((void)U::contract_for(name_arg()), 0)>*);
    template <typename U>
    static detect_no test(...);
    static const bool value = sizeof(test<TS>(0)) == sizeof(detect_yes);
};
template <typename TS>
const bool has_dynamic_contract<TS>::value;

/// Whether `TS::type_name()` exists.
template <typename TS>
struct has_type_name {
    template <typename U>
    static detect_yes test(detect_size<sizeof((void)U::type_name(), 0)>*);
    template <typename U>
    static detect_no test(...);
    static const bool value = sizeof(test<TS>(0)) == sizeof(detect_yes);
};
template <typename TS>
const bool has_type_name<TS>::value;

/// `TS::extensibility()` in the C ABI's numbering: `0` FINAL, `1`
/// APPENDABLE, `2` MUTABLE.
template <typename TS>
int wire_extensibility() {
    typedef ::zerodds::psm::DataRepresentationKind Kind;
    switch (TS::extensibility()) {
    case Kind::APPENDABLE: return 1;
    case Kind::MUTABLE: return 2;
    case Kind::FINAL: break;
    }
    return 0;
}

} // namespace detail


/// Default specialization for `dds::core::ByteSeq` (raw bytes).
template <>
struct topic_type_support< ::dds::core::ByteSeq> {
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

/// DDS-PSM-Cxx 1.0 (dds/topic/TopicTraits.hpp): whether `T` is a topic type. The value is 0 until
/// the type is registered with `REGISTER_TOPIC_TYPE`; the types of this header are registered.
template <typename T>
struct is_topic_type {
    enum { value = 0 };
};
template <>
struct is_topic_type< ::dds::core::ByteSeq> {
    enum { value = 1 };
};
template <>
struct is_topic_type<std::string> {
    enum { value = 1 };
};

/// The name of the topic type: `topic_type_support<T>::type_name()` when `T` has a support, the
/// string "Undefined" of the normative header otherwise. A specialization may replace it.
namespace detail {
template <typename T, bool HasName>
struct topic_type_name_impl {
    static std::string value() { return "Undefined"; }
};
template <typename T>
struct topic_type_name_impl<T, true> {
    static std::string value() { return topic_type_support<T>::type_name(); }
};
} // namespace detail

template <typename T, typename Enable = void>
struct topic_type_name
    : detail::topic_type_name_impl<T, detail::has_type_name<topic_type_support<T> >::value> {};
} // namespace topic
} // namespace dds

#define REGISTER_TOPIC_TYPE(TOPIC_TYPE)                  \
    namespace dds {                                      \
    namespace topic {                                    \
    template <>                                          \
    struct is_topic_type<TOPIC_TYPE> {                   \
        enum { value = 1 };                              \
    };                                                   \
    }                                                    \
    }

#endif // ZERODDS_DDS_TOPIC_TOPICTRAITS_HPP
