// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/qos_bridge.hpp — Konvertierung dds::core::*Qos → C-FFI
// ZeroDds*Qos. Internal, for Topic<T>/Publisher/DataWriter<T>/etc.
// Konstruktoren.

#ifndef ZERODDS_DDS_CORE_QOS_BRIDGE_HPP
#define ZERODDS_DDS_CORE_QOS_BRIDGE_HPP

#include <vector>

#include "dds/core/qos.hpp"
#include "zerodds.h"

namespace dds {
namespace core {
namespace detail {

// ---------------------------------------------------------------------------
// PartitionPtrs — keeps the `const char*` array referenced by
// ZeroDdsPartitionQosPolicy{names,names_len} alive for the duration of the
// create_* call. The C-FFI partition policy is caller-owned (Spec
// §2.2.3.4 PARTITION): the strings live in the source QoS's
// std::vector<std::string>, which outlives the by-value native struct, so we
// only need to materialise a stable array of pointers into them.
// ---------------------------------------------------------------------------
struct PartitionPtrs {
    std::vector<const char*> ptrs;

    // Fills `pol.names/names_len` to point at this holder's pointer array.
    // `src` must outlive both this holder and the create_* call.
    void bind(zerodds_ZeroDdsPartitionQosPolicy& pol,
              const std::vector<std::string>& src) {
        ptrs.clear();
        ptrs.reserve(src.size());
        for (const auto& s : src) {
            ptrs.push_back(s.c_str());
        }
        pol.names = ptrs.empty() ? nullptr : ptrs.data();
        pol.names_len = ptrs.size();
    }
};

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
    out.partition.names = nullptr;
    out.partition.names_len = 0;
    return out;
}

// Overload that also wires the PARTITION policy (Spec §2.2.3.4). `q` and
// `holder` must outlive the create_* call the native struct is passed to.
inline zerodds_ZeroDdsPublisherQos to_native(const PublisherQos& q, PartitionPtrs& holder) {
    zerodds_ZeroDdsPublisherQos out = to_native(q);
    holder.bind(out.partition, q.partition.name());
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

inline zerodds_ZeroDdsSubscriberQos to_native(const SubscriberQos& q, PartitionPtrs& holder) {
    zerodds_ZeroDdsSubscriberQos out = to_native(q);
    holder.bind(out.partition, q.partition.name());
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

inline zerodds_ZeroDdsDataWriterQos to_native(const DataWriterQos& q, PartitionPtrs& holder) {
    zerodds_ZeroDdsDataWriterQos out = to_native(q);
    holder.bind(out.partition, q.partition.name());
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

inline zerodds_ZeroDdsDataReaderQos to_native(const DataReaderQos& q, PartitionPtrs& holder) {
    zerodds_ZeroDdsDataReaderQos out = to_native(q);
    holder.bind(out.partition, q.partition.name());
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
