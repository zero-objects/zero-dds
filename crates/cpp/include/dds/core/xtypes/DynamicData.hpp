// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/DynamicData.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): `DynamicData`. The codec of
// the C API is behind it, so a value has the octets a compiled type of the same IDL has.

#ifndef ZERODDS_DDS_CORE_XTYPES_DYNAMICDATA_HPP
#define ZERODDS_DDS_CORE_XTYPES_DYNAMICDATA_HPP

#include "dds/core/xtypes/TDynamicData.hpp"

namespace dds {
namespace core {
namespace xtypes {

typedef TDynamicData<detail::DynamicData> DynamicData;

}  // namespace xtypes
}  // namespace core

/// The header of the OMG declares `DynamicData` in the namespace `dds::code::xtypes` (a misspelling
/// of `core`, DDS-PSM-Cxx 1.0 §2: where the PDF and the headers differ, the headers prevail); the
/// PDF and every other header say `dds::core::xtypes`. Both spellings name the same type.
namespace code = ::dds::core;
}  // namespace dds

#include "dds/core/xtypes/DynamicTopic.hpp"

#endif  // ZERODDS_DDS_CORE_XTYPES_DYNAMICDATA_HPP
