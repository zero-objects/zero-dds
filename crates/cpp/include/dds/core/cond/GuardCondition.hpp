// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/GuardCondition.hpp — GuardCondition (DDS-PSM-Cxx 1.0 §7.5).

#ifndef ZERODDS_DDS_CORE_COND_GUARDCONDITION_HPP
#define ZERODDS_DDS_CORE_COND_GUARDCONDITION_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/cond/TGuardCondition.hpp"
#include "dds/core/cond/detail/GuardConditionDelegate.hpp"

namespace dds {
namespace core {
namespace cond {

/// GuardCondition (Spec §7.5). Reference type with delegate pattern: `TGuardCondition`
/// over ZeroDDS's delegate.
class GuardCondition : public TGuardCondition<detail::GuardConditionDelegate> {
public:
    typedef TGuardCondition<detail::GuardConditionDelegate> BASE_T;
    typedef BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A guard condition sharing `ref`.
    explicit GuardCondition(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A new guard condition, not triggered.
    GuardCondition() {}

    /// The null GuardCondition (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    GuardCondition(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    GuardCondition(const GuardCondition&) = default;
    GuardCondition& operator=(const GuardCondition&) = default;
    GuardCondition(GuardCondition&&) = default;
    GuardCondition& operator=(GuardCondition&&) = default;
    ~GuardCondition() = default;
#endif
};

} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_GUARDCONDITION_HPP
