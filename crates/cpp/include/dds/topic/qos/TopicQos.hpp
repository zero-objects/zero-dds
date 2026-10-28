// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/qos/TopicQos.hpp — TopicQos (DDS-PSM-Cxx 1.0 §7.6).

#ifndef ZERODDS_DDS_TOPIC_QOS_TOPICQOS_HPP
#define ZERODDS_DDS_TOPIC_QOS_TOPICQOS_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/TEntityQos.hpp"
#include "dds/core/policy/CorePolicy.hpp"

namespace dds {
namespace topic {
namespace qos {

namespace detail {
namespace p = ::dds::core::policy;
typedef ::dds::core::detail::PolicySet<
    p::TopicData, p::Durability, p::DurabilityService, p::Deadline, p::LatencyBudget,
    p::Liveliness, p::Reliability, p::DestinationOrder, p::History, p::ResourceLimits,
    p::TransportPriority, p::Lifespan, p::Ownership, p::DataRepresentation>
    TopicQosPolicies;
} // namespace detail

class TopicQos : public ::dds::core::TEntityQos<detail::TopicQosPolicies> {
public:
    typedef ::dds::core::TEntityQos<detail::TopicQosPolicies> BASE_T;

    TopicQos() {}
#if ZERODDS_CXX11
    TopicQos(const TopicQos&) = default;
    TopicQos(TopicQos&&) = default;
    TopicQos& operator=(const TopicQos&) = default;
    TopicQos& operator=(TopicQos&&) = default;
#endif
    template <typename D>
    TopicQos(const ::dds::core::TEntityQos<D>& qos) : BASE_T(qos) {}
    using BASE_T::operator=;
};

} // namespace qos
} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_QOS_TOPICQOS_HPP
