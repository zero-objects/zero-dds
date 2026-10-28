// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/QosProvider.hpp — QosProvider (DDS-PSM-Cxx 1.0 §7.6.2.1): loads a
// QoS configuration from a URI and answers the QoS of each entity kind.
//
//   dds::core::QosProvider qos_provider("file:///smwr/hdd/config-qos.xml", "myprofile");
//   DataReader<ShapeType> dr(sub, topic, qos_provider.datareader_qos());
//
// The URI names a DDS-XML QoS document: `file:`, `data:` and `http:` URIs are
// read, another scheme is an UnsupportedError. The id of the `*_qos(id)` operations is the
// profile (`"Library::Profile"`, or the name of a profile of any library);
// without one the operations answer for the profile the provider was made
// with, or for the only profile the document has.

#ifndef ZERODDS_DDS_CORE_QOSPROVIDER_HPP
#define ZERODDS_DDS_CORE_QOSPROVIDER_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>
#include <string>

#include "dds/core/Reference.hpp"
#include "dds/core/TQosProvider.hpp"
#include "dds/core/detail/QosProviderDelegate.hpp"
#include "dds/domain/qos/DomainParticipantQos.hpp"
#include "dds/pub/qos/DataWriterQos.hpp"
#include "dds/pub/qos/PublisherQos.hpp"
#include "dds/sub/qos/DataReaderQos.hpp"
#include "dds/sub/qos/SubscriberQos.hpp"
#include "dds/topic/qos/TopicQos.hpp"

namespace dds {
namespace core {

class QosProvider : public TQosProvider<detail::QosProviderDelegate> {
public:
    typedef TQosProvider<detail::QosProviderDelegate> BASE_T;
    typedef BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A provider sharing `ref`.
    explicit QosProvider(const DELEGATE_REF_T& ref) : BASE_T(ref) {}
    explicit QosProvider(const std::string& uri, const std::string& profile)
        : BASE_T(uri, profile) {}
    explicit QosProvider(const std::string& uri) : BASE_T(uri) {}
};

/// The spelling of the specification text (§7.6.2.1).
namespace qos {
typedef ::dds::core::QosProvider QosProvider;
} // namespace qos

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_QOSPROVIDER_HPP
