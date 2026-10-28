// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/types.hpp — DDS-PSM-Cxx 1.0 §7.2.

#ifndef ZERODDS_DDS_CORE_TYPES_HPP
#define ZERODDS_DDS_CORE_TYPES_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <string>
#include <vector>

namespace dds {
namespace core {

typedef std::vector<uint8_t> ByteSeq;
typedef std::vector<std::string> StringSeq;

/// Null reference type (DDS-PSM-Cxx 1.0 §7.5.1.2).
class null_type { };
static const null_type null = null_type();

namespace policy {
typedef uint32_t QosPolicyId;

/// DDS-XTypes data representation ids (DDS-PSM-Cxx 1.0, dds/core/types.hpp).
typedef uint16_t DataRepresentationId;
typedef std::vector<DataRepresentationId> DataRepresentationIdSeq;
}

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_TYPES_HPP