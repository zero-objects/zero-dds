// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/EnumTypes.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): `EnumerationType`, `AliasType`
// and `BitSetType`. The value of an enumeration is the name of an enumerator (a string in
// `DynamicData`), the value of a bit set an object of its bit fields.

#ifndef ZERODDS_DDS_CORE_XTYPES_ENUMTYPES_HPP
#define ZERODDS_DDS_CORE_XTYPES_ENUMTYPES_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/xtypes/DynamicType.hpp"
#include "dds/core/xtypes/detail/EnumTypes.hpp"

namespace dds {
namespace core {
namespace xtypes {

/// An enumerator with the value the application assigned.
struct Enumerator {
    Enumerator(const std::string& n, uint32_t v) : name(n), value(v), assigned(true) {}
    explicit Enumerator(const std::string& n) : name(n), value(0), assigned(false) {}
    std::string name;
    uint32_t value;
    bool assigned;
};

template <typename UTYPE = uint32_t>
class EnumerationType : public TDynamicType<detail::EnumerationType> {
public:
    typedef TDynamicType<detail::EnumerationType> BASE_T;
    EnumerationType() ZERODDS_NOEXCEPT {}
    explicit EnumerationType(const std::string& name) : BASE_T(make(name, std::vector<Enumerator>())) {}
    EnumerationType(const std::string& name, const std::vector<Enumerator>& enumerators)
        : BASE_T(make(name, enumerators)) {}

    std::vector<Enumerator> enumerators() const {
        std::vector<Enumerator> out;
        const std::vector<detail::EnumeratorEntry>& list = this->delegate()->enumerators();
        for (std::vector<detail::EnumeratorEntry>::const_iterator it = list.begin();
             it != list.end(); ++it) {
            out.push_back(it->has_value ? Enumerator(it->name, it->value) : Enumerator(it->name));
        }
        return out;
    }
    EnumerationType add_enumerator(const Enumerator& e) const {
        std::vector<Enumerator> all = enumerators();
        all.push_back(e);
        return EnumerationType(this->name(), all);
    }
    EnumerationType add_enumerator(const std::string& name) const {
        return add_enumerator(Enumerator(name));
    }
    EnumerationType add_enumerator(const std::string& name, UTYPE value) const {
        return add_enumerator(Enumerator(name, static_cast<uint32_t>(value)));
    }

private:
    static typename BASE_T::DELEGATE_REF_T make(const std::string& name,
                                                const std::vector<Enumerator>& list) {
        std::vector<detail::EnumeratorEntry> entries;
        for (std::vector<Enumerator>::const_iterator it = list.begin(); it != list.end(); ++it) {
            entries.push_back(detail::EnumeratorEntry(it->name, it->assigned, it->value));
        }
        return ::zerodds::cxx::make_shared<detail::EnumerationType>(name, ZERODDS_MOVE(entries));
    }
};

/// A named alias of another type (IDL `typedef`).
class AliasType : public TDynamicType<detail::AliasType> {
public:
    typedef TDynamicType<detail::AliasType> BASE_T;
    AliasType() ZERODDS_NOEXCEPT {}
    AliasType(const std::string& name, const DynamicType& aliased)
        : BASE_T(::zerodds::cxx::make_shared<detail::AliasType>(name, aliased)) {}
    const DynamicType& aliased() const { return this->delegate()->aliased(); }
};

/// A bit set: named bit fields of a given width (IDL `bitset`).
struct BitField {
    BitField(const std::string& n, uint32_t b) : name(n), bits(b) {}
    std::string name;
    uint32_t bits;
};

class BitSetType : public TDynamicType<detail::BitSetType> {
public:
    typedef TDynamicType<detail::BitSetType> BASE_T;
    BitSetType() ZERODDS_NOEXCEPT {}
    BitSetType(const std::string& name, const std::vector<BitField>& fields) {
        std::vector<detail::BitfieldEntry> entries;
        for (std::vector<BitField>::const_iterator it = fields.begin(); it != fields.end(); ++it) {
            entries.push_back(detail::BitfieldEntry(it->name, it->bits));
        }
        static_cast<BASE_T&>(*this) =
            BASE_T(::zerodds::cxx::make_shared<detail::BitSetType>(name, ZERODDS_MOVE(entries)));
    }
    std::vector<BitField> fields() const {
        std::vector<BitField> out;
        const std::vector<detail::BitfieldEntry>& list = this->delegate()->fields();
        for (std::vector<detail::BitfieldEntry>::const_iterator it = list.begin(); it != list.end();
             ++it) {
            out.push_back(BitField(it->name, it->bits));
        }
        return out;
    }
};

/// A flag of a bit mask: its name and the position of its bit.
struct BitFlag {
    BitFlag(const std::string& n, uint32_t p) : name(n), position(p) {}
    std::string name;
    uint32_t position;
};

/// A bit mask: named flags in a value of `bit_bound` bits (IDL `bitmask`, XTypes 1.3
/// `TK_BITMASK`). The value in `DynamicData` is the set of the names of the flags that are set
/// (`std::set<std::string>`).
class BitMaskType : public TDynamicType<detail::BitMaskType> {
public:
    typedef TDynamicType<detail::BitMaskType> BASE_T;
    BitMaskType() ZERODDS_NOEXCEPT {}
    BitMaskType(const std::string& name, uint32_t bit_bound, const std::vector<BitFlag>& flags) {
        if (bit_bound == 0 || bit_bound > 64) {
            throw ::dds::core::InvalidArgumentError("BitMaskType: the bit bound is 1 to 64");
        }
        std::vector<detail::BitFlagEntry> entries;
        for (std::vector<BitFlag>::const_iterator it = flags.begin(); it != flags.end(); ++it) {
            if (it->position >= bit_bound) {
                throw ::dds::core::InvalidArgumentError("BitMaskType: the flag `" + it->name +
                                                        "` lies beyond the bit bound");
            }
            entries.push_back(detail::BitFlagEntry(it->name, it->position));
        }
        static_cast<BASE_T&>(*this) =
            BASE_T(::zerodds::cxx::make_shared<detail::BitMaskType>(name, bit_bound, ZERODDS_MOVE(entries)));
    }
    uint32_t bit_bound() const { return this->delegate()->bit_bound(); }
    std::vector<BitFlag> flags() const {
        std::vector<BitFlag> out;
        const std::vector<detail::BitFlagEntry>& list = this->delegate()->flags();
        for (std::vector<detail::BitFlagEntry>::const_iterator it = list.begin(); it != list.end();
             ++it) {
            out.push_back(BitFlag(it->name, it->position));
        }
        return out;
    }
};

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_ENUMTYPES_HPP
