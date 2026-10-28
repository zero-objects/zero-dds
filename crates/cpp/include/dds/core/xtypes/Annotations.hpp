// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/Annotations.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): the annotation classes and
// the functions that make them.

#ifndef ZERODDS_DDS_CORE_XTYPES_ANNOTATIONS_HPP
#define ZERODDS_DDS_CORE_XTYPES_ANNOTATIONS_HPP

#include "dds/core/xtypes/TAnnotation.hpp"
#include "dds/core/xtypes/detail/Annotation.hpp"

namespace dds {
namespace core {
namespace xtypes {

typedef TAnnotation<detail::Annotation> Annotation;
typedef TIdAnnotation<detail::IdAnnotation> IdAnnotation;
typedef TOptionalAnnotation<detail::OptionalAnnotation> OptionalAnnotation;
typedef TKeyAnnotation<detail::KeyAnnotation> KeyAnnotation;
typedef TSharedAnnotation<detail::SharedAnnotation> SharedAnnotation;
typedef TNestedAnnotation<detail::NestedAnnotation> NestedAnnotation;
typedef TExtensibilityAnnotation<detail::ExtensibilityAnnotation> ExtensibilityAnnotation;
typedef TMustUnderstandAnnotation<detail::MustUnderstandAnnotation> MustUnderstandAnnotation;
typedef TVerbatimAnnotation<detail::VerbatimAnnotation> VerbatimAnnotation;
typedef TBitsetAnnotation<detail::BitsetAnnotation> BitsetAnnotation;
typedef TBitBoundAnnotation<detail::BitBoundAnnotation> BitBoundAnnotation;

namespace annotation {

inline IdAnnotation Id(uint32_t id) { return IdAnnotation(id); }
/// The member may be absent (`@optional`).
inline OptionalAnnotation Optional() { return OptionalAnnotation(); }
inline KeyAnnotation Key() { return KeyAnnotation(); }
/// The member is held behind an indirection (`@external`).
inline SharedAnnotation Shared() { return SharedAnnotation(); }
inline NestedAnnotation Nested() { return NestedAnnotation(); }
inline ExtensibilityAnnotation Extensibility(ExtensibilityKind kind) {
    return ExtensibilityAnnotation(kind);
}
inline ExtensibilityAnnotation Final() { return ExtensibilityAnnotation(ExtensibilityKind::FINAL); }
inline ExtensibilityAnnotation Extensible() {
    return ExtensibilityAnnotation(ExtensibilityKind::EXTENSIBLE);
}
inline ExtensibilityAnnotation Mutable() {
    return ExtensibilityAnnotation(ExtensibilityKind::MUTABLE);
}
inline MustUnderstandAnnotation MustUnderstand() { return MustUnderstandAnnotation(); }
inline VerbatimAnnotation Verbatim(const std::string& text) { return VerbatimAnnotation(text); }
inline BitsetAnnotation Bitset() { return BitsetAnnotation(); }
inline BitBoundAnnotation BitBound(uint32_t bound) { return BitBoundAnnotation(bound); }

}  // namespace annotation

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_ANNOTATIONS_HPP
