// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/DynamicData.hpp — the delegate of DynamicData: a type, the handle the C
// API holds it as, and a value.

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_DYNAMICDATA_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_DYNAMICDATA_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/xtypes/DynamicType.hpp"
#include "dds/core/xtypes/detail/Idl.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

class DynamicData {
public:
    /// The default value of `type` (XTypes 1.3 Table 9).
    explicit DynamicData(const ::dds::core::xtypes::DynamicType& type)
        : type_(type), handle_(handle_of(type)), value_(handle_->default_value()) {}

    /// The value `value` of `type`, what a reader decoded.
    DynamicData(const ::dds::core::xtypes::DynamicType& type, ::zerodds::cxx::shared_ptr<DynTypeHandle> handle,
                Json value)
        : type_(type), handle_(ZERODDS_MOVE(handle)), value_(ZERODDS_MOVE(value)) {}

    const ::dds::core::xtypes::DynamicType& type() const { return type_; }
    const ::zerodds::cxx::shared_ptr<DynTypeHandle>& handle() const { return handle_; }
    const Json& json() const { return value_; }

    const Json& member(const std::string& name) const {
        const Json* m = value_.find(name);
        if (m == 0) {
            throw ::dds::core::InvalidArgumentError("DynamicData: " + type_.name() +
                                                    " has no member `" + name + "`");
        }
        return *m;
    }
    void set_member(const std::string& name, Json v) {
        (void)member(name);
        value_.set(name, ZERODDS_MOVE(v));
    }
    void set_json(Json v) { value_ = ZERODDS_MOVE(v); }

    std::vector<uint8_t> encode(bool xcdr2, bool big_endian) const {
        return handle_->encode(value_, xcdr2, big_endian);
    }
    void decode(const uint8_t* bytes, std::size_t len, bool xcdr2, bool big_endian) {
        value_ = handle_->decode(bytes, len, xcdr2, big_endian);
    }

    bool equals(const DynamicData& o) const {
        return type_.name() == o.type_.name() && value_.same(o.value_);
    }

private:
    ::dds::core::xtypes::DynamicType type_;
    ::zerodds::cxx::shared_ptr<DynTypeHandle> handle_;
    Json value_;
};

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_DYNAMICDATA_HPP
