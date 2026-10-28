// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/detail/borrow.hpp — wraps a delegate in the reference type it
// belongs to. Internal: the listener bridges and the Any holders make the
// entity a C handle names without a constructor of the public class.

#ifndef ZERODDS_DDS_CORE_DETAIL_BORROW_HPP
#define ZERODDS_DDS_CORE_DETAIL_BORROW_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>
#include <utility>

namespace dds {
namespace core {
namespace detail {

/// A `W` (a reference type with `DELEGATE_T` and a protected `impl_`)
/// holding `ref`.
template <typename W>
struct DelegateWrapper : W {
    explicit DelegateWrapper(typename W::DELEGATE_REF_T ref) : W() { this->impl_ = ZERODDS_MOVE(ref); }
};

template <typename W>
W wrap_delegate(typename W::DELEGATE_REF_T ref) {
    return DelegateWrapper<W>(ZERODDS_MOVE(ref));
}

/// A `W` for the entity behind the C handle `raw`, which the caller keeps
/// owning: the delegate does not delete it.
template <typename W, typename Raw>
W borrow(Raw* raw) {
    return wrap_delegate<W>(::zerodds::cxx::make_shared<typename W::DELEGATE_T>(raw));
}

} // namespace detail
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_DETAIL_BORROW_HPP
