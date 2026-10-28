// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/WeakReference.hpp — DDS-PSM-Cxx 1.0 §7.5.1.1.

#ifndef ZERODDS_DDS_CORE_WEAKREFERENCE_HPP
#define ZERODDS_DDS_CORE_WEAKREFERENCE_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>

#include "dds/core/Reference.hpp"
#include "dds/core/ref_traits.hpp"

namespace dds {
namespace core {

/// Weak reference to a reference type (DDS-PSM-Cxx 1.0 §7.5.1.1).
/// Does not prevent the delegate from being destroyed.
template <typename T>
class WeakReference {
public:
    typedef T ReferenceType;
    typedef typename T::DELEGATE_WEAK_REF_T DELEGATE_WEAK_REF_T;

    /// Create a weak reference from a reference.
    explicit WeakReference(const T& ref) ZERODDS_NOEXCEPT : impl_(ref.unchecked_delegate()) {}

#if ZERODDS_CXX11
    ~WeakReference() = default;
#endif

    /// Check if the underlying reference has expired.
    bool expired() const ZERODDS_NOEXCEPT { return impl_.expired(); }

    /// Lock the weak reference, returning a strong reference (or null if expired).
    T lock() const ZERODDS_NOEXCEPT {
        typename T::DELEGATE_REF_T sp = impl_.lock();
        return T(sp);
    }

private:
    DELEGATE_WEAK_REF_T impl_;
};

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_WEAKREFERENCE_HPP