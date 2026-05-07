// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/InstanceHandle.hpp — DDS-PSM-Cxx 1.0 §7.5.4.

#ifndef ZERODDS_DDS_CORE_INSTANCEHANDLE_HPP
#define ZERODDS_DDS_CORE_INSTANCEHANDLE_HPP

#include <cstdint>
#include <vector>

namespace dds {
namespace core {

/// InstanceHandle (DDS-PSM-Cxx 1.0 §7.5.4). Represents a handle to a
/// DDS data instance.
class InstanceHandle {
public:
    InstanceHandle() = default;
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
    uint64_t value_{0};
};

/// InstanceHandleSeq (DDS-PSM-Cxx 1.0 §7.5.5).
typedef std::vector<InstanceHandle> InstanceHandleSeq;

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_INSTANCEHANDLE_HPP
