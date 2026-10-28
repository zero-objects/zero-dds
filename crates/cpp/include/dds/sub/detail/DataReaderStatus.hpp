// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/detail/DataReaderStatus.hpp — the DataReader statuses from a C
// reader handle (DDS 1.4 §2.2.4.1). Internal; reading a status resets its
// change fields.

#ifndef ZERODDS_DDS_SUB_DETAIL_DATAREADERSTATUS_HPP
#define ZERODDS_DDS_SUB_DETAIL_DATAREADERSTATUS_HPP

#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/policy/QosPolicyCount.hpp"
#include "dds/core/status/Status.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {
namespace detail {

inline ::dds::core::status::SubscriptionMatchedStatus subscription_matched_status_of(
    zerodds_ZeroDdsDataReader* dr) {
    zerodds_ZeroDdsSubscriptionMatchedStatus s = zerodds_ZeroDdsSubscriptionMatchedStatus();
    ::zerodds::psm::check_status(zerodds_dr_get_subscription_matched_status(dr, &s),
                              "DataReader::subscription_matched_status");
    return ::dds::core::status::SubscriptionMatchedStatus(
        s.total_count, s.total_count_change, s.current_count, s.current_count_change,
        ::dds::core::InstanceHandle(s.last_publication_handle));
}

inline ::dds::core::status::SampleLostStatus sample_lost_status_of(zerodds_ZeroDdsDataReader* dr) {
    zerodds_ZeroDdsSampleLostStatus s = zerodds_ZeroDdsSampleLostStatus();
    ::zerodds::psm::check_status(zerodds_dr_get_sample_lost_status(dr, &s),
                              "DataReader::sample_lost_status");
    return ::dds::core::status::SampleLostStatus(s.total_count, s.total_count_change);
}

inline ::dds::core::status::RequestedDeadlineMissedStatus requested_deadline_missed_status_of(
    zerodds_ZeroDdsDataReader* dr) {
    zerodds_ZeroDdsRequestedDeadlineMissedStatus s = zerodds_ZeroDdsRequestedDeadlineMissedStatus();
    ::zerodds::psm::check_status(zerodds_dr_get_requested_deadline_missed_status(dr, &s),
                              "DataReader::requested_deadline_missed_status");
    return ::dds::core::status::RequestedDeadlineMissedStatus(
        s.total_count, s.total_count_change, ::dds::core::InstanceHandle(s.last_instance_handle));
}

inline ::dds::core::status::LivelinessChangedStatus liveliness_changed_status_of(
    zerodds_ZeroDdsDataReader* dr) {
    zerodds_ZeroDdsLivelinessChangedStatus s = zerodds_ZeroDdsLivelinessChangedStatus();
    ::zerodds::psm::check_status(zerodds_dr_get_liveliness_changed_status(dr, &s),
                              "DataReader::liveliness_changed_status");
    return ::dds::core::status::LivelinessChangedStatus(
        s.alive_count, s.not_alive_count, s.alive_count_change, s.not_alive_count_change,
        ::dds::core::InstanceHandle(s.last_publication_handle));
}

inline ::dds::core::status::SampleRejectedStatus sample_rejected_status_of(
    zerodds_ZeroDdsDataReader* dr) {
    zerodds_ZeroDdsSampleRejectedStatus s = zerodds_ZeroDdsSampleRejectedStatus();
    ::zerodds::psm::check_status(zerodds_dr_get_sample_rejected_status(dr, &s),
                              "DataReader::sample_rejected_status");
    // The C API numbers the reasons as DDS 1.4 does (0 not rejected, 1
    // instances limit, 2 samples limit, 3 samples per instance limit); the
    // state has one bit per reason.
    ::dds::core::status::SampleRejectedState reason =
        s.last_reason == 0
            ? ::dds::core::status::SampleRejectedState::not_rejected()
            : ::dds::core::status::SampleRejectedState(
                  ::dds::core::status::SampleRejectedState::MaskType(1u << (s.last_reason - 1)));
    return ::dds::core::status::SampleRejectedStatus(
        s.total_count, s.total_count_change, reason,
        ::dds::core::InstanceHandle(s.last_instance_handle));
}

inline ::dds::core::status::RequestedIncompatibleQosStatus requested_incompatible_qos_status_of(
    zerodds_ZeroDdsDataReader* dr) {
    zerodds_ZeroDdsIncompatibleQosStatusEx s = zerodds_ZeroDdsIncompatibleQosStatusEx();
    ::zerodds::psm::check_status(zerodds_dr_get_requested_incompatible_qos_status_ex(dr, &s),
                              "DataReader::requested_incompatible_qos_status");
    ::dds::core::policy::QosPolicyCountSeq policies;
    for (uint32_t i = 0; i < s.policies_len && i < 32u; ++i) {
        policies.push_back(
            ::dds::core::policy::QosPolicyCount(s.policies[i].policy_id, s.policies[i].count));
    }
    return ::dds::core::status::RequestedIncompatibleQosStatus(s.total_count, s.total_count_change,
                                                               s.last_policy_id, policies);
}

} // namespace detail
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DETAIL_DATAREADERSTATUS_HPP
