// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/domain/detail/DomainParticipantListenerCallbacks.hpp — the C callbacks
// that call a `DomainParticipantListener`. Included at the end of
// DomainParticipant.hpp, once `AnyTopic` is complete.

#ifndef ZERODDS_DDS_DOMAIN_DETAIL_DOMAINPARTICIPANT_LISTENER_CALLBACKS_HPP
#define ZERODDS_DDS_DOMAIN_DETAIL_DOMAINPARTICIPANT_LISTENER_CALLBACKS_HPP

#include "dds/core/detail/borrow.hpp"
#include "dds/domain/DomainParticipantListener.hpp"
#include "dds/topic/AnyTopic.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {
class Subscriber;
}
namespace domain {
namespace detail {

/// Calls `on_data_on_readers` with the subscriber behind `raw`. A template so the
/// complete `Subscriber` is needed only where it is instantiated, at the end of the
/// translation unit.
template <typename Sub>
void deliver_data_on_readers(::dds::domain::DomainParticipantListener* listener,
                             zerodds_ZeroDdsSubscriber* raw) {
    Sub sub = ::dds::core::detail::borrow<Sub>(raw);
    listener->on_data_on_readers(sub);
}

inline void dp_listener_on_inconsistent_topic(void* user_data, struct zerodds_ZeroDdsTopic* topic) {
    ::dds::domain::DomainParticipantListener* listener =
        static_cast< ::dds::domain::DomainParticipantListener*>(user_data);
    if (listener && topic) {
        // The topic the status is of, and the status it holds.
        ::dds::topic::AnyTopic td = ::dds::topic::detail::make_native_any_topic(topic);
        listener->on_inconsistent_topic(td, td.inconsistent_topic_status());
    }
}

inline void dp_listener_on_data_on_readers(void* user_data, struct zerodds_ZeroDdsSubscriber* subscriber) {
    ::dds::domain::DomainParticipantListener* listener =
        static_cast< ::dds::domain::DomainParticipantListener*>(user_data);
    if (listener && subscriber) {
        deliver_data_on_readers< ::dds::sub::Subscriber>(listener, subscriber);
    }
}

} // namespace detail
} // namespace domain
} // namespace dds

#endif // ZERODDS_DDS_DOMAIN_DETAIL_DOMAINPARTICIPANT_LISTENER_CALLBACKS_HPP
