// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/array.hpp — DDS-PSM-Cxx 1.0 §7.2: `dds::core::array<T, N>`, the
// fixed-size array the PSM names, and the free `begin`/`end`.
// `array` is `std::array` (C++03: an own class of the same shape).

#ifndef ZERODDS_DDS_CORE_ARRAY_HPP
#define ZERODDS_DDS_CORE_ARRAY_HPP


#include "zerodds/psm/cxx.hpp"
#include <algorithm>
#include <cstddef>
#include <iterator>
#include <stdexcept>

namespace dds {
namespace core {

template <typename T>
typename T::iterator begin(T& t) {
    return t.begin();
}
template <typename T>
typename T::iterator end(T& t) {
    return t.end();
}
template <typename T>
typename T::const_iterator begin(const T& t) {
    return t.begin();
}
template <typename T>
typename T::const_iterator end(const T& t) {
    return t.end();
}

/// `dds::core::array` is `std::array` (DDS-PSM-Cxx 1.0 §7.12; §7.4.2 allows an own type where
/// `std::array` is missing: with a C++03 compiler it is `::zerodds::cxx::array`, an aggregate with
/// the same members). A using-declaration names the class template in both cases.
using ::zerodds::cxx::array;
using ::zerodds::cxx::get;

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_ARRAY_HPP
