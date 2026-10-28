// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/Annotation.hpp — the delegates of the annotations.

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_ANNOTATION_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_ANNOTATION_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <string>

#include "dds/core/xtypes/TAnnotation.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

class Annotation {
public:
    explicit Annotation(AnnotationKind kind) : kind_(kind) {}
    virtual ~Annotation() {}
    AnnotationKind kind() const { return kind_; }

    /// Whether `other` is the same annotation with the same value.
    virtual bool equals(const Annotation& other) const { return kind_ == other.kind_; }

private:
    AnnotationKind kind_;
};

class IdAnnotation : public Annotation {
public:
    explicit IdAnnotation(uint32_t id) : Annotation(AnnotationKind::ID_ANNOTATION_TYPE), id_(id) {}
    uint32_t id() const { return id_; }
    bool equals(const Annotation& o) const ZERODDS_OVERRIDE {
        const IdAnnotation* p = dynamic_cast<const IdAnnotation*>(&o);
        return p != 0 && p->id_ == id_;
    }

private:
    uint32_t id_;
};

class OptionalAnnotation : public Annotation {
public:
    OptionalAnnotation() : Annotation(AnnotationKind::OPTIONAL_ANNOTATION_TYPE) {}
};

class KeyAnnotation : public Annotation {
public:
    KeyAnnotation() : Annotation(AnnotationKind::KEY_ANNOTATION_TYPE) {}
};

class SharedAnnotation : public Annotation {
public:
    SharedAnnotation() : Annotation(AnnotationKind::SHARED_ANNOTATION_TYPE) {}
};

class NestedAnnotation : public Annotation {
public:
    NestedAnnotation() : Annotation(AnnotationKind::NESTED_ANNOTATION_TYPE) {}
};

class ExtensibilityAnnotation : public Annotation {
public:
    explicit ExtensibilityAnnotation(ExtensibilityKind xkind)
        : Annotation(AnnotationKind::EXTENSIBILITY_ANNOTATION_TYPE), xkind_(xkind) {}
    ExtensibilityKind extensibility_kind() const { return xkind_; }
    bool equals(const Annotation& o) const ZERODDS_OVERRIDE {
        const ExtensibilityAnnotation* p = dynamic_cast<const ExtensibilityAnnotation*>(&o);
        return p != 0 && p->xkind_ == xkind_;
    }

private:
    ExtensibilityKind xkind_;
};

class MustUnderstandAnnotation : public Annotation {
public:
    MustUnderstandAnnotation() : Annotation(AnnotationKind::MUST_UNDERSTAND_ANNOTATION_TYPE) {}
};

class VerbatimAnnotation : public Annotation {
public:
    explicit VerbatimAnnotation(const std::string& text)
        : Annotation(AnnotationKind::VERBATIM_ANNOTATION_TYPE), text_(text) {}
    const std::string& verbatim_text() const { return text_; }
    bool equals(const Annotation& o) const ZERODDS_OVERRIDE {
        const VerbatimAnnotation* p = dynamic_cast<const VerbatimAnnotation*>(&o);
        return p != 0 && p->text_ == text_;
    }

private:
    std::string text_;
};

class BitsetAnnotation : public Annotation {
public:
    BitsetAnnotation() : Annotation(AnnotationKind::BITSET_ANNOTATION_TYPE) {}
};

class BitBoundAnnotation : public Annotation {
public:
    explicit BitBoundAnnotation(uint32_t bound)
        : Annotation(AnnotationKind::BIT_BOUND_ANNOTATION_TYPE), bound_(bound) {}
    uint32_t bound() const { return bound_; }
    bool equals(const Annotation& o) const ZERODDS_OVERRIDE {
        const BitBoundAnnotation* p = dynamic_cast<const BitBoundAnnotation*>(&o);
        return p != 0 && p->bound_ == bound_;
    }

private:
    uint32_t bound_;
};

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_ANNOTATION_HPP
