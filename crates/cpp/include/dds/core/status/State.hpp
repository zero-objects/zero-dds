// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/status/State.hpp — DDS-PSM-Cxx 1.0 §7.5.4: StatusMask and
// SampleRejectedState as bit sets, with the named masks of the normative
// header.

#ifndef ZERODDS_DDS_CORE_STATUS_STATE_HPP
#define ZERODDS_DDS_CORE_STATUS_STATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <bitset>
#include <stdint.h>

#include "dds/core/macros.hpp"

namespace dds {
namespace core {
namespace status {

/// Why a sample was rejected (DDS 1.4 §2.2.4.1 SampleRejectedStatusKind).
class SampleRejectedState : public std::bitset<OMG_DDS_STATE_BIT_COUNT> {
public:
    typedef std::bitset<OMG_DDS_STATE_BIT_COUNT> MaskType;

    SampleRejectedState() : MaskType() {}
#if ZERODDS_CXX11
    SampleRejectedState(const SampleRejectedState& src) = default;
    SampleRejectedState(SampleRejectedState&&) = default;
#endif
    SampleRejectedState(const MaskType& src) : MaskType(src) {}

    static const SampleRejectedState not_rejected() { return SampleRejectedState(0u); }
    static const SampleRejectedState rejected_by_instances_limit() {
        return SampleRejectedState(1u << 0);
    }
    static const SampleRejectedState rejected_by_samples_limit() {
        return SampleRejectedState(1u << 1);
    }
    static const SampleRejectedState rejected_by_samples_per_instance_limit() {
        return SampleRejectedState(1u << 2);
    }

#if ZERODDS_CXX11
    SampleRejectedState& operator=(const SampleRejectedState&) = default;
#endif

#if ZERODDS_CXX11
    SampleRejectedState& operator=(SampleRejectedState&&) = default;
#endif

private:
    explicit SampleRejectedState(uint32_t s) : MaskType(static_cast<unsigned long long>(s)) {}
};

/// The set of communication statuses (DDS 1.4 §2.2.4.1), one bit each.
class StatusMask : public std::bitset<OMG_DDS_STATUS_COUNT> {
public:
    typedef std::bitset<OMG_DDS_STATUS_COUNT> MaskType;

    StatusMask() {}
    explicit StatusMask(uint64_t mask) : MaskType(mask) {}
#if ZERODDS_CXX11
    StatusMask(const StatusMask& other) = default;
    StatusMask(StatusMask&&) = default;
    StatusMask& operator=(const StatusMask& other) = default;
    StatusMask& operator=(StatusMask&&) = default;
#endif
    ~StatusMask() {}

    static const StatusMask all() { return StatusMask(0x7fe7u); }
    static const StatusMask none() { return StatusMask(0u); }

    static const StatusMask inconsistent_topic() { return StatusMask(1u << 0); }
    static const StatusMask offered_deadline_missed() { return StatusMask(1u << 1); }
    static const StatusMask requested_deadline_missed() { return StatusMask(1u << 2); }
    static const StatusMask offered_incompatible_qos() { return StatusMask(1u << 5); }
    static const StatusMask requested_incompatible_qos() { return StatusMask(1u << 6); }
    static const StatusMask sample_lost() { return StatusMask(1u << 7); }
    static const StatusMask sample_rejected() { return StatusMask(1u << 8); }
    static const StatusMask data_on_readers() { return StatusMask(1u << 9); }
    static const StatusMask data_available() { return StatusMask(1u << 10); }
    static const StatusMask liveliness_lost() { return StatusMask(1u << 11); }
    static const StatusMask liveliness_changed() { return StatusMask(1u << 12); }
    static const StatusMask publication_matched() { return StatusMask(1u << 13); }
    static const StatusMask subscription_matched() { return StatusMask(1u << 14); }
};

} // namespace status
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_STATUS_STATE_HPP
