// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/TSample.hpp — DDS-PSM-Cxx 1.0 `TSample<T, DELEGATE>`: a sample and its SampleInfo.

#ifndef ZERODDS_DDS_SUB_TSAMPLE_HPP
#define ZERODDS_DDS_SUB_TSAMPLE_HPP


#include "zerodds/psm/cxx.hpp"
#include <utility>

#include "dds/core/Value.hpp"
#include "dds/sub/SampleInfo.hpp"

namespace dds {
namespace sub {

/// A sample of type `T` with its SampleInfo, over any delegate.
template <typename T, typename DELEGATE>
class TSample : public ::dds::core::Value<DELEGATE> {
public:
    typedef T DataType;

    /// A sample with invalid data.
    TSample() : ::dds::core::Value<DELEGATE>() {}
    TSample(const T& data, const SampleInfo& info) : ::dds::core::Value<DELEGATE>(data, info) {}
#if ZERODDS_CXX11
    TSample(T&& data, const SampleInfo& info)
        : ::dds::core::Value<DELEGATE>(DELEGATE(ZERODDS_MOVE(data), info)) {}
    TSample(const TSample& other) = default;
    TSample(TSample&& other) = default;
    TSample& operator=(const TSample& other) = default;
    TSample& operator=(TSample&& other) = default;
#endif

    const DataType& data() const { return this->d_.data(); }
    void data(const DataType& d) { this->d_.data(d); }
    const SampleInfo& info() const { return this->d_.info(); }
    void info(const SampleInfo& i) { this->d_.info(i); }
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_TSAMPLE_HPP
