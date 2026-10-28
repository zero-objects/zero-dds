// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/ContentFilteredTopic.hpp — DDS-PSM-Cxx 1.0 §7.8.

#ifndef ZERODDS_DDS_TOPIC_CONTENTFILTEREDTOPIC_HPP
#define ZERODDS_DDS_TOPIC_CONTENTFILTEREDTOPIC_HPP


#include "zerodds/psm/cxx.hpp"
#include <string>
#include <vector>

#include "dds/core/Reference.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/core/TEntity.hpp"
#include "dds/topic/Filter.hpp"
#include "dds/topic/Topic.hpp"
#include "dds/topic/detail/ContentFilteredTopicDelegate.hpp"
#include "dds/topic/TContentFilteredTopic.hpp"
#include "zerodds.h"

namespace dds {
namespace domain {
class DomainParticipant;
}

namespace topic {

/// ContentFilteredTopic<T> (Spec §7.8).
/// Reference type with delegate pattern (DDS-PSM-Cxx 1.0 §7.5.1.1, §7.5.1.2).
template <typename T>
class ContentFilteredTopic : public detail::TContentFilteredTopic<T, detail::ContentFilteredTopicDelegate<T> > {
public:
    typedef detail::ContentFilteredTopicDelegate<T> DELEGATE_T;
    typedef detail::TContentFilteredTopic<T, DELEGATE_T> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A content filtered topic sharing `ref`.
    explicit ContentFilteredTopic(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A content filtered view of `topic` under `name` (DDS-PSM-Cxx 1.0 §7.8).
    ContentFilteredTopic(const Topic<T>& topic, const std::string& name,
                         const ::dds::topic::Filter& filter)
        : BASE_T(topic, name, filter) {}

    /// Constructs a null content filtered topic.
protected:
    ContentFilteredTopic() ZERODDS_NOEXCEPT {}
public:

    /// The null ContentFilteredTopic (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    ContentFilteredTopic(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    ContentFilteredTopic(const ContentFilteredTopic&) = default;
    ContentFilteredTopic& operator=(const ContentFilteredTopic&) = default;
    ContentFilteredTopic(ContentFilteredTopic&&) = default;
    ContentFilteredTopic& operator=(ContentFilteredTopic&&) = default;
    ~ContentFilteredTopic() = default;
#endif
};

namespace detail {
template <typename T>
inline void ContentFilteredTopicDelegate<T>::enrol(
    const ::dds::domain::DomainParticipant& dp,
    const ::zerodds::cxx::shared_ptr<ContentFilteredTopicDelegate>& self) {
    dp->child_topics().add(self, DelegateCloser<ContentFilteredTopicDelegate>());
}
} // namespace detail

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_CONTENTFILTEREDTOPIC_HPP
