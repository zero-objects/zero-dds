// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/qos/SubscriberQos.hpp — SubscriberQos (DDS-PSM-Cxx 1.0 §7.6).

#ifndef ZERODDS_DDS_SUB_QOS_SUBSCRIBERQOS_HPP
#define ZERODDS_DDS_SUB_QOS_SUBSCRIBERQOS_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/TEntityQos.hpp"
#include "dds/core/policy/CorePolicy.hpp"

namespace dds {
namespace sub {
namespace qos {

namespace detail {
namespace p = ::dds::core::policy;
typedef ::dds::core::detail::PolicySet<p::Presentation, p::Partition, p::GroupData,
                                       p::EntityFactory>
    SubscriberQosPolicies;
} // namespace detail

class SubscriberQos : public ::dds::core::TEntityQos<detail::SubscriberQosPolicies> {
public:
    typedef ::dds::core::TEntityQos<detail::SubscriberQosPolicies> BASE_T;

    SubscriberQos() {}
#if ZERODDS_CXX11
    SubscriberQos(const SubscriberQos&) = default;
    SubscriberQos(SubscriberQos&&) = default;
    SubscriberQos& operator=(const SubscriberQos&) = default;
    SubscriberQos& operator=(SubscriberQos&&) = default;
#endif
    template <typename D>
    SubscriberQos(const ::dds::core::TEntityQos<D>& qos) : BASE_T(qos) {}
    using BASE_T::operator=;
};

} // namespace qos
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_QOS_SUBSCRIBERQOS_HPP
