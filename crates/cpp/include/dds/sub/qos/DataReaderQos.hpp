// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/qos/DataReaderQos.hpp — DataReaderQos (DDS-PSM-Cxx 1.0 §7.6).
// PARTITION, PRESENTATION and GROUP_DATA belong to the Subscriber, TOPIC_DATA
// to the Topic (DDS 1.4 §2.2.3).

#ifndef ZERODDS_DDS_SUB_QOS_DATAREADERQOS_HPP
#define ZERODDS_DDS_SUB_QOS_DATAREADERQOS_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/TEntityQos.hpp"
#include "dds/core/policy/CorePolicy.hpp"

namespace dds {
namespace sub {
namespace qos {

namespace detail {
namespace p = ::dds::core::policy;
typedef ::dds::core::detail::PolicySet<
    p::UserData, p::Durability, p::Deadline, p::LatencyBudget, p::Liveliness, p::Reliability,
    p::DestinationOrder, p::History, p::ResourceLimits, p::Ownership, p::TimeBasedFilter,
    p::ReaderDataLifecycle, p::DataRepresentation, p::TypeConsistencyEnforcement>
    DataReaderQosPolicies;
} // namespace detail

class DataReaderQos : public ::dds::core::TEntityQos<detail::DataReaderQosPolicies> {
public:
    typedef ::dds::core::TEntityQos<detail::DataReaderQosPolicies> BASE_T;

    DataReaderQos() {}
#if ZERODDS_CXX11
    DataReaderQos(const DataReaderQos&) = default;
    DataReaderQos(DataReaderQos&&) = default;
    DataReaderQos& operator=(const DataReaderQos&) = default;
    DataReaderQos& operator=(DataReaderQos&&) = default;
#endif
    /// Takes the policies of another QoS kind, the Topic's among them.
    template <typename D>
    DataReaderQos(const ::dds::core::TEntityQos<D>& qos) : BASE_T(qos) {}
    using BASE_T::operator=;
};

} // namespace qos
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_QOS_DATAREADERQOS_HPP
