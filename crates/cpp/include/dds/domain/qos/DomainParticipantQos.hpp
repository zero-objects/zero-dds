// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/domain/qos/DomainParticipantQos.hpp — DomainParticipantQos
// (DDS-PSM-Cxx 1.0 §7.6).

#ifndef ZERODDS_DDS_DOMAIN_QOS_DOMAINPARTICIPANTQOS_HPP
#define ZERODDS_DDS_DOMAIN_QOS_DOMAINPARTICIPANTQOS_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/TEntityQos.hpp"
#include "dds/core/policy/CorePolicy.hpp"

namespace dds {
namespace domain {
namespace qos {

namespace detail {
namespace p = ::dds::core::policy;
typedef ::dds::core::detail::PolicySet<p::UserData, p::EntityFactory> DomainParticipantQosPolicies;
} // namespace detail

class DomainParticipantQos : public ::dds::core::TEntityQos<detail::DomainParticipantQosPolicies> {
public:
    typedef ::dds::core::TEntityQos<detail::DomainParticipantQosPolicies> BASE_T;

    DomainParticipantQos() {}
#if ZERODDS_CXX11
    DomainParticipantQos(const DomainParticipantQos&) = default;
    DomainParticipantQos(DomainParticipantQos&&) = default;
    DomainParticipantQos& operator=(const DomainParticipantQos&) = default;
    DomainParticipantQos& operator=(DomainParticipantQos&&) = default;
#endif
    template <typename D>
    DomainParticipantQos(const ::dds::core::TEntityQos<D>& qos) : BASE_T(qos) {}
    using BASE_T::operator=;
};

} // namespace qos
} // namespace domain
} // namespace dds

#endif // ZERODDS_DDS_DOMAIN_QOS_DOMAINPARTICIPANTQOS_HPP
