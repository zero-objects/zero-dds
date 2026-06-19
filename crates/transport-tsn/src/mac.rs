// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IEEE 802 MAC address model — Spec §7.2.3 Tab 7.20.

/// 48-bit IEEE 802 MAC address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    /// IEEE 802 broadcast MAC (`FF:FF:FF:FF:FF:FF`).
    pub const BROADCAST: Self = Self([0xFF; 6]);

    /// Constructor.
    #[must_use]
    pub const fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    /// IEEE 802.3 — multicast if the LSB of the first octet is set.
    #[must_use]
    pub const fn is_multicast(self) -> bool {
        (self.0[0] & 0x01) != 0
    }

    /// Spec IEEE 802 — locally administered if the 2nd LSB of the first
    /// octet is set.
    #[must_use]
    pub const fn is_locally_administered(self) -> bool {
        (self.0[0] & 0x02) != 0
    }

    /// `true` for FF:FF:FF:FF:FF:FF.
    #[must_use]
    pub const fn is_broadcast(self) -> bool {
        let b = self.0;
        b[0] == 0xFF && b[1] == 0xFF && b[2] == 0xFF && b[3] == 0xFF && b[4] == 0xFF && b[5] == 0xFF
    }
}

impl core::fmt::Display for MacAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let b = self.0;
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            b[0], b[1], b[2], b[3], b[4], b[5]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_constant_is_all_ff() {
        assert_eq!(MacAddress::BROADCAST.0, [0xFF; 6]);
        assert!(MacAddress::BROADCAST.is_broadcast());
        assert!(MacAddress::BROADCAST.is_multicast());
    }

    #[test]
    fn unicast_lsb_zero_is_not_multicast() {
        // First byte even → unicast.
        let m = MacAddress::new([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
        assert!(!m.is_multicast());
    }

    #[test]
    fn multicast_lsb_set_detected() {
        // First byte odd → multicast.
        let m = MacAddress::new([0x01, 0x00, 0x5E, 0x00, 0x00, 0x01]);
        assert!(m.is_multicast());
        assert!(!m.is_broadcast());
    }

    #[test]
    fn locally_administered_2nd_bit_set() {
        let m = MacAddress::new([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
        assert!(m.is_locally_administered());
        let global = MacAddress::new([0x00, 0x00, 0x00, 0x00, 0x00, 0x01]);
        assert!(!global.is_locally_administered());
    }

    #[test]
    fn display_format_matches_canonical_colon_separated_hex() {
        let m = MacAddress::new([0xDE, 0xAD, 0xBE, 0xEF, 0x12, 0x34]);
        assert_eq!(alloc::format!("{m}"), "DE:AD:BE:EF:12:34");
    }
}
