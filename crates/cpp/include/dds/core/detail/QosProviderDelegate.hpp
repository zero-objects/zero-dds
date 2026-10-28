// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/detail/QosProviderDelegate.hpp — delegate of QosProvider.

#ifndef ZERODDS_DDS_CORE_DETAIL_QOSPROVIDERDELEGATE_HPP
#define ZERODDS_DDS_CORE_DETAIL_QOSPROVIDERDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <string>

#include "dds/core/Exception.hpp"
#include "dds/core/qos_bridge.hpp"
#include "zerodds.h"

namespace dds {
namespace core {
namespace detail {

/// The getter `fetch_qos` calls: `getter(provider, id, n)` with `id` null when empty.
template <typename Native>
class QosProviderGetter {
public:
    typedef int (*Fn)(zerodds_ZeroDdsQosProvider*, const char*, Native*);
    QosProviderGetter(zerodds_ZeroDdsQosProvider* provider, const std::string& id, Fn getter)
        : provider_(provider), id_(id), getter_(getter) {}
    int operator()(Native* n) const {
        return getter_(provider_, id_.empty() ? static_cast<const char*>(0) : id_.c_str(), n);
    }

private:
    zerodds_ZeroDdsQosProvider* provider_;
    std::string id_;
    Fn getter_;
};

class QosProviderDelegate {
    template <typename Native, typename Result>
    Result fetch_qos(const std::string& id,
                     int (*getter)(zerodds_ZeroDdsQosProvider*, const char*, Native*),
                     Result (*convert)(const Native&), const OctetMembers<Native>& octets) {
        return ::dds::core::detail::fetch_qos<Native>(
            QosProviderGetter<Native>(provider_, id, getter), convert, "QosProvider", octets);
    }

public:
    /// Loads the QoS libraries `uri` names. `profile` is the profile the
    /// operations without an id answer for.
    QosProviderDelegate(const std::string& uri, const std::string& profile) : provider_(0) {
        int status = 0;
        provider_ = zerodds_qosprovider_create(
            uri.c_str(), profile.empty() ? static_cast<const char*>(0) : profile.c_str(), &status);
        if (provider_ == nullptr) {
            if (status == -13) {
                throw UnsupportedError("QosProvider: this URI scheme has no client: " + uri);
            }
            throw InvalidArgumentError("QosProvider: cannot load the QoS profiles of " + uri +
                                       (profile.empty() ? "" : " (profile " + profile + ")"));
        }
    }
    ~QosProviderDelegate() { zerodds_qosprovider_destroy(provider_); }

    ZERODDS_NONCOPYABLE(QosProviderDelegate)

    ::dds::domain::qos::DomainParticipantQos participant_qos(const std::string& id) {
        return fetch_qos<zerodds_ZeroDdsDomainParticipantQos>(id, &zerodds_qosprovider_participant_qos,
                             &participant_qos_of,
                             OctetMembers<zerodds_ZeroDdsDomainParticipantQos>()
                                 (&zerodds_ZeroDdsDomainParticipantQos::user_data));
    }
    ::dds::topic::qos::TopicQos topic_qos(const std::string& id) {
        return fetch_qos<zerodds_ZeroDdsTopicQos>(id, &zerodds_qosprovider_topic_qos,
                             &topic_qos_of,
                             OctetMembers<zerodds_ZeroDdsTopicQos>()
                                 (&zerodds_ZeroDdsTopicQos::topic_data));
    }
    ::dds::sub::qos::SubscriberQos subscriber_qos(const std::string& id) {
        return fetch_qos<zerodds_ZeroDdsSubscriberQos>(id, &zerodds_qosprovider_subscriber_qos,
                             &subscriber_qos_of,
                             OctetMembers<zerodds_ZeroDdsSubscriberQos>()
                                 (&zerodds_ZeroDdsSubscriberQos::group_data));
    }
    ::dds::sub::qos::DataReaderQos datareader_qos(const std::string& id) {
        return fetch_qos<zerodds_ZeroDdsDataReaderQos>(id, &zerodds_qosprovider_datareader_qos,
                             &datareader_qos_of,
                             OctetMembers<zerodds_ZeroDdsDataReaderQos>()
                                 (&zerodds_ZeroDdsDataReaderQos::user_data)
                                 (&zerodds_ZeroDdsDataReaderQos::topic_data)
                                 (&zerodds_ZeroDdsDataReaderQos::group_data));
    }
    ::dds::pub::qos::PublisherQos publisher_qos(const std::string& id) {
        return fetch_qos<zerodds_ZeroDdsPublisherQos>(id, &zerodds_qosprovider_publisher_qos,
                             &publisher_qos_of,
                             OctetMembers<zerodds_ZeroDdsPublisherQos>()
                                 (&zerodds_ZeroDdsPublisherQos::group_data));
    }
    ::dds::pub::qos::DataWriterQos datawriter_qos(const std::string& id) {
        return fetch_qos<zerodds_ZeroDdsDataWriterQos>(id, &zerodds_qosprovider_datawriter_qos,
                             &datawriter_qos_of,
                             OctetMembers<zerodds_ZeroDdsDataWriterQos>()
                                 (&zerodds_ZeroDdsDataWriterQos::user_data)
                                 (&zerodds_ZeroDdsDataWriterQos::topic_data)
                                 (&zerodds_ZeroDdsDataWriterQos::group_data));
    }

private:
    zerodds_ZeroDdsQosProvider* provider_;
};

} // namespace detail
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_DETAIL_QOSPROVIDERDELEGATE_HPP
