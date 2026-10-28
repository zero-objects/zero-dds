// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/qos_bridge.hpp — conversion of the QoS classes (DDS-PSM-Cxx 1.0
// §7.6) to the C API's ZeroDds*Qos structs and back. Internal, for the
// entity constructors and the `qos()` operations.

#ifndef ZERODDS_DDS_CORE_QOS_BRIDGE_HPP
#define ZERODDS_DDS_CORE_QOS_BRIDGE_HPP

#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/detail/policy_bridge.hpp"
#include "dds/domain/qos/DomainParticipantQos.hpp"
#include "dds/pub/qos/DataWriterQos.hpp"
#include "dds/pub/qos/PublisherQos.hpp"
#include "dds/sub/qos/DataReaderQos.hpp"
#include "dds/sub/qos/SubscriberQos.hpp"
#include "dds/topic/qos/TopicQos.hpp"
#include "zerodds.h"

namespace dds {
namespace core {
namespace detail {

// ---- To the C API ------------------------------------------------------

inline zerodds_ZeroDdsTopicQos to_native(const ::dds::topic::qos::TopicQos& q) {
    zerodds_ZeroDdsTopicQos out = zerodds_ZeroDdsTopicQos();
    fill(out.durability, q.policy<p::Durability>());
    fill(out.durability_service, q.policy<p::DurabilityService>());
    fill(out.deadline, q.policy<p::Deadline>());
    fill(out.latency_budget, q.policy<p::LatencyBudget>());
    fill(out.liveliness, q.policy<p::Liveliness>());
    fill(out.reliability, q.policy<p::Reliability>());
    fill(out.destination_order, q.policy<p::DestinationOrder>());
    fill(out.history, q.policy<p::History>());
    fill(out.resource_limits, q.policy<p::ResourceLimits>());
    fill(out.transport_priority, q.policy<p::TransportPriority>());
    fill(out.lifespan, q.policy<p::Lifespan>());
    fill(out.ownership, q.policy<p::Ownership>());
    fill(out.topic_data, q.policy<p::TopicData>());
    fill(out.data_representation, q.policy<p::DataRepresentation>());
    return out;
}

/// `q` and `holder` must outlive the create_* call the native struct is
/// passed to: the PARTITION names are referenced, not copied.
inline zerodds_ZeroDdsPublisherQos to_native(const ::dds::pub::qos::PublisherQos& q,
                                             PartitionPtrs& holder) {
    zerodds_ZeroDdsPublisherQos out = zerodds_ZeroDdsPublisherQos();
    fill(out.presentation, q.policy<p::Presentation>());
    fill(out.entity_factory, q.policy<p::EntityFactory>());
    fill(out.group_data, q.policy<p::GroupData>());
    holder.bind(out.partition, q.policy<p::Partition>().name());
    return out;
}

inline zerodds_ZeroDdsSubscriberQos to_native(const ::dds::sub::qos::SubscriberQos& q,
                                              PartitionPtrs& holder) {
    zerodds_ZeroDdsSubscriberQos out = zerodds_ZeroDdsSubscriberQos();
    fill(out.presentation, q.policy<p::Presentation>());
    fill(out.entity_factory, q.policy<p::EntityFactory>());
    fill(out.group_data, q.policy<p::GroupData>());
    holder.bind(out.partition, q.policy<p::Partition>().name());
    return out;
}

/// PARTITION, PRESENTATION and GROUP_DATA are the Publisher's and
/// TOPIC_DATA the Topic's (DDS 1.4 §2.2.3); the writer's own members of the
/// C struct stay empty, which makes the writer take the Publisher's and the
/// Topic's.
inline zerodds_ZeroDdsDataWriterQos to_native(const ::dds::pub::qos::DataWriterQos& q) {
    zerodds_ZeroDdsDataWriterQos out = zerodds_ZeroDdsDataWriterQos();
    fill(out.reliability, q.policy<p::Reliability>());
    fill(out.durability, q.policy<p::Durability>());
    fill(out.durability_service, q.policy<p::DurabilityService>());
    fill(out.deadline, q.policy<p::Deadline>());
    fill(out.latency_budget, q.policy<p::LatencyBudget>());
    fill(out.liveliness, q.policy<p::Liveliness>());
    fill(out.destination_order, q.policy<p::DestinationOrder>());
    fill(out.lifespan, q.policy<p::Lifespan>());
    fill(out.ownership, q.policy<p::Ownership>());
    fill(out.ownership_strength, q.policy<p::OwnershipStrength>());
    fill(out.history, q.policy<p::History>());
    fill(out.resource_limits, q.policy<p::ResourceLimits>());
    fill(out.transport_priority, q.policy<p::TransportPriority>());
    fill(out.writer_data_lifecycle, q.policy<p::WriterDataLifecycle>());
    fill(out.user_data, q.policy<p::UserData>());
    fill(out.data_representation, q.policy<p::DataRepresentation>());
    return out;
}

inline zerodds_ZeroDdsDataReaderQos to_native(const ::dds::sub::qos::DataReaderQos& q) {
    zerodds_ZeroDdsDataReaderQos out = zerodds_ZeroDdsDataReaderQos();
    fill(out.reliability, q.policy<p::Reliability>());
    fill(out.durability, q.policy<p::Durability>());
    fill(out.deadline, q.policy<p::Deadline>());
    fill(out.latency_budget, q.policy<p::LatencyBudget>());
    fill(out.liveliness, q.policy<p::Liveliness>());
    fill(out.destination_order, q.policy<p::DestinationOrder>());
    fill(out.ownership, q.policy<p::Ownership>());
    fill(out.history, q.policy<p::History>());
    fill(out.resource_limits, q.policy<p::ResourceLimits>());
    fill(out.time_based_filter, q.policy<p::TimeBasedFilter>());
    fill(out.reader_data_lifecycle, q.policy<p::ReaderDataLifecycle>());
    fill(out.user_data, q.policy<p::UserData>());
    fill(out.data_representation, q.policy<p::DataRepresentation>());
    fill(out.type_consistency, q.policy<p::TypeConsistencyEnforcement>());
    return out;
}

inline zerodds_ZeroDdsDomainParticipantQos to_native(
    const ::dds::domain::qos::DomainParticipantQos& q) {
    zerodds_ZeroDdsDomainParticipantQos out = zerodds_ZeroDdsDomainParticipantQos();
    fill(out.entity_factory, q.policy<p::EntityFactory>());
    fill(out.user_data, q.policy<p::UserData>());
    return out;
}

// ---- From the C API ----------------------------------------------------

/// Calls a C API QoS getter and converts the result. The octet sequences of
/// a QoS (USER_DATA, TOPIC_DATA, GROUP_DATA) are copied into buffers the
/// caller provides; a getter that finds a buffer too small answers
/// OUT_OF_RESOURCES with the length it needs, and is called again with
/// buffers of that length. `octets` names the members of `Native` that are
/// such buffers.
/// The members of `Native` that are octet-buffer policies, in the order they are listed:
/// `OctetMembers<N>()(&N::user_data)(&N::topic_data)`.
template <typename Native>
class OctetMembers {
public:
    typedef zerodds_ZeroDdsUserDataQosPolicy Native::*Member;

    OctetMembers& operator()(Member member) {
        members_.push_back(member);
        return *this;
    }
    std::size_t size() const { return members_.size(); }
    Member operator[](std::size_t i) const { return members_[i]; }

private:
    std::vector<Member> members_;
};

/// The argument of `fetch_qos` for a C API getter `int fn(Handle, Native*)` called with `handle`.
template <typename Native, typename Handle>
class QosGetter1 {
public:
    typedef int (*Fn)(Handle, Native*);
    QosGetter1(Fn fn, Handle handle) : fn_(fn), handle_(handle) {}
    int operator()(Native* n) const { return fn_(handle_, n); }

private:
    Fn fn_;
    Handle handle_;
};

template <typename T>
struct QosIdentity {
    typedef T type;
};

template <typename Native, typename Handle>
inline QosGetter1<Native, Handle> make_qos_getter(int (*fn)(Handle, Native*),
                                                  typename QosIdentity<Handle>::type handle) {
    return QosGetter1<Native, Handle>(fn, handle);
}

/// Calls a C API QoS getter and converts the result. The octet sequences of
/// a QoS (USER_DATA, TOPIC_DATA, GROUP_DATA) are copied into buffers the
/// caller provides; a getter that finds a buffer too small answers
/// OUT_OF_RESOURCES with the length it needs, and is called again with
/// buffers of that length. `octets` names the members of `Native` that are
/// such buffers. `get` is a function object that takes a `Native*` and answers the status code
/// (`make_qos_getter` makes one from a C function and its handle); `convert` makes the QoS of
/// the native struct.
template <typename Native, typename Result, typename Get>
inline Result fetch_qos(Get get, Result (*convert)(const Native&), const char* what,
                        const OctetMembers<Native>& octets = OctetMembers<Native>()) {
    Native n = Native();
    std::vector<std::vector<uint8_t> > bufs(octets.size());
    for (std::size_t attempt = 0; attempt <= octets.size() + 1; ++attempt) {
        const int rc = get(&n);
        if (rc == 0) {
            return convert(n);
        }
        const int out_of_resources = -8;
        if (rc != out_of_resources) {
            ::zerodds::psm::check_status(rc, what);
        }
        for (std::size_t i = 0; i < octets.size(); ++i) {
            zerodds_ZeroDdsUserDataQosPolicy& policy = n.*(octets[i]);
            if (policy.value_len > bufs[i].size()) {
                bufs[i].resize(policy.value_len);
            }
            policy.value = bufs[i].empty() ? static_cast<const uint8_t*>(0) : &bufs[i][0];
            policy.value_len = bufs[i].size();
        }
    }
    throw ::dds::core::Error(std::string(what) + ": the QoS did not fit its buffers");
}

inline ::dds::topic::qos::TopicQos topic_qos_of(const zerodds_ZeroDdsTopicQos& n) {
    ::dds::topic::qos::TopicQos q;
    q << read(n.durability) << read(n.durability_service) << read(n.deadline)
      << read(n.latency_budget) << read(n.liveliness) << read(n.reliability)
      << read(n.destination_order) << read(n.history) << read(n.resource_limits)
      << read(n.transport_priority) << read(n.lifespan) << read(n.ownership)
      << read_topic_data(n.topic_data) << read(n.data_representation);
    return q;
}

inline ::dds::pub::qos::PublisherQos publisher_qos_of(const zerodds_ZeroDdsPublisherQos& n) {
    ::dds::pub::qos::PublisherQos q;
    q << read(n.presentation) << read(n.partition) << read_group_data(n.group_data)
      << read(n.entity_factory);
    return q;
}

inline ::dds::sub::qos::SubscriberQos subscriber_qos_of(const zerodds_ZeroDdsSubscriberQos& n) {
    ::dds::sub::qos::SubscriberQos q;
    q << read(n.presentation) << read(n.partition) << read_group_data(n.group_data)
      << read(n.entity_factory);
    return q;
}

inline ::dds::pub::qos::DataWriterQos datawriter_qos_of(const zerodds_ZeroDdsDataWriterQos& n) {
    ::dds::pub::qos::DataWriterQos q;
    q << read(n.reliability) << read(n.durability) << read(n.durability_service)
      << read(n.deadline) << read(n.latency_budget) << read(n.liveliness)
      << read(n.destination_order) << read(n.lifespan) << read(n.ownership)
      << read(n.ownership_strength) << read(n.history) << read(n.resource_limits)
      << read(n.transport_priority) << read(n.writer_data_lifecycle) << read_user_data(n.user_data)
      << read(n.data_representation);
    return q;
}

inline ::dds::sub::qos::DataReaderQos datareader_qos_of(const zerodds_ZeroDdsDataReaderQos& n) {
    ::dds::sub::qos::DataReaderQos q;
    q << read(n.reliability) << read(n.durability) << read(n.deadline) << read(n.latency_budget)
      << read(n.liveliness) << read(n.destination_order) << read(n.ownership) << read(n.history)
      << read(n.resource_limits) << read(n.time_based_filter) << read(n.reader_data_lifecycle)
      << read_user_data(n.user_data) << read(n.data_representation) << read(n.type_consistency);
    return q;
}

inline ::dds::domain::qos::DomainParticipantQos participant_qos_of(
    const zerodds_ZeroDdsDomainParticipantQos& n) {
    ::dds::domain::qos::DomainParticipantQos q;
    q << read(n.entity_factory) << read_user_data(n.user_data);
    return q;
}

} // namespace detail
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_QOS_BRIDGE_HPP
