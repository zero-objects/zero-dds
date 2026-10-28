// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/domain/DomainParticipantListener.hpp — DomainParticipantListener
// (DDS-PSM-Cxx 1.0 §7.7).

#ifndef ZERODDS_DDS_DOMAIN_DOMAINPARTICIPANTLISTENER_HPP
#define ZERODDS_DDS_DOMAIN_DOMAINPARTICIPANTLISTENER_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/pub/PublisherListener.hpp"
#include "dds/sub/SubscriberListener.hpp"
#include "dds/topic/AnyTopicListener.hpp"

namespace dds {
namespace domain {

class DomainParticipantListener : public virtual ::dds::pub::PublisherListener,
                                  public virtual ::dds::sub::SubscriberListener,
                                  public virtual ::dds::topic::AnyTopicListener {
public:
    virtual ~DomainParticipantListener() {}
};

class NoOpDomainParticipantListener : public virtual DomainParticipantListener,
                                      public virtual ::dds::pub::NoOpPublisherListener,
                                      public virtual ::dds::sub::NoOpSubscriberListener,
                                      public virtual ::dds::topic::NoOpAnyTopicListener {
public:
    virtual ~NoOpDomainParticipantListener() {}
};

} // namespace domain
} // namespace dds

#endif // ZERODDS_DDS_DOMAIN_DOMAINPARTICIPANTLISTENER_HPP
