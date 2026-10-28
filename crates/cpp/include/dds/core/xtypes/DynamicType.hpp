// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/DynamicType.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): `DynamicType`.

#ifndef ZERODDS_DDS_CORE_XTYPES_DYNAMICTYPE_HPP
#define ZERODDS_DDS_CORE_XTYPES_DYNAMICTYPE_HPP

#include "dds/core/xtypes/TDynamicType.hpp"

namespace dds {
namespace core {
namespace xtypes {

typedef TDynamicType<detail::DynamicType> DynamicType;

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DYNAMICTYPE_HPP
