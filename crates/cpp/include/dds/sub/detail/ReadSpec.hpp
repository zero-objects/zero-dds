// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/detail/ReadSpec.hpp — what one read or take asks for.

#ifndef ZERODDS_DDS_SUB_DETAIL_READSPEC_HPP
#define ZERODDS_DDS_SUB_DETAIL_READSPEC_HPP

#include <stdint.h>

#include "dds/core/InstanceHandle.hpp"
#include "dds/sub/Query.hpp"
#include "dds/sub/status/DataState.hpp"

namespace dds {
namespace sub {
namespace detail {

/// What one read or take asks for: how many samples, which states, which
/// instance, which content filter (DDS 1.4 §2.2.2.5.3.8 to §2.2.2.5.3.15).
struct ReadSpec {
    /// Which instances a read covers. A struct-wrapped plain enumeration, so that `Scope::All`
    /// is valid with a C++03 compiler.
    struct Scope {
        enum Value { All, Instance, NextInstance };
    };

    ReadSpec()
        : max_samples(0), state(), scope(Scope::All), instance(), query(0) {}

    /// 0 is no limit.
    uint32_t max_samples;
    ::dds::sub::status::DataState state;
    Scope::Value scope;
    ::dds::core::InstanceHandle instance;
    const ::dds::sub::Query* query;
};

} // namespace detail
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DETAIL_READSPEC_HPP
