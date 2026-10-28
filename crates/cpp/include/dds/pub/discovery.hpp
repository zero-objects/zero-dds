// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/discovery.hpp — DDS-PSM-Cxx 1.0 `dds::pub::matched_subscriptions`
// and `dds::pub::matched_subscription_data` (DDS 1.4 §2.2.2.4.2
// get_matched_subscriptions, get_matched_subscription_data).

#ifndef ZERODDS_DDS_PUB_DISCOVERY_HPP
#define ZERODDS_DDS_PUB_DISCOVERY_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include "zerodds/psm/handle_seq.hpp"
#include <stdint.h>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/domain/DomainParticipant.hpp"
#include "dds/pub/Publisher.hpp"
#include "dds/topic/BuiltinTopic.hpp"
#include "zerodds.h"

namespace dds {
namespace pub {

/// Ignores the entity `handle` names (DomainParticipant::ignore_publication).
inline void ignore(const ::dds::domain::DomainParticipant& dp,
                   const ::dds::core::InstanceHandle& handle) {
    dp->ignore_publication(handle);
}

/// Ignores each entity of `[begin, end)`.
template <typename FwdIterator>
void ignore(const ::dds::domain::DomainParticipant& dp, FwdIterator begin, FwdIterator end) {
    for (; begin != end; ++begin) {
        dp->ignore_publication(*begin);
    }
}

/// The handles of the subscriptions `writer` is matched with.
template <typename T>
std::vector< ::dds::core::InstanceHandle> matched_subscriptions(const DataWriter<T>& writer) {
    return ::zerodds::psm::matched_handles(&zerodds_dw_get_matched_subscriptions, writer->native_handle(),
                                           "matched_subscriptions");
}

/// The handles of the matched entities, copied to `[begin, end)` as far as the
/// range reaches. Returns all handles as a sequence that stays valid until the
/// next call of this function on the same thread, so a caller sees how many did
/// not fit (DDS-PSM-Cxx 1.0 declares the return as a reference).
template <typename T, typename FwdIterator>
::dds::core::InstanceHandleSeq& matched_subscriptions(const DataWriter<T>& entity, FwdIterator begin,
                                                    FwdIterator end) {
    return ::zerodds::psm::copy_handles<0>(matched_subscriptions(entity), begin, end);
}

/// The DCPSSubscription data of the subscription `handle` names, which
/// `writer` is matched with.
///
/// Throws `dds::core::InvalidArgumentError` when `handle` names no
/// subscription the writer is matched with.
template <typename T>
::dds::topic::SubscriptionBuiltinTopicData matched_subscription_data(
    const DataWriter<T>& writer, const ::dds::core::InstanceHandle& handle) {
    zerodds_ZeroDdsSubscriptionBuiltinTopicData native = zerodds_ZeroDdsSubscriptionBuiltinTopicData();
    ::zerodds::psm::check_status(
        zerodds_dw_get_matched_subscription_data(writer->native_handle(), handle.value(), &native),
        "matched_subscription_data");
    ::dds::topic::SubscriptionBuiltinTopicData data =
        ::dds::topic::SubscriptionBuiltinTopicData::from_native(native);
    zerodds_subscription_builtin_topic_data_free(&native);
    return data;
}

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_DISCOVERY_HPP
