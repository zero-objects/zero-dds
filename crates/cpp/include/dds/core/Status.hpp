// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/Status.hpp — DDS-PSM-Cxx 1.0 §7.5.7 Status-Strukturen.

#ifndef ZERODDS_DDS_CORE_STATUS_HPP
#define ZERODDS_DDS_CORE_STATUS_HPP

#include <cstdint>
#include <vector>

#include "dds/core/InstanceHandle.hpp"

namespace dds {
namespace core {
namespace status {

/// Status-Mask Bits (Spec §7.5.7).
enum class StatusKind : uint32_t {
    InconsistentTopic = 1u << 0,
    OfferedDeadlineMissed = 1u << 1,
    RequestedDeadlineMissed = 1u << 2,
    OfferedIncompatibleQos = 1u << 5,
    RequestedIncompatibleQos = 1u << 6,
    SampleLost = 1u << 7,
    SampleRejected = 1u << 8,
    DataOnReaders = 1u << 9,
    DataAvailable = 1u << 10,
    LivelinessLost = 1u << 11,
    LivelinessChanged = 1u << 12,
    PublicationMatched = 1u << 13,
    SubscriptionMatched = 1u << 14,
};

/// StatusMask (Spec §7.5.7.0).
class StatusMask {
public:
    StatusMask() = default;
    explicit StatusMask(uint32_t v) : value_(v) {}
    static StatusMask all() { return StatusMask(0xFFFFFFFFu); }
    static StatusMask none() { return StatusMask(0u); }
    uint32_t value() const { return value_; }
    StatusMask& operator|=(StatusKind k) {
        value_ |= static_cast<uint32_t>(k);
        return *this;
    }
    bool test(StatusKind k) const {
        return (value_ & static_cast<uint32_t>(k)) != 0;
    }
private:
    uint32_t value_{0};
};

class InconsistentTopicStatus {
public:
    int32_t total_count{0};
    int32_t total_count_change{0};
};

class LivelinessLostStatus {
public:
    int32_t total_count{0};
    int32_t total_count_change{0};
};

class OfferedDeadlineMissedStatus {
public:
    int32_t total_count{0};
    int32_t total_count_change{0};
    InstanceHandle last_instance_handle;
};

class RequestedDeadlineMissedStatus {
public:
    int32_t total_count{0};
    int32_t total_count_change{0};
    InstanceHandle last_instance_handle;
};

class PublicationMatchedStatus {
public:
    int32_t total_count{0};
    int32_t total_count_change{0};
    int32_t current_count{0};
    int32_t current_count_change{0};
    InstanceHandle last_subscription_handle;
};

class SubscriptionMatchedStatus {
public:
    int32_t total_count{0};
    int32_t total_count_change{0};
    int32_t current_count{0};
    int32_t current_count_change{0};
    InstanceHandle last_publication_handle;
};

class LivelinessChangedStatus {
public:
    int32_t alive_count{0};
    int32_t not_alive_count{0};
    int32_t alive_count_change{0};
    int32_t not_alive_count_change{0};
    InstanceHandle last_publication_handle;
};

class OfferedIncompatibleQosStatus {
public:
    int32_t total_count{0};
    int32_t total_count_change{0};
    uint32_t last_policy_id{0};
};

class RequestedIncompatibleQosStatus {
public:
    int32_t total_count{0};
    int32_t total_count_change{0};
    uint32_t last_policy_id{0};
};

class SampleLostStatus {
public:
    int32_t total_count{0};
    int32_t total_count_change{0};
};

enum class SampleRejectedKind : uint32_t {
    NotRejected = 0,
    RejectedByInstancesLimit = 1,
    RejectedBySamplesLimit = 2,
    RejectedBySamplesPerInstanceLimit = 3,
};

class SampleRejectedStatus {
public:
    int32_t total_count{0};
    int32_t total_count_change{0};
    SampleRejectedKind last_reason{SampleRejectedKind::NotRejected};
    InstanceHandle last_instance_handle;
};

} // namespace status
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_STATUS_HPP
