// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/StructType.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): `StructType`.

#ifndef ZERODDS_DDS_CORE_XTYPES_STRUCTTYPE_HPP
#define ZERODDS_DDS_CORE_XTYPES_STRUCTTYPE_HPP

#include "dds/core/xtypes/TStructType.hpp"

namespace dds {
namespace core {
namespace xtypes {

typedef TStructType<detail::StructType> StructType;
typedef TStructForwardDeclaration<detail::StructType> StructForwardDeclaration;

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_STRUCTTYPE_HPP
