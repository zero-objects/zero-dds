// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/detail/CoherentAccess.hpp — storage of `dds::sub::CoherentAccess`.

#ifndef ZERODDS_DDS_SUB_DETAIL_COHERENTACCESS_HPP
#define ZERODDS_DDS_SUB_DETAIL_COHERENTACCESS_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/sub/Subscriber.hpp"
#include "dds/sub/TCoherentAccess.hpp"

namespace dds {
namespace sub {
namespace detail {

class CoherentAccessDelegate {
public:
    explicit CoherentAccessDelegate(const ::dds::sub::Subscriber& sub) : sub_(sub), active_(true) {
        sub_->begin_access();
    }
    ZERODDS_NONCOPYABLE(CoherentAccessDelegate)
    ~CoherentAccessDelegate() {
        try {
            end();
        } catch (...) {
            // A destructor does not throw; `end()` reports the failure.
        }
    }

    void end() {
        if (active_) {
            active_ = false;
            sub_->end_access();
        }
    }

private:
    ::dds::sub::Subscriber sub_;
    bool active_;
};

typedef ::dds::sub::TCoherentAccess<CoherentAccessDelegate> CoherentAccess;

} // namespace detail
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DETAIL_COHERENTACCESS_HPP
