// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/detail/macros.hpp — the implementation side of dds/core/macros.hpp.

#ifndef ZERODDS_DDS_CORE_DETAIL_MACROS_HPP
#define ZERODDS_DDS_CORE_DETAIL_MACROS_HPP

#include "zerodds/psm/cxx.hpp"

// `static_assert` with C++11, a negative array size typedef before (see zerodds/psm/cxx.hpp).
#define OMG_DDS_STATIC_ASSERT_DETAIL(expr) ZERODDS_STATIC_ASSERT((expr), #expr)

// ZeroDDS has no logging hook in the C++ headers: the macro evaluates nothing.
#define OMG_DDS_LOG_DETAIL(kind, msg) ((void)0)

#define OMG_DDS_DEFAULT_STATE_BIT_COUNT_DETAIL 16
#define OMG_DDS_DEFAULT_STATUS_COUNT_DETAIL 15

// Header-only library: nothing to export.
#define OMG_DDS_API_DETAIL

#endif // ZERODDS_DDS_CORE_DETAIL_MACROS_HPP
