// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/AnyTopicListener.hpp — AnyTopicListener (DDS-PSM-Cxx 1.0 §7.8).

#ifndef ZERODDS_DDS_TOPIC_ANYTOPICLISTENER_HPP
#define ZERODDS_DDS_TOPIC_ANYTOPICLISTENER_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/status/Status.hpp"

namespace dds {
namespace topic {

class AnyTopic;

class AnyTopicListener {
public:
    virtual ~AnyTopicListener() {}

    virtual void on_inconsistent_topic(
        AnyTopic& topic, const ::dds::core::status::InconsistentTopicStatus& status) = 0;
};

class NoOpAnyTopicListener : public virtual AnyTopicListener {
public:
    virtual ~NoOpAnyTopicListener() {}

    void on_inconsistent_topic(AnyTopic&,
                               const ::dds::core::status::InconsistentTopicStatus&) ZERODDS_OVERRIDE {}
};

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_ANYTOPICLISTENER_HPP
