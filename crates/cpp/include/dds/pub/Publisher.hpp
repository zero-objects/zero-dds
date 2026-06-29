// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/Publisher.hpp + DataWriter — DDS-PSM-Cxx 1.0 §7.5.14.

#ifndef ZERODDS_DDS_PUB_PUBLISHER_HPP
#define ZERODDS_DDS_PUB_PUBLISHER_HPP

#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/Status.hpp"
#include "dds/core/Time.hpp"
#include "dds/core/qos.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/domain/DomainParticipant.hpp"
#include "dds/topic/Topic.hpp"
#include "zerodds.h"

namespace dds {
namespace pub {

/// Publisher (Spec §7.5.14.1).
class Publisher {
public:
    /// Default-constructed via Participant + default QoS.
    explicit Publisher(::dds::domain::DomainParticipant& dp)
        : participant_(dp.native_handle()) {
        handle_ = zerodds_dp_create_publisher(participant_, nullptr);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("Publisher::create failed");
        }
    }
    /// With QoS — converted into C-FFI form via `qos_bridge.hpp`.
    Publisher(::dds::domain::DomainParticipant& dp, const ::dds::core::PublisherQos& qos)
        : participant_(dp.native_handle()) {
        ::dds::core::detail::PartitionPtrs part;
        auto native = ::dds::core::detail::to_native(qos, part);
        handle_ = zerodds_dp_create_publisher(participant_, &native);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("Publisher::create with QoS failed");
        }
    }

    Publisher(const Publisher&) = delete;
    Publisher& operator=(const Publisher&) = delete;
    Publisher(Publisher&& o) noexcept
        : handle_(o.handle_), participant_(o.participant_) {
        o.handle_ = nullptr;
        o.participant_ = nullptr;
    }
    Publisher& operator=(Publisher&& o) noexcept {
        if (this != &o) {
            close();
            handle_ = o.handle_;
            participant_ = o.participant_;
            o.handle_ = nullptr;
            o.participant_ = nullptr;
        }
        return *this;
    }
    ~Publisher() { close(); }

    /// `suspend_publications`.
    void suspend_publications() {
        ::dds::core::check_status(zerodds_pub_suspend_publications(handle_),
                                  "Publisher::suspend_publications");
    }
    /// `resume_publications`.
    void resume_publications() {
        ::dds::core::check_status(zerodds_pub_resume_publications(handle_),
                                  "Publisher::resume_publications");
    }
    /// `begin_coherent_changes`.
    void begin_coherent_changes() {
        ::dds::core::check_status(zerodds_pub_begin_coherent_changes(handle_),
                                  "Publisher::begin_coherent_changes");
    }
    /// `end_coherent_changes`.
    void end_coherent_changes() {
        ::dds::core::check_status(zerodds_pub_end_coherent_changes(handle_),
                                  "Publisher::end_coherent_changes");
    }
    /// `wait_for_acknowledgments(timeout)`.
    void wait_for_acknowledgments(const ::dds::core::Duration& d) {
        uint64_t ms = static_cast<uint64_t>(d.sec()) * 1000ULL + d.nanosec() / 1000000ULL;
        int rc = zerodds_pub_wait_for_acknowledgments(handle_, ms);
        if (rc == -4) throw ::dds::core::TimeoutError("Publisher::wait_for_acks");
        ::dds::core::check_status(rc, "Publisher::wait_for_acknowledgments");
    }

    /// Native handle (for DataWriter construction).
    zerodds_ZeroDdsPublisher* native_handle() const { return handle_; }

private:
    void close() {
        if (handle_ != nullptr && participant_ != nullptr) {
            zerodds_pub_delete_contained_entities(handle_);
            zerodds_dp_delete_publisher(participant_, handle_);
            handle_ = nullptr;
            participant_ = nullptr;
        }
    }
    zerodds_ZeroDdsPublisher* handle_{nullptr};
    zerodds_ZeroDdsDomainParticipant* participant_{nullptr};
};

/// DataWriter<T> (Spec §7.5.14.5).
template <typename T>
class DataWriter {
public:
    DataWriter() = default;
    /// Constructed via Pub + Topic with default QoS.
    DataWriter(Publisher& pub, ::dds::topic::Topic<T>& topic)
        : publisher_(pub.native_handle()) {
        handle_ = zerodds_pub_create_datawriter(publisher_, topic.native_handle(), nullptr);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("DataWriter::create failed");
        }
    }
    /// With QoS.
    DataWriter(Publisher& pub, ::dds::topic::Topic<T>& topic, const ::dds::core::DataWriterQos& qos)
        : publisher_(pub.native_handle()) {
        ::dds::core::detail::PartitionPtrs part;
        auto native = ::dds::core::detail::to_native(qos, part);
        handle_ = zerodds_pub_create_datawriter(publisher_, topic.native_handle(), &native);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("DataWriter::create with QoS failed");
        }
    }

    DataWriter(const DataWriter&) = delete;
    DataWriter& operator=(const DataWriter&) = delete;
    DataWriter(DataWriter&& o) noexcept : handle_(o.handle_), publisher_(o.publisher_) {
        o.handle_ = nullptr;
        o.publisher_ = nullptr;
    }
    DataWriter& operator=(DataWriter&& o) noexcept {
        if (this != &o) {
            close();
            handle_ = o.handle_;
            publisher_ = o.publisher_;
            o.handle_ = nullptr;
            o.publisher_ = nullptr;
        }
        return *this;
    }
    ~DataWriter() { close(); }

    /// Writes a sample instance.
    void write(const T& sample) {
        std::vector<uint8_t> buf = ::dds::topic::topic_type_support<T>::encode(sample);
        int rc = zerodds_dw_write(handle_, buf.data(), buf.size(), 0);
        ::dds::core::check_status(rc, "DataWriter::write");
    }
    /// Writes with source timestamp.
    void write(const T& sample, const ::dds::core::Time& ts) {
        std::vector<uint8_t> buf = ::dds::topic::topic_type_support<T>::encode(sample);
        int rc = zerodds_dw_write_w_timestamp(handle_, buf.data(), buf.size(), 0,
                                              ts.sec(), ts.nanosec());
        ::dds::core::check_status(rc, "DataWriter::write@ts");
    }

    /// `register_instance(sample)` (Spec §2.2.2.4.2.5). Returns the
    /// instance handle that subsequent keyed writes/dispose/unregister can
    /// reference. Drives `zerodds_dw_register_instance` with the sample's
    /// 16-byte XCDR2 key serialization.
    ::dds::core::InstanceHandle register_instance(const T& sample) {
        auto kh = ::dds::topic::topic_type_support<T>::key_hash(sample);
        uint64_t h = 0;
        int rc = zerodds_dw_register_instance(handle_, kh.data(), kh.size(), &h);
        ::dds::core::check_status(rc, "DataWriter::register_instance");
        return ::dds::core::InstanceHandle(h);
    }
    /// `register_instance_w_timestamp(sample, ts)`.
    ::dds::core::InstanceHandle register_instance(const T& sample,
                                                  const ::dds::core::Time& ts) {
        auto kh = ::dds::topic::topic_type_support<T>::key_hash(sample);
        uint64_t h = 0;
        int rc = zerodds_dw_register_instance_w_timestamp(
            handle_, kh.data(), kh.size(), ts.sec(), ts.nanosec(), &h);
        ::dds::core::check_status(rc, "DataWriter::register_instance@ts");
        return ::dds::core::InstanceHandle(h);
    }

    /// `lookup_instance(sample)` (Spec §2.2.2.4.2.16). Maps a keyed sample
    /// to its instance handle without registering it.
    ::dds::core::InstanceHandle lookup_instance(const T& sample) {
        auto kh = ::dds::topic::topic_type_support<T>::key_hash(sample);
        uint64_t h = 0;
        int rc = zerodds_dw_lookup_instance(handle_, kh.data(), kh.size(), &h);
        ::dds::core::check_status(rc, "DataWriter::lookup_instance");
        return ::dds::core::InstanceHandle(h);
    }

    /// `dispose(sample)` (Spec §2.2.2.4.2.13) — emits the DISPOSE lifecycle
    /// so matched readers see the instance as NOT_ALIVE_DISPOSED. The key is
    /// taken from `sample` (only the @key fields are significant).
    void dispose_instance(const T& sample) {
        auto kh = ::dds::topic::topic_type_support<T>::key_hash(sample);
        int rc = zerodds_dw_dispose(handle_, kh.data(), 0);
        ::dds::core::check_status(rc, "DataWriter::dispose");
    }
    /// `dispose_w_timestamp(sample, ts)`.
    void dispose_instance(const T& sample, const ::dds::core::Time& ts) {
        auto kh = ::dds::topic::topic_type_support<T>::key_hash(sample);
        int rc = zerodds_dw_dispose_w_timestamp(handle_, kh.data(), 0,
                                                ts.sec(), ts.nanosec());
        ::dds::core::check_status(rc, "DataWriter::dispose@ts");
    }

    /// `unregister_instance(sample)` (Spec §2.2.2.4.2.7) — emits the
    /// UNREGISTER lifecycle so readers see NOT_ALIVE_NO_WRITERS. Resolves the
    /// handle from the sample key, then unregisters by handle.
    void unregister_instance(const T& sample) {
        ::dds::core::InstanceHandle h = lookup_instance(sample);
        int rc = zerodds_dw_unregister_instance(handle_, h.value());
        ::dds::core::check_status(rc, "DataWriter::unregister_instance");
    }
    /// `unregister_instance_w_timestamp(sample, ts)`.
    void unregister_instance(const T& sample, const ::dds::core::Time& ts) {
        ::dds::core::InstanceHandle h = lookup_instance(sample);
        int rc = zerodds_dw_unregister_instance_w_timestamp(
            handle_, h.value(), ts.sec(), ts.nanosec());
        ::dds::core::check_status(rc, "DataWriter::unregister_instance@ts");
    }
    /// Handle-based unregister (Spec §2.2.2.4.2.7, handle overload).
    void unregister_instance(const ::dds::core::InstanceHandle& h) {
        int rc = zerodds_dw_unregister_instance(handle_, h.value());
        ::dds::core::check_status(rc, "DataWriter::unregister_instance(handle)");
    }

    /// `wait_for_acknowledgments`.
    void wait_for_acknowledgments(const ::dds::core::Duration& d) {
        uint64_t ms = static_cast<uint64_t>(d.sec()) * 1000ULL + d.nanosec() / 1000000ULL;
        int rc = zerodds_dw_wait_for_acknowledgments(handle_, ms);
        if (rc == -4) throw ::dds::core::TimeoutError("DataWriter::wait_for_acks");
        ::dds::core::check_status(rc, "DataWriter::wait_for_acknowledgments");
    }
    /// `assert_liveliness`.
    void assert_liveliness() {
        ::dds::core::check_status(zerodds_dw_assert_liveliness(handle_),
                                  "DataWriter::assert_liveliness");
    }
    /// `wait_for_matched(min, timeout)`.
    void wait_for_matched(int32_t min, const ::dds::core::Duration& d) {
        uint64_t ms = static_cast<uint64_t>(d.sec()) * 1000ULL + d.nanosec() / 1000000ULL;
        int rc = zerodds_dw_wait_for_matched(handle_, min, ms);
        if (rc == -4) throw ::dds::core::TimeoutError("DataWriter::wait_for_matched");
        ::dds::core::check_status(rc, "DataWriter::wait_for_matched");
    }

    /// `publication_matched_status`.
    ::dds::core::status::PublicationMatchedStatus publication_matched_status() {
        zerodds_ZeroDdsPublicationMatchedStatus s{};
        ::dds::core::check_status(zerodds_dw_get_publication_matched_status(handle_, &s),
                                  "DataWriter::publication_matched_status");
        ::dds::core::status::PublicationMatchedStatus out;
        out.total_count = s.total_count;
        out.total_count_change = s.total_count_change;
        out.current_count = s.current_count;
        out.current_count_change = s.current_count_change;
        out.last_subscription_handle =
            ::dds::core::InstanceHandle(s.last_subscription_handle);
        return out;
    }
    /// `liveliness_lost_status`.
    ::dds::core::status::LivelinessLostStatus liveliness_lost_status() {
        zerodds_ZeroDdsLivelinessLostStatus s{};
        ::dds::core::check_status(zerodds_dw_get_liveliness_lost_status(handle_, &s),
                                  "DataWriter::liveliness_lost_status");
        ::dds::core::status::LivelinessLostStatus out;
        out.total_count = s.total_count;
        out.total_count_change = s.total_count_change;
        return out;
    }
    /// `offered_deadline_missed_status`.
    ::dds::core::status::OfferedDeadlineMissedStatus offered_deadline_missed_status() {
        zerodds_ZeroDdsOfferedDeadlineMissedStatus s{};
        ::dds::core::check_status(zerodds_dw_get_offered_deadline_missed_status(handle_, &s),
                                  "DataWriter::offered_deadline_missed_status");
        ::dds::core::status::OfferedDeadlineMissedStatus out;
        out.total_count = s.total_count;
        out.total_count_change = s.total_count_change;
        out.last_instance_handle = ::dds::core::InstanceHandle(s.last_instance_handle);
        return out;
    }
    /// `offered_incompatible_qos_status`.
    ::dds::core::status::OfferedIncompatibleQosStatus offered_incompatible_qos_status() {
        zerodds_ZeroDdsOfferedIncompatibleQosStatus s{};
        ::dds::core::check_status(zerodds_dw_get_offered_incompatible_qos_status(handle_, &s),
                                  "DataWriter::offered_incompatible_qos_status");
        ::dds::core::status::OfferedIncompatibleQosStatus out;
        out.total_count = s.total_count;
        out.total_count_change = s.total_count_change;
        out.last_policy_id = s.last_policy_id;
        return out;
    }

    /// Native handle.
    zerodds_ZeroDdsDataWriter* native_handle() const { return handle_; }

private:
    void close() {
        if (handle_ != nullptr && publisher_ != nullptr) {
            zerodds_pub_delete_datawriter(publisher_, handle_);
            handle_ = nullptr;
            publisher_ = nullptr;
        }
    }
    zerodds_ZeroDdsDataWriter* handle_{nullptr};
    zerodds_ZeroDdsPublisher* publisher_{nullptr};
};

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_PUBLISHER_HPP
