// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/domain/TDomainId.hpp — DDS-PSM-Cxx 1.0 `TDomainId<DELEGATE>`.

#ifndef ZERODDS_DDS_DOMAIN_TDOMAINID_HPP
#define ZERODDS_DDS_DOMAIN_TDOMAINID_HPP

#include <stdint.h>

#include "dds/core/Value.hpp"

namespace dds {
namespace domain {

/// The identifier of a DDS domain, over any delegate.
template <typename DELEGATE>
class TDomainId : public ::dds::core::Value<DELEGATE> {
public:
    template <typename ARG0>
    TDomainId(ARG0 id) : ::dds::core::Value<DELEGATE>(id) {}

    template <typename ARG0, typename ARG1>
    TDomainId(ARG0 arg0, ARG1 arg1) : ::dds::core::Value<DELEGATE>(arg0, arg1) {}

    /// The id as the integer the participant constructors take.
    operator uint32_t() const { return this->d_.value(); }
    uint32_t value() const { return this->d_.value(); }

    /// The default domain, 0 (DDS 1.4 §2.2.2.2.2).
    static const TDomainId default_domain() { return TDomainId(DELEGATE::default_domain()); }
};

} // namespace domain
} // namespace dds

#endif // ZERODDS_DDS_DOMAIN_TDOMAINID_HPP
