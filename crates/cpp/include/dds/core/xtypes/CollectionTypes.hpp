// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/CollectionTypes.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): the collection types.

#ifndef ZERODDS_DDS_CORE_XTYPES_COLLECTIONTYPES_HPP
#define ZERODDS_DDS_CORE_XTYPES_COLLECTIONTYPES_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/xtypes/TCollectionTypes.hpp"

namespace dds {
namespace core {
namespace xtypes {

typedef TCollectionType<detail::CollectionType> CollectionType;
typedef TMapType<detail::MapType> MapType;
typedef TSequenceType<detail::SequenceType> SequenceType;
typedef TArrayType<detail::ArrayType> ArrayType;
/// `StringType<char>` is `string`, `StringType<wchar_t>` is `wstring`.
#if ZERODDS_CXX11
template <typename CHAR_T>
using StringType = TStringType<CHAR_T, detail::StringType>;
#else
// C++03 has no template alias: a class template with the constructors of `TStringType`.
template <typename CHAR_T>
class StringType : public TStringType<CHAR_T, detail::StringType> {
public:
    StringType() : TStringType<CHAR_T, detail::StringType>() {}
    explicit StringType(uint32_t bounds) : TStringType<CHAR_T, detail::StringType>(bounds) {}
};
#endif

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_COLLECTIONTYPES_HPP
