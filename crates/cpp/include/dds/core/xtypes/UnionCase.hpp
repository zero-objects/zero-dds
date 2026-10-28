// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/UnionCase.hpp — DDS-PSM-Cxx 1.0 §7.5 (xtypes): a case of a union.

#ifndef ZERODDS_DDS_CORE_XTYPES_UNIONCASE_HPP
#define ZERODDS_DDS_CORE_XTYPES_UNIONCASE_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>

#include "dds/core/Reference.hpp"
#include "dds/core/xtypes/MemberType.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

template <typename T>
class UnionCase {
public:
    UnionCase(T discriminator, const ::dds::core::xtypes::MemberType& member)
        : discriminator_(discriminator), member_(member), is_default_(false) {}
    /// The default branch: it has no label.
    explicit UnionCase(const ::dds::core::xtypes::MemberType& member)
        : discriminator_(), member_(member), is_default_(true) {}
    T discriminator() const { return discriminator_; }
    bool is_default() const { return is_default_; }
    const ::dds::core::xtypes::MemberType& member() const { return member_; }

private:
    T discriminator_;
    ::dds::core::xtypes::MemberType member_;
    bool is_default_;
};

}  // namespace detail

/// A case of a union: the value of the discriminator that selects it, and its member.
template <typename T, template <typename Q> class DELEGATE = detail::UnionCase>
class UnionCase : public ::dds::core::Reference<DELEGATE<T> > {
public:
    typedef DELEGATE<T> DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE<T> > BASE_T;

    /// A null case.
    UnionCase() ZERODDS_NOEXCEPT {}
    UnionCase(T discriminator, const MemberType& member)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE<T> >(discriminator, member)) {}
    /// The default branch of a union (IDL `default:`): it has no label, `is_default()` is true.
    explicit UnionCase(const MemberType& member)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE<T> >(member)) {}

    T discriminator() const { return this->delegate()->discriminator(); }
    bool is_default() const { return this->delegate()->is_default(); }
    const MemberType& member() const { return this->delegate()->member(); }
};

}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_UNIONCASE_HPP
