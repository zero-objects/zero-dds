// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/TWaitSet.hpp — DDS-PSM-Cxx 1.0 §7.5 `TWaitSet<DELEGATE>`.

#ifndef ZERODDS_DDS_CORE_COND_TWAITSET_HPP
#define ZERODDS_DDS_CORE_COND_TWAITSET_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>
#include <vector>

#include "dds/core/Duration.hpp"
#include "dds/core/Reference.hpp"
#include "dds/core/cond/TCondition.hpp"

namespace dds {
namespace core {
namespace cond {

/// A set of conditions to wait on, over any delegate.
template <typename DELEGATE>
class TWaitSet : public ::dds::core::Reference<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;
    typedef TCondition<> Condition;
    typedef std::vector<Condition> ConditionSeq;

    /// A new, empty wait set.
    TWaitSet() : BASE_T(::zerodds::cxx::make_shared<DELEGATE>()) {}

    /// A wait set sharing `ref`.
    explicit TWaitSet(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

#if ZERODDS_CXX11
    TWaitSet(const TWaitSet&) = default;
    TWaitSet& operator=(const TWaitSet&) = default;
    TWaitSet(TWaitSet&&) = default;
    TWaitSet& operator=(TWaitSet&&) = default;
    ~TWaitSet() = default;
#endif

    /// Blocks until a condition holds or `timeout` passes (`TimeoutError`); returns the
    /// conditions that hold.
    const ConditionSeq wait(const ::dds::core::Duration& timeout) {
        return this->delegate()->wait(timeout);
    }
    const ConditionSeq wait() { return wait(::dds::core::Duration::infinite()); }
    ConditionSeq& wait(ConditionSeq& triggered, const ::dds::core::Duration& timeout) {
        triggered = wait(timeout);
        return triggered;
    }
    ConditionSeq& wait(ConditionSeq& triggered) {
        return wait(triggered, ::dds::core::Duration::infinite());
    }

    /// Waits, then calls `dispatch()` on each condition that holds.
    void dispatch() { dispatch(::dds::core::Duration::infinite()); }
    void dispatch(const ::dds::core::Duration& timeout) {
        ConditionSeq active = wait(timeout);
        for (typename ConditionSeq::iterator c = active.begin(); c != active.end(); ++c) {
            c->dispatch();
        }
    }

    template <typename D>
    TWaitSet& operator+=(const TCondition<D>& cond) {
        attach_condition(cond);
        return *this;
    }
    template <typename D>
    TWaitSet& operator-=(const TCondition<D>& cond) {
        detach_condition(cond);
        return *this;
    }

    /// Attaches `cond`; attaching one twice is one attachment.
    template <typename D>
    TWaitSet& attach_condition(const TCondition<D>& cond) {
        this->delegate()->attach_condition(cond);
        return *this;
    }
    /// Detaches `cond`; false when it was not attached.
    template <typename D>
    bool detach_condition(const TCondition<D>& cond) {
        return this->delegate()->detach_condition(cond);
    }

    /// The attached conditions.
    const ConditionSeq conditions() const { return this->delegate()->conditions(); }
    ConditionSeq& conditions(ConditionSeq& conds) const {
        conds = this->delegate()->conditions();
        return conds;
    }
};

} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_TWAITSET_HPP
