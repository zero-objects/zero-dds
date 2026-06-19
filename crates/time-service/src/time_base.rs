// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `TimeBase` module from OMG Time Service 1.1 §1.3.2.
//!
//! ```idl
//! module TimeBase {
//!     typedef unsigned long long TimeT;
//!     typedef TimeT             InaccuracyT;
//!     typedef short             TdfT;
//!     struct UtcT {
//!         TimeT       time;        // 8 octets
//!         unsigned long inacclo;   // 4 octets
//!         unsigned short inacchi;  // 2 octets
//!         TdfT        tdf;         // 2 octets
//!         // total 16 octets.
//!     };
//!     struct IntervalT {
//!         TimeT       lower_bound;
//!         TimeT       upper_bound;
//!     };
//! };
//! ```

/// `TimeBase::TimeT` (spec §1.3.2.1) — 64-bit tick counter with
/// 100-nanosecond resolution. For absolute time the base is
/// `15 October 1582 00:00:00` (Gregorian calendar). For
/// relative time the base is context-dependent.
pub type TimeT = u64;

/// `TimeBase::InaccuracyT` (spec §1.3.2.2) — 48-bit value in
/// 100-nanosecond units. We pack it as a `u64` and
/// enforce the range on `pack`/`unpack` (see [`UtcT::set_inaccuracy`]).
pub type InaccuracyT = u64;

/// `TimeBase::TdfT` (spec §1.3.2.3) — 16-bit signed short, gives
/// the time-displacement factor in minutes (Greenwich = 0,
/// East = positive, West = negative).
pub type TdfT = i16;

/// 100-nanosecond steps between 15 October 1582 00:00:00 (UTC)
/// and 1 January 1970 00:00:00 (UNIX epoch).
///
/// Value computed from 141,427 days: 141_427 * 24 * 60 * 60 * 10_000_000.
pub const UTC_EPOCH_TO_UNIX_TICKS: TimeT = 122_192_928_000_000_000;

/// Number of 100ns ticks per second.
pub const TICKS_PER_SECOND: u64 = 10_000_000;

/// `TimeBase::UtcT` (spec §1.3.2.4) — Universal-Time-Coordinated
/// struct with a 16-byte wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UtcT {
    /// Time value (TimeT). 100ns ticks since 1582 for absolute time
    /// or a relative value for the duration form.
    pub time: TimeT,
    /// Low 32 bits of the inaccuracy (`inacclo`).
    pub inacclo: u32,
    /// High 16 bits of the inaccuracy (`inacchi`).
    pub inacchi: u16,
    /// Time-displacement factor (`tdf`).
    pub tdf: TdfT,
}

impl UtcT {
    /// Constructor with all spec fields. The inaccuracy is capped to
    /// 48 bits (spec §1.3.2.4).
    #[must_use]
    pub const fn new(time: TimeT, inaccuracy: InaccuracyT, tdf: TdfT) -> Self {
        let inacc = inaccuracy & 0x0000_FFFF_FFFF_FFFF;
        Self {
            time,
            inacclo: (inacc & 0xFFFF_FFFF) as u32,
            inacchi: ((inacc >> 32) & 0xFFFF) as u16,
            tdf,
        }
    }

    /// Returns the inaccuracy as a 48-bit composite value
    /// (`inacchi` << 32 | `inacclo`). Spec §1.3.2.2 + §1.3.2.4.
    #[must_use]
    pub const fn inaccuracy(self) -> InaccuracyT {
        ((self.inacchi as u64) << 32) | (self.inacclo as u64)
    }

    /// Sets the inaccuracy (clamps to 48 bits).
    pub const fn set_inaccuracy(&mut self, value: InaccuracyT) {
        let v = value & 0x0000_FFFF_FFFF_FFFF;
        self.inacclo = (v & 0xFFFF_FFFF) as u32;
        self.inacchi = ((v >> 32) & 0xFFFF) as u16;
    }

    /// Spec §1.3.2.4 — UTC time + tdf*600,000,000 yields the local
    /// time (in 100ns ticks). Returns the local-time TimeT.
    /// 600_000_000 = 60 sec * 10_000_000 ticks/sec.
    #[must_use]
    pub const fn local_time(self) -> TimeT {
        let tdf_ticks = (self.tdf as i64) * 600_000_000;
        // Safe `wrapping_add` per the spec mandate (saturate is
        // non-spec; we follow the formula exactly).
        let signed = self.time as i64;
        signed.wrapping_add(tdf_ticks) as TimeT
    }

    /// Encode as the 16-byte wire form (TimeT 8B little-endian, inacclo 4B,
    /// inacchi 2B, tdf 2B). Order per the spec struct layout.
    #[must_use]
    pub fn to_wire(self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.time.to_le_bytes());
        buf[8..12].copy_from_slice(&self.inacclo.to_le_bytes());
        buf[12..14].copy_from_slice(&self.inacchi.to_le_bytes());
        buf[14..16].copy_from_slice(&self.tdf.to_le_bytes());
        buf
    }

    /// Decode from the 16-byte wire form.
    #[must_use]
    pub fn from_wire(buf: [u8; 16]) -> Self {
        let mut time_b = [0u8; 8];
        time_b.copy_from_slice(&buf[0..8]);
        let mut lo = [0u8; 4];
        lo.copy_from_slice(&buf[8..12]);
        let mut hi = [0u8; 2];
        hi.copy_from_slice(&buf[12..14]);
        let mut tdf = [0u8; 2];
        tdf.copy_from_slice(&buf[14..16]);
        Self {
            time: TimeT::from_le_bytes(time_b),
            inacclo: u32::from_le_bytes(lo),
            inacchi: u16::from_le_bytes(hi),
            tdf: TdfT::from_le_bytes(tdf),
        }
    }
}

/// `TimeBase::IntervalT` (spec §1.3.2.5) — time interval with
/// `lower_bound` and `upper_bound`. Lower > upper is invalid and
/// is rejected on construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IntervalT {
    /// Lower bound.
    pub lower_bound: TimeT,
    /// Upper bound.
    pub upper_bound: TimeT,
}

impl IntervalT {
    /// Spec §1.3.2.5: `lower_bound > upper_bound` is invalid.
    /// Returns `None` in that case.
    #[must_use]
    pub const fn new(lower_bound: TimeT, upper_bound: TimeT) -> Option<Self> {
        if lower_bound > upper_bound {
            None
        } else {
            Some(Self {
                lower_bound,
                upper_bound,
            })
        }
    }

    /// Encode as 16 bytes (2x TimeT little-endian).
    #[must_use]
    pub fn to_wire(self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.lower_bound.to_le_bytes());
        buf[8..16].copy_from_slice(&self.upper_bound.to_le_bytes());
        buf
    }

    /// Decode from 16-byte wire.
    #[must_use]
    pub fn from_wire(buf: [u8; 16]) -> Self {
        let mut lo = [0u8; 8];
        lo.copy_from_slice(&buf[0..8]);
        let mut hi = [0u8; 8];
        hi.copy_from_slice(&buf[8..16]);
        Self {
            lower_bound: TimeT::from_le_bytes(lo),
            upper_bound: TimeT::from_le_bytes(hi),
        }
    }
}

/// Helper: returns the current time as a `TimeT` value (100ns
/// ticks since 1582). On `std` platforms via `SystemTime`. On
/// `no_std` this function is not available.
#[cfg(feature = "std")]
#[must_use]
pub fn current_time() -> TimeT {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => {
            let nanos = d.as_secs() as u128 * 1_000_000_000 + d.subsec_nanos() as u128;
            let ticks_since_unix = (nanos / 100) as u64;
            UTC_EPOCH_TO_UNIX_TICKS.wrapping_add(ticks_since_unix)
        }
        Err(_) => 0,
    }
}

/// `no_std` stub: always returns 0. On `no_std` there is no
/// wall clock; a real time source is injected by the embedding via
/// `TimeService::with_source(...)`. Whoever calls `current_time()`
/// directly gets `0`, which in [`TimeService::universal_time`]
/// leads to `TimeUnavailable`.
#[cfg(not(feature = "std"))]
#[must_use]
pub fn current_time() -> TimeT {
    0
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn utct_size_is_16_octets() {
        // Spec §1.3.2.4 — the total UtcT layout size is 16 octets.
        let utc = UtcT::new(0x0123_4567_89AB_CDEF, 0xFFFF_FFFF_FFFF, 60);
        let wire = utc.to_wire();
        assert_eq!(wire.len(), 16);
    }

    #[test]
    fn intervalt_size_is_16_octets() {
        // Spec §1.3.2.5 — IntervalT = 2x TimeT = 16 octets.
        let i = IntervalT::new(0, 0).expect("ok");
        let wire = i.to_wire();
        assert_eq!(wire.len(), 16);
    }

    #[test]
    fn inaccuracy_caps_at_48_bits() {
        // Spec §1.3.2.2 + §1.3.2.4 — inaccuracy has 48 bits.
        let utc = UtcT::new(0, u64::MAX, 0);
        assert_eq!(utc.inaccuracy(), 0x0000_FFFF_FFFF_FFFF);
    }

    #[test]
    fn utct_wire_roundtrip_preserves_all_fields() {
        let original = UtcT::new(123_456_789_012_345_678, 0xAABB_CCDD_EEFF, -480);
        let wire = original.to_wire();
        let decoded = UtcT::from_wire(wire);
        assert_eq!(decoded, original);
    }

    #[test]
    fn intervalt_wire_roundtrip_preserves_bounds() {
        let i = IntervalT::new(100, 200).expect("ok");
        let decoded = IntervalT::from_wire(i.to_wire());
        assert_eq!(decoded.lower_bound, 100);
        assert_eq!(decoded.upper_bound, 200);
    }

    #[test]
    fn intervalt_rejects_lower_greater_than_upper() {
        // Spec §1.3.2.5: lower > upper is invalid.
        assert!(IntervalT::new(200, 100).is_none());
    }

    #[test]
    fn local_time_applies_tdf() {
        // Spec §1.3.2.4: utc.time + utc.tdf * 600_000_000 == local time.
        // tdf = 60 (Berlin summer time, +60 minutes); ticks/min =
        // 60 sec * 10_000_000 = 600_000_000.
        let utc = UtcT::new(1_000_000_000, 0, 60);
        let expected = 1_000_000_000_i64 + 60 * 600_000_000;
        assert_eq!(utc.local_time(), expected as TimeT);
    }

    #[test]
    fn local_time_negative_tdf_west_of_greenwich() {
        // Spec §1.3.2.3 — west of Greenwich = negative.
        let utc = UtcT::new(1_000_000_000_000, 0, -480); // PST (-8h)
        let expected = 1_000_000_000_000_i64 + (-480) * 600_000_000;
        assert_eq!(utc.local_time(), expected as TimeT);
    }

    #[cfg(feature = "std")]
    #[test]
    fn current_time_is_recent_century() {
        // Sanity: current_time returns a value in a plausible range
        // (after 2020, before 2200). The tick range for 2020-01-01 is
        // (2020 - 1582 = 438 Jahre) * 365.25 * 24 * 60 * 60 * 10_000_000
        // ≈ 1.38e17.
        let t = current_time();
        assert!(t > 130_000_000_000_000_000);
        assert!(t < 200_000_000_000_000_000);
    }
}
