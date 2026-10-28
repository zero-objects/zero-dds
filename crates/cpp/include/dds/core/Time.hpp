// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/Time.hpp — Time (DDS-PSM-Cxx 1.0 §7.5.6): an absolute time stamp
// in seconds and nanoseconds, with the conversions and the arithmetic of the
// normative header.

#ifndef ZERODDS_DDS_CORE_TIME_HPP
#define ZERODDS_DDS_CORE_TIME_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <limits>
#include <stdexcept>

#include "dds/core/Duration.hpp"

namespace dds {
namespace core {

class Time {
public:
    /// {-1, 0xffffffff}.
    static const Time invalid() { return Time(-1, 0xFFFFFFFFu); }

    static const Time from_microsecs(int64_t microseconds) {
        return from_total(microseconds, 1000000LL, 1000u);
    }
    static const Time from_millisecs(int64_t milliseconds) {
        return from_total(milliseconds, 1000LL, 1000000u);
    }
    static const Time from_secs(double seconds) {
        int64_t secs = static_cast<int64_t>(seconds);
        int64_t nanos = static_cast<int64_t>((seconds - static_cast<double>(secs)) * 1e9);
        if (nanos < 0) {
            nanos += 1000000000LL;
            secs -= 1;
        }
        return Time(secs, static_cast<uint32_t>(nanos));
    }

    Time() : sec_(0), nanosec_(0) {}
    explicit Time(int64_t sec, uint32_t nanosec = 0) : sec_(sec), nanosec_(nanosec) {}

    int64_t sec() const { return sec_; }
    void sec(int64_t s) { sec_ = s; }
    uint32_t nanosec() const { return nanosec_; }
    void nanosec(uint32_t ns) { nanosec_ = ns; }

    int compare(const Time& that) const {
        if (sec_ != that.sec_) return sec_ < that.sec_ ? -1 : 1;
        if (nanosec_ != that.nanosec_) return nanosec_ < that.nanosec_ ? -1 : 1;
        return 0;
    }
    bool operator>(const Time& that) const { return compare(that) > 0; }
    bool operator>=(const Time& that) const { return compare(that) >= 0; }
    bool operator==(const Time& that) const { return compare(that) == 0; }
    bool operator!=(const Time& that) const { return compare(that) != 0; }
    bool operator<=(const Time& that) const { return compare(that) <= 0; }
    bool operator<(const Time& that) const { return compare(that) < 0; }

    Time& operator+=(const Duration& a_ti) {
        int64_t n = static_cast<int64_t>(nanosec_) + a_ti.nanosec();
        sec_ += static_cast<int64_t>(a_ti.sec()) + n / 1000000000LL;
        nanosec_ = static_cast<uint32_t>(n % 1000000000LL);
        return *this;
    }
    Time& operator-=(const Duration& a_ti) {
        int64_t n = static_cast<int64_t>(nanosec_) - a_ti.nanosec();
        int64_t s = sec_ - a_ti.sec();
        if (n < 0) {
            n += 1000000000LL;
            s -= 1;
        }
        sec_ = s;
        nanosec_ = static_cast<uint32_t>(n);
        return *this;
    }

    int64_t to_millisecs() const { return sec_ * 1000 + static_cast<int64_t>(nanosec_ / 1000000u); }
    int64_t to_microsecs() const { return sec_ * 1000000 + static_cast<int64_t>(nanosec_ / 1000u); }
    double to_secs() const { return static_cast<double>(sec_) + static_cast<double>(nanosec_) * 1e-9; }

private:
    static const Time from_total(int64_t total, int64_t per_second, uint32_t nanos_per_unit) {
        int64_t s = total / per_second;
        int64_t rem = total % per_second;
        if (rem < 0) {
            rem += per_second;
            s -= 1;
        }
        return Time(s, static_cast<uint32_t>(rem) * nanos_per_unit);
    }

    int64_t sec_;
    uint32_t nanosec_;
};

inline const Time operator+(const Time& lhs, const Duration& rhs) {
    Time r = lhs;
    r += rhs;
    return r;
}
inline const Time operator+(const Duration& lhs, const Time& rhs) { return rhs + lhs; }
inline const Time operator-(const Time& lhs, const Duration& rhs) {
    Time r = lhs;
    r -= rhs;
    return r;
}

namespace detail {
/// The seconds of `t` as the C API takes them (`int32_t`); a Time past that
/// range is an error, not a wrapped value.
inline int32_t seconds_of(const Time& t) {
    if (t.sec() > std::numeric_limits<int32_t>::max() ||
        t.sec() < std::numeric_limits<int32_t>::min()) {
        throw std::overflow_error("Time: seconds out of the range of the C API (int32)");
    }
    return static_cast<int32_t>(t.sec());
}
} // namespace detail

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_TIME_HPP
