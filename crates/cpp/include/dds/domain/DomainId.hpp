// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/domain/DomainId.hpp — DDS-PSM-Cxx 1.0 `dds::domain::DomainId`.

#ifndef ZERODDS_DDS_DOMAIN_DOMAINID_HPP
#define ZERODDS_DDS_DOMAIN_DOMAINID_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "dds/domain/TDomainId.hpp"

namespace dds {
namespace domain {
namespace detail {

/// The state of a DomainId: the number of the domain.
class DomainId {
public:
    DomainId() : value_(0) {}
    DomainId(uint32_t id) : value_(id) {}

    uint32_t value() const { return value_; }
    bool operator==(const DomainId& other) const { return value_ == other.value_; }
    bool operator!=(const DomainId& other) const { return value_ != other.value_; }

    /// DDS 1.4 §2.2.2.2.2: the default domain is 0.
    static DomainId default_domain() { return DomainId(0); }

private:
    uint32_t value_;
};

} // namespace detail

typedef TDomainId<detail::DomainId> DomainId;

} // namespace domain
} // namespace dds

#endif // ZERODDS_DDS_DOMAIN_DOMAINID_HPP
