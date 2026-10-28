// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/LoanedSamples.hpp — LoanedSamples<T> (DDS-PSM-Cxx 1.0 §7.5.4,
// §7.12): the samples a read or take returned. A move-only type; the loan the
// C API opened is returned before `read` or `take` returns.

#ifndef ZERODDS_DDS_SUB_LOANEDSAMPLES_HPP
#define ZERODDS_DDS_SUB_LOANEDSAMPLES_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <utility>
#include <vector>

#include "dds/sub/Sample.hpp"

namespace dds {
namespace sub {

template <typename T>
class SharedSamples;

template <typename T>
class LoanedSamples {
public:
    typedef T DataType;
    typedef Sample<T> value_type;
    typedef typename std::vector<Sample<T> >::const_iterator const_iterator;
    typedef const_iterator iterator;

    LoanedSamples() : samples_() {}
    /// The samples of one read or take.
    explicit LoanedSamples(std::vector<Sample<T> > samples) : samples_(ZERODDS_MOVE(samples)) {}

#if ZERODDS_CXX11
    // Move-only (§7.12). A C++03 compiler has no move operations: there the copy of the
    // implicit copy operations stands in for them.
    ZERODDS_NONCOPYABLE(LoanedSamples)
    LoanedSamples(LoanedSamples&&) ZERODDS_NOEXCEPT = default;
    LoanedSamples& operator=(LoanedSamples&&) ZERODDS_NOEXCEPT = default;
    ~LoanedSamples() = default;
#endif

    const_iterator begin() const { return samples_.begin(); }
    const_iterator end() const { return samples_.end(); }
    /// `const_iterator` irrespective of the constness of the object (§7.12).
    const_iterator cbegin() const { return samples_.begin(); }
    const_iterator cend() const { return samples_.end(); }
    uint32_t length() const { return static_cast<uint32_t>(samples_.size()); }

private:
    friend class SharedSamples<T>;
    std::vector<Sample<T> > samples_;
};

/// §7.12: behaves like `std::move`.
template <typename T>
LoanedSamples<T> move(LoanedSamples<T>& a) {
    return LoanedSamples<T>(ZERODDS_MOVE(a));
}

/// §7.12: namespace level functions for range-based `for`.
template <typename T>
typename LoanedSamples<T>::const_iterator begin(const LoanedSamples<T>& s) {
    return s.begin();
}
template <typename T>
typename LoanedSamples<T>::const_iterator end(const LoanedSamples<T>& s) {
    return s.end();
}
template <typename T>
typename LoanedSamples<T>::const_iterator cbegin(const LoanedSamples<T>& s) {
    return s.cbegin();
}
template <typename T>
typename LoanedSamples<T>::const_iterator cend(const LoanedSamples<T>& s) {
    return s.cend();
}

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_LOANEDSAMPLES_HPP
