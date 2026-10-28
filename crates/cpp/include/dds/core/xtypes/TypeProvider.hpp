// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/TypeProvider.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): `TypeProvider`.

#ifndef ZERODDS_DDS_CORE_XTYPES_TYPEPROVIDER_HPP
#define ZERODDS_DDS_CORE_XTYPES_TYPEPROVIDER_HPP

#include "dds/core/xtypes/TTypeProvider.hpp"

namespace dds {
namespace core {
namespace xtypes {

typedef TTypeProvider<detail::TypeProvider> TypeProvider;

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_TYPEPROVIDER_HPP
