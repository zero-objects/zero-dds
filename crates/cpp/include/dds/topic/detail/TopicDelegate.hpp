// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/detail/TopicDelegate.hpp — Delegate for Topic.

#ifndef ZERODDS_DDS_TOPIC_DETAIL_TOPICDELEGATE_HPP
#define ZERODDS_DDS_TOPIC_DETAIL_TOPICDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include <string>

#include "dds/core/Exception.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/topic/TopicTraits.hpp"
#include "dds/topic/detail/TypeObjectsOf.hpp"
#include "zerodds.h"

namespace dds {
namespace domain {
class DomainParticipant;
}

namespace topic {

template <typename T>
class TopicListener;

namespace detail {

// Helper to get type name with fallback for types without topic_type_support.
template <typename T>
inline std::string get_type_name_impl(::zerodds::cxx::true_type) {
    return ::dds::topic::topic_type_support<T>::type_name();
}

template <typename T>
inline std::string get_type_name_impl(::zerodds::cxx::false_type) {
    return "UnknownType";
}

template <typename T>
inline std::string get_type_name() {
    return get_type_name_impl<T>(::zerodds::cxx::integral_constant<
                                 bool, has_type_name< ::dds::topic::topic_type_support<T> >::value>());
}

/// The deleter of a shared native handle the caller keeps: frees nothing.
struct NullDeleter {
    template <typename P>
    void operator()(P*) const {}
};

/// The deleter of a native topic: deletes it from its participant.
struct TopicDeleter {
    explicit TopicDeleter(const ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant>& part)
        : part_(part) {}
    void operator()(zerodds_ZeroDdsTopic* t) const { zerodds_dp_delete_topic(part_.get(), t); }
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> part_;
};

/// `zerodds_topic_set_type_objects` as the `call` of `with_type_objects`.
struct SetTopicTypeObjects {
    explicit SetTopicTypeObjects(zerodds_ZeroDdsTopic* h) : h_(h) {}
    int operator()(const uint8_t* object, std::size_t len, const uint8_t* const* deps,
                   const std::size_t* lens, std::size_t n, int keyed, uint32_t ext) const {
        return zerodds_topic_set_type_objects(h_, object, len, deps, lens, n, keyed, ext);
    }
    zerodds_ZeroDdsTopic* h_;
};

/// The QoS of a native topic.
inline ::dds::topic::qos::TopicQos fetch_topic_qos(zerodds_ZeroDdsTopic* t, const char* what) {
    return ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_topic_get_qos, t),
        &::dds::core::detail::topic_qos_of, what,
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsTopicQos>()(
            &zerodds_ZeroDdsTopicQos::topic_data));
}

/// Delegate for Topic<T> (DDS-PSM-Cxx 1.0 §7.8).
template <typename T>
class TopicDelegate {
public:
    TopicDelegate(const ::dds::domain::DomainParticipant& dp, const std::string& name);
    TopicDelegate(const ::dds::domain::DomainParticipant& dp, const std::string& name,
                  const std::string& type_name);
    TopicDelegate(const ::dds::domain::DomainParticipant& dp, const std::string& name,
                  const ::dds::topic::qos::TopicQos& qos);
    TopicDelegate(const ::dds::domain::DomainParticipant& dp, const std::string& name,
                  const std::string& type_name, const ::dds::topic::qos::TopicQos& qos);
    /// Borrow constructor for the listener bridge: wraps a topic the caller
    /// owns, without deleting it.
    explicit TopicDelegate(zerodds_ZeroDdsTopic* h);
    ~TopicDelegate();

    // Non-copyable, non-movable (owned by shared_ptr).
    ZERODDS_NONCOPYABLE(TopicDelegate)
#if ZERODDS_CXX11
    TopicDelegate(TopicDelegate&&) = delete;
    TopicDelegate& operator=(TopicDelegate&&) = delete;
#endif

    zerodds_ZeroDdsTopic* native_handle() const { return handle_; }
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsTopic> topic() const { return topic_; }
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant() const { return participant_; }
    const std::string& name() const { return name_; }
    const std::string& type_name() const { return type_name_; }

    // Entity operations.
    void enable();
    ::dds::core::status::StatusMask status_changes() const;
    ::dds::core::InstanceHandle instance_handle() const;
    void close();
    void retain();

    /// Registers a new topic with its participant (`delete_contained_entities` of the participant).
    void enrol(const ::dds::domain::DomainParticipant& dp, const ::zerodds::cxx::shared_ptr<TopicDelegate>& self);

    // Topic operations.
    const ::dds::core::status::InconsistentTopicStatus& inconsistent_topic_status();
    const ::dds::topic::qos::TopicQos& qos() const;
    void qos(const ::dds::topic::qos::TopicQos& qos);

    // Listener support.
    void attach_listener(const zerodds_ZeroDdsTopicListener& vt, uint32_t mask);
    void detach_listener();
    /// Installs `l` for `mask`, `nullptr` removes it. Defined in Topic.hpp.
    void listener(::dds::topic::TopicListener<T>* l, const ::dds::core::status::StatusMask& mask);
    void set_listener(::dds::topic::TopicListener<T>* l) { listener_ = l; }
    ::dds::topic::TopicListener<T>* listener() const { return listener_; }

private:
    void init(const ::dds::domain::DomainParticipant& dp, const std::string& name,
              const std::string& type_name, const ::dds::topic::qos::TopicQos* qos);

    static zerodds_ZeroDdsTopic* create_topic(zerodds_ZeroDdsDomainParticipant* dp,
                                               const std::string& name,
                                               const std::string& type_name,
                                               const zerodds_ZeroDdsTopicQos* qos);

    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant_;
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsTopic> topic_;
    zerodds_ZeroDdsTopic* handle_;
    std::string name_;
    std::string type_name_;
    mutable ::dds::topic::qos::TopicQos qos_;
    ::dds::topic::TopicListener<T>* listener_;
    zerodds_ZeroDdsTopicListener* listener_reg_;
    ::dds::core::status::InconsistentTopicStatus inconsistent_status_;
    bool closed_;
};

// Inline implementations (header-only library).

template <typename T>
inline void TopicDelegate<T>::init(const ::dds::domain::DomainParticipant& dp,
                                   const std::string& name, const std::string& type_name,
                                   const ::dds::topic::qos::TopicQos* qos) {
    participant_ = dp.delegate()->participant();
    zerodds_ZeroDdsTopicQos native = zerodds_ZeroDdsTopicQos();
    if (qos != 0) {
        native = ::dds::core::detail::to_native(*qos);
    }
    handle_ = create_topic(participant_.get(), name, type_name, qos != 0 ? &native : 0);
    if (handle_ == 0) {
        throw ::dds::core::Error("TopicDelegate::create failed");
    }
    name_ = name;
    type_name_ = type_name;
    // The topic knows its type, so that a MultiTopic can name the members of the topics of its
    // FROM clause. A hand-written support has no object and leaves the topic untyped.
    {
        zerodds_ZeroDdsTopic* h = handle_;
        // A refusal leaves the topic untyped; a MultiTopic that names it then fails with the
        // reason, and every other use of the topic is as before.
        (void)with_type_objects<T>(type_name, SetTopicTypeObjects(h));
    }
    topic_ = ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsTopic>(handle_, TopicDeleter(participant_));
}

template <typename T>
inline TopicDelegate<T>::TopicDelegate(const ::dds::domain::DomainParticipant& dp,
                                       const std::string& name)
    : participant_(), topic_(), handle_(0), name_(), type_name_(), qos_(), listener_(0),
      listener_reg_(0), inconsistent_status_(), closed_(false) {
    init(dp, name, get_type_name<T>(), 0);
}

template <typename T>
inline TopicDelegate<T>::TopicDelegate(const ::dds::domain::DomainParticipant& dp,
                                       const std::string& name, const std::string& type_name)
    : participant_(), topic_(), handle_(0), name_(), type_name_(), qos_(), listener_(0),
      listener_reg_(0), inconsistent_status_(), closed_(false) {
    init(dp, name, type_name, 0);
}

template <typename T>
inline TopicDelegate<T>::TopicDelegate(const ::dds::domain::DomainParticipant& dp,
                                       const std::string& name,
                                       const ::dds::topic::qos::TopicQos& qos)
    : participant_(), topic_(), handle_(0), name_(), type_name_(), qos_(), listener_(0),
      listener_reg_(0), inconsistent_status_(), closed_(false) {
    init(dp, name, get_type_name<T>(), &qos);
}

template <typename T>
inline TopicDelegate<T>::TopicDelegate(const ::dds::domain::DomainParticipant& dp,
                                       const std::string& name, const std::string& type_name,
                                       const ::dds::topic::qos::TopicQos& qos)
    : participant_(), topic_(), handle_(0), name_(), type_name_(), qos_(), listener_(0),
      listener_reg_(0), inconsistent_status_(), closed_(false) {
    init(dp, name, type_name, &qos);
}

template <typename T>
inline TopicDelegate<T>::TopicDelegate(zerodds_ZeroDdsTopic* h)
    : participant_(), topic_(), handle_(h), name_(), type_name_(), qos_(), listener_(0),
      listener_reg_(0), inconsistent_status_(), closed_(false) {
    topic_ = ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsTopic>(h, NullDeleter());
    participant_ = ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant>(
        zerodds_topic_get_participant(h), NullDeleter());
    if (char* n = zerodds_topic_get_name(h)) {
        name_ = n;
        zerodds_string_free(n);
    }
    if (char* n = zerodds_topic_get_type_name(h)) {
        type_name_ = n;
        zerodds_string_free(n);
    }
}

template <typename T>
inline zerodds_ZeroDdsTopic* TopicDelegate<T>::create_topic(
    zerodds_ZeroDdsDomainParticipant* dp, const std::string& name,
    const std::string& type_name, const zerodds_ZeroDdsTopicQos* qos) {
    return zerodds_dp_create_topic(dp, name.c_str(), type_name.c_str(), qos);
}

template <typename T>
inline TopicDelegate<T>::~TopicDelegate() {
    if (!closed_ && handle_ != 0) {
        detach_listener();
        close();
    }
}

template <typename T>
inline void TopicDelegate<T>::enable() {
    ::zerodds::psm::check_status(zerodds_topic_enable(topic_.get()), "TopicDelegate::enable");
}

template <typename T>
inline ::dds::core::status::StatusMask TopicDelegate<T>::status_changes() const {
    uint32_t changes = 0;
    ::zerodds::psm::check_status(zerodds_topic_get_status_changes(topic_.get(), &changes), "TopicDelegate::status_changes");
    return ::dds::core::status::StatusMask(changes);
}

template <typename T>
inline ::dds::core::InstanceHandle TopicDelegate<T>::instance_handle() const {
    uint64_t handle = 0;
    ::zerodds::psm::check_status(zerodds_topic_get_instance_handle(topic_.get(), &handle), "TopicDelegate::instance_handle");
    return ::dds::core::InstanceHandle(handle);
}

template <typename T>
inline void TopicDelegate<T>::attach_listener(const zerodds_ZeroDdsTopicListener& vt,
                                              uint32_t mask) {
    detach_listener();
    listener_reg_ = new zerodds_ZeroDdsTopicListener(vt);
    ::zerodds::psm::check_status(zerodds_topic_set_listener(topic_.get(), listener_reg_, mask),
                              "TopicDelegate::attach_listener");
}

template <typename T>
inline void TopicDelegate<T>::detach_listener() {
    if (listener_reg_ != 0) {
        zerodds_topic_set_listener(topic_.get(), 0, 0);
        delete listener_reg_;
        listener_reg_ = 0;
    }
}

template <typename T>
inline void TopicDelegate<T>::close() {
    if (!closed_ && topic_) {
        topic_.reset(); // Deleter will clean up
        closed_ = true;
    }
}

template <typename T>
inline void TopicDelegate<T>::retain() {
    // No-op; shared_ptr lifetime management handles retention.
}

template <typename T>
inline const ::dds::core::status::InconsistentTopicStatus& TopicDelegate<T>::inconsistent_topic_status() {
    zerodds_ZeroDdsInconsistentTopicStatus native = zerodds_ZeroDdsInconsistentTopicStatus();
    ::zerodds::psm::check_status(zerodds_topic_get_inconsistent_topic_status(topic_.get(), &native),
                              "TopicDelegate::inconsistent_topic_status");
    inconsistent_status_.total_count(native.total_count);
    inconsistent_status_.total_count_change(native.total_count_change);
    return inconsistent_status_;
}

template <typename T>
inline const ::dds::topic::qos::TopicQos& TopicDelegate<T>::qos() const {
    qos_ = fetch_topic_qos(topic_.get(), "TopicDelegate::qos");
    return qos_;
}

template <typename T>
inline void TopicDelegate<T>::qos(const ::dds::topic::qos::TopicQos& q) {
    zerodds_ZeroDdsTopicQos native = ::dds::core::detail::to_native(q);
    ::zerodds::psm::check_status(zerodds_topic_set_qos(topic_.get(), &native), "TopicDelegate::qos");
    qos_ = q;
}

} // namespace detail
} // namespace topic
} // namespace dds

// Include full DomainParticipant definition for method implementations.
#include "dds/domain/DomainParticipant.hpp"

#endif // ZERODDS_DDS_TOPIC_DETAIL_TOPICDELEGATE_HPP