// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/TDynamicType.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes) `TDynamicType<DELEGATE>`.

#ifndef ZERODDS_DDS_CORE_XTYPES_TDYNAMICTYPE_HPP
#define ZERODDS_DDS_CORE_XTYPES_TDYNAMICTYPE_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>
#include <string>
#include <vector>

#include "dds/core/Reference.hpp"
#include "dds/core/xtypes/Annotations.hpp"
#include "dds/core/xtypes/TypeKind.hpp"
#include "dds/core/xtypes/detail/DynamicType.hpp"

namespace dds {
namespace core {
namespace xtypes {

/// A type that is known at run time, over any delegate. A reference type: copies share the type.
template <typename DELEGATE = detail::DynamicType>
class TDynamicType : public ::dds::core::Reference<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A null type.
    TDynamicType() ZERODDS_NOEXCEPT {}
    TDynamicType(const ::dds::core::null_type&) ZERODDS_NOEXCEPT {}
    explicit TDynamicType(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// From a type of a more specific kind (a structure is a dynamic type).
    template <typename D>
    TDynamicType(const TDynamicType<D>& other) : BASE_T(other.delegate()) {}

    TypeKind kind() const { return this->delegate()->kind(); }
    const std::string& name() const { return this->delegate()->name(); }
    const std::vector<Annotation>& annotations() const { return this->delegate()->annotations(); }

    /// Structural equality (the references need not be the same).
    bool operator==(const TDynamicType& that) const {
        if (this->is_nil() || that.is_nil()) {
            return this->is_nil() && that.is_nil();
        }
        return this->delegate()->equals(*that.delegate());
    }
    bool operator!=(const TDynamicType& that) const { return !(*this == that); }
    bool operator==(const ::dds::core::null_type&) const { return this->is_nil(); }
    bool operator!=(const ::dds::core::null_type&) const { return !this->is_nil(); }

protected:
    TDynamicType(const std::string& name, TypeKind kind)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(name, kind)) {}
    TDynamicType(const std::string& name, TypeKind kind, const Annotation& annotation)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(name, kind, detail::single_annotation(annotation))) {}
    TDynamicType(const std::string& name, TypeKind kind, const std::vector<Annotation>& annotations)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(name, kind, annotations)) {}
    template <typename FWI>
    TDynamicType(const std::string& name, TypeKind kind, const FWI& annotation_begin,
                 const FWI& annotation_end)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(name, kind,
                                            std::vector<Annotation>(annotation_begin, annotation_end))) {}
};

template <typename T>
bool isPrimitiveType(const TDynamicType<T>& t) {
    return (static_cast<int>(t.kind().underlying()) & static_cast<int>(TypeKind::PRIMITIVE_TYPE)) != 0;
}
template <typename T>
bool isConstructedType(const TDynamicType<T>& t) {
    return (static_cast<int>(t.kind().underlying()) & static_cast<int>(TypeKind::CONSTRUCTED_TYPE)) != 0;
}
template <typename T>
bool isCollectionType(const TDynamicType<T>& t) {
    return (static_cast<int>(t.kind().underlying()) & static_cast<int>(TypeKind::COLLECTION_TYPE)) != 0;
}
template <typename T>
bool isAggregationType(const TDynamicType<T>& t) {
    return (static_cast<int>(t.kind().underlying()) & static_cast<int>(TypeKind::AGGREGATION_TYPE)) != 0;
}

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_TDYNAMICTYPE_HPP
