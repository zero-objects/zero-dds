// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/Time.hpp — DDS-PSM-Cxx 1.0 §7.5.2 Time + §7.5.3 Duration.

#ifndef ZERODDS_DDS_CORE_TIME_HPP
#define ZERODDS_DDS_CORE_TIME_HPP

#include <cstdint>
#include <limits>

namespace dds {
namespace core {

/// Time (DDS-PSM-Cxx 1.0 §7.5.2). Represents an absolute timestamp.
class Time {
public:
    /// Default constructs a zero-time.
    Time() = default;
    /// Constructs from seconds + nanoseconds.
    Time(int32_t sec, uint32_t nanosec) : sec_(sec), nanosec_(nanosec) {}
    /// Sentinel "invalid time".
    static Time invalid() { return Time(-1, 0xFFFFFFFFu); }
    /// Seconds component.
    int32_t sec() const { return sec_; }
    /// Nanoseconds component.
    uint32_t nanosec() const { return nanosec_; }
    /// Sets seconds.
    void sec(int32_t v) { sec_ = v; }
    /// Sets nanoseconds.
    void nanosec(uint32_t v) { nanosec_ = v; }
    bool operator==(const Time& rhs) const {
        return sec_ == rhs.sec_ && nanosec_ == rhs.nanosec_;
    }
    bool operator!=(const Time& rhs) const { return !(*this == rhs); }
private:
    int32_t sec_{0};
    uint32_t nanosec_{0};
};

/// Duration (DDS-PSM-Cxx 1.0 §7.5.3). Represents a time interval.
class Duration {
public:
    Duration() = default;
    Duration(int32_t sec, uint32_t nanosec) : sec_(sec), nanosec_(nanosec) {}
    /// DURATION_ZERO.
    static Duration zero() { return Duration(0, 0); }
    /// DURATION_INFINITE.
    static Duration infinite() {
        return Duration(std::numeric_limits<int32_t>::max(), 0xFFFFFFFFu);
    }
    /// From milliseconds.
    static Duration from_millis(int64_t ms) {
        int32_t s = static_cast<int32_t>(ms / 1000);
        int32_t rem = static_cast<int32_t>(ms % 1000);
        if (rem < 0) {
            rem += 1000;
            s -= 1;
        }
        return Duration(s, static_cast<uint32_t>(rem) * 1000000u);
    }
    /// From seconds.
    static Duration from_secs(int32_t s) { return Duration(s, 0); }

    int32_t sec() const { return sec_; }
    uint32_t nanosec() const { return nanosec_; }
    void sec(int32_t v) { sec_ = v; }
    void nanosec(uint32_t v) { nanosec_ = v; }

    bool is_infinite() const {
        return sec_ == std::numeric_limits<int32_t>::max() && nanosec_ == 0xFFFFFFFFu;
    }
    bool operator==(const Duration& rhs) const {
        return sec_ == rhs.sec_ && nanosec_ == rhs.nanosec_;
    }
    bool operator!=(const Duration& rhs) const { return !(*this == rhs); }
private:
    int32_t sec_{0};
    uint32_t nanosec_{0};
};

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_TIME_HPP
