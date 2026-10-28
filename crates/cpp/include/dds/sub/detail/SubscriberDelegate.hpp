// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/detail/SubscriberDelegate.hpp — Delegate for Subscriber.

#ifndef ZERODDS_DDS_SUB_DETAIL_SUBSCRIBERDELEGATE_HPP
#define ZERODDS_DDS_SUB_DETAIL_SUBSCRIBERDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include <memory>

#include "dds/core/Exception.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/domain/detail/DomainParticipantDelegate.hpp"
#include "dds/sub/SubscriberListener.hpp"
#include "dds/core/detail/EntityTracker.hpp"
#include "dds/sub/AnyDataReader.hpp"
#include "dds/sub/detail/QosFetch.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {
namespace detail {

class SubscriberDelegate {
public:
    explicit SubscriberDelegate(const ::dds::domain::DomainParticipant& dp);
    SubscriberDelegate(const ::dds::domain::DomainParticipant& dp,
                       const ::dds::sub::qos::SubscriberQos& qos);
    /// Borrow constructor for the listener bridge: wraps a subscriber the
    /// caller owns, without deleting it.
    explicit SubscriberDelegate(zerodds_ZeroDdsSubscriber* h);
    /// The built-in subscriber of `dp` (DDS 1.4 §2.2.5), which `dds::sub::builtin_subscriber`
    /// returns as a `dds::sub::Subscriber`. It holds the built-in readers and no other entity:
    /// `native_handle()` is null.
    SubscriberDelegate(const ::dds::domain::DomainParticipant& dp,
                       ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsBuiltinSubscriber> builtin);
    ~SubscriberDelegate();

    // Non-copyable, non-movable (owned by shared_ptr).
    ZERODDS_NONCOPYABLE(SubscriberDelegate)
#if ZERODDS_CXX11
    SubscriberDelegate(SubscriberDelegate&&) = delete;
    SubscriberDelegate& operator=(SubscriberDelegate&&) = delete;
#endif

    zerodds_ZeroDdsSubscriber* native_handle() const { return subscriber_.get(); }
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsSubscriber> subscriber() const { return subscriber_; }
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant() const { return participant_; }
    /// The C handle when this is the built-in subscriber of a participant, else null.
    zerodds_ZeroDdsBuiltinSubscriber* builtin_handle() const { return builtin_.get(); }

    // Entity operations.
    void enable();
    ::dds::core::status::StatusMask status_changes() const;
    ::dds::core::InstanceHandle instance_handle() const;
    void close();
    void retain();

    /// Closes the readers of this subscriber the application still holds, and registers a new
    /// subscriber with its participant (`delete_contained_entities` of the participant).
    void delete_contained_entities();
    void enrol(const ::dds::domain::DomainParticipant& dp,
               const ::zerodds::cxx::shared_ptr<SubscriberDelegate>& self);

    // Subscriber operations.
    void begin_access();
    void end_access();
    void notify_datareaders();

    const ::dds::sub::qos::SubscriberQos& qos() const;
    void qos(const ::dds::sub::qos::SubscriberQos& q);
    ::dds::sub::qos::DataReaderQos default_datareader_qos() const;
    void default_datareader_qos(const ::dds::sub::qos::DataReaderQos& q);

    // Listener support.
    /// The readers of this subscriber the application still holds
    /// (`dds::sub::find`).
    ::dds::core::detail::EntityTracker< ::dds::sub::AnyDataReader>& readers() const { return readers_; }

    void attach_listener(const zerodds_ZeroDdsSubscriberListener& vt, uint32_t mask);
    void detach_listener();
    /// Installs `l` for `mask`, `nullptr` removes it. Defined in Subscriber.hpp.
    void listener(::dds::sub::SubscriberListener* l, const ::dds::core::status::StatusMask& mask);
    void set_listener(::dds::sub::SubscriberListener* l) { listener_ = l; }
    ::dds::sub::SubscriberListener* listener() const { return listener_; }

private:
    void init(const ::dds::domain::DomainParticipant& dp, const zerodds_ZeroDdsSubscriberQos* qos);

    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant_;
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsSubscriber> subscriber_;
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsBuiltinSubscriber> builtin_;
    mutable ::dds::sub::qos::SubscriberQos qos_;
    mutable ::dds::core::detail::EntityTracker< ::dds::sub::AnyDataReader> readers_;
    zerodds_ZeroDdsSubscriberListener* listener_reg_;
    ::dds::sub::SubscriberListener* listener_;
    bool closed_;
};

/// Deletes a subscriber the delegate created, with its readers, before the participant goes.
class SubscriberDeleter {
public:
    explicit SubscriberDeleter(
        const ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant>& participant)
        : participant_(participant) {}
    void operator()(zerodds_ZeroDdsSubscriber* s) const {
        zerodds_sub_delete_contained_entities(s);
        zerodds_dp_delete_subscriber(participant_.get(), s);
    }

private:
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant_;
};

/// The deleter of a handle the caller keeps owning.
template <typename X>
struct NoopDeleter {
    void operator()(X*) const {}
};

inline void SubscriberDelegate::init(const ::dds::domain::DomainParticipant& dp,
                                     const zerodds_ZeroDdsSubscriberQos* qos) {
    participant_ = dp.delegate()->participant();
    zerodds_ZeroDdsSubscriber* raw = zerodds_dp_create_subscriber(participant_.get(), qos);
    if (raw == 0) {
        throw ::dds::core::Error("SubscriberDelegate::create failed");
    }
    subscriber_ = ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsSubscriber>(
        raw, SubscriberDeleter(participant_));
}

inline SubscriberDelegate::SubscriberDelegate(const ::dds::domain::DomainParticipant& dp)
    : listener_reg_(0), listener_(0), closed_(false) {
    init(dp, 0);
}

inline SubscriberDelegate::SubscriberDelegate(const ::dds::domain::DomainParticipant& dp,
                                              const ::dds::sub::qos::SubscriberQos& qos)
    : listener_reg_(0), listener_(0), closed_(false) {
    ::dds::core::detail::PartitionPtrs part_ptrs;
    const zerodds_ZeroDdsSubscriberQos native = ::dds::core::detail::to_native(qos, part_ptrs);
    init(dp, &native);
}

inline SubscriberDelegate::SubscriberDelegate(zerodds_ZeroDdsSubscriber* h)
    : listener_reg_(0), listener_(0), closed_(false) {
    subscriber_ = ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsSubscriber>(
        h, NoopDeleter<zerodds_ZeroDdsSubscriber>());
    participant_ = ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant>(
        zerodds_sub_get_participant(h), NoopDeleter<zerodds_ZeroDdsDomainParticipant>());
}

inline SubscriberDelegate::SubscriberDelegate(
    const ::dds::domain::DomainParticipant& dp,
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsBuiltinSubscriber> builtin)
    : participant_(dp.delegate()->participant()),
      builtin_(ZERODDS_MOVE(builtin)),
      listener_reg_(0),
      listener_(0),
      closed_(false) {
    if (!builtin_) {
        throw ::dds::core::Error("SubscriberDelegate: the participant has no built-in subscriber");
    }
}

inline SubscriberDelegate::~SubscriberDelegate() {
    if (!closed_ && subscriber_) {
        detach_listener();
        subscriber_.reset(); // Deleter will clean up
    }
}

inline void SubscriberDelegate::enable() {
    ::zerodds::psm::check_status(zerodds_sub_enable(subscriber_.get()), "SubscriberDelegate::enable");
}

inline ::dds::core::status::StatusMask SubscriberDelegate::status_changes() const {
    uint32_t changes = 0;
    ::zerodds::psm::check_status(zerodds_sub_get_status_changes(subscriber_.get(), &changes),
                              "SubscriberDelegate::status_changes");
    return ::dds::core::status::StatusMask(changes);
}

inline ::dds::core::InstanceHandle SubscriberDelegate::instance_handle() const {
    uint64_t handle = 0;
    ::zerodds::psm::check_status(zerodds_sub_get_instance_handle(subscriber_.get(), &handle),
                              "SubscriberDelegate::instance_handle");
    return ::dds::core::InstanceHandle(handle);
}

inline void SubscriberDelegate::close() {
    if (!closed_ && subscriber_) {
        detach_listener();
        subscriber_.reset(); // Deleter will clean up
        closed_ = true;
    }
}

inline void SubscriberDelegate::retain() {
    // No-op; shared_ptr lifetime management handles retention.
}

inline void SubscriberDelegate::begin_access() {
    ::zerodds::psm::check_status(zerodds_sub_begin_access(subscriber_.get()),
                              "SubscriberDelegate::begin_access");
}

inline void SubscriberDelegate::end_access() {
    ::zerodds::psm::check_status(zerodds_sub_end_access(subscriber_.get()),
                              "SubscriberDelegate::end_access");
}

inline void SubscriberDelegate::notify_datareaders() {
    ::zerodds::psm::check_status(zerodds_sub_notify_datareaders(subscriber_.get()),
                              "SubscriberDelegate::notify_datareaders");
}

inline const ::dds::sub::qos::SubscriberQos& SubscriberDelegate::qos() const {
    qos_ = fetch_subscriber_qos(subscriber_.get(), "SubscriberDelegate::qos");
    return qos_;
}

inline void SubscriberDelegate::qos(const ::dds::sub::qos::SubscriberQos& q) {
    ::dds::core::detail::PartitionPtrs part_ptrs;
    zerodds_ZeroDdsSubscriberQos native = ::dds::core::detail::to_native(q, part_ptrs);
    ::zerodds::psm::check_status(zerodds_sub_set_qos(subscriber_.get(), &native),
                              "SubscriberDelegate::qos");
    qos_ = q;
}

inline ::dds::sub::qos::DataReaderQos SubscriberDelegate::default_datareader_qos() const {
    return fetch_default_datareader_qos(subscriber_.get(),
                                        "SubscriberDelegate::default_datareader_qos");
}

inline void SubscriberDelegate::default_datareader_qos(const ::dds::sub::qos::DataReaderQos& q) {
    zerodds_ZeroDdsDataReaderQos native = ::dds::core::detail::to_native(q);
    ::zerodds::psm::check_status(zerodds_sub_set_default_datareader_qos(subscriber_.get(), &native),
                              "SubscriberDelegate::default_datareader_qos");
}

inline void SubscriberDelegate::attach_listener(const zerodds_ZeroDdsSubscriberListener& vt,
                                                uint32_t mask) {
    detach_listener();
    listener_reg_ = new zerodds_ZeroDdsSubscriberListener(vt);
    ::zerodds::psm::check_status(zerodds_sub_set_listener(subscriber_.get(), listener_reg_, mask),
                              "SubscriberDelegate::attach_listener");
}

inline void SubscriberDelegate::detach_listener() {
    if (listener_reg_ != 0) {
        zerodds_sub_set_listener(subscriber_.get(), 0, 0);
        delete listener_reg_;
        listener_reg_ = 0;
    }
}

} // namespace detail
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DETAIL_SUBSCRIBERDELEGATE_HPP
