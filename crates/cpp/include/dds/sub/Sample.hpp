// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/Sample.hpp — Sample<T> (DDS-PSM-Cxx 1.0 §7.5.4): the data and the
// SampleInfo of one sample.

#ifndef ZERODDS_DDS_SUB_SAMPLE_HPP
#define ZERODDS_DDS_SUB_SAMPLE_HPP


#include "zerodds/psm/cxx.hpp"
#include <utility>

#include "dds/sub/SampleInfo.hpp"
#include "dds/sub/TSample.hpp"

namespace dds {
namespace sub {

namespace detail {

template <typename T>
class SampleDelegate {
public:
    typedef T DataType;

    /// A sample with invalid data.
    SampleDelegate() : data_(), info_() {}
    SampleDelegate(const T& data, const ::dds::sub::SampleInfo& info) : data_(data), info_(info) {}
#if ZERODDS_CXX11
    SampleDelegate(T&& data, const ::dds::sub::SampleInfo& info) : data_(ZERODDS_MOVE(data)), info_(info) {}
#endif
#if ZERODDS_CXX11
    SampleDelegate(const SampleDelegate& other) = default;
    SampleDelegate(SampleDelegate&& other) = default;
    SampleDelegate& operator=(const SampleDelegate& other) = default;
    SampleDelegate& operator=(SampleDelegate&& other) = default;
#endif

    const DataType& data() const { return data_; }
    void data(const DataType& d) { data_ = d; }

    const ::dds::sub::SampleInfo& info() const { return info_; }
    void info(const ::dds::sub::SampleInfo& i) { info_ = i; }

private:
    T data_;
    ::dds::sub::SampleInfo info_;
};

} // namespace detail

/// A sample with its meta-data (Spec §7.5.4 `TSample`): ZeroDDS's delegate under `TSample`.
template <typename T>
class Sample : public TSample<T, detail::SampleDelegate<T> > {
public:
    typedef TSample<T, detail::SampleDelegate<T> > BASE_T;

    /// A sample with invalid data.
    Sample() : BASE_T() {}
    Sample(const T& data, const ::dds::sub::SampleInfo& info) : BASE_T(data, info) {}
#if ZERODDS_CXX11
    Sample(T&& data, const ::dds::sub::SampleInfo& info) : BASE_T(ZERODDS_MOVE(data), info) {}
#endif
#if ZERODDS_CXX11
    Sample(const Sample& other) = default;
    Sample(Sample&& other) = default;
    Sample& operator=(const Sample& other) = default;
    Sample& operator=(Sample&& other) = default;
#endif
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_SAMPLE_HPP
