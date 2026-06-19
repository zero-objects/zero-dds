// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/qos.hpp — DDS-PSM-Cxx 1.0 QoS-Sets §7.5.6.

#ifndef ZERODDS_DDS_CORE_QOS_HPP
#define ZERODDS_DDS_CORE_QOS_HPP

#include "dds/core/policy/CorePolicy.hpp"

namespace dds {
namespace core {

namespace policy_ns = ::dds::core::policy;

/// DomainParticipantFactoryQos.
class DomainParticipantFactoryQos {
public:
    policy_ns::EntityFactory& entity_factory() { return entity_factory_; }
    const policy_ns::EntityFactory& entity_factory() const { return entity_factory_; }
    DomainParticipantFactoryQos& operator<<(const policy_ns::EntityFactory& p) {
        entity_factory_ = p;
        return *this;
    }
private:
    policy_ns::EntityFactory entity_factory_{};
};

/// DomainParticipantQos.
class DomainParticipantQos {
public:
    policy_ns::UserData& user_data() { return user_data_; }
    const policy_ns::UserData& user_data() const { return user_data_; }
    policy_ns::EntityFactory& entity_factory() { return entity_factory_; }
    const policy_ns::EntityFactory& entity_factory() const { return entity_factory_; }
    DomainParticipantQos& operator<<(const policy_ns::UserData& p) {
        user_data_ = p;
        return *this;
    }
    DomainParticipantQos& operator<<(const policy_ns::EntityFactory& p) {
        entity_factory_ = p;
        return *this;
    }
private:
    policy_ns::UserData user_data_;
    policy_ns::EntityFactory entity_factory_;
};

/// TopicQos.
class TopicQos {
public:
    policy_ns::Durability durability;
    policy_ns::DurabilityService durability_service;
    policy_ns::Deadline deadline;
    policy_ns::LatencyBudget latency_budget;
    policy_ns::Liveliness liveliness;
    policy_ns::Reliability reliability;
    policy_ns::DestinationOrder destination_order;
    policy_ns::History history;
    policy_ns::ResourceLimits resource_limits;
    policy_ns::TransportPriority transport_priority;
    policy_ns::Lifespan lifespan;
    policy_ns::Ownership ownership;
    policy_ns::TopicData topic_data;
    template <typename P>
    TopicQos& operator<<(const P& p) {
        return policy_assign_(*this, p);
    }
private:
    template <typename Q>
    static Q& policy_assign_(Q& q, const policy_ns::Durability& p) {
        q.durability = p;
        return q;
    }
    template <typename Q>
    static Q& policy_assign_(Q& q, const policy_ns::Deadline& p) {
        q.deadline = p;
        return q;
    }
    template <typename Q>
    static Q& policy_assign_(Q& q, const policy_ns::Reliability& p) {
        q.reliability = p;
        return q;
    }
    template <typename Q>
    static Q& policy_assign_(Q& q, const policy_ns::History& p) {
        q.history = p;
        return q;
    }
};

/// PublisherQos.
class PublisherQos {
public:
    policy_ns::Presentation presentation;
    policy_ns::Partition partition;
    policy_ns::GroupData group_data;
    policy_ns::EntityFactory entity_factory;
};

/// SubscriberQos (same field set as PublisherQos per Spec §7.5.6).
class SubscriberQos {
public:
    policy_ns::Presentation presentation;
    policy_ns::Partition partition;
    policy_ns::GroupData group_data;
    policy_ns::EntityFactory entity_factory;
};

/// DataWriterQos.
class DataWriterQos {
public:
    policy_ns::Reliability reliability{policy_ns::Reliability::Reliable_()};
    policy_ns::Durability durability;
    policy_ns::DurabilityService durability_service;
    policy_ns::Deadline deadline;
    policy_ns::LatencyBudget latency_budget;
    policy_ns::Liveliness liveliness;
    policy_ns::DestinationOrder destination_order;
    policy_ns::Lifespan lifespan;
    policy_ns::Ownership ownership;
    policy_ns::OwnershipStrength ownership_strength;
    policy_ns::Partition partition;
    policy_ns::Presentation presentation;
    policy_ns::History history;
    policy_ns::ResourceLimits resource_limits;
    policy_ns::TransportPriority transport_priority;
    policy_ns::WriterDataLifecycle writer_data_lifecycle;
    policy_ns::UserData user_data;
    policy_ns::TopicData topic_data;
    policy_ns::GroupData group_data;
};

/// DataReaderQos.
class DataReaderQos {
public:
    policy_ns::Reliability reliability{policy_ns::Reliability::BestEffort()};
    policy_ns::Durability durability;
    policy_ns::Deadline deadline;
    policy_ns::LatencyBudget latency_budget;
    policy_ns::Liveliness liveliness;
    policy_ns::DestinationOrder destination_order;
    policy_ns::Ownership ownership;
    policy_ns::Partition partition;
    policy_ns::Presentation presentation;
    policy_ns::History history;
    policy_ns::ResourceLimits resource_limits;
    policy_ns::TimeBasedFilter time_based_filter;
    policy_ns::ReaderDataLifecycle reader_data_lifecycle;
    policy_ns::UserData user_data;
    policy_ns::TopicData topic_data;
    policy_ns::GroupData group_data;
};

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_QOS_HPP
