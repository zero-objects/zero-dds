// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/Subscriber.hpp — Subscriber (DDS-PSM-Cxx 1.0 §7.10).

#ifndef ZERODDS_DDS_SUB_SUBSCRIBER_HPP
#define ZERODDS_DDS_SUB_SUBSCRIBER_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "dds/core/detail/borrow.hpp"
#include "dds/core/Reference.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/core/TEntity.hpp"
#include "dds/domain/DomainParticipant.hpp"
#include "dds/sub/SubscriberListener.hpp"
#include "dds/sub/TSubscriber.hpp"
#include "dds/sub/detail/SubscriberDelegate.hpp"
#include "dds/topic/Topic.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {
namespace detail {
class CoherentAccessDelegate;
}

/// Subscriber (Spec §7.10). Reference type with delegate pattern
/// (DDS-PSM-Cxx 1.0 §7.5.1.1, §7.5.1.2): `TSubscriber` over ZeroDDS's delegate.
class Subscriber : public TSubscriber<detail::SubscriberDelegate> {
public:
    typedef TSubscriber<detail::SubscriberDelegate> BASE_T;

    /// A subscriber sharing `ref`.
    explicit Subscriber(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A subscriber of `dp` with the default QoS.
    explicit Subscriber(const ::dds::domain::DomainParticipant& dp) : BASE_T(dp) {}

    /// With QoS, and optionally a listener.
    Subscriber(const ::dds::domain::DomainParticipant& dp, const ::dds::sub::qos::SubscriberQos& qos,
               ::dds::sub::SubscriberListener* listener = nullptr,
               const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(dp, qos, listener, mask) {}

    /// A null subscriber.
protected:
    Subscriber() ZERODDS_NOEXCEPT {}
public:

    /// The null Subscriber (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    Subscriber(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    Subscriber(const Subscriber&) = default;
    Subscriber& operator=(const Subscriber&) = default;
    Subscriber(Subscriber&&) = default;
    Subscriber& operator=(Subscriber&&) = default;
    ~Subscriber() = default;
#endif

private:
    friend class ::dds::sub::detail::CoherentAccessDelegate;
};

} // namespace sub
} // namespace dds

// The readers, their listeners and AnyDataReader need the complete Subscriber.
#include "dds/sub/DataReader.hpp"
#include "dds/sub/detail/SubscriberListenerBridge.hpp"

namespace dds {
namespace sub {

namespace detail {
inline void SubscriberDelegate::delete_contained_entities() {
    typedef ::dds::core::detail::EntityTracker< ::dds::sub::AnyDataReader> Tracker;
    const std::vector<Tracker::Live> lives = readers_.live();
    for (std::vector<Tracker::Live>::const_iterator live = lives.begin(); live != lives.end();
         ++live) {
        live->entry.to_any(live->delegate)->close();
    }
}

/// Closes a subscriber the participant tracks, with the readers it still holds.
struct SubscriberChildCloser {
    void operator()(const ::zerodds::cxx::shared_ptr<void>& child) const {
        const ::zerodds::cxx::shared_ptr<SubscriberDelegate> subscriber =
            ::zerodds::cxx::static_pointer_cast<SubscriberDelegate>(child);
        subscriber->delete_contained_entities();
        subscriber->close();
    }
};

inline void SubscriberDelegate::enrol(const ::dds::domain::DomainParticipant& dp,
                                      const ::zerodds::cxx::shared_ptr<SubscriberDelegate>& self) {
    dp->child_subscribers().add(self, SubscriberChildCloser());
}

inline void SubscriberListenerBridge::cb_data_on_readers(void* ud,
                                                         zerodds_ZeroDdsSubscriber* raw) {
    if (SubscriberListener* l = static_cast<SubscriberListener*>(ud)) {
        Subscriber s = ::dds::core::detail::borrow<Subscriber>(raw);
        l->on_data_on_readers(s);
    }
}

inline void SubscriberListenerBridge::attach(SubscriberDelegate& sub, SubscriberListener* listener,
                                             uint32_t mask) {
    zerodds_ZeroDdsSubscriberListener vt = zerodds_ZeroDdsSubscriberListener();
    vt.user_data = static_cast<void*>(listener);
    vt.on_data_on_readers = &cb_data_on_readers;
    vt.on_sample_lost = &cb_sample_lost;
    vt.on_sample_rejected = &cb_sample_rejected;
    vt.on_liveliness_changed = &cb_liveliness_changed;
    vt.on_subscription_matched = &cb_subscription_matched;
    vt.on_requested_deadline_missed = &cb_requested_deadline_missed;
    vt.on_requested_incompatible_qos = &cb_requested_incompatible_qos;
    vt.on_data_available = &cb_data_available;
    sub.attach_listener(vt, mask);
}
} // namespace detail

inline void detail::SubscriberDelegate::listener(SubscriberListener* l,
                                                 const ::dds::core::status::StatusMask& mask) {
    set_listener(l);
    if (l == 0) {
        detach_listener();
        return;
    }
    detail::SubscriberListenerBridge::attach(*this, l, static_cast<uint32_t>(mask.to_ulong()));
}

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_SUBSCRIBER_HPP
