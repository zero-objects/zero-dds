// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP Flags-Octet — Spec §15.4.1.
//!
//! GIOP 1.0 nutzt das 7. Header-Byte als reines `byte_order`-Octet
//! (0=BE, 1=LE). Ab GIOP 1.1 ist es ein Bitfield:
//!
//! * Bit 0 — `byte_order` (0=BE, 1=LE).
//! * Bit 1 — `fragment_bit` (1 = mehr Fragments folgen).
//! * Bits 2..7 — reserviert (Spec §15.4.1: "set to 0").

use zerodds_cdr::Endianness;

/// GIOP-Flags-Octet (Spec §15.4.1 ab GIOP 1.1; in GIOP 1.0 nur
/// `byte_order_bit` benutzt).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Flags(pub u8);

impl Flags {
    /// Bit 0 — `byte_order` (1 = Little-Endian).
    pub const BYTE_ORDER_BIT: u8 = 0b0000_0001;
    /// Bit 1 — `fragment_bit` (1 = mehr Fragments folgen).
    pub const FRAGMENT_BIT: u8 = 0b0000_0010;

    /// Konstruiert Flags aus einer [`Endianness`].
    #[must_use]
    pub const fn from_endianness(e: Endianness) -> Self {
        match e {
            Endianness::Big => Self(0),
            Endianness::Little => Self(Self::BYTE_ORDER_BIT),
        }
    }

    /// Liefert die [`Endianness`] aus dem Bit-0.
    #[must_use]
    pub const fn endianness(self) -> Endianness {
        if self.0 & Self::BYTE_ORDER_BIT != 0 {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }

    /// `true` wenn das Fragment-Bit gesetzt ist.
    #[must_use]
    pub const fn has_more_fragments(self) -> bool {
        self.0 & Self::FRAGMENT_BIT != 0
    }

    /// Setzt das Fragment-Bit.
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
