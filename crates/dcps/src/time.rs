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

    /// Constructor.
    #[must_use]
    pub const fn new(sec: i32, nanosec: u32) -> Self {
        Self { sec, nanosec }
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
        let sec = (ms / 1000) as i32;
        let nanosec = ((ms % 1000) * 1_000_000) as u32;
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

impl Duration {
    /// `DURATION_ZERO` (spec): 0 seconds.
    pub const ZERO: Self = Self { sec: 0, nanosec: 0 };

    /// `DURATION_INFINITE` (spec): symbolic maximum.
    pub const INFINITE: Self = Self {
        sec: 0x7FFF_FFFF,
        nanosec: 0xFFFF_FFFF,
    };

    /// Constructor.
    #[must_use]
    pub const fn new(sec: i32, nanosec: u32) -> Self {
        Self { sec, nanosec }
    }

    /// `true` if the value is [`Self::INFINITE`].
    #[must_use]
    pub const fn is_infinite(&self) -> bool {
        self.sec == 0x7FFF_FFFF && self.nanosec == 0xFFFF_FFFF
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
        let sec = (ms / 1000) as i32;
        let nanosec = ((ms % 1000) * 1_000_000) as u32;
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

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

    #[test]
    fn duration_from_and_as_millis_roundtrip() {
        // Spec §7.5.6.5: conversion to/from milliseconds.
        let d = Duration::from_millis(2_500);
        assert_eq!(d.seconds(), 2);
        assert_eq!(d.nanoseconds(), 500_000_000);
        assert_eq!(d.as_millis(), 2_500);
    }
}
