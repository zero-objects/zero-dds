// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/domain/TDomainParticipant.hpp — DDS-PSM-Cxx 1.0 §7.7
// `TDomainParticipant<DELEGATE>`.

#ifndef ZERODDS_DDS_DOMAIN_TDOMAINPARTICIPANT_HPP
#define ZERODDS_DDS_DOMAIN_TDOMAINPARTICIPANT_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <memory>

#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/TEntity.hpp"
#include "dds/core/Time.hpp"
#include "dds/core/status/State.hpp"
#include "dds/domain/DomainParticipantListener.hpp"
#include "dds/domain/qos/DomainParticipantQos.hpp"
#include "dds/pub/qos/PublisherQos.hpp"
#include "dds/sub/qos/SubscriberQos.hpp"
#include "dds/topic/qos/TopicQos.hpp"

namespace dds {
namespace domain {

/// A reference to a domain participant whose behaviour a `DELEGATE` provides.
template <typename DELEGATE>
class TDomainParticipant : public ::dds::core::TEntity<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef ::dds::core::TEntity<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;
    typedef ::dds::domain::DomainParticipantListener Listener;

    /// A null participant.
protected:
    TDomainParticipant() ZERODDS_NOEXCEPT {}
public:

    /// A participant sharing `ref`.
    explicit TDomainParticipant(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A participant in domain `did` with the default QoS.
    explicit TDomainParticipant(uint32_t did) : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(did)) {}

    /// With QoS, and optionally a listener.
    TDomainParticipant(uint32_t id, const ::dds::domain::qos::DomainParticipantQos& qos,
                       ::dds::domain::DomainParticipantListener* listener = 0,
                       const ::dds::core::status::StatusMask& mask =
                           ::dds::core::status::StatusMask::all())
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(id, qos)) {
        if (listener != 0) {
            this->listener(listener, mask);
        }
    }

#if ZERODDS_CXX11
    TDomainParticipant(const TDomainParticipant&) = default;
    TDomainParticipant& operator=(const TDomainParticipant&) = default;
    TDomainParticipant(TDomainParticipant&&) = default;
    TDomainParticipant& operator=(TDomainParticipant&&) = default;
    ~TDomainParticipant() = default;
#endif

    void listener(Listener* the_listener, const ::dds::core::status::StatusMask& event_mask) {
        this->delegate()->listener(the_listener, event_mask);
        ::dds::core::detail::retain_for_listener(this->delegate(), the_listener != 0);
    }
    Listener* listener() const { return this->delegate()->listener(); }

    const ::dds::domain::qos::DomainParticipantQos& qos() const { return this->delegate()->qos(); }
    void qos(const ::dds::domain::qos::DomainParticipantQos& the_qos) {
        this->delegate()->qos(the_qos);
    }

    uint32_t domain_id() const { return this->delegate()->domain_id(); }

    /// Asserts liveliness for all MANUAL_BY_PARTICIPANT writers.
    void assert_liveliness() { this->delegate()->assert_liveliness(); }

    bool contains_entity(const ::dds::core::InstanceHandle& handle) const {
        return this->delegate()->contains_entity(handle);
    }

    ::dds::core::Time current_time() const { return this->delegate()->current_time(); }

    /// The QoS participants get when none is given (the factory's default).
    static ::dds::domain::qos::DomainParticipantQos default_participant_qos() {
        return DELEGATE::factory_default_participant_qos();
    }
    static void default_participant_qos(const ::dds::domain::qos::DomainParticipantQos& qos) {
        DELEGATE::set_factory_default_participant_qos(qos);
    }

    /// The QoS the publishers, subscribers and topics of this participant get when none is given.
    ::dds::pub::qos::PublisherQos default_publisher_qos() const {
        return this->delegate()->default_publisher_qos();
    }
    TDomainParticipant& default_publisher_qos(const ::dds::pub::qos::PublisherQos& qos) {
        this->delegate()->default_publisher_qos(qos);
        return *this;
    }
    ::dds::sub::qos::SubscriberQos default_subscriber_qos() const {
        return this->delegate()->default_subscriber_qos();
    }
    TDomainParticipant& default_subscriber_qos(const ::dds::sub::qos::SubscriberQos& qos) {
        this->delegate()->default_subscriber_qos(qos);
        return *this;
    }
    ::dds::topic::qos::TopicQos default_topic_qos() const {
        return this->delegate()->default_topic_qos();
    }
    TDomainParticipant& default_topic_qos(const ::dds::topic::qos::TopicQos& qos) {
        this->delegate()->default_topic_qos(qos);
        return *this;
    }
};

} // namespace domain
} // namespace dds

#endif // ZERODDS_DDS_DOMAIN_TDOMAINPARTICIPANT_HPP
