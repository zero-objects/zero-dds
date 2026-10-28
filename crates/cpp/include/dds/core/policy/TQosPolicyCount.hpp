// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/policy/TQosPolicyCount.hpp — DDS-PSM-Cxx 1.0 §7.6.1 `TQosPolicyCount<D>`.

#ifndef ZERODDS_DDS_CORE_POLICY_TQOSPOLICYCOUNT_HPP
#define ZERODDS_DDS_CORE_POLICY_TQOSPOLICYCOUNT_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "dds/core/Value.hpp"
#include "dds/core/types.hpp"

namespace dds {
namespace core {
namespace policy {

/// How often one QoS policy was incompatible, over any delegate (DDS 1.4 §2.2.4.1).
template <typename D>
class TQosPolicyCount : public ::dds::core::Value<D> {
public:
    TQosPolicyCount(QosPolicyId policy_id, int32_t count)
        : ::dds::core::Value<D>(policy_id, count) {}
#if ZERODDS_CXX11
    TQosPolicyCount(const TQosPolicyCount& other) = default;
    TQosPolicyCount(TQosPolicyCount&&) = default;
    TQosPolicyCount& operator=(const TQosPolicyCount& other) = default;
    TQosPolicyCount& operator=(TQosPolicyCount&&) = default;
#endif

    QosPolicyId policy_id() const { return this->d_.policy_id(); }
    int32_t count() const { return this->d_.count(); }
};

} // namespace policy
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_POLICY_TQOSPOLICYCOUNT_HPP
