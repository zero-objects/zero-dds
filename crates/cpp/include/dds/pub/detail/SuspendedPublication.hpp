// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/detail/SuspendedPublication.hpp — storage of `dds::pub::SuspendedPublication`.

#ifndef ZERODDS_DDS_PUB_DETAIL_SUSPENDEDPUBLICATION_HPP
#define ZERODDS_DDS_PUB_DETAIL_SUSPENDEDPUBLICATION_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/pub/Publisher.hpp"
#include "dds/pub/TSuspendedPublication.hpp"

namespace dds {
namespace pub {
namespace detail {

class SuspendedPublicationDelegate {
public:
    explicit SuspendedPublicationDelegate(const ::dds::pub::Publisher& pub)
        : pub_(pub), active_(true) {
        pub_->suspend_publications();
    }
    ZERODDS_NONCOPYABLE(SuspendedPublicationDelegate)
    ~SuspendedPublicationDelegate() {
        try {
            resume();
        } catch (...) {
            // A destructor does not throw; `resume()` reports the failure.
        }
    }

    void resume() {
        if (active_) {
            active_ = false;
            pub_->resume_publications();
        }
    }

private:
    ::dds::pub::Publisher pub_;
    bool active_;
};

typedef ::dds::pub::TSuspendedPublication<SuspendedPublicationDelegate> SuspendedPublication;

} // namespace detail
} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_DETAIL_SUSPENDEDPUBLICATION_HPP
