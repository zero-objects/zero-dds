// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/qos/PublisherQos.hpp — PublisherQos (DDS-PSM-Cxx 1.0 §7.6).

#ifndef ZERODDS_DDS_PUB_QOS_PUBLISHERQOS_HPP
#define ZERODDS_DDS_PUB_QOS_PUBLISHERQOS_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/TEntityQos.hpp"
#include "dds/core/policy/CorePolicy.hpp"

namespace dds {
namespace pub {
namespace qos {

namespace detail {
namespace p = ::dds::core::policy;
typedef ::dds::core::detail::PolicySet<p::Presentation, p::Partition, p::GroupData,
                                       p::EntityFactory>
    PublisherQosPolicies;
} // namespace detail

class PublisherQos : public ::dds::core::TEntityQos<detail::PublisherQosPolicies> {
public:
    typedef ::dds::core::TEntityQos<detail::PublisherQosPolicies> BASE_T;

    PublisherQos() {}
#if ZERODDS_CXX11
    PublisherQos(const PublisherQos&) = default;
    PublisherQos(PublisherQos&&) = default;
    PublisherQos& operator=(const PublisherQos&) = default;
    PublisherQos& operator=(PublisherQos&&) = default;
#endif
    template <typename D>
    PublisherQos(const ::dds::core::TEntityQos<D>& qos) : BASE_T(qos) {}
    using BASE_T::operator=;
};

} // namespace qos
} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_QOS_PUBLISHERQOS_HPP
