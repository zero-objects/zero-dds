// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/TRank.hpp — DDS-PSM-Cxx 1.0 `TRank<D>`.

#ifndef ZERODDS_DDS_SUB_TRANK_HPP
#define ZERODDS_DDS_SUB_TRANK_HPP

#include <stdint.h>

#include "dds/core/Value.hpp"

namespace dds {
namespace sub {

/// The rank of a sample among those a read returned, over any delegate.
template <typename D>
class TRank : public ::dds::core::Value<D> {
public:
    TRank() : ::dds::core::Value<D>() {}
    TRank(int32_t s, int32_t a, int32_t ag) : ::dds::core::Value<D>(s, a, ag) {}

    int32_t absolute_generation() const { return this->d_.absolute_generation(); }
    int32_t generation() const { return this->d_.generation(); }
    int32_t sample() const { return this->d_.sample(); }
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_TRANK_HPP
