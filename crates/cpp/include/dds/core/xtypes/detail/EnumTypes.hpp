// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/EnumTypes.hpp — the delegates of enumerations, aliases and bit sets.

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_ENUMTYPES_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_ENUMTYPES_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <string>
#include <utility>
#include <vector>

#include "dds/core/xtypes/DynamicType.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

/// One enumerator: its name and, when the application assigned one, its value.
struct EnumeratorEntry {
    std::string name;
    bool has_value;
    uint32_t value;
    EnumeratorEntry() : name(), has_value(false), value(0) {}
    EnumeratorEntry(const std::string& n, bool has, uint32_t v) : name(n), has_value(has), value(v) {}
};

class EnumerationType : public DynamicType {
public:
    EnumerationType(const std::string& name, std::vector<EnumeratorEntry> enumerators,
                    std::vector< ::dds::core::xtypes::Annotation> annotations =
                        std::vector< ::dds::core::xtypes::Annotation>())
        : DynamicType(name, TypeKind::ENUMERATION_TYPE, ZERODDS_MOVE(annotations)),
          enumerators_(ZERODDS_MOVE(enumerators)) {}
    const std::vector<EnumeratorEntry>& enumerators() const { return enumerators_; }
    bool equals(const DynamicType& other) const ZERODDS_OVERRIDE {
        const EnumerationType* p = dynamic_cast<const EnumerationType*>(&other);
        if (!DynamicType::equals(other) || p == 0 ||
            p->enumerators_.size() != enumerators_.size()) {
            return false;
        }
        for (std::size_t i = 0; i < enumerators_.size(); ++i) {
            const EnumeratorEntry& a = enumerators_[i];
            const EnumeratorEntry& b = p->enumerators_[i];
            if (a.name != b.name || a.has_value != b.has_value || a.value != b.value) {
                return false;
            }
        }
        return true;
    }

private:
    std::vector<EnumeratorEntry> enumerators_;
};

class AliasType : public DynamicType {
public:
    AliasType(const std::string& name, const ::dds::core::xtypes::DynamicType& aliased)
        : DynamicType(name, TypeKind::ALIAS_TYPE), aliased_(aliased) {}
    const ::dds::core::xtypes::DynamicType& aliased() const { return aliased_; }
    bool equals(const DynamicType& other) const ZERODDS_OVERRIDE {
        const AliasType* p = dynamic_cast<const AliasType*>(&other);
        return DynamicType::equals(other) && p != 0 && p->aliased_ == aliased_;
    }

private:
    ::dds::core::xtypes::DynamicType aliased_;
};

struct BitfieldEntry {
    std::string name;
    uint32_t bits;
    BitfieldEntry() : name(), bits(0) {}
    BitfieldEntry(const std::string& n, uint32_t b) : name(n), bits(b) {}
};

class BitSetType : public DynamicType {
public:
    BitSetType(const std::string& name, std::vector<BitfieldEntry> fields)
        : DynamicType(name, TypeKind::BITSET_TYPE), fields_(ZERODDS_MOVE(fields)) {}
    const std::vector<BitfieldEntry>& fields() const { return fields_; }
    bool equals(const DynamicType& other) const ZERODDS_OVERRIDE {
        const BitSetType* p = dynamic_cast<const BitSetType*>(&other);
        if (!DynamicType::equals(other) || p == 0 || p->fields_.size() != fields_.size()) {
            return false;
        }
        for (std::size_t i = 0; i < fields_.size(); ++i) {
            if (fields_[i].name != p->fields_[i].name || fields_[i].bits != p->fields_[i].bits) {
                return false;
            }
        }
        return true;
    }

private:
    std::vector<BitfieldEntry> fields_;
};

/// One flag of a bit mask: its name and the position of its bit.
struct BitFlagEntry {
    std::string name;
    uint32_t position;
    BitFlagEntry() : name(), position(0) {}
    BitFlagEntry(const std::string& n, uint32_t p) : name(n), position(p) {}
};

class BitMaskType : public DynamicType {
public:
    BitMaskType(const std::string& name, uint32_t bit_bound, std::vector<BitFlagEntry> flags)
        : DynamicType(name, TypeKind::BITMASK_TYPE), bit_bound_(bit_bound), flags_(ZERODDS_MOVE(flags)) {}
    uint32_t bit_bound() const { return bit_bound_; }
    const std::vector<BitFlagEntry>& flags() const { return flags_; }
    bool equals(const DynamicType& other) const ZERODDS_OVERRIDE {
        const BitMaskType* p = dynamic_cast<const BitMaskType*>(&other);
        if (!DynamicType::equals(other) || p == 0 || p->bit_bound_ != bit_bound_ ||
            p->flags_.size() != flags_.size()) {
            return false;
        }
        for (std::size_t i = 0; i < flags_.size(); ++i) {
            if (flags_[i].name != p->flags_[i].name || flags_[i].position != p->flags_[i].position) {
                return false;
            }
        }
        return true;
    }

private:
    uint32_t bit_bound_;
    std::vector<BitFlagEntry> flags_;
};

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_ENUMTYPES_HPP
