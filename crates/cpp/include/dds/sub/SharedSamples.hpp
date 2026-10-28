// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/SharedSamples.hpp — SharedSamples<T> (DDS-PSM-Cxx 1.0 §7.12): the
// sharable version of LoanedSamples<T>. A reference type: copies share the
// samples, which are released with the last copy.

#ifndef ZERODDS_DDS_SUB_SHAREDSAMPLES_HPP
#define ZERODDS_DDS_SUB_SHAREDSAMPLES_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <memory>
#include <utility>
#include <vector>

#include "dds/core/Reference.hpp"
#include "dds/sub/LoanedSamples.hpp"

namespace dds {
namespace sub {
namespace detail {

template <typename T>
class SharedSamplesDelegate {
public:
    typedef typename std::vector<Sample<T> >::const_iterator const_iterator;
    typedef const_iterator iterator;

    explicit SharedSamplesDelegate(std::vector<Sample<T> > samples)
        : samples_(ZERODDS_MOVE(samples)) {}

    const_iterator begin() const { return samples_.begin(); }
    const_iterator end() const { return samples_.end(); }
    uint32_t length() const { return static_cast<uint32_t>(samples_.size()); }

private:
    std::vector<Sample<T> > samples_;
};

} // namespace detail

template <typename T>
class SharedSamples : public ::dds::core::Reference<detail::SharedSamplesDelegate<T> > {
public:
    typedef T DataType;
    typedef detail::SharedSamplesDelegate<T> DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE_T> BASE_T;
    typedef typename DELEGATE_T::const_iterator const_iterator;
    typedef const_iterator iterator;

    /// Takes the samples from `ls`: the destruction of `ls` has no effect on
    /// them, and they are released when the last copy of this object is.
    SharedSamples(LoanedSamples<T> ls)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE_T>(ZERODDS_MOVE(ls.samples_))) {}
    /// The null SharedSamples (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    SharedSamples(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T() {}

#if ZERODDS_CXX11
    SharedSamples(const SharedSamples&) = default;
#endif

#if ZERODDS_CXX11
    SharedSamples(SharedSamples&&) = default;
    SharedSamples& operator=(const SharedSamples&) = default;
    SharedSamples& operator=(SharedSamples&&) = default;
    ~SharedSamples() = default;
#endif

    const_iterator begin() const { return this->delegate()->begin(); }
    const_iterator end() const { return this->delegate()->end(); }
    const_iterator cbegin() const { return this->delegate()->begin(); }
    const_iterator cend() const { return this->delegate()->end(); }
    uint32_t length() const { return this->delegate()->length(); }
};

template <typename T>
typename SharedSamples<T>::const_iterator begin(const SharedSamples<T>& s) {
    return s.begin();
}
template <typename T>
typename SharedSamples<T>::const_iterator end(const SharedSamples<T>& s) {
    return s.end();
}
template <typename T>
typename SharedSamples<T>::const_iterator cbegin(const SharedSamples<T>& s) {
    return s.cbegin();
}
template <typename T>
typename SharedSamples<T>::const_iterator cend(const SharedSamples<T>& s) {
    return s.cend();
}

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_SHAREDSAMPLES_HPP
