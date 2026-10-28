// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/detail/DataWriterStatus.hpp — the four DataWriter statuses from a
// C writer handle (DDS 1.4 §2.2.4.1). Internal; reading a status resets its
// change fields.

#ifndef ZERODDS_DDS_PUB_DETAIL_DATAWRITERSTATUS_HPP
#define ZERODDS_DDS_PUB_DETAIL_DATAWRITERSTATUS_HPP

#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/policy/QosPolicyCount.hpp"
#include "dds/core/status/Status.hpp"
#include "zerodds.h"

namespace dds {
namespace pub {
namespace detail {

inline ::dds::core::status::PublicationMatchedStatus publication_matched_status_of(
    zerodds_ZeroDdsDataWriter* dw) {
    zerodds_ZeroDdsPublicationMatchedStatus s = zerodds_ZeroDdsPublicationMatchedStatus();
    ::zerodds::psm::check_status(zerodds_dw_get_publication_matched_status(dw, &s),
                              "DataWriter::publication_matched_status");
    return ::dds::core::status::PublicationMatchedStatus(
        s.total_count, s.total_count_change, s.current_count, s.current_count_change,
        ::dds::core::InstanceHandle(s.last_subscription_handle));
}

inline ::dds::core::status::LivelinessLostStatus liveliness_lost_status_of(
    zerodds_ZeroDdsDataWriter* dw) {
    zerodds_ZeroDdsLivelinessLostStatus s = zerodds_ZeroDdsLivelinessLostStatus();
    ::zerodds::psm::check_status(zerodds_dw_get_liveliness_lost_status(dw, &s),
                              "DataWriter::liveliness_lost_status");
    return ::dds::core::status::LivelinessLostStatus(s.total_count, s.total_count_change);
}

inline ::dds::core::status::OfferedDeadlineMissedStatus offered_deadline_missed_status_of(
    zerodds_ZeroDdsDataWriter* dw) {
    zerodds_ZeroDdsOfferedDeadlineMissedStatus s = zerodds_ZeroDdsOfferedDeadlineMissedStatus();
    ::zerodds::psm::check_status(zerodds_dw_get_offered_deadline_missed_status(dw, &s),
                              "DataWriter::offered_deadline_missed_status");
    return ::dds::core::status::OfferedDeadlineMissedStatus(
        s.total_count, s.total_count_change, ::dds::core::InstanceHandle(s.last_instance_handle));
}

inline ::dds::core::status::OfferedIncompatibleQosStatus offered_incompatible_qos_status_of(
    zerodds_ZeroDdsDataWriter* dw) {
    zerodds_ZeroDdsIncompatibleQosStatusEx s = zerodds_ZeroDdsIncompatibleQosStatusEx();
    ::zerodds::psm::check_status(zerodds_dw_get_offered_incompatible_qos_status_ex(dw, &s),
                              "DataWriter::offered_incompatible_qos_status");
    ::dds::core::policy::QosPolicyCountSeq policies;
    for (uint32_t i = 0; i < s.policies_len && i < 32u; ++i) {
        policies.push_back(
            ::dds::core::policy::QosPolicyCount(s.policies[i].policy_id, s.policies[i].count));
    }
    return ::dds::core::status::OfferedIncompatibleQosStatus(s.total_count, s.total_count_change,
                                                             s.last_policy_id, policies);
}

} // namespace detail
} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_DETAIL_DATAWRITERSTATUS_HPP
