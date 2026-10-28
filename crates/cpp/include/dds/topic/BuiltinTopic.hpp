// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/BuiltinTopic.hpp — DDS-PSM-Cxx 1.0 §7.8 + DDS 1.4 §2.2.5.

#ifndef ZERODDS_DDS_TOPIC_BUILTINTOPIC_HPP
#define ZERODDS_DDS_TOPIC_BUILTINTOPIC_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "dds/topic/BuiltinTopicKey.hpp"
#include "dds/topic/TBuiltinTopic.hpp"
#include <cstddef>
#include <stdint.h>
#include <string>
#include <vector>

#include "dds/core/Time.hpp"
#include "dds/core/detail/policy_bridge.hpp"
#include "dds/core/policy/CorePolicy.hpp"
#include "zerodds.h"

namespace dds {
namespace topic {

namespace detail {

inline ::dds::core::Duration duration_of(const zerodds_ZeroDdsDuration& d) {
    return ::dds::core::Duration(d.sec, d.nanosec);
}

inline std::vector<uint8_t> octets_of(const zerodds_ZeroDdsUserDataQosPolicy& p) {
    if (p.value == 0 || p.value_len == 0) {
        return std::vector<uint8_t>();
    }
    return std::vector<uint8_t>(p.value, p.value + p.value_len);
}

inline std::vector<std::string> names_of(const zerodds_ZeroDdsPartitionQosPolicy& p) {
    std::vector<std::string> names;
    for (uintptr_t i = 0; i < p.names_len; ++i) {
        names.push_back(std::string(p.names[i] != 0 ? p.names[i] : ""));
    }
    return names;
}

inline ::dds::topic::BuiltinTopicKey key_of(const uint8_t (&k)[16]) {
    ::dds::topic::BuiltinTopicKey::VALUE_T out = ::dds::topic::BuiltinTopicKey::VALUE_T();
    for (std::size_t i = 0; i < 16; ++i) {
        out[i] = k[i];
    }
    return ::dds::topic::BuiltinTopicKey(out);
}

/// What a discovered `DataWriter` and a discovered `DataReader` both
/// carry (DDS 1.4 §2.2.5): the keys, the names and the twelve policies the
/// two builtin topics share. `PublicationBuiltinTopicData` and
/// `SubscriptionBuiltinTopicData` add their own.
class EndpointBuiltinTopicData {
public:
    const ::dds::topic::BuiltinTopicKey& key() const { return key_; }
    const ::dds::topic::BuiltinTopicKey& participant_key() const { return participant_key_; }
    const std::string& topic_name() const { return topic_name_; }
    const std::string& type_name() const { return type_name_; }
    const ::dds::core::policy::Durability& durability() const { return durability_; }
    const ::dds::core::policy::Deadline& deadline() const { return deadline_; }
    const ::dds::core::policy::LatencyBudget& latency_budget() const { return latency_budget_; }
    const ::dds::core::policy::Liveliness& liveliness() const { return liveliness_; }
    const ::dds::core::policy::Reliability& reliability() const { return reliability_; }
    const ::dds::core::policy::UserData& user_data() const { return user_data_; }
    const ::dds::core::policy::Ownership& ownership() const { return ownership_; }
    const ::dds::core::policy::DestinationOrder& destination_order() const {
        return destination_order_; }
    const ::dds::core::policy::Presentation& presentation() const { return presentation_; }
    const ::dds::core::policy::Partition& partition() const { return partition_; }
    const ::dds::core::policy::TopicData& topic_data() const { return topic_data_; }
    const ::dds::core::policy::GroupData& group_data() const { return group_data_; }

protected:
    /// The shared members out of either C API struct; both name them alike.
    template <typename Native>
    void assign_shared(const Native& n) {
        key_ = key_of(n.key);
        participant_key_ = key_of(n.participant_key);
        topic_name_ = n.topic_name != 0 ? n.topic_name : "";
        type_name_ = n.type_name != 0 ? n.type_name : "";
        durability_ = ::dds::core::detail::read(n.durability);
        deadline_ = ::dds::core::detail::read(n.deadline);
        latency_budget_ = ::dds::core::detail::read(n.latency_budget);
        liveliness_ = ::dds::core::detail::read(n.liveliness);
        reliability_ = ::dds::core::detail::read(n.reliability);
        user_data_ = ::dds::core::detail::read_user_data(n.user_data);
        ownership_ = ::dds::core::detail::read(n.ownership);
        destination_order_ = ::dds::core::detail::read(n.destination_order);
        presentation_ = ::dds::core::detail::read(n.presentation);
        partition_ = ::dds::core::detail::read(n.partition);
        topic_data_ = ::dds::core::detail::read_topic_data(n.topic_data);
        group_data_ = ::dds::core::detail::read_group_data(n.group_data);
    }

private:
    ::dds::topic::BuiltinTopicKey key_;
    ::dds::topic::BuiltinTopicKey participant_key_;
    std::string topic_name_;
    std::string type_name_;
    ::dds::core::policy::Durability durability_;
    ::dds::core::policy::Deadline deadline_;
    ::dds::core::policy::LatencyBudget latency_budget_;
    ::dds::core::policy::Liveliness liveliness_;
    ::dds::core::policy::Reliability reliability_;
    ::dds::core::policy::UserData user_data_;
    ::dds::core::policy::Ownership ownership_;
    ::dds::core::policy::DestinationOrder destination_order_;
    ::dds::core::policy::Presentation presentation_;
    ::dds::core::policy::Partition partition_;
    ::dds::core::policy::TopicData topic_data_;
    ::dds::core::policy::GroupData group_data_;
};

} // namespace detail

namespace detail {

/// `ParticipantBuiltinTopicData` (Spec §2.2.5.1).
class ParticipantBuiltinTopicData {
public:
    ParticipantBuiltinTopicData() {}

    /// From the C API's struct; copies everything it holds.
    static ParticipantBuiltinTopicData from_native(const zerodds_ZeroDdsParticipantBuiltinTopicData& n) {
        ParticipantBuiltinTopicData d;
        d.key_ = detail::key_of(n.guid);
        if (n.user_data && n.user_data_len > 0) {
            d.user_data_ = ::dds::core::policy::UserData(
                std::vector<uint8_t>(n.user_data, n.user_data + n.user_data_len));
        }
        return d;
    }

    const ::dds::topic::BuiltinTopicKey& key() const { return key_; }
    const ::dds::core::policy::UserData& user_data() const { return user_data_; }

private:
    ::dds::topic::BuiltinTopicKey key_;
    ::dds::core::policy::UserData user_data_;
};

/// `TopicBuiltinTopicData` (Spec §2.2.5.2): every member DDS 1.4 §2.2.5
/// lists for a discovered topic, each as its QoS policy class.
class TopicBuiltinTopicData {
public:
    TopicBuiltinTopicData() {}

    /// From the C API's struct; copies everything it holds.
    static TopicBuiltinTopicData from_native(const zerodds_ZeroDdsTopicBuiltinTopicData& n) {
        TopicBuiltinTopicData d;
        d.key_ = detail::key_of(n.key);
        d.name_ = n.name != 0 ? n.name : "";
        d.type_name_ = n.type_name != 0 ? n.type_name : "";
        d.durability_ = ::dds::core::detail::read(n.durability);
        d.durability_service_ = ::dds::core::detail::read(n.durability_service);
        d.deadline_ = ::dds::core::detail::read(n.deadline);
        d.latency_budget_ = ::dds::core::detail::read(n.latency_budget);
        d.liveliness_ = ::dds::core::detail::read(n.liveliness);
        d.reliability_ = ::dds::core::detail::read(n.reliability);
        d.transport_priority_ = ::dds::core::detail::read(n.transport_priority);
        d.lifespan_ = ::dds::core::detail::read(n.lifespan);
        d.destination_order_ = ::dds::core::detail::read(n.destination_order);
        d.history_ = ::dds::core::detail::read(n.history);
        d.resource_limits_ = ::dds::core::detail::read(n.resource_limits);
        d.ownership_ = ::dds::core::detail::read(n.ownership);
        d.topic_data_ = ::dds::core::detail::read_topic_data(n.topic_data);
        return d;
    }

    const ::dds::topic::BuiltinTopicKey& key() const { return key_; }
    const std::string& name() const { return name_; }
    const std::string& type_name() const { return type_name_; }
    const ::dds::core::policy::Durability& durability() const { return durability_; }
    const ::dds::core::policy::DurabilityService& durability_service() const {
        return durability_service_; }
    const ::dds::core::policy::Deadline& deadline() const { return deadline_; }
    const ::dds::core::policy::LatencyBudget& latency_budget() const { return latency_budget_; }
    const ::dds::core::policy::Liveliness& liveliness() const { return liveliness_; }
    const ::dds::core::policy::Reliability& reliability() const { return reliability_; }
    const ::dds::core::policy::TransportPriority& transport_priority() const {
        return transport_priority_; }
    const ::dds::core::policy::Lifespan& lifespan() const { return lifespan_; }
    const ::dds::core::policy::DestinationOrder& destination_order() const {
        return destination_order_; }
    const ::dds::core::policy::History& history() const { return history_; }
    const ::dds::core::policy::ResourceLimits& resource_limits() const { return resource_limits_; }
    const ::dds::core::policy::Ownership& ownership() const { return ownership_; }
    const ::dds::core::policy::TopicData& topic_data() const { return topic_data_; }

private:
    ::dds::topic::BuiltinTopicKey key_;
    std::string name_;
    std::string type_name_;
    ::dds::core::policy::Durability durability_;
    ::dds::core::policy::DurabilityService durability_service_;
    ::dds::core::policy::Deadline deadline_;
    ::dds::core::policy::LatencyBudget latency_budget_;
    ::dds::core::policy::Liveliness liveliness_;
    ::dds::core::policy::Reliability reliability_;
    ::dds::core::policy::TransportPriority transport_priority_;
    ::dds::core::policy::Lifespan lifespan_;
    ::dds::core::policy::DestinationOrder destination_order_;
    ::dds::core::policy::History history_;
    ::dds::core::policy::ResourceLimits resource_limits_;
    ::dds::core::policy::Ownership ownership_;
    ::dds::core::policy::TopicData topic_data_;
};

/// `PublicationBuiltinTopicData` (DDS 1.4 §2.2.5, DDS-PSM-Cxx §7.8):
/// every member of a discovered `DataWriter` the specification lists, each
/// as its QoS policy.
class PublicationBuiltinTopicData : public detail::EndpointBuiltinTopicData {
public:
    PublicationBuiltinTopicData() {}

    /// From the C API's struct; copies everything it holds.
    static PublicationBuiltinTopicData from_native(
        const zerodds_ZeroDdsPublicationBuiltinTopicData& n) {
        PublicationBuiltinTopicData d;
        d.assign_shared(n);
        d.durability_service_ = ::dds::core::detail::read(n.durability_service);
        d.lifespan_ = ::dds::core::detail::read(n.lifespan);
        d.ownership_strength_ = ::dds::core::detail::read(n.ownership_strength);
        return d;
    }

    const ::dds::core::policy::DurabilityService& durability_service() const {
        return durability_service_; }
    const ::dds::core::policy::Lifespan& lifespan() const { return lifespan_; }
    const ::dds::core::policy::OwnershipStrength& ownership_strength() const {
        return ownership_strength_; }

private:
    ::dds::core::policy::DurabilityService durability_service_;
    ::dds::core::policy::Lifespan lifespan_;
    ::dds::core::policy::OwnershipStrength ownership_strength_;
};

/// `SubscriptionBuiltinTopicData` (DDS 1.4 §2.2.5, DDS-PSM-Cxx §7.8):
/// every member of a discovered `DataReader` the specification lists.
class SubscriptionBuiltinTopicData : public detail::EndpointBuiltinTopicData {
public:
    SubscriptionBuiltinTopicData() {}

    /// From the C API's struct; copies everything it holds.
    static SubscriptionBuiltinTopicData from_native(
        const zerodds_ZeroDdsSubscriptionBuiltinTopicData& n) {
        SubscriptionBuiltinTopicData d;
        d.assign_shared(n);
        d.time_based_filter_ = ::dds::core::detail::read(n.time_based_filter);
        return d;
    }

    const ::dds::core::policy::TimeBasedFilter& time_based_filter() const {
        return time_based_filter_; }

private:
    ::dds::core::policy::TimeBasedFilter time_based_filter_;
};

} // namespace detail

typedef TParticipantBuiltinTopicData<detail::ParticipantBuiltinTopicData> ParticipantBuiltinTopicData;
/// The name dds/topic/BuiltinTopic.hpp of DDS-PSM-Cxx 1.0 gives the same type.
typedef ParticipantBuiltinTopicData ParticipantTopicData;
typedef TTopicBuiltinTopicData<detail::TopicBuiltinTopicData> TopicBuiltinTopicData;
typedef TPublicationBuiltinTopicData<detail::PublicationBuiltinTopicData> PublicationBuiltinTopicData;
typedef TSubscriptionBuiltinTopicData<detail::SubscriptionBuiltinTopicData> SubscriptionBuiltinTopicData;

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_BUILTINTOPIC_HPP