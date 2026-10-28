// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/classic/convert.hpp — what a header generated with `--cpp-mapping classic` uses to hand
// a type of the classic OMG IDL to C++ mapping to the codec.
//
// The generator writes the types of the classic mapping (`CORBA::Long`, `CORBA::String_var`,
// `zerodds::classic::Sequence<T>`, structures with public members, unions with accessor
// functions, plain enumerations) in the namespaces of the IDL, and the native types the codec
// reads and writes in `zd_native`. `Conv<C>` is the join, as `zerodds::psm::Conv<P>` is for the
// PSM mapping of DDS-PSM-Cxx: for a classic type `C` it names the native type and converts both
// ways, and the `topic_type_support` of `C` passes the sample through it.

#ifndef ZERODDS_CLASSIC_CONVERT_HPP
#define ZERODDS_CLASSIC_CONVERT_HPP

#include <array>
#include <cstddef>
#include <memory>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

#include "zerodds/classic.hpp"
#include "zerodds/map.hpp"
#include "zerodds/psm/convert.hpp"

namespace zerodds {
namespace classic {

/// The native type of the classic type `C` and the conversions between them. The primary
/// template is the one of the PSM mapping — the identity, and the conversions of `wchar_t`,
/// `std::wstring`, `long double` and the standard containers.
template <typename C, typename Enable = void>
struct Conv : ::zerodds::psm::Conv<C> {};

/// `string` — a `CORBA::String_var`; the native string is a `std::string`. A null pointer is the
/// empty string.
template <>
struct Conv<::CORBA::String_var> {
    using Native = std::string;
    static Native to_native(const ::CORBA::String_var& v) {
        const char* text = v.in();
        return text != nullptr ? std::string(text) : std::string();
    }
    static ::CORBA::String_var from_native(const Native& n) { return ::CORBA::String_var(n.c_str()); }
};

/// `wstring` — a `CORBA::WString_var`; the native string holds UTF-8.
template <>
struct Conv<::CORBA::WString_var> {
    using Native = std::string;
    static Native to_native(const ::CORBA::WString_var& v) {
        const wchar_t* text = v.in();
        return ::zerodds::psm::Conv<std::wstring>::to_native(text != nullptr ? std::wstring(text)
                                                                             : std::wstring());
    }
    static ::CORBA::WString_var from_native(const Native& n) {
        const std::wstring wide = ::zerodds::psm::Conv<std::wstring>::from_native(n);
        return ::CORBA::WString_var(wide.c_str());
    }
};

/// `sequence<T>` — `zerodds::classic::Sequence<T>`; the native sequence is a `std::vector`.
template <typename T>
struct Conv<::zerodds::classic::Sequence<T>> {
    using Native = std::vector<typename Conv<T>::Native>;
    static Native to_native(const ::zerodds::classic::Sequence<T>& v) {
        Native out;
        out.reserve(v.length());
        for (::CORBA::ULong i = 0; i < v.length(); ++i) {
            out.push_back(Conv<T>::to_native(v[i]));
        }
        return out;
    }
    static ::zerodds::classic::Sequence<T> from_native(const Native& n) {
        ::zerodds::classic::Sequence<T> out;
        out.length(static_cast<::CORBA::ULong>(n.size()));
        for (std::size_t i = 0; i < n.size(); ++i) {
            out[static_cast<::CORBA::ULong>(i)] = Conv<T>::from_native(n[i]);
        }
        return out;
    }
};

/// `@optional` — `zerodds::classic::Optional<T>`; the native member is a plain `T *`, NULL when
/// absent (DDS-XTypes 1.3 §7.5.1.2.4.2), which the class that holds it owns and deletes.
template <typename T>
struct Conv<::zerodds::classic::Optional<T>> {
    using Native = typename Conv<T>::Native*;
    static Native to_native(const ::zerodds::classic::Optional<T>& v) {
        if (!v.has_value()) {
            return nullptr;
        }
        return new typename Conv<T>::Native(Conv<T>::to_native(v.value()));
    }
    static ::zerodds::classic::Optional<T> from_native(const Native& n) {
        if (n == nullptr) {
            return ::zerodds::classic::Optional<T>();
        }
        return ::zerodds::classic::Optional<T>(Conv<T>::from_native(*n));
    }
};

/// `@external` — `zerodds::classic::Var<T>`; the native member is a `std::shared_ptr`.
template <typename T>
struct Conv<::zerodds::classic::Var<T>> {
    using Native = std::shared_ptr<typename Conv<T>::Native>;
    static Native to_native(const ::zerodds::classic::Var<T>& v) {
        if (v.ptr() == nullptr) {
            return nullptr;
        }
        return std::make_shared<typename Conv<T>::Native>(Conv<T>::to_native(*v));
    }
    static ::zerodds::classic::Var<T> from_native(const Native& n) {
        if (!n) {
            return ::zerodds::classic::Var<T>();
        }
        return ::zerodds::classic::Var<T>(new T(Conv<T>::from_native(*n)));
    }
};

/// `map<K, V>` — `zerodds::Map`, whose entries are converted one by one.
template <typename K, typename V, std::uint32_t Bound>
struct Conv<::zerodds::Map<K, V, Bound>> {
    using Native = ::zerodds::Map<typename Conv<K>::Native, typename Conv<V>::Native, Bound>;
    static Native to_native(const ::zerodds::Map<K, V, Bound>& v) {
        typename Native::Base out;
        out.reserve(v.size());
        for (const auto& e : v) {
            out.emplace_back(Conv<K>::to_native(e.first), Conv<V>::to_native(e.second));
        }
        Native map(out);
        return map;
    }
    static ::zerodds::Map<K, V, Bound> from_native(const Native& n) {
        typename ::zerodds::Map<K, V, Bound>::Base out;
        out.reserve(n.size());
        for (const auto& e : n) {
            out.emplace_back(Conv<K>::from_native(e.first), Conv<V>::from_native(e.second));
        }
        ::zerodds::Map<K, V, Bound> map(out);
        return map;
    }
};

// ---- one member --------------------------------------------------------------------------

/// The member `c` of a classic structure, as the native member `n` has it. An array is a C array
/// on the classic side and a `std::array` on the native one, converted element by element.
template <typename C, typename N>
void to_native_member(N& n, const C& c) {
    n = ::zerodds::psm::Shape<N, typename Conv<C>::Native>::apply(Conv<C>::to_native(c));
}

template <typename C, std::size_t S, typename N>
void to_native_member(N& n, const C (&c)[S]) {
    for (std::size_t i = 0; i < S; ++i) {
        to_native_member(n[i], c[i]);
    }
}

/// The native member `n`, as the classic member `c` has it.
template <typename C, typename N>
void from_native_member(C& c, const N& n) {
    c = Conv<C>::from_native(::zerodds::psm::Shape<typename Conv<C>::Native, N>::apply(n));
}

template <typename C, std::size_t S, typename N>
void from_native_member(C (&c)[S], const N& n) {
    for (std::size_t i = 0; i < S; ++i) {
        from_native_member(c[i], n[i]);
    }
}

}  // namespace classic
}  // namespace zerodds

#endif  // ZERODDS_CLASSIC_CONVERT_HPP
