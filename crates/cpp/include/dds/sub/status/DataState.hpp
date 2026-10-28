// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/status/DataState.hpp — DDS-PSM-Cxx 1.0 §7.5.4: SampleState,
// ViewState, InstanceState and the DataState that combines them. The bits are
// the values of DDS 1.4 §2.2.2.5.1 (the values the C API takes and returns).

#ifndef ZERODDS_DDS_SUB_STATUS_DATASTATE_HPP
#define ZERODDS_DDS_SUB_STATUS_DATASTATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <bitset>
#include <stdint.h>

#include "dds/core/status/State.hpp"

namespace dds {
namespace sub {
namespace status {

/// Whether the sample was read before (DDS 1.4 SampleStateKind).
class SampleState : public std::bitset<OMG_DDS_STATE_BIT_COUNT> {
public:
    typedef std::bitset<OMG_DDS_STATE_BIT_COUNT> MaskType;

    SampleState() : MaskType() {}
    explicit SampleState(uint32_t i) : MaskType(i) {}
#if ZERODDS_CXX11
    SampleState(const SampleState& src) = default;
    SampleState(SampleState&&) = default;
#endif
    SampleState(const MaskType& src) : MaskType(src) {}
#if ZERODDS_CXX11
    SampleState& operator=(const SampleState&) = default;
    SampleState& operator=(SampleState&&) = default;
#endif

    static const SampleState read() { return SampleState(1u); }
    static const SampleState not_read() { return SampleState(2u); }
    static const SampleState any() { return SampleState(3u); }
};

/// Whether the instance is new to the reader (DDS 1.4 ViewStateKind).
class ViewState : public std::bitset<OMG_DDS_STATE_BIT_COUNT> {
public:
    typedef std::bitset<OMG_DDS_STATE_BIT_COUNT> MaskType;

    ViewState() : MaskType() {}
    explicit ViewState(uint32_t m) : MaskType(m) {}
#if ZERODDS_CXX11
    ViewState(const ViewState& src) = default;
    ViewState(ViewState&&) = default;
#endif
    ViewState(const MaskType& src) : MaskType(src) {}
#if ZERODDS_CXX11
    ViewState& operator=(const ViewState&) = default;
    ViewState& operator=(ViewState&&) = default;
#endif

    static const ViewState new_view() { return ViewState(1u); }
    static const ViewState not_new_view() { return ViewState(2u); }
    static const ViewState any() { return ViewState(3u); }
};

/// Whether the instance is alive (DDS 1.4 InstanceStateKind).
class InstanceState : public std::bitset<OMG_DDS_STATE_BIT_COUNT> {
public:
    typedef std::bitset<OMG_DDS_STATE_BIT_COUNT> MaskType;

    InstanceState() : MaskType() {}
    explicit InstanceState(uint32_t m) : MaskType(m) {}
#if ZERODDS_CXX11
    InstanceState(const InstanceState& src) = default;
    InstanceState(InstanceState&&) = default;
#endif
    InstanceState(const MaskType& src) : MaskType(src) {}
#if ZERODDS_CXX11
    InstanceState& operator=(const InstanceState&) = default;
    InstanceState& operator=(InstanceState&&) = default;
#endif

    static const InstanceState alive() { return InstanceState(1u); }
    static const InstanceState not_alive_disposed() { return InstanceState(2u); }
    static const InstanceState not_alive_no_writers() { return InstanceState(4u); }
    static const InstanceState not_alive_mask() { return InstanceState(6u); }
    static const InstanceState any() { return InstanceState(7u); }
};

/// The sample, view and instance state of a sample or of a filter.
class DataState {
public:
    DataState()
        : ss_(SampleState::any()), vs_(ViewState::any()), is_(InstanceState::any()) {}
    /* implicit */ DataState(const SampleState& ss)
        : ss_(ss), vs_(ViewState::any()), is_(InstanceState::any()) {}
    /* implicit */ DataState(const ViewState& vs)
        : ss_(SampleState::any()), vs_(vs), is_(InstanceState::any()) {}
    /* implicit */ DataState(const InstanceState& is)
        : ss_(SampleState::any()), vs_(ViewState::any()), is_(is) {}
    DataState(const SampleState& ss, const ViewState& vs, const InstanceState& is)
        : ss_(ss), vs_(vs), is_(is) {}

    DataState& operator<<(const SampleState& ss) {
        ss_ = ss;
        return *this;
    }
    DataState& operator<<(const InstanceState& is) {
        is_ = is;
        return *this;
    }
    DataState& operator<<(const ViewState& vs) {
        vs_ = vs;
        return *this;
    }
    const DataState& operator>>(SampleState& ss) const {
        ss = ss_;
        return *this;
    }
    const DataState& operator>>(InstanceState& is) const {
        is = is_;
        return *this;
    }
    const DataState& operator>>(ViewState& vs) const {
        vs = vs_;
        return *this;
    }

    const SampleState& sample_state() const { return ss_; }
    void sample_state(const SampleState& ss) { *this << ss; }
    const InstanceState& instance_state() const { return is_; }
    void instance_state(const InstanceState& is) { *this << is; }
    const ViewState& view_state() const { return vs_; }
    void view_state(const ViewState& vs) { *this << vs; }

    static DataState any() {
        return DataState(SampleState::any(), ViewState::any(), InstanceState::any());
    }
    static DataState new_data() {
        return DataState(SampleState::not_read(), ViewState::any(), InstanceState::alive());
    }
    static DataState any_data() {
        return DataState(SampleState::any(), ViewState::any(), InstanceState::alive());
    }
    static DataState new_instance() {
        return DataState(SampleState::any(), ViewState::new_view(), InstanceState::alive());
    }

    bool operator==(const DataState& other) const {
        return ss_ == other.ss_ && vs_ == other.vs_ && is_ == other.is_;
    }
    bool operator!=(const DataState& other) const { return !(*this == other); }

private:
    SampleState ss_;
    ViewState vs_;
    InstanceState is_;
};

} // namespace status
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_STATUS_DATASTATE_HPP
