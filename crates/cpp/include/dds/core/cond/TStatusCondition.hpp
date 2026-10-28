// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/TStatusCondition.hpp — DDS-PSM-Cxx 1.0 §7.5 `TStatusCondition<DELEGATE>`.

#ifndef ZERODDS_DDS_CORE_COND_TSTATUSCONDITION_HPP
#define ZERODDS_DDS_CORE_COND_TSTATUSCONDITION_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>
#include <utility>

#include "dds/core/Entity.hpp"
#include "dds/core/cond/TCondition.hpp"
#include "dds/core/detail/EntityAdapter.hpp"
#include "dds/core/status/State.hpp"

namespace dds {
namespace core {
namespace cond {

namespace detail {

/// Whether `E` is an entity reference: `entity->native_handle()` is a valid expression.
template <typename E>
class HasNativeHandle {
    typedef char yes_type;
    struct no_type {
        char pad[2];
    };
    template <typename G>
    static const G& make();
    template <typename G>
    static yes_type test(char (*)[sizeof((make<G>()->native_handle(), 0))]);
    template <typename G>
    static no_type test(...);

public:
    static const bool value = sizeof(test<E>(0)) == sizeof(yes_type);
};

} // namespace detail

/// A condition on the statuses of an entity, over any delegate.
template <typename DELEGATE>
class TStatusCondition : public TCondition<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef TCondition<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A null status condition.
    TStatusCondition() ZERODDS_NOEXCEPT {}

    /// A status condition sharing `ref`.
    explicit TStatusCondition(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// The status condition of `entity`, any entity reference type of this library.
    template <typename E>
    explicit TStatusCondition(
        const E& entity,
        typename ::zerodds::cxx::enable_if<detail::HasNativeHandle<E>::value>::type* = 0)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(static_cast<void*>(entity->native_handle()),
                                            ::dds::core::detail::as_entity(entity))) {}

#if ZERODDS_CXX11
    TStatusCondition(const TStatusCondition&) = default;
    TStatusCondition& operator=(const TStatusCondition&) = default;
    TStatusCondition(TStatusCondition&&) = default;
    TStatusCondition& operator=(TStatusCondition&&) = default;
    ~TStatusCondition() = default;
#endif

    /// The statuses the condition reacts to.
    void enabled_statuses(const ::dds::core::status::StatusMask& status) const {
        this->delegate()->enabled_statuses(static_cast<uint32_t>(status.to_ulong()));
    }
    const ::dds::core::status::StatusMask enabled_statuses() const {
        return ::dds::core::status::StatusMask(this->delegate()->enabled_statuses());
    }

    /// The entity the condition is bound to.
    const ::dds::core::Entity& entity() const { return this->delegate()->entity(); }
};

} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_TSTATUSCONDITION_HPP
