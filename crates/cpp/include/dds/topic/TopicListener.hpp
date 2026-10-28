// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TopicListener.hpp — TopicListener<T> (DDS-PSM-Cxx 1.0 §7.8).

#ifndef ZERODDS_DDS_TOPIC_TOPICLISTENER_HPP
#define ZERODDS_DDS_TOPIC_TOPICLISTENER_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/status/Status.hpp"

namespace dds {
namespace topic {

template <typename T>
class Topic;

template <typename T>
class TopicListener {
public:
    virtual ~TopicListener() {}

    virtual void on_inconsistent_topic(
        Topic<T>& topic, const ::dds::core::status::InconsistentTopicStatus& status) = 0;
};

template <typename T>
class NoOpTopicListener : public virtual TopicListener<T> {
public:
    virtual ~NoOpTopicListener() {}

    void on_inconsistent_topic(Topic<T>&,
                               const ::dds::core::status::InconsistentTopicStatus&) ZERODDS_OVERRIDE {}
};

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_TOPICLISTENER_HPP
