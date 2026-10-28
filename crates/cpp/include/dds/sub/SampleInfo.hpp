// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/SampleInfo.hpp — SampleInfo (DDS-PSM-Cxx 1.0 §7.5.4): the meta-data
// of a sample (DDS 1.4 §2.2.2.5.1.1).

#ifndef ZERODDS_DDS_SUB_SAMPLEINFO_HPP
#define ZERODDS_DDS_SUB_SAMPLEINFO_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "dds/core/InstanceHandle.hpp"
#include "dds/core/Time.hpp"
#include "dds/sub/GenerationCount.hpp"
#include "dds/sub/Rank.hpp"
#include "dds/sub/TSampleInfo.hpp"
#include "dds/sub/status/DataState.hpp"

namespace dds {
namespace sub {

namespace detail {

class SampleInfo {
public:
    SampleInfo()
        : timestamp_(),
          state_(status::SampleState(), status::ViewState(), status::InstanceState()),
          generation_count_(),
          rank_(),
          valid_(false),
          instance_handle_(),
          publication_handle_() {}
    /// All members at once; the reader builds a SampleInfo with it.
    SampleInfo(const ::dds::core::Time& timestamp, const status::DataState& state,
               const ::dds::sub::GenerationCount& generation_count, const ::dds::sub::Rank& rank, bool valid,
               const ::dds::core::InstanceHandle& instance_handle,
               const ::dds::core::InstanceHandle& publication_handle)
        : timestamp_(timestamp),
          state_(state),
          generation_count_(generation_count),
          rank_(rank),
          valid_(valid),
          instance_handle_(instance_handle),
          publication_handle_(publication_handle) {}

    /// The source time stamp.
    const ::dds::core::Time timestamp() const { return timestamp_; }
    /// Sample, view and instance state.
    const status::DataState state() const { return state_; }
    ::dds::sub::GenerationCount generation_count() const { return generation_count_; }
    ::dds::sub::Rank rank() const { return rank_; }
    /// Whether the sample carries data (DDS 1.4 `valid_data`).
    bool valid() const { return valid_; }
    ::dds::core::InstanceHandle instance_handle() const { return instance_handle_; }
    ::dds::core::InstanceHandle publication_handle() const { return publication_handle_; }

private:
    ::dds::core::Time timestamp_;
    status::DataState state_;
    ::dds::sub::GenerationCount generation_count_;
    ::dds::sub::Rank rank_;
    bool valid_;
    ::dds::core::InstanceHandle instance_handle_;
    ::dds::core::InstanceHandle publication_handle_;
};

} // namespace detail

typedef TSampleInfo<detail::SampleInfo> SampleInfo;

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_SAMPLEINFO_HPP
