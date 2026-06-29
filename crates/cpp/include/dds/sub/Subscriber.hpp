// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/Subscriber.hpp + DataReader + Sample — DDS-PSM-Cxx 1.0 §7.5.15.

#ifndef ZERODDS_DDS_SUB_SUBSCRIBER_HPP
#define ZERODDS_DDS_SUB_SUBSCRIBER_HPP

#include <cstring>
#include <type_traits>
#include <utility>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/Status.hpp"
#include "dds/core/Time.hpp"
#include "dds/core/qos.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/domain/DomainParticipant.hpp"
#include "dds/topic/Topic.hpp"
#include "dds/topic/xcdr2.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {

namespace detail {

// Decode arity adapter (Spec-neutral). idl-cpp emits a 3-arg
// decode(buf,len,XcdrVersion); hand-written topic_type_support<T>
// specializations (TopicTraits.hpp ByteSeq/string) use the 2-arg
// decode(buf,len). DataReader<T>::take() dispatches to whichever the
// specialization provides, so the typed reader compiles against both
// without a hand-added overload in the generated header.
template <typename TS>
auto decode_sample(const uint8_t* buf, size_t len, int)
    -> decltype(TS::decode(buf, len, ::dds::topic::xcdr2::XcdrVersion::Xcdr2)) {
    return TS::decode(buf, len, ::dds::topic::xcdr2::XcdrVersion::Xcdr2);
}
template <typename TS>
auto decode_sample(const uint8_t* buf, size_t len, long)
    -> decltype(TS::decode(buf, len)) {
    return TS::decode(buf, len);
}

} // namespace detail

/// SampleInfo (Spec §7.5.15.6).
class SampleInfo {
public:
    SampleInfo() = default;
    explicit SampleInfo(const zerodds_ZeroDdsSampleInfo& c) {
        sample_state = c.sample_state;
        view_state = c.view_state;
        instance_state = c.instance_state;
        valid_data = c.valid_data;
        instance_handle = ::dds::core::InstanceHandle(c.instance_handle);
        publication_handle = ::dds::core::InstanceHandle(c.publication_handle);
        source_timestamp = ::dds::core::Time(c.source_timestamp_sec, c.source_timestamp_nanosec);
        disposed_generation_count = c.disposed_generation_count;
        no_writers_generation_count = c.no_writers_generation_count;
        sample_rank = c.sample_rank;
        generation_rank = c.generation_rank;
        absolute_generation_rank = c.absolute_generation_rank;
    }
    uint32_t sample_state{0};
    uint32_t view_state{0};
    uint32_t instance_state{0};
    bool valid_data{false};
    ::dds::core::InstanceHandle instance_handle;
    ::dds::core::InstanceHandle publication_handle;
    ::dds::core::Time source_timestamp;
    int32_t disposed_generation_count{0};
    int32_t no_writers_generation_count{0};
    int32_t sample_rank{0};
    int32_t generation_rank{0};
    int32_t absolute_generation_rank{0};
};

/// Sample<T> (Spec §7.5.15.5).
template <typename T>
class Sample {
public:
    Sample() = default;
    Sample(T data, SampleInfo info)
        : data_(std::move(data)), info_(std::move(info)) {}
    const T& data() const { return data_; }
    T& data() { return data_; }
    const SampleInfo& info() const { return info_; }
private:
    T data_{};
    SampleInfo info_{};
};

/// LoanedSamples<T>: container for take/read results.
template <typename T>
class LoanedSamples {
public:
    using value_type = Sample<T>;
    using iterator = typename std::vector<Sample<T>>::iterator;
    using const_iterator = typename std::vector<Sample<T>>::const_iterator;

    LoanedSamples() = default;
    explicit LoanedSamples(std::vector<Sample<T>> v) : samples_(std::move(v)) {}

    size_t length() const { return samples_.size(); }
    iterator begin() { return samples_.begin(); }
    iterator end() { return samples_.end(); }
    const_iterator begin() const { return samples_.begin(); }
    const_iterator end() const { return samples_.end(); }
    const Sample<T>& operator[](size_t i) const { return samples_[i]; }
    Sample<T>& operator[](size_t i) { return samples_[i]; }
private:
    std::vector<Sample<T>> samples_;
};

/// Subscriber (Spec §7.5.15.1).
class Subscriber {
public:
    explicit Subscriber(::dds::domain::DomainParticipant& dp)
        : participant_(dp.native_handle()) {
        handle_ = zerodds_dp_create_subscriber(participant_, nullptr);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("Subscriber::create failed");
        }
    }
    Subscriber(::dds::domain::DomainParticipant& dp, const ::dds::core::SubscriberQos& qos)
        : participant_(dp.native_handle()) {
        ::dds::core::detail::PartitionPtrs part;
        auto native = ::dds::core::detail::to_native(qos, part);
        handle_ = zerodds_dp_create_subscriber(participant_, &native);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("Subscriber::create with QoS failed");
        }
    }
    Subscriber(const Subscriber&) = delete;
    Subscriber& operator=(const Subscriber&) = delete;
    Subscriber(Subscriber&& o) noexcept : handle_(o.handle_), participant_(o.participant_) {
        o.handle_ = nullptr;
        o.participant_ = nullptr;
    }
    Subscriber& operator=(Subscriber&& o) noexcept {
        if (this != &o) {
            close();
            handle_ = o.handle_;
            participant_ = o.participant_;
            o.handle_ = nullptr;
            o.participant_ = nullptr;
        }
        return *this;
    }
    ~Subscriber() { close(); }

    /// `begin_access` / `end_access`.
    void begin_access() {
        ::dds::core::check_status(zerodds_sub_begin_access(handle_), "Subscriber::begin_access");
    }
    void end_access() {
        ::dds::core::check_status(zerodds_sub_end_access(handle_), "Subscriber::end_access");
    }

    /// Native Handle.
    zerodds_ZeroDdsSubscriber* native_handle() const { return handle_; }

private:
    void close() {
        if (handle_ != nullptr && participant_ != nullptr) {
            zerodds_sub_delete_contained_entities(handle_);
            zerodds_dp_delete_subscriber(participant_, handle_);
            handle_ = nullptr;
            participant_ = nullptr;
        }
    }
    zerodds_ZeroDdsSubscriber* handle_{nullptr};
    zerodds_ZeroDdsDomainParticipant* participant_{nullptr};
};

/// DataReader<T> (Spec §7.5.15.5).
template <typename T>
class DataReader {
public:
    DataReader() = default;
    /// Constructed via Sub + Topic with default QoS.
    DataReader(Subscriber& sub, ::dds::topic::Topic<T>& topic)
        : subscriber_(sub.native_handle()) {
        handle_ = zerodds_sub_create_datareader(subscriber_, topic.native_handle(), nullptr);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("DataReader::create failed");
        }
    }
    DataReader(Subscriber& sub, ::dds::topic::Topic<T>& topic, const ::dds::core::DataReaderQos& qos)
        : subscriber_(sub.native_handle()) {
        ::dds::core::detail::PartitionPtrs part;
        auto native = ::dds::core::detail::to_native(qos, part);
        handle_ = zerodds_sub_create_datareader(subscriber_, topic.native_handle(), &native);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("DataReader::create with QoS failed");
        }
    }

    /// CFT-aware constructors (Spec §7.5.13.7) — binds the reader to a
    /// ContentFilteredTopic so its filter expression is evaluated on every
    /// take/read. Templated on the CFT type to avoid a hard include cycle;
    /// any type exposing `native_cft_handle()` works (i.e.
    /// `dds::topic::ContentFilteredTopic<T>`).
    template <typename CftT>
    DataReader(Subscriber& sub, CftT& cft)
        : subscriber_(sub.native_handle()) {
        handle_ = zerodds_sub_create_datareader_with_cft(
            subscriber_, cft.native_cft_handle(), nullptr);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("DataReader::create (CFT) failed");
        }
    }
    template <typename CftT>
    DataReader(Subscriber& sub, CftT& cft, const ::dds::core::DataReaderQos& qos)
        : subscriber_(sub.native_handle()) {
        ::dds::core::detail::PartitionPtrs part;
        auto native = ::dds::core::detail::to_native(qos, part);
        handle_ = zerodds_sub_create_datareader_with_cft(
            subscriber_, cft.native_cft_handle(), &native);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("DataReader::create (CFT) with QoS failed");
        }
    }

    DataReader(const DataReader&) = delete;
    DataReader& operator=(const DataReader&) = delete;
    DataReader(DataReader&& o) noexcept : handle_(o.handle_), subscriber_(o.subscriber_) {
        o.handle_ = nullptr;
        o.subscriber_ = nullptr;
    }
    DataReader& operator=(DataReader&& o) noexcept {
        if (this != &o) {
            close();
            handle_ = o.handle_;
            subscriber_ = o.subscriber_;
            o.handle_ = nullptr;
            o.subscriber_ = nullptr;
        }
        return *this;
    }
    ~DataReader() { close(); }

    /// `take` — removes all samples received so far.
    LoanedSamples<T> take(size_t max_samples = 0) {
        zerodds_ZeroDdsSampleArray arr{};
        int rc = zerodds_dr_take(handle_, &arr, max_samples, 0, 0, 0);
        if (rc == -7 /*NoData*/) {
            return LoanedSamples<T>();
        }
        ::dds::core::check_status(rc, "DataReader::take");

        std::vector<Sample<T>> out;
        out.reserve(arr.count);
        for (size_t i = 0; i < arr.count; ++i) {
            const uint8_t* buf = arr.buffers[i];
            size_t len = arr.lengths[i];
            T data{};
            if (arr.infos[i].valid_data) {
                data = detail::decode_sample<::dds::topic::topic_type_support<T>>(buf, len, 0);
            }
            out.emplace_back(std::move(data), SampleInfo(arr.infos[i]));
        }
        zerodds_dr_return_loan(handle_, &arr);
        return LoanedSamples<T>(std::move(out));
    }
    /// `read` — like `take` but non-destructive (RC1: same path as take).
    LoanedSamples<T> read(size_t max_samples = 0) { return take(max_samples); }

    /// `wait_for_matched`.
    void wait_for_matched(int32_t min, const ::dds::core::Duration& d) {
        uint64_t ms = static_cast<uint64_t>(d.sec()) * 1000ULL + d.nanosec() / 1000000ULL;
        int rc = zerodds_dr_wait_for_matched(handle_, min, ms);
        if (rc == -4) throw ::dds::core::TimeoutError("DataReader::wait_for_matched");
        ::dds::core::check_status(rc, "DataReader::wait_for_matched");
    }

    /// `subscription_matched_status`.
    ::dds::core::status::SubscriptionMatchedStatus subscription_matched_status() {
        zerodds_ZeroDdsSubscriptionMatchedStatus s{};
        ::dds::core::check_status(zerodds_dr_get_subscription_matched_status(handle_, &s),
                                  "DataReader::subscription_matched_status");
        ::dds::core::status::SubscriptionMatchedStatus out;
        out.total_count = s.total_count;
        out.total_count_change = s.total_count_change;
        out.current_count = s.current_count;
        out.current_count_change = s.current_count_change;
        out.last_publication_handle = ::dds::core::InstanceHandle(s.last_publication_handle);
        return out;
    }
    /// `sample_lost_status`.
    ::dds::core::status::SampleLostStatus sample_lost_status() {
        zerodds_ZeroDdsSampleLostStatus s{};
        ::dds::core::check_status(zerodds_dr_get_sample_lost_status(handle_, &s),
                                  "DataReader::sample_lost_status");
        ::dds::core::status::SampleLostStatus out;
        out.total_count = s.total_count;
        out.total_count_change = s.total_count_change;
        return out;
    }
    /// `requested_deadline_missed_status` (Spec §2.2.2.5.2.6). Increments
    /// each time the offered DEADLINE period elapses with no new sample for
    /// an instance the reader tracks.
    ::dds::core::status::RequestedDeadlineMissedStatus requested_deadline_missed_status() {
        zerodds_ZeroDdsRequestedDeadlineMissedStatus s{};
        ::dds::core::check_status(zerodds_dr_get_requested_deadline_missed_status(handle_, &s),
                                  "DataReader::requested_deadline_missed_status");
        ::dds::core::status::RequestedDeadlineMissedStatus out;
        out.total_count = s.total_count;
        out.total_count_change = s.total_count_change;
        out.last_instance_handle = ::dds::core::InstanceHandle(s.last_instance_handle);
        return out;
    }
    /// `liveliness_changed_status`.
    ::dds::core::status::LivelinessChangedStatus liveliness_changed_status() {
        zerodds_ZeroDdsLivelinessChangedStatus s{};
        ::dds::core::check_status(zerodds_dr_get_liveliness_changed_status(handle_, &s),
                                  "DataReader::liveliness_changed_status");
        ::dds::core::status::LivelinessChangedStatus out;
        out.alive_count = s.alive_count;
        out.not_alive_count = s.not_alive_count;
        out.alive_count_change = s.alive_count_change;
        out.not_alive_count_change = s.not_alive_count_change;
        out.last_publication_handle = ::dds::core::InstanceHandle(s.last_publication_handle);
        return out;
    }

    /// Native Handle.
    zerodds_ZeroDdsDataReader* native_handle() const { return handle_; }

private:
    void close() {
        if (handle_ != nullptr && subscriber_ != nullptr) {
            zerodds_sub_delete_datareader(subscriber_, handle_);
            handle_ = nullptr;
            subscriber_ = nullptr;
        }
    }
    zerodds_ZeroDdsDataReader* handle_{nullptr};
    zerodds_ZeroDdsSubscriber* subscriber_{nullptr};
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_SUBSCRIBER_HPP
