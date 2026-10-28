// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/StatusCondition.hpp — StatusCondition (DDS-PSM-Cxx 1.0 §7.5).

#ifndef ZERODDS_DDS_CORE_COND_STATUSCONDITION_HPP
#define ZERODDS_DDS_CORE_COND_STATUSCONDITION_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "dds/core/cond/TStatusCondition.hpp"
#include "dds/core/cond/detail/StatusConditionDelegate.hpp"

namespace dds {
namespace core {
namespace cond {

/// StatusCondition (Spec §7.5). Reference type with delegate pattern: `TStatusCondition`
/// over ZeroDDS's delegate. Bound to an entity.
class StatusCondition : public TStatusCondition<detail::StatusConditionDelegate> {
public:
    typedef TStatusCondition<detail::StatusConditionDelegate> BASE_T;
    typedef BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A status condition sharing `ref`.
    explicit StatusCondition(const DELEGATE_REF_T& ref) : BASE_T(ref) {}
    /// The status condition of `entity`, any entity reference type of this library.
    template <typename E>
    explicit StatusCondition(
        const E& entity,
        typename ::zerodds::cxx::enable_if<detail::HasNativeHandle<E>::value>::type* = 0)
        : BASE_T(entity) {}

    /// The null StatusCondition (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    StatusCondition(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    StatusCondition(const StatusCondition&) = default;
    StatusCondition& operator=(const StatusCondition&) = default;
    StatusCondition(StatusCondition&&) = default;
    StatusCondition& operator=(StatusCondition&&) = default;
    ~StatusCondition() = default;
#endif
};

} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_STATUSCONDITION_HPP
