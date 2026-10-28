// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/TMemberType.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes) `TMemberType<DELEGATE>`:
// a member of a structure or of a union, with its annotations.

#ifndef ZERODDS_DDS_CORE_XTYPES_TMEMBERTYPE_HPP
#define ZERODDS_DDS_CORE_XTYPES_TMEMBERTYPE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <algorithm>
#include <memory>
#include <string>
#include <vector>

#include "dds/core/Reference.hpp"
#include "dds/core/xtypes/Annotations.hpp"
#include "dds/core/xtypes/DynamicType.hpp"
#include "dds/core/xtypes/detail/MemberType.hpp"

namespace dds {
namespace core {
namespace xtypes {

template <typename DELEGATE = detail::MemberType>
class TMemberType : public ::dds::core::Reference<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A null member.
    TMemberType() ZERODDS_NOEXCEPT {}
    explicit TMemberType(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    TMemberType(const std::string& name, const DynamicType& type)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(name, type)) {}
    TMemberType(const std::string& name, const DynamicType& type, const Annotation& annotation)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(name, type, detail::single_annotation(annotation))) {}
    template <typename AnnotationIter>
    TMemberType(const std::string& name, const DynamicType& type, const AnnotationIter& begin,
                const AnnotationIter& end)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(name, type, std::vector<Annotation>(begin, end))) {}
    TMemberType(const std::string& name, const DynamicType& type,
                const std::vector<Annotation>& annotations)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(name, type, annotations)) {}

    const std::string& name() const { return this->delegate()->name(); }
    const DynamicType& type() const { return this->delegate()->type(); }
    const std::vector<Annotation>& annotations() const { return this->delegate()->annotations(); }

    /// This member with `annotation` added; the member itself does not change.
    TMemberType add_annotation(const Annotation& annotation) {
        std::vector<Annotation> next = annotations();
        next.push_back(annotation);
        return TMemberType(name(), type(), next);
    }

    /// This member without `annotation`; the member itself does not change.
    TMemberType remove_annotation(const Annotation& annotation) {
        std::vector<Annotation> next;
        const std::vector<Annotation>& own = annotations();
        for (std::vector<Annotation>::const_iterator it = own.begin(); it != own.end(); ++it) {
            if (!(*it)->equals(*annotation.delegate())) {
                next.push_back(*it);
            }
        }
        return TMemberType(name(), type(), next);
    }

    bool operator==(const TMemberType& that) const {
        if (this->is_nil() || that.is_nil()) {
            return this->is_nil() && that.is_nil();
        }
        return this->delegate()->equals(*that.delegate());
    }
    bool operator!=(const TMemberType& that) const { return !(*this == that); }
};

namespace detail {
inline bool has_annotation(const std::vector< ::dds::core::xtypes::Annotation>& list,
                           AnnotationKind kind) {
    for (std::vector< ::dds::core::xtypes::Annotation>::const_iterator it = list.begin();
         it != list.end(); ++it) {
        if (it->kind() == kind) {
            return true;
        }
    }
    return false;
}

/// The extensibility the annotations state; XTypes 1.3 §7.3.1.2.1.8 makes an unannotated type
/// extensible (appendable).
inline ExtensibilityKind extensibility_of(const std::vector< ::dds::core::xtypes::Annotation>& list) {
    for (std::vector< ::dds::core::xtypes::Annotation>::const_iterator it = list.begin();
         it != list.end(); ++it) {
        if (it->kind() == AnnotationKind::EXTENSIBILITY_ANNOTATION_TYPE) {
            return ::zerodds::cxx::static_pointer_cast<detail::ExtensibilityAnnotation>(it->delegate())->extensibility_kind();
        }
    }
    return ExtensibilityKind::EXTENSIBLE;
}
}  // namespace detail

template <typename T>
bool isOptional(const TMemberType<T>& m) {
    return detail::has_annotation(m.annotations(), AnnotationKind::OPTIONAL_ANNOTATION_TYPE);
}
template <typename T>
bool isShared(const TMemberType<T>& m) {
    return detail::has_annotation(m.annotations(), AnnotationKind::SHARED_ANNOTATION_TYPE);
}
template <typename T>
bool isKey(const TMemberType<T>& m) {
    return detail::has_annotation(m.annotations(), AnnotationKind::KEY_ANNOTATION_TYPE);
}
template <typename T>
bool isMustUnderstand(const TMemberType<T>& m) {
    return detail::has_annotation(m.annotations(), AnnotationKind::MUST_UNDERSTAND_ANNOTATION_TYPE);
}
template <typename T>
bool isBitset(const TMemberType<T>& m) {
    return detail::has_annotation(m.annotations(), AnnotationKind::BITSET_ANNOTATION_TYPE);
}
template <typename T>
bool hasBitbound(const TMemberType<T>& m) {
    return detail::has_annotation(m.annotations(), AnnotationKind::BIT_BOUND_ANNOTATION_TYPE);
}
template <typename T>
int32_t getBitbound(const TMemberType<T>& m) {
    const std::vector<Annotation>& list = m.annotations();
    for (std::vector<Annotation>::const_iterator it = list.begin(); it != list.end(); ++it) {
        if (it->kind() == AnnotationKind::BIT_BOUND_ANNOTATION_TYPE) {
            return static_cast<int32_t>(
                ::zerodds::cxx::static_pointer_cast<detail::BitBoundAnnotation>(it->delegate())->bound());
        }
    }
    return -1;
}
template <typename T>
bool hasId(const TMemberType<T>& m) {
    return detail::has_annotation(m.annotations(), AnnotationKind::ID_ANNOTATION_TYPE);
}
template <typename T>
int32_t getId(const TMemberType<T>& m) {
    const std::vector<Annotation>& list = m.annotations();
    for (std::vector<Annotation>::const_iterator it = list.begin(); it != list.end(); ++it) {
        if (it->kind() == AnnotationKind::ID_ANNOTATION_TYPE) {
            return static_cast<int32_t>(
                ::zerodds::cxx::static_pointer_cast<detail::IdAnnotation>(it->delegate())->id());
        }
    }
    return -1;
}

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_TMEMBERTYPE_HPP
