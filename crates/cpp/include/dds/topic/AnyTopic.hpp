// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/AnyTopic.hpp — AnyTopic (DDS-PSM-Cxx 1.0 §7.5.3): a Topic of any
// topic type, with the operations that do not depend on the type, and
// `get<T>()` to get the typed Topic back.

#ifndef ZERODDS_DDS_TOPIC_ANYTOPIC_HPP
#define ZERODDS_DDS_TOPIC_ANYTOPIC_HPP


#include "zerodds/psm/cxx.hpp"
#include <string>
#include <typeinfo>

#include "dds/core/Reference.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/topic/qos/TopicQos.hpp"
#include "zerodds.h"

namespace dds {
namespace domain {
class DomainParticipant;
} // namespace domain

namespace topic {

template <typename T>
class Topic;
class AnyTopic;

namespace detail {

/// What AnyTopic does without knowing the topic type.
class THolderBase {
public:
    virtual ~THolderBase() {}
    virtual const ::dds::domain::DomainParticipant& domain_participant() const = 0;
    virtual const ::dds::core::status::InconsistentTopicStatus& inconsistent_topic_status() = 0;
    virtual const ::dds::topic::qos::TopicQos& qos() const = 0;
    virtual void qos(const ::dds::topic::qos::TopicQos& q) = 0;
    virtual const std::string& name() const = 0;
    virtual const std::string& type_name() const = 0;
    /// The topic type this holder was made for, `typeid(void)` for a topic
    /// known by its C handle alone.
    virtual const std::type_info& type() const = 0;
};

/// Holds a `Topic<T>`; defined where the participant is complete.
template <typename T>
class THolder;

/// Holds the C handle of a topic whose type the caller did not name (the
/// topic a listener or a lookup hands out); defined where the participant is
/// complete.
class NativeTopicHolder;

struct AnyTopicAccess;

} // namespace detail
class AnyTopic;
namespace detail {
/// An AnyTopic for the topic behind a C handle whose type the caller did not
/// name (a listener callback, `find_topic`); the handle stays owned by its
/// creator.
AnyTopic make_native_any_topic(zerodds_ZeroDdsTopic* raw);
} // namespace detail

class AnyTopic : public ::dds::core::Reference<detail::THolderBase> {
public:
    typedef detail::THolderBase DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE_T> BASE_T;

    /// The null AnyTopic (`dds::core::null`).
    AnyTopic(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T() {}
    /// A null AnyTopic, holding no topic.
protected:
    AnyTopic() ZERODDS_NOEXCEPT {}
public:

    template <typename T>
    AnyTopic(const Topic<T>& t);

    const ::dds::domain::DomainParticipant& domain_participant() const {
        return holder().domain_participant();
    }
    const ::dds::core::status::InconsistentTopicStatus& inconsistent_topic_status() {
        return holder().inconsistent_topic_status();
    }
    const ::dds::topic::qos::TopicQos& qos() const { return holder().qos(); }
    void qos(const ::dds::topic::qos::TopicQos& q) { holder().qos(q); }

    /// The topic as a `Topic<T>`; `InvalidDowncastError` if it is of another
    /// type.
    template <typename T>
    Topic<T> get();

    const detail::THolderBase* operator->() const { return &holder(); }
    detail::THolderBase* operator->() { return &holder(); }

    bool operator==(const AnyTopic& other) const { return this->impl_ == other.impl_; }
    bool operator!=(const AnyTopic& other) const { return !(*this == other); }

private:
    friend struct detail::AnyTopicAccess;

    detail::THolderBase& holder() const {
        if (!this->impl_) {
            throw ::dds::core::NullReferenceError("AnyTopic: null");
        }
        return *this->impl_;
    }

};

/// The topic of `at` as a `Topic<T>`.
template <typename T>
Topic<T> get(const AnyTopic& at) {
    AnyTopic copy = at;
    return copy.get<T>();
}

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_ANYTOPIC_HPP
