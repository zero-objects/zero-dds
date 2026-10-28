// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/Reference.hpp — DDS-PSM-Cxx 1.0 §7.5.1.1.

#ifndef ZERODDS_DDS_CORE_REFERENCE_HPP
#define ZERODDS_DDS_CORE_REFERENCE_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>

#include "dds/core/types.hpp"
#include "dds/core/ref_traits.hpp"
#include "dds/core/Exception.hpp"

namespace dds {
namespace core {

/// Base class for all reference types (DDS-PSM-Cxx 1.0 §7.5.1.1).
/// A Reference never points to an invalid object; it is either null or
/// holds a shared_ptr to a valid delegate.
template <typename DELEGATE>
class Reference {
public:
    typedef DELEGATE DELEGATE_T;
    typedef typename smart_ptr_traits<DELEGATE>::ref_type DELEGATE_REF_T;
    typedef typename smart_ptr_traits<DELEGATE>::weak_ref_type DELEGATE_WEAK_REF_T;

    /// Creates a null reference.
    explicit Reference(const null_type&) ZERODDS_NOEXCEPT : impl_(nullptr) {}


    /// Copy constructor shares the delegate (reference semantics).
    Reference(const Reference& other) ZERODDS_NOEXCEPT : impl_(other.impl_) {}

    /// Cross-type copy constructor (requires delegate compatibility).
    template <typename D>
    Reference(const Reference<D>& other) ZERODDS_NOEXCEPT
        : impl_(other.unchecked_delegate()) {}

    /// Construct from a raw delegate pointer (takes ownership).
    explicit Reference(DELEGATE* p) ZERODDS_NOEXCEPT : impl_(p) {}

    /// Construct from a shared_ptr to delegate.
    explicit Reference(const DELEGATE_REF_T& ref) ZERODDS_NOEXCEPT : impl_(ref) {}

#if ZERODDS_CXX11
    ~Reference() = default;
#endif

    /// Implicit conversion to delegate reference.
    operator DELEGATE_REF_T() const ZERODDS_NOEXCEPT { return impl_; }

    /// Equality based on referential equality of the delegate.
    template <typename R>
    bool operator==(const R& other) const ZERODDS_NOEXCEPT {
        return impl_ == other.impl_;
    }

    template <typename R>
    bool operator!=(const R& other) const ZERODDS_NOEXCEPT {
        return impl_ != other.impl_;
    }

    /// Assignment from another reference type.
    template <typename D>
    Reference& operator=(const Reference<D>& other) ZERODDS_NOEXCEPT {
        impl_ = other.impl_;
        return *this;
    }

    /// Assignment from null.
    Reference& operator=(const null_type&) ZERODDS_NOEXCEPT {
        impl_.reset();
        return *this;
    }

    /// Check if this reference is null.
    bool is_nil() const ZERODDS_NOEXCEPT { return !impl_; }

    /// Comparison with null.
    bool operator==(const null_type&) const ZERODDS_NOEXCEPT { return is_nil(); }
    bool operator!=(const null_type&) const ZERODDS_NOEXCEPT { return !is_nil(); }

    /// Access the underlying delegate (const). A null reference has none: `NullReferenceError`,
    /// so that no call goes through a null pointer (DDS-PSM-Cxx 1.0 §7.5.1.1: references are safe).
    const DELEGATE_REF_T& delegate() const {
        require_delegate();
        return impl_;
    }

    /// Access the underlying delegate (non-const); `NullReferenceError` for a null reference.
    DELEGATE_REF_T& delegate() {
        require_delegate();
        return impl_;
    }

    /// The delegate reference as it is, null or not; for code that has to look at a null reference.
    const DELEGATE_REF_T& unchecked_delegate() const ZERODDS_NOEXCEPT { return impl_; }

    /// Direct delegate method access; `NullReferenceError` for a null reference.
    DELEGATE* operator->() {
        require_delegate();
        return impl_.get();
    }
    const DELEGATE* operator->() const {
        require_delegate();
        return impl_.get();
    }

    /// Implicit conversion to delegate reference.
    operator DELEGATE_REF_T& () ZERODDS_NOEXCEPT { return impl_; }
    operator const DELEGATE_REF_T& () const ZERODDS_NOEXCEPT { return impl_; }

protected:
    /// A null reference: a reference type is made through its factory constructors or from
    /// `dds::core::null` (DDS-PSM-Cxx 1.0 §7.5.1.2), not default-constructed.
    Reference() ZERODDS_NOEXCEPT : impl_(nullptr) {}

    void require_delegate() const {
        if (!impl_) {
            throw NullReferenceError("a call through a null reference");
        }
    }

    DELEGATE_REF_T impl_;
};

/// Reverse comparison with null.
template <typename D>
bool operator==(const null_type&, const Reference<D>& r) ZERODDS_NOEXCEPT {
    return r.is_nil();
}

template <typename D>
bool operator!=(const null_type&, const Reference<D>& r) ZERODDS_NOEXCEPT {
    return !r.is_nil();
}

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_REFERENCE_HPP