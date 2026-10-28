// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/TTypeProvider.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): `TTypeProvider<DELEGATE>`.
//
// Creates types from an external representation: IDL or a DDS-XML type library at a file:,
// data:, http: or https: URI.

#ifndef ZERODDS_DDS_CORE_XTYPES_TTYPEPROVIDER_HPP
#define ZERODDS_DDS_CORE_XTYPES_TTYPEPROVIDER_HPP

#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/xtypes/DynamicType.hpp"
#include "dds/core/xtypes/detail/TypeProvider.hpp"

namespace dds {
namespace core {
namespace xtypes {

template <typename DELEGATE>
class TTypeProvider {
public:
    /// The first type the document defines.
    static DynamicType load_type(const std::string& uri) {
        const detail::Json doc = DELEGATE::describe(uri);
        ::zerodds::psm::xtypes_detail::TypeBuilder builder(doc);
        if (builder.names().empty()) {
            throw ::dds::core::InvalidArgumentError("TypeProvider: the document defines no type");
        }
        return builder.build(builder.names().front());
    }

    /// Every type the document defines, in the order it declares them.
    static std::vector<DynamicType> load_types(const std::string& uri) {
        const detail::Json doc = DELEGATE::describe(uri);
        ::zerodds::psm::xtypes_detail::TypeBuilder builder(doc);
        std::vector<DynamicType> out;
        const std::vector<std::string>& names = builder.names();
        for (std::vector<std::string>::const_iterator it = names.begin(); it != names.end(); ++it) {
            out.push_back(builder.build(*it));
        }
        return out;
    }

    /// The type `name` (`a::b::T`) of the document.
    static DynamicType load_type(const std::string& uri, const std::string& name) {
        const detail::Json doc = DELEGATE::describe(uri);
        ::zerodds::psm::xtypes_detail::TypeBuilder builder(doc);
        return builder.build(name.rfind("::", 0) == 0 ? name.substr(2) : name);
    }
};

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_TTYPEPROVIDER_HPP
