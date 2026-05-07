// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Endianness-Konfiguration fuer XCDR-Streams (XCDR2 §7.4.1).
//!
//! In XCDR ist die Endianness ein **Stream-Property** — sie wird einmal
//! im RTPS-Header oder durch die `EncapsulationKind`-Markierung gesetzt
//! und gilt fuer den gesamten payload. Encoder und Decoder werden mit
//! der Endianness konstruiert; primitive Konvertierungen erfolgen
//! anhand dieses Settings.

/// Byte-Order eines CDR-Streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Endianness {
    /// Big-Endian (Network-Byte-Order). Encapsulation `CDR2_BE` bzw.
    /// `PL_CDR2_BE` in OMG-XTypes Tabelle 7-37.
    Big,
    /// Little-Endian. Encapsulation `CDR2_LE` bzw. `PL_CDR2_LE`.
    /// Default fuer die meisten DDS-Implementierungen auf x86/ARM.
    #[default]
    Little,
}

impl Endianness {
    /// Konvertiert ein `u16` in 2 Byte gemaess der Endianness.
    #[must_use]
    pub fn write_u16(self, value: u16) -> [u8; 2] {
        match self {
            Self::Big => value.to_be_bytes(),
            Self::Little => value.to_le_bytes(),
        }
    }

    /// Konvertiert ein `u32` in 4 Byte.
    #[must_use]
    pub fn write_u32(self, value: u32) -> [u8; 4] {
        match self {
            Self::Big => value.to_be_bytes(),
            Self::Little => value.to_le_bytes(),
        }
    }

    /// Konvertiert ein `u64` in 8 Byte.
    #[must_use]
    pub fn write_u64(self, value: u64) -> [u8; 8] {
        match self {
            Self::Big => value.to_be_bytes(),
            Self::Little => value.to_le_bytes(),
        }
    }

    /// Liest ein `u16` aus 2 Byte gemaess der Endianness.
    #[must_use]
    pub fn read_u16(self, bytes: [u8; 2]) -> u16 {
        match self {
            Self::Big => u16::from_be_bytes(bytes),
            Self::Little => u16::from_le_bytes(bytes),
        }
    }

    /// Liest ein `u32` aus 4 Byte.
    #[must_use]
    pub fn read_u32(self, bytes: [u8; 4]) -> u32 {
        match self {
            Self::Big => u32::from_be_bytes(bytes),
            Self::Little => u32::from_le_bytes(bytes),
        }
    }

    /// Liest ein `u64` aus 8 Byte.
    #[must_use]
    pub fn read_u64(self, bytes: [u8; 8]) -> u64 {
        match self {
            Self::Big => u64::from_be_bytes(bytes),
            Self::Little => u64::from_le_bytes(bytes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_little() {
        assert_eq!(Endianness::default(), Endianness::Little);
    }

    #[test]
    fn u16_roundtrip_be() {
        let bytes = Endianness::Big.write_u16(0x1234);
        assert_eq!(bytes, [0x12, 0x34]);
        assert_eq!(Endianness::Big.read_u16(bytes), 0x1234);
    }

    #[test]
    fn u16_roundtrip_le() {
        let bytes = Endianness::Little.write_u16(0x1234);
        assert_eq!(bytes, [0x34, 0x12]);
        assert_eq!(Endianness::Little.read_u16(bytes), 0x1234);
    }

    #[test]
    fn u32_roundtrip_be() {
        let bytes = Endianness::Big.write_u32(0xDEAD_BEEF);
        assert_eq!(bytes, [0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(Endianness::Big.read_u32(bytes), 0xDEAD_BEEF);
    }

    #[test]
    fn u32_roundtrip_le() {
        let bytes = Endianness::Little.write_u32(0xDEAD_BEEF);
        assert_eq!(bytes, [0xEF, 0xBE, 0xAD, 0xDE]);
        assert_eq!(Endianness::Little.read_u32(bytes), 0xDEAD_BEEF);
    }

    #[test]
    fn u64_roundtrip_be() {
        let v: u64 = 0x0102_0304_0506_0708;
        let bytes = Endianness::Big.write_u64(v);
        assert_eq!(bytes, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(Endianness::Big.read_u64(bytes), v);
    }

    #[test]
    fn u64_roundtrip_le() {
        let v: u64 = 0x0102_0304_0506_0708;
        let bytes = Endianness::Little.write_u64(v);
        assert_eq!(bytes, [8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(Endianness::Little.read_u64(bytes), v);
    }

    #[test]
    fn endianness_is_copy_and_eq() {
        let e = Endianness::Big;
        let copy = e;
        assert_eq!(e, copy);
    }
}
