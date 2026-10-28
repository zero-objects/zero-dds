// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/psm/convert.hpp — what a header generated with `--cpp-mapping psm` uses to
// hand a DDS-PSM-Cxx 1.0 §8.1 topic type to the codec.
//
// The generator writes two sets of types. The native set (`zd_native::…`) is what the
// wire codec reads and writes. The PSM set is what the application sees, in the
// namespaces of the IDL modules: classes with the accessors of §7.4.6, `wchar_t`,
// `std::wstring`, `long double`, `std::map`, `dds::core::array` and `dds::core::optional`
// (Table 7.1, §8.1.4). `Conv<P>` is the join: for a PSM type `P` it names the native
// type and converts both ways. The codec, the TypeObject and the endpoint gates are the
// native ones; a PSM sample costs one conversion per `write` and per `take`.

#ifndef ZERODDS_PSM_CONVERT_HPP
#define ZERODDS_PSM_CONVERT_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <array>
#include <cmath>
#include <cstddef>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

#include "dds/core/Optional.hpp"
#include "dds/core/array.hpp"
#include "dds/core/external.hpp"
#include "zerodds/wire.hpp"

namespace zerodds {
namespace psm {

/// The native type of the PSM type `P` and the conversions between them. The primary
/// template is the identity: the PSM type and the native type are one.
template <typename P, typename Enable = void>
struct Conv {
    typedef P Native;
    static Native to_native(const P& v) { return v; }
    static P from_native(const Native& n) { return n; }
};

// ---- characters and strings -------------------------------------------------------

/// `wchar` — the native holder is the 16-bit code unit (XTypes 1.3 §7.4.1.1.2), the PSM
/// type is `wchar_t` (Table 7.1).
template <>
struct Conv<wchar_t> {
    typedef uint16_t Native;
    static Native to_native(wchar_t v) { return static_cast<uint16_t>(v); }
    static wchar_t from_native(Native n) { return static_cast<wchar_t>(n); }
};

/// `wstring` — the native type holds UTF-8; the PSM type is `std::wstring`.
template <>
struct Conv<std::wstring> {
    typedef std::string Native;
    static Native to_native(const std::wstring& v) {
        std::vector<char32_t> points;
        points.reserve(v.size());
        for (std::size_t i = 0; i < v.size(); ++i) {
            uint32_t unit = static_cast<uint32_t>(v[i]);
            // A 16-bit `wchar_t` holds UTF-16: a high surrogate and the low one after it
            // are one code point.
            if (sizeof(wchar_t) == 2 && unit >= 0xD800 && unit <= 0xDBFF && i + 1 < v.size()) {
                const uint32_t low = static_cast<uint32_t>(v[i + 1]);
                if (low >= 0xDC00 && low <= 0xDFFF) {
                    unit = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                    ++i;
                }
            }
            points.push_back(static_cast<char32_t>(unit));
        }
        return wire::detail::utf8_of(points);
    }
    static std::wstring from_native(const Native& n) {
        std::wstring out;
        for (char32_t point : wire::detail::utf8_code_points(n)) {
            if (sizeof(wchar_t) == 2 && point > 0xFFFF) {
                const uint32_t v = static_cast<uint32_t>(point) - 0x10000;
                out.push_back(static_cast<wchar_t>(0xD800 + (v >> 10)));
                out.push_back(static_cast<wchar_t>(0xDC00 + (v & 0x3FF)));
            } else {
                out.push_back(static_cast<wchar_t>(point));
            }
        }
        return out;
    }
};

// ---- long double --------------------------------------------------------------------

/// `long double` (Table 7.1) over the 16 octets of an IEEE binary128 value. Converting to
/// `long double` rounds to the width of the platform's `long double`; where that is as
/// wide as `double`, the value is the `double` nearest to the binary128 value.
template <>
struct Conv<long double> {
    typedef wire::Float128 Native;

    static Native to_native(long double value) {
        if (std::isnan(value)) {
            return Native::quiet_nan();
        }
        const bool negative = std::signbit(value);
        if (std::isinf(value)) {
            return Native::infinity(negative);
        }
        if (value == 0.0L) {
            return Native::zero(negative);
        }
        const long double magnitude = std::fabs(value);
        int exponent = 0;
        // magnitude = fraction * 2^exponent with fraction in [0.5, 1)
        long double fraction = std::frexp(magnitude, &exponent);
        // value = (2 * fraction) * 2^(exponent - 1), the significand in [1, 2)
        int unbiased = exponent - 1;
        uint64_t biased = 0;
        long double significand = 0.0L;
        if (unbiased >= -16382) {
            biased = static_cast<uint64_t>(unbiased + 16383);
            significand = 2.0L * fraction - 1.0L;  // the 112 fraction bits, as a fraction
        } else {
            // Subnormal in binary128: no implicit leading one.
            biased = 0;
            significand = std::ldexp(magnitude, 16382);
        }
        uint64_t high = 0;
        uint64_t low = 0;
        for (int bit = 0; bit < 112; ++bit) {
            significand *= 2.0L;
            const bool one = significand >= 1.0L;
            if (one) {
                significand -= 1.0L;
            }
            if (bit < 48) {
                high = (high << 1) | (one ? 1U : 0U);
            } else {
                low = (low << 1) | (one ? 1U : 0U);
            }
        }
        high |= biased << 48;
        if (negative) {
            high |= uint64_t{1} << 63;
        }
        return Native::from_parts(high, low);
    }

    static long double from_native(const Native& n) {
        const bool negative = (n.high >> 63) != 0;
        const uint64_t biased = (n.high >> 48) & 0x7FFF;
        if (biased == 0x7FFF) {
            const bool fraction_zero = (n.high & 0x0000FFFFFFFFFFFFULL) == 0 && n.low == 0;
            if (fraction_zero) {
                return negative ? -HUGE_VALL : HUGE_VALL;
            }
            return std::nanl("");
        }
        long double sum = biased == 0 ? 0.0L : 1.0L;
        long double scale = 0.5L;
        for (int bit = 0; bit < 112; ++bit) {
            const bool one = bit < 48 ? ((n.high >> (47 - bit)) & 1U) != 0
                                      : ((n.low >> (111 - bit)) & 1U) != 0;
            if (one) {
                sum += scale;
            }
            scale *= 0.5L;
        }
        const int exponent = biased == 0 ? -16382 : static_cast<int>(biased) - 16383;
        const long double value = std::ldexp(sum, exponent);
        return negative ? -value : value;
    }
};

// ---- containers ---------------------------------------------------------------------

template <typename E>
struct Conv<std::vector<E> > {
    typedef std::vector<typename Conv<E>::Native> Native;
    static Native to_native(const std::vector<E>& v) {
        Native out;
        out.reserve(v.size());
        for (const auto& e : v) {
            out.push_back(Conv<E>::to_native(e));
        }
        return out;
    }
    static std::vector<E> from_native(const Native& n) {
        std::vector<E> out;
        out.reserve(n.size());
        for (const auto& e : n) {
            out.push_back(Conv<E>::from_native(e));
        }
        return out;
    }
};

template <typename E, std::size_t N>
struct Conv<::dds::core::array<E, N> > {
    typedef std::array<typename Conv<E>::Native, N> Native;
    static Native to_native(const ::dds::core::array<E, N>& v) {
        Native out{};
        for (std::size_t i = 0; i < N; ++i) {
            out[i] = Conv<E>::to_native(v[i]);
        }
        return out;
    }
    static ::dds::core::array<E, N> from_native(const Native& n) {
        ::dds::core::array<E, N> out{};
        for (std::size_t i = 0; i < N; ++i) {
            out[i] = Conv<E>::from_native(n[i]);
        }
        return out;
    }
};

/// `map<K, V>` — `std::map` for the application (Table 7.1), a vector of entries in key
/// order for the codec.
template <typename K, typename V>
struct Conv<std::map<K, V> > {
    typedef std::vector<std::pair<typename Conv<K>::Native, typename Conv<V>::Native> > Native;
    static Native to_native(const std::map<K, V>& v) {
        Native out;
        out.reserve(v.size());
        for (const auto& entry : v) {
            out.emplace_back(Conv<K>::to_native(entry.first), Conv<V>::to_native(entry.second));
        }
        return out;
    }
    static std::map<K, V> from_native(const Native& n) {
        std::map<K, V> out;
        for (const auto& entry : n) {
            out.emplace(Conv<K>::from_native(entry.first), Conv<V>::from_native(entry.second));
        }
        return out;
    }
};

/// `@optional` — `dds::core::optional` (§8.1.4) for the application, a plain pointer for the
/// codec (DDS-XTypes 1.3 §7.5.1.2.4.2: NULL for an omitted value). The native type owns what
/// the pointer points at, so `to_native` hands over an object the native member takes.
template <typename E>
struct Conv<::dds::core::optional<E> > {
    typedef typename Conv<E>::Native* Native;
    static Native to_native(const ::dds::core::optional<E>& v) {
        if (!v.is_set()) {
            return nullptr;
        }
        return new typename Conv<E>::Native(Conv<E>::to_native(v.get()));
    }
    static ::dds::core::optional<E> from_native(const Native& n) {
        if (n == nullptr) {
            return ::dds::core::optional<E>();
        }
        return ::dds::core::optional<E>(Conv<E>::from_native(*n));
    }
};

/// `@external` and `@shared` members — a `::zerodds::cxx::shared_ptr` on both sides.
template <typename E>
struct Conv<::zerodds::cxx::shared_ptr<E> > {
    typedef ::zerodds::cxx::shared_ptr<typename Conv<E>::Native> Native;
    static Native to_native(const ::zerodds::cxx::shared_ptr<E>& v) {
        if (!v) {
            return nullptr;
        }
        return ::zerodds::cxx::make_shared<typename Conv<E>::Native>(Conv<E>::to_native(*v));
    }
    static ::zerodds::cxx::shared_ptr<E> from_native(const Native& n) {
        if (!n) {
            return nullptr;
        }
        return ::zerodds::cxx::make_shared<E>(Conv<E>::from_native(*n));
    }
};

/// `@external` members in the C++ binding for DDS-PSM-CXX — `dds::core::external<E>`
/// (DDS-XTypes 1.3 §7.5.1.2.3.3), a `::zerodds::cxx::shared_ptr` on the native side.
template <typename E>
struct Conv<::dds::core::external<E> > {
    typedef ::zerodds::cxx::shared_ptr<typename Conv<E>::Native> Native;
    static Native to_native(const ::dds::core::external<E>& v) {
        if (!v) {
            return nullptr;
        }
        return ::zerodds::cxx::make_shared<typename Conv<E>::Native>(Conv<E>::to_native(*v));
    }
    static ::dds::core::external<E> from_native(const Native& n) {
        if (!n) {
            return ::dds::core::external<E>();
        }
        return ::dds::core::external<E>(::zerodds::cxx::make_shared<E>(Conv<E>::from_native(*n)));
    }
};

// ---- the shape of a native member ------------------------------------------------------

/// `Shape<To, From>::apply(from)`: `from` as a `To`, where the two differ only in the element
/// types they are made of.
///
/// DDS-PSM-Cxx 1.0 spells `octet` and `uint8` both `uint8_t`; the native binding tells them
/// apart, as XTypes 1.3 §7.5.1.1.2 asks (`octet` is `zerodds::wire::Octet`, `uint8` is
/// `std::uint8_t`). `Conv<uint8_t>` cannot know which of the two a member is, so the generated
/// conversion of a member hands the result of `Conv` to `Shape` with the type the native member
/// has. The types are the same everywhere but where an octet is, and there `To(from)` or the
/// element-wise conversion of a container is the whole of it.
template <typename To, typename From, typename Enable = void>
struct Shape {
    static To apply(const From& from) { return To(from); }
};

template <typename T>
struct Shape<T, T, void> {
    static T apply(const T& from) { return from; }
};

template <typename A, typename B>
struct Shape<std::vector<A>, std::vector<B>, typename std::enable_if<!std::is_same<A, B>::value>::type> {
    static std::vector<A> apply(const std::vector<B>& from) {
        std::vector<A> out;
        out.reserve(from.size());
        for (const auto& e : from) {
            out.push_back(Shape<A, B>::apply(e));
        }
        return out;
    }
};

template <typename A, std::size_t N, typename B>
struct Shape<std::array<A, N>, std::array<B, N>, typename std::enable_if<!std::is_same<A, B>::value>::type> {
    static std::array<A, N> apply(const std::array<B, N>& from) {
        std::array<A, N> out{};
        for (std::size_t i = 0; i < N; ++i) {
            out[i] = Shape<A, B>::apply(from[i]);
        }
        return out;
    }
};

template <typename A, typename B>
struct Shape<std::optional<A>, std::optional<B>, typename std::enable_if<!std::is_same<A, B>::value>::type> {
    static std::optional<A> apply(const std::optional<B>& from) {
        if (!from.has_value()) {
            return std::nullopt;
        }
        return Shape<A, B>::apply(*from);
    }
};

template <typename A, typename B>
struct Shape<std::shared_ptr<A>, std::shared_ptr<B>, typename std::enable_if<!std::is_same<A, B>::value>::type> {
    static std::shared_ptr<A> apply(const std::shared_ptr<B>& from) {
        if (!from) {
            return nullptr;
        }
        return std::make_shared<A>(Shape<A, B>::apply(*from));
    }
};

/// A native map is a `zerodds::Map` and `Conv` of a `std::map` is a vector of pairs.
template <typename K, typename V, std::uint32_t Bound, typename K2, typename V2>
struct Shape<::zerodds::Map<K, V, Bound>, std::vector<std::pair<K2, V2>>> {
    static ::zerodds::Map<K, V, Bound> apply(const std::vector<std::pair<K2, V2>>& from) {
        typename ::zerodds::Map<K, V, Bound>::Base out;
        out.reserve(from.size());
        for (const auto& e : from) {
            out.emplace_back(Shape<K, K2>::apply(e.first), Shape<V, V2>::apply(e.second));
        }
        return ::zerodds::Map<K, V, Bound>(out);
    }
};

template <typename K, typename V, std::uint32_t Bound, typename K2, typename V2>
struct Shape<std::vector<std::pair<K2, V2>>, ::zerodds::Map<K, V, Bound>> {
    static std::vector<std::pair<K2, V2>> apply(const ::zerodds::Map<K, V, Bound>& from) {
        std::vector<std::pair<K2, V2>> out;
        out.reserve(from.size());
        for (const auto& e : from) {
            out.emplace_back(Shape<K2, K>::apply(e.first), Shape<V2, V>::apply(e.second));
        }
        return out;
    }
};

}  // namespace psm
}  // namespace zerodds

#endif  // ZERODDS_PSM_CONVERT_HPP
