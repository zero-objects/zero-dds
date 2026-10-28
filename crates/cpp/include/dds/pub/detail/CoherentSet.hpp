// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/detail/CoherentSet.hpp — storage of `dds::pub::CoherentSet`.

#ifndef ZERODDS_DDS_PUB_DETAIL_COHERENTSET_HPP
#define ZERODDS_DDS_PUB_DETAIL_COHERENTSET_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/pub/Publisher.hpp"
#include "dds/pub/TCoherentSet.hpp"

namespace dds {
namespace pub {
namespace detail {

class CoherentSetDelegate {
public:
    explicit CoherentSetDelegate(const ::dds::pub::Publisher& pub) : pub_(pub), active_(true) {
        pub_->begin_coherent_changes();
    }
    ZERODDS_NONCOPYABLE(CoherentSetDelegate)
    ~CoherentSetDelegate() {
        try {
            end();
        } catch (...) {
            // A destructor does not throw; `end()` reports the failure.
        }
    }

    void end() {
        if (active_) {
            active_ = false;
            pub_->end_coherent_changes();
        }
    }

private:
    ::dds::pub::Publisher pub_;
    bool active_;
};

typedef ::dds::pub::TCoherentSet<CoherentSetDelegate> CoherentSet;

} // namespace detail
} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_DETAIL_COHERENTSET_HPP
