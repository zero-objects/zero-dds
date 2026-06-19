// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP flags octet — spec §15.4.1.
//!
//! GIOP 1.0 uses the 7th header byte as a pure `byte_order` octet
//! (0=BE, 1=LE). As of GIOP 1.1 it is a bitfield:
//!
//! * Bit 0 — `byte_order` (0=BE, 1=LE).
//! * Bit 1 — `fragment_bit` (1 = more fragments follow).
//! * Bits 2..7 — reserved (spec §15.4.1: "set to 0").

use zerodds_cdr::Endianness;

/// GIOP flags octet (spec §15.4.1 as of GIOP 1.1; in GIOP 1.0 only
/// `byte_order_bit` is used).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Flags(pub u8);

impl Flags {
    /// Bit 0 — `byte_order` (1 = Little-Endian).
    pub const BYTE_ORDER_BIT: u8 = 0b0000_0001;
    /// Bit 1 — `fragment_bit` (1 = more fragments follow).
    pub const FRAGMENT_BIT: u8 = 0b0000_0010;

    /// Constructs flags from an [`Endianness`].
    #[must_use]
    pub const fn from_endianness(e: Endianness) -> Self {
        match e {
            Endianness::Big => Self(0),
            Endianness::Little => Self(Self::BYTE_ORDER_BIT),
        }
    }

    /// Returns the [`Endianness`] from bit 0.
    #[must_use]
    pub const fn endianness(self) -> Endianness {
        if self.0 & Self::BYTE_ORDER_BIT != 0 {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }

    /// `true` if the fragment bit is set.
    #[must_use]
    pub const fn has_more_fragments(self) -> bool {
        self.0 & Self::FRAGMENT_BIT != 0
    }

    /// Sets the fragment bit.
    #[must_use]
    pub const fn with_fragment(mut self, more: bool) -> Self {
        if more {
            self.0 |= Self::FRAGMENT_BIT;
        } else {
            self.0 &= !Self::FRAGMENT_BIT;
        }
        self
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn from_endianness_round_trip() {
        assert_eq!(Flags::from_endianness(Endianness::Big).0, 0);
        assert_eq!(
            Flags::from_endianness(Endianness::Little).0,
            Flags::BYTE_ORDER_BIT
        );
        assert_eq!(
            Flags::from_endianness(Endianness::Little).endianness(),
            Endianness::Little
        );
    }

    #[test]
    fn fragment_bit_default_off() {
        assert!(!Flags::default().has_more_fragments());
    }

    #[test]
    fn with_fragment_sets_and_clears_bit() {
        let f = Flags::default().with_fragment(true);
        assert!(f.has_more_fragments());
        let g = f.with_fragment(false);
        assert!(!g.has_more_fragments());
    }

    #[test]
    fn fragment_bit_value_matches_spec() {
        // Spec §15.4.1: Bit 1 = 0x02.
        assert_eq!(Flags::FRAGMENT_BIT, 0x02);
    }

    #[test]
    fn byte_order_bit_value_matches_spec() {
        // Spec §15.4.1: Bit 0 = 0x01.
        assert_eq!(Flags::BYTE_ORDER_BIT, 0x01);
    }

    #[test]
    fn fragment_and_byte_order_can_combine() {
        let f = Flags(Flags::BYTE_ORDER_BIT | Flags::FRAGMENT_BIT);
        assert_eq!(f.endianness(), Endianness::Little);
        assert!(f.has_more_fragments());
    }
}
