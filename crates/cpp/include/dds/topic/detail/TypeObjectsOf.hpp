// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/detail/TypeObjectsOf.hpp — the CompleteTypeObject of a topic type, handed to a C call
// that takes it (`zerodds_dr_set_type_objects`, `zerodds_topic_set_type_objects`, ...).

#ifndef ZERODDS_DDS_TOPIC_DETAIL_TYPEOBJECTSOF_HPP
#define ZERODDS_DDS_TOPIC_DETAIL_TYPEOBJECTSOF_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <cstddef>
#include <string>
#include <vector>

#include "dds/topic/TopicTraits.hpp"

namespace dds {
namespace topic {
namespace detail {

/// Whether `topic_type_support<T>` carries the `CompleteTypeObject` members
/// `idlc cpp` emits (`type_object`, `type_object_len`, `is_keyed`,
/// `extensibility`); the hand-written `ByteSeq`/`std::string` supports do not.
template <typename S>
struct has_type_object {
    template <typename U>
    static detect_yes test(detect_size<sizeof((void)U::type_object(), (void)U::type_object_len(),
                                              (void)U::is_keyed(), (void)U::extensibility(), 0)>*);
    template <typename U>
    static detect_no test(...);
    static const bool value = sizeof(test<S>(0)) == sizeof(detect_yes);
};
template <typename S>
const bool has_type_object<S>::value;

/// Whether the support also names the TypeObjects its root refers to
/// (`idlc cpp` emits `type_object_dependency_count/_dependency/_len`).
template <typename S>
struct has_type_object_dependencies {
    template <typename U>
    static detect_yes test(
        detect_size<sizeof((void)U::type_object_dependency_count(),
                           (void)U::type_object_dependency(std::size_t(0)),
                           (void)U::type_object_dependency_len(std::size_t(0)), 0)>*);
    template <typename U>
    static detect_no test(...);
    static const bool value = sizeof(test<S>(0)) == sizeof(detect_yes);
};
template <typename S>
const bool has_type_object_dependencies<S>::value;

/// `&v[0]`, or a null pointer for an empty vector.
template <typename E>
inline const E* vector_data_or_null(const std::vector<E>& v) {
    return v.empty() ? static_cast<const E*>(0) : &v[0];
}

/// The selector of the variants of `with_type_objects`: `0` the support finds the type by the
/// topic's type name, `1` it carries a generated object, `2` it has none.
template <typename S>
struct type_objects_kind {
    static const int value = has_dynamic_contract<S>::value ? 0 : (has_type_object<S>::value ? 1 : 2);
};
template <typename S>
const int type_objects_kind<S>::value;

/// A support that finds the type by name: `handle` is the type of the contract `contract_for`
/// returned.
template <typename Handle, typename Call>
inline int call_with_handle(const Handle& handle, Call call) {
    std::vector<const uint8_t*> deps;
    std::vector<std::size_t> lens;
    for (std::size_t i = 0; i < handle.dependencies().size(); ++i) {
        const std::vector<uint8_t>& d = handle.dependencies()[i];
        deps.push_back(vector_data_or_null(d));
        lens.push_back(d.size());
    }
    return call(vector_data_or_null(handle.type_object()), handle.type_object().size(),
                vector_data_or_null(deps), vector_data_or_null(lens), deps.size(),
                handle.keyed() ? 1 : 0, static_cast<uint32_t>(handle.extensibility()));
}

template <typename Contract, typename Call>
inline int call_with_dynamic_contract(const Contract& contract, Call call) {
    return call_with_handle(*contract->handle, call);
}

template <typename T, typename Call>
inline int with_type_objects_impl(const std::string& type_name, Call call,
                                  ::zerodds::cxx::integral_constant<int, 0>) {
    typedef ::dds::topic::topic_type_support<T> support;
    return call_with_dynamic_contract(support::contract_for(type_name), call);
}

/// Calls the dependency accessors when the support has them.
template <typename S>
inline std::size_t collect_dependencies(std::vector<const uint8_t*>& deps,
                                        std::vector<std::size_t>& lens,
                                        ::zerodds::cxx::integral_constant<bool, true>) {
    const std::size_t n = S::type_object_dependency_count();
    for (std::size_t i = 0; i < n; ++i) {
        deps.push_back(S::type_object_dependency(i));
        lens.push_back(S::type_object_dependency_len(i));
    }
    return n;
}
template <typename S>
inline std::size_t collect_dependencies(std::vector<const uint8_t*>&, std::vector<std::size_t>&,
                                        ::zerodds::cxx::integral_constant<bool, false>) {
    return 0;
}

template <typename T, typename Call>
inline int with_type_objects_impl(const std::string&, Call call,
                                  ::zerodds::cxx::integral_constant<int, 1>) {
    typedef ::dds::topic::topic_type_support<T> support;
    const uint8_t* object = support::type_object();
    const std::size_t len = support::type_object_len();
    if (object == 0 || len == 0) {
        return -1000;
    }
    std::vector<const uint8_t*> deps;
    std::vector<std::size_t> lens;
    const std::size_t n = collect_dependencies<support>(
        deps, lens,
        ::zerodds::cxx::integral_constant<bool, has_type_object_dependencies<support>::value>());
    return call(object, len, n ? vector_data_or_null(deps) : static_cast<const uint8_t* const*>(0),
                n ? vector_data_or_null(lens) : static_cast<const std::size_t*>(0), n,
                support::is_keyed() ? 1 : 0, static_cast<uint32_t>(support::extensibility()));
}

template <typename T, typename Call>
inline int with_type_objects_impl(const std::string&, Call,
                                  ::zerodds::cxx::integral_constant<int, 2>) {
    return -1000;
}

/// Calls `call(object, len, dependencies, dependency_lens, dependency_count, keyed,
/// extensibility)` with the CompleteTypeObject of `T`'s topic type support and returns what it
/// returns; `-1000` when the support has no object (a hand-written one), without calling.
/// `type_name` names the topic type for a support that builds its object by name.
template <typename T, typename Call>
int with_type_objects(const std::string& type_name, Call call) {
    typedef ::dds::topic::topic_type_support<T> support;
    return with_type_objects_impl<T>(
        type_name, call,
        ::zerodds::cxx::integral_constant<int, type_objects_kind<support>::value>());
}

} // namespace detail
} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_DETAIL_TYPEOBJECTSOF_HPP
