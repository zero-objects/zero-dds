// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/Idl.hpp — a DynamicType written as IDL, which the C API lowers to the
// model every other column uses.

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_IDL_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_IDL_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <map>
#include <memory>
#include <set>
#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/xtypes/CollectionTypes.hpp"
#include "dds/core/xtypes/EnumTypes.hpp"
#include "dds/core/xtypes/PrimitiveTypes.hpp"
#include "dds/core/xtypes/StructType.hpp"
#include "dds/core/xtypes/UnionType.hpp"
#include "dds/core/xtypes/detail/DynTypeHandle.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

class IdlWriter {
public:
    /// The IDL for `root` and every structure and union it names.
    std::string write(const ::dds::core::xtypes::DynamicType& root) {
        visit(root);
        return out_;
    }

    /// `a::b::T` as `::a::b::T`.
    static std::string scoped(const std::string& name) {
        return name.rfind("::", 0) == 0 ? name : "::" + name;
    }

private:
    static std::vector<std::string> split(const std::string& name) {
        std::vector<std::string> parts;
        std::size_t at = 0;
        while (at <= name.size()) {
            const std::size_t next = name.find("::", at);
            const std::string part = name.substr(at, next == std::string::npos ? next : next - at);
            if (!part.empty()) {
                parts.push_back(part);
            }
            if (next == std::string::npos) {
                break;
            }
            at = next + 2;
        }
        return parts;
    }

    static void identifier(const std::string& name) {
        if (name.empty()) {
            throw ::dds::core::InvalidArgumentError("DynamicType: a name is empty");
        }
        for (std::string::const_iterator it = name.begin(); it != name.end(); ++it) {
            const char c = *it;
            const bool ok = (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
                            (c >= '0' && c <= '9') || c == '_';
            if (!ok) {
                throw ::dds::core::InvalidArgumentError("DynamicType: `" + name +
                                                        "` is not an IDL identifier");
            }
        }
    }

    std::string annotations_of(const std::vector< ::dds::core::xtypes::Annotation>& list,
                               bool member) const {
        std::string out;
        for (std::vector< ::dds::core::xtypes::Annotation>::const_iterator ai = list.begin();
             ai != list.end(); ++ai) {
            const ::dds::core::xtypes::Annotation& a = *ai;
            switch (a.kind().underlying()) {
            case AnnotationKind::KEY_ANNOTATION_TYPE: out += "@key "; break;
            case AnnotationKind::OPTIONAL_ANNOTATION_TYPE: out += "@optional "; break;
            case AnnotationKind::SHARED_ANNOTATION_TYPE: out += "@external "; break;
            case AnnotationKind::MUST_UNDERSTAND_ANNOTATION_TYPE: out += "@must_understand "; break;
            case AnnotationKind::NESTED_ANNOTATION_TYPE:
                if (!member) {
                    out += "@nested ";
                }
                break;
            case AnnotationKind::ID_ANNOTATION_TYPE:
                out += "@id(" +
                       ::zerodds::cxx::to_string(::zerodds::cxx::static_pointer_cast<detail::IdAnnotation>(a.delegate())->id()) +
                       ") ";
                break;
            case AnnotationKind::EXTENSIBILITY_ANNOTATION_TYPE: {
                if (!member) {
                    const ExtensibilityKind kind =
                        ::zerodds::cxx::static_pointer_cast<detail::ExtensibilityAnnotation>(a.delegate())
                            ->extensibility_kind();
                    out += kind == ExtensibilityKind::FINAL
                               ? "@final "
                               : (kind == ExtensibilityKind::MUTABLE ? "@mutable " : "@appendable ");
                }
                break;
            }
            // `@verbatim` text is for the generated source, not for the type; bit bounds and
            // `@bit_set` belong to enumerations and bitsets, which the PSM has no class for.
            default: break;
            }
        }
        return out;
    }

    /// `type name` as a declaration: an array is written with its extents behind the name
    /// (`long name[2][3];`), which is the only place IDL takes an array.
    std::string declare(const ::dds::core::xtypes::DynamicType& t, const std::string& name) {
        if (!t.is_nil() && t.kind() == TypeKind::ARRAY_TYPE) {
            std::string extents;
            ::dds::core::xtypes::DynamicType element = t;
            while (element.kind() == TypeKind::ARRAY_TYPE) {
                const ::zerodds::cxx::shared_ptr<detail::ArrayType> a =
                ::zerodds::cxx::static_pointer_cast<detail::ArrayType>(element.delegate());
                const std::vector<uint32_t>& dims = a->dimensions();
                for (std::vector<uint32_t>::const_iterator di = dims.begin(); di != dims.end();
                     ++di) {
                    extents += "[" + ::zerodds::cxx::to_string(*di) + "]";
                }
                element = a->element_type();
            }
            return spell(element) + " " + name + extents;
        }
        return spell(t) + " " + name;
    }

    std::string spell(const ::dds::core::xtypes::DynamicType& t) {
        if (t.is_nil()) {
            throw ::dds::core::InvalidArgumentError("DynamicType: a null type");
        }
        const int kind = static_cast<int>(t.kind().underlying());
        if ((kind & static_cast<int>(TypeKind::PRIMITIVE_TYPE)) != 0) {
            return t.name();
        }
        switch (t.kind().underlying()) {
        case TypeKind::STRING_TYPE: {
            const ::zerodds::cxx::shared_ptr<detail::CollectionType> c =
                ::zerodds::cxx::static_pointer_cast<detail::CollectionType>(t.delegate());
            std::string s = t.name();
            if (c->bounds() != kUnbounded) {
                s += "<" + ::zerodds::cxx::to_string(c->bounds()) + ">";
            }
            return s;
        }
        case TypeKind::SEQUENCE_TYPE: {
            const ::zerodds::cxx::shared_ptr<detail::SequenceType> s =
                ::zerodds::cxx::static_pointer_cast<detail::SequenceType>(t.delegate());
            std::string text = "sequence<" + spell(s->element_type());
            if (s->bounds() != kUnbounded) {
                text += ", " + ::zerodds::cxx::to_string(s->bounds());
            }
            return text + ">";
        }
        case TypeKind::MAP_TYPE: {
            const ::zerodds::cxx::shared_ptr<detail::MapType> m =
                ::zerodds::cxx::static_pointer_cast<detail::MapType>(t.delegate());
            std::string text = "map<" + spell(m->key_type()) + ", " + spell(m->value_type());
            if (m->bounds() != kUnbounded) {
                text += ", " + ::zerodds::cxx::to_string(m->bounds());
            }
            return text + ">";
        }
        case TypeKind::ARRAY_TYPE:
            throw ::dds::core::UnsupportedError(
                "DynamicType: an array is a member, a union case or an alias; the element of a "
                "sequence or a map takes an AliasType of the array");
        case TypeKind::STRUCTURE_TYPE:
        case TypeKind::UNION_TYPE:
        case TypeKind::ENUMERATION_TYPE:
        case TypeKind::ALIAS_TYPE:
        case TypeKind::BITSET_TYPE:
        case TypeKind::BITMASK_TYPE:
            visit(t);
            return scoped(t.name());
        default:
            throw ::dds::core::UnsupportedError(
                "DynamicType: this kind of type has no member form here: " + t.name());
        }
    }

    static std::string label(const ::dds::core::xtypes::DynamicType& disc, long long value) {
        switch (disc.kind().underlying()) {
        case TypeKind::BOOLEAN_TYPE: return value != 0 ? "TRUE" : "FALSE";
        case TypeKind::CHAR_8_TYPE:
        case TypeKind::CHAR_32_TYPE: {
            // A character label is a literal: a printable character as itself, anything else
            // as its code.
            std::string s = disc.kind() == TypeKind::CHAR_32_TYPE ? "L'" : "'";
            if (value >= 0x20 && value < 0x7F && value != '\'' && value != '\\') {
                s.push_back(static_cast<char>(value));
            } else {
                static const char* hex = "0123456789ABCDEF";
                s += "\\x";
                s.push_back(hex[(value >> 4) & 0xF]);
                s.push_back(hex[value & 0xF]);
            }
            return s + "'";
        }
        case TypeKind::ENUMERATION_TYPE: {
            // The label names the enumerator whose value it is (the value the application
            // assigned, else the ordinal after the one before).
            const ::zerodds::cxx::shared_ptr<detail::EnumerationType> e =
                ::zerodds::cxx::static_pointer_cast<detail::EnumerationType>(disc.delegate());
            long long next = 0;
            const std::vector<EnumeratorEntry>& enumerators = e->enumerators();
            for (std::vector<EnumeratorEntry>::const_iterator ci = enumerators.begin();
                 ci != enumerators.end(); ++ci) {
                const EnumeratorEntry& c = *ci;
                const long long v = c.has_value ? static_cast<long long>(c.value) : next;
                next = v + 1;
                if (v == value) {
                    const std::string full = disc.name();
                    const std::size_t cut = full.rfind("::");
                    return cut == std::string::npos ? c.name : scoped(full.substr(0, cut) + "::" + c.name);
                }
            }
            throw ::dds::core::InvalidArgumentError("DynamicType: the label " + ::zerodds::cxx::to_string(value) +
                                                    " is no enumerator of " + disc.name());
        }
        default: return ::zerodds::cxx::to_string(value);
        }
    }

    void open_modules(const std::vector<std::string>& modules) {
        for (std::vector<std::string>::const_iterator it = modules.begin(); it != modules.end(); ++it) {
            out_ += "module " + *it + " { ";
        }
    }
    void close_modules(const std::vector<std::string>& modules) {
        for (std::size_t i = 0; i < modules.size(); ++i) {
            out_ += "}; ";
        }
    }

    void visit(const ::dds::core::xtypes::DynamicType& t) {
        const std::string key = t.name();
        if (done_.count(key) != 0) {
            return;
        }
        if (visiting_.count(key) != 0) {
            throw ::dds::core::UnsupportedError("DynamicType: " + key + " contains itself");
        }
        visiting_.insert(key);
        std::vector<std::string> parts = split(t.name());
        if (parts.empty()) {
            throw ::dds::core::InvalidArgumentError("DynamicType: a type has no name");
        }
        for (std::vector<std::string>::const_iterator it = parts.begin(); it != parts.end(); ++it) {
            identifier(*it);
        }
        const std::string simple = parts.back();
        parts.pop_back();

        std::string body;
        if (t.kind() == TypeKind::STRUCTURE_TYPE) {
            const ::zerodds::cxx::shared_ptr<detail::StructType> s =
                ::zerodds::cxx::static_pointer_cast<detail::StructType>(t.delegate());
            std::string base;
            if (s->parent() != 0) {
                ::dds::core::xtypes::StructType parent(s->parent());
                visit(parent);
                base = " : " + scoped(parent.name());
            }
            std::string members;
            const std::vector< ::dds::core::xtypes::MemberType>& own = s->members();
            for (std::vector< ::dds::core::xtypes::MemberType>::const_iterator mi = own.begin();
                 mi != own.end(); ++mi) {
                const ::dds::core::xtypes::MemberType& m = *mi;
                identifier(m.name());
                members += "    " + annotations_of(m.annotations(), true) + declare(m.type(), m.name()) +
                           ";\n";
            }
            body = annotations_of(t.annotations(), false) + "struct " + simple + base + " {\n" +
                   members + "}";
        } else if (t.kind() == TypeKind::UNION_TYPE) {
            const ::zerodds::cxx::shared_ptr<detail::UnionTypeBase> u =
                ::zerodds::cxx::dynamic_pointer_cast<detail::UnionTypeBase>(t.delegate());
            if (!u) {
                throw ::dds::core::UnsupportedError("DynamicType: a union of an unknown form");
            }
            std::string cases;
            // The labels of one member stand together; a member's labels follow each other in
            // `case_infos()`, as the application listed them, and `default:` is a label of its own.
            const std::vector<UnionTypeBase::CaseInfo> infos = u->case_infos();
            for (std::size_t i = 0; i < infos.size(); ++i) {
                const UnionTypeBase::CaseInfo& c = infos[i];
                identifier(c.member.name());
                std::string labels = c.is_default ? "    default: "
                                                  : "    case " + label(u->discriminator_type(), c.label) + ": ";
                // Labels of the same member (equal by name) join into one declaration.
                if (i + 1 < infos.size() && infos[i + 1].member.name() == c.member.name()) {
                    cases += labels + "\n";
                    continue;
                }
                cases += labels + annotations_of(c.member.annotations(), true) +
                         declare(c.member.type(), c.member.name()) + ";\n";
            }
            body = annotations_of(t.annotations(), false) + "union " + simple + " switch (" +
                   spell(u->discriminator_type()) + ") {\n" + cases + "}";
        } else if (t.kind() == TypeKind::ENUMERATION_TYPE) {
            const ::zerodds::cxx::shared_ptr<detail::EnumerationType> e =
                ::zerodds::cxx::static_pointer_cast<detail::EnumerationType>(t.delegate());
            std::string list;
            const std::vector<EnumeratorEntry>& enumerators = e->enumerators();
            for (std::vector<EnumeratorEntry>::const_iterator ci = enumerators.begin();
                 ci != enumerators.end(); ++ci) {
                const EnumeratorEntry& c = *ci;
                identifier(c.name);
                if (!list.empty()) {
                    list += ", ";
                }
                list += (c.has_value ? "@value(" + ::zerodds::cxx::to_string(c.value) + ") " : std::string()) + c.name;
            }
            body = annotations_of(t.annotations(), false) + "enum " + simple + " { " + list + " }";
        } else if (t.kind() == TypeKind::ALIAS_TYPE) {
            const ::zerodds::cxx::shared_ptr<detail::AliasType> a =
                ::zerodds::cxx::static_pointer_cast<detail::AliasType>(t.delegate());
            body = "typedef " + declare(a->aliased(), simple);
        } else if (t.kind() == TypeKind::BITMASK_TYPE) {
            const ::zerodds::cxx::shared_ptr<detail::BitMaskType> b =
                ::zerodds::cxx::static_pointer_cast<detail::BitMaskType>(t.delegate());
            std::string flags;
            const std::vector<BitFlagEntry>& flag_list = b->flags();
            for (std::vector<BitFlagEntry>::const_iterator fi = flag_list.begin();
                 fi != flag_list.end(); ++fi) {
                const BitFlagEntry& f = *fi;
                identifier(f.name);
                if (!flags.empty()) {
                    flags += ", ";
                }
                flags += "@position(" + ::zerodds::cxx::to_string(f.position) + ") " + f.name;
            }
            body = "@bit_bound(" + ::zerodds::cxx::to_string(b->bit_bound()) + ") bitmask " + simple + " { " +
                   flags + " }";
        } else if (t.kind() == TypeKind::BITSET_TYPE) {
            const ::zerodds::cxx::shared_ptr<detail::BitSetType> b =
                ::zerodds::cxx::static_pointer_cast<detail::BitSetType>(t.delegate());
            std::string fields;
            const std::vector<BitfieldEntry>& field_list = b->fields();
            for (std::vector<BitfieldEntry>::const_iterator fi = field_list.begin();
                 fi != field_list.end(); ++fi) {
                const BitfieldEntry& f = *fi;
                identifier(f.name);
                fields += "    bitfield<" + ::zerodds::cxx::to_string(f.bits) + "> " + f.name + ";\n";
            }
            body = "bitset " + simple + " {\n" + fields + "}";
        } else {
            throw ::dds::core::UnsupportedError("DynamicType: " + key +
                                                " is not a structure, union, enumeration, alias, bit set or bit mask");
        }
        open_modules(parts);
        out_ += body + "; ";
        close_modules(parts);
        out_ += "\n";
        visiting_.erase(key);
        done_.insert(key);
    }

    std::string out_;
    std::set<std::string> done_;
    std::set<std::string> visiting_;
};

/// The handle for `type`, shared by everything that was made from the same type.
inline ::zerodds::cxx::shared_ptr<DynTypeHandle> handle_of(const ::dds::core::xtypes::DynamicType& type) {
    static ::zerodds::cxx::mutex mutex;
    static std::map<std::string, ::zerodds::cxx::weak_ptr<DynTypeHandle> > cache;
    IdlWriter writer;
    const std::string idl = writer.write(type);
    const std::string key = IdlWriter::scoped(type.name()) + "\n" + idl;
    ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex);
    typedef std::map<std::string, ::zerodds::cxx::weak_ptr<DynTypeHandle> > Cache;
    for (Cache::iterator it = cache.begin(); it != cache.end();) {
        if (it->second.expired()) {
            cache.erase(it++);
        } else {
            ++it;
        }
    }
    Cache::iterator found = cache.find(key);
    if (found != cache.end()) {
        ::zerodds::cxx::shared_ptr<DynTypeHandle> live = found->second.lock();
        if (live) {
            return live;
        }
    }
    ::zerodds::cxx::shared_ptr<DynTypeHandle> handle = ::zerodds::cxx::make_shared<DynTypeHandle>(idl, type.name());
    cache[key] = handle;
    return handle;
}

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_IDL_HPP
