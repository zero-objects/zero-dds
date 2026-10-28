// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/detail/policy_bridge.hpp — conversion of the QoS policy classes
// (DDS-PSM-Cxx 1.0 §7.6.1) to and from the C API's policy structs. Internal.
// The kinds are mapped enumerator by enumerator: the C API numbers
// RELIABILITY 1 and 2, the safe_enum counts from 0.

#ifndef ZERODDS_DDS_CORE_DETAIL_POLICY_BRIDGE_HPP
#define ZERODDS_DDS_CORE_DETAIL_POLICY_BRIDGE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <limits>
#include <string>
#include <vector>

#include "dds/core/Duration.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/policy/CorePolicy.hpp"
#include "zerodds.h"

namespace dds {
namespace core {
namespace detail {

namespace p = ::dds::core::policy;

// ---- Durations ---------------------------------------------------------

inline zerodds_ZeroDdsDuration to_native(const Duration& d) {
    zerodds_ZeroDdsDuration out;
    if (::dds::core::detail::is_infinite(d)) {
        // The C API's infinite marker is {INT32_MAX, UINT32_MAX}, not the
        // DDS 1.2 {0x7fffffff, 0x7fffffff} the C++ Duration carries.
        out.sec = std::numeric_limits<int32_t>::max();
        out.nanosec = std::numeric_limits<uint32_t>::max();
    } else {
        out.sec = d.sec();
        out.nanosec = d.nanosec();
    }
    return out;
}

inline Duration duration_of(const zerodds_ZeroDdsDuration& d) {
    if (d.sec == std::numeric_limits<int32_t>::max() &&
        (d.nanosec == std::numeric_limits<uint32_t>::max() || d.nanosec == 0x7FFFFFFFu)) {
        return Duration::infinite();
    }
    return Duration(d.sec, d.nanosec);
}

// ---- Kinds -------------------------------------------------------------

inline uint32_t native_of(p::ReliabilityKind::type k) {
    return k == p::ReliabilityKind::RELIABLE ? 2u : 1u;
}
inline p::ReliabilityKind::type reliability_kind_of(uint32_t k) {
    return k == 2u ? p::ReliabilityKind::RELIABLE : p::ReliabilityKind::BEST_EFFORT;
}

inline uint32_t native_of(p::DurabilityKind::type k) {
    switch (k) {
    case p::DurabilityKind::VOLATILE: return 0u;
    case p::DurabilityKind::TRANSIENT_LOCAL: return 1u;
    case p::DurabilityKind::TRANSIENT: return 2u;
    case p::DurabilityKind::PERSISTENT: return 3u;
    }
    return 0u;
}
inline p::DurabilityKind::type durability_kind_of(uint32_t k) {
    switch (k) {
    case 1u: return p::DurabilityKind::TRANSIENT_LOCAL;
    case 2u: return p::DurabilityKind::TRANSIENT;
    case 3u: return p::DurabilityKind::PERSISTENT;
    default: return p::DurabilityKind::VOLATILE;
    }
}

inline uint32_t native_of(p::HistoryKind::type k) {
    return k == p::HistoryKind::KEEP_ALL ? 1u : 0u;
}
inline p::HistoryKind::type history_kind_of(uint32_t k) {
    return k == 1u ? p::HistoryKind::KEEP_ALL : p::HistoryKind::KEEP_LAST;
}

inline uint32_t native_of(p::OwnershipKind::type k) {
    return k == p::OwnershipKind::EXCLUSIVE ? 1u : 0u;
}
inline p::OwnershipKind::type ownership_kind_of(uint32_t k) {
    return k == 1u ? p::OwnershipKind::EXCLUSIVE : p::OwnershipKind::SHARED;
}

inline uint32_t native_of(p::LivelinessKind::type k) {
    switch (k) {
    case p::LivelinessKind::AUTOMATIC: return 0u;
    case p::LivelinessKind::MANUAL_BY_PARTICIPANT: return 1u;
    case p::LivelinessKind::MANUAL_BY_TOPIC: return 2u;
    }
    return 0u;
}
inline p::LivelinessKind::type liveliness_kind_of(uint32_t k) {
    switch (k) {
    case 1u: return p::LivelinessKind::MANUAL_BY_PARTICIPANT;
    case 2u: return p::LivelinessKind::MANUAL_BY_TOPIC;
    default: return p::LivelinessKind::AUTOMATIC;
    }
}

inline uint32_t native_of(p::DestinationOrderKind::type k) {
    return k == p::DestinationOrderKind::BY_SOURCE_TIMESTAMP ? 1u : 0u;
}
inline p::DestinationOrderKind::type destination_order_kind_of(uint32_t k) {
    return k == 1u ? p::DestinationOrderKind::BY_SOURCE_TIMESTAMP
                   : p::DestinationOrderKind::BY_RECEPTION_TIMESTAMP;
}

inline uint32_t native_of(p::PresentationAccessScopeKind::type k) {
    switch (k) {
    case p::PresentationAccessScopeKind::INSTANCE: return 0u;
    case p::PresentationAccessScopeKind::TOPIC: return 1u;
    case p::PresentationAccessScopeKind::GROUP: return 2u;
    }
    return 0u;
}
inline p::PresentationAccessScopeKind::type access_scope_of(uint32_t k) {
    switch (k) {
    case 1u: return p::PresentationAccessScopeKind::TOPIC;
    case 2u: return p::PresentationAccessScopeKind::GROUP;
    default: return p::PresentationAccessScopeKind::INSTANCE;
    }
}

// ---- Octet sequences and partition names -------------------------------

template <typename Native>
inline ByteSeq octets_of(const Native& n) {
    if (n.value == nullptr || n.value_len == 0) {
        return ByteSeq();
    }
    return ByteSeq(n.value, n.value + n.value_len);
}

inline StringSeq names_of(const zerodds_ZeroDdsPartitionQosPolicy& n) {
    StringSeq names;
    for (uintptr_t i = 0; i < n.names_len; ++i) {
        names.push_back(std::string(n.names[i] != nullptr ? n.names[i] : ""));
    }
    return names;
}

/// Keeps the `const char*` array a ZeroDdsPartitionQosPolicy points at alive
/// for the duration of the create_* call: the strings live in the source
/// QoS, which outlives the by-value native struct.
struct PartitionPtrs {
    std::vector<const char*> ptrs;
    /// A copy of the names: `Partition::name()` returns by value, so the strings the pointers
    /// name must belong to this object.
    StringSeq names;

    void bind(zerodds_ZeroDdsPartitionQosPolicy& pol, const StringSeq& src) {
        names = src;
        ptrs.clear();
        ptrs.reserve(names.size());
        for (StringSeq::const_iterator s = names.begin(); s != names.end(); ++s) {
            ptrs.push_back(s->c_str());
        }
        pol.names = ptrs.empty() ? static_cast<const char* const*>(0) : &ptrs[0];
        pol.names_len = ptrs.size();
    }
};

// ---- Policies to the C API --------------------------------------------

/// `out` points at the octets of a policy object: its `begin()` and `end()` give the storage the
/// policy keeps, which `value()` (a copy, as in the normative headers) does not.
template <typename Native>
inline void fill_octets(Native& out, const uint8_t* begin, const uint8_t* end) {
    out.value = begin == end ? static_cast<const uint8_t*>(0) : begin;
    out.value_len = static_cast<std::size_t>(end - begin);
}

inline void fill(zerodds_ZeroDdsDurabilityQosPolicy& o, const p::Durability& v) {
    o.kind = native_of(v.kind());
}
inline void fill(zerodds_ZeroDdsDurabilityServiceQosPolicy& o, const p::DurabilityService& v) {
    o.service_cleanup_delay = to_native(v.service_cleanup_delay());
    o.history_kind = native_of(v.history_kind());
    o.history_depth = v.history_depth();
    o.max_samples = v.max_samples();
    o.max_instances = v.max_instances();
    o.max_samples_per_instance = v.max_samples_per_instance();
}
inline void fill(zerodds_ZeroDdsDeadlineQosPolicy& o, const p::Deadline& v) {
    o.period = to_native(v.period());
}
inline void fill(zerodds_ZeroDdsLatencyBudgetQosPolicy& o, const p::LatencyBudget& v) {
    o.duration = to_native(v.duration());
}
inline void fill(zerodds_ZeroDdsLivelinessQosPolicy& o, const p::Liveliness& v) {
    o.kind = native_of(v.kind());
    o.lease_duration = to_native(v.lease_duration());
}
inline void fill(zerodds_ZeroDdsReliabilityQosPolicy& o, const p::Reliability& v) {
    o.kind = native_of(v.kind());
    o.max_blocking_time = to_native(v.max_blocking_time());
}
inline void fill(zerodds_ZeroDdsDestinationOrderQosPolicy& o, const p::DestinationOrder& v) {
    o.kind = native_of(v.kind());
}
inline void fill(zerodds_ZeroDdsHistoryQosPolicy& o, const p::History& v) {
    o.kind = native_of(v.kind());
    o.depth = v.depth();
}
inline void fill(zerodds_ZeroDdsResourceLimitsQosPolicy& o, const p::ResourceLimits& v) {
    o.max_samples = v.max_samples();
    o.max_instances = v.max_instances();
    o.max_samples_per_instance = v.max_samples_per_instance();
}
inline void fill(zerodds_ZeroDdsTransportPriorityQosPolicy& o, const p::TransportPriority& v) {
    o.value = v.value();
}
inline void fill(zerodds_ZeroDdsLifespanQosPolicy& o, const p::Lifespan& v) {
    o.duration = to_native(v.duration());
}
inline void fill(zerodds_ZeroDdsOwnershipQosPolicy& o, const p::Ownership& v) {
    o.kind = native_of(v.kind());
}
inline void fill(zerodds_ZeroDdsOwnershipStrengthQosPolicy& o, const p::OwnershipStrength& v) {
    o.value = v.value();
}
inline void fill(zerodds_ZeroDdsPresentationQosPolicy& o, const p::Presentation& v) {
    o.access_scope = native_of(v.access_scope());
    o.coherent_access = v.coherent_access();
    o.ordered_access = v.ordered_access();
}
inline void fill(zerodds_ZeroDdsWriterDataLifecycleQosPolicy& o, const p::WriterDataLifecycle& v) {
    o.autodispose_unregistered_instances = v.autodispose_unregistered_instances();
}
inline void fill(zerodds_ZeroDdsReaderDataLifecycleQosPolicy& o, const p::ReaderDataLifecycle& v) {
    o.autopurge_nowriter_samples_delay = to_native(v.autopurge_nowriter_samples_delay());
    o.autopurge_disposed_samples_delay = to_native(v.autopurge_disposed_samples_delay());
}
inline void fill(zerodds_ZeroDdsTimeBasedFilterQosPolicy& o, const p::TimeBasedFilter& v) {
    o.minimum_separation = to_native(v.minimum_separation());
}
inline void fill(zerodds_ZeroDdsEntityFactoryQosPolicy& o, const p::EntityFactory& v) {
    o.autoenable_created_entities = v.autoenable_created_entities();
}
inline void fill(zerodds_ZeroDdsUserDataQosPolicy& o, const p::UserData& v) {
    fill_octets(o, v.begin(), v.end());
}
inline void fill(zerodds_ZeroDdsTopicDataQosPolicy& o, const p::TopicData& v) {
    fill_octets(o, v.begin(), v.end());
}
inline void fill(zerodds_ZeroDdsGroupDataQosPolicy& o, const p::GroupData& v) {
    fill_octets(o, v.begin(), v.end());
}

/// DDS-XTypes 1.3 Annex D: a list the application gave. The C struct holds at most
/// `ZERODDS_DATA_REPRESENTATION_MAX` identifiers; a longer list is refused, not cut.
inline void fill(zerodds_ZeroDdsDataRepresentationQosPolicy& o, const p::DataRepresentation& v) {
    o = zerodds_ZeroDdsDataRepresentationQosPolicy();
    if (!v.is_set()) {
        return;
    }
    const p::DataRepresentationIdSeq list = v.value();
    if (list.size() > sizeof(o.ids) / sizeof(o.ids[0])) {
        throw ::dds::core::InvalidArgumentError(
            "DataRepresentation: a list of more than 8 identifiers does not fit");
    }
    o.is_set = 1;
    o.count = static_cast<uint8_t>(list.size());
    for (std::size_t i = 0; i < list.size(); ++i) {
        o.ids[i] = static_cast<int16_t>(list[i]);
    }
}
/// The kind is mapped from the four values of the PSM to the two of XTypes 1.3: the assignable
/// kind allows type coercion, the three others do not.
inline void fill(zerodds_ZeroDdsTypeConsistencyEnforcementQosPolicy& o,
                 const p::TypeConsistencyEnforcement& v) {
    o = zerodds_ZeroDdsTypeConsistencyEnforcementQosPolicy();
    if (!v.is_set()) {
        return;
    }
    o.is_set = 1;
    o.kind = v.kind() == p::TypeConsistencyEnforcementKind::ASSIGNABLE_TYPE_CONSISTENCY ? 1 : 0;
    o.ignore_sequence_bounds = v.ignore_sequence_bounds() ? 1 : 0;
    o.ignore_string_bounds = v.ignore_string_bounds() ? 1 : 0;
    o.ignore_member_names = v.ignore_member_names() ? 1 : 0;
    o.prevent_type_widening = v.prevent_type_widening() ? 1 : 0;
    o.force_type_validation = v.force_type_validation() ? 1 : 0;
}

// ---- Policies from the C API ------------------------------------------

inline p::Durability read(const zerodds_ZeroDdsDurabilityQosPolicy& n) {
    return p::Durability(durability_kind_of(n.kind));
}
inline p::DurabilityService read(const zerodds_ZeroDdsDurabilityServiceQosPolicy& n) {
    return p::DurabilityService(duration_of(n.service_cleanup_delay), history_kind_of(n.history_kind),
                                n.history_depth, n.max_samples, n.max_instances,
                                n.max_samples_per_instance);
}
inline p::Deadline read(const zerodds_ZeroDdsDeadlineQosPolicy& n) {
    return p::Deadline(duration_of(n.period));
}
inline p::LatencyBudget read(const zerodds_ZeroDdsLatencyBudgetQosPolicy& n) {
    return p::LatencyBudget(duration_of(n.duration));
}
inline p::Liveliness read(const zerodds_ZeroDdsLivelinessQosPolicy& n) {
    return p::Liveliness(liveliness_kind_of(n.kind), duration_of(n.lease_duration));
}
inline p::Reliability read(const zerodds_ZeroDdsReliabilityQosPolicy& n) {
    return p::Reliability(reliability_kind_of(n.kind), duration_of(n.max_blocking_time));
}
inline p::DestinationOrder read(const zerodds_ZeroDdsDestinationOrderQosPolicy& n) {
    return p::DestinationOrder(destination_order_kind_of(n.kind));
}
inline p::History read(const zerodds_ZeroDdsHistoryQosPolicy& n) {
    return p::History(history_kind_of(n.kind), n.depth);
}
inline p::ResourceLimits read(const zerodds_ZeroDdsResourceLimitsQosPolicy& n) {
    return p::ResourceLimits(n.max_samples, n.max_instances, n.max_samples_per_instance);
}
inline p::TransportPriority read(const zerodds_ZeroDdsTransportPriorityQosPolicy& n) {
    return p::TransportPriority(n.value);
}
inline p::Lifespan read(const zerodds_ZeroDdsLifespanQosPolicy& n) {
    return p::Lifespan(duration_of(n.duration));
}
inline p::Ownership read(const zerodds_ZeroDdsOwnershipQosPolicy& n) {
    return p::Ownership(ownership_kind_of(n.kind));
}
inline p::OwnershipStrength read(const zerodds_ZeroDdsOwnershipStrengthQosPolicy& n) {
    return p::OwnershipStrength(n.value);
}
inline p::Presentation read(const zerodds_ZeroDdsPresentationQosPolicy& n) {
    return p::Presentation(access_scope_of(n.access_scope), n.coherent_access, n.ordered_access);
}
inline p::Partition read(const zerodds_ZeroDdsPartitionQosPolicy& n) {
    return p::Partition(names_of(n));
}
inline p::WriterDataLifecycle read(const zerodds_ZeroDdsWriterDataLifecycleQosPolicy& n) {
    return p::WriterDataLifecycle(n.autodispose_unregistered_instances);
}
inline p::ReaderDataLifecycle read(const zerodds_ZeroDdsReaderDataLifecycleQosPolicy& n) {
    return p::ReaderDataLifecycle(duration_of(n.autopurge_nowriter_samples_delay),
                                  duration_of(n.autopurge_disposed_samples_delay));
}
inline p::TimeBasedFilter read(const zerodds_ZeroDdsTimeBasedFilterQosPolicy& n) {
    return p::TimeBasedFilter(duration_of(n.minimum_separation));
}
inline p::EntityFactory read(const zerodds_ZeroDdsEntityFactoryQosPolicy& n) {
    return p::EntityFactory(n.autoenable_created_entities);
}
inline p::DataRepresentation read(const zerodds_ZeroDdsDataRepresentationQosPolicy& n) {
    if (n.is_set == 0) {
        return p::DataRepresentation();
    }
    p::DataRepresentationIdSeq list;
    const std::size_t count = n.count < sizeof(n.ids) / sizeof(n.ids[0])
                                  ? n.count
                                  : sizeof(n.ids) / sizeof(n.ids[0]);
    for (std::size_t i = 0; i < count; ++i) {
        list.push_back(static_cast<p::DataRepresentationId>(n.ids[i]));
    }
    return p::DataRepresentation(list);
}
/// Kind 0 (DISALLOW_TYPE_COERCION) reads as the first of the PSM's kinds that forbids coercion.
inline p::TypeConsistencyEnforcement read(const zerodds_ZeroDdsTypeConsistencyEnforcementQosPolicy& n) {
    if (n.is_set == 0) {
        return p::TypeConsistencyEnforcement();
    }
    p::TypeConsistencyEnforcement out(
        n.kind == 0 ? p::TypeConsistencyEnforcementKind::EXACT_TYPE_TYPE_CONSISTENCY
                    : p::TypeConsistencyEnforcementKind::ASSIGNABLE_TYPE_CONSISTENCY);
    out.ignore_sequence_bounds(n.ignore_sequence_bounds != 0)
        .ignore_string_bounds(n.ignore_string_bounds != 0)
        .ignore_member_names(n.ignore_member_names != 0)
        .prevent_type_widening(n.prevent_type_widening != 0)
        .force_type_validation(n.force_type_validation != 0);
    return out;
}
// The three data policies share one C struct, so they cannot share a name.
inline p::UserData read_user_data(const zerodds_ZeroDdsUserDataQosPolicy& n) {
    return p::UserData(octets_of(n));
}
inline p::TopicData read_topic_data(const zerodds_ZeroDdsTopicDataQosPolicy& n) {
    return p::TopicData(octets_of(n));
}
inline p::GroupData read_group_data(const zerodds_ZeroDdsGroupDataQosPolicy& n) {
    return p::GroupData(octets_of(n));
}

} // namespace detail
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_DETAIL_POLICY_BRIDGE_HPP
