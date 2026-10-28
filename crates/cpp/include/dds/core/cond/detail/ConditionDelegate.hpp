// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/detail/ConditionDelegate.hpp — Delegate for Condition.

#ifndef ZERODDS_DDS_CORE_COND_DETAIL_CONDITIONDELEGATE_HPP
#define ZERODDS_DDS_CORE_COND_DETAIL_CONDITIONDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"

#include "dds/core/Exception.hpp"
#include "zerodds.h"

namespace dds {
namespace core {
namespace cond {
namespace detail {

/// Delegate for Condition (DDS-PSM-Cxx 1.0 §7.5).
class ConditionDelegate {
public:
    explicit ConditionDelegate(void* handle = nullptr) : handle_(handle) {}
    virtual ~ConditionDelegate() {}

    // Non-copyable, non-movable (owned by shared_ptr).
    ZERODDS_NONCOPYABLE(ConditionDelegate)
#if ZERODDS_CXX11
    ConditionDelegate(ConditionDelegate&&) = delete;
    ConditionDelegate& operator=(ConditionDelegate&&) = delete;
#endif

    void* native_handle() const { return handle_; }
    bool trigger_value() const {
        return handle_ != nullptr && zerodds_condition_get_trigger_value(handle_);
    }
    virtual void dispatch() {
        // Default implementation does nothing.
    }

protected:
    void* handle_;
};

} // namespace detail
} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_DETAIL_CONDITIONDELEGATE_HPP