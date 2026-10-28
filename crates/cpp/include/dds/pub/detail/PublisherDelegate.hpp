// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/detail/PublisherDelegate.hpp — Delegate for Publisher.

#ifndef ZERODDS_DDS_PUB_DETAIL_PUBLISHERDELEGATE_HPP
#define ZERODDS_DDS_PUB_DETAIL_PUBLISHERDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"

#include "dds/core/Exception.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/domain/detail/DomainParticipantDelegate.hpp"
#include "dds/pub/PublisherListener.hpp"
#include "dds/core/detail/EntityTracker.hpp"
#include "dds/pub/AnyDataWriter.hpp"
#include "zerodds.h"

namespace dds {
namespace pub {
namespace detail {

/// A deleter that does nothing: for a native handle the caller keeps owning.
struct NoopDeleter {
    template <typename P>
    void operator()(P*) const {}
};

/// The deleter of the shared native publisher: frees what the publisher contains, then the
/// publisher. Holds the participant so that it outlives the publisher.
struct PublisherDeleter {
    explicit PublisherDeleter(const ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant>& part)
        : part_(part) {}
    void operator()(zerodds_ZeroDdsPublisher* p) const {
        zerodds_pub_delete_contained_entities(p);
        zerodds_dp_delete_publisher(part_.get(), p);
    }
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> part_;
};

class PublisherDelegate {
public:
    explicit PublisherDelegate(const ::dds::domain::DomainParticipant& dp);
    PublisherDelegate(const ::dds::domain::DomainParticipant& dp,
                      const ::dds::pub::qos::PublisherQos& qos);
    /// Borrow constructor for the listener bridge: wraps a publisher the
    /// caller owns, without deleting it.
    explicit PublisherDelegate(zerodds_ZeroDdsPublisher* h);
    ~PublisherDelegate();

    // Non-copyable, non-movable (owned by shared_ptr).
    ZERODDS_NONCOPYABLE(PublisherDelegate)
#if ZERODDS_CXX11
    PublisherDelegate(PublisherDelegate&&) = delete;
    PublisherDelegate& operator=(PublisherDelegate&&) = delete;
#endif

    zerodds_ZeroDdsPublisher* native_handle() const { return publisher_.get(); }
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsPublisher> publisher() const { return publisher_; }
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant() const { return participant_; }

    // Entity operations.
    void enable();
    ::dds::core::status::StatusMask status_changes() const;
    ::dds::core::InstanceHandle instance_handle() const;
    void close();
    void retain();

    /// Closes the writers of this publisher the application still holds, and registers a new
    /// publisher with its participant (`delete_contained_entities` of the participant).
    void delete_contained_entities();
    void enrol(const ::dds::domain::DomainParticipant& dp,
               const ::zerodds::cxx::shared_ptr<PublisherDelegate>& self);

    // Publisher operations.
    void suspend_publications();
    void resume_publications();
    void begin_coherent_changes();
    void end_coherent_changes();
    void wait_for_acknowledgments(const ::dds::core::Duration& d);

    const ::dds::pub::qos::PublisherQos& qos() const;
    void qos(const ::dds::pub::qos::PublisherQos& q);
    ::dds::pub::qos::DataWriterQos default_writer_qos() const;
    void default_writer_qos(const ::dds::pub::qos::DataWriterQos& q);

    /// The writers of this publisher the application still holds
    /// (`dds::pub::find`).
    ::dds::core::detail::EntityTracker< ::dds::pub::AnyDataWriter>& writers() const { return writers_; }

    // Listener support.
    void attach_listener(const zerodds_ZeroDdsPublisherListener& vt, uint32_t mask);
    void detach_listener();
    /// Installs `l` for `mask`, `nullptr` removes it. Defined in Publisher.hpp.
    void listener(::dds::pub::PublisherListener* l, const ::dds::core::status::StatusMask& mask);
    void set_listener(::dds::pub::PublisherListener* l) { listener_ = l; }
    ::dds::pub::PublisherListener* listener() const { return listener_; }

private:
    void init(const ::dds::domain::DomainParticipant& dp, const zerodds_ZeroDdsPublisherQos* qos);

    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant_;
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsPublisher> publisher_;
    mutable ::dds::pub::qos::PublisherQos qos_;
    mutable ::dds::core::detail::EntityTracker< ::dds::pub::AnyDataWriter> writers_;
    zerodds_ZeroDdsPublisherListener* listener_reg_;
    ::dds::pub::PublisherListener* listener_;
    bool closed_;
};

inline void PublisherDelegate::init(const ::dds::domain::DomainParticipant& dp,
                                    const zerodds_ZeroDdsPublisherQos* qos) {
    participant_ = dp.delegate()->participant();
    zerodds_ZeroDdsPublisher* raw = zerodds_dp_create_publisher(participant_.get(), qos);
    if (raw == 0) {
        throw ::dds::core::Error("PublisherDelegate::create failed");
    }
    publisher_ = ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsPublisher>(raw, PublisherDeleter(participant_));
}

inline PublisherDelegate::PublisherDelegate(const ::dds::domain::DomainParticipant& dp)
    : participant_(), publisher_(), qos_(), writers_(), listener_reg_(0), listener_(0), closed_(false) {
    init(dp, 0);
}

inline PublisherDelegate::PublisherDelegate(const ::dds::domain::DomainParticipant& dp,
                                            const ::dds::pub::qos::PublisherQos& qos)
    : participant_(), publisher_(), qos_(), writers_(), listener_reg_(0), listener_(0), closed_(false) {
    ::dds::core::detail::PartitionPtrs part_ptrs;
    zerodds_ZeroDdsPublisherQos native = ::dds::core::detail::to_native(qos, part_ptrs);
    init(dp, &native);
}

inline PublisherDelegate::PublisherDelegate(zerodds_ZeroDdsPublisher* h)
    : participant_(), publisher_(), qos_(), writers_(), listener_reg_(0), listener_(0), closed_(false) {
    publisher_ = ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsPublisher>(h, NoopDeleter());
    participant_ = ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant>(
        zerodds_pub_get_participant(h), NoopDeleter());
}

inline PublisherDelegate::~PublisherDelegate() {
    if (!closed_ && publisher_) {
        detach_listener();
        publisher_.reset(); // Deleter will clean up
    }
}

inline void PublisherDelegate::enable() {
    ::zerodds::psm::check_status(zerodds_pub_enable(publisher_.get()), "PublisherDelegate::enable");
}

inline ::dds::core::status::StatusMask PublisherDelegate::status_changes() const {
    uint32_t changes = 0;
    ::zerodds::psm::check_status(zerodds_pub_get_status_changes(publisher_.get(), &changes),
                              "PublisherDelegate::status_changes");
    return ::dds::core::status::StatusMask(changes);
}

inline ::dds::core::InstanceHandle PublisherDelegate::instance_handle() const {
    uint64_t handle = 0;
    ::zerodds::psm::check_status(zerodds_pub_get_instance_handle(publisher_.get(), &handle),
                              "PublisherDelegate::instance_handle");
    return ::dds::core::InstanceHandle(handle);
}

inline void PublisherDelegate::close() {
    if (!closed_ && publisher_) {
        detach_listener();
        publisher_.reset(); // Deleter will clean up
        closed_ = true;
    }
}

inline void PublisherDelegate::retain() {
    // No-op; shared_ptr lifetime management handles retention.
}

inline void PublisherDelegate::suspend_publications() {
    ::zerodds::psm::check_status(zerodds_pub_suspend_publications(publisher_.get()),
                              "PublisherDelegate::suspend_publications");
}

inline void PublisherDelegate::resume_publications() {
    ::zerodds::psm::check_status(zerodds_pub_resume_publications(publisher_.get()),
                              "PublisherDelegate::resume_publications");
}

inline void PublisherDelegate::begin_coherent_changes() {
    ::zerodds::psm::check_status(zerodds_pub_begin_coherent_changes(publisher_.get()),
                              "PublisherDelegate::begin_coherent_changes");
}

inline void PublisherDelegate::end_coherent_changes() {
    ::zerodds::psm::check_status(zerodds_pub_end_coherent_changes(publisher_.get()),
                              "PublisherDelegate::end_coherent_changes");
}

inline void PublisherDelegate::wait_for_acknowledgments(const ::dds::core::Duration& d) {
    uint64_t ms = ::dds::core::detail::is_infinite(d) ? ZERODDS_BLOCKING_INFINITE
                                  : static_cast<uint64_t>(d.to_millisecs());
    int rc = zerodds_pub_wait_for_acknowledgments(publisher_.get(), ms);
    if (rc == -4) throw ::dds::core::TimeoutError("PublisherDelegate::wait_for_acks");
    ::zerodds::psm::check_status(rc, "PublisherDelegate::wait_for_acknowledgments");
}

inline const ::dds::pub::qos::PublisherQos& PublisherDelegate::qos() const {
    qos_ = ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_pub_get_qos, publisher_.get()),
        &::dds::core::detail::publisher_qos_of, "PublisherDelegate::qos",
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsPublisherQos>()(&zerodds_ZeroDdsPublisherQos::group_data));
    return qos_;
}

inline void PublisherDelegate::qos(const ::dds::pub::qos::PublisherQos& q) {
    ::dds::core::detail::PartitionPtrs part_ptrs;
    zerodds_ZeroDdsPublisherQos native = ::dds::core::detail::to_native(q, part_ptrs);
    ::zerodds::psm::check_status(zerodds_pub_set_qos(publisher_.get(), &native),
                              "PublisherDelegate::qos");
    qos_ = q;
}

inline ::dds::pub::qos::DataWriterQos PublisherDelegate::default_writer_qos() const {
    return ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_pub_get_default_datawriter_qos,
                                               publisher_.get()),
        &::dds::core::detail::datawriter_qos_of, "PublisherDelegate::default_writer_qos",
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsDataWriterQos>()(&zerodds_ZeroDdsDataWriterQos::user_data)(&zerodds_ZeroDdsDataWriterQos::topic_data)(&zerodds_ZeroDdsDataWriterQos::group_data));
}

inline void PublisherDelegate::default_writer_qos(const ::dds::pub::qos::DataWriterQos& q) {
    zerodds_ZeroDdsDataWriterQos native = ::dds::core::detail::to_native(q);
    ::zerodds::psm::check_status(zerodds_pub_set_default_datawriter_qos(publisher_.get(), &native),
                              "PublisherDelegate::default_writer_qos");
}

inline void PublisherDelegate::attach_listener(const zerodds_ZeroDdsPublisherListener& vt,
                                               uint32_t mask) {
    detach_listener();
    listener_reg_ = new zerodds_ZeroDdsPublisherListener(vt);
    ::zerodds::psm::check_status(zerodds_pub_set_listener(publisher_.get(), listener_reg_, mask),
                              "PublisherDelegate::attach_listener");
}

inline void PublisherDelegate::detach_listener() {
    if (listener_reg_ != 0) {
        zerodds_pub_set_listener(publisher_.get(), 0, 0);
        delete listener_reg_;
        listener_reg_ = 0;
    }
}

} // namespace detail
} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_DETAIL_PUBLISHERDELEGATE_HPP
