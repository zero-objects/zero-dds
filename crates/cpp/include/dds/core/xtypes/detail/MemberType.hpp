// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/MemberType.hpp — the delegate of a member.

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_MEMBERTYPE_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_MEMBERTYPE_HPP


#include "zerodds/psm/cxx.hpp"
#include <string>
#include <utility>
#include <vector>

#include "dds/core/xtypes/Annotations.hpp"
#include "dds/core/xtypes/DynamicType.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

class MemberType {
public:
    MemberType(const std::string& name, ::dds::core::xtypes::DynamicType type,
               std::vector< ::dds::core::xtypes::Annotation> annotations =
                   std::vector< ::dds::core::xtypes::Annotation>())
        : name_(name), type_(ZERODDS_MOVE(type)), annotations_(ZERODDS_MOVE(annotations)) {}

    const std::string& name() const { return name_; }
    const ::dds::core::xtypes::DynamicType& type() const { return type_; }
    const std::vector< ::dds::core::xtypes::Annotation>& annotations() const { return annotations_; }
    std::vector< ::dds::core::xtypes::Annotation>& annotations() { return annotations_; }

    bool equals(const MemberType& other) const {
        return name_ == other.name_ && type_ == other.type_ &&
               same_annotations(annotations_, other.annotations_);
    }

private:
    std::string name_;
    ::dds::core::xtypes::DynamicType type_;
    std::vector< ::dds::core::xtypes::Annotation> annotations_;
};

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_MEMBERTYPE_HPP
