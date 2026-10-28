// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/detail/GuardConditionDelegate.hpp — Delegate for GuardCondition.

#ifndef ZERODDS_DDS_CORE_COND_DETAIL_GUARDCONDITIONDELEGATE_HPP
#define ZERODDS_DDS_CORE_COND_DETAIL_GUARDCONDITIONDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include "zerodds/psm/check_status.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/cond/detail/ConditionDelegate.hpp"
#include <utility>
#include "zerodds.h"

namespace dds {
namespace core {
namespace cond {
namespace detail {

/// Delegate for GuardCondition (DDS-PSM-Cxx 1.0 §7.5).
class GuardConditionDelegate : public ConditionDelegate {
public:
    GuardConditionDelegate() : ConditionDelegate(zerodds_guardcondition_create()) {}
    ~GuardConditionDelegate() ZERODDS_OVERRIDE {
        if (handle_ != nullptr) {
            zerodds_guardcondition_destroy(static_cast<zerodds_ZeroDdsGuardCondition*>(handle_));
            handle_ = nullptr;
        }
    }

    // Non-copyable, non-movable (owned by shared_ptr).
    ZERODDS_NONCOPYABLE(GuardConditionDelegate)
#if ZERODDS_CXX11
    GuardConditionDelegate(GuardConditionDelegate&&) = delete;
    GuardConditionDelegate& operator=(GuardConditionDelegate&&) = delete;
#endif

    /// Bring base class trigger_value() const (getter) into scope.
    using ConditionDelegate::trigger_value;

    /// The function `dispatch()` calls while the condition holds.
    void handler(::zerodds::cxx::function<void()> f) {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
        handler_ = ZERODDS_MOVE(f);
    }
    void reset_handler() {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
        handler_ = ::zerodds::cxx::function<void()>();
    }
    void dispatch() ZERODDS_OVERRIDE {
        // A copy: the handler is called without the lock, and another thread may replace it.
        ::zerodds::cxx::function<void()> f;
        {
            ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
            f = handler_;
        }
        if (f && trigger_value()) {
            f();
        }
    }

    /// Sets the trigger value.
    void trigger_value(bool v) {
        int rc = zerodds_guardcondition_set_trigger_value(
            static_cast<zerodds_ZeroDdsGuardCondition*>(handle_), v);
        ::zerodds::psm::check_status(rc, "GuardConditionDelegate::trigger_value");
    }

private:
    ::zerodds::cxx::mutex mutex_;
    ::zerodds::cxx::function<void()> handler_;
};

} // namespace detail
} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_DETAIL_GUARDCONDITIONDELEGATE_HPP