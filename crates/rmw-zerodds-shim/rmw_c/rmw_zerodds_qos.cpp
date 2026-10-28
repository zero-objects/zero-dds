// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// rmw_zerodds_qos.cpp — rmw_qos_profile_check_compatible.
//
// The check is the reference implementation in rmw_dds_common, which
// rmw_cyclonedds_cpp (1.3.4, rmw_node.cpp) and rmw_fastrtps_cpp call as
// well: every RMW then gives `ros2 doctor --report` and rcl's incompatible-QoS
// warnings the same answer for the same pair of profiles. It is C++, so it
// lives here and not in rmw_zerodds.c; the RMW used to report every pair
// compatible.

#include "rmw/qos_profiles.h"
#include "rmw/types.h"
#include "rmw_dds_common/qos.hpp"

extern "C" rmw_ret_t rmw_qos_profile_check_compatible(
  const rmw_qos_profile_t publisher_profile,
  const rmw_qos_profile_t subscription_profile,
  rmw_qos_compatibility_type_t * compatibility,
  char * reason,
  size_t reason_size)
{
  return rmw_dds_common::qos_profile_check_compatible(
    publisher_profile, subscription_profile, compatibility, reason, reason_size);
}
