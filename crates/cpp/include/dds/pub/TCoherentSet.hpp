// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/TCoherentSet.hpp — DDS-PSM-Cxx 1.0 `TCoherentSet<D>`.

#ifndef ZERODDS_DDS_PUB_TCOHERENTSET_HPP
#define ZERODDS_DDS_PUB_TCOHERENTSET_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/Value.hpp"
#include "dds/pub/Publisher.hpp"

namespace dds {
namespace pub {

/// Groups the writes of a publisher into one coherent change set: the set
/// begins in the constructor and ends in `end()` or, at the latest, in the
/// destructor.
template <typename DELEGATE>
class TCoherentSet : public ::dds::core::Value<DELEGATE> {
public:
    explicit TCoherentSet(const ::dds::pub::Publisher& pub) : ::dds::core::Value<DELEGATE>(pub) {}

    ZERODDS_NONCOPYABLE(TCoherentSet)

    /// Ends the set.
    void end() { this->delegate().end(); }

#if ZERODDS_CXX11
    ~TCoherentSet() = default;
#endif
};

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_TCOHERENTSET_HPP
