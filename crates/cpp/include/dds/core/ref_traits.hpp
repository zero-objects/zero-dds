// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/ref_traits.hpp — DDS-PSM-Cxx 1.0 §7.2 (type traits for Reference).

#ifndef ZERODDS_DDS_CORE_REF_TRAITS_HPP
#define ZERODDS_DDS_CORE_REF_TRAITS_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>

#include "dds/core/types.hpp"
#include "dds/core/Exception.hpp"

namespace dds {
namespace core {

/// Base-of check for delegate types (used in polymorphic casts).
template <typename T1, typename T2>
struct is_base_of : ::zerodds::cxx::is_base_of<T1, T2> { };

/// Same-type check for delegate types.
template <typename T1, typename T2>
struct is_same : ::zerodds::cxx::is_same<T1, T2> { };

/// Smart pointer traits for a delegate type.
/// DELEGATE_REF_T = ::zerodds::cxx::shared_ptr<DELEGATE>
/// DELEGATE_WEAK_REF_T = ::zerodds::cxx::weak_ptr<DELEGATE>
template <typename T>
struct smart_ptr_traits {
    typedef ::zerodds::cxx::shared_ptr<T> ref_type;
    typedef ::zerodds::cxx::weak_ptr<T> weak_ref_type;
};

/// Polymorphic downcast for reference types.
/// Throws InvalidDowncastError if the cast fails.
template <typename TO, typename FROM>
TO polymorphic_cast(const FROM& from) {
    typename TO::DELEGATE_REF_T dr =
        ::zerodds::cxx::dynamic_pointer_cast<typename TO::DELEGATE_T>(from.delegate());
    TO to(dr);
    if (to == null) {
        throw InvalidDowncastError("Attempted invalid downcast.");
    }
    return to;
}

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_REF_TRAITS_HPP