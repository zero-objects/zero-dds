// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/TAnnotation.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): the annotations of a type
// and of a member, as templates over a delegate.

#ifndef ZERODDS_DDS_CORE_XTYPES_TANNOTATION_HPP
#define ZERODDS_DDS_CORE_XTYPES_TANNOTATION_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <memory>
#include <string>

#include "dds/core/Reference.hpp"
#include "dds/core/SafeEnumeration.hpp"

namespace dds {
namespace core {
namespace xtypes {

struct AnnotationKind_def {
    enum type {
        ID_ANNOTATION_TYPE,
        OPTIONAL_ANNOTATION_TYPE,
        KEY_ANNOTATION_TYPE,
        SHARED_ANNOTATION_TYPE,
        NESTED_ANNOTATION_TYPE,
        EXTENSIBILITY_ANNOTATION_TYPE,
        MUST_UNDERSTAND_ANNOTATION_TYPE,
        VERBATIM_ANNOTATION_TYPE,
        BITSET_ANNOTATION_TYPE,
        BIT_BOUND_ANNOTATION_TYPE
    };
};
typedef ::dds::core::safe_enum<AnnotationKind_def> AnnotationKind;

struct ExtensibilityKind_def {
    enum type { FINAL, EXTENSIBLE, MUTABLE };
};
typedef ::dds::core::safe_enum<ExtensibilityKind_def> ExtensibilityKind;

/// An annotation, over any delegate. A reference type: copies share the annotation.
template <typename DELEGATE>
class TAnnotation : public ::dds::core::Reference<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A null annotation.
    TAnnotation() ZERODDS_NOEXCEPT {}
    explicit TAnnotation(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// From an annotation of a more specific kind.
    template <typename D>
    TAnnotation(const TAnnotation<D>& other) : BASE_T(other.delegate()) {}

    AnnotationKind kind() const { return this->delegate()->kind(); }
};

template <typename DELEGATE>
class TIdAnnotation : public TAnnotation<DELEGATE> {
public:
    explicit TIdAnnotation(uint32_t id) : TAnnotation<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(id)) {}
    uint32_t id() const { return this->delegate()->id(); }
};

template <typename DELEGATE>
class TOptionalAnnotation : public TAnnotation<DELEGATE> {
public:
    TOptionalAnnotation() : TAnnotation<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>()) {}
};

template <typename DELEGATE>
class TKeyAnnotation : public TAnnotation<DELEGATE> {
public:
    TKeyAnnotation() : TAnnotation<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>()) {}
};

template <typename DELEGATE>
class TSharedAnnotation : public TAnnotation<DELEGATE> {
public:
    TSharedAnnotation() : TAnnotation<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>()) {}
};

template <typename DELEGATE>
class TNestedAnnotation : public TAnnotation<DELEGATE> {
public:
    TNestedAnnotation() : TAnnotation<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>()) {}
};

template <typename DELEGATE>
class TExtensibilityAnnotation : public TAnnotation<DELEGATE> {
public:
    explicit TExtensibilityAnnotation(ExtensibilityKind xkind)
        : TAnnotation<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(xkind)) {}
    ExtensibilityKind extensibility_kind() const { return this->delegate()->extensibility_kind(); }
};

template <typename DELEGATE>
class TMustUnderstandAnnotation : public TAnnotation<DELEGATE> {
public:
    TMustUnderstandAnnotation() : TAnnotation<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>()) {}
};

template <typename DELEGATE>
class TVerbatimAnnotation : public TAnnotation<DELEGATE> {
public:
    explicit TVerbatimAnnotation(const std::string& text)
        : TAnnotation<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(text)) {}
    const std::string& verbatim_text() const { return this->delegate()->verbatim_text(); }
};

template <typename DELEGATE>
class TBitsetAnnotation : public TAnnotation<DELEGATE> {
public:
    TBitsetAnnotation() : TAnnotation<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>()) {}
};

template <typename DELEGATE>
class TBitBoundAnnotation : public TAnnotation<DELEGATE> {
public:
    explicit TBitBoundAnnotation(uint32_t bound)
        : TAnnotation<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(bound)) {}
    uint32_t bound() const { return this->delegate()->bound(); }
};

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_TANNOTATION_HPP
