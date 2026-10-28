// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/TEntity.hpp — DDS-PSM-Cxx 1.0 §7.5.1.2.

#ifndef ZERODDS_DDS_CORE_TENTITY_HPP
#define ZERODDS_DDS_CORE_TENTITY_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/Reference.hpp"
#include "dds/core/detail/Retain.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/status/Status.hpp"

namespace dds {
namespace core {

template <typename DELEGATE>
class TEntity : public Reference<DELEGATE> {
public:
    typedef Reference<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    TEntity() : BASE_T(null) {}
    explicit TEntity(const null_type& n) ZERODDS_NOEXCEPT : BASE_T(n) {}
    template <typename D>
    TEntity(const Reference<D>& other) ZERODDS_NOEXCEPT : BASE_T(other) {}
    explicit TEntity(DELEGATE* p) ZERODDS_NOEXCEPT : BASE_T(p) {}
    explicit TEntity(const DELEGATE_REF_T& ref) ZERODDS_NOEXCEPT : BASE_T(ref) {}

#if ZERODDS_CXX11
    ~TEntity() = default;
#endif

    /// Enable the entity.
    void enable() { this->delegate()->enable(); }

    /// Get triggered status changes.
    status::StatusMask status_changes() const { return this->delegate()->status_changes(); }

    /// Get the instance handle of this entity.
    InstanceHandle instance_handle() const { return this->delegate()->instance_handle(); }

    /// Close the entity and release resources.
    void close() {
        // Ends the retention first: the delegate is destroyed with the last reference.
        const DELEGATE_REF_T keep = this->delegate();
        detail::release_entity(keep);
        keep->close();
    }

    /// Retain the entity (prevent automatic cleanup).
    void retain() {
        this->delegate()->retain();
        detail::retain_entity(this->delegate());
    }
};

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_TENTITY_HPP