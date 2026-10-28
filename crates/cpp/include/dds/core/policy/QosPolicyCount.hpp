// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/policy/QosPolicyCount.hpp — DDS-PSM-Cxx 1.0 §7.5.4, DDS 1.4
// §2.2.4.1 QosPolicyCount.

#ifndef ZERODDS_DDS_CORE_POLICY_QOSPOLICYCOUNT_HPP
#define ZERODDS_DDS_CORE_POLICY_QOSPOLICYCOUNT_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <vector>

#include "dds/core/policy/TQosPolicyCount.hpp"
#include "dds/core/types.hpp"

namespace dds {
namespace core {
namespace policy {

/// How often one QoS policy was incompatible (DDS 1.4 §2.2.4.1).
namespace detail {

class QosPolicyCount {
public:
    QosPolicyCount(QosPolicyId policy_id, int32_t count) : policy_id_(policy_id), count_(count) {}
#if ZERODDS_CXX11
    QosPolicyCount(const QosPolicyCount& other) = default;
    QosPolicyCount(QosPolicyCount&&) = default;
    QosPolicyCount& operator=(const QosPolicyCount& other) = default;
    QosPolicyCount& operator=(QosPolicyCount&&) = default;
#endif

    QosPolicyId policy_id() const { return policy_id_; }
    int32_t count() const { return count_; }

    bool operator==(const QosPolicyCount& rhs) const {
        return policy_id_ == rhs.policy_id_ && count_ == rhs.count_;
    }

private:
    QosPolicyId policy_id_;
    int32_t count_;
};

} // namespace detail

typedef TQosPolicyCount<detail::QosPolicyCount> QosPolicyCount;
typedef std::vector<QosPolicyCount> QosPolicyCountSeq;

} // namespace policy
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_POLICY_QOSPOLICYCOUNT_HPP
