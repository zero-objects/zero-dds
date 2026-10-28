// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/TStructType.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes) `TStructType<DELEGATE>`.

#ifndef ZERODDS_DDS_CORE_XTYPES_TSTRUCTTYPE_HPP
#define ZERODDS_DDS_CORE_XTYPES_TSTRUCTTYPE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <algorithm>
#include <memory>
#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/xtypes/DynamicType.hpp"
#include "dds/core/xtypes/MemberType.hpp"
#include "dds/core/xtypes/detail/StructType.hpp"

namespace dds {
namespace core {
namespace xtypes {

/// A structure type, over any delegate. A reference type: the operations that add or remove
/// a member or an annotation answer a new structure type and leave this one as it is.
template <typename DELEGATE>
class TStructType : public TDynamicType<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef typename TDynamicType<DELEGATE>::DELEGATE_REF_T DELEGATE_REF_T;

    /// A null structure type.
    TStructType() ZERODDS_NOEXCEPT {}
    explicit TStructType(const DELEGATE_REF_T& ref) : TDynamicType<DELEGATE>(ref) {}

    explicit TStructType(const std::string& name)
        : TDynamicType<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(name)) {}

    TStructType(const std::string& name, const TStructType& parent,
                const std::vector<MemberType>& members)
        : TDynamicType<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(name, parent.unchecked_delegate(), members)) {}

    template <typename MemberIter>
    TStructType(const std::string& name, const TStructType& parent, const MemberIter& begin,
                const MemberIter& end)
        : TDynamicType<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(
              name, parent.unchecked_delegate(), std::vector<MemberType>(begin, end))) {}

    TStructType(const std::string& name, const TStructType& parent,
                const std::vector<MemberType>& members, const Annotation& annotation)
        : TDynamicType<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(name, parent.unchecked_delegate(), members,
                                                           detail::single_annotation(annotation))) {}

    TStructType(const std::string& name, const TStructType& parent,
                const std::vector<MemberType>& members, const std::vector<Annotation>& annotations)
        : TDynamicType<DELEGATE>(
              ::zerodds::cxx::make_shared<DELEGATE>(name, parent.unchecked_delegate(), members, annotations)) {}

    template <typename AnnotationIter, typename MemberIter>
    TStructType(const std::string& name, const TStructType& parent, const MemberIter& begin,
                const MemberIter& end, const AnnotationIter& abegin, const AnnotationIter& aend)
        : TDynamicType<DELEGATE>(::zerodds::cxx::make_shared<DELEGATE>(
              name, parent.unchecked_delegate(), std::vector<MemberType>(begin, end),
              std::vector<Annotation>(abegin, aend))) {}

    /// The structure this one extends; null when it extends none.
    TStructType parent() const { return TStructType(this->delegate()->parent()); }

    /// The members this structure declares, not the parent's.
    const std::vector<MemberType>& members() const { return this->delegate()->members(); }

    /// The member with the id `id`; the ids count the parent's members first unless `@id`
    /// says otherwise.
    const MemberType& member(uint32_t id) const {
        const std::vector<MemberType> all = all_members();
        for (uint32_t i = 0; i < all.size(); ++i) {
            if (member_id_of(all[i], i) == id) {
                return find_own(all[i]);
            }
        }
        throw ::dds::core::InvalidArgumentError("StructType::member: no member with this id");
    }

    const MemberType& member(const std::string& name) const {
        const std::vector<MemberType>& own_members = members();
        for (typename std::vector<MemberType>::const_iterator it = own_members.begin();
             it != own_members.end(); ++it) {
            if (it->name() == name) {
                return *it;
            }
        }
        const TStructType p = parent();
        if (!p.is_nil()) {
            return p.member(name);
        }
        throw ::dds::core::InvalidArgumentError("StructType::member: no member of this name: " + name);
    }

    /// The members of the parents first, then this structure's own.
    std::vector<MemberType> all_members() const {
        std::vector<MemberType> out;
        const TStructType p = parent();
        if (!p.is_nil()) {
            out = p.all_members();
        }
        out.insert(out.end(), members().begin(), members().end());
        return out;
    }

    /// The id of the member `index` of `all_members()`: its `@id`, otherwise its position.
    static uint32_t member_id_of(const MemberType& m, uint32_t position) {
        return hasId(m) ? static_cast<uint32_t>(getId(m)) : position;
    }

    TStructType add_member(const MemberType& member) const {
        std::vector<MemberType> next = members();
        next.push_back(member);
        return TStructType(this->name(), parent(), next, this->annotations());
    }
    TStructType remove_member(const MemberType& member) const {
        std::vector<MemberType> next;
        const std::vector<MemberType>& own_members = members();
        for (typename std::vector<MemberType>::const_iterator it = own_members.begin();
             it != own_members.end(); ++it) {
            if (!(*it == member)) {
                next.push_back(*it);
            }
        }
        return TStructType(this->name(), parent(), next, this->annotations());
    }
    TStructType add_annotation(const Annotation& annotation) const {
        std::vector<Annotation> next = this->annotations();
        next.push_back(annotation);
        return TStructType(this->name(), parent(), members(), next);
    }
    TStructType remove_annotation(const Annotation& annotation) const {
        std::vector<Annotation> next;
        const std::vector<Annotation>& own = this->annotations();
        for (std::vector<Annotation>::const_iterator it = own.begin(); it != own.end(); ++it) {
            if (!(*it)->equals(*annotation.delegate())) {
                next.push_back(*it);
            }
        }
        return TStructType(this->name(), parent(), members(), next);
    }

private:
    /// The stored member equal to `m`, which may be the parent's.
    const MemberType& find_own(const MemberType& m) const {
        const std::vector<MemberType>& own_members = members();
        for (typename std::vector<MemberType>::const_iterator it = own_members.begin();
             it != own_members.end(); ++it) {
            if (*it == m) {
                return *it;
            }
        }
        const TStructType p = parent();
        return p.find_own(m);
    }
};

/// A structure type that is declared and not yet defined.
template <typename DELEGATE>
class TStructForwardDeclaration : public TDynamicType<DELEGATE> {
public:
    explicit TStructForwardDeclaration(const std::string& name)
        : TDynamicType<DELEGATE>(name, TypeKind::STRUCTURE_FWD_DECL_TYPE) {}
};

template <typename T>
bool isFinal(const TStructType<T>& s) {
    return detail::extensibility_of(s.annotations()) == ExtensibilityKind::FINAL;
}
template <typename T>
bool isExtensible(const TStructType<T>& s) {
    return detail::extensibility_of(s.annotations()) == ExtensibilityKind::EXTENSIBLE;
}
template <typename T>
bool isMutable(const TStructType<T>& s) {
    return detail::extensibility_of(s.annotations()) == ExtensibilityKind::MUTABLE;
}
template <typename T>
bool isNested(const TStructType<T>& s) {
    return detail::has_annotation(s.annotations(), AnnotationKind::NESTED_ANNOTATION_TYPE);
}

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_TSTRUCTTYPE_HPP
