// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/Rank.hpp — Rank (DDS-PSM-Cxx 1.0 §7.5.4): the three ranks of a
// sample (DDS 1.4 §2.2.2.5.1.5).

#ifndef ZERODDS_DDS_SUB_RANK_HPP
#define ZERODDS_DDS_SUB_RANK_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>

#include "dds/sub/TRank.hpp"

namespace dds {
namespace sub {

namespace detail {

class Rank {
public:
    Rank() : sample_(0), absolute_generation_(0), generation_(0) {}
    Rank(int32_t s, int32_t a, int32_t ag) : sample_(s), absolute_generation_(a), generation_(ag) {}

    int32_t absolute_generation() const { return absolute_generation_; }
    int32_t generation() const { return generation_; }
    int32_t sample() const { return sample_; }

    bool operator==(const Rank& other) const {
        return sample_ == other.sample_ && absolute_generation_ == other.absolute_generation_ &&
               generation_ == other.generation_;
    }
    bool operator!=(const Rank& other) const { return !(*this == other); }

private:
    int32_t sample_;
    int32_t absolute_generation_;
    int32_t generation_;
};

} // namespace detail

typedef TRank<detail::Rank> Rank;

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_RANK_HPP
