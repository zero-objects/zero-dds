// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `Time_t` and `Duration_t` (DDS-DCPS 1.4 §2.3.3 IDL PSM).
//!
//! The spec defines:
//! * `struct Time_t { long sec; unsigned long nanosec; }`
//! * `struct Duration_t { long sec; unsigned long nanosec; }`
//! * Reserved sentinels: `TIME_INVALID`, `TIME_INFINITE`,
//!   `DURATION_ZERO`, `DURATION_INFINITE`.
//!
//! The wire form is 8 bytes (4 sec + 4 nanosec). We wrap it here with a
//! Rust API that coexists with `core::time::Duration` and
//! `std::time::SystemTime`. The RTPS wire layer has its own `Duration` in
//! `zerodds_rtps::participant_data::Duration` — we convert as needed.

extern crate alloc;

/// Nanoseconds in a second; DDS 1.4 §2.3.2 bounds `nanosec` below it.
const NANOS_PER_SEC: u32 = 1_000_000_000;

/// `Time_t` — wall-clock timestamp since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Time {
    /// Seconds since 1970-01-01 UTC.
    pub sec: i32,
    /// Nanoseconds component [0, 999_999_999].
    pub nanosec: u32,
}

impl Time {
    /// `TIME_ZERO` — spec constant (DDSI-RTPS §8.3.2 + DCPS §2.3.3).
    pub const ZERO: Self = Self { sec: 0, nanosec: 0 };

    /// Reserved sentinel "invalid time" (spec convention:
    /// sec=-1, nanosec=0xFFFF_FFFF).
    pub const INVALID: Self = Self {
        sec: -1,
        nanosec: 0xFFFF_FFFF,
    };

    /// Reserved sentinel "infinitely far in the future".
    pub const INFINITE: Self = Self {
        sec: 0x7FFF_FFFF,
        nanosec: 0xFFFF_FFFE,
    };

    /// `true` if the value equals the [`Self::ZERO`] sentinel.
    #[must_use]
    pub const fn is_zero(&self) -> bool {
        self.sec == 0 && self.nanosec == 0
    }

    /// Constructor. DDS 1.4 §2.3.2: "the nanosec field must verify
    /// 0 <= nanosec < 1000000000", so a `nanosec` of a second or more
    /// carries into `sec` (saturating); the reserved [`Self::INVALID`] and
    /// [`Self::INFINITE`] values are kept as they are.
    #[must_use]
    pub const fn new(sec: i32, nanosec: u32) -> Self {
        let reserved =
            (sec == -1 && nanosec == 0xFFFF_FFFF) || (sec == 0x7FFF_FFFF && nanosec == 0xFFFF_FFFE);
        if reserved || nanosec < NANOS_PER_SEC {
            return Self { sec, nanosec };
        }
        Self {
            sec: sec.saturating_add((nanosec / NANOS_PER_SEC) as i32),
            nanosec: nanosec % NANOS_PER_SEC,
        }
    }

    /// `true` if `nanosec` is in `[0, 1000000000)` (DDS 1.4 §2.3.2) or the
    /// value is the reserved [`Self::INVALID`] or [`Self::INFINITE`]. The
    /// fields are public, so a value built by hand may not be.
    #[must_use]
    pub const fn is_normalized(&self) -> bool {
        self.nanosec < NANOS_PER_SEC || self.is_invalid() || self.is_infinite()
    }

    /// `true` if the value equals the [`Self::INVALID`] sentinel.
    #[must_use]
    pub const fn is_invalid(&self) -> bool {
        self.sec == -1 && self.nanosec == 0xFFFF_FFFF
    }

    /// `true` if the value equals the [`Self::INFINITE`] sentinel.
    #[must_use]
    pub const fn is_infinite(&self) -> bool {
        self.sec == 0x7FFF_FFFF && self.nanosec == 0xFFFF_FFFE
    }

    /// Spec §7.5.6.1 — `seconds`-Accessor (DDS-PSM-Cxx Time::seconds()).
    #[must_use]
    pub const fn seconds(&self) -> i32 {
        self.sec
    }

    /// Spec §7.5.6.1 — `nanoseconds`-Accessor.
    #[must_use]
    pub const fn nanoseconds(&self) -> u32 {
        self.nanosec
    }

    /// Spec §7.5.6.2 — Time + Duration (second increment).
    #[must_use]
    pub fn add_duration(self, d: Duration) -> Self {
        let total_ns = u64::from(self.nanosec) + u64::from(d.nanosec);
        let extra_sec = (total_ns / 1_000_000_000) as i32;
        let nanosec = (total_ns % 1_000_000_000) as u32;
        Self {
            sec: self.sec.saturating_add(d.sec).saturating_add(extra_sec),
            nanosec,
        }
    }

    /// Spec §7.5.6.3 — Time from a millisecond integer.
    #[must_use]
    pub const fn from_millis(ms: i64) -> Self {
        // Floor division: a negative `ms` keeps nanosec in [0, 1e9)
        // (DDS 1.4 §2.3.2) and the value is `sec + nanosec / 1e9`.
        let sec = ms.div_euclid(1000) as i32;
        let nanosec = (ms.rem_euclid(1000) * 1_000_000) as u32;
        Self { sec, nanosec }
    }

    /// Spec §7.5.6.3 — Time as a millisecond integer.
    #[must_use]
    pub const fn as_millis(&self) -> i64 {
        (self.sec as i64) * 1000 + (self.nanosec as i64) / 1_000_000
    }
}

/// `Duration_t` — a relative time span (can be negative when
/// `sec < 0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Duration {
    /// Seconds.
    pub sec: i32,
    /// Nanoseconds-Anteil [0, 999_999_999].
    pub nanosec: u32,
}

/// DDS 1.4 §2.3.2: the durations of the QoS policies are
/// [`zerodds_qos::Duration`], the form of the RTPS announcement (`seconds`
/// and a `fraction` of 2^-32 s); `Duration_t` of the specification has `sec`
/// and `nanosec`. `DURATION_INFINITE` maps to `DURATION_INFINITE` (§2.3.3).
impl From<zerodds_qos::Duration> for Duration {
    fn from(d: zerodds_qos::Duration) -> Self {
        if d.is_infinite() {
            return Self::INFINITE;
        }
        let (sec, nanosec) = d.to_sec_nanosec();
        Self { sec, nanosec }
    }
}

/// The inverse of `From<zerodds_qos::Duration>`.
impl From<Duration> for zerodds_qos::Duration {
    fn from(d: Duration) -> Self {
        if d.is_infinite() {
            return Self::INFINITE;
        }
        Self::from_sec_nanosec(d.sec, d.nanosec)
    }
}

impl Duration {
    /// `DURATION_ZERO` (spec): 0 seconds.
    pub const ZERO: Self = Self { sec: 0, nanosec: 0 };

    /// `DURATION_INFINITE` (DDS 1.4 §2.3.3: `DURATION_INFINITE_SEC` and
    /// `DURATION_INFINITE_NSEC`, both `0x7fffffff`): symbolic maximum.
    pub const INFINITE: Self = Self {
        sec: 0x7FFF_FFFF,
        nanosec: 0x7FFF_FFFF,
    };

    /// Constructor. DDS 1.4 §2.3.2: "the nanosec field must verify
    /// 0 <= nanosec < 1000000000", so a `nanosec` of a second or more
    /// carries into `sec` (saturating); [`Self::INFINITE`] is kept as it is.
    #[must_use]
    pub const fn new(sec: i32, nanosec: u32) -> Self {
        if (sec == 0x7FFF_FFFF && nanosec == 0x7FFF_FFFF) || nanosec < NANOS_PER_SEC {
            return Self { sec, nanosec };
        }
        Self {
            sec: sec.saturating_add((nanosec / NANOS_PER_SEC) as i32),
            nanosec: nanosec % NANOS_PER_SEC,
        }
    }

    /// `true` if `nanosec` is in `[0, 1000000000)` (DDS 1.4 §2.3.2) or the
    /// value is [`Self::INFINITE`]. The fields are public, so a value built
    /// by hand may not be.
    #[must_use]
    pub const fn is_normalized(&self) -> bool {
        self.nanosec < NANOS_PER_SEC || self.is_infinite()
    }

    /// `true` if the value is [`Self::INFINITE`].
    #[must_use]
    pub const fn is_infinite(&self) -> bool {
        self.sec == 0x7FFF_FFFF && self.nanosec == 0x7FFF_FFFF
    }

    /// `true` if the value is [`Self::ZERO`].
    #[must_use]
    pub const fn is_zero(&self) -> bool {
        self.sec == 0 && self.nanosec == 0
    }

    /// Conversion into [`core::time::Duration`]. `INFINITE` maps to
    /// `Duration::MAX`; negative seconds map to `ZERO`
    /// (core::time::Duration is unsigned).
    #[must_use]
    pub fn to_core(self) -> core::time::Duration {
        if self.is_infinite() {
            core::time::Duration::MAX
        } else if self.sec < 0 {
            core::time::Duration::ZERO
        } else {
            core::time::Duration::new(self.sec as u64, self.nanosec)
        }
    }

    /// Conversion from [`core::time::Duration`] (lossy, clamps seconds
    /// to `i32::MAX`).
    #[must_use]
    pub fn from_core(d: core::time::Duration) -> Self {
        let sec = i32::try_from(d.as_secs()).unwrap_or(i32::MAX);
        Self {
            sec,
            nanosec: d.subsec_nanos(),
        }
    }

    /// Spec §7.5.6.1 — `seconds`-Accessor (DDS-PSM-Cxx Duration::seconds()).
    #[must_use]
    pub const fn seconds(&self) -> i32 {
        self.sec
    }

    /// Spec §7.5.6.1 — `nanoseconds`-Accessor.
    #[must_use]
    pub const fn nanoseconds(&self) -> u32 {
        self.nanosec
    }

    /// Spec §7.5.6.4 — Duration + Duration (increment).
    #[must_use]
    pub fn add_duration(self, d: Duration) -> Self {
        let total_ns = u64::from(self.nanosec) + u64::from(d.nanosec);
        let extra_sec = (total_ns / 1_000_000_000) as i32;
        let nanosec = (total_ns % 1_000_000_000) as u32;
        Self {
            sec: self.sec.saturating_add(d.sec).saturating_add(extra_sec),
            nanosec,
        }
    }

    /// Spec §7.5.6.5 — Duration from a millisecond integer.
    #[must_use]
    pub const fn from_millis(ms: i64) -> Self {
        // Floor division: a negative `ms` keeps nanosec in [0, 1e9)
        // (DDS 1.4 §2.3.2) and the value is `sec + nanosec / 1e9`.
        let sec = ms.div_euclid(1000) as i32;
        let nanosec = (ms.rem_euclid(1000) * 1_000_000) as u32;
        Self { sec, nanosec }
    }

    /// Spec §7.5.6.5 — Duration as a millisecond integer.
    #[must_use]
    pub const fn as_millis(&self) -> i64 {
        (self.sec as i64) * 1000 + (self.nanosec as i64) / 1_000_000
    }
}

/// Current wall-clock time as a [`Time`]. On `std` platforms via
/// `std::time::SystemTime`. On `no_std` (no std feature) the function
/// returns `Time::INVALID` — the caller must provide the source itself
/// (e.g. a monotonic hardware clock).
///
/// Spec reference: DDS-DCPS 1.4 §2.2.2.2.1.32 `get_current_time`.
#[must_use]
#[cfg(feature = "std")]
pub fn get_current_time() -> Time {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| Time {
            sec: i32::try_from(d.as_secs()).unwrap_or(i32::MAX),
            nanosec: d.subsec_nanos(),
        })
        .unwrap_or(Time::INVALID)
}

/// `no_std` stub for [`get_current_time`].
#[must_use]
#[cfg(not(feature = "std"))]
pub fn get_current_time() -> Time {
    Time::INVALID
}

/// Converts a DCPS [`Time`] (sec + nanosec) into the RTPS wire `Time_t`
/// carried by an INFO_TS submessage (DDSI-RTPS §9.3.2.1): a 32-bit second
/// count plus a 32-bit binary fraction in units of 2⁻³² s. Round-tripped by
/// [`he_timestamp_to_time`]; both use the spec fraction so the value is
/// interoperable with Cyclone / FastDDS / RTI.
#[must_use]
pub fn time_to_he_timestamp(t: Time) -> zerodds_rtps::header_extension::HeTimestamp {
    // DDSI-RTPS 2.5 §9.3.2.1: TIME_INVALID is {0xffffffff, 0xffffffff} and
    // TIME_INFINITE {0xffffffff, 0xfffffffe}.
    if t.is_invalid() {
        return zerodds_rtps::header_extension::HeTimestamp::TIME_INVALID;
    }
    if t.is_infinite() {
        return zerodds_rtps::header_extension::HeTimestamp::TIME_INFINITE;
    }
    // fraction = round(nanosec * 2^32 / 1e9); it was cut, and a written
    // timestamp came back one nanosecond early.
    let d = zerodds_qos::Duration::from_sec_nanosec(t.sec, t.nanosec);
    // The DCPS `Time` counts seconds as a `long`; the octets are the same.
    zerodds_rtps::header_extension::HeTimestamp {
        // The seconds are a `u32` on the wire, a `long` in `Duration`: the
        // same 32 bits.
        seconds: u32::from_ne_bytes(d.seconds.to_ne_bytes()),
        fraction: d.fraction,
    }
}

/// Inverse of [`time_to_he_timestamp`]: an RTPS `Time_t` from a received
/// INFO_TS back into a DCPS [`Time`] for `SampleInfo.source_timestamp`.
#[must_use]
pub fn he_timestamp_to_time(ts: zerodds_rtps::header_extension::HeTimestamp) -> Time {
    // §9.3.2.1: the two reserved values of `Time_t` are no point in time.
    if ts == zerodds_rtps::header_extension::HeTimestamp::TIME_INVALID {
        return Time::INVALID;
    }
    if ts == zerodds_rtps::header_extension::HeTimestamp::TIME_INFINITE {
        return Time::INFINITE;
    }
    // nanosec = round(fraction * 1e9 / 2^32), a whole second carrying.
    let (sec, nanosec) = zerodds_qos::Duration {
        seconds: i32::from_ne_bytes(ts.seconds.to_ne_bytes()),
        fraction: ts.fraction,
    }
    .to_sec_nanosec();
    Time { sec, nanosec }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// DDSI-RTPS 2.5 §9.3.2.1: "TIME_INVALID: seconds = 0xffffffff, fraction =
    /// 0xffffffff; TIME_INFINITE: seconds = 0xffffffff, fraction =
    /// 0xfffffffe". The two sentinels cross the wire as those octets and come
    /// back as the sentinels.
    #[test]
    fn the_reserved_time_t_values_cross_the_wire_as_the_spec_gives_them() {
        let invalid = time_to_he_timestamp(Time::INVALID);
        assert_eq!(
            (invalid.seconds, invalid.fraction),
            (0xFFFF_FFFF, 0xFFFF_FFFF)
        );
        let infinite = time_to_he_timestamp(Time::INFINITE);
        assert_eq!(
            (infinite.seconds, infinite.fraction),
            (0xFFFF_FFFF, 0xFFFF_FFFE)
        );
        assert_eq!(he_timestamp_to_time(invalid), Time::INVALID);
        assert_eq!(he_timestamp_to_time(infinite), Time::INFINITE);
        // An ordinary time is not touched.
        let t = Time::new(1_700_000_000, 500_000_000);
        assert_eq!(he_timestamp_to_time(time_to_he_timestamp(t)), t);
    }

    #[test]
    fn time_zero_sentinel() {
        assert!(Time::ZERO.is_zero());
        assert!(!Time::ZERO.is_invalid());
        assert!(!Time::ZERO.is_infinite());
        assert_eq!(Time::ZERO, Time::default());
    }

    #[test]
    fn time_invalid_sentinel() {
        assert!(Time::INVALID.is_invalid());
        assert!(!Time::INVALID.is_infinite());
        let zero = Time::default();
        assert!(!zero.is_invalid());
    }

    #[test]
    fn time_infinite_sentinel() {
        assert!(Time::INFINITE.is_infinite());
        assert!(!Time::INFINITE.is_invalid());
    }

    #[test]
    fn duration_zero_and_infinite_sentinels() {
        assert!(Duration::ZERO.is_zero());
        assert!(Duration::INFINITE.is_infinite());
        assert!(!Duration::INFINITE.is_zero());
    }

    #[test]
    fn duration_to_core_maps_infinite_to_max() {
        assert_eq!(Duration::INFINITE.to_core(), core::time::Duration::MAX);
    }

    #[test]
    fn duration_to_core_preserves_value() {
        let d = Duration::new(5, 500_000_000);
        let c = d.to_core();
        assert_eq!(c.as_secs(), 5);
        assert_eq!(c.subsec_nanos(), 500_000_000);
    }

    #[test]
    fn duration_from_core_roundtrip() {
        let c = core::time::Duration::new(42, 123_456_789);
        let d = Duration::from_core(c);
        assert_eq!(d.sec, 42);
        assert_eq!(d.nanosec, 123_456_789);
        assert_eq!(d.to_core(), c);
    }

    #[test]
    fn duration_negative_sec_maps_to_zero() {
        let d = Duration::new(-1, 0);
        assert_eq!(d.to_core(), core::time::Duration::ZERO);
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_current_time_is_recent() {
        let t = get_current_time();
        assert!(!t.is_invalid());
        // sec should be > 1_700_000_000 (Nov 2023+).
        assert!(t.sec > 1_700_000_000);
    }

    // ========================================================================
    // Spec §7.5.6 (DDS-PSM-Cxx 1.0) Iron-Rule-Tracker
    // ========================================================================

    #[test]
    fn time_seconds_and_nanoseconds_accessors() {
        // Spec §7.5.6.1: Time::seconds() / nanoseconds().
        let t = Time::new(7, 250_000_000);
        assert_eq!(t.seconds(), 7);
        assert_eq!(t.nanoseconds(), 250_000_000);
    }

    #[test]
    fn duration_seconds_and_nanoseconds_accessors() {
        // Spec §7.5.6.1: Duration::seconds() / nanoseconds().
        let d = Duration::new(3, 100_000_000);
        assert_eq!(d.seconds(), 3);
        assert_eq!(d.nanoseconds(), 100_000_000);
    }

    #[test]
    fn time_add_duration_carries_seconds() {
        // Spec §7.5.6.2: Time increment via Duration with
        // nanosecond carry.
        let t = Time::new(10, 800_000_000);
        let inc = t.add_duration(Duration::new(2, 500_000_000));
        assert_eq!(inc.seconds(), 13);
        assert_eq!(inc.nanoseconds(), 300_000_000);
    }

    #[test]
    fn time_from_and_as_millis_roundtrip() {
        // Spec §7.5.6.3: conversion to/from milliseconds.
        let t = Time::from_millis(12_345);
        assert_eq!(t.seconds(), 12);
        assert_eq!(t.nanoseconds(), 345_000_000);
        assert_eq!(t.as_millis(), 12_345);
    }

    #[test]
    fn duration_add_duration_carries_seconds() {
        // Spec §7.5.6.4: Duration + Duration with nanosecond carry.
        let d = Duration::new(5, 700_000_000);
        let inc = d.add_duration(Duration::new(1, 600_000_000));
        assert_eq!(inc.seconds(), 7);
        assert_eq!(inc.nanoseconds(), 300_000_000);
    }

    /// DDS 1.4 §2.3.2: "the nanosec field must verify
    /// 0 <= nanosec < 1000000000" — 1 500 000 000 ns is 1 s and 500 000 000 ns.
    #[test]
    fn the_constructors_keep_nanosec_below_a_second() {
        let t = Time::new(10, 1_500_000_000);
        assert_eq!((t.sec, t.nanosec), (11, 500_000_000));
        assert!(t.is_normalized());
        let d = Duration::new(3, 2_000_000_000);
        assert_eq!((d.sec, d.nanosec), (5, 0));
        assert!(d.is_normalized());
        // 0xFFFFFFFF ns is 4 s and 294 967 295 ns.
        let big = Duration::new(0, 0xFFFF_FFFF);
        assert_eq!((big.sec, big.nanosec), (4, 294_967_295));
        // The seconds saturate instead of wrapping.
        assert_eq!(Time::new(i32::MAX - 1, 3_000_000_000).sec, i32::MAX);
    }

    /// §2.3.2: a QoS duration is a `Duration_t` of `sec` and `nanosec` by the
    /// conversions, and the infinite one stays infinite.
    #[test]
    fn a_qos_duration_converts_to_sec_and_nanosec_and_back() {
        let q = zerodds_qos::Duration::from_millis(1_250);
        let d = Duration::from(q);
        assert_eq!((d.sec, d.nanosec), (1, 250_000_000));
        assert_eq!(zerodds_qos::Duration::from(d), q);
        assert_eq!(
            Duration::from(zerodds_qos::Duration::INFINITE),
            Duration::INFINITE
        );
        assert_eq!(
            zerodds_qos::Duration::from(Duration::INFINITE),
            zerodds_qos::Duration::INFINITE
        );
        assert_eq!(Duration::from(zerodds_qos::Duration::ZERO), Duration::ZERO);
    }

    /// DDS 1.4 §2.3.3: `DURATION_INFINITE_SEC` and `DURATION_INFINITE_NSEC`
    /// are `0x7fffffff`, `TIME_INVALID_SEC` is -1 and `TIME_INVALID_NSEC`
    /// `0xffffffff`.
    #[test]
    fn the_reserved_values_are_those_of_the_idl() {
        assert_eq!(
            (Duration::INFINITE.sec, Duration::INFINITE.nanosec),
            (0x7fff_ffff, 0x7fff_ffff)
        );
        assert_eq!(
            (Time::INVALID.sec, Time::INVALID.nanosec),
            (-1, 0xffff_ffff)
        );
        assert_eq!((Duration::ZERO.sec, Duration::ZERO.nanosec), (0, 0));
        assert!(Time::INVALID.is_invalid());
        assert!(!Time::new(0, 0).is_invalid());
    }

    /// The reserved values of §2.3.3 carry a `nanosec` above a second by
    /// definition and are not carried.
    #[test]
    fn the_reserved_values_are_kept() {
        assert_eq!(Time::new(-1, 0xFFFF_FFFF), Time::INVALID);
        assert_eq!(Time::new(0x7FFF_FFFF, 0xFFFF_FFFE), Time::INFINITE);
        assert_eq!(Duration::new(0x7FFF_FFFF, 0x7FFF_FFFF), Duration::INFINITE);
        assert!(Time::INVALID.is_normalized());
        assert!(Time::INFINITE.is_normalized());
        assert!(Duration::INFINITE.is_normalized());
    }

    /// A value built through the public fields can be out of range; it is
    /// reported as not normalized.
    #[test]
    fn a_hand_built_value_can_be_checked() {
        assert!(
            !Time {
                sec: 1,
                nanosec: 1_000_000_000
            }
            .is_normalized()
        );
        assert!(
            Time {
                sec: 1,
                nanosec: 999_999_999
            }
            .is_normalized()
        );
        assert!(
            !Duration {
                sec: 1,
                nanosec: 1_000_000_000
            }
            .is_normalized()
        );
    }

    /// 1 ms before the epoch is -1 s and 999 000 000 ns (§2.3.2), not a
    /// `nanosec` of -1 000 000 read as unsigned.
    #[test]
    fn from_millis_of_a_negative_value_is_normalized() {
        let t = Time::from_millis(-1);
        assert_eq!((t.sec, t.nanosec), (-1, 999_000_000));
        assert_eq!(t.as_millis(), -1);
        let d = Duration::from_millis(-1_500);
        assert_eq!((d.sec, d.nanosec), (-2, 500_000_000));
        assert_eq!(d.as_millis(), -1_500);
    }

    #[test]
    fn duration_from_and_as_millis_roundtrip() {
        // Spec §7.5.6.5: conversion to/from milliseconds.
        let d = Duration::from_millis(2_500);
        assert_eq!(d.seconds(), 2);
        assert_eq!(d.nanoseconds(), 500_000_000);
        assert_eq!(d.as_millis(), 2_500);
    }
}
