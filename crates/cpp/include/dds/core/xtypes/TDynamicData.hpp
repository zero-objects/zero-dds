// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/TDynamicData.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): `TDynamicData<DELEGATE>`.
//
// Reads and writes the members of a value of a DynamicType by name or by member id, without
// compile-time knowledge of the type. Members of a structure; a union's members are not
// addressable here.

#ifndef ZERODDS_DDS_CORE_XTYPES_TDYNAMICDATA_HPP
#define ZERODDS_DDS_CORE_XTYPES_TDYNAMICDATA_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <algorithm>
#include <memory>
#include <set>
#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/Reference.hpp"
#include "dds/core/xtypes/DynamicType.hpp"
#include "dds/core/xtypes/EnumTypes.hpp"
#include "dds/core/xtypes/MemberType.hpp"
#include "dds/core/xtypes/StructType.hpp"
#include "dds/core/xtypes/detail/DynamicData.hpp"
#include "dds/core/xtypes/detail/JsonValue.hpp"

namespace dds {
namespace core {
namespace xtypes {

namespace detail {
/// Orders the flags of a bit mask by the position of their bit.
struct BitFlagPositionLess {
    bool operator()(const BitFlagEntry& a, const BitFlagEntry& b) const {
        return a.position < b.position;
    }
};
}  // namespace detail

template <typename DELEGATE>
class TDynamicData : public ::dds::core::Reference<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A null value, the placeholder a container needs before a sample is decoded into it.
    TDynamicData() ZERODDS_NOEXCEPT {}
    TDynamicData(const ::dds::core::null_type&) ZERODDS_NOEXCEPT {}
    explicit TDynamicData(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// The default value of `type` (XTypes 1.3 Table 9).
    TDynamicData(const DynamicType& type) : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(type)) {}

    DynamicType type() const { return this->delegate()->type(); }

    /// The value of the member `mid`.
    template <typename T>
    T value(uint32_t mid) const {
        return detail::from_json<T>(this->delegate()->member(name_of(mid)));
    }
    template <typename T>
    T value(const std::string& mid) const {
        return detail::from_json<T>(this->delegate()->member(mid));
    }
    /// Sets the member `mid`.
    template <typename T>
    void value(uint32_t mid, const T& v) const {
        this->delegate()->set_member(name_of(mid), detail::to_json(v));
    }
    template <typename T>
    void value(const std::string& mid, const T& v) const {
        this->delegate()->set_member(mid, detail::to_json(v));
    }
    void value(const std::string& mid, const char* v) const {
        this->delegate()->set_member(mid, detail::to_json(v));
    }

    /// Sets a bit mask member to the flags in `flags`; a name the bit mask does not have is
    /// `InvalidArgumentError`. The flags are kept in the order of their bits, as the codec gives
    /// them back.
    void value(const std::string& mid, const std::set<std::string>& flags) const {
        const MemberType m = structure().member(mid);
        if (m.type().kind() != TypeKind::BITMASK_TYPE) {
            this->delegate()->set_member(mid, detail::to_json(flags));
            return;
        }
        const ::zerodds::cxx::shared_ptr<detail::BitMaskType> mask = ::zerodds::cxx::static_pointer_cast<detail::BitMaskType>(m.type().delegate());
        std::vector<detail::BitFlagEntry> ordered = mask->flags();
        std::sort(ordered.begin(), ordered.end(), detail::BitFlagPositionLess());
        detail::Json out = detail::Json::array();
        std::size_t used = 0;
        for (std::vector<detail::BitFlagEntry>::const_iterator it = ordered.begin();
             it != ordered.end(); ++it) {
            if (flags.count(it->name) != 0) {
                out.items().push_back(detail::Json::string(it->name));
                ++used;
            }
        }
        if (used != flags.size()) {
            throw ::dds::core::InvalidArgumentError("DynamicData: " + mask->name() +
                                                    " has no such flag");
        }
        this->delegate()->set_member(mid, ZERODDS_MOVE(out));
    }
    void value(uint32_t mid, const std::set<std::string>& flags) const {
        value(name_of(mid), flags);
    }

    MemberType member_type(uint32_t id) const { return structure().member(id); }
    MemberType member_type(const std::string& name) const { return structure().member(name); }

    uint32_t member_id(const std::string& name) const {
        const std::vector<MemberType> all = structure().all_members();
        for (uint32_t i = 0; i < all.size(); ++i) {
            if (all[i].name() == name) {
                return StructType::member_id_of(all[i], i);
            }
        }
        throw ::dds::core::InvalidArgumentError("DynamicData: no member `" + name + "`");
    }

    /// The body of a sample: XCDR2 little endian by default, XCDR1 when `xcdr2` is false.
    std::vector<uint8_t> encode(bool xcdr2 = true, bool big_endian = false) const {
        require_value();
        return this->delegate()->encode(xcdr2, big_endian);
    }
    /// Replaces the value with the one `bytes` hold.
    void decode(const std::vector<uint8_t>& bytes, bool xcdr2 = true, bool big_endian = false) {
        require_value();
        this->delegate()->decode(bytes.empty() ? 0 : &bytes[0], bytes.size(), xcdr2, big_endian);
    }

    /// The value as JSON text, in the dialect of the C API.
    std::string to_json_text() const {
        require_value();
        return this->delegate()->json().dump();
    }
    void from_json_text(const std::string& text) {
        require_value();
        this->delegate()->set_json(detail::Json::parse(text));
    }

    /// Whether both are values of types of the same name and hold the same value.
    bool operator==(const TDynamicData& o) const {
        if (this->is_nil() || o.is_nil()) {
            return this->is_nil() == o.is_nil();
        }
        return this->delegate()->equals(*o.delegate());
    }
    bool operator!=(const TDynamicData& o) const { return !(*this == o); }

private:
    void require_value() const {
        if (this->is_nil()) {
            throw ::dds::core::PreconditionNotMetError("DynamicData: a null value");
        }
    }
    StructType structure() const {
        require_value();
        const DynamicType t = this->delegate()->type();
        if (t.kind() != TypeKind::STRUCTURE_TYPE) {
            throw ::dds::core::UnsupportedError(
                "DynamicData: members are addressed for structures only");
        }
        return StructType(::zerodds::cxx::static_pointer_cast<detail::StructType>(t.delegate()));
    }
    std::string name_of(uint32_t id) const { return structure().member(id).name(); }
};

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_TDYNAMICDATA_HPP
