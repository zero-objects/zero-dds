// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TBuiltinTopic.hpp — DDS-PSM-Cxx 1.0 §7.8 the data of the builtin topics as
// templates over a delegate `D`.

#ifndef ZERODDS_DDS_TOPIC_TBUILTINTOPIC_HPP
#define ZERODDS_DDS_TOPIC_TBUILTINTOPIC_HPP

#include <string>

#include "dds/core/Value.hpp"
#include "dds/core/policy/CorePolicy.hpp"
#include "dds/topic/BuiltinTopicKey.hpp"
#include "zerodds.h"

// One read-only attribute, forwarded to the delegate by const reference
// (the built-in topic data are read-only, DDS 1.4 §2.2.5).
#define ZERODDS_TBUILTIN_GET(T, name) \
    const T& name() const { return this->d_.name(); }

// The attributes PublicationBuiltinTopicData and SubscriptionBuiltinTopicData
// both carry (DDS 1.4 §2.2.5: the endpoint's key, its participant's, the
// names and the QoS both sides announce).
#define ZERODDS_TBUILTIN_ENDPOINT_GETTERS \
    ZERODDS_TBUILTIN_GET(::dds::topic::BuiltinTopicKey, key) \
    ZERODDS_TBUILTIN_GET(::dds::topic::BuiltinTopicKey, participant_key) \
    ZERODDS_TBUILTIN_GET(std::string, topic_name) \
    ZERODDS_TBUILTIN_GET(std::string, type_name) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Durability, durability) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Deadline, deadline) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::LatencyBudget, latency_budget) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Liveliness, liveliness) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Reliability, reliability) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::UserData, user_data) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Ownership, ownership) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::DestinationOrder, destination_order) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Presentation, presentation) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Partition, partition) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::TopicData, topic_data) \
    ZERODDS_TBUILTIN_GET(::dds::core::policy::GroupData, group_data)

namespace dds {
namespace topic {

/// `ParticipantBuiltinTopicData` over any delegate `D` (DDS 1.4 §2.2.5).
template <typename D>
class TParticipantBuiltinTopicData : public ::dds::core::Value<D> {
public:
    TParticipantBuiltinTopicData() : ::dds::core::Value<D>() {}
    explicit TParticipantBuiltinTopicData(const D& d) : ::dds::core::Value<D>(d) {}

    /// From the C API's struct; copies everything it holds.
    static TParticipantBuiltinTopicData from_native(const zerodds_ZeroDdsParticipantBuiltinTopicData& n) { return TParticipantBuiltinTopicData(D::from_native(n)); }

    ZERODDS_TBUILTIN_GET(::dds::topic::BuiltinTopicKey, key)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::UserData, user_data)
};

/// `TopicBuiltinTopicData` over any delegate `D` (DDS 1.4 §2.2.5).
template <typename D>
class TTopicBuiltinTopicData : public ::dds::core::Value<D> {
public:
    TTopicBuiltinTopicData() : ::dds::core::Value<D>() {}
    explicit TTopicBuiltinTopicData(const D& d) : ::dds::core::Value<D>(d) {}

    /// From the C API's struct; copies everything it holds.
    static TTopicBuiltinTopicData from_native(const zerodds_ZeroDdsTopicBuiltinTopicData& n) { return TTopicBuiltinTopicData(D::from_native(n)); }

    ZERODDS_TBUILTIN_GET(::dds::topic::BuiltinTopicKey, key)
    ZERODDS_TBUILTIN_GET(std::string, name)
    ZERODDS_TBUILTIN_GET(std::string, type_name)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Durability, durability)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::DurabilityService, durability_service)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Deadline, deadline)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::LatencyBudget, latency_budget)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Liveliness, liveliness)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Reliability, reliability)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::TransportPriority, transport_priority)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Lifespan, lifespan)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::DestinationOrder, destination_order)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::History, history)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::ResourceLimits, resource_limits)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Ownership, ownership)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::TopicData, topic_data)
};

/// `PublicationBuiltinTopicData` over any delegate `D` (DDS 1.4 §2.2.5).
template <typename D>
class TPublicationBuiltinTopicData : public ::dds::core::Value<D> {
public:
    TPublicationBuiltinTopicData() : ::dds::core::Value<D>() {}
    explicit TPublicationBuiltinTopicData(const D& d) : ::dds::core::Value<D>(d) {}

    /// From the C API's struct; copies everything it holds.
    static TPublicationBuiltinTopicData from_native(const zerodds_ZeroDdsPublicationBuiltinTopicData& n) { return TPublicationBuiltinTopicData(D::from_native(n)); }

    ZERODDS_TBUILTIN_ENDPOINT_GETTERS
    ZERODDS_TBUILTIN_GET(::dds::core::policy::DurabilityService, durability_service)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::Lifespan, lifespan)
    ZERODDS_TBUILTIN_GET(::dds::core::policy::OwnershipStrength, ownership_strength)
};

/// `SubscriptionBuiltinTopicData` over any delegate `D` (DDS 1.4 §2.2.5).
template <typename D>
class TSubscriptionBuiltinTopicData : public ::dds::core::Value<D> {
public:
    TSubscriptionBuiltinTopicData() : ::dds::core::Value<D>() {}
    explicit TSubscriptionBuiltinTopicData(const D& d) : ::dds::core::Value<D>(d) {}

    /// From the C API's struct; copies everything it holds.
    static TSubscriptionBuiltinTopicData from_native(const zerodds_ZeroDdsSubscriptionBuiltinTopicData& n) { return TSubscriptionBuiltinTopicData(D::from_native(n)); }

    ZERODDS_TBUILTIN_ENDPOINT_GETTERS
    ZERODDS_TBUILTIN_GET(::dds::core::policy::TimeBasedFilter, time_based_filter)
};

} // namespace topic
} // namespace dds

#undef ZERODDS_TBUILTIN_ENDPOINT_GETTERS
#undef ZERODDS_TBUILTIN_GET

#endif // ZERODDS_DDS_TOPIC_TBUILTINTOPIC_HPP
