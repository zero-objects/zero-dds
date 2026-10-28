// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/detail/AnyTopicDetail.hpp — the holders behind AnyTopic and
// AnyTopicDescription. Included where DomainParticipant and Topic are
// complete.

#ifndef ZERODDS_DDS_TOPIC_DETAIL_ANYTOPICDETAIL_HPP
#define ZERODDS_DDS_TOPIC_DETAIL_ANYTOPICDETAIL_HPP


#include "zerodds/psm/cxx.hpp"
#include "zerodds/psm/check_status.hpp"
#include "dds/core/detail/borrow.hpp"
#include <string>
#include <typeinfo>

#include "dds/domain/DomainParticipant.hpp"
#include "dds/topic/AnyTopic.hpp"
#include "dds/topic/AnyTopicDescription.hpp"
#include "dds/topic/Topic.hpp"
#include "dds/topic/detail/TopicDelegate.hpp"

namespace dds {
namespace topic {
namespace detail {

template <typename T>
class THolder : public THolderBase {
public:
    explicit THolder(const Topic<T>& t)
        : topic_(t), dp_(t.domain_participant()), name_(t.name()), type_name_(t.type_name()) {}

    const ::dds::domain::DomainParticipant& domain_participant() const ZERODDS_OVERRIDE { return dp_; }
    const ::dds::core::status::InconsistentTopicStatus& inconsistent_topic_status() ZERODDS_OVERRIDE {
        return topic_.inconsistent_topic_status();
    }
    const ::dds::topic::qos::TopicQos& qos() const ZERODDS_OVERRIDE { return topic_.qos(); }
    void qos(const ::dds::topic::qos::TopicQos& q) ZERODDS_OVERRIDE { topic_.qos(q); }
    const std::string& name() const ZERODDS_OVERRIDE { return name_; }
    const std::string& type_name() const ZERODDS_OVERRIDE { return type_name_; }
    const std::type_info& type() const ZERODDS_OVERRIDE { return typeid(T); }

    Topic<T> topic() const { return topic_; }

private:
    Topic<T> topic_;
    ::dds::domain::DomainParticipant dp_;
    std::string name_;
    std::string type_name_;
};

class NativeTopicHolder : public THolderBase {
public:
    explicit NativeTopicHolder(zerodds_ZeroDdsTopic* raw)
        : raw_(raw), dp_(::dds::core::null), name_(), type_name_(), status_(), qos_() {
        ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> dp(
            zerodds_topic_get_participant(raw), NullDeleter());
        dp_ = make_participant_of(ZERODDS_MOVE(dp));
        if (char* n = zerodds_topic_get_name(raw)) {
            name_ = n;
            zerodds_string_free(n);
        }
        if (char* n = zerodds_topic_get_type_name(raw)) {
            type_name_ = n;
            zerodds_string_free(n);
        }
    }

    const ::dds::domain::DomainParticipant& domain_participant() const ZERODDS_OVERRIDE { return dp_; }
    const ::dds::core::status::InconsistentTopicStatus& inconsistent_topic_status() ZERODDS_OVERRIDE {
        zerodds_ZeroDdsInconsistentTopicStatus native = zerodds_ZeroDdsInconsistentTopicStatus();
        ::zerodds::psm::check_status(zerodds_topic_get_inconsistent_topic_status(raw_, &native),
                                  "AnyTopic::inconsistent_topic_status");
        status_.total_count(native.total_count);
        status_.total_count_change(native.total_count_change);
        return status_;
    }
    const ::dds::topic::qos::TopicQos& qos() const ZERODDS_OVERRIDE {
        qos_ = fetch_topic_qos(raw_, "AnyTopic::qos");
        return qos_;
    }
    void qos(const ::dds::topic::qos::TopicQos& q) ZERODDS_OVERRIDE {
        zerodds_ZeroDdsTopicQos native = ::dds::core::detail::to_native(q);
        ::zerodds::psm::check_status(zerodds_topic_set_qos(raw_, &native), "AnyTopic::qos");
    }
    const std::string& name() const ZERODDS_OVERRIDE { return name_; }
    const std::string& type_name() const ZERODDS_OVERRIDE { return type_name_; }
    const std::type_info& type() const ZERODDS_OVERRIDE { return typeid(void); }

    zerodds_ZeroDdsTopic* native() const { return raw_; }

private:
    static ::dds::domain::DomainParticipant make_participant_of(
        ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> p) {
        return ::dds::domain::detail::make_participant(ZERODDS_MOVE(p));
    }

    zerodds_ZeroDdsTopic* raw_;
    ::dds::domain::DomainParticipant dp_;
    std::string name_;
    std::string type_name_;
    ::dds::core::status::InconsistentTopicStatus status_;
    mutable ::dds::topic::qos::TopicQos qos_;
};

/// A topic another participant announced and this one has discovered
/// (DCPSTopic): the data of the announcement, no local topic. `get<T>()`
/// creates the local `Topic<T>` of the same name.
class DiscoveredTopicHolder : public THolderBase {
public:
    DiscoveredTopicHolder(const ::dds::domain::DomainParticipant& dp,
                          const ::dds::topic::TopicBuiltinTopicData& data)
        : dp_(dp), data_(data), status_(), qos_() {
        qos_ << data.topic_data() << data.durability() << data.durability_service()
             << data.deadline() << data.latency_budget() << data.liveliness()
             << data.reliability() << data.destination_order() << data.history()
             << data.resource_limits() << data.transport_priority() << data.lifespan()
             << data.ownership();
    }

    const ::dds::domain::DomainParticipant& domain_participant() const ZERODDS_OVERRIDE { return dp_; }
    const ::dds::core::status::InconsistentTopicStatus& inconsistent_topic_status() ZERODDS_OVERRIDE {
        return status_;
    }
    const ::dds::topic::qos::TopicQos& qos() const ZERODDS_OVERRIDE { return qos_; }
    void qos(const ::dds::topic::qos::TopicQos&) ZERODDS_OVERRIDE {
        throw ::dds::core::IllegalOperationError(
            "AnyTopic::qos: a discovered topic belongs to another participant");
    }
    const std::string& name() const ZERODDS_OVERRIDE { return data_.name(); }
    const std::string& type_name() const ZERODDS_OVERRIDE { return data_.type_name(); }
    const std::type_info& type() const ZERODDS_OVERRIDE { return typeid(void); }

    const ::dds::topic::TopicBuiltinTopicData& data() const { return data_; }

private:
    ::dds::domain::DomainParticipant dp_;
    ::dds::topic::TopicBuiltinTopicData data_;
    ::dds::core::status::InconsistentTopicStatus status_;
    ::dds::topic::qos::TopicQos qos_;
};

inline AnyTopic make_discovered_any_topic(const ::dds::domain::DomainParticipant& dp,
                                          const ::dds::topic::TopicBuiltinTopicData& data);

struct AnyTopicAccess {
    static AnyTopic make(::zerodds::cxx::shared_ptr<THolderBase> holder) {
        AnyTopic any;
        any.impl_ = ZERODDS_MOVE(holder);
        return any;
    }
    static THolderBase* holder(AnyTopic& a) { return a.impl_.get(); }
};

inline AnyTopic make_native_any_topic(zerodds_ZeroDdsTopic* raw) {
    return AnyTopicAccess::make(::zerodds::cxx::make_shared<NativeTopicHolder>(raw));
}

inline AnyTopic make_discovered_any_topic(const ::dds::domain::DomainParticipant& dp,
                                          const ::dds::topic::TopicBuiltinTopicData& data) {
    return AnyTopicAccess::make(::zerodds::cxx::make_shared<DiscoveredTopicHolder>(dp, data));
}

template <typename T, typename DELEGATE>
class TDHolder : public TDHolderBase {
public:
    explicit TDHolder(const TopicDescription<T, DELEGATE>& t)
        : description_(t),
          dp_(t.domain_participant()),
          name_(t.name()),
          type_name_(t.type_name()) {}

    const ::dds::domain::DomainParticipant& domain_participant() const ZERODDS_OVERRIDE { return dp_; }
    const std::string& name() const ZERODDS_OVERRIDE { return name_; }
    const std::string& type_name() const ZERODDS_OVERRIDE { return type_name_; }
    const std::type_info& type() const ZERODDS_OVERRIDE {
        return typeid(TopicDescription<T, DELEGATE>);
    }
    const void* description() const ZERODDS_OVERRIDE { return &description_; }

private:
    TopicDescription<T, DELEGATE> description_;
    ::dds::domain::DomainParticipant dp_;
    std::string name_;
    std::string type_name_;
};

} // namespace detail

template <typename T>
AnyTopic::AnyTopic(const Topic<T>& t)
    : BASE_T(::zerodds::cxx::make_shared<detail::THolder<T> >(t)) {}

template <typename T>
Topic<T> AnyTopic::get() {
    detail::THolderBase& h = holder();
    if (detail::THolder<T>* typed = dynamic_cast<detail::THolder<T>*>(&h)) {
        return typed->topic();
    }
    if (detail::NativeTopicHolder* native = dynamic_cast<detail::NativeTopicHolder*>(&h)) {
        if (native->type_name() == detail::get_type_name<T>()) {
            return ::dds::core::detail::borrow<Topic<T> >(native->native());
        }
    }
    if (detail::DiscoveredTopicHolder* discovered = dynamic_cast<detail::DiscoveredTopicHolder*>(&h)) {
        if (discovered->type_name() == detail::get_type_name<T>()) {
            return Topic<T>(discovered->domain_participant(), discovered->name());
        }
    }
    throw ::dds::core::InvalidDowncastError("AnyTopic::get: the topic is of another type");
}

template <typename T, typename DELEGATE>
AnyTopicDescription::AnyTopicDescription(const TopicDescription<T, DELEGATE>& t)
    : holder_(::zerodds::cxx::make_shared<detail::TDHolder<T, DELEGATE> >(t)) {}

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_DETAIL_ANYTOPICDETAIL_HPP
