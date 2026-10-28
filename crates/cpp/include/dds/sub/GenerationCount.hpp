// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/GenerationCount.hpp — GenerationCount (DDS-PSM-Cxx 1.0 §7.5.4):
// the disposed and no-writers generation counts of a sample.

#ifndef ZERODDS_DDS_SUB_GENERATIONCOUNT_HPP
#define ZERODDS_DDS_SUB_GENERATIONCOUNT_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>

#include "dds/sub/TGenerationCount.hpp"

namespace dds {
namespace sub {

namespace detail {

class GenerationCount {
public:
    GenerationCount() : disposed_(0), no_writers_(0) {}
    GenerationCount(int32_t dgc, int32_t nwgc) : disposed_(dgc), no_writers_(nwgc) {}

    int32_t disposed() const { return disposed_; }
    int32_t no_writers() const { return no_writers_; }

    bool operator==(const GenerationCount& other) const {
        return disposed_ == other.disposed_ && no_writers_ == other.no_writers_;
    }
    bool operator!=(const GenerationCount& other) const { return !(*this == other); }

private:
    int32_t disposed_;
    int32_t no_writers_;
};

} // namespace detail

typedef TGenerationCount<detail::GenerationCount> GenerationCount;

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_GENERATIONCOUNT_HPP
