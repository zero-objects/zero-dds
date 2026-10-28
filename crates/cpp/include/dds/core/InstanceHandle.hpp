// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/InstanceHandle.hpp — DDS-PSM-Cxx 1.0 §7.5.4.

#ifndef ZERODDS_DDS_CORE_INSTANCEHANDLE_HPP
#define ZERODDS_DDS_CORE_INSTANCEHANDLE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>

#include "dds/core/TInstanceHandle.hpp"
#include <vector>

namespace dds {
namespace core {

namespace detail {

/// The state of an InstanceHandle: a 64-bit handle, 0 for HANDLE_NIL.
class InstanceHandle {
public:
    InstanceHandle() : value_(0) {}
    /// Constructs from a 64-bit raw handle.
    explicit InstanceHandle(uint64_t v) : value_(v) {}
    /// HANDLE_NIL sentinel.
    static InstanceHandle nil() { return InstanceHandle(0); }
    /// Raw 64-bit value.
    uint64_t value() const { return value_; }
    /// True if HANDLE_NIL.
    bool is_nil() const { return value_ == 0; }

    bool operator==(const InstanceHandle& rhs) const { return value_ == rhs.value_; }
    bool operator!=(const InstanceHandle& rhs) const { return !(*this == rhs); }
    bool operator<(const InstanceHandle& rhs) const { return value_ < rhs.value_; }

private:
    uint64_t value_;
};

} // namespace detail

/// InstanceHandle (DDS-PSM-Cxx 1.0 §7.5.4). Represents a handle to a DDS data instance.
typedef TInstanceHandle<detail::InstanceHandle> InstanceHandle;

/// InstanceHandleSeq (DDS-PSM-Cxx 1.0 §7.5.5).
typedef std::vector<InstanceHandle> InstanceHandleSeq;

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_INSTANCEHANDLE_HPP
