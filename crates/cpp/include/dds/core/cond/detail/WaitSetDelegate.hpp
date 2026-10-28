// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/detail/WaitSetDelegate.hpp — Delegate for WaitSet.

#ifndef ZERODDS_DDS_CORE_COND_DETAIL_WAITSETDELEGATE_HPP
#define ZERODDS_DDS_CORE_COND_DETAIL_WAITSETDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include <vector>
#include <algorithm>

#include "dds/core/Exception.hpp"
#include "dds/core/Time.hpp"
#include "dds/core/cond/TCondition.hpp"
#include "zerodds.h"

namespace dds {
namespace core {
namespace cond {
namespace detail {

/// Finds the attached condition with the native handle `handle`.
class HasConditionHandle {
public:
    explicit HasConditionHandle(void* handle) : handle_(handle) {}
    bool operator()(const ::dds::core::cond::TCondition<>& existing) const {
        void* h = existing.is_nil() ? static_cast<void*>(0) : existing->native_handle();
        return h == handle_;
    }

private:
    void* handle_;
};

/// Delegate for WaitSet (DDS-PSM-Cxx 1.0 §7.5).
class WaitSetDelegate {
public:
    WaitSetDelegate() : ws_(zerodds_waitset_create()) {}
    ~WaitSetDelegate() {
        if (ws_ != 0) {
            zerodds_waitset_destroy(ws_);
        }
    }

    // Non-copyable, non-movable (owned by shared_ptr).
    ZERODDS_NONCOPYABLE(WaitSetDelegate)
#if ZERODDS_CXX11
    WaitSetDelegate(WaitSetDelegate&&) = delete;
    WaitSetDelegate& operator=(WaitSetDelegate&&) = delete;
#endif

    zerodds_ZeroDdsWaitSet* native_handle() const { return ws_; }

    typedef ::dds::core::cond::TCondition<> Condition;
    typedef std::vector<Condition> ConditionSeq;

    template <typename D>
    void attach_condition(const ::dds::core::cond::TCondition<D>& c) {
        // The list of attached conditions is shared by every thread that uses the WaitSet
        // (DDS-PSM-Cxx 1.0 §7.3: its operations are reentrant).
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
        int rc = zerodds_waitset_attach_condition(ws_, handle_of(c));
        ::zerodds::psm::check_status(rc, "WaitSetDelegate::attach_condition");
        // Store a copy of the condition (reference semantics = shared delegate).
        if (handle_of(c)) {
            // Avoid duplicates.
            if (std::find_if(attached_.begin(), attached_.end(), HasConditionHandle(handle_of(c))) ==
                attached_.end()) {
                attached_.push_back(Condition(c));
            }
        }
    }
    template <typename D>
    bool detach_condition(const ::dds::core::cond::TCondition<D>& c) {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
        std::vector<Condition>::iterator it =
            std::find_if(attached_.begin(), attached_.end(), HasConditionHandle(handle_of(c)));
        if (it == attached_.end()) {
            return false;
        }
        int rc = zerodds_waitset_detach_condition(ws_, handle_of(c));
        ::zerodds::psm::check_status(rc, "WaitSetDelegate::detach_condition");
        attached_.erase(it);
        return true;
    }

    /// The attached conditions.
    ConditionSeq conditions() const {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
        return attached_;
    }

    ConditionSeq wait(const ::dds::core::Duration& timeout) {
        ConditionSeq triggered;
        std::vector<void*> raw_out(64, static_cast<void*>(0));
        size_t count = 0;
        int rc;
        for (;;) {
            rc = zerodds_waitset_wait(ws_, raw_out.empty() ? 0 : &raw_out[0], raw_out.size(), &count,
                                      timeout.sec(),
                                      static_cast<uint32_t>(timeout.nanosec()));
            if (rc != -8) { // ZERODDS_OUT_OF_RESOURCES
                break;
            }
            // More active conditions than the buffer holds: `count` is the
            // total, grow to it and retry.
            raw_out.resize(count);
        }
        if (rc == -4) {
            throw ::dds::core::TimeoutError("WaitSetDelegate::wait");
        }
        ::zerodds::psm::check_status(rc, "WaitSetDelegate::wait");

        // Match triggered handles to attached conditions.
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
        for (size_t i = 0; i < count; ++i) {
            void* handle = raw_out[i];
            std::vector<Condition>::iterator it =
                std::find_if(attached_.begin(), attached_.end(), HasConditionHandle(handle));
            if (it != attached_.end()) {
                triggered.push_back(*it);
            }
        }
        return triggered;
    }

private:
    template <typename D>
    static void* handle_of(const ::dds::core::cond::TCondition<D>& c) {
        return c.is_nil() ? static_cast<void*>(0) : c->native_handle();
    }

    zerodds_ZeroDdsWaitSet* ws_;
    mutable ::zerodds::cxx::mutex mutex_;
    std::vector<Condition> attached_;
};

} // namespace detail
} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_DETAIL_WAITSETDELEGATE_HPP