// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/MemberType.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): `MemberType`.

#ifndef ZERODDS_DDS_CORE_XTYPES_MEMBERTYPE_HPP
#define ZERODDS_DDS_CORE_XTYPES_MEMBERTYPE_HPP

#include "dds/core/xtypes/TMemberType.hpp"

namespace dds {
namespace core {
namespace xtypes {

typedef TMemberType<detail::MemberType> MemberType;

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_MEMBERTYPE_HPP
