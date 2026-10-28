// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/StructType.hpp — the delegate of a structure.

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_STRUCTTYPE_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_STRUCTTYPE_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "dds/core/xtypes/Annotations.hpp"
#include "dds/core/xtypes/DynamicType.hpp"
#include "dds/core/xtypes/MemberType.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

class StructType : public DynamicType {
public:
    StructType(const std::string& name, TypeKind kind = TypeKind::STRUCTURE_TYPE)
        : DynamicType(name, kind) {}
    StructType(const std::string& name, ::zerodds::cxx::shared_ptr<StructType> parent,
               std::vector< ::dds::core::xtypes::MemberType> members,
               std::vector< ::dds::core::xtypes::Annotation> annotations =
                   std::vector< ::dds::core::xtypes::Annotation>())
        : DynamicType(name, TypeKind::STRUCTURE_TYPE, ZERODDS_MOVE(annotations)),
          parent_(ZERODDS_MOVE(parent)),
          members_(ZERODDS_MOVE(members)) {}

    const ::zerodds::cxx::shared_ptr<StructType>& parent() const { return parent_; }
    const std::vector< ::dds::core::xtypes::MemberType>& members() const { return members_; }

    bool equals(const DynamicType& other) const ZERODDS_OVERRIDE {
        const StructType* p = dynamic_cast<const StructType*>(&other);
        if (!DynamicType::equals(other) || p == 0 || p->members_.size() != members_.size()) {
            return false;
        }
        if ((parent_ == 0) != (p->parent_ == 0) ||
            (parent_ != 0 && !parent_->equals(*p->parent_))) {
            return false;
        }
        for (std::size_t i = 0; i < members_.size(); ++i) {
            if (!(members_[i] == p->members_[i])) {
                return false;
            }
        }
        return true;
    }

private:
    ::zerodds::cxx::shared_ptr<StructType> parent_;
    std::vector< ::dds::core::xtypes::MemberType> members_;
};

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_STRUCTTYPE_HPP
