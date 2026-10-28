// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/status/TStatus.hpp — DDS-PSM-Cxx 1.0 §7.5.4 the status classes as
// templates over a delegate `D`. Each attribute has the three accessors of
// §7.4.6: a getter, a getter returning a reference, and a setter.

#ifndef ZERODDS_DDS_CORE_STATUS_TSTATUS_HPP
#define ZERODDS_DDS_CORE_STATUS_TSTATUS_HPP

#include <stdint.h>

#include "dds/core/InstanceHandle.hpp"
#include "dds/core/Value.hpp"
#include "dds/core/policy/QosPolicyCount.hpp"
#include "dds/core/status/State.hpp"
#include "dds/core/types.hpp"

// The three accessors of one attribute, forwarded to the delegate: by value
// (NT) or by const reference (CT), as ZERODDS_STATUS_NT/CT in Status.hpp.
#define ZERODDS_TSTATUS_NT(NT, name)                    \
    NT name() const { return this->d_.name(); }         \
    NT& name() { return this->d_.name(); }              \
    void name(NT value) { this->d_.name(value); }
#define ZERODDS_TSTATUS_CT(CT, name)                    \
    CT name() const { return this->d_.name(); }         \
    CT& name() { return this->d_.name(); }              \
    void name(const CT& value) { this->d_.name(value); }

// The attributes of OfferedIncompatibleQosStatus and
// RequestedIncompatibleQosStatus; `policies(dst)` copies the per-policy counts
// into `dst` and returns it.
#define ZERODDS_TSTATUS_INCOMPATIBLE_QOS                                       \
    ZERODDS_TSTATUS_NT(int32_t, total_count)                                   \
    ZERODDS_TSTATUS_NT(int32_t, total_count_change)                            \
    ZERODDS_TSTATUS_NT(policy::QosPolicyId, last_policy_id)                    \
    policy::QosPolicyCountSeq policies() const { return this->d_.policies(); } \
    policy::QosPolicyCountSeq& policies() { return this->d_.policies(); }      \
    policy::QosPolicyCountSeq& policies(policy::QosPolicyCountSeq& dst) const { \
        return this->d_.policies(dst);                                         \
    }

namespace dds {
namespace core {
namespace status {

/// The `InconsistentTopicStatus` API over any delegate `D` (DDS-PSM-Cxx 1.0 §7.5.4).
template <typename D>
class TInconsistentTopicStatus : public ::dds::core::Value<D> {
public:
    TInconsistentTopicStatus() : ::dds::core::Value<D>() {}
    TInconsistentTopicStatus(int32_t total_count, int32_t total_count_change)
        : ::dds::core::Value<D>(total_count, total_count_change) {}

    ZERODDS_TSTATUS_NT(int32_t, total_count)
    ZERODDS_TSTATUS_NT(int32_t, total_count_change)
};

/// The `LivelinessLostStatus` API over any delegate `D` (DDS-PSM-Cxx 1.0 §7.5.4).
template <typename D>
class TLivelinessLostStatus : public ::dds::core::Value<D> {
public:
    TLivelinessLostStatus() : ::dds::core::Value<D>() {}
    TLivelinessLostStatus(int32_t total_count, int32_t total_count_change)
        : ::dds::core::Value<D>(total_count, total_count_change) {}

    ZERODDS_TSTATUS_NT(int32_t, total_count)
    ZERODDS_TSTATUS_NT(int32_t, total_count_change)
};

/// The `SampleLostStatus` API over any delegate `D` (DDS-PSM-Cxx 1.0 §7.5.4).
template <typename D>
class TSampleLostStatus : public ::dds::core::Value<D> {
public:
    TSampleLostStatus() : ::dds::core::Value<D>() {}
    TSampleLostStatus(int32_t total_count, int32_t total_count_change)
        : ::dds::core::Value<D>(total_count, total_count_change) {}

    ZERODDS_TSTATUS_NT(int32_t, total_count)
    ZERODDS_TSTATUS_NT(int32_t, total_count_change)
};

/// The `OfferedDeadlineMissedStatus` API over any delegate `D` (DDS-PSM-Cxx 1.0 §7.5.4).
template <typename D>
class TOfferedDeadlineMissedStatus : public ::dds::core::Value<D> {
public:
    TOfferedDeadlineMissedStatus() : ::dds::core::Value<D>() {}
    TOfferedDeadlineMissedStatus(int32_t total_count, int32_t total_count_change, const InstanceHandle& last_instance_handle)
        : ::dds::core::Value<D>(total_count, total_count_change, last_instance_handle) {}

    ZERODDS_TSTATUS_NT(int32_t, total_count)
    ZERODDS_TSTATUS_NT(int32_t, total_count_change)
    ZERODDS_TSTATUS_CT(InstanceHandle, last_instance_handle)
};

/// The `RequestedDeadlineMissedStatus` API over any delegate `D` (DDS-PSM-Cxx 1.0 §7.5.4).
template <typename D>
class TRequestedDeadlineMissedStatus : public ::dds::core::Value<D> {
public:
    TRequestedDeadlineMissedStatus() : ::dds::core::Value<D>() {}
    TRequestedDeadlineMissedStatus(int32_t total_count, int32_t total_count_change, const InstanceHandle& last_instance_handle)
        : ::dds::core::Value<D>(total_count, total_count_change, last_instance_handle) {}

    ZERODDS_TSTATUS_NT(int32_t, total_count)
    ZERODDS_TSTATUS_NT(int32_t, total_count_change)
    ZERODDS_TSTATUS_CT(InstanceHandle, last_instance_handle)
};

/// The `PublicationMatchedStatus` API over any delegate `D` (DDS-PSM-Cxx 1.0 §7.5.4).
template <typename D>
class TPublicationMatchedStatus : public ::dds::core::Value<D> {
public:
    TPublicationMatchedStatus() : ::dds::core::Value<D>() {}
    TPublicationMatchedStatus(int32_t total_count, int32_t total_count_change, int32_t current_count, int32_t current_count_change, const InstanceHandle& last_subscription_handle)
        : ::dds::core::Value<D>(total_count, total_count_change, current_count, current_count_change, last_subscription_handle) {}

    ZERODDS_TSTATUS_NT(int32_t, total_count)
    ZERODDS_TSTATUS_NT(int32_t, total_count_change)
    ZERODDS_TSTATUS_NT(int32_t, current_count)
    ZERODDS_TSTATUS_NT(int32_t, current_count_change)
    ZERODDS_TSTATUS_CT(InstanceHandle, last_subscription_handle)
};

/// The `SubscriptionMatchedStatus` API over any delegate `D` (DDS-PSM-Cxx 1.0 §7.5.4).
template <typename D>
class TSubscriptionMatchedStatus : public ::dds::core::Value<D> {
public:
    TSubscriptionMatchedStatus() : ::dds::core::Value<D>() {}
    TSubscriptionMatchedStatus(int32_t total_count, int32_t total_count_change, int32_t current_count, int32_t current_count_change, const InstanceHandle& last_publication_handle)
        : ::dds::core::Value<D>(total_count, total_count_change, current_count, current_count_change, last_publication_handle) {}

    ZERODDS_TSTATUS_NT(int32_t, total_count)
    ZERODDS_TSTATUS_NT(int32_t, total_count_change)
    ZERODDS_TSTATUS_NT(int32_t, current_count)
    ZERODDS_TSTATUS_NT(int32_t, current_count_change)
    ZERODDS_TSTATUS_CT(InstanceHandle, last_publication_handle)
};

/// The `LivelinessChangedStatus` API over any delegate `D` (DDS-PSM-Cxx 1.0 §7.5.4).
template <typename D>
class TLivelinessChangedStatus : public ::dds::core::Value<D> {
public:
    TLivelinessChangedStatus() : ::dds::core::Value<D>() {}
    TLivelinessChangedStatus(int32_t alive_count, int32_t not_alive_count, int32_t alive_count_change, int32_t not_alive_count_change, const InstanceHandle& last_publication_handle)
        : ::dds::core::Value<D>(alive_count, not_alive_count, alive_count_change, not_alive_count_change, last_publication_handle) {}

    ZERODDS_TSTATUS_NT(int32_t, alive_count)
    ZERODDS_TSTATUS_NT(int32_t, not_alive_count)
    ZERODDS_TSTATUS_NT(int32_t, alive_count_change)
    ZERODDS_TSTATUS_NT(int32_t, not_alive_count_change)
    ZERODDS_TSTATUS_CT(InstanceHandle, last_publication_handle)
};

/// The `OfferedIncompatibleQosStatus` API over any delegate `D` (DDS-PSM-Cxx 1.0 §7.5.4).
template <typename D>
class TOfferedIncompatibleQosStatus : public ::dds::core::Value<D> {
public:
    TOfferedIncompatibleQosStatus() : ::dds::core::Value<D>() {}
    TOfferedIncompatibleQosStatus(int32_t total_count, int32_t total_count_change, policy::QosPolicyId last_policy_id, const policy::QosPolicyCountSeq& policies)
        : ::dds::core::Value<D>(total_count, total_count_change, last_policy_id, policies) {}

    ZERODDS_TSTATUS_INCOMPATIBLE_QOS
};

/// The `RequestedIncompatibleQosStatus` API over any delegate `D` (DDS-PSM-Cxx 1.0 §7.5.4).
template <typename D>
class TRequestedIncompatibleQosStatus : public ::dds::core::Value<D> {
public:
    TRequestedIncompatibleQosStatus() : ::dds::core::Value<D>() {}
    TRequestedIncompatibleQosStatus(int32_t total_count, int32_t total_count_change, policy::QosPolicyId last_policy_id, const policy::QosPolicyCountSeq& policies)
        : ::dds::core::Value<D>(total_count, total_count_change, last_policy_id, policies) {}

    ZERODDS_TSTATUS_INCOMPATIBLE_QOS
};

/// The `SampleRejectedStatus` API over any delegate `D` (DDS-PSM-Cxx 1.0 §7.5.4).
template <typename D>
class TSampleRejectedStatus : public ::dds::core::Value<D> {
public:
    TSampleRejectedStatus() : ::dds::core::Value<D>() {}
    TSampleRejectedStatus(int32_t total_count, int32_t total_count_change, const SampleRejectedState& last_reason, const InstanceHandle& last_instance_handle)
        : ::dds::core::Value<D>(total_count, total_count_change, last_reason, last_instance_handle) {}

    ZERODDS_TSTATUS_NT(int32_t, total_count)
    ZERODDS_TSTATUS_NT(int32_t, total_count_change)
    ZERODDS_TSTATUS_CT(SampleRejectedState, last_reason)
    ZERODDS_TSTATUS_CT(InstanceHandle, last_instance_handle)
};

} // namespace status
} // namespace core
} // namespace dds

#undef ZERODDS_TSTATUS_INCOMPATIBLE_QOS
#undef ZERODDS_TSTATUS_NT
#undef ZERODDS_TSTATUS_CT

#endif // ZERODDS_DDS_CORE_STATUS_TSTATUS_HPP
