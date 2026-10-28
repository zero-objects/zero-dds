// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/CollectionTypes.hpp — the delegates of the collection types.

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_COLLECTIONTYPES_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_COLLECTIONTYPES_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <string>
#include <vector>

#include "dds/core/xtypes/DynamicType.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

/// The bound that means "no bound".
const uint32_t kUnbounded = 0xFFFFFFFFu;

class CollectionType : public DynamicType {
public:
    CollectionType(const std::string& name, TypeKind kind, uint32_t bounds = kUnbounded)
        : DynamicType(name, kind), bounds_(bounds) {}
    uint32_t bounds() const { return bounds_; }
    bool equals(const DynamicType& other) const ZERODDS_OVERRIDE {
        const CollectionType* p = dynamic_cast<const CollectionType*>(&other);
        return DynamicType::equals(other) && p != 0 && p->bounds_ == bounds_;
    }

private:
    uint32_t bounds_;
};

class SequenceType : public CollectionType {
public:
    SequenceType(const ::dds::core::xtypes::DynamicType& element, uint32_t bounds)
        : CollectionType("sequence", TypeKind::SEQUENCE_TYPE, bounds), element_(element) {}
    const ::dds::core::xtypes::DynamicType& element_type() const { return element_; }
    bool equals(const DynamicType& other) const ZERODDS_OVERRIDE {
        const SequenceType* p = dynamic_cast<const SequenceType*>(&other);
        return CollectionType::equals(other) && p != 0 && p->element_ == element_;
    }

private:
    ::dds::core::xtypes::DynamicType element_;
};

class MapType : public CollectionType {
public:
    MapType(const ::dds::core::xtypes::DynamicType& key,
            const ::dds::core::xtypes::DynamicType& value, uint32_t bounds)
        : CollectionType("map", TypeKind::MAP_TYPE, bounds), key_(key), value_(value) {}
    const ::dds::core::xtypes::DynamicType& key_type() const { return key_; }
    const ::dds::core::xtypes::DynamicType& value_type() const { return value_; }
    bool equals(const DynamicType& other) const ZERODDS_OVERRIDE {
        const MapType* p = dynamic_cast<const MapType*>(&other);
        return CollectionType::equals(other) && p != 0 && p->key_ == key_ && p->value_ == value_;
    }

private:
    ::dds::core::xtypes::DynamicType key_;
    ::dds::core::xtypes::DynamicType value_;
};

/// An array: an element type and one extent per dimension (the outermost first).
class ArrayType : public CollectionType {
public:
    ArrayType(const ::dds::core::xtypes::DynamicType& element, const std::vector<uint32_t>& dimensions)
        : CollectionType("array", TypeKind::ARRAY_TYPE, total(dimensions)),
          element_(element),
          dimensions_(dimensions) {}
    const ::dds::core::xtypes::DynamicType& element_type() const { return element_; }
    const std::vector<uint32_t>& dimensions() const { return dimensions_; }
    bool equals(const DynamicType& other) const ZERODDS_OVERRIDE {
        const ArrayType* p = dynamic_cast<const ArrayType*>(&other);
        return CollectionType::equals(other) && p != 0 && p->element_ == element_ &&
               p->dimensions_ == dimensions_;
    }

private:
    static uint32_t total(const std::vector<uint32_t>& dimensions) {
        uint64_t n = 1;
        for (std::vector<uint32_t>::const_iterator it = dimensions.begin(); it != dimensions.end();
             ++it) {
            n *= *it;
        }
        return n > static_cast<uint64_t>(0xFFFFFFFEu) ? 0xFFFFFFFEu : static_cast<uint32_t>(n);
    }

    ::dds::core::xtypes::DynamicType element_;
    std::vector<uint32_t> dimensions_;
};

/// A string of `CHAR_T` (`char` or `wchar_t`).
template <typename CHAR_T>
class StringType : public CollectionType {
public:
    explicit StringType(uint32_t bounds)
        : CollectionType(sizeof(CHAR_T) == sizeof(char) ? "string" : "wstring",
                         TypeKind::STRING_TYPE, bounds) {}
    static bool wide() { return sizeof(CHAR_T) != sizeof(char); }
};

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_COLLECTIONTYPES_HPP
