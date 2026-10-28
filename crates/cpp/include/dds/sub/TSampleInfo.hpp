// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/TSampleInfo.hpp — DDS-PSM-Cxx 1.0 `TSampleInfo<D>`.

#ifndef ZERODDS_DDS_SUB_TSAMPLEINFO_HPP
#define ZERODDS_DDS_SUB_TSAMPLEINFO_HPP

#include "dds/core/InstanceHandle.hpp"
#include "dds/core/Time.hpp"
#include "dds/core/Value.hpp"
#include "dds/sub/GenerationCount.hpp"
#include "dds/sub/Rank.hpp"
#include "dds/sub/status/DataState.hpp"

namespace dds {
namespace sub {

/// The meta-data of a sample a reader returned, over any delegate.
template <typename D>
class TSampleInfo : public ::dds::core::Value<D> {
public:
    TSampleInfo() : ::dds::core::Value<D>() {}

    /// All members at once; the reader builds a SampleInfo with it.
    TSampleInfo(const ::dds::core::Time& timestamp, const status::DataState& state,
                const GenerationCount& generation_count, const Rank& rank, bool valid,
                const ::dds::core::InstanceHandle& instance_handle,
                const ::dds::core::InstanceHandle& publication_handle)
        : ::dds::core::Value<D>(D(timestamp, state, generation_count, rank, valid, instance_handle,
                                  publication_handle)) {}

    /// The source time stamp.
    const ::dds::core::Time timestamp() const { return this->d_.timestamp(); }
    /// Sample, view and instance state.
    const status::DataState state() const { return this->d_.state(); }
    GenerationCount generation_count() const { return this->d_.generation_count(); }
    Rank rank() const { return this->d_.rank(); }
    /// Whether the sample carries data (DDS 1.4 `valid_data`).
    bool valid() const { return this->d_.valid(); }
    ::dds::core::InstanceHandle instance_handle() const { return this->d_.instance_handle(); }
    ::dds::core::InstanceHandle publication_handle() const { return this->d_.publication_handle(); }
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_TSAMPLEINFO_HPP
