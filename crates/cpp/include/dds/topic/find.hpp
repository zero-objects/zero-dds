// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/find.hpp — DDS-PSM-Cxx 1.0 `dds::topic::find`: a topic of a
// participant by name.

#ifndef ZERODDS_DDS_TOPIC_FIND_HPP
#define ZERODDS_DDS_TOPIC_FIND_HPP


#include "zerodds/psm/cxx.hpp"
#include <string>

#include "dds/domain/DomainParticipant.hpp"
#include "dds/topic/AnyTopic.hpp"
#include "dds/topic/Topic.hpp"

namespace dds {
namespace topic {
namespace detail {

template <typename TOPIC>
struct TopicFinder;

template <typename T>
struct TopicFinder<Topic<T> > {
    static Topic<T> from(AnyTopic any) { return any.get<T>(); }
};

template <>
struct TopicFinder<AnyTopic> {
    static AnyTopic from(AnyTopic any) { return any; }
};

} // namespace detail

/// The topic `topic_name` of `dp`; a null `TOPIC` when the participant has none.
/// Throws `InvalidDowncastError` when it is a `Topic<T>` of another type.
template <typename TOPIC>
TOPIC find(const ::dds::domain::DomainParticipant& dp, const std::string& topic_name) {
    AnyTopic any = dp->find_topic(topic_name);
    if (any == ::dds::core::null) {
        return TOPIC(::dds::core::null);
    }
    return detail::TopicFinder<TOPIC>::from(ZERODDS_MOVE(any));
}

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_FIND_HPP
