// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/TGuardCondition.hpp — DDS-PSM-Cxx 1.0 §7.5 `TGuardCondition<DELEGATE>`.

#ifndef ZERODDS_DDS_CORE_COND_TGUARDCONDITION_HPP
#define ZERODDS_DDS_CORE_COND_TGUARDCONDITION_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>
#include <utility>

#include "dds/core/cond/TCondition.hpp"

namespace dds {
namespace core {
namespace cond {

namespace detail {

/// Whether `F` can be called as `f(a)` with `f` a `const F&` and `a` a `const A&`.
template <typename F, typename A>
class IsCallableWithOne {
    typedef char yes_type;
    struct no_type {
        char pad[2];
    };
    template <typename G>
    static const G& make();
    template <typename G, typename B>
    static yes_type test(char (*)[sizeof((make<G>()(make<B>()), 0))]);
    template <typename G, typename B>
    static no_type test(...);

public:
    static const bool value = sizeof(test<F, A>(0)) == sizeof(yes_type);
};

/// The function object `TGuardCondition::handler` registers: calls `func` with the guard
/// condition as argument if `func` takes one, without otherwise. Holds the delegate weakly.
template <typename Functor, typename Guard, typename DELEGATE>
class GuardHandler {
public:
    GuardHandler(const Functor& func, const ::zerodds::cxx::weak_ptr<DELEGATE>& weak)
        : func_(func), weak_(weak) {}

    void operator()() const {
        invoke(::zerodds::cxx::integral_constant<bool, IsCallableWithOne<Functor, Guard>::value>());
    }

private:
    void invoke(::zerodds::cxx::true_type) const {
        ::zerodds::cxx::shared_ptr<DELEGATE> d = weak_.lock();
        if (d.get() != 0) {
            func_(Guard(d));
        }
    }
    void invoke(::zerodds::cxx::false_type) const { func_(); }

    Functor func_;
    ::zerodds::cxx::weak_ptr<DELEGATE> weak_;
};

} // namespace detail

/// A condition the application triggers itself, over any delegate.
template <typename DELEGATE>
class TGuardCondition : public TCondition<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef TCondition<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A new guard condition, not triggered.
    TGuardCondition() : BASE_T(::zerodds::cxx::make_shared<DELEGATE>()) {}

    /// A guard condition sharing `ref`.
    explicit TGuardCondition(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

#if ZERODDS_CXX11
    TGuardCondition(const TGuardCondition&) = default;
    TGuardCondition& operator=(const TGuardCondition&) = default;
    TGuardCondition(TGuardCondition&&) = default;
    TGuardCondition& operator=(TGuardCondition&&) = default;
    ~TGuardCondition() = default;
#endif

    /// Registers `func`, which `dispatch()` calls while the condition holds: with the
    /// condition as argument if `func` takes one, without otherwise.
    template <typename Functor>
    void handler(const Functor& func) {
        ::zerodds::cxx::weak_ptr<DELEGATE> weak = this->delegate();
        this->delegate()->handler(
            detail::GuardHandler<Functor, TGuardCondition, DELEGATE>(func, weak));
    }
    void reset_handler() { this->delegate()->reset_handler(); }

    /// Sets the trigger value.
    void trigger_value(bool value) { this->delegate()->trigger_value(value); }

    /// The trigger value.
    using BASE_T::trigger_value;
};

} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_TGUARDCONDITION_HPP
