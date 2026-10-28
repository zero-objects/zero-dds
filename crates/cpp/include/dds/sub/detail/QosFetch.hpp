// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/detail/QosFetch.hpp — the QoS getters of the C API for the readers and subscribers.
// Internal. The loop that calls a getter again with buffers for the octet sequences is
// `dds::core::detail::fetch_qos` (dds/core/qos_bridge.hpp); these are its three uses for readers
// and subscribers.

#ifndef ZERODDS_DDS_SUB_DETAIL_QOSFETCH_HPP
#define ZERODDS_DDS_SUB_DETAIL_QOSFETCH_HPP

#include "zerodds/psm/cxx.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/sub/qos/DataReaderQos.hpp"
#include "dds/sub/qos/SubscriberQos.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {
namespace detail {

/// The QoS of the reader behind `dr` (`dds::core::detail::fetch_qos`).
inline ::dds::sub::qos::DataReaderQos fetch_datareader_qos(zerodds_ZeroDdsDataReader* dr,
                                                           const char* what) {
    return ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_dr_get_qos, dr),
        &::dds::core::detail::datareader_qos_of, what,
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsDataReaderQos>()(
            &zerodds_ZeroDdsDataReaderQos::user_data)(&zerodds_ZeroDdsDataReaderQos::topic_data)(
            &zerodds_ZeroDdsDataReaderQos::group_data));
}

/// The default reader QoS of the subscriber behind `s`.
inline ::dds::sub::qos::DataReaderQos fetch_default_datareader_qos(zerodds_ZeroDdsSubscriber* s,
                                                                   const char* what) {
    return ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_sub_get_default_datareader_qos, s),
        &::dds::core::detail::datareader_qos_of, what,
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsDataReaderQos>()(
            &zerodds_ZeroDdsDataReaderQos::user_data)(&zerodds_ZeroDdsDataReaderQos::topic_data)(
            &zerodds_ZeroDdsDataReaderQos::group_data));
}

/// The QoS of the subscriber behind `s`.
inline ::dds::sub::qos::SubscriberQos fetch_subscriber_qos(zerodds_ZeroDdsSubscriber* s,
                                                           const char* what) {
    return ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_sub_get_qos, s),
        &::dds::core::detail::subscriber_qos_of, what,
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsSubscriberQos>()(
            &zerodds_ZeroDdsSubscriberQos::group_data));
}

}  // namespace detail
}  // namespace sub
}  // namespace dds

#endif  // ZERODDS_DDS_SUB_DETAIL_QOSFETCH_HPP
