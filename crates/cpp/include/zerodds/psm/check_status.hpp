// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/psm/check_status.hpp — maps a C ABI return code to the exception
// DDS-PSM-Cxx 1.0 §7.5.5 names. Vendor helper, kept out of `dds::core`
// (§2.2).

#ifndef ZERODDS_PSM_CHECK_STATUS_HPP
#define ZERODDS_PSM_CHECK_STATUS_HPP

#include "dds/core/Exception.hpp"

namespace zerodds {
namespace psm {

/// Throws the right `dds::core::*` exception based on the C-FFI status code.
inline void check_status(int code, const char* what) {
    switch (code) {
        case 0: return; // ZERODDS_OK
        case -1: throw ::dds::core::Error(what);
        case -2: throw ::dds::core::AlreadyClosedError(what);
        case -3: throw ::dds::core::InvalidArgumentError(what);
        case -4: throw ::dds::core::TimeoutError(what);
        case -5: throw ::dds::core::PreconditionNotMetError(what);
        case -6: throw ::dds::core::InvalidArgumentError(what);
        case -7: return; // NO_DATA — not an exception
        case -8: throw ::dds::core::OutOfResourcesError(what);
        case -9: throw ::dds::core::NotEnabledError(what);
        case -10: throw ::dds::core::ImmutablePolicyError(what);
        case -11: throw ::dds::core::InconsistentPolicyError(what);
        case -12: throw ::dds::core::AlreadyClosedError(what);
        case -13: throw ::dds::core::UnsupportedError(what);
        case -14: throw ::dds::core::IllegalOperationError(what);
        default: throw ::dds::core::Error(what);
    }
}

} // namespace psm
} // namespace zerodds

#endif // ZERODDS_PSM_CHECK_STATUS_HPP
