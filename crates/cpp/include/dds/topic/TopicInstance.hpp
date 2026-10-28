// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TopicInstance.hpp — TopicInstance<T> (DDS-PSM-Cxx 1.0 §7.8): an
// instance handle with the sample (or the key) it stands for.

#ifndef ZERODDS_DDS_TOPIC_TOPICINSTANCE_HPP
#define ZERODDS_DDS_TOPIC_TOPICINSTANCE_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/InstanceHandle.hpp"

namespace dds {
namespace topic {

template <typename T>
class TopicInstance {
public:
    TopicInstance() : h_(), sample_() {}
    TopicInstance(const ::dds::core::InstanceHandle& h) : h_(h), sample_() {}
    TopicInstance(const ::dds::core::InstanceHandle& h, const T& the_sample)
        : h_(h), sample_(the_sample) {}

    operator const ::dds::core::InstanceHandle() const { return h_; }
    const ::dds::core::InstanceHandle handle() const { return h_; }
    void handle(const ::dds::core::InstanceHandle& h) { h_ = h; }
    const T& sample() const { return sample_; }
    T& sample() { return sample_; }
    void sample(const T& the_sample) { sample_ = the_sample; }

private:
    ::dds::core::InstanceHandle h_;
    T sample_;
};

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_TOPICINSTANCE_HPP
