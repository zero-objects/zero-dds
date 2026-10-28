// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/refmacros.hpp — DDS-PSM-Cxx 1.0 §7.2: the macros that declare a reference type.

#ifndef ZERODDS_DDS_CORE_REFMACROS_HPP
#define ZERODDS_DDS_CORE_REFMACROS_HPP

#include "dds/core/Reference.hpp"
#include "dds/core/macros.hpp"
#include "dds/core/ref_traits.hpp"

// The typedefs a reference type with delegate type DELEGATE carries.
#define DECLARE_TYPE_TRAITS(TYPE) typedef TYPE TYPE##_T;

// Typedefs of the delegate and its reference.
#define OMG_DDS_REF_TYPE_DELEGATE(TYPE, BASE, DELEGATE)                                  \
public:                                                                                  \
    typedef DELEGATE DELEGATE_T;                                                         \
    typedef typename ::dds::core::smart_ptr_traits<DELEGATE>::ref_type DELEGATE_REF_T;   \
    typedef typename ::dds::core::smart_ptr_traits<DELEGATE>::weak_ref_type              \
        DELEGATE_WEAK_REF_T;

// A reference type without a default constructor: construction from a reference to
// the delegate and from another handle on a delegate of the same family, and the
// assignment between such handles.
#define OMG_DDS_REF_TYPE_NODC(TYPE, BASE, DELEGATE)                                      \
    OMG_DDS_REF_TYPE_DELEGATE(TYPE, BASE, DELEGATE)                                      \
public:                                                                                  \
    typedef BASE<DELEGATE> BASE_T;                                                                                  \
    explicit TYPE(const DELEGATE_REF_T& ref) : BASE<DELEGATE_T>(ref) {}                  \
    template <typename H__>                                                              \
    TYPE(const H__& h) : BASE<DELEGATE_T>(h) {                                           \
        OMG_DDS_STATIC_ASSERT(                                                           \
            (::dds::core::is_base_of<DELEGATE_T, typename H__::DELEGATE_T>::value));     \
    }                                                                                    \
    template <typename T__>                                                              \
    TYPE& operator=(const T__& rhs) {                                                    \
        OMG_DDS_STATIC_ASSERT(                                                           \
            (::dds::core::is_base_of<DELEGATE_T, typename T__::DELEGATE_T>::value));     \
        if (this != reinterpret_cast<const TYPE*>(&rhs)) {                               \
            *this = TYPE(rhs);                                                           \
        }                                                                                \
        return *this;                                                                    \
    }

// The same plus the null handle.
#define OMG_DDS_REF_TYPE(TYPE, BASE, DELEGATE)                                           \
    OMG_DDS_REF_TYPE_NODC(TYPE, BASE, DELEGATE)                                          \
public:                                                                                  \
    TYPE(const ::dds::core::null_type&) : BASE<DELEGATE>() {}

// A reference type whose base takes the sample type `T_PARAM` and the delegate template.
#define OMG_DDS_REF_TYPE_NODC_T(TYPE, BASE, T_PARAM, DELEGATE)                           \
public:                                                                                  \
    typedef DELEGATE<T_PARAM> DELEGATE_T;                                                \
    typedef typename ::dds::core::smart_ptr_traits<DELEGATE<T_PARAM> >::ref_type          \
        DELEGATE_REF_T;                                                                  \
    typedef BASE<T_PARAM, DELEGATE> BASE_T;                                              \
    explicit TYPE(const DELEGATE_REF_T& ref) : BASE<T_PARAM, DELEGATE>(ref) {}           \
    template <typename H__>                                                              \
    TYPE(const H__& h) : BASE<T_PARAM, DELEGATE>(h) {                                    \
        OMG_DDS_STATIC_ASSERT(                                                           \
            (::dds::core::is_base_of<DELEGATE_T, typename H__::DELEGATE_T>::value));     \
    }

#define OMG_DDS_REF_TYPE_T(TYPE, BASE, T_PARAM, DELEGATE)                                \
    OMG_DDS_REF_TYPE_NODC_T(TYPE, BASE, T_PARAM, DELEGATE)                               \
public:                                                                                  \
    TYPE(const ::dds::core::null_type&) : BASE<T_PARAM, DELEGATE>() {}

// The null handle only, for a type that has its own constructors.
#define OMG_DDS_BASIC_REF_TYPE(TYPE, BASE, DELEGATE)                                     \
public:                                                                                  \
    TYPE(const ::dds::core::null_type&) : BASE<DELEGATE>() {}

// ---- the macros the OMG's refmacros.hpp has besides the composed ones ----------------------

// The base part of a reference type without a default constructor: the typedef of the base,
// the protected constructor from a raw delegate pointer, and construction from another handle
// on a delegate of the same family. Needs DELEGATE_T (see OMG_DDS_REF_TYPE_DELEGATE).
#define OMG_DDS_REF_TYPE_BASE(TYPE, BASE, DELEGATE)                                      \
public:                                                                                  \
    typedef BASE<DELEGATE> BASE_T;                                                       \
protected:                                                                               \
    explicit TYPE(DELEGATE_T* p) : BASE<DELEGATE_T>(p) {}                                \
public:                                                                                  \
    template <typename H__>                                                              \
    TYPE(const H__& h) : BASE<DELEGATE_T>(h) {                                           \
        OMG_DDS_STATIC_ASSERT(                                                           \
            (::dds::core::is_base_of<DELEGATE_T, typename H__::DELEGATE_T>::value));     \
    }

// As OMG_DDS_REF_TYPE_BASE, for a base that takes the sample type `T_PARAM` and the delegate
// template.
#define OMG_DDS_REF_TYPE_BASE_T(TYPE, BASE, T_PARAM, DELEGATE)                           \
public:                                                                                  \
    typedef BASE<T_PARAM, DELEGATE> BASE_T;                                              \
protected:                                                                               \
    explicit TYPE(DELEGATE_T* p) : BASE<T_PARAM, DELEGATE>(p) {}                         \
public:                                                                                  \
    template <typename H__>                                                              \
    TYPE(const H__& h) : BASE<T_PARAM, DELEGATE>(h) {                                    \
        OMG_DDS_STATIC_ASSERT(                                                           \
            (::dds::core::is_base_of<DELEGATE_T, typename H__::DELEGATE_T>::value));     \
    }

// The delegate typedefs and the assignment between handles, for a base with a sample type.
#define OMG_DDS_REF_TYPE_DELEGATE_T(TYPE, BASE, T_PARAM, DELEGATE)                       \
public:                                                                                  \
    typedef DELEGATE<T_PARAM> DELEGATE_T;                                                \
    typedef typename ::dds::core::smart_ptr_traits<DELEGATE<T_PARAM> >::ref_type          \
        DELEGATE_REF_T;                                                                  \
    typedef typename ::dds::core::smart_ptr_traits<DELEGATE<T_PARAM> >::weak_ref_type     \
        DELEGATE_WEAK_REF_T;                                                             \
    template <typename T__>                                                              \
    TYPE& operator=(const T__& rhs) {                                                    \
        OMG_DDS_STATIC_ASSERT(                                                           \
            (::dds::core::is_base_of<DELEGATE_T, typename T__::DELEGATE_T>::value));     \
        if (this != reinterpret_cast<const TYPE*>(&rhs)) {                               \
            *this = TYPE(rhs);                                                           \
        }                                                                                \
        return *this;                                                                    \
    }

// A reference type that is built from a pointer to its delegate: the typedefs, the explicit
// constructors from `DELEGATE_T*`, from any pointer to a type derived from the delegate, from
// the reference and from another handle, and the assignment between handles.
#define DDS_PTR_CTOR_REF_TYPE(TYPE, BASE, DELEGATE)                                      \
public:                                                                                  \
    typedef BASE<DELEGATE> BASE_T;                                                       \
    typedef DELEGATE DELEGATE_T;                                                         \
    typedef typename ::dds::core::smart_ptr_traits<DELEGATE>::ref_type DELEGATE_REF_T;   \
    typedef typename ::dds::core::smart_ptr_traits<DELEGATE>::weak_ref_type              \
        DELEGATE_WEAK_REF_T;                                                             \
    explicit TYPE(DELEGATE_T* p) : BASE<DELEGATE_T>(p) {}                                \
    template <typename PTR>                                                              \
    explicit TYPE(PTR* ptr) : BASE<DELEGATE_T>(DELEGATE_REF_T(ptr)) {                    \
        OMG_DDS_STATIC_ASSERT((::dds::core::is_base_of<DELEGATE_T, PTR>::value));        \
    }                                                                                    \
    explicit TYPE(const DELEGATE_REF_T& ref) : BASE<DELEGATE_T>(ref) {}                  \
    template <typename H__>                                                              \
    explicit TYPE(const H__& h) : BASE<DELEGATE_T>(h) {                                  \
        OMG_DDS_STATIC_ASSERT(                                                           \
            (::dds::core::is_base_of<DELEGATE_T, typename H__::DELEGATE_T>::value));     \
    }                                                                                    \
    template <typename T__>                                                              \
    TYPE& operator=(const T__& rhs) {                                                    \
        OMG_DDS_STATIC_ASSERT(                                                           \
            (::dds::core::is_base_of<DELEGATE_T, typename T__::DELEGATE_T>::value));     \
        if (this != reinterpret_cast<const TYPE*>(&rhs)) {                               \
            *this = TYPE(rhs);                                                           \
        }                                                                                \
        return *this;                                                                    \
    }

#endif // ZERODDS_DDS_CORE_REFMACROS_HPP
