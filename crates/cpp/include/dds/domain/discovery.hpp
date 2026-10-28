// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/domain/discovery.hpp — DDS-PSM-Cxx 1.0 `dds::domain::ignore`.

#ifndef ZERODDS_DDS_DOMAIN_DISCOVERY_HPP
#define ZERODDS_DDS_DOMAIN_DISCOVERY_HPP

#include "dds/core/InstanceHandle.hpp"
#include "dds/domain/DomainParticipant.hpp"

namespace dds {
namespace domain {

/// Ignores the entity `handle` names (DomainParticipant::ignore_participant).
inline void ignore(const ::dds::domain::DomainParticipant& dp,
                   const ::dds::core::InstanceHandle& handle) {
    dp->ignore_participant(handle);
}

/// Ignores each entity of `[begin, end)`.
template <typename FwdIterator>
void ignore(const ::dds::domain::DomainParticipant& dp, FwdIterator begin, FwdIterator end) {
    for (; begin != end; ++begin) {
        dp->ignore_participant(*begin);
    }
}

} // namespace domain
} // namespace dds

#endif // ZERODDS_DDS_DOMAIN_DISCOVERY_HPP
