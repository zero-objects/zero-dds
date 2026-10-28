// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/TSuspendedPublication.hpp — DDS-PSM-Cxx 1.0 `TSuspendedPublication<D>`.

#ifndef ZERODDS_DDS_PUB_TSUSPENDEDPUBLICATION_HPP
#define ZERODDS_DDS_PUB_TSUSPENDEDPUBLICATION_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/Value.hpp"
#include "dds/pub/Publisher.hpp"

namespace dds {
namespace pub {

/// Suspends the publications of a publisher from the constructor until
/// `resume()` or, at the latest, the destructor.
template <typename DELEGATE>
class TSuspendedPublication : public ::dds::core::Value<DELEGATE> {
public:
    explicit TSuspendedPublication(const ::dds::pub::Publisher& pub)
        : ::dds::core::Value<DELEGATE>(pub) {}

    ZERODDS_NONCOPYABLE(TSuspendedPublication)

    /// Resumes the publications explicitly.
    void resume() { this->delegate().resume(); }

#if ZERODDS_CXX11
    ~TSuspendedPublication() = default;
#endif
};

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_TSUSPENDEDPUBLICATION_HPP
