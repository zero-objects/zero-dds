// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/Duration.hpp — Duration (DDS-PSM-Cxx 1.0 §7.5.6): a time
// interval in seconds and nanoseconds, with the conversions and the
// arithmetic of the normative header.

#ifndef ZERODDS_DDS_CORE_DURATION_HPP
#define ZERODDS_DDS_CORE_DURATION_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <limits>
#include <stdexcept>

namespace dds {
namespace core {

class Duration {
public:
    /// {0, 0}.
    static const Duration zero() { return Duration(0, 0); }
    /// DURATION_INFINITE (DDS 1.2 IDL §9.3.2: seconds and nanoseconds are
    /// both 0x7fffffff).
    static const Duration infinite() {
        return Duration(std::numeric_limits<int32_t>::max(), 0x7FFFFFFFu);
    }

    Duration() : sec_(0), nanosec_(0) {}
    explicit Duration(int32_t sec, uint32_t nanosec = 0) : sec_(sec), nanosec_(nanosec) {}
#if ZERODDS_CXX11
    ~Duration() = default;
#endif

    /// Throws `std::overflow_error` when the seconds do not fit an `int32_t`
    /// instead of wrapping.
    static const Duration from_microsecs(int64_t microseconds) {
        return from_total(microseconds, 1000000LL, 1000u);
    }
    static const Duration from_millisecs(int64_t milliseconds) {
        return from_total(milliseconds, 1000LL, 1000000u);
    }
    static const Duration from_secs(double seconds) {
        const double whole = static_cast<double>(static_cast<int64_t>(seconds));
        double fraction = seconds - whole;
        int64_t secs = static_cast<int64_t>(whole);
        int64_t nanos = static_cast<int64_t>(fraction * 1e9);
        if (nanos < 0) {
            nanos += 1000000000LL;
            secs -= 1;
        }
        return make(secs, static_cast<uint32_t>(nanos));
    }

    int32_t sec() const { return sec_; }
    void sec(int32_t s) { sec_ = s; }
    uint32_t nanosec() const { return nanosec_; }
    /// Sets nanoseconds, normalising a value of one second or more into the
    /// seconds component so `nanosec()` always answers in [0, 999999999].
    void nanosec(uint32_t ns) {
        sec_ += static_cast<int32_t>(ns / 1000000000u);
        nanosec_ = ns % 1000000000u;
    }

    /// Negative, zero or positive as `*this` is less than, equal to or
    /// greater than `that`; the infinite duration is greater than any other.
    int compare(const Duration& that) const {
        const bool a = infinite_(*this);
        const bool b = infinite_(that);
        if (a || b) {
            return a == b ? 0 : (a ? 1 : -1);
        }
        if (sec_ != that.sec_) return sec_ < that.sec_ ? -1 : 1;
        if (nanosec_ != that.nanosec_) return nanosec_ < that.nanosec_ ? -1 : 1;
        return 0;
    }
    bool operator>(const Duration& that) const { return compare(that) > 0; }
    bool operator>=(const Duration& that) const { return compare(that) >= 0; }
    bool operator==(const Duration& that) const { return compare(that) == 0; }
    bool operator!=(const Duration& that) const { return compare(that) != 0; }
    bool operator<=(const Duration& that) const { return compare(that) <= 0; }
    bool operator<(const Duration& that) const { return compare(that) < 0; }

    /// An infinite operand makes the sum infinite; a sum or difference
    /// outside the range of `int32_t` seconds saturates.
    Duration& operator+=(const Duration& a_ti) {
        if (infinite_(*this) || infinite_(a_ti)) {
            *this = infinite();
            return *this;
        }
        *this = make(static_cast<int64_t>(sec_) + a_ti.sec_, 0, nanosec_ + a_ti.nanosec_);
        return *this;
    }
    Duration& operator-=(const Duration& a_ti) {
        if (infinite_(*this)) {
            return *this;
        }
        if (infinite_(a_ti)) {
            *this = Duration(std::numeric_limits<int32_t>::min(), 0);
            return *this;
        }
        int64_t s = static_cast<int64_t>(sec_) - a_ti.sec_;
        int64_t n = static_cast<int64_t>(nanosec_) - static_cast<int64_t>(a_ti.nanosec_);
        if (n < 0) {
            n += 1000000000LL;
            s -= 1;
        }
        *this = make(s, static_cast<uint32_t>(n));
        return *this;
    }
    const Duration operator+(const Duration& other) const {
        Duration r = *this;
        r += other;
        return r;
    }
    const Duration operator-(const Duration& other) const {
        Duration r = *this;
        r -= other;
        return r;
    }

    /// Not meaningful for `infinite()`.
    int64_t to_millisecs() const {
        return static_cast<int64_t>(sec_) * 1000 + static_cast<int64_t>(nanosec_ / 1000000u);
    }
    int64_t to_microsecs() const {
        return static_cast<int64_t>(sec_) * 1000000 + static_cast<int64_t>(nanosec_ / 1000u);
    }
    double to_secs() const { return static_cast<double>(sec_) + static_cast<double>(nanosec_) * 1e-9; }

private:
    /// True for the DDS 1.2 infinite value {0x7fffffff, 0x7fffffff}; the C
    /// API's {0x7fffffff, 0xffffffff} is infinite too.
    static bool infinite_(const Duration& d) {
        return d.sec_ == std::numeric_limits<int32_t>::max() &&
               (d.nanosec_ == 0x7FFFFFFFu || d.nanosec_ == 0xFFFFFFFFu);
    }

    static const Duration from_total(int64_t total, int64_t per_second, uint32_t nanos_per_unit) {
        int64_t s = total / per_second;
        int64_t rem = total % per_second;
        if (rem < 0) {
            rem += per_second;
            s -= 1;
        }
        return make(s, static_cast<uint32_t>(rem) * nanos_per_unit);
    }

    static Duration make(int64_t sec, uint32_t nanosec, uint32_t extra_nanosec = 0) {
        int64_t n = static_cast<int64_t>(nanosec) + extra_nanosec;
        sec += n / 1000000000LL;
        n %= 1000000000LL;
        if (sec > std::numeric_limits<int32_t>::max() || sec < std::numeric_limits<int32_t>::min()) {
            throw std::overflow_error("Duration: seconds out of the range of int32");
        }
        return Duration(static_cast<int32_t>(sec), static_cast<uint32_t>(n));
    }

    int32_t sec_;
    uint32_t nanosec_;
};

const Duration operator*(uint32_t lhs, const Duration& rhs);
const Duration operator*(const Duration& lhs, uint32_t rhs);
const Duration operator/(const Duration& lhs, uint32_t rhs);

inline const Duration operator*(const Duration& lhs, uint32_t rhs) {
    const int64_t total_nanos =
        (static_cast<int64_t>(lhs.sec()) * 1000000000LL + static_cast<int64_t>(lhs.nanosec())) *
        static_cast<int64_t>(rhs);
    const int64_t s = total_nanos / 1000000000LL;
    if (s > std::numeric_limits<int32_t>::max() || s < std::numeric_limits<int32_t>::min()) {
        throw std::overflow_error("Duration: product out of the range of int32 seconds");
    }
    return Duration(static_cast<int32_t>(s), static_cast<uint32_t>(total_nanos % 1000000000LL));
}
inline const Duration operator*(uint32_t lhs, const Duration& rhs) { return rhs * lhs; }
inline const Duration operator/(const Duration& lhs, uint32_t rhs) {
    if (rhs == 0) {
        throw std::invalid_argument("Duration: division by zero");
    }
    const int64_t total_nanos =
        (static_cast<int64_t>(lhs.sec()) * 1000000000LL + static_cast<int64_t>(lhs.nanosec())) /
        static_cast<int64_t>(rhs);
    return Duration(static_cast<int32_t>(total_nanos / 1000000000LL),
                    static_cast<uint32_t>(total_nanos % 1000000000LL));
}

namespace detail {
/// Whether `d` is the infinite duration, in either spelling (DDS 1.2
/// {0x7fffffff, 0x7fffffff}, the C API's {0x7fffffff, 0xffffffff}).
inline bool is_infinite(const Duration& d) { return d == Duration::infinite(); }
} // namespace detail

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_DURATION_HPP
