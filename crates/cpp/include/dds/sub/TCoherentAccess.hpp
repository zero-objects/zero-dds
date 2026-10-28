// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/TCoherentAccess.hpp — DDS-PSM-Cxx 1.0 `TCoherentAccess<D>`.

#ifndef ZERODDS_DDS_SUB_TCOHERENTACCESS_HPP
#define ZERODDS_DDS_SUB_TCOHERENTACCESS_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/Value.hpp"
#include "dds/sub/Subscriber.hpp"

namespace dds {
namespace sub {

/// Coherent access to the readers of a subscriber: begins in the
/// constructor and ends in `end()` or, at the latest, the destructor.
template <typename DELEGATE>
class TCoherentAccess : public ::dds::core::Value<DELEGATE> {
public:
    explicit TCoherentAccess(const ::dds::sub::Subscriber& sub) : ::dds::core::Value<DELEGATE>(sub) {}

    ZERODDS_NONCOPYABLE(TCoherentAccess)

    /// Ends the access explicitly.
    void end() { this->delegate().end(); }

#if ZERODDS_CXX11
    ~TCoherentAccess() = default;
#endif
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_TCOHERENTACCESS_HPP
