// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/TQosProvider.hpp — DDS-PSM-Cxx 1.0 §7.6.2.1 `TQosProvider<DELEGATE>`.

#ifndef ZERODDS_DDS_CORE_TQOSPROVIDER_HPP
#define ZERODDS_DDS_CORE_TQOSPROVIDER_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>
#include <string>

#include "dds/core/Reference.hpp"
#include "dds/domain/qos/DomainParticipantQos.hpp"
#include "dds/pub/qos/DataWriterQos.hpp"
#include "dds/pub/qos/PublisherQos.hpp"
#include "dds/sub/qos/DataReaderQos.hpp"
#include "dds/sub/qos/SubscriberQos.hpp"
#include "dds/topic/qos/TopicQos.hpp"

namespace dds {
namespace core {

/// Loads QoS from a URI, over any delegate.
template <typename DELEGATE>
class TQosProvider : public Reference<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef Reference<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A provider sharing `ref`.
    explicit TQosProvider(const DELEGATE_REF_T& ref) : BASE_T(ref) {}
    explicit TQosProvider(const std::string& uri, const std::string& profile)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(uri, profile)) {}
    explicit TQosProvider(const std::string& uri)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(uri, std::string())) {}

    ::dds::domain::qos::DomainParticipantQos participant_qos() { return participant_qos(""); }
    ::dds::domain::qos::DomainParticipantQos participant_qos(const std::string& id) {
        return this->delegate()->participant_qos(id);
    }

    ::dds::topic::qos::TopicQos topic_qos() { return topic_qos(""); }
    ::dds::topic::qos::TopicQos topic_qos(const std::string& id) {
        return this->delegate()->topic_qos(id);
    }

    ::dds::sub::qos::SubscriberQos subscriber_qos() { return subscriber_qos(""); }
    ::dds::sub::qos::SubscriberQos subscriber_qos(const std::string& id) {
        return this->delegate()->subscriber_qos(id);
    }

    ::dds::sub::qos::DataReaderQos datareader_qos() { return datareader_qos(""); }
    ::dds::sub::qos::DataReaderQos datareader_qos(const std::string& id) {
        return this->delegate()->datareader_qos(id);
    }

    ::dds::pub::qos::PublisherQos publisher_qos() { return publisher_qos(""); }
    ::dds::pub::qos::PublisherQos publisher_qos(const std::string& id) {
        return this->delegate()->publisher_qos(id);
    }

    ::dds::pub::qos::DataWriterQos datawriter_qos() { return datawriter_qos(""); }
    ::dds::pub::qos::DataWriterQos datawriter_qos(const std::string& id) {
        return this->delegate()->datawriter_qos(id);
    }
};

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_TQOSPROVIDER_HPP
