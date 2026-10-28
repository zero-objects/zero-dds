// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TContentFilteredTopic.hpp — DDS-PSM-Cxx 1.0 §7.8.

#ifndef ZERODDS_DDS_TOPIC_TCONTENTFILTEREDTOPIC_HPP
#define ZERODDS_DDS_TOPIC_TCONTENTFILTEREDTOPIC_HPP


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
#include "zerodds.h"

namespace dds {
namespace domain {
class DomainParticipant;
}

namespace topic {

namespace detail {

/// The ContentFilteredTopic API over any delegate (Spec §7.8).
template <typename T, typename DELEGATE>
class TContentFilteredTopic : public TopicDescription<T, DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef TopicDescription<T, DELEGATE_T> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// Constructs a null content filtered topic.
protected:
    TContentFilteredTopic() ZERODDS_NOEXCEPT {}
public:

    /// A content filtered topic sharing `ref`.
    explicit TContentFilteredTopic(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A content filtered view of `topic` under `name` (DDS-PSM-Cxx 1.0 §7.8).
    TContentFilteredTopic(const Topic<T>& topic, const std::string& name,
                          const ::dds::topic::Filter& filter)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE_T>(topic.domain_participant(), name, topic,
                                              filter.expression(),
                                              std::vector<std::string>(filter.begin(),
                                                                       filter.end()))) {
        this->impl_->enrol(topic.domain_participant(), this->impl_);
    }

    // Reference semantics: copy/assign share the delegate.
#if ZERODDS_CXX11
    TContentFilteredTopic(const TContentFilteredTopic&) = default;
    TContentFilteredTopic& operator=(const TContentFilteredTopic&) = default;
    TContentFilteredTopic(TContentFilteredTopic&&) = default;
    TContentFilteredTopic& operator=(TContentFilteredTopic&&) = default;
    ~TContentFilteredTopic() = default;
#endif

    /// The name the ContentFilteredTopic was created with (DDS 1.2
    /// §7.1.2.3.1.3: "The name used to create the TopicDescription").
    std::string name() const {
        if (this->impl_) {
            return this->delegate()->name();
        }
        return BASE_T::name();
    }

    /// The filter expression (Spec §7.8).
    std::string filter_expression() const {
        if (this->impl_) {
            return this->delegate()->filter_expression();
        }
        return std::string();
    }

    /// The filter parameters (Spec §7.8 `filter_parameters()`).
    std::vector<std::string> filter_parameters() const {
        if (this->impl_) {
            return this->delegate()->filter_parameters();
        }
        return std::vector<std::string>();
    }

    /// Replaces the filter parameters; DataReaders already created on this
    /// topic filter with them from their next take.
    template <typename FwdIterator>
    void filter_parameters(const FwdIterator& begin, const FwdIterator& end) {
        if (this->impl_) {
            this->delegate()->filter_parameters(begin, end);
        }
    }

    /// The topic this one filters.
    const Topic<T>& topic() const {
        if (!this->impl_) {
            throw ::dds::core::NullReferenceError("ContentFilteredTopic::topic: null");
        }
        return this->delegate()->related_topic();
    }
};

} // namespace detail

} // namespace topic
} // namespace dds

// Include full DomainParticipant definition for method implementations.
#include "dds/domain/DomainParticipant.hpp"

#endif // ZERODDS_DDS_TOPIC_TCONTENTFILTEREDTOPIC_HPP
