// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/TGenerationCount.hpp — DDS-PSM-Cxx 1.0 `TGenerationCount<D>`.

#ifndef ZERODDS_DDS_SUB_TGENERATIONCOUNT_HPP
#define ZERODDS_DDS_SUB_TGENERATIONCOUNT_HPP

#include <stdint.h>

#include "dds/core/Value.hpp"

namespace dds {
namespace sub {

/// The generation counts of a sample's instance, over any delegate.
template <typename D>
class TGenerationCount : public ::dds::core::Value<D> {
public:
    TGenerationCount() : ::dds::core::Value<D>() {}
    TGenerationCount(int32_t dgc, int32_t nwgc) : ::dds::core::Value<D>(dgc, nwgc) {}

    int32_t disposed() const { return this->d_.disposed(); }
    int32_t no_writers() const { return this->d_.no_writers(); }
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_TGENERATIONCOUNT_HPP
