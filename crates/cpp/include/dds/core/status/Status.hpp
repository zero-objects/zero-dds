// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/status/Status.hpp — DDS-PSM-Cxx 1.0 §7.5.4 Status classes. Each
// attribute has a constructor argument and the accessors of §7.4.6: a getter,
// a getter returning a reference to the state, and a setter.

#ifndef ZERODDS_DDS_CORE_STATUS_STATUS_HPP
#define ZERODDS_DDS_CORE_STATUS_STATUS_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <vector>

#include "dds/core/InstanceHandle.hpp"
#include "dds/core/policy/QosPolicyCount.hpp"
#include "dds/core/status/State.hpp"
#include "dds/core/Value.hpp"
#include "dds/core/status/TStatus.hpp"
#include "dds/core/types.hpp"

// §7.4.6 accessors: native attribute NT, constructed attribute CT.
#define ZERODDS_STATUS_NT(NT, name)     \
    NT name() const { return name##_; } \
    NT& name() { return name##_; }      \
    void name(NT value) { name##_ = value; }
#define ZERODDS_STATUS_CT(CT, name)                            \
    const CT& name() const { return name##_; }                 \
    CT& name() { return name##_; }                             \
    void name(const CT& value) { name##_ = value; }

namespace dds {
namespace core {
namespace status {

/// Marker class of the DATA_AVAILABLE status (no state).
class DataAvailableStatus {};
/// Marker class of the DATA_ON_READERS status (no state).
class DataOnReadersStatus {};

namespace detail {

class InconsistentTopicStatus {
public:
    InconsistentTopicStatus() : total_count_(0), total_count_change_(0) {}
    InconsistentTopicStatus(int32_t total_count, int32_t total_count_change)
        : total_count_(total_count),
          total_count_change_(total_count_change) {}

    ZERODDS_STATUS_NT(int32_t, total_count)
    ZERODDS_STATUS_NT(int32_t, total_count_change)

private:
    int32_t total_count_;
    int32_t total_count_change_;
};

class LivelinessLostStatus {
public:
    LivelinessLostStatus() : total_count_(0), total_count_change_(0) {}
    LivelinessLostStatus(int32_t total_count, int32_t total_count_change)
        : total_count_(total_count),
          total_count_change_(total_count_change) {}

    ZERODDS_STATUS_NT(int32_t, total_count)
    ZERODDS_STATUS_NT(int32_t, total_count_change)

private:
    int32_t total_count_;
    int32_t total_count_change_;
};

class SampleLostStatus {
public:
    SampleLostStatus() : total_count_(0), total_count_change_(0) {}
    SampleLostStatus(int32_t total_count, int32_t total_count_change)
        : total_count_(total_count),
          total_count_change_(total_count_change) {}

    ZERODDS_STATUS_NT(int32_t, total_count)
    ZERODDS_STATUS_NT(int32_t, total_count_change)

private:
    int32_t total_count_;
    int32_t total_count_change_;
};

class OfferedDeadlineMissedStatus {
public:
    OfferedDeadlineMissedStatus() : total_count_(0), total_count_change_(0) {}
    OfferedDeadlineMissedStatus(int32_t total_count, int32_t total_count_change, const InstanceHandle& last_instance_handle)
        : total_count_(total_count),
          total_count_change_(total_count_change),
          last_instance_handle_(last_instance_handle) {}

    ZERODDS_STATUS_NT(int32_t, total_count)
    ZERODDS_STATUS_NT(int32_t, total_count_change)
    ZERODDS_STATUS_CT(InstanceHandle, last_instance_handle)

private:
    int32_t total_count_;
    int32_t total_count_change_;
    InstanceHandle last_instance_handle_;
};

class RequestedDeadlineMissedStatus {
public:
    RequestedDeadlineMissedStatus() : total_count_(0), total_count_change_(0) {}
    RequestedDeadlineMissedStatus(int32_t total_count, int32_t total_count_change, const InstanceHandle& last_instance_handle)
        : total_count_(total_count),
          total_count_change_(total_count_change),
          last_instance_handle_(last_instance_handle) {}

    ZERODDS_STATUS_NT(int32_t, total_count)
    ZERODDS_STATUS_NT(int32_t, total_count_change)
    ZERODDS_STATUS_CT(InstanceHandle, last_instance_handle)

private:
    int32_t total_count_;
    int32_t total_count_change_;
    InstanceHandle last_instance_handle_;
};

class PublicationMatchedStatus {
public:
    PublicationMatchedStatus()
        : total_count_(0),
          total_count_change_(0),
          current_count_(0),
          current_count_change_(0) {}
    PublicationMatchedStatus(int32_t total_count, int32_t total_count_change, int32_t current_count, int32_t current_count_change, const InstanceHandle& last_subscription_handle)
        : total_count_(total_count),
          total_count_change_(total_count_change),
          current_count_(current_count),
          current_count_change_(current_count_change),
          last_subscription_handle_(last_subscription_handle) {}

    ZERODDS_STATUS_NT(int32_t, total_count)
    ZERODDS_STATUS_NT(int32_t, total_count_change)
    ZERODDS_STATUS_NT(int32_t, current_count)
    ZERODDS_STATUS_NT(int32_t, current_count_change)
    ZERODDS_STATUS_CT(InstanceHandle, last_subscription_handle)

private:
    int32_t total_count_;
    int32_t total_count_change_;
    int32_t current_count_;
    int32_t current_count_change_;
    InstanceHandle last_subscription_handle_;
};

class SubscriptionMatchedStatus {
public:
    SubscriptionMatchedStatus()
        : total_count_(0),
          total_count_change_(0),
          current_count_(0),
          current_count_change_(0) {}
    SubscriptionMatchedStatus(int32_t total_count, int32_t total_count_change, int32_t current_count, int32_t current_count_change, const InstanceHandle& last_publication_handle)
        : total_count_(total_count),
          total_count_change_(total_count_change),
          current_count_(current_count),
          current_count_change_(current_count_change),
          last_publication_handle_(last_publication_handle) {}

    ZERODDS_STATUS_NT(int32_t, total_count)
    ZERODDS_STATUS_NT(int32_t, total_count_change)
    ZERODDS_STATUS_NT(int32_t, current_count)
    ZERODDS_STATUS_NT(int32_t, current_count_change)
    ZERODDS_STATUS_CT(InstanceHandle, last_publication_handle)

private:
    int32_t total_count_;
    int32_t total_count_change_;
    int32_t current_count_;
    int32_t current_count_change_;
    InstanceHandle last_publication_handle_;
};

class LivelinessChangedStatus {
public:
    LivelinessChangedStatus()
        : alive_count_(0),
          not_alive_count_(0),
          alive_count_change_(0),
          not_alive_count_change_(0) {}
    LivelinessChangedStatus(int32_t alive_count, int32_t not_alive_count, int32_t alive_count_change, int32_t not_alive_count_change, const InstanceHandle& last_publication_handle)
        : alive_count_(alive_count),
          not_alive_count_(not_alive_count),
          alive_count_change_(alive_count_change),
          not_alive_count_change_(not_alive_count_change),
          last_publication_handle_(last_publication_handle) {}

    ZERODDS_STATUS_NT(int32_t, alive_count)
    ZERODDS_STATUS_NT(int32_t, not_alive_count)
    ZERODDS_STATUS_NT(int32_t, alive_count_change)
    ZERODDS_STATUS_NT(int32_t, not_alive_count_change)
    ZERODDS_STATUS_CT(InstanceHandle, last_publication_handle)

private:
    int32_t alive_count_;
    int32_t not_alive_count_;
    int32_t alive_count_change_;
    int32_t not_alive_count_change_;
    InstanceHandle last_publication_handle_;
};

/// The delegate of both OfferedIncompatibleQosStatus and
/// RequestedIncompatibleQosStatus: the two statuses carry the same
/// attributes (DDS 1.4 §2.2.4.1); their public types stay distinct through
/// the two templates of TStatus.hpp.
class IncompatibleQosStatus {
public:
    IncompatibleQosStatus() : total_count_(0), total_count_change_(0), last_policy_id_(0) {}
    IncompatibleQosStatus(int32_t total_count, int32_t total_count_change, policy::QosPolicyId last_policy_id, const policy::QosPolicyCountSeq& policies)
        : total_count_(total_count),
          total_count_change_(total_count_change),
          last_policy_id_(last_policy_id),
          policies_(policies) {}

    ZERODDS_STATUS_NT(int32_t, total_count)
    ZERODDS_STATUS_NT(int32_t, total_count_change)
    ZERODDS_STATUS_NT(policy::QosPolicyId, last_policy_id)
    // The normative header has `policies(dst)` that copies the counts; the
    // sequence is set through the reference accessor.
    const policy::QosPolicyCountSeq& policies() const { return policies_; }
    policy::QosPolicyCountSeq& policies() { return policies_; }
    /// Copies the per-policy counts into `dst` and returns it.
    policy::QosPolicyCountSeq& policies(policy::QosPolicyCountSeq& dst) const {
        dst = policies_;
        return dst;
    }

private:
    int32_t total_count_;
    int32_t total_count_change_;
    policy::QosPolicyId last_policy_id_;
    policy::QosPolicyCountSeq policies_;
};

class SampleRejectedStatus {
public:
    SampleRejectedStatus() : total_count_(0), total_count_change_(0) {}
    SampleRejectedStatus(int32_t total_count, int32_t total_count_change, const SampleRejectedState& last_reason, const InstanceHandle& last_instance_handle)
        : total_count_(total_count),
          total_count_change_(total_count_change),
          last_reason_(last_reason),
          last_instance_handle_(last_instance_handle) {}

    ZERODDS_STATUS_NT(int32_t, total_count)
    ZERODDS_STATUS_NT(int32_t, total_count_change)
    ZERODDS_STATUS_CT(SampleRejectedState, last_reason)
    ZERODDS_STATUS_CT(InstanceHandle, last_instance_handle)

private:
    int32_t total_count_;
    int32_t total_count_change_;
    SampleRejectedState last_reason_;
    InstanceHandle last_instance_handle_;
};

} // namespace detail

typedef TInconsistentTopicStatus<detail::InconsistentTopicStatus> InconsistentTopicStatus;
typedef TLivelinessLostStatus<detail::LivelinessLostStatus> LivelinessLostStatus;
typedef TSampleLostStatus<detail::SampleLostStatus> SampleLostStatus;
typedef TOfferedDeadlineMissedStatus<detail::OfferedDeadlineMissedStatus> OfferedDeadlineMissedStatus;
typedef TRequestedDeadlineMissedStatus<detail::RequestedDeadlineMissedStatus> RequestedDeadlineMissedStatus;
typedef TPublicationMatchedStatus<detail::PublicationMatchedStatus> PublicationMatchedStatus;
typedef TSubscriptionMatchedStatus<detail::SubscriptionMatchedStatus> SubscriptionMatchedStatus;
typedef TLivelinessChangedStatus<detail::LivelinessChangedStatus> LivelinessChangedStatus;
typedef TOfferedIncompatibleQosStatus<detail::IncompatibleQosStatus> OfferedIncompatibleQosStatus;
typedef TRequestedIncompatibleQosStatus<detail::IncompatibleQosStatus> RequestedIncompatibleQosStatus;
typedef TSampleRejectedStatus<detail::SampleRejectedStatus> SampleRejectedStatus;

// The state associated with each status (DDS-PSM-Cxx 1.0, dds/core/status/Status.hpp).
template <typename STATUS>
StatusMask get_status();

#define ZERODDS_GET_STATUS(STATUS, MASK) \
    template <>                          \
    inline StatusMask get_status<STATUS>() { return StatusMask::MASK(); }
ZERODDS_GET_STATUS(InconsistentTopicStatus, inconsistent_topic)
ZERODDS_GET_STATUS(OfferedDeadlineMissedStatus, offered_deadline_missed)
ZERODDS_GET_STATUS(RequestedDeadlineMissedStatus, requested_deadline_missed)
ZERODDS_GET_STATUS(OfferedIncompatibleQosStatus, offered_incompatible_qos)
ZERODDS_GET_STATUS(RequestedIncompatibleQosStatus, requested_incompatible_qos)
ZERODDS_GET_STATUS(SampleLostStatus, sample_lost)
ZERODDS_GET_STATUS(SampleRejectedStatus, sample_rejected)
ZERODDS_GET_STATUS(DataOnReadersStatus, data_on_readers)
ZERODDS_GET_STATUS(DataAvailableStatus, data_available)
ZERODDS_GET_STATUS(LivelinessLostStatus, liveliness_lost)
ZERODDS_GET_STATUS(LivelinessChangedStatus, liveliness_changed)
ZERODDS_GET_STATUS(PublicationMatchedStatus, publication_matched)
ZERODDS_GET_STATUS(SubscriptionMatchedStatus, subscription_matched)
#undef ZERODDS_GET_STATUS

} // namespace status
} // namespace core
} // namespace dds

#undef ZERODDS_STATUS_NT
#undef ZERODDS_STATUS_CT

#endif // ZERODDS_DDS_CORE_STATUS_STATUS_HPP
