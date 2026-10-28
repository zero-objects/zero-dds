// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/TCollectionTypes.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): the collection types as
// templates over a delegate.

#ifndef ZERODDS_DDS_CORE_XTYPES_TCOLLECTIONTYPES_HPP
#define ZERODDS_DDS_CORE_XTYPES_TCOLLECTIONTYPES_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <memory>
#include <vector>

#include "dds/core/xtypes/DynamicType.hpp"
#include "dds/core/xtypes/detail/CollectionTypes.hpp"

namespace dds {
namespace core {
namespace xtypes {

template <typename DELEGATE>
class TCollectionType : public TDynamicType<DELEGATE> {
public:
    static const uint32_t UNBOUNDED = 0xFFFFFFFFu;

    TCollectionType() ZERODDS_NOEXCEPT {}
    explicit TCollectionType(const typename TDynamicType<DELEGATE>::DELEGATE_REF_T& ref)
        : TDynamicType<DELEGATE>(ref) {}
    template <typename D>
    TCollectionType(const TCollectionType<D>& other) : TDynamicType<DELEGATE>(other.delegate()) {}

    uint32_t bounds() const { return this->delegate()->bounds(); }
};

template <typename DELEGATE>
class TMapType : public TCollectionType<DELEGATE> {
public:
    TMapType(const DynamicType& key_type, const DynamicType& value_type)
        : TCollectionType<DELEGATE>(
              ::zerodds::cxx::make_shared<DELEGATE>(key_type, value_type, detail::kUnbounded)) {}
    TMapType(const DynamicType& key_type, const DynamicType& value_type, uint32_t bounds)
        : TCollectionType<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(key_type, value_type, bounds)) {}

    const DynamicType& key_type() const { return this->delegate()->key_type(); }
    const DynamicType& value_type() const { return this->delegate()->value_type(); }
};

template <typename DELEGATE>
class TSequenceType : public TCollectionType<DELEGATE> {
public:
    explicit TSequenceType(const DynamicType& type)
        : TCollectionType<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(type, detail::kUnbounded)) {}
    TSequenceType(const DynamicType& type, uint32_t bounds)
        : TCollectionType<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(type, bounds)) {}

    const DynamicType& element_type() const { return this->delegate()->element_type(); }
    /// The spelling of the normative header.
    const DynamicType& key_type() const { return element_type(); }
};

/// An array type (`TypeKind::ARRAY_TYPE`). The normative headers name the kind but declare no
/// class for it; this is an extension of ZeroDDS in `dds::core::xtypes`.
template <typename DELEGATE>
class TArrayType : public TCollectionType<DELEGATE> {
public:
    TArrayType(const DynamicType& element, const std::vector<uint32_t>& dimensions)
        : TCollectionType<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(element, dimensions)) {}
    TArrayType(const DynamicType& element, uint32_t extent)
        : TCollectionType<DELEGATE>(
              ::zerodds::cxx::make_shared<DELEGATE>(element, std::vector<uint32_t>(1, extent))) {}

    const DynamicType& element_type() const { return this->delegate()->element_type(); }
    /// The extent of each dimension, the outermost first.
    const std::vector<uint32_t>& dimensions() const { return this->delegate()->dimensions(); }
};

template <typename CHAR_T, template <typename C> class DELEGATE = detail::StringType>
class TStringType : public TCollectionType<DELEGATE<CHAR_T> > {
public:
    TStringType() : TCollectionType<DELEGATE<CHAR_T> >(::zerodds::cxx::make_shared<DELEGATE<CHAR_T> >(detail::kUnbounded)) {}
    explicit TStringType(uint32_t bounds)
        : TCollectionType<DELEGATE<CHAR_T> >(::zerodds::cxx::make_shared<DELEGATE<CHAR_T> >(bounds)) {}
};

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_TCOLLECTIONTYPES_HPP
