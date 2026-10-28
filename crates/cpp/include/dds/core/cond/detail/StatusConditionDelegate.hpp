// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/detail/StatusConditionDelegate.hpp — Delegate for StatusCondition.

#ifndef ZERODDS_DDS_CORE_COND_DETAIL_STATUSCONDITIONDELEGATE_HPP
#define ZERODDS_DDS_CORE_COND_DETAIL_STATUSCONDITIONDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/Entity.hpp"
#include "dds/core/cond/detail/ConditionDelegate.hpp"
#include <utility>
#include "zerodds.h"

namespace dds {
namespace core {
namespace cond {
namespace detail {

/// Delegate for StatusCondition (DDS-PSM-Cxx 1.0 §7.5).
class StatusConditionDelegate : public ConditionDelegate {
public:
    StatusConditionDelegate(void* entity_handle, ::dds::core::Entity entity)
        : ConditionDelegate(zerodds_entity_get_statuscondition(entity_handle)),
          entity_(ZERODDS_MOVE(entity)) {}

    /// The entity the condition is bound to.
    const ::dds::core::Entity& entity() const { return entity_; }
    ~StatusConditionDelegate() ZERODDS_OVERRIDE {
        if (handle_ != nullptr) {
            zerodds_statuscondition_destroy(
                static_cast<zerodds_ZeroDdsStatusCondition*>(handle_));
            handle_ = nullptr;
        }
    }

    // Non-copyable, non-movable (owned by shared_ptr).
    ZERODDS_NONCOPYABLE(StatusConditionDelegate)
#if ZERODDS_CXX11
    StatusConditionDelegate(StatusConditionDelegate&&) = delete;
    StatusConditionDelegate& operator=(StatusConditionDelegate&&) = delete;
#endif

    void enabled_statuses(uint32_t mask) {
        int rc = zerodds_statuscondition_set_enabled_statuses(
            static_cast<zerodds_ZeroDdsStatusCondition*>(handle_), mask);
        ::zerodds::psm::check_status(rc, "StatusConditionDelegate::enabled_statuses");
    }
    uint32_t enabled_statuses() const {
        return zerodds_statuscondition_get_enabled_statuses(
            static_cast<zerodds_ZeroDdsStatusCondition*>(handle_));
    }

private:
    ::dds::core::Entity entity_;
};

} // namespace detail
} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_DETAIL_STATUSCONDITIONDELEGATE_HPP