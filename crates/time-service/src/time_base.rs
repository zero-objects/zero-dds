// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `TimeBase`-Modul aus OMG Time Service 1.1 §1.3.2.
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

/// `TimeBase::TimeT` (Spec §1.3.2.1) — 64-bit Tick-Counter mit
/// 100-Nanosekunden-Aufloesung. Fuer absolute Zeit ist die Basis
/// `15 October 1582 00:00:00` (Gregorianischer Kalender). Fuer
/// relative Zeit ist die Basis kontext-abhaengig.
pub type TimeT = u64;

/// `TimeBase::InaccuracyT` (Spec §1.3.2.2) — 48-bit Wert in
/// 100-Nanosekunden-Einheiten. Wir packen ihn als `u64` und
/// erzwingen Range bei `pack`/`unpack` (siehe [`UtcT::set_inaccuracy`]).
pub type InaccuracyT = u64;

/// `TimeBase::TdfT` (Spec §1.3.2.3) — 16-bit signed short, gibt
/// die Time-Displacement-Factor in Minuten an (Greenwich = 0,
/// East = positiv, West = negativ).
pub type TdfT = i16;

/// 100-Nanosekunden-Schritte zwischen 15 October 1582 00:00:00 (UTC)
/// und 1 January 1970 00:00:00 (UNIX-Epoch).
///
/// Wert berechnet aus 141,427 Tagen: 141_427 * 24 * 60 * 60 * 10_000_000.
pub const UTC_EPOCH_TO_UNIX_TICKS: TimeT = 122_192_928_000_000_000;

/// Anzahl 100ns-Ticks pro Sekunde.
pub const TICKS_PER_SECOND: u64 = 10_000_000;

/// `TimeBase::UtcT` (Spec §1.3.2.4) — Universal-Time-Coordinated
/// Struct mit 16 Bytes Wire-Format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UtcT {
    /// Time-Wert (TimeT). 100ns-Ticks since 1582 fuer absolute Zeit
    /// bzw. relativer Wert fuer Duration-Form.
    pub time: TimeT,
    /// Niedrige 32 Bit der Inaccuracy (`inacclo`).
    pub inacclo: u32,
    /// Hohe 16 Bit der Inaccuracy (`inacchi`).
    pub inacchi: u16,
    /// Time-Displacement-Factor (`tdf`).
    pub tdf: TdfT,
}

impl UtcT {
    /// Konstruktor mit allen Spec-Feldern. Inaccuracy wird auf 48 bit
    /// gekappt (Spec §1.3.2.4).
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

    /// Liefert die Inaccuracy als 48-bit-zusammengesetzten Wert
    /// (`inacchi` << 32 | `inacclo`). Spec §1.3.2.2 + §1.3.2.4.
    #[must_use]
    pub const fn inaccuracy(self) -> InaccuracyT {
        ((self.inacchi as u64) << 32) | (self.inacclo as u64)
    }

    /// Setzt Inaccuracy (kappt auf 48 bit).
    pub const fn set_inaccuracy(&mut self, value: InaccuracyT) {
        let v = value & 0x0000_FFFF_FFFF_FFFF;
        self.inacclo = (v & 0xFFFF_FFFF) as u32;
        self.inacchi = ((v >> 32) & 0xFFFF) as u16;
    }

    /// Spec §1.3.2.4 — UTC-time + tdf*600,000,000 ergibt die lokale
    /// Zeit (in 100ns-Ticks). Liefert die lokale Zeit-TimeT.
    /// 600_000_000 = 60 sec * 10_000_000 ticks/sec.
    #[must_use]
    pub const fn local_time(self) -> TimeT {
        let tdf_ticks = (self.tdf as i64) * 600_000_000;
        // Sicheres `wrapping_add` analog Spec-Vorgabe (saturate ist
        // Spec-fremd; wir folgen exakt der Formel).
        let signed = self.time as i64;
        signed.wrapping_add(tdf_ticks) as TimeT
    }

    /// Encode als 16-Byte Wire-Form (TimeT 8B little-endian, inacclo 4B,
    /// inacchi 2B, tdf 2B). Reihenfolge gemaess Spec-Struct-Layout.
    #[must_use]
    pub fn to_wire(self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.time.to_le_bytes());
        buf[8..12].copy_from_slice(&self.inacclo.to_le_bytes());
        buf[12..14].copy_from_slice(&self.inacchi.to_le_bytes());
        buf[14..16].copy_from_slice(&self.tdf.to_le_bytes());
        buf
    }

    /// Decode aus 16-Byte Wire-Form.
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

/// `TimeBase::IntervalT` (Spec §1.3.2.5) — Zeit-Intervall mit
/// `lower_bound` und `upper_bound`. Lower > Upper ist invalid und
/// wird beim Konstruieren rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IntervalT {
    /// Untere Grenze.
    pub lower_bound: TimeT,
    /// Obere Grenze.
    pub upper_bound: TimeT,
}

impl IntervalT {
    /// Spec §1.3.2.5: `lower_bound > upper_bound` ist invalid.
    /// Liefert `None` in diesem Fall.
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

    /// Encode als 16-Byte (2x TimeT little-endian).
    #[must_use]
    pub fn to_wire(self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.lower_bound.to_le_bytes());
        buf[8..16].copy_from_slice(&self.upper_bound.to_le_bytes());
        buf
    }

    /// Decode aus 16-Byte Wire.
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

/// Hilfsfunktion: liefert die aktuelle Zeit als `TimeT`-Wert (100ns-
/// Ticks since 1582). Auf `std`-Plattformen via `SystemTime`. Auf
/// `no_std` ist diese Funktion nicht verfuegbar.
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

/// `no_std`-Stub: liefert immer 0. Auf `no_std` gibt es keine
/// Wall-Clock; eine echte Zeitquelle wird vom Embedding via
/// `TimeService::with_source(...)` injiziert. Wer `current_time()`
/// direkt aufruft, bekommt `0`, was im [`TimeService::universal_time`]
/// zu `TimeUnavailable` fuehrt.
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
        // Spec §1.3.2.4 — UtcT-Layout-Total-Groesse ist 16 Octets.
        let utc = UtcT::new(0x0123_4567_89AB_CDEF, 0xFFFF_FFFF_FFFF, 60);
        let wire = utc.to_wire();
        assert_eq!(wire.len(), 16);
    }

    #[test]
    fn intervalt_size_is_16_octets() {
        // Spec §1.3.2.5 — IntervalT = 2x TimeT = 16 Octets.
        let i = IntervalT::new(0, 0).expect("ok");
        let wire = i.to_wire();
        assert_eq!(wire.len(), 16);
    }

    #[test]
    fn inaccuracy_caps_at_48_bits() {
        // Spec §1.3.2.2 + §1.3.2.4 — Inaccuracy hat 48 bit.
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
        // Spec §1.3.2.5: lower > upper ist invalid.
        assert!(IntervalT::new(200, 100).is_none());
    }

    #[test]
    fn local_time_applies_tdf() {
        // Spec §1.3.2.4: utc.time + utc.tdf * 600_000_000 == lokale Zeit.
        // tdf = 60 (Berlin Sommerzeit, +60 Minuten); ticks/min =
        // 60 sec * 10_000_000 = 600_000_000.
        let utc = UtcT::new(1_000_000_000, 0, 60);
        let expected = 1_000_000_000_i64 + 60 * 600_000_000;
        assert_eq!(utc.local_time(), expected as TimeT);
    }

    #[test]
    fn local_time_negative_tdf_west_of_greenwich() {
        // Spec §1.3.2.3 — westlich Greenwich = negativ.
        let utc = UtcT::new(1_000_000_000_000, 0, -480); // PST (-8h)
        let expected = 1_000_000_000_000_i64 + (-480) * 600_000_000;
        assert_eq!(utc.local_time(), expected as TimeT);
    }

    #[cfg(feature = "std")]
    #[test]
    fn current_time_is_recent_century() {
        // Sanity: current_time gibt Wert in plausiblem Bereich
        // (nach 2020, vor 2200). Tick-Range fuer 2020-01-01 ist
        // (2020 - 1582 = 438 Jahre) * 365.25 * 24 * 60 * 60 * 10_000_000
        // ≈ 1.38e17.
        let t = current_time();
        assert!(t > 130_000_000_000_000_000);
        assert!(t < 200_000_000_000_000_000);
    }
}
