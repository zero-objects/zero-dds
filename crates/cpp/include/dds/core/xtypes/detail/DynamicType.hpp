// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/DynamicType.hpp — the delegates of the dynamic types.
//
// A type is a tree: a name, a kind, annotations, and what its kind adds (members, an element
// type, a bound). The delegates are immutable once shared; the operations that "add" a member
// or an annotation build a new delegate.

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_DYNAMICTYPE_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_DYNAMICTYPE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "dds/core/Reference.hpp"
#include "dds/core/xtypes/Annotations.hpp"
#include "dds/core/xtypes/TypeKind.hpp"

namespace dds {
namespace core {
namespace xtypes {

template <typename DELEGATE>
class TDynamicType;
template <typename DELEGATE>
class TMemberType;

namespace detail {

/// Whether two lists of annotations hold the same annotations in the same order.
inline bool same_annotations(const std::vector< ::dds::core::xtypes::Annotation>& a,
                             const std::vector< ::dds::core::xtypes::Annotation>& b) {
    if (a.size() != b.size()) {
        return false;
    }
    for (std::size_t i = 0; i < a.size(); ++i) {
        if (!a[i]->equals(*b[i].delegate())) {
            return false;
        }
    }
    return true;
}

/// A list of one annotation.
inline std::vector< ::dds::core::xtypes::Annotation> single_annotation(
    const ::dds::core::xtypes::Annotation& a) {
    return std::vector< ::dds::core::xtypes::Annotation>(1, a);
}

class DynamicType {
public:
    DynamicType(const std::string& name, TypeKind kind,
                std::vector< ::dds::core::xtypes::Annotation> annotations =
                    std::vector< ::dds::core::xtypes::Annotation>())
        : name_(name), kind_(kind), annotations_(ZERODDS_MOVE(annotations)) {}
    virtual ~DynamicType() {}

    TypeKind kind() const { return kind_; }
    const std::string& name() const { return name_; }
    const std::vector< ::dds::core::xtypes::Annotation>& annotations() const { return annotations_; }

    /// Structural equality: same kind, name, annotations and, for the kinds that have them,
    /// the same parts.
    virtual bool equals(const DynamicType& other) const {
        return kind_ == other.kind_ && name_ == other.name_ &&
               same_annotations(annotations_, other.annotations_);
    }

protected:
    std::string name_;
    TypeKind kind_;
    std::vector< ::dds::core::xtypes::Annotation> annotations_;
};

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_DYNAMICTYPE_HPP
