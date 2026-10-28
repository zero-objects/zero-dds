// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/TCondition.hpp — Condition template (DDS-PSM-Cxx 1.0 §7.5).

#ifndef ZERODDS_DDS_CORE_COND_TCONDITION_HPP
#define ZERODDS_DDS_CORE_COND_TCONDITION_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/Reference.hpp"
#include "dds/core/Time.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/cond/detail/ConditionDelegate.hpp"

namespace dds {
namespace core {
namespace cond {

/// Condition template (Spec §7.5). Reference type with delegate pattern.
template <typename DELEGATE = detail::ConditionDelegate>
class TCondition : public ::dds::core::Reference<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE_T> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// The constructors of Reference: null, from delegate*, from shared_ptr, from another
    /// reference type.
    explicit TCondition(const ::dds::core::null_type& n) ZERODDS_NOEXCEPT : BASE_T(n) {}
    explicit TCondition(DELEGATE* p) ZERODDS_NOEXCEPT : BASE_T(p) {}
    explicit TCondition(const DELEGATE_REF_T& ref) ZERODDS_NOEXCEPT : BASE_T(ref) {}
    template <typename D>
    TCondition(const ::dds::core::Reference<D>& other) ZERODDS_NOEXCEPT : BASE_T(other) {}

    /// Cross-type constructor from any TCondition with compatible delegate.
    template <typename D>
    TCondition(const TCondition<D>& other) ZERODDS_NOEXCEPT : BASE_T(other) {}

    /// Trigger value (Spec §7.5).
    bool trigger_value() const {
        return this->impl_ ? this->delegate()->trigger_value() : false;
    }

    /// Dispatches the functors registered with the condition.
    void dispatch() {
        if (this->impl_) {
            this->delegate()->dispatch();
        }
    }
};

} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_TCONDITION_HPP