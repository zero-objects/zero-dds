// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Differentiated Services Code Point — RFC 2474.
//!
//! Spec §3 (Normative References) — RFC 2474 + RFC 3290 + RFC 8939
//! (DetNet) referenziert. DSCP wird im IPv4-ToS-Octet bzw. IPv6-
//! Traffic-Class-Octet als 6-bit-Wert kodiert.

/// Differentiated Services Code Point (6 Bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dscp(u8);

impl Dscp {
    /// `0` Default-Forwarding (RFC 2474 §4.1).
    pub const DEFAULT: Self = Self(0);
    /// `46` Expedited-Forwarding (RFC 3246) — typisch fuer
    /// Real-Time-Streams.
    pub const EXPEDITED_FORWARDING: Self = Self(46);
    /// `10` Assured-Forwarding 11 (low-drop, AF1x family).
    pub const AF11: Self = Self(10);
    /// `18` Assured-Forwarding 21.
    pub const AF21: Self = Self(18);
    /// `26` Assured-Forwarding 31.
    pub const AF31: Self = Self(26);
    /// `34` Assured-Forwarding 41.
    pub const AF41: Self = Self(34);

    /// Konstruktor mit Validation.
    ///
    /// # Errors
    /// `Err(())` wenn `value > 63` (DSCP ist 6-bit).
    pub const fn new(value: u8) -> Result<Self, DscpError> {
        if value > 63 {
            return Err(DscpError::OutOfRange(value));
        }
        Ok(Self(value))
    }

    /// Spec — DSCP-Wert (0..=63).
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }

    /// Wandelt zu IPv4-ToS-Octet (DSCP in den oberen 6 Bits, ECN in
    /// den unteren 2 Bits = 0).
    #[must_use]
    pub const fn to_tos_octet(self) -> u8 {
        self.0 << 2
    }

    /// Decodet vom IPv4-ToS-Octet (obere 6 Bits = DSCP, untere 2 Bits
    /// ECN ignoriert).
    #[must_use]
    pub const fn from_tos_octet(tos: u8) -> Self {
        Self((tos >> 2) & 0x3F)
    }
}

/// DSCP-Validation-Fehler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DscpError {
    /// Wert > 63.
    OutOfRange(u8),
}

impl core::fmt::Display for DscpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OutOfRange(v) => write!(f, "DSCP {v} > 63"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DscpError {}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn dscp_well_known_constants_match_rfc() {
        // RFC 2474/3246.
        assert_eq!(Dscp::DEFAULT.value(), 0);
        assert_eq!(Dscp::EXPEDITED_FORWARDING.value(), 46);
        assert_eq!(Dscp::AF11.value(), 10);
        assert_eq!(Dscp::AF41.value(), 34);
    }

    #[test]
    fn dscp_above_63_rejected() {
        assert_eq!(Dscp::new(64), Err(DscpError::OutOfRange(64)));
        assert_eq!(Dscp::new(255), Err(DscpError::OutOfRange(255)));
    }

    #[test]
    fn tos_octet_round_trip_preserves_dscp() {
        for v in 0u8..=63 {
            let d = Dscp::new(v).expect("ok");
            let tos = d.to_tos_octet();
            assert_eq!(tos & 0xFC, v << 2);
            assert_eq!(Dscp::from_tos_octet(tos).value(), v);
        }
    }

    #[test]
    fn from_tos_ignores_ecn_bits() {
        // EF (46) | ECN bits 11.
        let tos = (46u8 << 2) | 0x03;
        assert_eq!(Dscp::from_tos_octet(tos).value(), 46);
    }
}
