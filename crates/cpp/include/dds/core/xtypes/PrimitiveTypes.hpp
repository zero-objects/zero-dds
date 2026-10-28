// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/PrimitiveTypes.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): the primitive types.
// `NAME` is the IDL keyword the type is written as.

#ifndef ZERODDS_DDS_CORE_XTYPES_PRIMITIVETYPES_HPP
#define ZERODDS_DDS_CORE_XTYPES_PRIMITIVETYPES_HPP

#include <stdint.h>

#include <string>

#include "dds/core/xtypes/DynamicType.hpp"
#include "dds/core/xtypes/TypeKind.hpp"

namespace dds {
namespace core {
namespace xtypes {

template <typename T>
struct dynamic_type_traits {
    static const TypeKind::type TYPE_ID = TypeKind::NO_TYPE;
    static const char* name() { return ""; }
};

#define ZERODDS_XTYPES_PRIMITIVE(CPP, KIND, IDL)                          \
    template <>                                                           \
    struct dynamic_type_traits<CPP> {                                     \
        static const TypeKind::type TYPE_ID = TypeKind::KIND;            \
        static const char* name() { return IDL; }                         \
    };

ZERODDS_XTYPES_PRIMITIVE(bool, BOOLEAN_TYPE, "boolean")
ZERODDS_XTYPES_PRIMITIVE(uint8_t, UINT_8_TYPE, "octet")
ZERODDS_XTYPES_PRIMITIVE(int16_t, INT_16_TYPE, "short")
ZERODDS_XTYPES_PRIMITIVE(uint16_t, UINT_16_TYPE, "unsigned short")
ZERODDS_XTYPES_PRIMITIVE(int32_t, INT_32_TYPE, "long")
ZERODDS_XTYPES_PRIMITIVE(uint32_t, UINT_32_TYPE, "unsigned long")
ZERODDS_XTYPES_PRIMITIVE(int64_t, INT_64_TYPE, "long long")
ZERODDS_XTYPES_PRIMITIVE(uint64_t, UINT_64_TYPE, "unsigned long long")
ZERODDS_XTYPES_PRIMITIVE(float, FLOAT_32_TYPE, "float")
ZERODDS_XTYPES_PRIMITIVE(double, FLOAT_64_TYPE, "double")
ZERODDS_XTYPES_PRIMITIVE(long double, FLOAT_128_TYPE, "long double")
ZERODDS_XTYPES_PRIMITIVE(char, CHAR_8_TYPE, "char")
ZERODDS_XTYPES_PRIMITIVE(wchar_t, CHAR_32_TYPE, "wchar")
#undef ZERODDS_XTYPES_PRIMITIVE

/// The dynamic type of the primitive `T`.
template <typename T>
class TPrimitiveType : public DynamicType {
public:
    TPrimitiveType()
        : DynamicType(std::string(dynamic_type_traits<T>::name()),
                      TypeKind(dynamic_type_traits<T>::TYPE_ID)) {}
};

template <typename T>
TPrimitiveType<T> PrimitiveType() {
    static const TPrimitiveType<T> t;
    return t;
}

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_PRIMITIVETYPES_HPP
