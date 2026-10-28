// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/discovery.hpp — DDS-PSM-Cxx 1.0 `dds::sub::matched_publications`
// and `dds::sub::matched_publication_data` (DDS 1.4 §2.2.2.5.3
// get_matched_publications, get_matched_publication_data).

#ifndef ZERODDS_DDS_SUB_DISCOVERY_HPP
#define ZERODDS_DDS_SUB_DISCOVERY_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include "zerodds/psm/handle_seq.hpp"
#include <stdint.h>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/domain/DomainParticipant.hpp"
#include "dds/sub/Subscriber.hpp"
#include "dds/topic/BuiltinTopic.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {

/// Ignores the entity `handle` names (DomainParticipant::ignore_subscription).
inline void ignore(const ::dds::domain::DomainParticipant& dp,
                   const ::dds::core::InstanceHandle& handle) {
    dp->ignore_subscription(handle);
}

/// Ignores each entity of `[begin, end)`.
template <typename FwdIterator>
void ignore(const ::dds::domain::DomainParticipant& dp, FwdIterator begin, FwdIterator end) {
    for (; begin != end; ++begin) {
        dp->ignore_subscription(*begin);
    }
}

/// The handles of the publications `reader` is matched with.
template <typename T>
std::vector< ::dds::core::InstanceHandle> matched_publications(const DataReader<T>& reader) {
    return ::zerodds::psm::matched_handles(&zerodds_dr_get_matched_publications, reader->native_handle(),
                                           "matched_publications");
}

/// The handles of the matched entities, copied to `[begin, end)` as far as the
/// range reaches. Returns all handles as a sequence that stays valid until the
/// next call of this function on the same thread, so a caller sees how many did
/// not fit (DDS-PSM-Cxx 1.0 declares the return as a reference).
template <typename T, typename FwdIterator>
::dds::core::InstanceHandleSeq& matched_publications(const DataReader<T>& entity, FwdIterator begin,
                                                    FwdIterator end) {
    return ::zerodds::psm::copy_handles<1>(matched_publications(entity), begin, end);
}

/// The DCPSPublication data of the publication `handle` names, which
/// `reader` is matched with.
///
/// Throws `dds::core::InvalidArgumentError` when `handle` names no
/// publication the reader is matched with.
template <typename T>
::dds::topic::PublicationBuiltinTopicData matched_publication_data(
    const DataReader<T>& reader, const ::dds::core::InstanceHandle& handle) {
    zerodds_ZeroDdsPublicationBuiltinTopicData native = zerodds_ZeroDdsPublicationBuiltinTopicData();
    ::zerodds::psm::check_status(
        zerodds_dr_get_matched_publication_data(reader->native_handle(), handle.value(), &native),
        "matched_publication_data");
    ::dds::topic::PublicationBuiltinTopicData data =
        ::dds::topic::PublicationBuiltinTopicData::from_native(native);
    zerodds_publication_builtin_topic_data_free(&native);
    return data;
}

/// `matched_publications_data` is the name the normative header of DDS-PSM-Cxx 1.0 gives the
/// operation (dds/sub/discovery.hpp); `matched_publication_data` is the same call.
template <typename T>
::dds::topic::PublicationBuiltinTopicData matched_publications_data(
    const DataReader<T>& reader, const ::dds::core::InstanceHandle& handle) {
    return matched_publication_data(reader, handle);
}

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DISCOVERY_HPP
