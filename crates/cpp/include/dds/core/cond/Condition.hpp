// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/Condition.hpp — DDS-PSM-Cxx 1.0 §7.5.10 Condition + WaitSet.

#ifndef ZERODDS_DDS_CORE_COND_CONDITION_HPP
#define ZERODDS_DDS_CORE_COND_CONDITION_HPP

#include <cstdint>
#include <vector>

#include "dds/core/Time.hpp"
#include "dds/core/Exception.hpp"
#include "zerodds.h"

namespace dds {
namespace core {
namespace cond {

/// Condition (Spec §7.5.10.1). Base type — owns a `void*` to a
/// C-FFI condition handle.
class Condition {
public:
    Condition() = default;
    explicit Condition(void* handle) : handle_(handle) {}
    Condition(const Condition&) = delete;
    Condition& operator=(const Condition&) = delete;
    Condition(Condition&& o) noexcept : handle_(o.handle_) { o.handle_ = nullptr; }
    Condition& operator=(Condition&& o) noexcept {
        if (this != &o) {
            handle_ = o.handle_;
            o.handle_ = nullptr;
        }
        return *this;
    }
    virtual ~Condition() = default;
    /// Trigger-Wert (Spec §7.5.10.1.1).
    bool trigger_value() const {
        return handle_ != nullptr && zerodds_condition_get_trigger_value(handle_);
    }
    /// Internal handle (used by WaitSet).
    void* native_handle() const { return handle_; }
protected:
    void* handle_{nullptr};
};

/// GuardCondition (Spec §7.5.10.5).
class GuardCondition : public Condition {
public:
    GuardCondition() : Condition(zerodds_guardcondition_create()) {}
    ~GuardCondition() override {
        if (handle_ != nullptr) {
            zerodds_guardcondition_destroy(static_cast<zerodds_ZeroDdsGuardCondition*>(handle_));
            handle_ = nullptr;
        }
    }
    /// Sets the trigger value.
    void trigger_value(bool v) {
        int rc = zerodds_guardcondition_set_trigger_value(
            static_cast<zerodds_ZeroDdsGuardCondition*>(handle_), v);
        ::dds::core::check_status(rc, "GuardCondition::trigger_value");
    }
    using Condition::trigger_value;
};

/// StatusCondition (Spec §7.5.10.4). Bound to an entity.
class StatusCondition : public Condition {
public:
    explicit StatusCondition(void* entity_handle)
        : Condition(zerodds_entity_get_statuscondition(entity_handle)) {}
    ~StatusCondition() override {
        if (handle_ != nullptr) {
            zerodds_statuscondition_destroy(
                static_cast<zerodds_ZeroDdsStatusCondition*>(handle_));
            handle_ = nullptr;
        }
    }
    /// Set enabled-statuses mask.
    void enabled_statuses(uint32_t mask) {
        int rc = zerodds_statuscondition_set_enabled_statuses(
            static_cast<zerodds_ZeroDdsStatusCondition*>(handle_), mask);
        ::dds::core::check_status(rc, "StatusCondition::enabled_statuses");
    }
    /// Get enabled-statuses mask.
    uint32_t enabled_statuses() const {
        return zerodds_statuscondition_get_enabled_statuses(
            static_cast<zerodds_ZeroDdsStatusCondition*>(handle_));
    }
};

/// WaitSet (Spec §7.5.10.2).
class WaitSet {
public:
    WaitSet() : ws_(zerodds_waitset_create()) {}
    WaitSet(const WaitSet&) = delete;
    WaitSet& operator=(const WaitSet&) = delete;
    WaitSet(WaitSet&& o) noexcept : ws_(o.ws_) { o.ws_ = nullptr; }
    WaitSet& operator=(WaitSet&& o) noexcept {
        if (this != &o) {
            ws_ = o.ws_;
            o.ws_ = nullptr;
        }
        return *this;
    }
    ~WaitSet() {
        if (ws_ != nullptr) {
            zerodds_waitset_destroy(ws_);
        }
    }

    /// Attach Condition.
    WaitSet& operator+=(const Condition& c) {
        attach_condition(c);
        return *this;
    }
    /// Attach.
    void attach_condition(const Condition& c) {
        int rc = zerodds_waitset_attach_condition(ws_, c.native_handle());
        ::dds::core::check_status(rc, "WaitSet::attach_condition");
    }
    /// Detach.
    void detach_condition(const Condition& c) {
        int rc = zerodds_waitset_detach_condition(ws_, c.native_handle());
        ::dds::core::check_status(rc, "WaitSet::detach_condition");
    }
    /// Wait, returns active condition handles (raw `void*` per Spec §7.5.10.2.1).
    std::vector<void*> wait(const Duration& timeout) {
        std::vector<void*> out(64, nullptr);
        size_t count = 0;
        int rc = zerodds_waitset_wait(ws_, out.data(), out.size(), &count,
                                      timeout.sec(),
                                      static_cast<uint32_t>(timeout.nanosec()));
        if (rc == -4) {
            throw ::dds::core::TimeoutError("WaitSet::wait");
        }
        ::dds::core::check_status(rc, "WaitSet::wait");
        out.resize(count);
        return out;
    }

private:
    zerodds_ZeroDdsWaitSet* ws_{nullptr};
};

} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_CONDITION_HPP
