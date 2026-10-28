// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/UnionType.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): `UnionType<T>`, a union whose
// discriminator is of the primitive type `T`.

#ifndef ZERODDS_DDS_CORE_XTYPES_UNIONTYPE_HPP
#define ZERODDS_DDS_CORE_XTYPES_UNIONTYPE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <memory>
#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/xtypes/DynamicType.hpp"
#include "dds/core/xtypes/EnumTypes.hpp"
#include "dds/core/xtypes/PrimitiveTypes.hpp"
#include "dds/core/xtypes/UnionCase.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

/// The part of a union delegate that does not depend on the discriminator type, for the code
/// that writes the type as IDL.
class UnionTypeBase : public DynamicType {
public:
    UnionTypeBase(const std::string& name, TypeKind kind,
                  const std::vector< ::dds::core::xtypes::Annotation>& annotations =
                      std::vector< ::dds::core::xtypes::Annotation>())
        : DynamicType(name, kind, annotations) {}
    virtual const ::dds::core::xtypes::DynamicType& discriminator_type() const = 0;
    /// A case: the label (unused for the default branch) and its member.
    struct CaseInfo {
        long long label;
        bool is_default;
        ::dds::core::xtypes::MemberType member;
        CaseInfo() : label(0), is_default(false), member() {}
        CaseInfo(long long l, bool d, const ::dds::core::xtypes::MemberType& m)
            : label(l), is_default(d), member(m) {}
    };
    virtual std::vector<CaseInfo> case_infos() const = 0;
};

template <typename T>
class UnionType : public UnionTypeBase {
public:
    UnionType(const std::string& name, const ::dds::core::xtypes::DynamicType& discriminator,
              std::vector< ::dds::core::xtypes::UnionCase<T> > cases,
              std::vector< ::dds::core::xtypes::Annotation> annotations =
                  std::vector< ::dds::core::xtypes::Annotation>())
        : UnionTypeBase(name, TypeKind::UNION_TYPE, ZERODDS_MOVE(annotations)),
          discriminator_(discriminator),
          cases_(ZERODDS_MOVE(cases)) {}

    const ::dds::core::xtypes::DynamicType& discriminator_type() const ZERODDS_OVERRIDE {
        return discriminator_;
    }
    const std::vector< ::dds::core::xtypes::UnionCase<T> >& union_cases() const { return cases_; }
    std::vector<CaseInfo> case_infos() const ZERODDS_OVERRIDE {
        std::vector<CaseInfo> out;
        for (typename std::vector< ::dds::core::xtypes::UnionCase<T> >::const_iterator it =
                 cases_.begin();
             it != cases_.end(); ++it) {
            out.push_back(CaseInfo(it->is_default() ? 0 : static_cast<long long>(it->discriminator()),
                                   it->is_default(), it->member()));
        }
        return out;
    }

    bool equals(const DynamicType& other) const ZERODDS_OVERRIDE {
        const UnionType<T>* p = dynamic_cast<const UnionType<T>*>(&other);
        if (!DynamicType::equals(other) || p == 0 || p->cases_.size() != cases_.size() ||
            !(discriminator_ == p->discriminator_)) {
            return false;
        }
        for (std::size_t i = 0; i < cases_.size(); ++i) {
            if (cases_[i].is_default() != p->cases_[i].is_default() ||
                cases_[i].discriminator() != p->cases_[i].discriminator() ||
                !(cases_[i].member() == p->cases_[i].member())) {
                return false;
            }
        }
        return true;
    }

private:
    ::dds::core::xtypes::DynamicType discriminator_;
    std::vector< ::dds::core::xtypes::UnionCase<T> > cases_;
};

}  // namespace detail

/// A union type, over the discriminator type `T`. A reference type; the operations that add or
/// remove answer a new union type.
template <typename T, template <typename Q> class DELEGATE = detail::UnionType>
class UnionType : public TDynamicType<DELEGATE<T> > {
public:
    typedef DELEGATE<T> DELEGATE_T;

    /// A null union type.
    UnionType() ZERODDS_NOEXCEPT {}

    /// A union type sharing `ref`.
    explicit UnionType(const typename TDynamicType<DELEGATE<T> >::DELEGATE_REF_T& ref)
        : TDynamicType<DELEGATE<T> >(ref) {}

    UnionType(const std::string& name, const TPrimitiveType<T>& discriminator_type,
              const std::vector<UnionCase<T> >& cases)
        : TDynamicType<DELEGATE<T> >(
              ::zerodds::cxx::make_shared<DELEGATE<T> >(name, discriminator_type, cases)) {}
    UnionType(const std::string& name, const TPrimitiveType<T>& discriminator_type,
              const std::vector<UnionCase<T> >& cases, const Annotation& annotation)
        : TDynamicType<DELEGATE<T> >(::zerodds::cxx::make_shared<DELEGATE<T> >(
              name, discriminator_type, cases, detail::single_annotation(annotation))) {}
    UnionType(const std::string& name, const TPrimitiveType<T>& discriminator_type,
              const std::vector<UnionCase<T> >& cases, const std::vector<Annotation>& annotations)
        : TDynamicType<DELEGATE<T> >(
              ::zerodds::cxx::make_shared<DELEGATE<T> >(name, discriminator_type, cases, annotations)) {}

    /// A union that switches on an enumeration: the labels of the cases are the values of its
    /// enumerators (those the application assigned, else their ordinals).
    template <typename UTYPE>
    UnionType(const std::string& name, const EnumerationType<UTYPE>& discriminator_type,
              const std::vector<UnionCase<T> >& cases)
        : TDynamicType<DELEGATE<T> >(
              ::zerodds::cxx::make_shared<DELEGATE<T> >(name, discriminator_type, cases)) {}
    template <typename UTYPE>
    UnionType(const std::string& name, const EnumerationType<UTYPE>& discriminator_type,
              const std::vector<UnionCase<T> >& cases, const Annotation& annotation)
        : TDynamicType<DELEGATE<T> >(::zerodds::cxx::make_shared<DELEGATE<T> >(
              name, discriminator_type, cases, detail::single_annotation(annotation))) {}
    template <typename UTYPE>
    UnionType(const std::string& name, const EnumerationType<UTYPE>& discriminator_type,
              const std::vector<UnionCase<T> >& cases, const std::vector<Annotation>& annotations)
        : TDynamicType<DELEGATE<T> >(
              ::zerodds::cxx::make_shared<DELEGATE<T> >(name, discriminator_type, cases, annotations)) {}

    /// The type of the discriminator.
    DynamicType discriminator_type() const { return this->delegate()->discriminator_type(); }

    const std::vector<UnionCase<T> >& members() const { return this->delegate()->union_cases(); }

    const MemberType& member(uint32_t id) const {
        if (id < members().size()) {
            return members()[id].member();
        }
        throw ::dds::core::InvalidArgumentError("UnionType::member: no member with this id");
    }
    const MemberType& member(const std::string& name) const {
        const std::vector<UnionCase<T> >& list = members();
        for (typename std::vector<UnionCase<T> >::const_iterator it = list.begin(); it != list.end();
             ++it) {
            if (it->member().name() == name) {
                return it->member();
            }
        }
        throw ::dds::core::InvalidArgumentError("UnionType::member: no member of this name: " + name);
    }

    UnionType add_member(const UnionCase<T>& member) const {
        std::vector<UnionCase<T> > next = members();
        next.push_back(member);
        return UnionType(this->name(), discriminator_type(), next, this->annotations(), Rebuild());
    }
    UnionType remove_member(const UnionCase<T>& member) const {
        std::vector<UnionCase<T> > next;
        const std::vector<UnionCase<T> >& list = members();
        for (typename std::vector<UnionCase<T> >::const_iterator it = list.begin(); it != list.end();
             ++it) {
            if (!(it->is_default() == member.is_default() &&
                  it->discriminator() == member.discriminator() && it->member() == member.member())) {
                next.push_back(*it);
            }
        }
        return UnionType(this->name(), discriminator_type(), next, this->annotations(), Rebuild());
    }
    UnionType add_annotation(const Annotation& annotation) const {
        std::vector<Annotation> next = this->annotations();
        next.push_back(annotation);
        return UnionType(this->name(), discriminator_type(), members(), next, Rebuild());
    }
    UnionType remove_annotation(const Annotation& annotation) const {
        std::vector<Annotation> next;
        const std::vector<Annotation>& own = this->annotations();
        for (std::vector<Annotation>::const_iterator it = own.begin(); it != own.end(); ++it) {
            if (!(*it)->equals(*annotation.delegate())) {
                next.push_back(*it);
            }
        }
        return UnionType(this->name(), discriminator_type(), members(), next, Rebuild());
    }

private:
    struct Rebuild {};
    UnionType(const std::string& name, const DynamicType& discriminator_type,
              const std::vector<UnionCase<T> >& cases, const std::vector<Annotation>& annotations,
              Rebuild)
        : TDynamicType<DELEGATE<T> >(
              ::zerodds::cxx::make_shared<DELEGATE<T> >(name, discriminator_type, cases, annotations)) {}
};

/// A union type that is declared and not yet defined.
template <typename DELEGATE>
class TUnionForwardDeclaration : public TDynamicType<DELEGATE> {
public:
    explicit TUnionForwardDeclaration(const std::string& name)
        : TDynamicType<DELEGATE>(name, TypeKind::UNION_FWD_DECL_TYPE) {}
};

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_UNIONTYPE_HPP
