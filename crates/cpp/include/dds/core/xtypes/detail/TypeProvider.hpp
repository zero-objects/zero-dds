// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/TypeProvider.hpp — types from a document. The C API reads the document
// (IDL or a DDS-XML type library, at a file:, data:, http: or https: URI) and describes its
// types as JSON; this builds the DynamicType trees from it.

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_TYPEPROVIDER_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_TYPEPROVIDER_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <cerrno>
#include <cstdlib>
#include <map>
#include <set>
#include <stdexcept>
#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/xtypes/Annotations.hpp"
#include "dds/core/xtypes/CollectionTypes.hpp"
#include "dds/core/xtypes/EnumTypes.hpp"
#include "dds/core/xtypes/PrimitiveTypes.hpp"
#include "dds/core/xtypes/StructType.hpp"
#include "dds/core/xtypes/UnionType.hpp"
#include "dds/core/xtypes/detail/Json.hpp"
#include "zerodds.h"
#include "zerodds/psm/check_status.hpp"

namespace zerodds {
namespace psm {
namespace xtypes_detail {

using ::dds::core::xtypes::detail::Json;
using ::dds::core::xtypes::detail::kUnbounded;


class TypeBuilder {
public:
    explicit TypeBuilder(const Json& document) {
        const Json* types = document.find("types");
        if (types == 0) {
            throw ::dds::core::InvalidDataError("TypeProvider: the description has no types");
        }
        for (std::vector<Json>::const_iterator t_it = types->items().begin(); t_it != types->items().end(); ++t_it) {
            const Json& t = *t_it;
            const std::string name = text(t, "name");
            declared_[name] = &t;
            // An annotation declaration is not a type of data: it is never "a type the
            // document defines" for load_type or load_types, and asking for it by name is
            // refused in `declare`.
            if (text(t, "kind") != "annotation") {
                order_.push_back(name);
            }
        }
    }

    const std::vector<std::string>& names() const { return order_; }

    ::dds::core::xtypes::DynamicType build(const std::string& name) {
        std::map<std::string, ::dds::core::xtypes::DynamicType>::iterator done = built_.find(name);
        if (done != built_.end()) {
            return done->second;
        }
        std::map<std::string, const Json*>::iterator found = declared_.find(name);
        if (found == declared_.end()) {
            throw ::dds::core::InvalidArgumentError("TypeProvider: the document defines no type `" +
                                                    name + "`");
        }
        if (!building_.insert(name).second) {
            throw ::dds::core::UnsupportedError("TypeProvider: `" + name + "` contains itself");
        }
        ::dds::core::xtypes::DynamicType type = declare(*found->second, name);
        building_.erase(name);
        built_[name] = type;
        return type;
    }

private:
    static const Json& field(const Json& j, const char* key) {
        const Json* f = j.find(key);
        if (f == 0) {
            throw ::dds::core::InvalidDataError(std::string("TypeProvider: the description has no `") +
                                                key + "`");
        }
        return *f;
    }
    static std::string text(const Json& j, const char* key) { return field(j, key).text(); }
    /// The semantics of `std::stoul` and `std::stoll`: `std::invalid_argument` when nothing
    /// converts, `std::out_of_range` when the value does not fit.
    static unsigned long stoul_(const std::string& s) {
        errno = 0;
        char* end = 0;
        const unsigned long v = std::strtoul(s.c_str(), &end, 10);
        if (end == s.c_str()) {
            throw std::invalid_argument("stoul");
        }
        if (errno == ERANGE) {
            throw std::out_of_range("stoul");
        }
        return v;
    }
    static long long stoll_(const std::string& s) {
        errno = 0;
        char* end = 0;
        const long long v = std::strtoll(s.c_str(), &end, 10);
        if (end == s.c_str()) {
            throw std::invalid_argument("stoll");
        }
        if (errno == ERANGE) {
            throw std::out_of_range("stoll");
        }
        return v;
    }
    static bool flag(const Json& j, const char* key) {
        const Json* f = j.find(key);
        return f != 0 && f->kind() == Json::Kind::Bool && f->as_bool();
    }
    static bool present(const Json& j, const char* key) {
        const Json* f = j.find(key);
        return f != 0 && f->kind() != Json::Kind::Null;
    }
    static uint32_t bound(const Json& j) {
        return present(j, "bound") ? static_cast<uint32_t>(stoul_(text(j, "bound")))
                                   : kUnbounded;
    }

    ::dds::core::xtypes::DynamicType primitive(const std::string& n) {
        using namespace ::dds::core::xtypes;
        if (n == "boolean") return PrimitiveType<bool>();
        if (n == "octet") return PrimitiveType<uint8_t>();
        if (n == "char") return PrimitiveType<char>();
        if (n == "wchar") return PrimitiveType<wchar_t>();
        if (n == "short") return PrimitiveType<int16_t>();
        if (n == "unsigned short") return PrimitiveType<uint16_t>();
        if (n == "long") return PrimitiveType<int32_t>();
        if (n == "unsigned long") return PrimitiveType<uint32_t>();
        if (n == "long long") return PrimitiveType<int64_t>();
        if (n == "unsigned long long") return PrimitiveType<uint64_t>();
        if (n == "float") return PrimitiveType<float>();
        if (n == "double") return PrimitiveType<double>();
        if (n == "long double") return PrimitiveType<long double>();
        throw ::dds::core::UnsupportedError("TypeProvider: the primitive type `" + n + "` has no class");
    }

    ::dds::core::xtypes::DynamicType spec(const Json& t) {
        using namespace ::dds::core::xtypes;
        const std::string k = text(t, "k");
        if (k == "prim") return primitive(text(t, "n"));
        if (k == "string") {
            return flag(t, "wide") ? ::dds::core::xtypes::DynamicType(StringType<wchar_t>(bound(t)))
                                   : ::dds::core::xtypes::DynamicType(StringType<char>(bound(t)));
        }
        if (k == "seq") return SequenceType(spec(field(t, "e")), bound(t));
        if (k == "array") {
            std::vector<uint32_t> extents;
            for (std::vector<Json>::const_iterator d_it = field(t, "dims").items().begin(); d_it != field(t, "dims").items().end(); ++d_it) {
            const Json& d = *d_it;
                extents.push_back(static_cast<uint32_t>(stoul_(d.text())));
            }
            return ArrayType(spec(field(t, "e")), extents);
        }
        if (k == "map") return MapType(spec(field(t, "key")), spec(field(t, "v")), bound(t));
        if (k == "ref") return build(text(t, "n"));
        throw ::dds::core::UnsupportedError("TypeProvider: a member of this kind has no class: " +
                                            (present(t, "what") ? text(t, "what") : k));
    }

    static std::vector< ::dds::core::xtypes::Annotation> member_annotations(const Json& m) {
        using namespace ::dds::core::xtypes;
        std::vector< ::dds::core::xtypes::Annotation> out;
        if (flag(m, "key")) out.push_back(annotation::Key());
        if (flag(m, "optional")) out.push_back(annotation::Optional());
        if (flag(m, "external")) out.push_back(annotation::Shared());
        if (flag(m, "must_understand")) out.push_back(annotation::MustUnderstand());
        if (present(m, "id")) {
            out.push_back(annotation::Id(static_cast<uint32_t>(stoul_(text(m, "id")))));
        }
        return out;
    }

    static std::vector< ::dds::core::xtypes::Annotation> type_annotations(const Json& d) {
        using namespace ::dds::core::xtypes;
        std::vector< ::dds::core::xtypes::Annotation> out;
        if (present(d, "ext")) {
            const std::string e = text(d, "ext");
            if (e == "final") out.push_back(annotation::Final());
            else if (e == "mutable") out.push_back(annotation::Mutable());
            else out.push_back(annotation::Extensible());
        }
        if (flag(d, "nested")) out.push_back(annotation::Nested());
        return out;
    }

    ::dds::core::xtypes::DynamicType cases_type(const Json& c) { return spec(field(c, "type")); }

    /// The cases of the union `d` with discriminator type `D`: each label of a
    /// case read as a signed number, or as an unsigned one (enumerator values).
    template <typename D>
    std::vector< ::dds::core::xtypes::UnionCase<D> > union_cases(const Json& d, bool unsigned_labels) {
        using namespace ::dds::core::xtypes;
        std::vector<UnionCase<D> > cases;
        for (std::vector<Json>::const_iterator c_it = field(d, "cases").items().begin(); c_it != field(d, "cases").items().end(); ++c_it) {
            const Json& c = *c_it;
            const MemberType member(text(c, "name"), cases_type(c));
            for (std::vector<Json>::const_iterator label_it = field(c, "labels").items().begin(); label_it != field(c, "labels").items().end(); ++label_it) {
                const std::string& t = label_it->text();
                const D value = unsigned_labels ? static_cast<D>(stoul_(t)) : static_cast<D>(stoll_(t));
                cases.push_back(UnionCase<D>(value, member));
            }
            if (flag(c, "default")) {
                cases.push_back(UnionCase<D>(member));
            }
        }
        return cases;
    }

    template <typename D>
    ::dds::core::xtypes::DynamicType union_of(const Json& d, const std::string& name) {
        using namespace ::dds::core::xtypes;
        const std::vector<UnionCase<D> > cases = union_cases<D>(d, false);
        return UnionType<D>(name, PrimitiveType<D>(), cases, type_annotations(d));
    }

    /// A union that switches on the enumeration `disc`: the labels are enumerator values.
    ::dds::core::xtypes::DynamicType union_on_enum(const Json& d, const std::string& name,
                                                   const ::dds::core::xtypes::DynamicType& disc) {
        using namespace ::dds::core::xtypes;
        const std::vector<UnionCase<uint32_t> > cases = union_cases<uint32_t>(d, true);
        EnumerationType<uint32_t> discriminator;
        static_cast<TDynamicType<detail::EnumerationType>&>(discriminator) =
            TDynamicType<detail::EnumerationType>(
                ::zerodds::cxx::static_pointer_cast<detail::EnumerationType>(disc.delegate()));
        return UnionType<uint32_t>(name, discriminator, cases, type_annotations(d));
    }

    ::dds::core::xtypes::DynamicType declare(const Json& d, const std::string& name) {
        using namespace ::dds::core::xtypes;
        const std::string kind = text(d, "kind");
        if (kind == "struct") {
            StructType parent;
            if (present(d, "base")) {
                parent = StructType(::zerodds::cxx::static_pointer_cast<detail::StructType>(
                    build(text(d, "base")).delegate()));
            }
            std::vector<MemberType> members;
            for (std::vector<Json>::const_iterator m_it = field(d, "members").items().begin(); m_it != field(d, "members").items().end(); ++m_it) {
            const Json& m = *m_it;
                members.push_back(MemberType(text(m, "name"), spec(field(m, "type")), member_annotations(m)));
            }
            return StructType(name, parent, members, type_annotations(d));
        }
        if (kind == "union") {
            const Json& disc = field(d, "disc");
            if (text(disc, "k") == "ref") {
                const ::dds::core::xtypes::DynamicType dt = build(text(disc, "n"));
                if (dt.kind() == TypeKind::ENUMERATION_TYPE) {
                    return union_on_enum(d, name, dt);
                }
                throw ::dds::core::UnsupportedError("TypeProvider: the union `" + name +
                                                    "` has a discriminator of type " + text(disc, "n") +
                                                    ", which is neither a primitive type nor an enumeration");
            }
            if (text(disc, "k") != "prim") {
                throw ::dds::core::UnsupportedError("TypeProvider: the union `" + name +
                                                    "` has a discriminator that is not a primitive type");
            }
            const std::string n = text(disc, "n");
            if (n == "boolean") return union_of<bool>(d, name);
            if (n == "char") return union_of<char>(d, name);
            if (n == "wchar") return union_of<wchar_t>(d, name);
            if (n == "octet") return union_of<uint8_t>(d, name);
            if (n == "short") return union_of<int16_t>(d, name);
            if (n == "unsigned short") return union_of<uint16_t>(d, name);
            if (n == "long") return union_of<int32_t>(d, name);
            if (n == "unsigned long") return union_of<uint32_t>(d, name);
            if (n == "long long") return union_of<int64_t>(d, name);
            if (n == "unsigned long long") return union_of<uint64_t>(d, name);
            throw ::dds::core::UnsupportedError("TypeProvider: the union `" + name +
                                                "` has a discriminator of type " + n);
        }
        if (kind == "enum") {
            std::vector<Enumerator> list;
            for (std::vector<Json>::const_iterator e_it = field(d, "enumerators").items().begin(); e_it != field(d, "enumerators").items().end(); ++e_it) {
            const Json& e = *e_it;
                list.push_back(present(e, "value")
                                   ? Enumerator(text(e, "name"),
                                                static_cast<uint32_t>(stoul_(text(e, "value"))))
                                   : Enumerator(text(e, "name")));
            }
            return EnumerationType<uint32_t>(name, list);
        }
        if (kind == "alias") {
            return AliasType(name, spec(field(d, "aliased")));
        }
        if (kind == "bitmask") {
            std::vector<BitFlag> flags;
            for (std::vector<Json>::const_iterator f_it = field(d, "flags").items().begin(); f_it != field(d, "flags").items().end(); ++f_it) {
            const Json& f = *f_it;
                flags.push_back(BitFlag(text(f, "name"), static_cast<uint32_t>(stoul_(text(f, "position")))));
            }
            return BitMaskType(name, static_cast<uint32_t>(stoul_(text(d, "bit_bound"))), flags);
        }
        if (kind == "bitset") {
            std::vector<BitField> fields;
            for (std::vector<Json>::const_iterator f_it = field(d, "fields").items().begin(); f_it != field(d, "fields").items().end(); ++f_it) {
            const Json& f = *f_it;
                fields.push_back(BitField(text(f, "name"), static_cast<uint32_t>(stoul_(text(f, "bits")))));
            }
            return BitSetType(name, fields);
        }
        throw ::dds::core::UnsupportedError(
            "TypeProvider: `" + name + "` is " + kind +
            (present(d, "what") ? " (" + text(d, "what") + ")" : std::string()) +
            ", which has no DynamicType class");
    }

    std::vector<std::string> order_;
    std::map<std::string, const Json*> declared_;
    std::map<std::string, ::dds::core::xtypes::DynamicType> built_;
    std::set<std::string> building_;
};

}  // namespace xtypes_detail
}  // namespace psm
}  // namespace zerodds

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

class TypeProvider {
public:
    /// The description the C API gives of the types at `uri`.
    static Json describe(const std::string& uri) {
        char* text = 0;
        ::zerodds::psm::check_status(zerodds_types_load(uri.c_str(), &text),
                                     ("TypeProvider: the types at `" + uri + "` cannot be loaded").c_str());
        Json doc = Json::parse(text);
        zerodds_string_free(text);
        return doc;
    }
};

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_TYPEPROVIDER_HPP
