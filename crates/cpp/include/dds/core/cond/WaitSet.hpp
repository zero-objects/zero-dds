// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/WaitSet.hpp — WaitSet (DDS-PSM-Cxx 1.0 §7.5).

#ifndef ZERODDS_DDS_CORE_COND_WAITSET_HPP
#define ZERODDS_DDS_CORE_COND_WAITSET_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/cond/TWaitSet.hpp"
#include "dds/core/cond/detail/WaitSetDelegate.hpp"

namespace dds {
namespace core {
namespace cond {

/// WaitSet (Spec §7.5). Reference type with delegate pattern: `TWaitSet` over
/// ZeroDDS's delegate.
class WaitSet : public TWaitSet<detail::WaitSetDelegate> {
public:
    typedef TWaitSet<detail::WaitSetDelegate> BASE_T;
    typedef BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A wait set sharing `ref`.
    explicit WaitSet(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A new, empty wait set.
    WaitSet() {}

    /// The null WaitSet (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    WaitSet(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    WaitSet(const WaitSet&) = default;
    WaitSet& operator=(const WaitSet&) = default;
    WaitSet(WaitSet&&) = default;
    WaitSet& operator=(WaitSet&&) = default;
    ~WaitSet() = default;
#endif
};

} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_WAITSET_HPP
