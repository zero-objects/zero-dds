// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/domain/detail/DomainParticipantDelegate.hpp — Delegate for DomainParticipant.

#ifndef ZERODDS_DDS_DOMAIN_DETAIL_DOMAINPARTICIPANTDELEGATE_HPP
#define ZERODDS_DDS_DOMAIN_DETAIL_DOMAINPARTICIPANTDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include <string>
#include <vector>

#include "dds/core/TEntity.hpp"
#include "dds/core/detail/ChildTracker.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/topic/BuiltinTopic.hpp"
#include "dds/topic/AnyTopic.hpp"
#include "dds/pub/qos/PublisherQos.hpp"
#include "dds/sub/qos/SubscriberQos.hpp"
#include "dds/topic/qos/TopicQos.hpp"
#include "zerodds.h"

namespace dds {
namespace domain {

class DomainParticipantListener;

} // namespace domain

namespace domain {
namespace detail {

// Forward declare types from dds::topic that are used in this header.
namespace topic {
class TopicBuiltinTopicData;
class ParticipantBuiltinTopicData;
class AnyTopic;
} // namespace topic

using dds::topic::TopicBuiltinTopicData;
using dds::topic::ParticipantBuiltinTopicData;
using dds::topic::AnyTopic;

// Forward declare the C callback functions.
extern "C" {
void dp_listener_on_inconsistent_topic(void* user_data, struct zerodds_ZeroDdsTopic* topic);
void dp_listener_on_data_on_readers(void* user_data, struct zerodds_ZeroDdsSubscriber* subscriber);
}

/// The deleter of the shared native participant: frees what the participant contains, then the
/// participant.
struct ParticipantDeleter {
    explicit ParticipantDeleter(const zerodds_ZeroDdsDomainParticipantFactory* f) : fact_(f) {}
    void operator()(zerodds_ZeroDdsDomainParticipant* p) const {
        zerodds_dp_delete_contained_entities(p);
        zerodds_dpf_delete_participant(fact_, p);
    }
    const zerodds_ZeroDdsDomainParticipantFactory* fact_;
};

/// The deleter of a native builtin subscriber handle (null is allowed).
struct BuiltinSubscriberDeleter {
    void operator()(zerodds_ZeroDdsBuiltinSubscriber* h) const {
        if (h != 0) {
            zerodds_builtin_subscriber_destroy(h);
        }
    }
};

/// Delegate for DomainParticipant (DDS-PSM-Cxx 1.0 §7.7).
class DomainParticipantDelegate {
public:
    explicit DomainParticipantDelegate(uint32_t domain_id);
    DomainParticipantDelegate(uint32_t domain_id, const ::dds::domain::qos::DomainParticipantQos& qos);
    /// Wraps a participant the caller shares ownership of (a topic or an
    /// endpoint hands its participant to the application this way).
    explicit DomainParticipantDelegate(::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> p)
        : factory_(zerodds_dpf_get_instance()),
          participant_(ZERODDS_MOVE(p)),
          qos_(),
          listener_reg_(0),
          closed_(false) {}
    ~DomainParticipantDelegate();

    // Non-copyable, non-movable (owned by shared_ptr).
    ZERODDS_NONCOPYABLE(DomainParticipantDelegate)
#if ZERODDS_CXX11
    DomainParticipantDelegate(DomainParticipantDelegate&&) = delete;
    DomainParticipantDelegate& operator=(DomainParticipantDelegate&&) = delete;
#endif

    zerodds_ZeroDdsDomainParticipant* native_handle() const { return participant_.get(); }
    const zerodds_ZeroDdsDomainParticipantFactory* factory() const { return factory_; }
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant() const { return participant_; }

    // Entity operations (spec §7.5.1.2).
    void enable();
    ::dds::core::status::StatusMask status_changes() const;
    ::dds::core::InstanceHandle instance_handle() const;
    void close();
    void retain();

    // DomainParticipant-specific operations.
    uint32_t domain_id() const;
    void assert_liveliness();
    bool contains_entity(const ::dds::core::InstanceHandle& h) const;
    void ignore_participant(const ::dds::core::InstanceHandle& h) const;
    void ignore_topic(const ::dds::core::InstanceHandle& h) const;
    void ignore_publication(const ::dds::core::InstanceHandle& h) const;
    void ignore_subscription(const ::dds::core::InstanceHandle& h) const;
    /// Closes every publisher, subscriber, topic and content-filtered topic created with this
    /// participant and still held (the entities they contain first), then the C API's own list.
    void delete_contained_entities();

    /// The children created with this participant (`delete_contained_entities`).
    ::dds::core::detail::ChildTracker& child_publishers() const { return publishers_; }
    ::dds::core::detail::ChildTracker& child_subscribers() const { return subscribers_; }
    ::dds::core::detail::ChildTracker& child_topics() const { return topics_; }

    // Discovery operations.
    ::dds::core::InstanceHandleSeq get_discovered_participants() const;
    ::dds::core::InstanceHandleSeq get_discovered_topics() const;
    ::dds::topic::TopicBuiltinTopicData get_discovered_topic_data(
        const ::dds::core::InstanceHandle& handle) const;
    size_t discovered_publications_count() const;
    size_t discovered_subscriptions_count() const;

    // QoS operations.
    const ::dds::domain::qos::DomainParticipantQos& qos() const;
    void qos(const ::dds::domain::qos::DomainParticipantQos& qos);

    // Listener operations.
    void listener(::dds::domain::DomainParticipantListener* l,
                  const ::dds::core::status::StatusMask& mask);
    ::dds::domain::DomainParticipantListener* listener() const;

    // Factory default QoS.
    /// The default the factory applies to new participants (`dds::domain::TDomainParticipant`).
    static ::dds::domain::qos::DomainParticipantQos factory_default_participant_qos();
    static void set_factory_default_participant_qos(
        const ::dds::domain::qos::DomainParticipantQos& qos);

    ::dds::domain::qos::DomainParticipantQos default_participant_qos() const;
    void default_participant_qos(const ::dds::domain::qos::DomainParticipantQos& qos);

    // Topic operations.
    ::dds::topic::AnyTopic find_topic(const std::string& name) const;
    ::dds::topic::AnyTopic lookup_topicdescription(const std::string& name) const;
    // DDS-XTypes 1.3 §7.6.4.2: the topics of one name, in the order they were created; `index` 0
    // is the first. A null topic for an index past the last.
    ::dds::topic::AnyTopic lookup_topicdescription(const std::string& name, uint32_t index) const;
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsBuiltinSubscriber> get_builtin_subscriber() const;

    // Time operations.
    ::dds::core::Time current_time() const;

    // Default QoS for child entities.
    ::dds::pub::qos::PublisherQos default_publisher_qos() const;
    void default_publisher_qos(const ::dds::pub::qos::PublisherQos& qos);
    ::dds::sub::qos::SubscriberQos default_subscriber_qos() const;
    void default_subscriber_qos(const ::dds::sub::qos::SubscriberQos& qos);
    ::dds::topic::qos::TopicQos default_topic_qos() const;
    void default_topic_qos(const ::dds::topic::qos::TopicQos& qos);

    // Get discovered participant data.
    ::dds::topic::ParticipantBuiltinTopicData get_discovered_participant_data(
        const ::dds::core::InstanceHandle& handle) const;

private:
    const zerodds_ZeroDdsDomainParticipantFactory* factory_;
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant_;
    mutable ::dds::domain::qos::DomainParticipantQos qos_;
    zerodds_ZeroDdsDomainParticipantListener* listener_reg_;
    bool closed_;
    mutable ::dds::core::detail::ChildTracker publishers_;
    mutable ::dds::core::detail::ChildTracker subscribers_;
    mutable ::dds::core::detail::ChildTracker topics_;
};

// Inline implementations (header-only library).

inline DomainParticipantDelegate::DomainParticipantDelegate(uint32_t domain_id)
    : factory_(0), participant_(), qos_(), listener_reg_(0), closed_(false) {
    factory_ = zerodds_dpf_get_instance();
    zerodds_ZeroDdsDomainParticipant* raw =
        zerodds_dpf_create_participant(factory_, domain_id, 0);
    if (raw == 0) {
        throw ::dds::core::Error("DomainParticipantDelegate::create failed");
    }
    participant_ = ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant>(
        raw, ParticipantDeleter(factory_));
}

inline DomainParticipantDelegate::DomainParticipantDelegate(
    uint32_t domain_id, const ::dds::domain::qos::DomainParticipantQos& qos)
    : factory_(0), participant_(), qos_(), listener_reg_(0), closed_(false) {
    factory_ = zerodds_dpf_get_instance();
    zerodds_ZeroDdsDomainParticipantQos native = ::dds::core::detail::to_native(qos);
    zerodds_ZeroDdsDomainParticipant* raw =
        zerodds_dpf_create_participant(factory_, domain_id, &native);
    if (raw == 0) {
        throw ::dds::core::Error("DomainParticipantDelegate::create with QoS failed");
    }
    participant_ = ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant>(
        raw, ParticipantDeleter(factory_));
}

inline DomainParticipantDelegate::~DomainParticipantDelegate() {
    if (!closed_ && participant_) {
        participant_.reset(); // Deleter will clean up
    }
}

inline void DomainParticipantDelegate::enable() {
    ::zerodds::psm::check_status(zerodds_dp_enable(participant_.get()), "DomainParticipantDelegate::enable");
}

inline ::dds::core::status::StatusMask DomainParticipantDelegate::status_changes() const {
    uint32_t changes = 0;
    ::zerodds::psm::check_status(zerodds_dp_get_status_changes(participant_.get(), &changes), "DomainParticipantDelegate::status_changes");
    return ::dds::core::status::StatusMask(changes);
}

inline ::dds::core::InstanceHandle DomainParticipantDelegate::instance_handle() const {
    uint64_t handle = 0;
    ::zerodds::psm::check_status(zerodds_dp_get_instance_handle(participant_.get(), &handle), "DomainParticipantDelegate::instance_handle");
    return ::dds::core::InstanceHandle(handle);
}

inline void DomainParticipantDelegate::close() {
    if (!closed_ && participant_) {
        participant_.reset(); // Deleter will clean up
        closed_ = true;
    }
}

inline void DomainParticipantDelegate::retain() {
    // No-op; shared_ptr lifetime management handles retention.
}

inline uint32_t DomainParticipantDelegate::domain_id() const {
    return zerodds_dp_get_domain_id(participant_.get());
}

inline void DomainParticipantDelegate::assert_liveliness() {
    int rc = zerodds_dp_assert_liveliness(participant_.get());
    ::zerodds::psm::check_status(rc, "DomainParticipantDelegate::assert_liveliness");
}

inline bool DomainParticipantDelegate::contains_entity(const ::dds::core::InstanceHandle& h) const {
    return zerodds_dp_contains_entity(participant_.get(), h.value()) != 0;
}

inline void DomainParticipantDelegate::ignore_participant(const ::dds::core::InstanceHandle& h) const {
    int rc = zerodds_dp_ignore_participant(participant_.get(), h.value());
    ::zerodds::psm::check_status(rc, "DomainParticipantDelegate::ignore_participant");
}

inline void DomainParticipantDelegate::ignore_topic(const ::dds::core::InstanceHandle& h) const {
    int rc = zerodds_dp_ignore_topic(participant_.get(), h.value());
    ::zerodds::psm::check_status(rc, "DomainParticipantDelegate::ignore_topic");
}

inline void DomainParticipantDelegate::ignore_publication(const ::dds::core::InstanceHandle& h) const {
    int rc = zerodds_dp_ignore_publication(participant_.get(), h.value());
    ::zerodds::psm::check_status(rc, "DomainParticipantDelegate::ignore_publication");
}

inline void DomainParticipantDelegate::ignore_subscription(const ::dds::core::InstanceHandle& h) const {
    int rc = zerodds_dp_ignore_subscription(participant_.get(), h.value());
    ::zerodds::psm::check_status(rc, "DomainParticipantDelegate::ignore_subscription");
}

inline void DomainParticipantDelegate::delete_contained_entities() {
    // The C API frees what a participant contains; the delegates of those entities would free
    // it again. They close first, publishers and subscribers with their writers and readers,
    // then the topics they used.
    publishers_.close_all();
    subscribers_.close_all();
    topics_.close_all();
    int rc = zerodds_dp_delete_contained_entities(participant_.get());
    ::zerodds::psm::check_status(rc, "DomainParticipantDelegate::delete_contained_entities");
}

inline ::dds::core::InstanceHandleSeq DomainParticipantDelegate::get_discovered_participants() const {
    std::vector<uint64_t> raw(64, 0);
    size_t count = 0;
    int rc;
    for (;;) {
        rc = zerodds_dp_get_discovered_participants(participant_.get(), &raw[0], &count, raw.size());
        if (rc != -8) { // ZERODDS_OUT_OF_RESOURCES
            break;
        }
        raw.resize(count);
    }
    ::zerodds::psm::check_status(rc, "DomainParticipantDelegate::get_discovered_participants");
    ::dds::core::InstanceHandleSeq out;
    out.reserve(count);
    for (size_t i = 0; i < count; ++i) {
        out.push_back(::dds::core::InstanceHandle(raw[i]));
    }
    return out;
}

inline ::dds::core::InstanceHandleSeq DomainParticipantDelegate::get_discovered_topics() const {
    std::vector<uint64_t> raw(64, 0);
    size_t count = 0;
    int rc;
    for (;;) {
        rc = zerodds_dp_get_discovered_topics(participant_.get(), &raw[0], &count, raw.size());
        if (rc != -8) {
            break;
        }
        raw.resize(count);
    }
    ::zerodds::psm::check_status(rc, "DomainParticipantDelegate::get_discovered_topics");
    ::dds::core::InstanceHandleSeq out;
    out.reserve(count);
    for (size_t i = 0; i < count; ++i) {
        out.push_back(::dds::core::InstanceHandle(raw[i]));
    }
    return out;
}

inline ::dds::topic::TopicBuiltinTopicData DomainParticipantDelegate::get_discovered_topic_data(
    const ::dds::core::InstanceHandle& handle) const {
    zerodds_ZeroDdsTopicBuiltinTopicData native = zerodds_ZeroDdsTopicBuiltinTopicData();
    int rc = zerodds_dp_get_discovered_topic_data(participant_.get(), handle.value(), &native);
    ::zerodds::psm::check_status(rc, "DomainParticipantDelegate::get_discovered_topic_data");
    ::dds::topic::TopicBuiltinTopicData data = ::dds::topic::TopicBuiltinTopicData::from_native(native);
    zerodds_topic_builtin_topic_data_free(&native);
    return data;
}

inline size_t DomainParticipantDelegate::discovered_publications_count() const {
    return zerodds_dp_discovered_publications_count(participant_.get());
}

inline size_t DomainParticipantDelegate::discovered_subscriptions_count() const {
    return zerodds_dp_discovered_subscriptions_count(participant_.get());
}

// QoS operations.
inline const ::dds::domain::qos::DomainParticipantQos& DomainParticipantDelegate::qos() const {
    qos_ = ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_dp_get_qos, participant_.get()),
        &::dds::core::detail::participant_qos_of, "DomainParticipantDelegate::qos",
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsDomainParticipantQos>()(&zerodds_ZeroDdsDomainParticipantQos::user_data));
    return qos_;
}

inline void DomainParticipantDelegate::qos(const ::dds::domain::qos::DomainParticipantQos& qos) {
    zerodds_ZeroDdsDomainParticipantQos native = ::dds::core::detail::to_native(qos);
    ::zerodds::psm::check_status(zerodds_dp_set_qos(participant_.get(), &native),
                              "DomainParticipantDelegate::qos");
    qos_ = qos;
}

// Listener operations.
inline void DomainParticipantDelegate::listener(::dds::domain::DomainParticipantListener* l,
                                                const ::dds::core::status::StatusMask& mask) {
    if (l) {
        listener_reg_ = new zerodds_ZeroDdsDomainParticipantListener();
        listener_reg_->user_data = l;
        listener_reg_->on_inconsistent_topic = dp_listener_on_inconsistent_topic;
        listener_reg_->on_data_on_readers = dp_listener_on_data_on_readers;
        ::zerodds::psm::check_status(
            zerodds_dp_set_listener(participant_.get(), listener_reg_, static_cast<uint32_t>(mask.to_ulong())),
            "DomainParticipantDelegate::listener");
    } else {
        ::zerodds::psm::check_status(
            zerodds_dp_set_listener(participant_.get(), 0, 0),
            "DomainParticipantDelegate::listener (clear)");
        delete listener_reg_;
        listener_reg_ = 0;
    }
}

inline ::dds::domain::DomainParticipantListener* DomainParticipantDelegate::listener() const {
    return listener_reg_ ? static_cast< ::dds::domain::DomainParticipantListener*>(listener_reg_->user_data) : 0;
}

inline ::dds::domain::qos::DomainParticipantQos
DomainParticipantDelegate::factory_default_participant_qos() {
    const zerodds_ZeroDdsDomainParticipantFactory* factory = zerodds_dpf_get_instance();
    return ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_dpf_get_default_participant_qos, factory),
        &::dds::core::detail::participant_qos_of, "DomainParticipant::default_participant_qos",
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsDomainParticipantQos>()(&zerodds_ZeroDdsDomainParticipantQos::user_data));
}

inline void DomainParticipantDelegate::set_factory_default_participant_qos(
    const ::dds::domain::qos::DomainParticipantQos& qos) {
    const zerodds_ZeroDdsDomainParticipantFactory* factory = zerodds_dpf_get_instance();
    zerodds_ZeroDdsDomainParticipantQos native = ::dds::core::detail::to_native(qos);
    ::zerodds::psm::check_status(zerodds_dpf_set_default_participant_qos(factory, &native),
                                 "DomainParticipant::default_participant_qos");
}

// Factory default QoS.
inline ::dds::domain::qos::DomainParticipantQos DomainParticipantDelegate::default_participant_qos() const {
    return ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_dpf_get_default_participant_qos, factory_),
        &::dds::core::detail::participant_qos_of, "DomainParticipantDelegate::default_participant_qos",
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsDomainParticipantQos>()(&zerodds_ZeroDdsDomainParticipantQos::user_data));
}

inline void DomainParticipantDelegate::default_participant_qos(const ::dds::domain::qos::DomainParticipantQos& qos) {
    zerodds_ZeroDdsDomainParticipantQos native = ::dds::core::detail::to_native(qos);
    ::zerodds::psm::check_status(zerodds_dpf_set_default_participant_qos(factory_, &native),
                              "DomainParticipantDelegate::default_participant_qos");
}

// Topic operations.
inline ::dds::topic::AnyTopic DomainParticipantDelegate::find_topic(const std::string& name) const {
    zerodds_ZeroDdsTopic* topic = zerodds_dp_find_topic(participant_.get(), name.c_str());
    if (!topic) {
        return ::dds::topic::AnyTopic(::dds::core::null);
    }
    return ::dds::topic::detail::make_native_any_topic(topic);
}

inline ::dds::topic::AnyTopic DomainParticipantDelegate::lookup_topicdescription(const std::string& name) const {
    zerodds_ZeroDdsTopic* topic = zerodds_dp_lookup_topicdescription(participant_.get(), name.c_str());
    if (!topic) {
        return ::dds::topic::AnyTopic(::dds::core::null);
    }
    return ::dds::topic::detail::make_native_any_topic(topic);
}

inline ::dds::topic::AnyTopic DomainParticipantDelegate::lookup_topicdescription(const std::string& name,
                                                                               uint32_t index) const {
    zerodds_ZeroDdsTopic* topic = zerodds_dp_lookup_topicdescription_at(participant_.get(), name.c_str(), index);
    if (!topic) {
        return ::dds::topic::AnyTopic(::dds::core::null);
    }
    return ::dds::topic::detail::make_native_any_topic(topic);
}

inline ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsBuiltinSubscriber>
DomainParticipantDelegate::get_builtin_subscriber() const {
    zerodds_ZeroDdsBuiltinSubscriber* bs = zerodds_dp_get_builtin_subscriber(participant_.get());
    return ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsBuiltinSubscriber>(bs, BuiltinSubscriberDeleter());
}

// Time operations.
inline ::dds::core::Time DomainParticipantDelegate::current_time() const {
    zerodds_ZeroDdsTime native = zerodds_ZeroDdsTime();
    ::zerodds::psm::check_status(zerodds_dp_get_current_time(participant_.get(), &native),
                              "DomainParticipantDelegate::current_time");
    return ::dds::core::Time(native.sec, native.nanosec);
}

// Default QoS for child entities.
inline ::dds::pub::qos::PublisherQos DomainParticipantDelegate::default_publisher_qos() const {
    return ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_dp_get_default_publisher_qos, participant_.get()),
        &::dds::core::detail::publisher_qos_of, "DomainParticipantDelegate::default_publisher_qos",
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsPublisherQos>()(&zerodds_ZeroDdsPublisherQos::group_data));
}

inline void DomainParticipantDelegate::default_publisher_qos(const ::dds::pub::qos::PublisherQos& qos) {
    ::dds::core::detail::PartitionPtrs part_ptrs;
    zerodds_ZeroDdsPublisherQos native = ::dds::core::detail::to_native(qos, part_ptrs);
    ::zerodds::psm::check_status(zerodds_dp_set_default_publisher_qos(participant_.get(), &native),
                              "DomainParticipantDelegate::default_publisher_qos");
}

inline ::dds::sub::qos::SubscriberQos DomainParticipantDelegate::default_subscriber_qos() const {
    return ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_dp_get_default_subscriber_qos, participant_.get()),
        &::dds::core::detail::subscriber_qos_of, "DomainParticipantDelegate::default_subscriber_qos",
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsSubscriberQos>()(&zerodds_ZeroDdsSubscriberQos::group_data));
}

inline void DomainParticipantDelegate::default_subscriber_qos(const ::dds::sub::qos::SubscriberQos& qos) {
    ::dds::core::detail::PartitionPtrs part_ptrs;
    zerodds_ZeroDdsSubscriberQos native = ::dds::core::detail::to_native(qos, part_ptrs);
    ::zerodds::psm::check_status(zerodds_dp_set_default_subscriber_qos(participant_.get(), &native),
                              "DomainParticipantDelegate::default_subscriber_qos");
}

inline ::dds::topic::qos::TopicQos DomainParticipantDelegate::default_topic_qos() const {
    return ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_dp_get_default_topic_qos, participant_.get()),
        &::dds::core::detail::topic_qos_of, "DomainParticipantDelegate::default_topic_qos",
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsTopicQos>()(&zerodds_ZeroDdsTopicQos::topic_data));
}

inline void DomainParticipantDelegate::default_topic_qos(const ::dds::topic::qos::TopicQos& qos) {
    zerodds_ZeroDdsTopicQos native = ::dds::core::detail::to_native(qos);
    ::zerodds::psm::check_status(zerodds_dp_set_default_topic_qos(participant_.get(), &native),
                              "DomainParticipantDelegate::default_topic_qos");
}

// Get discovered participant data.
inline ::dds::topic::ParticipantBuiltinTopicData DomainParticipantDelegate::get_discovered_participant_data(
    const ::dds::core::InstanceHandle& handle) const {
    zerodds_ZeroDdsParticipantBuiltinTopicData native = zerodds_ZeroDdsParticipantBuiltinTopicData();
    ::zerodds::psm::check_status(zerodds_dp_get_discovered_participant_data(participant_.get(),
                                                                          handle.value(), &native),
                              "DomainParticipantDelegate::get_discovered_participant_data");
    return ::dds::topic::ParticipantBuiltinTopicData::from_native(native);
}

} // namespace detail
} // namespace domain
} // namespace dds

#endif // ZERODDS_DDS_DOMAIN_DETAIL_DOMAINPARTICIPANTDELEGATE_HPP