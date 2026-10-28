// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/qos/DataWriterQos.hpp — DataWriterQos (DDS-PSM-Cxx 1.0 §7.6).
// PARTITION, PRESENTATION and GROUP_DATA belong to the Publisher, TOPIC_DATA
// to the Topic (DDS 1.4 §2.2.3).

#ifndef ZERODDS_DDS_PUB_QOS_DATAWRITERQOS_HPP
#define ZERODDS_DDS_PUB_QOS_DATAWRITERQOS_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/TEntityQos.hpp"
#include "dds/core/policy/CorePolicy.hpp"

namespace dds {
namespace pub {
namespace qos {

namespace detail {
namespace p = ::dds::core::policy;
typedef ::dds::core::detail::PolicySet<
    p::UserData, p::Durability, p::DurabilityService, p::Deadline, p::LatencyBudget,
    p::Liveliness, p::Reliability, p::DestinationOrder, p::History, p::ResourceLimits,
    p::TransportPriority, p::Lifespan, p::Ownership, p::OwnershipStrength,
    p::WriterDataLifecycle, p::DataRepresentation>
    DataWriterQosPolicies;
} // namespace detail

class DataWriterQos : public ::dds::core::TEntityQos<detail::DataWriterQosPolicies> {
public:
    typedef ::dds::core::TEntityQos<detail::DataWriterQosPolicies> BASE_T;

    /// DDS 1.4 §2.2.3: the default RELIABILITY of a DataWriter is RELIABLE.
    DataWriterQos() { init_reliability(); }
#if ZERODDS_CXX11
    DataWriterQos(const DataWriterQos&) = default;
    DataWriterQos(DataWriterQos&&) = default;
    DataWriterQos& operator=(const DataWriterQos&) = default;
    DataWriterQos& operator=(DataWriterQos&&) = default;
#endif
    /// Takes the policies of another QoS kind, the Topic's among them
    /// (`DataWriterQos dwqos(tqos)`); the DataWriter's RELIABLE default stays
    /// where the other QoS has no RELIABILITY.
    template <typename D>
    DataWriterQos(const ::dds::core::TEntityQos<D>& qos) : BASE_T() {
        init_reliability();
        BASE_T::operator=(qos);
    }
    using BASE_T::operator=;

private:
    void init_reliability() {
        policy(::dds::core::policy::Reliability(::dds::core::policy::ReliabilityKind::RELIABLE,
                                                ::dds::core::Duration::from_millisecs(100)));
    }
};

} // namespace qos
} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_QOS_DATAWRITERQOS_HPP
