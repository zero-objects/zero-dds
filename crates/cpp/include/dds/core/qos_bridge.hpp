// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/qos_bridge.hpp — Konvertierung dds::core::*Qos → C-FFI
// ZeroDds*Qos. Internal, for Topic<T>/Publisher/DataWriter<T>/etc.
// Konstruktoren.

#ifndef ZERODDS_DDS_CORE_QOS_BRIDGE_HPP
#define ZERODDS_DDS_CORE_QOS_BRIDGE_HPP

#include "dds/core/qos.hpp"
#include "zerodds.h"

namespace dds {
namespace core {
namespace detail {

inline zerodds_ZeroDdsDuration to_native(const Duration& d) {
    zerodds_ZeroDdsDuration out;
    out.sec = d.sec();
    out.nanosec = d.nanosec();
    return out;
}

inline zerodds_ZeroDdsTopicQos to_native(const TopicQos& q) {
    zerodds_ZeroDdsTopicQos out{};
    out.durability.kind = static_cast<uint32_t>(q.durability.kind());
    out.durability_service.service_cleanup_delay = to_native(q.durability_service.service_cleanup_delay());
    out.durability_service.history_kind = static_cast<uint32_t>(q.durability_service.history_kind());
    out.durability_service.history_depth = q.durability_service.history_depth();
    out.durability_service.max_samples = q.durability_service.max_samples();
    out.durability_service.max_instances = q.durability_service.max_instances();
    out.durability_service.max_samples_per_instance = q.durability_service.max_samples_per_instance();
    out.deadline.period = to_native(q.deadline.period());
    out.latency_budget.duration = to_native(q.latency_budget.duration());
    out.liveliness.kind = static_cast<uint32_t>(q.liveliness.kind());
    out.liveliness.lease_duration = to_native(q.liveliness.lease_duration());
    out.reliability.kind = static_cast<uint32_t>(q.reliability.kind());
    out.reliability.max_blocking_time = to_native(q.reliability.max_blocking_time());
    out.destination_order.kind = static_cast<uint32_t>(q.destination_order.kind());
    out.history.kind = static_cast<uint32_t>(q.history.kind());
    out.history.depth = q.history.depth();
    out.resource_limits.max_samples = q.resource_limits.max_samples();
    out.resource_limits.max_instances = q.resource_limits.max_instances();
    out.resource_limits.max_samples_per_instance = q.resource_limits.max_samples_per_instance();
    out.transport_priority.value = q.transport_priority.value();
    out.lifespan.duration = to_native(q.lifespan.duration());
    out.ownership.kind = static_cast<uint32_t>(q.ownership.kind());
    out.topic_data.value = q.topic_data.value().empty() ? nullptr : q.topic_data.value().data();
    out.topic_data.value_len = q.topic_data.value().size();
    return out;
}

inline zerodds_ZeroDdsPublisherQos to_native(const PublisherQos& q) {
    zerodds_ZeroDdsPublisherQos out{};
    out.presentation.access_scope = static_cast<uint32_t>(q.presentation.access_scope());
    out.presentation.coherent_access = q.presentation.coherent_access();
    out.presentation.ordered_access = q.presentation.ordered_access();
    out.entity_factory.autoenable_created_entities = q.entity_factory.autoenable_created_entities();
    out.group_data.value = q.group_data.value().empty() ? nullptr : q.group_data.value().data();
    out.group_data.value_len = q.group_data.value().size();
    out.partition.names = nullptr; // partition bridge in a follow-up patch
    out.partition.names_len = 0;
    return out;
}

inline zerodds_ZeroDdsSubscriberQos to_native(const SubscriberQos& q) {
    zerodds_ZeroDdsSubscriberQos out{};
    out.presentation.access_scope = static_cast<uint32_t>(q.presentation.access_scope());
    out.presentation.coherent_access = q.presentation.coherent_access();
    out.presentation.ordered_access = q.presentation.ordered_access();
    out.entity_factory.autoenable_created_entities = q.entity_factory.autoenable_created_entities();
    out.group_data.value = q.group_data.value().empty() ? nullptr : q.group_data.value().data();
    out.group_data.value_len = q.group_data.value().size();
    out.partition.names = nullptr;
    out.partition.names_len = 0;
    return out;
}

inline zerodds_ZeroDdsDataWriterQos to_native(const DataWriterQos& q) {
    zerodds_ZeroDdsDataWriterQos out{};
    out.reliability.kind = static_cast<uint32_t>(q.reliability.kind());
    out.reliability.max_blocking_time = to_native(q.reliability.max_blocking_time());
    out.durability.kind = static_cast<uint32_t>(q.durability.kind());
    out.durability_service.service_cleanup_delay = to_native(q.durability_service.service_cleanup_delay());
    out.durability_service.history_kind = static_cast<uint32_t>(q.durability_service.history_kind());
    out.durability_service.history_depth = q.durability_service.history_depth();
    out.durability_service.max_samples = q.durability_service.max_samples();
    out.durability_service.max_instances = q.durability_service.max_instances();
    out.durability_service.max_samples_per_instance = q.durability_service.max_samples_per_instance();
    out.deadline.period = to_native(q.deadline.period());
    out.latency_budget.duration = to_native(q.latency_budget.duration());
    out.liveliness.kind = static_cast<uint32_t>(q.liveliness.kind());
    out.liveliness.lease_duration = to_native(q.liveliness.lease_duration());
    out.destination_order.kind = static_cast<uint32_t>(q.destination_order.kind());
    out.lifespan.duration = to_native(q.lifespan.duration());
    out.ownership.kind = static_cast<uint32_t>(q.ownership.kind());
    out.ownership_strength.value = q.ownership_strength.value();
    out.presentation.access_scope = static_cast<uint32_t>(q.presentation.access_scope());
    out.presentation.coherent_access = q.presentation.coherent_access();
    out.presentation.ordered_access = q.presentation.ordered_access();
    out.history.kind = static_cast<uint32_t>(q.history.kind());
    out.history.depth = q.history.depth();
    out.resource_limits.max_samples = q.resource_limits.max_samples();
    out.resource_limits.max_instances = q.resource_limits.max_instances();
    out.resource_limits.max_samples_per_instance = q.resource_limits.max_samples_per_instance();
    out.transport_priority.value = q.transport_priority.value();
    out.writer_data_lifecycle.autodispose_unregistered_instances =
        q.writer_data_lifecycle.autodispose_unregistered_instances();
    out.user_data.value = q.user_data.value().empty() ? nullptr : q.user_data.value().data();
    out.user_data.value_len = q.user_data.value().size();
    out.topic_data.value = q.topic_data.value().empty() ? nullptr : q.topic_data.value().data();
    out.topic_data.value_len = q.topic_data.value().size();
    out.group_data.value = q.group_data.value().empty() ? nullptr : q.group_data.value().data();
    out.group_data.value_len = q.group_data.value().size();
    out.partition.names = nullptr;
    out.partition.names_len = 0;
    return out;
}

inline zerodds_ZeroDdsDataReaderQos to_native(const DataReaderQos& q) {
    zerodds_ZeroDdsDataReaderQos out{};
    out.reliability.kind = static_cast<uint32_t>(q.reliability.kind());
    out.reliability.max_blocking_time = to_native(q.reliability.max_blocking_time());
    out.durability.kind = static_cast<uint32_t>(q.durability.kind());
    out.deadline.period = to_native(q.deadline.period());
    out.latency_budget.duration = to_native(q.latency_budget.duration());
    out.liveliness.kind = static_cast<uint32_t>(q.liveliness.kind());
    out.liveliness.lease_duration = to_native(q.liveliness.lease_duration());
    out.destination_order.kind = static_cast<uint32_t>(q.destination_order.kind());
    out.ownership.kind = static_cast<uint32_t>(q.ownership.kind());
    out.presentation.access_scope = static_cast<uint32_t>(q.presentation.access_scope());
    out.presentation.coherent_access = q.presentation.coherent_access();
    out.presentation.ordered_access = q.presentation.ordered_access();
    out.history.kind = static_cast<uint32_t>(q.history.kind());
    out.history.depth = q.history.depth();
    out.resource_limits.max_samples = q.resource_limits.max_samples();
    out.resource_limits.max_instances = q.resource_limits.max_instances();
    out.resource_limits.max_samples_per_instance = q.resource_limits.max_samples_per_instance();
    out.time_based_filter.minimum_separation = to_native(q.time_based_filter.minimum_separation());
    out.reader_data_lifecycle.autopurge_nowriter_samples_delay =
        to_native(q.reader_data_lifecycle.autopurge_nowriter_samples_delay());
    out.reader_data_lifecycle.autopurge_disposed_samples_delay =
        to_native(q.reader_data_lifecycle.autopurge_disposed_samples_delay());
    out.user_data.value = q.user_data.value().empty() ? nullptr : q.user_data.value().data();
    out.user_data.value_len = q.user_data.value().size();
    out.topic_data.value = q.topic_data.value().empty() ? nullptr : q.topic_data.value().data();
    out.topic_data.value_len = q.topic_data.value().size();
    out.group_data.value = q.group_data.value().empty() ? nullptr : q.group_data.value().data();
    out.group_data.value_len = q.group_data.value().size();
    out.partition.names = nullptr;
    out.partition.names_len = 0;
    return out;
}

inline zerodds_ZeroDdsDomainParticipantQos to_native(const DomainParticipantQos& q) {
    zerodds_ZeroDdsDomainParticipantQos out{};
    out.entity_factory.autoenable_created_entities = q.entity_factory().autoenable_created_entities();
    out.user_data.value = q.user_data().value().empty() ? nullptr : q.user_data().value().data();
    out.user_data.value_len = q.user_data().value().size();
    return out;
}

} // namespace detail
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_QOS_BRIDGE_HPP
