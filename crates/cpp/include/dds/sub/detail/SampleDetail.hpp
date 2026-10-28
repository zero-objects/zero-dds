// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/detail/SampleDetail.hpp — Internal helpers for Sample decoding.

#ifndef ZERODDS_DDS_SUB_DETAIL_SAMPLEDETAIL_HPP
#define ZERODDS_DDS_SUB_DETAIL_SAMPLEDETAIL_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <cstddef>

#include "dds/sub/SampleInfo.hpp"
#include "dds/topic/TopicTraits.hpp"
#include "zerodds/psm/encoding.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {
namespace detail {

// The tags of the detection traits below. A trait asks whether an expression compiles by putting
// it into a `sizeof` in the parameter of a function template: ISO C++03 has no `decltype` or
// `void_t`, and a substitution failure in the expression removes the overload.
typedef char probe_yes;
struct probe_no {
    char pad[2];
};
template <std::size_t N>
struct probe_size {};

/// Whether `TS::decode(buf, len, version, big_endian)` exists (what
/// `zerodds-idlc generate --lang c++` emits).
template <typename TS>
struct has_decode_with_encoding {
    static const uint8_t* buf_arg();
    static ::zerodds::wire::Encoding version_arg();
    template <typename U>
    static probe_yes test(
        probe_size<sizeof(U::decode(buf_arg(), std::size_t(0), version_arg(), false))>*);
    template <typename U>
    static probe_no test(...);
    static const bool value = sizeof(test<TS>(0)) == sizeof(probe_yes);
};
template <typename TS>
const bool has_decode_with_encoding<TS>::value;

// Decode arity adapter. `zerodds-idlc generate --lang c++` emits
// decode(buf, len, zerodds::wire::Encoding, bool big_endian); hand-written
// topic_type_support<T> specializations (TopicTraits.hpp ByteSeq/string)
// use decode(buf, len). DataReader<T>::take() dispatches to whichever the
// specialization provides, and hands the generated one the XCDR version and
// byte order the sample's SampleInfo reports, as zerodds::TypedReader<T> does.
/// The sample the support `topic_type_support<T>` decodes from `buf`: with the version and
/// byte order when it takes them.
template <typename T>
inline T decode_sample_with(const uint8_t* buf, std::size_t len, ::zerodds::wire::Encoding version,
                            bool big_endian, ::zerodds::cxx::integral_constant<bool, true>) {
    return ::dds::topic::topic_type_support<T>::decode(buf, len, version, big_endian);
}
template <typename T>
inline T decode_sample_with(const uint8_t* buf, std::size_t len, ::zerodds::wire::Encoding,
                            bool, ::zerodds::cxx::integral_constant<bool, false>) {
    return ::dds::topic::topic_type_support<T>::decode(buf, len);
}

template <typename T>
inline T decode_sample(const uint8_t* buf, std::size_t len, ::zerodds::wire::Encoding version,
                       bool big_endian) {
    return decode_sample_with<T>(
        buf, len, version, big_endian,
        ::zerodds::cxx::integral_constant<
            bool, has_decode_with_encoding< ::dds::topic::topic_type_support<T> >::value>());
}

/// The SampleInfo of a C-API sample info: the DDS 1.4 state bits are the
/// ones the DataState classes use.
inline ::dds::sub::SampleInfo make_sample_info(const zerodds_ZeroDdsSampleInfo& c) {
    using namespace ::dds::sub::status;
    return ::dds::sub::SampleInfo(
        ::dds::core::Time(c.source_timestamp_sec, c.source_timestamp_nanosec),
        DataState(SampleState(c.sample_state), ViewState(c.view_state),
                  InstanceState(c.instance_state)),
        ::dds::sub::GenerationCount(c.disposed_generation_count, c.no_writers_generation_count),
        ::dds::sub::Rank(c.sample_rank, c.absolute_generation_rank, c.generation_rank),
        c.valid_data != 0, ::dds::core::InstanceHandle(c.instance_handle),
        ::dds::core::InstanceHandle(c.publication_handle));
}

/// Returns the loaned sample buffers on scope exit unless `release()` was
/// called, so a decode that throws (`std::bad_alloc`, a `std::out_of_range`
/// from a codec) still returns the loan instead of leaving it open. DDS-PSM-Cxx
/// 1.0 §7.3 requires loan-based read/take to be exception safe.
class LoanGuard {
public:
    LoanGuard(zerodds_ZeroDdsDataReader* dr, zerodds_ZeroDdsSampleArray* arr)
        : dr_(dr), arr_(arr), released_(false) {}
    ZERODDS_NONCOPYABLE(LoanGuard)
    ~LoanGuard() {
        if (!released_) {
            zerodds_dr_return_loan(dr_, arr_);
        }
    }
    void release() { released_ = true; }
private:
    zerodds_ZeroDdsDataReader* dr_;
    zerodds_ZeroDdsSampleArray* arr_;
    bool released_;
};

} // namespace detail
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DETAIL_SAMPLEDETAIL_HPP