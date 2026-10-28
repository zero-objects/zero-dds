// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/cond/Condition.hpp — DDS-PSM-Cxx 1.0 §7.5 Condition + WaitSet.
// Public header: provides the normative API types (typedefs).

#ifndef ZERODDS_DDS_CORE_COND_CONDITION_HPP
#define ZERODDS_DDS_CORE_COND_CONDITION_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/cond/TCondition.hpp"
#include "dds/core/cond/GuardCondition.hpp"
#include "dds/core/cond/StatusCondition.hpp"
#include "dds/core/cond/WaitSet.hpp"

namespace dds {
namespace core {
namespace cond {

/// Condition (Spec §7.5). Base condition type with default delegate.
typedef TCondition<> Condition;

} // namespace cond
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_COND_CONDITION_HPP