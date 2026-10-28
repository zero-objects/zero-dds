// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/Publisher.hpp — Publisher (DDS-PSM-Cxx 1.0 §7.9).

#ifndef ZERODDS_DDS_PUB_PUBLISHER_HPP
#define ZERODDS_DDS_PUB_PUBLISHER_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <vector>

#include "dds/core/detail/borrow.hpp"

#include "dds/core/Reference.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/Time.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/core/TEntity.hpp"
#include "dds/domain/DomainParticipant.hpp"
#include "dds/pub/PublisherListener.hpp"
#include "dds/pub/TPublisher.hpp"
#include "dds/pub/detail/PublisherDelegate.hpp"
#include "dds/topic/Topic.hpp"
#include "zerodds.h"

namespace dds {
namespace pub {

/// Publisher (Spec §7.9). Reference type with delegate pattern
/// (DDS-PSM-Cxx 1.0 §7.5.1.1, §7.5.1.2): `TPublisher` over ZeroDDS's delegate.
class Publisher : public TPublisher<detail::PublisherDelegate> {
public:
    typedef TPublisher<detail::PublisherDelegate> BASE_T;

    /// A publisher over a reference to its delegate.
    explicit Publisher(const BASE_T::DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A publisher of `dp` with the default QoS.
    explicit Publisher(const ::dds::domain::DomainParticipant& dp) : BASE_T(dp) {}

    /// With QoS, and optionally a listener.
    Publisher(const ::dds::domain::DomainParticipant& dp, const ::dds::pub::qos::PublisherQos& qos,
              ::dds::pub::PublisherListener* listener = 0,
              const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(dp, qos, listener, mask) {}

    /// A null publisher.
protected:
    Publisher() ZERODDS_NOEXCEPT {}
public:

    /// The null Publisher (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    Publisher(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    Publisher(const Publisher&) = default;
    Publisher& operator=(const Publisher&) = default;
    Publisher(Publisher&&) = default;
    Publisher& operator=(Publisher&&) = default;
    ~Publisher() = default;
#endif
};

} // namespace pub
} // namespace dds

// The writers, their listeners and AnyDataWriter need the complete Publisher.
#include "dds/pub/DataWriter.hpp"
#include "dds/pub/detail/PublisherListenerBridge.hpp"

namespace dds {
namespace pub {

namespace detail {
/// Closes a publisher a participant tracks: the writers it contains first, then the publisher.
struct ClosePublisher {
    void operator()(const ::zerodds::cxx::shared_ptr<void>& child) const {
        const ::zerodds::cxx::shared_ptr<PublisherDelegate> publisher =
            ::zerodds::cxx::static_pointer_cast<PublisherDelegate>(child);
        publisher->delete_contained_entities();
        publisher->close();
    }
};

inline void PublisherDelegate::delete_contained_entities() {
    typedef ::dds::core::detail::EntityTracker< ::dds::pub::AnyDataWriter> Tracker;
    const std::vector<Tracker::Live> lives = writers_.live();
    for (std::vector<Tracker::Live>::const_iterator it = lives.begin(); it != lives.end(); ++it) {
        it->entry.to_any(it->delegate).close();
    }
}

inline void PublisherDelegate::enrol(const ::dds::domain::DomainParticipant& dp,
                                     const ::zerodds::cxx::shared_ptr<PublisherDelegate>& self) {
    dp->child_publishers().add(self, ClosePublisher());
}

inline void PublisherListenerBridge::attach(PublisherDelegate& pub, PublisherListener* listener,
                                            uint32_t mask) {
    zerodds_ZeroDdsPublisherListener vt = zerodds_ZeroDdsPublisherListener();
    vt.user_data = static_cast<void*>(listener);
    vt.on_offered_deadline_missed = &cb_offered_deadline_missed;
    vt.on_liveliness_lost = &cb_liveliness_lost;
    vt.on_offered_incompatible_qos = &cb_offered_incompatible_qos;
    vt.on_publication_matched = &cb_publication_matched;
    pub.attach_listener(vt, mask);
}

inline void PublisherDelegate::listener(PublisherListener* l,
                                        const ::dds::core::status::StatusMask& mask) {
    set_listener(l);
    if (l == 0) {
        detach_listener();
        return;
    }
    PublisherListenerBridge::attach(*this, l, static_cast<uint32_t>(mask.to_ulong()));
}
} // namespace detail

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_PUBLISHER_HPP
