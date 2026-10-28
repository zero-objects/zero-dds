// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/BuiltinDataReader.hpp — the DataReaders of the built-in subscriber (DDS 1.4 §2.2.5,
// DDS-PSM-Cxx 1.0 §7.8): `DataReader<ParticipantBuiltinTopicData>`, `DataReader<
// TopicBuiltinTopicData>`, `DataReader<PublicationBuiltinTopicData>` and `DataReader<
// SubscriptionBuiltinTopicData>`, found with `dds::sub::find` on the built-in subscriber and
// read or taken like any other reader.

#ifndef ZERODDS_DDS_SUB_BUILTINDATAREADER_HPP
#define ZERODDS_DDS_SUB_BUILTINDATAREADER_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "dds/sub/detail/DataReaderFwd.hpp"
#include "dds/core/Exception.hpp"
#include "dds/sub/LoanedSamples.hpp"
#include "dds/sub/Sample.hpp"
#include "zerodds.h"
#include "dds/sub/detail/SampleDetail.hpp"
#include "dds/topic/BuiltinTopic.hpp"
#include "zerodds.h"
#include "zerodds/psm/check_status.hpp"

namespace dds {
namespace sub {


namespace detail {

/// The name of the built-in topic, and the sample of the C API as the C++ data type.
template <typename D>
struct BuiltinTopicOf;

template <>
struct BuiltinTopicOf< ::dds::topic::ParticipantBuiltinTopicData> {
    static const char* name() { return "DCPSParticipant"; }
    static ::dds::topic::ParticipantBuiltinTopicData data(const zerodds_ZeroDdsBuiltinSample& s) {
        return ::dds::topic::ParticipantBuiltinTopicData::from_native(s.participant);
    }
};
template <>
struct BuiltinTopicOf< ::dds::topic::TopicBuiltinTopicData> {
    static const char* name() { return "DCPSTopic"; }
    static ::dds::topic::TopicBuiltinTopicData data(const zerodds_ZeroDdsBuiltinSample& s) {
        return ::dds::topic::TopicBuiltinTopicData::from_native(s.topic);
    }
};
template <>
struct BuiltinTopicOf< ::dds::topic::PublicationBuiltinTopicData> {
    static const char* name() { return "DCPSPublication"; }
    static ::dds::topic::PublicationBuiltinTopicData data(const zerodds_ZeroDdsBuiltinSample& s) {
        return ::dds::topic::PublicationBuiltinTopicData::from_native(s.publication);
    }
};
template <>
struct BuiltinTopicOf< ::dds::topic::SubscriptionBuiltinTopicData> {
    static const char* name() { return "DCPSSubscription"; }
    static ::dds::topic::SubscriptionBuiltinTopicData data(const zerodds_ZeroDdsBuiltinSample& s) {
        return ::dds::topic::SubscriptionBuiltinTopicData::from_native(s.subscription);
    }
};

/// Destroys a built-in reader handle with the last copy of the reader.
struct BuiltinReaderDeleter {
    void operator()(zerodds_ZeroDdsBuiltinReader* h) const {
        if (h != 0) {
            zerodds_builtin_reader_destroy(h);
        }
    }
};

/// What the four readers of the built-in subscriber share: `read` and `take` of every sample
/// the reader holds. A reader is a reference: copies share the C handle.
template <typename D>
class BuiltinDataReader {
protected:
    BuiltinDataReader() ZERODDS_NOEXCEPT {}

public:

    /// Takes over `handle`; released with the last copy.
    explicit BuiltinDataReader(zerodds_ZeroDdsBuiltinReader* handle)
        : handle_(handle, BuiltinReaderDeleter()) {}

    bool operator==(const BuiltinDataReader& other) const { return handle_ == other.handle_; }
    bool operator!=(const BuiltinDataReader& other) const { return !(*this == other); }
    bool operator==(const ::dds::core::null_type&) const { return handle_ == nullptr; }
    bool operator!=(const ::dds::core::null_type&) const { return handle_ != nullptr; }

    /// The topic name this reader is for.
    static const char* topic_name() { return BuiltinTopicOf<D>::name(); }

    zerodds_ZeroDdsBuiltinReader* native_handle() const { return handle_.get(); }

    /// Every sample the reader holds, left in the reader (`sample_state` READ afterwards).
    LoanedSamples<D> read() { return LoanedSamples<D>(fetch(false)); }
    /// Every sample the reader holds, removed from the reader.
    LoanedSamples<D> take() { return LoanedSamples<D>(fetch(true)); }

private:
    std::vector<Sample<D> > fetch(bool take) {
        if (!handle_) {
            throw ::dds::core::NullReferenceError("a call through a null reference");
        }
        zerodds_ZeroDdsBuiltinSample* raw = nullptr;
        uintptr_t count = 0;
        ::zerodds::psm::check_status(
            zerodds_builtin_reader_read(handle_.get(), take ? 1 : 0, &raw, &count),
            take ? "DataReader<built-in>::take" : "DataReader<built-in>::read");
        // The array is released on every path out, a throwing conversion included.
        struct Release {
            Release(zerodds_ZeroDdsBuiltinSample* s, uintptr_t c) : samples(s), count(c) {}
            ~Release() { zerodds_builtin_samples_free(samples, count); }
            zerodds_ZeroDdsBuiltinSample* samples;
            uintptr_t count;
        } release(raw, count);
        std::vector<Sample<D> > out;
        out.reserve(count);
        for (uintptr_t i = 0; i < count; ++i) {
            out.push_back(Sample<D>(BuiltinTopicOf<D>::data(raw[i]), make_sample_info(raw[i].info)));
        }
        return out;
    }

    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsBuiltinReader> handle_;
};

} // namespace detail

/// `DataReader<ParticipantBuiltinTopicData>`: the reader of `DCPSParticipant`.
template <>
class DataReader< ::dds::topic::ParticipantBuiltinTopicData>
    : public detail::BuiltinDataReader< ::dds::topic::ParticipantBuiltinTopicData> {
public:
    typedef detail::BuiltinDataReader< ::dds::topic::ParticipantBuiltinTopicData> BASE_T;

    DataReader(const ::dds::core::null_type&) ZERODDS_NOEXCEPT {}
    /// Takes over `handle`; released with the last copy.
    explicit DataReader(zerodds_ZeroDdsBuiltinReader* handle) : BASE_T(handle) {}
};
/// `DataReader<TopicBuiltinTopicData>`: the reader of `DCPSTopic`.
template <>
class DataReader< ::dds::topic::TopicBuiltinTopicData>
    : public detail::BuiltinDataReader< ::dds::topic::TopicBuiltinTopicData> {
public:
    typedef detail::BuiltinDataReader< ::dds::topic::TopicBuiltinTopicData> BASE_T;

    DataReader(const ::dds::core::null_type&) ZERODDS_NOEXCEPT {}
    /// Takes over `handle`; released with the last copy.
    explicit DataReader(zerodds_ZeroDdsBuiltinReader* handle) : BASE_T(handle) {}
};
/// `DataReader<PublicationBuiltinTopicData>`: the reader of `DCPSPublication`.
template <>
class DataReader< ::dds::topic::PublicationBuiltinTopicData>
    : public detail::BuiltinDataReader< ::dds::topic::PublicationBuiltinTopicData> {
public:
    typedef detail::BuiltinDataReader< ::dds::topic::PublicationBuiltinTopicData> BASE_T;

    DataReader(const ::dds::core::null_type&) ZERODDS_NOEXCEPT {}
    /// Takes over `handle`; released with the last copy.
    explicit DataReader(zerodds_ZeroDdsBuiltinReader* handle) : BASE_T(handle) {}
};
/// `DataReader<SubscriptionBuiltinTopicData>`: the reader of `DCPSSubscription`.
template <>
class DataReader< ::dds::topic::SubscriptionBuiltinTopicData>
    : public detail::BuiltinDataReader< ::dds::topic::SubscriptionBuiltinTopicData> {
public:
    typedef detail::BuiltinDataReader< ::dds::topic::SubscriptionBuiltinTopicData> BASE_T;

    DataReader(const ::dds::core::null_type&) ZERODDS_NOEXCEPT {}
    /// Takes over `handle`; released with the last copy.
    explicit DataReader(zerodds_ZeroDdsBuiltinReader* handle) : BASE_T(handle) {}
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_BUILTINDATAREADER_HPP
