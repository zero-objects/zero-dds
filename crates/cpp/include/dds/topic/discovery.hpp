// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/discovery.hpp — DDS-PSM-Cxx 1.0 §7.8 `dds::topic::ignore`.

#ifndef ZERODDS_DDS_TOPIC_DISCOVERY_HPP
#define ZERODDS_DDS_TOPIC_DISCOVERY_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <string>
#include <vector>

#include "dds/core/Duration.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/domain/DomainParticipant.hpp"
#include "dds/topic/AnyTopic.hpp"
#include "dds/topic/Topic.hpp"
#include "dds/topic/find.hpp"

namespace dds {
namespace topic {
namespace detail {

/// Whether `dp` has discovered a topic named `name`.
inline bool topic_discovered(const ::dds::domain::DomainParticipant& dp, const std::string& name) {
    const ::dds::core::InstanceHandleSeq handles = dp->get_discovered_topics();
    for (std::size_t i = 0; i < handles.size(); ++i) {
        if (dp->get_discovered_topic_data(handles[i]).name() == name) {
            return true;
        }
    }
    return false;
}

template <typename TOPIC>
struct TopicDiscoverer;

/// A `Topic<T>`: created locally once the topic is discovered.
template <typename T>
struct TopicDiscoverer<Topic<T> > {
    static Topic<T> make(const ::dds::domain::DomainParticipant& dp, const std::string& name) {
        return Topic<T>(dp, name);
    }
};

/// An `AnyTopic`: the participant must already have the topic, because the
/// discovery data does not name a C++ type.
template <>
struct TopicDiscoverer<AnyTopic> {
    static AnyTopic make(const ::dds::domain::DomainParticipant& dp, const std::string& name) {
        AnyTopic any = dp->find_topic(name);
        if (any == ::dds::core::null) {
            throw ::dds::core::UnsupportedError(
                "discover<AnyTopic>: the participant has no local topic of this name");
        }
        return any;
    }
};


/// Whether `X` is a topic type `discover` can return: `AnyTopic` or `Topic<T>`.
template <typename X>
struct is_discoverable_topic : ::zerodds::cxx::false_type {};
template <>
struct is_discoverable_topic<AnyTopic> : ::zerodds::cxx::true_type {};
template <typename T>
struct is_discoverable_topic<Topic<T> > : ::zerodds::cxx::true_type {};

/// The `X` for a discovered topic, or false when the topic is of another type.
template <typename X>
struct DiscoveredAs;
template <>
struct DiscoveredAs<AnyTopic> {
    static bool make(const ::dds::domain::DomainParticipant& dp,
                     const ::dds::topic::TopicBuiltinTopicData& data, AnyTopic& out) {
        out = make_discovered_any_topic(dp, data);
        return true;
    }
};
template <typename T>
struct DiscoveredAs<Topic<T> > {
    static bool make(const ::dds::domain::DomainParticipant& dp,
                     const ::dds::topic::TopicBuiltinTopicData& data, Topic<T>& out) {
        if (data.type_name() != get_type_name<T>()) {
            return false;
        }
        out = Topic<T>(dp, data.name());
        return true;
    }
};

/// Calls `f(dp, data)` for every topic the participants of this process have
/// discovered. DDS-PSM-Cxx 1.0 gives these `discover` forms no participant:
/// they read from all participants the process created and still holds.
template <typename F>
void each_discovered_topic(F f) {
    typedef std::vector< ::zerodds::cxx::shared_ptr< ::dds::domain::detail::DomainParticipantDelegate> >
        Delegates;
    const Delegates delegates = ::dds::domain::detail::ParticipantRegistry::all();
    for (Delegates::const_iterator d = delegates.begin(); d != delegates.end(); ++d) {
        const ::dds::domain::DomainParticipant dp = ::dds::domain::detail::ParticipantAccess::share(*d);
        const ::dds::core::InstanceHandleSeq handles = (*d)->get_discovered_topics();
        for (std::size_t i = 0; i < handles.size(); ++i) {
            f(dp, (*d)->get_discovered_topic_data(handles[i]));
        }
    }
}

/// `discover<ANYTOPIC>(begin, max_size)`: copies up to `max_size` topics to `begin`.
template <typename ANYTOPIC, typename FwdIterator>
struct CollectTopics {
    CollectTopics(FwdIterator& begin, uint32_t max_size, int& n)
        : begin_(begin), max_size_(max_size), n_(n) {}
    void operator()(const ::dds::domain::DomainParticipant& dp,
                    const ::dds::topic::TopicBuiltinTopicData& data) const {
        ANYTOPIC t(::dds::core::null);
        if (static_cast<uint32_t>(n_) < max_size_ && DiscoveredAs<ANYTOPIC>::make(dp, data, t)) {
            *begin_ = ZERODDS_MOVE(t);
            ++begin_;
            ++n_;
        }
    }
    FwdIterator& begin_;
    uint32_t max_size_;
    int& n_;
};

/// `discover<ANYTOPIC>(begin)`: copies all topics to `begin`.
template <typename ANYTOPIC, typename BinIterator>
struct CollectAllTopics {
    CollectAllTopics(BinIterator& begin, int& n) : begin_(begin), n_(n) {}
    void operator()(const ::dds::domain::DomainParticipant& dp,
                    const ::dds::topic::TopicBuiltinTopicData& data) const {
        ANYTOPIC t(::dds::core::null);
        if (DiscoveredAs<ANYTOPIC>::make(dp, data, t)) {
            *begin_ = ZERODDS_MOVE(t);
            ++begin_;
            ++n_;
        }
    }
    BinIterator& begin_;
    int& n_;
};

/// `discover<TopicBuiltinTopicData>(begin, max_size)`.
template <typename FwdIterator>
struct CollectTopicData {
    CollectTopicData(FwdIterator& begin, uint32_t max_size, uint32_t& n)
        : begin_(begin), max_size_(max_size), n_(n) {}
    void operator()(const ::dds::domain::DomainParticipant&,
                    const ::dds::topic::TopicBuiltinTopicData& data) const {
        if (n_ < max_size_) {
            *begin_ = data;
            ++begin_;
            ++n_;
        }
    }
    FwdIterator& begin_;
    uint32_t max_size_;
    uint32_t& n_;
};

/// `discover<TopicBuiltinTopicData>(begin)`.
template <typename BinIterator>
struct CollectAllTopicData {
    CollectAllTopicData(BinIterator& begin, uint32_t& n) : begin_(begin), n_(n) {}
    void operator()(const ::dds::domain::DomainParticipant&,
                    const ::dds::topic::TopicBuiltinTopicData& data) const {
        *begin_ = data;
        ++begin_;
        ++n_;
    }
    BinIterator& begin_;
    uint32_t& n_;
};

} // namespace detail

/// Up to `max_size` of the topics the participants of this process have
/// discovered, as `ANYTOPIC` (`AnyTopic`, or `Topic<T>` for those of type `T`).
/// Returns how many it copied to `begin`.
template <typename ANYTOPIC, typename FwdIterator>
typename ::zerodds::cxx::enable_if<detail::is_discoverable_topic<ANYTOPIC>::value, int>::type discover(
    FwdIterator begin, uint32_t max_size) {
    int n = 0;
    detail::each_discovered_topic(detail::CollectTopics<ANYTOPIC, FwdIterator>(begin, max_size, n));
    return n;
}

/// All of them.
template <typename ANYTOPIC, typename BinIterator>
typename ::zerodds::cxx::enable_if<detail::is_discoverable_topic<ANYTOPIC>::value, int>::type discover(
    BinIterator begin) {
    int n = 0;
    detail::each_discovered_topic(detail::CollectAllTopics<ANYTOPIC, BinIterator>(begin, n));
    return n;
}

/// Up to `max_size` of the discovery data (`TopicBuiltinTopicData`) of the topics discovered.
template <typename TOPICDATA, typename FwdIterator>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_same<TOPICDATA, TopicBuiltinTopicData>::value,
                                   uint32_t>::type
discover(FwdIterator begin, uint32_t max_size) {
    uint32_t n = 0;
    detail::each_discovered_topic(detail::CollectTopicData<FwdIterator>(begin, max_size, n));
    return n;
}

/// All of them.
template <typename TOPICDATA, typename BinIterator>
typename ::zerodds::cxx::enable_if< ::zerodds::cxx::is_same<TOPICDATA, TopicBuiltinTopicData>::value,
                                   uint32_t>::type
discover(BinIterator begin) {
    uint32_t n = 0;
    detail::each_discovered_topic(detail::CollectAllTopicData<BinIterator>(begin, n));
    return n;
}

/// Waits until `dp` has discovered the topic `name` and returns it as a
/// `TOPIC`. Throws `TimeoutError` after `timeout`.
template <typename TOPIC>
TOPIC discover(const ::dds::domain::DomainParticipant& dp, const std::string& name,
               const ::dds::core::Duration& timeout = ::dds::core::Duration::infinite()) {
    const bool forever = ::dds::core::detail::is_infinite(timeout);
    const int64_t deadline = static_cast<int64_t>(::zerodds::cxx::monotonic_microseconds()) +
                             (forever ? 0 : timeout.to_microsecs());
    for (;;) {
        if (detail::topic_discovered(dp, name)) {
            return detail::TopicDiscoverer<TOPIC>::make(dp, name);
        }
        if (!forever &&
            static_cast<int64_t>(::zerodds::cxx::monotonic_microseconds()) >= deadline) {
            throw ::dds::core::TimeoutError("discover: the topic was not discovered in time");
        }
        ::zerodds::cxx::sleep_milliseconds(20);
    }
}

/// Ignores the entity `handle` names (DomainParticipant::ignore_topic).
inline void ignore(const ::dds::domain::DomainParticipant& dp,
                   const ::dds::core::InstanceHandle& handle) {
    dp->ignore_topic(handle);
}

/// Ignores each entity of `[begin, end)`.
template <typename FwdIterator>
void ignore(const ::dds::domain::DomainParticipant& dp, FwdIterator begin, FwdIterator end) {
    for (; begin != end; ++begin) {
        dp->ignore_topic(*begin);
    }
}

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_DISCOVERY_HPP
